# Local distribution

Run `python3 dist/build.py` to build using the locked dependencies. Cargo may download dependencies that are not cached. The script builds the release binary, packages source with Cargo, and creates a host-specific archive and SHA-256 checksum in `target/dist/`. The archive includes the documentation pages linked from the READMEs. The checksum is written alongside the archive. It does not publish or upload anything.

Rust 1.88.0 or newer and Python 3 are required. Build each target on its own host; this script does not cross-compile.
