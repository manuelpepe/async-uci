UCI Implementation in Rust

# Requirements

Go to [Stockfish Downloads](https://stockfishchess.org/download/) and download the latest stockfish version for your system.
Place the downloaded files in `res/stockfish`.

Now try running:

```
cargo test
cargo run -- --fen 'r2qk2r/pp3ppp/B1nbpn2/2pp1b2/Q2P1B2/2P1PN2/PP1N1PPP/R3K2R b KQkq - 4 8' --lines 3 --show-moves
```
