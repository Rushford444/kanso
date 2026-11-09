# Changelog

All notable changes are documented here, following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-11-08

### Added

- Initial release of `kdw-kanso`, with the `kanso` CLI and library.
- Offline instruction-file discovery and checks for frontmatter, token budgets,
  Markdown links and anchors, duplicate instructions, secret-like strings,
  headings and unrecognized shell commands.
- `check`, `score`, `explain`, `init` and conservative `fix` commands;
  deterministic human, `kanso/v1` JSON and SARIF 2.1.0 output with rule metadata.
- English and French documentation, reproducible examples and platform archives
  with SHA-256 checksums for Linux x86-64, macOS Intel and Apple Silicon, and
  Windows x86-64.
- Rust 1.88.0 minimum, tested in a dedicated CI job; MIT license.

### Known limits

- Static checks do not prove safety. Secret detection can miss values and does
  not redact paths. Review every finding and every proposed fix.
- Similarity checks can be quadratic; command recognition is not a shell parser.
- The mutation-testing report records remaining file-replacement race coverage
  gaps. Non-Unix file access does not provide the Unix open-flag protections.
- macOS binaries are not notarized. The READMEs document checksum verification
  and the quarantine workaround.

[0.1.0]: https://github.com/Rushford444/kanso/releases/tag/v0.1.0
