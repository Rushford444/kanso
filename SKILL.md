---
name: kanso
description: Use when checking coding-agent instruction files for broken links, token budgets, duplicate instructions, unsafe secrets, or structure.
---

# Kanso

## Install

Install noninteractively with `cargo install --git https://github.com/Rushford444/kanso --locked`. See the README for release binaries.

## Check a project

Run `kanso check .` from its root. Use `kanso check . --json` for versioned JSON or `kanso check . --format sarif` for SARIF 2.1.0. The checker runs offline and does not execute commands in documents. Commands do not prompt; `--yes` is accepted globally but is not needed for confirmation, including in CI.

## Review and fix

Use `kanso score .` for the score and `kanso explain KS007` to explain a rule. `kanso fix .` previews conservative fixes; use `kanso fix . --write` only when changes are wanted. Inspect the diff afterward.

## Exit codes

`check` returns 1 for any non-alias finding, including informational findings. A successful `score` returns 0 even when findings exist. Invalid usage, configuration, or a missing input returns 2; runtime failures return 3. Successful operations return 0.
