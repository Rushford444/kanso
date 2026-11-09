# Documentation project

This repository builds a static manual for a command-line product. Links should work in the generated site and examples should use the current interface. Review headings and accessible link labels whenever a page is reorganized.

## Working agreements

Start by reading the relevant files and identifying callers before changing behavior. Keep the patch focused on the requested outcome, and record any assumptions that a reviewer will need to verify. Do not update generated artifacts unless the task specifically requires them.

When a change affects a data format, add a representative example and check both accepted and rejected input. Preserve existing public behavior unless the task explicitly permits a breaking change. If a test requires network access, describe that requirement rather than hiding it behind a fallback.

## Verification

Run the smallest relevant test first. After that succeeds, run the project-wide checks available in the local environment. Report the exact command and result, including any skipped check and the reason it could not run. Never claim that a test passed when it was not executed.

```sh
cargo test
cargo fmt --check
```

## Review and delivery

Inspect the final diff for unrelated edits, sensitive information, and accidental generated files. Explain the effect of the change in plain language, and call out operational risks separately from planned improvements. Keep external actions such as publication or deployment behind explicit approval.
