use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about)]
pub struct CLIArgs {
    #[clap(flatten)]
    pub global: GlobalArgs,

    #[clap(subcommand)]
    pub command: Subcommands,
}

#[derive(Args, Debug, Clone)]
pub struct GlobalArgs {
    #[clap(short = 'P', long)]
    pub engine_path: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommands {
    /// Search for moves in a position. If max_depth, max_time and mate_in are 0,
    /// the engine will search until stopped.
    Search {
        /// FEN string of the position to search.
        /// i.e: 'r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8'
        #[arg(short, long)]
        fen: String,

        /// Print moves along with evaluation.
        #[arg(short = 'm', long)]
        show_moves: bool,

        /// Amount of lines to process, each line spawns a new engine process.
        #[arg(short, long, default_value = "1")]
        lines: usize,

        /// Search up to a set depth.
        #[arg(short = 'D', long, default_value = "0")]
        max_depth: usize,

        /// Search for a certain time in milliseconds.
        #[arg(short = 'T', long, default_value = "0")]
        max_time: usize,

        /// Search for a mate in a certain number of moves.
        #[arg(short = 'M', long, default_value = "0")]
        mate_in: usize,
    },

    /// List the available options for the current engine
    ListOptions {},
}
