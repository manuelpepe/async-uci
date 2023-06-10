use crate::parse::{parse_uci, OptionType, UCI};
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
pub trait ChessEngine {
    /// Start the UCI Protocol
    async fn start_uci(&mut self) -> Result<()>;

    /// Notify engine of new game start
    async fn new_game(&mut self) -> Result<()>;

    /// Notify engine of new position to search
    async fn set_position(&mut self, position: &str) -> Result<()>;

    /// Notify engine to search for best move until explicitly stopped
    async fn go_infinite(&mut self) -> Result<()>;

    /// Notify engine to search for best move to a certain depth
    async fn go_depth(&mut self, plies: usize) -> Result<()>;

    /// Notify engine to search for best move for a set time
    async fn go_time(&mut self, ms: usize) -> Result<()>;

    /// Notify engine to search for a mate in a certain number of moves
    async fn go_mate(&mut self, mate_in: usize) -> Result<()>;

    /// Notify engine to stop current search
    async fn stop(&mut self) -> Result<()>;

    /// Retrieve the latest evaluation from the engine
    async fn get_evaluation(&mut self) -> Option<Evaluation>;

    /// Retrieve the list of available options from the engine
    async fn get_options(&mut self) -> Result<Vec<EngineOption>>;

    /// Set an option in the engine
    async fn set_option(&mut self, option: String, value: String) -> Result<()>;
}

/// Engine can be created to spawn any Chess Engine that implements the UCI Protocol
pub struct Engine {
    stdin: ChildStdin,
    state: EngineState,
    _proc: Child,
}

impl Engine {
    pub async fn new(exe_path: &str) -> Result<Self> {
        let (proc, stdin, stdout) = spawn_process(exe_path)?;
        let state = EngineState::new(stdout).await;
        Ok(Engine {
            state: state,
            stdin: stdin,
            _proc: proc,
        })
    }
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

    /// Change current engine state
    async fn set_state(&mut self, new_state: EngineStateEnum) -> Result<()> {
        // TODO: Return old state
        let mut state = self.state.state.lock().expect("couldn't acquire lock");
        *state = new_state;
        Ok(())
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
    async fn start_uci(&mut self) -> Result<()> {
        self.send_command("uci\n".to_string()).await?;
        self.expect_uciok().await?;
        self.send_command("isready\n".to_string()).await?;
        self.expect_readyok().await?;
        Ok(())
    }

    async fn new_game(&mut self) -> Result<()> {
        self.send_command("ucinewgame\n".to_string()).await?;
        self.set_state(EngineStateEnum::Initialized).await?;
        self.send_command("isready\n".to_string()).await?;
        self.expect_readyok().await?;
        Ok(())
    }

    async fn set_position(&mut self, fen: &str) -> Result<()> {
        let cmd = format!("position fen {}\n", fen);
        self.send_command(cmd.to_string()).await
    }

    async fn go_infinite(&mut self) -> Result<()> {
        self.send_command("go infinite\n".to_string()).await?;
        self.set_state(EngineStateEnum::Thinking).await?;
        Ok(())
    }

    async fn go_depth(&mut self, depth: usize) -> Result<()> {
        self.send_command(format!("go depth {}\n", depth).to_string())
            .await?;
        self.set_state(EngineStateEnum::Thinking).await?;
        Ok(())
    }

    async fn go_time(&mut self, ms: usize) -> Result<()> {
        self.send_command(format!("go movetime {}\n", ms).to_string())
            .await?;
        self.set_state(EngineStateEnum::Thinking).await?;
        Ok(())
    }

    async fn go_mate(&mut self, mate_in: usize) -> Result<()> {
        self.send_command(format!("go mate {}\n", mate_in).to_string())
            .await?;
        self.set_state(EngineStateEnum::Thinking).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.send_command("stop\n".to_string()).await?;
        self.set_state(EngineStateEnum::Initialized).await?;
        Ok(())
    }

    async fn get_evaluation(&mut self) -> Option<Evaluation> {
        let ev = self.state.evaluation.lock().expect("couldn't acquire lock");
        return match &*ev {
            Some(e) => Some(e.clone()),
            None => None,
        };
    }

    async fn get_options(&mut self) -> Result<Vec<EngineOption>> {
        let options = self.state.options.lock().expect("couldn't acquire lock");
        Ok(options.clone())
    }

    async fn set_option(&mut self, option: String, value: String) -> Result<()> {
        let cmd = format!("setoption name {} value {}\n", option, value);
        self.send_command(cmd).await
    }
}

/// Engine evaluation info
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub score: isize,
    pub mate: isize,
    pub depth: isize,
    pub nodes: isize,
    pub seldepth: isize,
    pub multipv: isize,
    pub pv: Vec<String>,
    pub time: isize,
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
        f.write_fmt(format_args!(
            "score: {} mate: {} depth: {} nodes: {} seldepth: {} multipv: {} time: {}",
            self.score, self.mate, self.depth, self.nodes, self.seldepth, self.multipv, self.time
        ))?;
        if f.alternate() {
            f.write_fmt(format_args!("\npv: {}", self.pv.join(", ")))?;
        }
        Ok(())
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

#[derive(PartialEq, Debug, Clone)]
pub struct EngineOption {
    pub name: String,
    pub opt_type: OptionType,
}

/// Engine state handler with async stdout parsing
struct EngineState {
    state: Arc<Mutex<EngineStateEnum>>,
    evaluation: Arc<Mutex<Option<Evaluation>>>,
    options: Arc<Mutex<Vec<EngineOption>>>,
}

impl EngineState {
    async fn new(stdout: ChildStdout) -> Self {
        let ev = Arc::new(Mutex::new(None));
        let state = Arc::new(Mutex::new(EngineStateEnum::Uninitialized));
        let options = Arc::new(Mutex::new(Vec::new()));
        let stdout = BufReader::new(stdout);
        let engstate = EngineState {
            state: state.clone(),
            evaluation: ev.clone(),
            options: options.clone(),
        };
        tokio::spawn(async move {
            Self::process_stdout(stdout, state.clone(), ev.clone(), options.clone()).await
        });
        return engstate;
    }

    async fn process_stdout(
        mut stdout: BufReader<ChildStdout>,
        state: Arc<Mutex<EngineStateEnum>>,
        ev: Arc<Mutex<Option<Evaluation>>>,
        options: Arc<Mutex<Vec<EngineOption>>>,
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
                Ok(UCI::Option { name, opt_type }) => {
                    let mut options = options.lock().expect("couldn't aquire options lock");
                    options.push(EngineOption { name, opt_type });
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
