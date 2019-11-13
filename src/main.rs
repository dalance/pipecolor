mod colorize;
mod read_timeout;

use atty::Stream;
use colorize::{colorize, Config};
use error_chain::{error_chain, quick_main};
use nix::unistd::Pid;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    any(target_env = "gnu", target_env = "musl")
))]
use proc_reader::ProcReader;
use read_timeout::read_line_timeout;
use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use structopt::{clap, StructOpt};
use timeout_readwrite::TimeoutReader;

// -------------------------------------------------------------------------------------------------
// Option
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "pipecolor")]
#[structopt(raw(
    long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"
))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Files to show
    #[structopt(name = "FILE", parse(from_os_str))]
    pub files: Vec<PathBuf>,

    /// Colorize mode
    #[structopt(
        short = "m",
        long = "mode",
        default_value = "auto",
        possible_value = "auto",
        possible_value = "always",
        possible_value = "disable"
    )]
    pub mode: String,

    /// Config file
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    pub config: Option<PathBuf>,

    /// Timeout of stdin by milliseconds
    #[structopt(short = "t", long = "timeout", default_value = "500")]
    pub timeout: u64,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,

    /// Attach to the specified process
    #[structopt(short = "p", long = "process", conflicts_with = "FILE")]
    pub process: Option<i32>,
}

// -------------------------------------------------------------------------------------------------
// Config
// -------------------------------------------------------------------------------------------------

pub static DEFAULT_CONFIG: &'static str = r#"
[[lines]]
    pat   = "(Error).*"
    colors = ["Red", "LightRed"]
    tokens = []
[[lines]]
    pat   = "(Warning).*"
    colors = ["Yellow", "LightYellow"]
    tokens = []
[[lines]]
    pat   = "(Info).*"
    colors = ["Green", "LightGreen"]
    tokens = []
"#;

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

error_chain! {
    links {
        Colorize(crate::colorize::Error, crate::colorize::ErrorKind);
    }
    foreign_links {
        Io(std::io::Error);
        Toml(toml::de::Error);
    }
}

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

fn get_reader_file(path: &Path) -> Result<Box<dyn BufRead>> {
    let f =
        File::open(path).chain_err(|| format!("failed to open '{}'", path.to_string_lossy()))?;
    Ok(Box::new(BufReader::new(f)))
}

fn get_reader_stdin(timeout_millis: u64) -> Result<Box<dyn BufRead>> {
    Ok(Box::new(BufReader::new(TimeoutReader::new(
        stdin(),
        Duration::from_millis(timeout_millis),
    ))))
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    any(target_env = "gnu", target_env = "musl")
))]
fn get_reader_proc(pid: i32) -> Result<Box<dyn BufRead>> {
    let pid = Pid::from_raw(pid);
    Ok(Box::new(BufReader::new(ProcReader::from_stdany(pid))))
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    any(target_env = "gnu", target_env = "musl")
)))]
fn get_reader_proc(_pid: i32) -> Result<Box<BufRead>> {
    Err("--process option is supported on linux only".into())
}

fn get_config_path(opt: &Opt) -> Option<PathBuf> {
    if let Some(ref p) = opt.config {
        return Some(p.clone());
    } else if let Some(mut p) = dirs::home_dir() {
        p.push(".pipecolor.toml");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn output(
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
    use_color: bool,
    config: &Config,
    opt: &Opt,
) -> Result<()> {
    let mut s = String::new();
    loop {
        match read_line_timeout(reader, &mut s)? {
            (0, false) => {
                if opt.process.is_some() {
                    continue;
                } else {
                    break;
                }
            }
            (0, true) => continue,
            (_, _) => {
                if use_color {
                    let (s2, i) = colorize(s, config)?;
                    s = s2;
                    if opt.verbose {
                        if let Some(i) = i {
                            eprintln!("pipecolor: line matched to '{:?}'", config.lines[i].pat);
                        }
                    }
                }
                let _ = writer.write(s.as_bytes());
                let _ = writer.flush();
                s.clear();
            }
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

quick_main!(run);

fn run() -> Result<()> {
    let opt = Opt::from_args();
    run_opt(&opt)
}

fn run_opt(opt: &Opt) -> Result<()> {
    let config = get_config_path(opt);

    let config: Config = match config {
        Some(c) => {
            if opt.verbose {
                eprintln!("pipecolor: Read config from '{}'", c.to_string_lossy());
            }
            let mut f =
                File::open(&c).chain_err(|| format!("failed to open '{}'", c.to_string_lossy()))?;
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            toml::from_str(&s)
                .chain_err(|| format!("failed to parse toml '{}'", c.to_string_lossy()))?
        }
        None => toml::from_str(DEFAULT_CONFIG).unwrap(),
    };

    let use_color = match opt.mode.as_ref() {
        "auto" => atty::is(Stream::Stdout),
        "always" => true,
        "disable" => false,
        _ => true,
    };

    let mut writer = BufWriter::new(stdout());

    if let Some(pid) = opt.process {
        let mut reader = get_reader_proc(pid)?;
        let _ = output(&mut *reader, writer.get_mut(), use_color, &config, &opt)?;
    } else if opt.files.is_empty() {
        let mut reader = get_reader_stdin(opt.timeout)?;
        let _ = output(&mut *reader, writer.get_mut(), use_color, &config, &opt)?;
    } else {
        for f in &opt.files {
            let mut reader = get_reader_file(&f)?;
            let _ = output(&mut *reader, writer.get_mut(), use_color, &config, &opt)?;
        }
    };

    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        let args = vec![
            "pipecolor",
            "-c",
            "sample/pipecolor.toml",
            "sample/access_log",
            "sample/maillog",
        ];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_verbose() {
        let args = vec![
            "pipecolor",
            "-v",
            "-c",
            "sample/pipecolor.toml",
            "sample/access_log",
        ];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_mode() {
        let args = vec![
            "pipecolor",
            "-m",
            "always",
            "-c",
            "sample/pipecolor.toml",
            "sample/access_log",
        ];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());

        let args = vec![
            "pipecolor",
            "-m",
            "auto",
            "-c",
            "sample/pipecolor.toml",
            "sample/access_log",
        ];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());

        let args = vec![
            "pipecolor",
            "-m",
            "disable",
            "-c",
            "sample/pipecolor.toml",
            "sample/access_log",
        ];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_read_config_fail() {
        let args = vec!["pipecolor", "-c", "test", "sample/access_log"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_err());
    }
}
