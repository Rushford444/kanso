# Contributor instructions

## Project

Kanso is an offline Rust CLI and library that checks AI instruction files. The project uses Rust edition 2024 and declares Rust 1.88.0 as its minimum version. Keep diagnostics deterministic and do not execute commands from scanned documents.

## Build and test

From the repository root, build with `cargo build` and run tests with `cargo test`. Before submitting changes, run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
cargo nextest run
cargo test --doc
cargo deny check
```

The nextest and deny commands require their respective Cargo extensions. CI tests supported platforms; see `.github/workflows/ci.yml`.

## Changes

Add regression tests for behavioral changes. Preserve the versioned JSON contract and review snapshot changes. Do not use real secrets in fixtures. Return typed library errors for invalid input instead of panicking. Avoid unsafe code. Discuss user-visible changes in the pull request.
