use anyhow::Result;
use std::{collections::HashMap, fmt::Display, str::FromStr};
use thiserror::Error;

/// Supported UCI commands
#[derive(PartialEq, Debug)]
pub enum UCI {
    /// Sent after the 'uci' command
    UciOk,

    /// Sent after the 'isready' command
    ReadyOk,

    /// Engine sending info to GUI
    Info {
        cp: Option<isize>,
        mate: Option<isize>,
        depth: Option<isize>,
        seldepth: Option<isize>,
        nodes: Option<isize>,
        time: Option<isize>,
        multipv: Option<isize>,
        pv: Option<Vec<String>>,
    },

    /// Options can be set to modify the engine behaviour
    Option { name: String, opt_type: OptionType },
}

/// Possible types for Engine Options
#[derive(PartialEq, Debug, Clone)]
pub enum OptionType {
    Check {
        default: bool,
    },
    Spin {
        default: isize,
        min: isize,
        max: isize,
    },
    Combo {
        default: String,
        options: Vec<String>,
    },
    Button,
    String {
        default: String,
    },
}

impl OptionType {
    fn new(opt_type: String, line: String) -> Result<Self> {
        Ok(match opt_type.as_str() {
            "check" => OptionType::new_check(line)?,
            "spin" => OptionType::new_spin(line)?,
            "combo" => OptionType::new_combo(line)?,
            "button" => OptionType::new_button()?,
            "string" => OptionType::new_string(line)?,
            _ => return Err(UCIError::ParseError.into()),
        })
    }

    fn new_check(line: String) -> Result<Self> {
        let words = vec!["default"];
        let values = parse_line_values(line, words)?;
        Ok(OptionType::Check {
            default: values["default"].unwrap(),
        })
    }

    fn new_spin(line: String) -> Result<Self> {
        let words = vec!["default", "min", "max"];
        let values = parse_line_values(line, words)?;
        Ok(OptionType::Spin {
            default: values["default"].unwrap(),
            min: values["min"].unwrap(),
            max: values["max"].unwrap(),
        })
    }

    fn new_combo(line: String) -> Result<Self> {
        let words = vec!["default"];
        let values = parse_line_values(line.clone(), words)?;
        let line: Vec<&str> = line.split_whitespace().collect();
        let mut options = Vec::new();
        // TODO: Check if combo options can have spaces, in which case this will give incorrect results
        for ix in 0..line.len() {
            if line[ix] == "var" {
                options.push(line[ix + 1].to_string());
            }
        }
        Ok(OptionType::Combo {
            default: values["default"].clone().unwrap(),
            options: options,
        })
    }

    fn new_button() -> Result<Self> {
        Ok(OptionType::Button)
    }

    fn new_string(line: String) -> Result<Self> {
        let words = vec!["default"];
        let values = parse_line_values(line, words)?;
        Ok(OptionType::String {
            default: values["default"].clone().unwrap(),
        })
    }
}

/// Errors produced from UCI parsing
#[derive(Error, Debug)]
pub enum UCIError {
    /// Error parsing a UCI command
    ParseError,
}

impl Display for UCIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            UCIError::ParseError => "error parsing uci command",
        };
        return f.write_str(data);
    }
}

/// Parse an UCI command
pub fn parse_uci(line: String) -> Result<UCI> {
    let line = line.trim().to_string();
    let command = line.split_whitespace().next().unwrap_or("");
    match command {
        "info" => parse_info_line(line),
        "uciok" => Ok(UCI::UciOk),
        "readyok" => Ok(UCI::ReadyOk),
        "option" => parse_option_line(line),
        _ => Err(UCIError::ParseError.into()),
    }
}

/// parse_line_values parses the value following each word in the given line.
fn parse_line_values<T: FromStr + Default>(
    line: String,
    words: Vec<&str>,
) -> Result<HashMap<String, Option<T>>> {
    let line: Vec<&str> = line.split_whitespace().collect();
    let mut values = HashMap::with_capacity(words.len());
    for word in words.iter() {
        let mut i = line.iter();
        let value = match i.position(|x: &&str| x == word) {
            Some(ix) => match line.get(ix + 1) {
                Some(v) => v.parse::<T>().ok(),
                None => Some(T::default()),
            },
            None => None,
        };
        values.insert(word.to_string(), value);
    }
    Ok(values)
}

/// Parse an info line for all supported metadata
fn parse_info_line(line: String) -> Result<UCI> {
    let words = vec![
        "cp", "depth", "nodes", "seldepth", "mate", "time", "multipv",
    ];
    let values = parse_line_values(line.clone(), words)?;
    return Ok(UCI::Info {
        cp: values["cp"],
        mate: values["mate"],
        depth: values["depth"],
        nodes: values["nodes"],
        time: values["time"],
        multipv: values["multipv"],
        seldepth: values["seldepth"],
        pv: parse_pv(line),
    });
}

/// Parse an info line and return all the moves stated after 'pv'
fn parse_pv(line: String) -> Option<Vec<String>> {
    let line: Vec<&str> = line.split_whitespace().collect();
    let mut pv = Vec::new();
    let mut i = line.iter();
    match i.position(|x: &&str| *x == "pv") {
        Some(_) => {}
        None => return None, // early return if no pv is found
    };
    while let Some(word) = i.next() {
        pv.push(word.to_string());
    }
    Some(pv)
}

fn parse_option_line(line: String) -> Result<UCI> {
    // FIXME: handle `name`s with spaces (i.e. `option name Clear Hash type button`)
    let words = vec!["name", "type"];
    let values = parse_line_values(line.clone(), words)?;
    return Ok(UCI::Option {
        name: values["name"].clone().unwrap(),
        opt_type: OptionType::new(values["type"].clone().unwrap(), line)?,
    });
}

#[cfg(test)]
mod test {

    use crate::parse::{parse_info_line, UCI};
    use anyhow::Result;

    macro_rules! test_info_line {
        ($line:expr, $ev:expr) => {
            let ev = parse_info_line($line.to_string())?;
            assert_eq!(ev, $ev);
        };
    }

    #[tokio::test]
    async fn test_parse_info_line() -> Result<()> {
        test_info_line!("info depth 1 seldepth 1 multipv 1 score cp 59 nodes 56 nps 56000 hashfull 0 tbhits 0 time 1", 
            UCI::Info {
                cp: Some(59),
                mate: None,
                depth: Some(1),
                nodes: Some(56),
                seldepth: Some(1),
                multipv: Some(1),
                time: Some(1),
                pv: None,
            }
        );
        test_info_line!("info depth 1 seldepth 1 multipv 1 score cp 59 nodes 56 nps 56000 hashfull 0 tbhits 0 time 1 pv d6f4 e3f4", 
            UCI::Info {
                cp: Some(59),
                mate: None,
                depth: Some(1),
                nodes: Some(56),
                seldepth: Some(1),
                multipv: Some(1),
                time: Some(1),
                pv: Some(vec!["d6f4".to_string(), "e3f4".to_string()]),
            }
        );
        test_info_line!(
            "info depth 2 seldepth 2 multipv 1 score cp -27 nodes 227 nps 227000 hashfull 0 tbhits 0 time 1 pv a8b8 f4d6",
            UCI::Info {
                cp: Some(-27),
                mate: None,
                depth: Some(2),
                nodes: Some(227),
                seldepth: Some(2),
                multipv: Some(1),
                time: Some(1),
                pv: Some(vec!["a8b8".to_string(), "f4d6".to_string()]),
            }
        );
        test_info_line!(
            "info depth 24 seldepth 33 multipv 1 score cp -195 nodes 2499457 nps 642203 hashfull 812 tbhits 0 time 3892 pv d8a5 a4a5 c6a5 f4d6 b7a6 d6c5 f6d7 c5a3 f7f6 e1g1 a8c8 b2b3 e8f7 f1c1 d7b6 f3e1 f5g6 f2f3 h8d8 e3e4 a5c6 e1d3 e6e5 d3c5 d5e4 d2e4 g6e4 c5e4",
            UCI::Info {
                cp: Some(-195),
                mate: None,
                depth: Some(24),
                nodes: Some(2499457),
                seldepth: Some(33),
                multipv: Some(1),
                time: Some(3892),
                pv: Some(vec![
                    "d8a5".to_string(),
                    "a4a5".to_string(),
                    "c6a5".to_string(),
                    "f4d6".to_string(),
                    "b7a6".to_string(),
                    "d6c5".to_string(),
                    "f6d7".to_string(),
                    "c5a3".to_string(),
                    "f7f6".to_string(),
                    "e1g1".to_string(),
                    "a8c8".to_string(),
                    "b2b3".to_string(),
                    "e8f7".to_string(),
                    "f1c1".to_string(),
                    "d7b6".to_string(),
                    "f3e1".to_string(),
                    "f5g6".to_string(),
                    "f2f3".to_string(),
                    "h8d8".to_string(),
                    "e3e4".to_string(),
                    "a5c6".to_string(),
                    "e1d3".to_string(),
                    "e6e5".to_string(),
                    "d3c5".to_string(),
                    "d5e4".to_string(),
                    "d2e4".to_string(),
                    "g6e4".to_string(),
                    "c5e4".to_string(),
                ]),
            }
        );
        Ok(())
    }
}
