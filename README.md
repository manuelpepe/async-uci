# UCI Implementation in Rust

## Usage

Go to [Stockfish Downloads](https://stockfishchess.org/download/) and download the latest stockfish version for your system.
Place the downloaded files in `res/stockfish` and either set the `CHESS_ENGINE_PATH` environment variable or the global `-P` param 
to the stockfish executable location.

Now try running:

```
cargo test
cargo run -- search --fen 'r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8' --lines 3 --show-moves
```
