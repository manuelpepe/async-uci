use std::{
    process::Stdio,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
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
        match sf.state.get_line() {
            Some(l) => println!("{}", l),
            None => yield_now().await,
        }
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

#[async_trait]
impl ChessEngine for Engine {
    async fn new(exe_path: &str) -> Result<Self> {
        let (proc, stdin, stdout) = spawn_process(exe_path)?;
        let state = EngineState::new(stdout).await;
        let mut sf = Engine {
            state: state,
            stdin: stdin,
            proc: proc,
        };
        sf.state.expect_header().await?;
        Ok(sf)
    }

    async fn start_uci(&mut self) -> Result<()> {
        self.send_command("uci\n".to_string()).await?;
        self.state.expect_uciok().await?;
        self.send_command("isready\n".to_string()).await?;
        self.state.expect_readyok().await?;
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
        self.send_command("go infinite\n".to_string()).await
    }
}

enum EngineStateEnum {
    Uninitialized,
    Ready,
    Thinking,
}

struct EngineState {
    state: EngineStateEnum,
    queue: Arc<Mutex<Vec<String>>>,
}

impl EngineState {
    async fn new(stdout: ChildStdout) -> Self {
        let queue = Arc::new(Mutex::new(Vec::new()));
        let mut stdout = BufReader::new(stdout);
        let state = EngineState {
            state: EngineStateEnum::Uninitialized,
            queue: queue.clone(),
        };
        let queue = queue.clone();
        tokio::spawn(async move {
            loop {
                let mut str = String::new();
                stdout.read_line(&mut str).await.unwrap();
                let line = str.trim().to_string();
                queue.lock().unwrap().push(line);
            }
        });
        state
    }

    fn get_line(&mut self) -> Option<String> {
        self.queue.lock().unwrap().pop()
    }

    async fn expect_response(&mut self, response: &str) -> Result<()> {
        loop {
            match self.get_line() {
                Some(l) if l.eq(response) => break,
                _ => yield_now().await,
            }
        }
        Ok(())
    }

    async fn expect_header(&mut self) -> Result<()> {
        loop {
            match self.get_line() {
                Some(_) => break,
                None => yield_now().await,
            }
        }
        let _line = self.get_line();
        Ok(())
    }

    async fn expect_uciok(&mut self) -> Result<()> {
        self.expect_response("uciok").await?;
        Ok(())
    }

    async fn expect_readyok(&mut self) -> Result<()> {
        self.expect_response("readyok").await?;
        self.state = EngineStateEnum::Ready;
        Ok(())
    }
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
