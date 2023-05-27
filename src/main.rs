use anyhow::Result;
use clap::Parser;
use engine::{ChessEngine, Engine, Evaluation};
use tokio::task::yield_now;

mod cli;
mod engine;
mod parse;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::CLIArgs::parse();
    let sfpath =
        String::from("/root/rust/something_chess/res/stockfish/stockfish-ubuntu-20.04-x86-64");
    let mut sf = Engine::new(&sfpath).await?;
    sf.start_uci().await?;
    sf.new_game().await?;
    sf.set_position(&args.fen).await?;
    sf.go_infinite().await?;
    let mut last_eval = Evaluation::default();
    loop {
        if let Some(ev) = sf.get_evaluation().await {
            if ev != last_eval {
                if args.show_moves {
                    println!("{:#}", ev);
                } else {
                    println!("{:}", ev)
                }
                last_eval = ev;
            }
        }
        yield_now().await;
    }
}
