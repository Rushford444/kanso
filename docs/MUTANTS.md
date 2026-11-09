# Mutation testing

These historical pre-release campaigns use different candidate sets and test revisions.
They are not a mutation-coverage measurement of the current release. Do not add
their counts as unique mutations.

| Campaign | Candidates | Caught | Missed | Unviable | Timeout |
| --- | ---: | ---: | ---: | ---: | ---: |
| Initial | 177 | 111 | 40 | 26 | 0 |
| Filtered replay, including initial survivors | 62 | 44 | 11 | 6 | 1 |
| Separate diagnostic handling changes | 4 | 4 | 0 | 0 | 0 |

The replay timeout was `context_before` changing `+=` to `*=`. At a non-boundary UTF-8 position, the index remains at that position, causing an infinite loop; a 90-second test timeout detected this denial of service. It is not counted as a native caught outcome.

## Targeted follow-up

The 11 replay survivors include seven addressed by subsequent tests: one masking boundary, one per-file score calculation, one overlapping secret case and four range boundaries. Targeted replays caught all seven (1 + 1 + 1 + 2 + 2). These tests revisit candidates already counted above.

Four other survivors require qualification:

- `src/discovery.rs:46`, `|` to `&`: dropping required Unix open flags could change behavior during a file replacement race. The production guard exists, but a direct regression test is missing.
- `src/engine.rs:114`, removal of the `InvalidInput` arm: behavior can change after a replacement race. The production error handling exists, but a direct regression test is missing.
- `src/discovery.rs:46`, `|` to `^`: on the tested Linux target the two flag bits `0o4000` and `0o100000` are disjoint, so OR and XOR are equal. This conclusion is target-specific.
- `src/rules/mod.rs:548`, `||` to `&&` in workspace filtering: under the current canonical command-line traversal and `ignore` 0.4.30, empty or parent-component patterns cannot add a descendant directory; a possible project-directory match adds only scripts already read. This is a conditional analysis, not a proof for all inputs and glob implementations.

Run `cargo mutants --file src/score.rs --file "src/rules/*.rs"` to inspect mutation results.

## Native campaign A

The campaign used a frozen source/code/test manifest (80/80 files).
The campaign used `cargo-mutants 27.1.0`, `--test-tool nextest`, one job, a 90-second
test timeout, and a green baseline.

Final result: **106 tested — 94 caught, 2 missed, 10 unviable, 0 timeout.** The native
`cargo-mutants` exit code is **2**, caused solely by the two accepted missed mutants; it is
not a test failure or timeout. These counts are distinct from every campaign above and must
not be added to their totals.

| Status | Candidates | Outcomes | Caught | Missed | Unviable | Timeout |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Final | 106 | 106 | 94 | 2 | 10 | 0 |

The two missed mutants are in the same `project_root` canonicalization-failure fallback:

- `src/engine.rs:27:31`, guard `std::fs::symlink_metadata(&input).is_ok()` to `false`:
  the parent fallback is disabled when canonicalization fails after a successful metadata
  read. Non-equivalent, but reachable only through the file-replacement race or an equivalent
  transient failure; no deterministic regression test reaches it.
- `src/engine.rs:29:38`, delete `!`: a non-empty parent now fails the empty-parent filter and
  `.unwrap_or(".")` selects the current directory, so a race can return the wrong project
  root. This is a distinct non-equivalent race gap; no deterministic test triggers it.

Both are accepted coverage gaps, not caught mutants and not equivalent. The production
guards remain in place. The ten unviable mutants are all build failures from invalid
substitutions — three `Default::default()` values for types without `Default`, and seven
invalid `Range` constructors. They never reached the test phase; unviable is not caught.
No additional unexplained survivor is known. The native exit 2 is caused by the two accepted
missed mutants, not by a caught outcome or a timeout.

## Native campaign B

The native campaign tested **75 mutations: 65 caught, 2 missed, 8 unviable, and 0 timed out**. The command exited 2 because of the two missed mutations.

The eight unviable mutations failed to compile: seven invalid `Range` constructors and one unsupported let-chain `&&` to `||` substitution. None reached the tests.

The missed mutations affect the token-budget context boundary and cache-key digest validation. Targeted tests have been added. A separate native replay of the token-budget mutation passed its baseline and caught the one mutation (1 caught, 0 missed; exit 0). A separate native replay of the cache-key digest mutation also passed its baseline and caught the one mutation (1 caught, 0 missed; exit 0). Replay results must not be added to the campaign totals. The two race-related coverage gaps described for campaign A are distinct and remain unchanged.
