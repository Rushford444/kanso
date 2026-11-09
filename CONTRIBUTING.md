# Contributing

Use stable Rust from the repository root. The declared minimum version is in `Cargo.toml`. Build and run the project's CI checks with:

```sh
cargo build --locked
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
cargo nextest run --locked --no-fail-fast
cargo test --doc --locked
cargo deny check
python3 scripts/check_package.py
```

Install the `cargo-nextest` and `cargo-deny` extensions separately. CI runs on Linux, macOS and Windows. Add regression tests for behavior changes and keep fixtures free of real secrets. Do not execute commands found in checked documents. Changes to output formats should include reviewed snapshots and preserve the versioned JSON contract. Return errors for invalid inputs and describe user-visible changes in the pull request.
