# Benchmark

No performance figure is claimed for this release. To measure your own host,
from the repository root:

```sh
cargo build --release --locked
python3 scripts/bench.py
```

The script creates 1,000 directories with one short `AGENTS.md` each under
`target/`, runs one untimed warm-up, then five `kanso check <tree> --json`
processes. It verifies that each report lists exactly 1,000 files and prints
`seconds=[...]; median=...; max=...`. Process startup, discovery, tokenization,
JSON generation and output capture are included. Filesystem caches are warm;
this is not a cold-start or long-document benchmark. Record the exact source
commit, host, toolchain and command alongside any measurement.
