# Security

Please report security issues privately to the repository maintainers through the hosting platform's private vulnerability reporting feature. Do not put credentials or exploit details in public issues. Replace any exposed credentials immediately. Paths are reported verbatim, including in SARIF; do not place credentials in file or directory names. KS007 messages mask detected values and flag secret-like paths. Contextual detection only examines up to 128 preceding bytes on the same line, so separate-line context may be missed. The checks exclude `token budget` as a credential label and the `cache key format` example with a `deps-` hexadecimal digest; secrets in those contexts may be missed.

The checker is offline and does not execute commands it reads. Building the composite action may download Cargo dependencies on a clean runner; the check itself makes no network request.
