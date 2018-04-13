use regex::Regex;
use termion::color;
use termion::color::Color;

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

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
    }
}

// -------------------------------------------------------------------------------------------------
// Functions
// -------------------------------------------------------------------------------------------------

pub fn colorize(mut s: String, config: &Config) -> Result<(String, Option<usize>)> {
    #[derive(Debug)]
    enum PosType {
        Start,
        End,
    }

    let mut pos = Vec::new();
    let mut line_idx = None;

    for (i, line) in config.lines.iter().enumerate() {
        let cap = line.pat.captures(&s);
        if let Some(cap) = cap {
            line_idx = Some(i);
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
                            pos.insert(0, (PosType::End, mat.end(), token.colors[j].clone()));
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
            color::Fg(&*conv_color(&current_color.last())?)
        ));
        idx += s.len();
        s = rest;
    }

    ret.push_str(&s);
    Ok((ret, line_idx))
}

fn conv_color(s: &Option<&String>) -> Result<Box<Color>> {
    let ret: Box<Color> = if let &Some(ref s) = s {
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
            _ => {
                bail!(format!("failed to parse color name '{}'", s));
            }
        }
    } else {
        Box::new(color::Reset)
    };
    Ok(ret)
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use toml;

    pub static TEST_CONFIG: &'static str = r#"
    [[lines]]
        pat   = "A(.*) (.*) (.*) .*"
        colors = ["Black", "Blue", "Cyan", "Default"]
        [[lines.tokens]]
            pat   = "A"
            colors = ["Green"]
    [[lines]]
        pat   = "B(.*) (.*) (.*) .*"
        colors = ["LightBlack", "LightBlue", "LightCyan", "LightGreen"]
        tokens = []
    [[lines]]
        pat   = "C(.*) (.*) (.*) .*"
        colors = ["LightMagenta", "LightRed", "LightWhite", "LightYellow"]
        tokens = []
    [[lines]]
        pat   = "D(.*) (.*) (.*) .*"
        colors = ["Magenta", "Red", "White", "Yellow"]
        tokens = []
    "#;

    pub static TEST_CONFIG2: &'static str = r#"
    [[lines]]
        pat   = "A(.*) (.*) (.*) .*"
        colors = ["xxx", "Blue", "Cyan", "Default"]
        tokens = []
    "#;

    #[test]
    fn test_colorize() {
        let config: Config = toml::from_str(TEST_CONFIG).unwrap();
        let (ret, idx) = colorize(String::from("A123 456 789 xyz"), &config).unwrap();
        assert_eq!(ret, "\u{1b}[38;5;0m\u{1b}[38;5;2mA\u{1b}[38;5;0m\u{1b}[38;5;4m123\u{1b}[38;5;0m \u{1b}[38;5;6m456\u{1b}[38;5;0m \u{1b}[39m789\u{1b}[38;5;0m xyz\u{1b}[39m");
        assert_eq!(idx, Some(0));

        let (ret, idx) = colorize(String::from("B123 456 789 xyz"), &config).unwrap();
        assert_eq!(ret, "\u{1b}[38;5;8mB\u{1b}[38;5;12m123\u{1b}[38;5;8m \u{1b}[38;5;14m456\u{1b}[38;5;8m \u{1b}[38;5;10m789\u{1b}[38;5;8m xyz\u{1b}[39m");
        assert_eq!(idx, Some(1));

        let (ret, idx) = colorize(String::from("C123 456 789 xyz"), &config).unwrap();
        assert_eq!(ret, "\u{1b}[38;5;13mC\u{1b}[38;5;9m123\u{1b}[38;5;13m \u{1b}[38;5;15m456\u{1b}[38;5;13m \u{1b}[38;5;11m789\u{1b}[38;5;13m xyz\u{1b}[39m");
        assert_eq!(idx, Some(2));

        let (ret, idx) = colorize(String::from("D123 456 789 xyz"), &config).unwrap();
        assert_eq!(ret, "\u{1b}[38;5;5mD\u{1b}[38;5;1m123\u{1b}[38;5;5m \u{1b}[38;5;7m456\u{1b}[38;5;5m \u{1b}[38;5;3m789\u{1b}[38;5;5m xyz\u{1b}[39m");
        assert_eq!(idx, Some(3));

        let (ret, idx) = colorize(String::from("E123 456 789 xyz"), &config).unwrap();
        assert_eq!(ret, "E123 456 789 xyz");
        assert_eq!(idx, None);
    }

    #[test]
    fn test_colorize_fail() {
        let config: Config = toml::from_str(TEST_CONFIG2).unwrap();
        let ret = colorize(String::from("A123 456 789 xyz"), &config);
        assert_eq!(
            &format!("{:?}", ret)[0..50],
            "Err(Error(Msg(\"failed to parse color name \\\'xxx\\\'\""
        );
    }
}
