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

## ECS startup

Register the component types your world will use during startup, before its
first frame. A type is registered on its first successful insertion into the
world. The 32nd type triggers a one-time build of the despawn membership
index. If a deferred command inserts that type for the first time,
`CommandBuffer::flush` builds the index in the current frame. On the benchmark
host this added about 2 ms with 100,000 live entities.

During startup, spawn a temporary entity with `World::spawn` and use
`World::insert` to attach one value of each type the world will use. Once all
inserts succeed, despawn the temporary entity with `World::despawn`. The world
retains the component storages after despawn. With the `reflect` feature,
`World::register_type` registers reflection metadata only.

Licensed under either [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
