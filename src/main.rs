use std::process::Stdio;

use anyhow::Result;
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Hello, world!");
    Ok(())
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

/// Stockfish is the most popular Chess Engine
struct Stockfish {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Stockfish {
    // TODO: These methods could be implemented in a derive trait like 'SimpleEngine'
    async fn send_command(&mut self, command: String) -> Result<()> {
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut str = String::new();
        self.stdout.read_line(&mut str).await?;
        Ok(str.trim().to_string())
    }

    async fn wait_for_header(&mut self) -> Result<()> {
        self.read_line().await?;
        Ok(())
    }
}

#[async_trait]
impl ChessEngine for Stockfish {
    async fn new(exe_path: &str) -> Result<Self> {
        let mut cmd = Command::new(exe_path);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        let mut proc = cmd.spawn()?;
        let stdout = proc.stdout.take().expect("no stdout available");
        let stdin = proc.stdin.take().expect("no stdin available");
        let mut sf = Stockfish {
            stdout: BufReader::new(stdout),
            stdin: stdin,
        };
        // spawn process polling in separate task to make sure it makes progress.
        tokio::spawn(async move {
            let status = proc
                .wait()
                .await
                .expect("engine process encountered an error");

            println!("engine status was: {}", status);
        });
        sf.wait_for_header().await?;
        Ok(sf)
    }

    async fn start_uci(&mut self) -> Result<()> {
        self.send_command("uci\n".to_string()).await?;
        loop {
            let line = self.read_line().await?;
            println!("got: {}", &line);
            if line == "uciok" {
                break;
            }
        }
        self.send_command("isready\n".to_string()).await?;
        let line = self.read_line().await?;
        assert_eq!(line, "readyok");
        Ok(())
    }

    async fn new_game(&mut self) -> Result<()> {
        self.send_command("ucinewgame\n".to_string()).await
    }

    // r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8
    async fn set_position(&mut self, fen: &str) -> Result<()> {
        let cmd = format!("position fen {}\n", fen);
        self.send_command(cmd.to_string()).await
    }

    async fn go_infinite(&mut self) -> Result<()> {
        self.send_command("go infinite\n".to_string()).await
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use crate::{ChessEngine, Stockfish};

    macro_rules! test_file {
        ($fname:expr) => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/res/test/", $fname)
        };
    }

    #[tokio::test]
    async fn test_sf() -> Result<()> {
        let mut sf = Stockfish::new(test_file!("fakefish.sh")).await?;
        sf.start_uci().await?;
        Ok(())
    }
}
