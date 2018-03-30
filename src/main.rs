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

use regex::Regex;
use std::env::home_dir;
use std::fs::File;
use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Read, Write};
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

    /// Config file
    #[structopt(short = "c", long = "config", parse(from_os_str))]
    pub config: Option<PathBuf>,

    /// Apply the specific format only
    #[structopt(short = "f", long = "format")]
    pub formats: Vec<String>,

    /// Connect to the specific process
    #[structopt(short = "p", long = "pid")]
    pub pid: Option<usize>,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Config
// -------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Config {
    pub formats: Vec<Format>,
}

#[derive(Deserialize)]
pub struct Format {
    pub name: String,

    #[serde(with = "regex_serde")] pub pat: Regex,

    pub lines: Vec<Line>,
}

#[derive(Deserialize)]
pub struct Line {
    #[serde(with = "regex_serde")] pub pat: Regex,

    pub colors: Vec<String>,

    pub tokens: Vec<Token>,
}

#[derive(Deserialize)]
pub struct Token {
    #[serde(with = "regex_serde")] pub pat: Regex,

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
[[formats]]
    name = "Default"
    pat  = ".*"
    [[formats.lines]]
        pat   = "(Error).*"
        colors = ["Red", "LightRed"]
        tokens = []
    [[formats.lines]]
        pat   = "(Warning).*"
        colors = ["Yellow", "LightYellow"]
        tokens = []
    [[formats.lines]]
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

fn output(reader: &mut BufRead, writer: &mut Write, config: &Config, opt: &Opt) {
    let mut format = None;
    let mut s = String::new();
    loop {
        match reader.read_line(&mut s) {
            Ok(0) => break,
            Ok(_) => {
                format = detect_format(&s, config, opt).or(format);
                if let Some(format) = format {
                    s = apply_style(s, &config.formats[format]);
                }
                let _ = writer.write(s.as_bytes());
                //let _ = writer.flush();
                s.clear();
            }
            Err(_) => break,
        }
    }
}

fn detect_format(s: &str, config: &Config, opt: &Opt) -> Option<usize> {
    for (i, format) in config.formats.iter().enumerate() {
        let mat = format.pat.find(&s);
        if mat.is_some() {
            if opt.verbose {
                println!("pipecolor: Format '{}' is detected", format.name);
            }
            return Some(i);
        }
    }
    None
}

fn apply_style(mut s: String, format: &Format) -> String {

    #[derive(Debug)]
    enum PosType {
        Start,
        End,
    }

    let mut pos = Vec::new();

    for line in &format.lines {
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

    pos.sort_by_key( |&(_, p, _)| p);

    let mut current_color = vec![String::from("Default")];
    let mut ret = String::new();
    let mut idx = 0;
    for ( t, p, color ) in pos {
        match t {
            PosType::Start => { current_color.push(color); }
            PosType::End => { current_color.pop(); }
        }
        let rest = s.split_off(p-idx);

        ret.push_str(&format!("{}{}", s, color::Fg(&*conv_color(&current_color.last()))));
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

    let mut writer = BufWriter::new(stdout());

    if opt.files.is_empty() {
        let mut reader = get_reader_stdin()?;
        output(&mut *reader, writer.get_mut(), &config, &opt);
    } else {
        for f in &opt.files {
            let mut reader = get_reader_file(&f)?;
            output(&mut *reader, writer.get_mut(), &config, &opt);
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
        let args = vec!["pipecolor", "-c", "sample/pipecolor.toml", "sample/access_log", "sample/maillog"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_config() {
        let args = vec!["pipecolor", "-c", "test", "sample/access_log"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_err());
    }
}
