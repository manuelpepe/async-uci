use anyhow::{bail, Result};
use std::fmt::Display;
use thiserror::Error;

#[derive(PartialEq, Debug)]
pub enum UCI {
    UciOk,
    ReadyOk,
    Info {
        score: Option<isize>,
        mate: Option<isize>,
        depth: Option<isize>,
        nodes: Option<isize>,
    },
}

#[derive(Error, Debug)]
pub enum UCIError {
    ParseError,
}

impl Display for UCIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            UCIError::ParseError => "error parsing uci command",
        };
        f.write_str(data)?;
        Ok(())
    }
}

pub fn parse_uci(line: String) -> Result<UCI> {
    if line.starts_with("info") {
        match parse_info_line(line) {
            Ok(info) => return Ok(info),
            Err(_) => return Err(UCIError::ParseError.into()),
        }
    } else if line.starts_with("uciok") {
        return Ok(UCI::UciOk);
    } else if line.starts_with("readyok") {
        return Ok(UCI::ReadyOk);
    }
    bail!(UCIError::ParseError)
}

fn parse_info_line(line: String) -> Result<UCI> {
    // TODO: this is a bit of a hack, but it works for now
    // thanks copilot for recommending the above comment.
    let line: Vec<&str> = line.split_whitespace().collect();
    let words = vec!["cp", "depth", "nodes"];
    let mut values = Vec::with_capacity(words.len());
    for _ in 0..words.len() {
        values.push(None);
    }
    for (wix, word) in words.iter().enumerate() {
        let mut i = line.iter();
        let value = match i.position(|x: &&str| x == word) {
            Some(ix) => line[ix + 1].parse::<isize>().ok(),
            None => None,
        };
        values[wix] = value;
    }
    return Ok(UCI::Info {
        score: values[0],
        mate: None,
        depth: values[1],
        nodes: values[2],
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
        test_info_line!("info depth 1 seldepth 1 multipv 1 score cp 59 nodes 56 nps 56000 hashfull 0 tbhits 0 time 1 pv d6f4 e3f4", 
            UCI::Info {
                score: Some(59),
                mate: None,
                depth: Some(1),
                nodes: Some(56),
            }
        );
        test_info_line!(
            "info depth 2 seldepth 2 multipv 1 score cp -27 nodes 227 nps 227000 hashfull 0 tbhits 0 time 1 pv a8b8 f4d6",
            UCI::Info {
                score: Some(-27),
                mate: None,
                depth: Some(2),
                nodes: Some(227),
            }
        );
        test_info_line!(
            "info depth 24 seldepth 33 multipv 1 score cp -195 nodes 2499457 nps 642203 hashfull 812 tbhits 0 time 3892 pv d8a5 a4a5 c6a5 f4d6 b7a6 d6c5 f6d7 c5
a3 f7f6 e1g1 a8c8 b2b3 e8f7 f1c1 d7b6 f3e1 f5g6 f2f3 h8d8 e3e4 a5c6 e1d3 e6e5 d3c5 d5e4 d2e4 g6e4 c5e4",
            UCI::Info {
                score: Some(-195),
                mate: None,
                depth: Some(24),
                nodes: Some(2499457),
            }
        );
        Ok(())
    }
}
