use std::{
    fmt::Display,
    process::Stdio,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Result};
use async_trait::async_trait;
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
    loop {
        match sf.get_evaluation().await {
            Some(ev) => println!("evaluation is: {ev:?}"),
            None => println!("no evaluation yet"),
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
    proc: Child,
}

impl Engine {
    // TODO: These methods could be implemented in a derive trait like 'SimpleEngine'
    async fn send_command(&mut self, command: String) -> Result<()> {
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }
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
            proc: proc,
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

#[derive(Debug, Clone)]
struct Evaluation {
    score: i8,
    mate: i8,
    depth: u8,
    nodes: u8,
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
                        *ev = Some(Evaluation {
                            score,
                            mate,
                            depth,
                            nodes,
                        });
                    }
                    _ => continue,
                }
            }
        });
        return engstate;
    }
}

enum UCI {
    Header,
    UciOk,
    ReadyOk,
    Info {
        score: i8,
        mate: i8,
        depth: u8,
        nodes: u8,
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
        return Ok(UCI::Info {
            score: 1,
            mate: 1,
            depth: 1,
            nodes: 1,
        });
    } else if line.starts_with("uciok") {
        return Ok(UCI::UciOk);
    } else if line.starts_with("readyok") {
        return Ok(UCI::ReadyOk);
    }
    bail!(UCIError::ParseError)
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use crate::{ChessEngine, Engine};

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
}
