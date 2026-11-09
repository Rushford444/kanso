# kanso

[English](README.md) | [Français](README.fr.md)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/banner-dark.jpg">
  <img alt="kanso: messy instruction sheets cleaned step by step" src="docs/assets/banner-light.jpg" width="100%">
</picture>

Catch broken links, possible leaked secrets, and oversized context in agent instructions before the next run.

[CI](https://github.com/Rushford444/kanso/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) [![Latest release](https://img.shields.io/badge/release-v0.1.0-blue)](https://github.com/Rushford444/kanso/releases/latest)

Rename a setup guide without updating `AGENTS.md`, and an agent may follow a dead link. Repeat the same instruction across files, and it consumes context that could carry useful work. Kanso checks supported files locally and points to findings for review. It cannot guarantee an agent will follow the instructions.

## A real check

In a directory with an `AGENTS.md` containing a link to a missing `missing.md`, this is actual output from v0.1.0:

```text
$ kanso check .
./AGENTS.md:5:5: warning KS004: relative link target or anchor not found
Score: 95 / 100 (15 tokens)
$ kanso score .
./AGENTS.md: 95 / 100
Global: 95 / 100
$ kanso explain KS004
A relative Markdown link refers to a missing file or heading anchor.
```

The first command exits 1 because it found a warning; the other two exit 0. The [recording script](docs/demo.tape) recreates the fixture.

## Install

Download the archive for your platform from the [v0.1.0 release](https://github.com/Rushford444/kanso/releases/tag/v0.1.0) into a new, empty directory. These builds target x86-64 Linux (glibc), Intel and Apple Silicon macOS, and x86-64 Windows. Other architectures require a source build. Without `gh`, download the same two files from that page.

The release publishes `SHA256SUMS`, one SHA-256 line per archive; the commands below verify only the selected line. Checksums detect corruption, not a compromised release account.

Linux (x86-64):

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz$' SHA256SUMS | sha256sum -c -
tar -xzf kanso-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

macOS (Intel):

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-apple-darwin.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-apple-darwin.tar.gz$' SHA256SUMS | shasum -a 256 -c -
tar -xzf kanso-0.1.0-x86_64-apple-darwin.tar.gz
```

macOS (Apple Silicon):

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-aarch64-apple-darwin.tar.gz' -p SHA256SUMS
grep 'kanso-0.1.0-aarch64-apple-darwin.tar.gz$' SHA256SUMS | shasum -a 256 -c -
tar -xzf kanso-0.1.0-aarch64-apple-darwin.tar.gz
```

Windows (x86-64, Git Bash):

```sh
gh release download v0.1.0 -R Rushford444/kanso -p 'kanso-0.1.0-x86_64-pc-windows-msvc.zip' -p SHA256SUMS
grep 'kanso-0.1.0-x86_64-pc-windows-msvc.zip$' SHA256SUMS | sha256sum -c -
unzip kanso-0.1.0-x86_64-pc-windows-msvc.zip
```

Move the extracted binary to a directory on PATH. Each archive also contains the READMEs, LICENSE, CONTRIBUTING.md, SECURITY.md and the documentation pages the READMEs link, so those links work inside the archive.

macOS builds are not notarized; depending on the target, the binary may be signed ad hoc or unsigned. Notarization would require an Apple Developer account. A download through a browser can add the quarantine attribute, and Gatekeeper may then refuse to run the binary or stall its first launch while scanning it. In that case, remove the attribute before running it:

```sh
xattr -d com.apple.quarantine kanso
./kanso --version
```

A build from source with Cargo does not carry the quarantine attribute.

Alternatively, install from Git (requires Rust and network access):

```sh
GIT_TERMINAL_PROMPT=0 cargo install --git https://github.com/Rushford444/kanso --tag v0.1.0 --locked
```

Git installation requires network access and, while the repository is private, credentials preconfigured for Git. `GIT_TERMINAL_PROMPT=0` disables Git's terminal prompts: when credentials are missing, authentication fails instead of requesting them. Once the repository is public, the same command needs only network access.

The minimum supported Rust version is 1.88.0, because the locked `serde-saphyr` 0.0.21 uses let-chains stabilized in Rust 1.88.0; edition 2024 sets the floor at 1.85.0. The CI `msrv` job builds, lints, and tests that exact version. A crates.io release is planned, not available yet. To verify from this checkout without a published release, build with `cargo build --locked` and run `target/debug/kanso` (on Windows, `target\debug\kanso.exe`).

## Start in three commands

From your project's root, after installation:

```sh
kanso init
kanso check .
kanso score .
```

`init` creates `kanso.toml` and will not overwrite it. `check` exits 1 for any non-alias diagnostic, including informational findings; `score` exits 0 when analysis succeeds, including when findings exist.

## Common tasks

```sh
kanso explain KS007
kanso fix .
kanso fix . --write
```

## JSON and SARIF

```sh
kanso check . --json
kanso check . --format sarif > results.sarif
```

`fix` previews by default; `--write` changes files. Review the diff. SARIF redirection creates or replaces `results.sarif`, including when findings make the command exit 1. JSON uses `kanso/v1`; SARIF uses 2.1.0. A SARIF report declares every rule in `runs[0].tool.driver.rules` and links each result to its rule with `ruleId` and `ruleIndex`.

## Configuration

`kanso init` writes exactly this file:

```toml
# kanso configuration; uncomment to override the defaults.
# Warning and error thresholds for a single file, in estimated tokens.
# token_warn = 2000
# token_error = 6000
# Aggregate root-context limits.
# total_token_warn = 10000
# total_token_error = 20000
# similarity_threshold = 0.85
# max_file_bytes = 2097152
# disabled_rules = []
# ignore = []
```

See the [configuration reference](docs/configuration.md) and [usage guide](docs/usage.md).

## GitHub Actions

This workflow installs the pinned tag and fails when non-alias diagnostics are present. It requires network access and, while the repository is private, Git credentials preconfigured on the runner; it does not upload SARIF.

```yaml
name: Agent instructions
on: [push, pull_request]
permissions:
  contents: read
jobs:
  kanso:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # stable
      - run: GIT_TERMINAL_PROMPT=0 cargo install --git https://github.com/Rushford444/kanso --tag v0.1.0 --locked
      - run: kanso check .
```

For a SARIF artifact, redirect the SARIF command above, preserve its exit status, and use a separate upload step. Paths are not redacted from reports.

## For AI agents

Install from the pinned Git command above without prompts; it sets `GIT_TERMINAL_PROMPT=0`, so missing Git credentials fail instead of prompting. For an offline checkout use the local build. `kanso check . --json` gives machine-readable diagnostics; `--quiet` suppresses human success output. Commands are noninteractive; `--yes` is accepted but no confirmation is needed. For noninteractive CI, use `CI=true kanso check . --json --yes`. No network is needed for analysis. Exit codes are 0 success, 1 non-alias diagnostics from `check`, 2 usage error, and 3 runtime error. `score` reports findings but exits 0 on successful analysis. In CI, handle 1 as findings rather than a crash.

Paste into `AGENTS.md` or `CLAUDE.md`:

```markdown
## Instruction-file checks

Run `kanso check . --json` before submitting instruction-file changes.
Treat exit 1 as findings; inspect the report. Exit 2 is usage error and 3 is runtime error.
Preview `kanso fix .` before using `--write`; review every resulting diff.
```

## What is checked

Kanso discovers supported instruction files with gitignore rules. It checks skill frontmatter (KS001), file and root token budgets (KS002–KS003), links (KS004), exact and similar instructions (KS005–KS006), secret-like strings (KS007), headings (KS008), and unrecognized shell commands as informational findings (KS009). KS000 reports I/O problems and is not configurable. Root cross-file checks apply only to co-loaded root files; `SKILL.md` and nested scopes are excluded from the root context. Per-file checks still apply. Tokens use o200k.

The score starts at 100: each error deducts 15, warning 5, and informational finding 1, capped at 30 per rule and floored at zero. Global scoring applies this formula to all diagnostics, not an average. Aliases are not scored and do not cause `check` to fail. Analysis is offline; it never executes the commands it finds.

## Limits

This is static analysis, not a security audit or proof that instructions are correct. KS007 redacts detected values but not paths; do not put secrets in filenames. Its contextual detection looks back at most 128 bytes on the same line and has documented exclusions, so it can miss secrets. Fixes are conservative: raw HTML files are skipped and whitespace fixes are limited to simple list lines. Symlinked regular files inside the project are aliases; a target outside the selected subtree can still be analyzed, even if ignored. Unsafe or non-regular targets are reported.

## FAQ

**Does it send my files anywhere?** No. Analysis is offline; Git installation and CI setup require network access.

**Why did `check` fail on an informational finding?** Any non-alias diagnostic causes exit 1, so CI can choose its own policy from JSON if needed.

**Does a score of 100 guarantee safety?** No. It only means these checks produced no score deductions.

**Can I run it without writing files?** Yes. `check`, `score`, `explain`, and the default `fix` preview do not write changes. `init`, `fix --write`, and shell redirection do.

**Is it an agent or a general Markdown linter?** Neither. It is a focused static checker for supported agent instruction files.

## Alternatives

[markdownlint](https://github.com/DavidAnson/markdownlint) checks general Markdown style; Kanso focuses on agent instructions and cross-file context. [pre-commit](https://pre-commit.com/) orchestrates hooks rather than providing Kanso's checks; you can run Kanso as a hook. They solve different problems and can be used together.

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development and checks, and [SECURITY.md](SECURITY.md) for private security reporting.

## License

[MIT](LICENSE). Copyright (c) 2026 Kodawari.
