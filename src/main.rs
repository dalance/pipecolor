#[macro_use]
extern crate error_chain;
extern crate nix;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;
extern crate termion;
extern crate toml;

use regex::Regex;
use nix::sys::stat::{fstat, SFlag};
use std::env::home_dir;
use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use structopt::{clap, StructOpt};
use termion::color;
use termion::color::Color;

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
    #[structopt(short = "m", long = "mode", default_value = "auto", possible_value = "auto", possible_value = "always", possible_value = "disable")]
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

#[derive(Deserialize)]
pub struct Config {
    pub lines: Vec<Line>,
}

#[derive(Deserialize)]
pub struct Line {
    #[serde(with = "regex_serde")]
    pub pat: Regex,

    pub colors: Vec<String>,

    pub tokens: Vec<Token>,
}

#[derive(Deserialize)]
pub struct Token {
    #[serde(with = "regex_serde")]
    pub pat: Regex,

    pub colors: Vec<String>,
}

mod regex_serde {
    use regex::Regex;
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Regex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let r = Regex::new(&s).map_err(serde::de::Error::custom)?;
        Ok(r)
    }
}

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
    foreign_links {
        Io(::std::io::Error);
        Toml(::toml::de::Error);
        Nix(::nix::Error);
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

fn output(reader: &mut BufRead, writer: &mut Write, use_color: bool, config: &Config, _opt: &Opt) {
    let mut s = String::new();
    loop {
        match reader.read_line(&mut s) {
            Ok(0) => break,
            Ok(_) => {
                if use_color {
                    s = apply_style(s, &config);
                }
                let _ = writer.write(s.as_bytes());
                //let _ = writer.flush();
                s.clear();
            }
            Err(_) => break,
        }
    }
}

fn apply_style(mut s: String, config: &Config) -> String {
    #[derive(Debug)]
    enum PosType {
        Start,
        End,
    }

    let mut pos = Vec::new();

    for line in &config.lines {
        let cap = line.pat.captures(&s);
        if let Some(cap) = cap {
            for (j, mat) in cap.iter().enumerate() {
                if let Some(mat) = mat {
                    pos.push((PosType::Start, mat.start(), line.colors[j].clone()));
                    pos.push((PosType::End, mat.end(), line.colors[j].clone()));
                }
            }
            for token in &line.tokens {
                let cap = token.pat.captures(&s);
                if let Some(cap) = cap {
                    for (j, mat) in cap.iter().enumerate() {
                        if let Some(mat) = mat {
                            pos.push((PosType::Start, mat.start(), token.colors[j].clone()));
                            pos.push((PosType::End, mat.end(), token.colors[j].clone()));
                        }
                    }
                }
            }
            break;
        }
    }

    pos.sort_by_key(|&(_, p, _)| p);

    let mut current_color = vec![String::from("Default")];
    let mut ret = String::new();
    let mut idx = 0;
    for (t, p, color) in pos {
        match t {
            PosType::Start => {
                current_color.push(color);
            }
            PosType::End => {
                current_color.pop();
            }
        }
        let rest = s.split_off(p - idx);

        ret.push_str(&format!(
            "{}{}",
            s,
            color::Fg(&*conv_color(&current_color.last()))
        ));
        idx += s.len();
        s = rest;
    }

    ret.push_str(&s);
    ret
}

fn conv_color(s: &Option<&String>) -> Box<Color> {
    if let &Some(ref s) = s {
        match s.as_ref() {
            "Black" => Box::new(color::Black),
            "Blue" => Box::new(color::Blue),
            "Cyan" => Box::new(color::Cyan),
            "Default" => Box::new(color::Reset),
            "Green" => Box::new(color::Green),
            "LightBlack" => Box::new(color::LightBlack),
            "LightBlue" => Box::new(color::LightBlue),
            "LightCyan" => Box::new(color::LightCyan),
            "LightGreen" => Box::new(color::LightGreen),
            "LightMagenta" => Box::new(color::LightMagenta),
            "LightRed" => Box::new(color::LightRed),
            "LightWhite" => Box::new(color::LightWhite),
            "LightYellow" => Box::new(color::LightYellow),
            "Magenta" => Box::new(color::Magenta),
            "Red" => Box::new(color::Red),
            "White" => Box::new(color::White),
            "Yellow" => Box::new(color::Yellow),
            _ => Box::new(color::Reset),
        }
    } else {
        Box::new(color::Reset)
    }
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
                println!("pipecolor: Read config from '{}'", c.to_string_lossy());
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

    let stdout = stdout();
    let sflag = SFlag::from_bits_truncate(fstat(stdout.as_raw_fd())?.st_mode);
    let use_color = match opt.mode.as_ref() {
        "auto" => !sflag.contains(SFlag::S_IFREG),
        "always" => true,
        "disable" => false,
        _ => true,
    };

    let mut writer = BufWriter::new(stdout);

    if opt.files.is_empty() {
        let mut reader = get_reader_stdin()?;
        output(&mut *reader, writer.get_mut(), use_color, &config, &opt);
    } else {
        for f in &opt.files {
            let mut reader = get_reader_file(&f)?;
            output(&mut *reader, writer.get_mut(), use_color, &config, &opt);
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

    pub static TEST_CONFIG: &'static str = r#"
    [[lines]]
        pat   = "A(.*) (.*) (.*)"
        colors = ["Black", "Blue", "Cyan", "Default"]
        [[lines.tokens]]
            pat   = "A"
            colors = ["Green"]
    [[lines]]
        pat   = "B(.*) (.*) (.*)"
        colors = ["LightBlack", "LightBlue", "LightCyan", "LightGreen"]
        tokens = []
    [[lines]]
        pat   = "C(.*) (.*) (.*)"
        colors = ["LightMagenta", "LightRed", "LightWhite", "LightYellow"]
        tokens = []
    [[lines]]
        pat   = "D(.*) (.*) (.*)"
        colors = ["Magenta", "Red", "White", "Yellow"]
        tokens = []
    "#;

    pub static TEST_DATA: &'static str = r#"
A123 456 789 xyz
B123 456 789 xyz
C123 456 789 xyz
D123 456 789 xyz
    "#;

    pub static TEST_RESULT: &'static str = "\n\u{1b}[38;5;0m\u{1b}[38;5;2mA\u{1b}[38;5;4m\u{1b}[38;5;2m123 456\u{1b}[38;5;0m \u{1b}[38;5;6m789\u{1b}[38;5;0m \u{1b}[39mxyz\u{1b}[38;5;0m\u{1b}[39m\n\u{1b}[38;5;8mB\u{1b}[38;5;12m123 456\u{1b}[38;5;8m \u{1b}[38;5;14m789\u{1b}[38;5;8m \u{1b}[38;5;10mxyz\u{1b}[38;5;8m\u{1b}[39m\n\u{1b}[38;5;13mC\u{1b}[38;5;9m123 456\u{1b}[38;5;13m \u{1b}[38;5;15m789\u{1b}[38;5;13m \u{1b}[38;5;11mxyz\u{1b}[38;5;13m\u{1b}[39m\n\u{1b}[38;5;5mD\u{1b}[38;5;1m123 456\u{1b}[38;5;5m \u{1b}[38;5;7m789\u{1b}[38;5;5m \u{1b}[38;5;3mxyz\u{1b}[38;5;5m\u{1b}[39m\n    ";

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
    fn test_read_config_fail() {
        let args = vec!["pipecolor", "-c", "test", "sample/access_log"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_err());
    }

    #[test]
    fn test_output() {
        let args = vec!["pipecolor"];
        let opt = Opt::from_iter(args.iter());
        let config: Config = toml::from_str(TEST_CONFIG).unwrap();
        let mut reader = BufReader::new(TEST_DATA.as_bytes());
        let out = String::new();
        let mut writer = BufWriter::new(out.into_bytes());
        output(&mut reader, writer.get_mut(), &config, &opt);
        assert_eq!(
            TEST_RESULT,
            &String::from_utf8(writer.get_ref().to_vec()).unwrap()
        );
    }
}
