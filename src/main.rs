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
use std::io::{stdin, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use structopt::{clap, StructOpt};
use termion::{color, style};
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

    #[serde(with = "color_serde")] pub color: Box<Color>,

    pub tokens: Vec<Token>,
}

#[derive(Deserialize)]
pub struct Token {
    #[serde(with = "regex_serde")] pub pat: Regex,

    #[serde(with = "color_serde")] pub color: Box<Color>,
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

mod color_serde {
    use serde::{self, Deserialize, Deserializer};
    use termion::color::{self, Color};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Box<Color>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_ref() {
            "Black" => Ok(Box::new(color::Black)),
            "Blue" => Ok(Box::new(color::Blue)),
            "Cyan" => Ok(Box::new(color::Cyan)),
            "Green" => Ok(Box::new(color::Green)),
            "LightBlack" => Ok(Box::new(color::LightBlack)),
            "LightBlue" => Ok(Box::new(color::LightBlue)),
            "LightCyan" => Ok(Box::new(color::LightCyan)),
            "LightGreen" => Ok(Box::new(color::LightGreen)),
            "LightMagenta" => Ok(Box::new(color::LightMagenta)),
            "LightRed" => Ok(Box::new(color::LightRed)),
            "LightWhite" => Ok(Box::new(color::LightWhite)),
            "LightYellow" => Ok(Box::new(color::LightYellow)),
            "Magenta" => Ok(Box::new(color::Magenta)),
            "Red" => Ok(Box::new(color::Red)),
            "White" => Ok(Box::new(color::White)),
            "Yellow" => Ok(Box::new(color::Yellow)),
            x => Err(serde::de::Error::custom(format!(
                "failed to parse color '{}'",
                x
            ))),
        }
    }
}

pub static DEFAULT_CONFIG: &'static str = r#"
[[formats]]
    name = "Default"
    pat  = ".*"
    [[formats.lines]]
        pat   = "Error"
        color = "Red"
        [[formats.lines.tokens]]
            pat   = "Error"
            color = "LightRed"
    [[formats.lines]]
        pat   = "Warning"
        color = "Yellow"
        [[formats.lines.tokens]]
            pat   = "Warning"
            color = "LightYellow"
    [[formats.lines]]
        pat   = "Info"
        color = "Green"
        [[formats.lines.tokens]]
            pat   = "Info"
            color = "LightGreen"
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

fn output(mut reader: Box<BufRead>, config: &Config, opt: &Opt) {
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
                print!("{}", s);
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
    let mut mat_idx = None;
    {
        for (i, line) in format.lines.iter().enumerate() {
            let mat = line.pat.find(&s);
            if mat.is_some() {
                mat_idx = Some(i);
                break;
            }
        }
    }

    if let Some(mat_idx) = mat_idx {
        let mat_line = &format.lines[mat_idx];
        for token in &mat_line.tokens {
            let mut mat_str = None;
            {
                let mat = token.pat.find(&s);
                if let Some(mat) = mat {
                    mat_str = Some(String::from(mat.as_str()));
                }
            }
            if let Some(mat_str) = mat_str {
                s = s.replace(
                    &mat_str,
                    &format!(
                        "{}{}{}",
                        color::Fg(&*token.color),
                        mat_str,
                        color::Fg(&*mat_line.color)
                        ),
                );
            }
        }
        s = format!(
            "{}{}{}",
            color::Fg(&*mat_line.color),
            s,
            style::Reset
        );
    }

    s
}

fn get_config_path() -> Option<PathBuf> {
    match home_dir() {
        Some(mut p) => {
            p.push(".pipecolor.toml");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        }
        None => None,
    }
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

quick_main!(run);

fn run() -> Result<()> {
    let opt = Opt::from_args();
    let config = get_config_path();

    let config: Config = match config {
        Some(c) => {
            if opt.verbose {
                println!("pipecolor: Read config from '{}'", c.to_string_lossy());
            }
            let mut f = File::open(&c).chain_err(|| format!("failed to open '{}'", c.to_string_lossy()))?;
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            toml::from_str(&s).chain_err(|| format!("failed to parse toml '{}'", c.to_string_lossy()))?
        }
        None => toml::from_str(DEFAULT_CONFIG).unwrap(),
    };

    if opt.files.is_empty() {
        let reader = get_reader_stdin()?;
        output(reader, &config, &opt);
    } else {
        for f in &opt.files {
            let reader = get_reader_file(&f)?;
            output(reader, &config, &opt);
        }
    };

    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------
