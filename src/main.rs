use anyhow::Result;
use clap::Parser;
use cli::CLIArgs;
use engine::{ChessEngine, Engine, Evaluation};
use tokio::task::yield_now;

mod cli;
mod engine;
mod parse;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CLIArgs::parse();
    let engpath =
        String::from("/root/rust/something_chess/res/stockfish/stockfish-ubuntu-20.04-x86-64");
    let mut sf = spawn_engine(engpath, args.fen).await.unwrap();
    stream_engine_eval(&mut sf, args.show_moves).await.unwrap();
    Ok(())
}

async fn spawn_engine(path: String, fen: String) -> Result<Engine> {
    let mut eng = Engine::new(&path).await?;
    eng.start_uci().await?;
    eng.new_game().await?;
    eng.set_position(&fen).await?;
    eng.go_infinite().await?;
    Ok(eng)
}

async fn stream_engine_eval(engine: &mut Engine, show_moves: bool) -> Result<()> {
    let mut last_eval = Evaluation::default();
    loop {
        if let Some(ev) = engine.get_evaluation().await {
            if ev != last_eval {
                if show_moves {
                    println!("{ev:#}");
                } else {
                    println!("{ev:}")
                }
                last_eval = ev;
            }
        }
        yield_now().await;
    }
}
