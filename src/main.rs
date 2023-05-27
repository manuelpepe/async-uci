use anyhow::{bail, Result};
use async_trait::async_trait;
use std::{
    fmt::Display,
    process::Stdio,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    task::yield_now,
};

#[tokio::main]
async fn main() -> Result<()> {
    let sfpath =
        String::from("/root/rust/something_chess/res/stockfish/stockfish-ubuntu-20.04-x86-64");
    let position = "r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8";
    let mut sf = Engine::new(&sfpath).await?;
    sf.start_uci().await?;
    sf.new_game().await?;
    sf.set_position(position).await?;
    sf.go_infinite().await?;
    let mut last_eval = Evaluation {
        score: 0,
        mate: 0,
        depth: 0,
        nodes: 0,
    };
    loop {
        match sf.get_evaluation().await {
            Some(ev) if ev != last_eval => {
                println!("evaluation is: {ev:?}");
                last_eval = ev;
            }
            _ => {} //println!("no evaluation yet"),
        }
        yield_now().await;
    }
}

/// ChessEngine trait can be implemented for structures that implement the UCI Protocol
#[async_trait]
trait ChessEngine: Sized {
    async fn new(exe_path: &str) -> Result<Self>;
    async fn start_uci(&mut self) -> Result<()>;
    async fn new_game(&mut self) -> Result<()>;
    async fn set_position(&mut self, position: &str) -> Result<()>;
    async fn go_infinite(&mut self) -> Result<()>;
    async fn get_evaluation(&mut self) -> Option<Evaluation>;
}

/// Engine can be created to spawn any Chess Engine that implements the UCI Protocol
struct Engine {
    stdin: ChildStdin,
    state: EngineState,
    _proc: Child,
}

fn spawn_process(exe_path: &str) -> Result<(Child, ChildStdin, ChildStdout)> {
    let mut cmd = Command::new(exe_path);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    let mut proc = cmd.spawn()?;
    let stdout = proc.stdout.take().expect("no stdout available");
    let stdin = proc.stdin.take().expect("no stdin available");
    Ok((proc, stdin, stdout))
}

impl Engine {
    async fn send_command(&mut self, command: String) -> Result<()> {
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn _expect_state(&mut self, exp_state: &EngineStateEnum) -> Result<()> {
        let state = self.state.state.lock().expect("couldn't aquire state lock");
        if *exp_state == *state {
            return Ok(());
        }
        bail!("engine didn't respond with {:?}", exp_state)
    }

    async fn expect_state(&mut self, exp_state: EngineStateEnum) -> Result<()> {
        for _ in 0..10 {
            match self._expect_state(&exp_state).await {
                Ok(_) => return Ok(()),
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            };
        }
        bail!("engine didn't respond with {:?}", exp_state)
    }

    async fn expect_uciok(&mut self) -> Result<()> {
        self.expect_state(EngineStateEnum::Initialized).await
    }

    async fn expect_readyok(&mut self) -> Result<()> {
        self.expect_state(EngineStateEnum::Ready).await
    }
}

#[async_trait]
impl ChessEngine for Engine {
    async fn new(exe_path: &str) -> Result<Self> {
        let (proc, stdin, stdout) = spawn_process(exe_path)?;
        let state = EngineState::new(stdout).await;
        let sf = Engine {
            state: state,
            stdin: stdin,
            _proc: proc,
        };
        Ok(sf)
    }

    async fn start_uci(&mut self) -> Result<()> {
        self.send_command("uci\n".to_string()).await?;
        self.expect_uciok().await?;
        self.send_command("isready\n".to_string()).await?;
        self.expect_readyok().await?;
        Ok(())
    }

    async fn new_game(&mut self) -> Result<()> {
        self.send_command("ucinewgame\n".to_string()).await
    }

    async fn set_position(&mut self, fen: &str) -> Result<()> {
        let cmd = format!("position fen {}\n", fen);
        self.send_command(cmd.to_string()).await
    }

    async fn go_infinite(&mut self) -> Result<()> {
        self.send_command("go infinite\n".to_string()).await?;
        let mut s = self.state.state.lock().expect("couldn't acquire lock");
        *s = EngineStateEnum::Thinking;
        Ok(())
    }

    async fn get_evaluation(&mut self) -> Option<Evaluation> {
        let ev = self.state.evaluation.lock().expect("couldn't acquire lock");
        return match &*ev {
            Some(e) => Some(e.clone()),
            None => None,
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Evaluation {
    score: isize,
    mate: isize,
    depth: isize,
    nodes: isize,
}

#[derive(PartialEq, Debug)]
enum EngineStateEnum {
    Uninitialized,
    Initialized,
    Ready,
    Thinking,
}

struct EngineState {
    state: Arc<Mutex<EngineStateEnum>>,
    evaluation: Arc<Mutex<Option<Evaluation>>>,
}

impl EngineState {
    async fn new(stdout: ChildStdout) -> Self {
        let ev = Arc::new(Mutex::new(None));
        let state = Arc::new(Mutex::new(EngineStateEnum::Uninitialized));
        let mut stdout = BufReader::new(stdout);
        let engstate = EngineState {
            state: state.clone(),
            evaluation: ev.clone(),
        };
        let ev = ev.clone();
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                let mut str = String::new();
                stdout.read_line(&mut str).await.unwrap();
                match parse_uci(str) {
                    Ok(UCI::UciOk) => {
                        let mut state = state.lock().expect("couldn't aquire state lock");
                        *state = EngineStateEnum::Initialized;
                    }
                    Ok(UCI::ReadyOk) => {
                        let mut state = state.lock().expect("couldn't aquire state lock");
                        *state = EngineStateEnum::Ready;
                    }
                    Ok(UCI::Info {
                        score,
                        mate,
                        depth,
                        nodes,
                    }) => {
                        let mut ev = ev.lock().expect("couldn't aquire ev lock");
                        let def_ev = Evaluation {
                            score: 0,
                            mate: 0,
                            depth: 0,
                            nodes: 0,
                        };
                        let prev_ev = match ev.as_ref() {
                            Some(ev) => ev,
                            None => &def_ev,
                        };
                        *ev = Some(Evaluation {
                            score: score.unwrap_or(prev_ev.score),
                            mate: mate.unwrap_or(prev_ev.mate),
                            depth: depth.unwrap_or(prev_ev.depth),
                            nodes: nodes.unwrap_or(prev_ev.nodes),
                        });
                    }
                    _ => continue,
                }
            }
        });
        return engstate;
    }
}

#[derive(PartialEq, Debug)]
enum UCI {
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
enum UCIError {
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

fn parse_uci(line: String) -> Result<UCI> {
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
    use anyhow::Result;

    use crate::{parse_info_line, ChessEngine, Engine, UCI};

    macro_rules! test_file {
        ($fname:expr) => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/res/test/", $fname)
        };
    }

    #[tokio::test]
    async fn test_sf() -> Result<()> {
        let mut sf = Engine::new(test_file!("fakefish.sh")).await?;
        sf.start_uci().await?;
        Ok(())
    }

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
