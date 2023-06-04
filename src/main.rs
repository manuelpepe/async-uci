use anyhow::{bail, Result};
use clap::Parser;
use cli::{CLIArgs, Subcommands};
use engine::{ChessEngine, Engine, Evaluation};
use tokio::task::yield_now;

mod cli;
mod engine;
mod parse;

#[tokio::main]
async fn main() -> Result<()> {
    let args = CLIArgs::parse();
    let engpath = match args.global.engine_path {
        Some(path) => path,
        None => match std::env::var("CHESS_ENGINE_PATH") {
            Ok(path) => path,
            Err(_) => bail!("Couldn't find engine location. set CHESS_ENGINE_PATH environment variable or pass in --engine-path/-P"),
        },
    };
    println!("Using engine: {engpath}");
    match args.command {
        Subcommands::Search {
            fen,
            show_moves,
            lines,
            max_depth,
            max_time,
            mate_in,
        } => {
            search(
                engpath, fen, lines, show_moves, max_depth, max_time, mate_in,
            )
            .await?
        }
        Subcommands::ListOptions {} => list_options(engpath).await?,
    };
    Ok(())
}

async fn list_options(engpath: String) -> Result<()> {
    let mut eng = Engine::new(&engpath).await?;
    eng.start_uci().await?;
    let options = eng.get_options().await?;
    for opt in options {
        println!("{:?}", opt);
    }
    Ok(())
}

async fn search(
    engpath: String,
    fen: String,
    lines: usize,
    show_moves: bool,
    max_depth: usize,
    max_time: usize,
    mate_in: usize,
) -> Result<()> {
    let mut sf = spawn_engine(engpath, fen, lines.to_string()).await?;
    if max_depth > 0 {
        sf.go_depth(max_depth).await?;
    } else if max_time > 0 {
        sf.go_time(max_time).await?;
    } else if mate_in > 0 {
        sf.go_mate(mate_in).await?;
    } else {
        sf.go_infinite().await?;
    }
    stream_engine_eval(&mut sf, show_moves).await?;
    Ok(())
}

async fn spawn_engine(path: String, fen: String, lines: String) -> Result<Engine> {
    let mut eng = Engine::new(&path).await?;
    eng.start_uci().await?;
    eng.set_option("MultiPV".to_string(), lines).await?;
    eng.new_game().await?;
    eng.set_position(&fen).await?;
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
