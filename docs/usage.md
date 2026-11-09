# Usage

Run `kanso check .` from the project root. Pass one or more paths to `check`, `score`, or `fix`; with no paths, the current directory is used. Gitignore rules apply to discovery.

```sh
kanso check .
kanso check . --json
kanso check . --format sarif
kanso score .
kanso score . --badge
kanso explain KS007
kanso init
kanso fix .
kanso fix . --write
```

`init` creates a commented `kanso.toml` only if it does not already exist. `fix` previews by default; `--dry-run` explicitly previews, and `--write` applies conservative fixes. Review the diff before committing. Raw HTML files are skipped by fixes; whitespace fixes are limited to simple list lines.

## Output and exit codes

`--json` selects JSON with schema `kanso/v1`. `--format sarif` selects SARIF 2.1.0 for `check`. `--quiet` suppresses human output; `--config PATH` selects a configuration file. `--yes` is accepted globally. Commands are noninteractive.

| Exit | Meaning |
| --- | --- |
| 0 | Success. `score` also returns 0 when findings exist. |
| 1 | `check` found at least one non-alias diagnostic, including informational findings. |
| 2 | Invalid usage or configuration, unknown rule, or missing input path. |
| 3 | Runtime failure. |

KS000 reports I/O errors during analysis; a missing input is instead an exit-2 usage error. `score` begins at 100, deducts 15 per error, 5 per warning and 1 per informational finding, capped at 30 per rule and floored at 0. Alias information is not scored. The global score uses all diagnostics, not an average of file scores.

## Boundaries

The checker works offline and does not run commands from documents. It detects patterns, not every possible secret or contradiction. Secret values in diagnostics are masked but paths are not; never put secrets in file names. Read [security](../SECURITY.md) for limitations and [configuration](configuration.md) for thresholds.
