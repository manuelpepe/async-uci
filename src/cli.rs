use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
pub struct CLIArgs {
    /// FEN string of the position to search
    /// i.e: 'r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8'
    #[arg(short, long)]
    pub fen: String,

    /// Wether moves will be printed or not
    #[arg(short = 'm', long)]
    pub show_moves: bool,
}
