extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;
extern crate termion;
extern crate toml;

use regex::Regex;
use std::fs::File;
use std::io::{stdin, BufRead, BufReader};
use std::path::{Path, PathBuf};
use structopt::{clap, StructOpt};
use termion::{color, style};
use termion::color::Color;

// -------------------------------------------------------------------------------------------------
// Options
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "colored")]
#[structopt(raw(long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Files to show
    #[structopt(name = "FILE", parse(from_os_str))]
    pub files: Vec<PathBuf>,
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
        println!("{}", s);
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
    name = "Synopsys Frontend"
    pat  = "(Design Compiler\\(R\\)|PrimeTime \\(R\\)|Formality \\(R\\))"
    [[formats.styles]]
        pat             = "^Error:"
        color_matched   = "LightRed"
        color_unmatched = "Red"
    [[formats.styles]]
        pat             = "^Warning:"
        color_matched   = "LightYellow"
        color_unmatched = "Yellow"
    [[formats.styles]]
        pat             = "^Information:"
        color_matched   = "LightGreen"
        color_unmatched = "Green"
    [[formats.styles]]
        pat             = "^Verification SUCCEEDED"
        color_matched   = "LightGreen"
        color_unmatched = "Green"
    [[formats.styles]]
        pat             = "^Verification FAILED"
        color_matched   = "LightRed"
        color_unmatched = "Red"

[[formats]]
    name = "VCS"
    pat  = "Chronologic VCS \\(TM\\)"
    [[formats.styles]]
        pat             = "^Error-\\[.*\\]"
        color_matched   = "LightRed"
        color_unmatched = "Red"
    [[formats.styles]]
        pat             = "^Warning-\\[.*\\]"
        color_matched   = "LightYellow"
        color_unmatched = "Yellow"
    [[formats.styles]]
        pat             = "^Lint-\\[.*\\]"
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

fn output(mut reader: Box<BufRead>, formats: &Formats) {
    let mut format = None;
    let mut s = String::new();
    loop {
        match reader.read_line(&mut s) {
            Ok(0) => break,
            Ok(_) => {
                format = detect_format(&s, formats).or(format);
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

fn detect_format(s: &str, formats: &Formats) -> Option<usize> {
    for (i, format) in formats.formats.iter().enumerate() {
        let mat = format.pat.find(&s);
        if mat.is_some() {
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

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

fn main() {
    let opt = Opt::from_args();

    let formats: Formats = toml::from_str(DEFAULT_FORMAT).unwrap();

    if opt.files.is_empty() {
        let reader = get_reader_stdin();
        output(reader, &formats);
    } else {
        for f in opt.files {
            let reader = get_reader_file(&f);
            output(reader, &formats);
        }
    };
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------
