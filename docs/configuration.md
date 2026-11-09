# Configuration

`kanso init` creates a commented `kanso.toml` in the current directory without replacing an existing file. By default Kanso looks for `kanso.toml` in the project root inferred from the input paths; `--config PATH` selects a file explicitly. Unknown keys are rejected.

```toml
# Per-file token thresholds
 token_warn = 2000
 token_error = 6000
# Root-context token thresholds
 total_token_warn = 10000
 total_token_error = 20000
# Similarity range: 0.0 through 1.0
 similarity_threshold = 0.85
# Maximum bytes read from an instruction file or linked resource
 max_file_bytes = 2097152
# Rule IDs KS001 through KS009 only
 disabled_rules = []
# Discovery exclusions, using gitignore-style patterns
 ignore = []
```

All values shown are defaults. Warning thresholds must not exceed their error thresholds. The similarity threshold must be finite and between 0 and 1 inclusive, and `max_file_bytes` must be positive. KS000 is an I/O diagnostic, not a configurable rule. Token counts use o200k.

The root context consists of supported root instruction files, root `.cursor/rules/*.mdc`, and `.github/copilot-instructions.md`; it excludes `SKILL.md` and nested scopes. Cross-file checks apply only to this root context; per-file checks apply elsewhere. Gitignore rules also apply. In-project regular-file symlinks are treated as aliases; an alias target may be outside the selected subtree or match an ignore rule and still be analyzed.
