# Mythraan Engine

Mythraan is a game engine workspace written in Rust.

Run the workspace checks with the pinned Rust toolchain and `cargo-deny`:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo deny check
```

Licensed under either [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
