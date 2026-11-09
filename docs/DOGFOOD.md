# Dogfood

Run `kanso check .` from the repository root to inspect its instruction files.
The checked-in `kanso.toml` excludes the intentional rule fixtures. Findings
require review before changes are made; the command exits 1 for non-alias
diagnostics. A score of 100 is not a security audit.

For a small reproducible broken-link example, see `docs/demo.tape`. For a
synthetic large-tree measurement, see [BENCH.md](BENCH.md). Neither example
proves accuracy on a private or independently maintained instruction corpus.
