use std::process::Stdio;

use anyhow::Result;
use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
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
    fn new(exe_path: &str) -> Result<Self>;
    async fn send_command(&mut self, command: String) -> Result<()>;
    async fn read_bytes(&mut self, buf: &mut [u8]) -> Result<()>;
}

/// Stockfish is the most popular Chess Engine
struct Stockfish {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

#[async_trait]
impl ChessEngine for Stockfish {
    fn new(exe_path: &str) -> Result<Self> {
        let mut cmd = Command::new(exe_path);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        let mut proc = cmd.spawn()?;
        let sf = Stockfish {
            stdout: proc.stdout.take().expect("no stdout available"),
            stdin: proc.stdin.take().expect("no stdin available"),
        };
        // spawn process polling in separate task to make sure it makes progress.
        tokio::spawn(async move {
            let status = proc
                .wait()
                .await
                .expect("engine process encountered an error");

            println!("engine status was: {}", status);
        });
        Ok(sf)
    }

    async fn send_command(&mut self, command: String) -> Result<()> {
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_bytes(&mut self, mut buf: &mut [u8]) -> Result<()> {
        self.stdout.read_buf(&mut buf).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;

    use crate::{ChessEngine, Stockfish};

    #[tokio::test]
    async fn basic_process_io() -> Result<()> {
        let test = "asd".to_string();
        let mut sf = Stockfish::new("/usr/bin/cat")?;
        sf.send_command(test.clone()).await?;
        let mut buf = [0u8; 3];
        sf.read_bytes(&mut buf).await?;
        assert_eq!(String::from_utf8(buf.to_vec())?, test);
        Ok(())
    }

    #[tokio::test]
    async fn basic_process_io_longer() -> Result<()> {
        let test = "my string".to_string();
        let mut sf = Stockfish::new("/usr/bin/cat")?;
        sf.send_command(test.clone()).await?;
        let mut buf = [0u8; 9];
        sf.read_bytes(&mut buf).await?;
        assert_eq!(String::from_utf8(buf.to_vec())?, test);
        Ok(())
    }
}
