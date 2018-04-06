extern crate atty;
#[macro_use]
extern crate error_chain;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;
extern crate termion;
extern crate toml;

mod colorize;

use colorize::{colorize, Config};
use atty::Stream;
use std::env::home_dir;
use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use structopt::{clap, StructOpt};

// -------------------------------------------------------------------------------------------------
// Option
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "pipecolor")]
#[structopt(raw(long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Files to show
    #[structopt(name = "FILE", parse(from_os_str))]
    pub files: Vec<PathBuf>,

    /// Colorize mode
    #[structopt(short = "m", long = "mode", default_value = "auto", possible_value = "auto",
                possible_value = "always", possible_value = "disable")]
    pub mode: String,

    /// Config file
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    pub config: Option<PathBuf>,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
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
        Colorize(::colorize::Error, ::colorize::ErrorKind);
    }
    foreign_links {
        Io(::std::io::Error);
        Toml(::toml::de::Error);
    }
}

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

fn get_reader_file(path: &Path) -> Result<Box<BufRead>> {
    let f = File::open(path).chain_err(|| format!("failed to open '{}'", path.to_string_lossy()))?;
    Ok(Box::new(BufReader::new(f)))
}

fn get_reader_stdin() -> Result<Box<BufRead>> {
    Ok(Box::new(BufReader::new(stdin())))
}

fn get_config_path(opt: &Opt) -> Option<PathBuf> {
    if let Some(ref p) = opt.config {
        return Some(p.clone());
    } else if let Some(mut p) = home_dir() {
        p.push(".pipecolor.toml");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn output(reader: &mut BufRead, writer: &mut Write, use_color: bool, config: &Config, opt: &Opt) -> Result<()> {
    let mut s = String::new();
    loop {
        match reader.read_line(&mut s) {
            Ok(0) => break,
            Ok(_) => {
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
                //let _ = writer.flush();
                s.clear();
            }
            Err(_) => break,
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

    if opt.files.is_empty() {
        let mut reader = get_reader_stdin()?;
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
