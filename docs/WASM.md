# WASM

```bash
rustup target add wasm32-unknown-unknown
cargo build -p repair_nn_wasm --release --target wasm32-unknown-unknown
```

Core has no wasm-bindgen. Adapter only in repair_nn_wasm.
