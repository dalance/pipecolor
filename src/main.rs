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
// Options
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "pipecolor")]
#[structopt(raw(long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Files to show
    #[structopt(name = "FILE", parse(from_os_str))]
    pub files: Vec<PathBuf>,

    /// Show verbose message
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Formats
// -------------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Formats {
    pub formats: Vec<Format>,
}

#[derive(Deserialize)]
pub struct Format {
    pub name: String,

    #[serde(with = "regex_serde")] pub pat: Regex,

    pub styles: Vec<Style>,
}

#[derive(Deserialize)]
pub struct Style {
    #[serde(with = "regex_serde")] pub pat: Regex,

    #[serde(with = "color_serde")] pub color_matched: Box<Color>,

    #[serde(with = "color_serde")] pub color_unmatched: Box<Color>,
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

pub static DEFAULT_FORMAT: &'static str = r#"
[[formats]]
    name = "Default"
    pat  = ".*"
    [[formats.styles]]
        pat             = "Error"
        color_matched   = "LightRed"
        color_unmatched = "Red"
    [[formats.styles]]
        pat             = "Warning"
        color_matched   = "LightYellow"
        color_unmatched = "Yellow"
    [[formats.styles]]
        pat             = "Info"
        color_matched   = "LightGreen"
        color_unmatched = "Green"
"#;

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

fn get_reader_file(path: &Path) -> Box<BufRead> {
    let f = File::open(path).unwrap();
    Box::new(BufReader::new(f))
}

fn get_reader_stdin() -> Box<BufRead> {
    Box::new(BufReader::new(stdin()))
}

fn output(mut reader: Box<BufRead>, formats: &Formats, opt: &Opt) {
    let mut format = None;
    let mut s = String::new();
    loop {
        match reader.read_line(&mut s) {
            Ok(0) => break,
            Ok(_) => {
                format = detect_format(&s, formats, opt).or(format);
                if let Some(format) = format {
                    s = apply_style(s, &formats.formats[format]);
                }
                print!("{}", s);
                s.clear();
            }
            Err(_) => break,
        }
    }
}

fn detect_format(s: &str, formats: &Formats, opt: &Opt) -> Option<usize> {
    for (i, format) in formats.formats.iter().enumerate() {
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
    let mut mat_str: Option<String> = None;
    let mut mat_idx = 0;
    {
        for (i, style) in format.styles.iter().enumerate() {
            let mat = style.pat.find(&s);
            if let Some(mat) = mat {
                mat_str = Some(String::from(mat.as_str()));
                mat_idx = i;
                break;
            }
        }
    }

    if let Some(mat_str) = mat_str {
        let mat_style = &format.styles[mat_idx];
        s = s.replace(
            &mat_str,
            &format!(
                "{}{}{}",
                color::Fg(&*mat_style.color_matched),
                mat_str,
                color::Fg(&*mat_style.color_unmatched)
            ),
        );
        s = format!(
            "{}{}{}",
            color::Fg(&*mat_style.color_unmatched),
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

fn main() {
    let opt = Opt::from_args();
    let config = get_config_path();

    let formats: Formats = match config {
        Some(c) => {
            if opt.verbose {
                println!("pipecolor: Read config from '{}'", c.to_string_lossy());
            }
            let mut f = File::open(&c).unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            toml::from_str(&s).unwrap()
        }
        None => toml::from_str(DEFAULT_FORMAT).unwrap(),
    };

    if opt.files.is_empty() {
        let reader = get_reader_stdin();
        output(reader, &formats, &opt);
    } else {
        for f in &opt.files {
            let reader = get_reader_file(&f);
            output(reader, &formats, &opt);
        }
    };
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------
