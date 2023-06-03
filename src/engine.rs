use crate::parse::{parse_uci, UCI};
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::{
    fmt::Display,
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

/// ChessEngine trait can be implemented for structures that implement the UCI Protocol
#[async_trait]
pub trait ChessEngine: Sized {
    /// Create new engine from executable
    async fn new(exe_path: &str) -> Result<Self>;

    /// Start the UCI Protocol
    async fn start_uci(&mut self) -> Result<()>;

    /// Notify engine of new game start
    async fn new_game(&mut self) -> Result<()>;

    /// Notify engine of new position to search
    async fn set_position(&mut self, position: &str) -> Result<()>;

    /// Notify engine to search for best move until explicitly stopped
    async fn go_infinite(&mut self) -> Result<()>;

    /// Retrieve the latest evaluation from the engine
    async fn get_evaluation(&mut self) -> Option<Evaluation>;
}

/// Engine can be created to spawn any Chess Engine that implements the UCI Protocol
pub struct Engine {
    stdin: ChildStdin,
    state: EngineState,
    _proc: Child,
}

impl Engine {
    /// Send a command to the engine
    async fn send_command(&mut self, command: String) -> Result<()> {
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Check if the expected state is the current engine state
    async fn _expect_state(&mut self, exp_state: &EngineStateEnum) -> Result<()> {
        let state = self.state.state.lock().expect("couldn't aquire state lock");
        if *exp_state == *state {
            return Ok(());
        }
        bail!("engine didn't respond with {:?}", exp_state)
    }

    /// Check if the expected state is the current engine state, retries a couple of times
    /// waiting between attempts.
    async fn expect_state(&mut self, exp_state: EngineStateEnum) -> Result<()> {
        for _ in 0..10 {
            match self._expect_state(&exp_state).await {
                Ok(_) => return Ok(()),
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            };
        }
        bail!("engine didn't respond with {:?}", exp_state)
    }

    /// Check if the engine initialized UCI
    async fn expect_uciok(&mut self) -> Result<()> {
        self.expect_state(EngineStateEnum::Initialized).await
    }

    /// Check if the engine is ready to receive commands
    async fn expect_readyok(&mut self) -> Result<()> {
        self.expect_state(EngineStateEnum::Ready).await
    }
}

/// Spawn a subprocess and return handles for stdin and stdout
fn spawn_process(exe_path: &str) -> Result<(Child, ChildStdin, ChildStdout)> {
    let mut cmd = Command::new(exe_path);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    let mut proc = cmd.spawn()?;
    let stdout = proc.stdout.take().expect("no stdout available");
    let stdin = proc.stdin.take().expect("no stdin available");
    Ok((proc, stdin, stdout))
}

#[async_trait]
impl ChessEngine for Engine {
    async fn new(exe_path: &str) -> Result<Self> {
        let (proc, stdin, stdout) = spawn_process(exe_path)?;
        let state = EngineState::new(stdout).await;
        Ok(Engine {
            state: state,
            stdin: stdin,
            _proc: proc,
        })
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

/// Engine evaluation info
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    score: isize,
    mate: isize,
    depth: isize,
    nodes: isize,
    seldepth: isize,
    multipv: isize,
    pv: Vec<String>,
    time: isize,
}

impl Default for Evaluation {
    /// Create evaluation with empty values
    fn default() -> Self {
        Evaluation {
            score: 0,
            mate: 0,
            depth: 0,
            nodes: 0,
            seldepth: 0,
            multipv: 0,
            pv: vec![],
            time: 0,
        }
    }
}

impl Display for Evaluation {
    /// The alternate ("{:#}") operator will add the moves in pv to the output
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!(
                "score: {} mate: {} depth: {} nodes: {} seldepth: {} multipv: {} time: {} \npv: {}",
                self.score,
                self.mate,
                self.depth,
                self.nodes,
                self.seldepth,
                self.multipv,
                self.time,
                self.pv.join(", ")
            ))
        } else {
            f.write_fmt(format_args!(
                "score: {} mate: {} depth: {} nodes: {} seldepth: {} multipv: {} time: {}",
                self.score,
                self.mate,
                self.depth,
                self.nodes,
                self.seldepth,
                self.multipv,
                self.time
            ))
        }
    }
}

/// Posible engine states
#[derive(PartialEq, Debug)]
enum EngineStateEnum {
    Uninitialized,
    Initialized,
    Ready,
    Thinking,
}

/// Engine state handler with async stdout parsing
struct EngineState {
    state: Arc<Mutex<EngineStateEnum>>,
    evaluation: Arc<Mutex<Option<Evaluation>>>,
}

impl EngineState {
    async fn new(stdout: ChildStdout) -> Self {
        let ev = Arc::new(Mutex::new(None));
        let state = Arc::new(Mutex::new(EngineStateEnum::Uninitialized));
        let stdout = BufReader::new(stdout);
        let engstate = EngineState {
            state: state.clone(),
            evaluation: ev.clone(),
        };
        tokio::spawn(async move { Self::process_stdout(stdout, state.clone(), ev.clone()).await });
        return engstate;
    }

    async fn process_stdout(
        mut stdout: BufReader<ChildStdout>,
        state: Arc<Mutex<EngineStateEnum>>,
        ev: Arc<Mutex<Option<Evaluation>>>,
    ) {
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
                    cp,
                    mate,
                    depth,
                    nodes,
                    seldepth,
                    time,
                    multipv,
                    pv,
                }) => {
                    let mut ev = ev.lock().expect("couldn't aquire ev lock");
                    let def_ev = Evaluation::default();
                    let prev_ev = match ev.as_ref() {
                        Some(ev) => ev,
                        None => &def_ev,
                    };
                    *ev = Some(Evaluation {
                        score: cp.unwrap_or(prev_ev.score),
                        mate: mate.unwrap_or(prev_ev.mate),
                        depth: depth.unwrap_or(prev_ev.depth),
                        nodes: nodes.unwrap_or(prev_ev.nodes),
                        seldepth: seldepth.unwrap_or(prev_ev.seldepth),
                        multipv: multipv.unwrap_or(prev_ev.multipv),
                        pv: pv.unwrap_or(prev_ev.pv.clone()),
                        time: time.unwrap_or(prev_ev.time),
                    });
                }
                _ => continue,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use crate::engine::{ChessEngine, Engine};

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
