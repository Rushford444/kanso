use std::{fs, process::Command};

#[test]
fn m5_paths_survive_all_check_formats_while_messages_hide_values() {
    let dir = tempfile::tempdir().unwrap();
    let name = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let folder = dir.path().join(name);
    fs::create_dir(&folder).unwrap();
    let path = folder.join("AGENTS.md");
    let secret = "AKIAIOSFODNN7EXAMPLE";
    fs::write(&path, format!("# Title\n\ncredential: {secret}\n")).unwrap();
    for args in [
        vec!["check", "--json"],
        vec!["check", "--format", "sarif"],
        vec!["check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(name), "path missing: {stdout}");
        assert!(!stdout.contains(secret), "message leaked: {stdout}");
        if stdout.starts_with('{') {
            let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
            if args_are_sarif(&parsed) {
                assert!(
                    parsed["runs"][0]["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(
                            |r| r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                                .as_str()
                                .unwrap()
                                .contains(name)
                        )
                );
            }
        }
    }
}
fn args_are_sarif(value: &serde_json::Value) -> bool {
    value.get("runs").is_some()
}

#[test]
fn m6_detects_contextual_aws_and_hex_without_flagging_unrelated_hex() {
    let aws = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    let hex = "0123456789abcdef0123456789abcdef";
    let source = format!(
        "# Credentials\n\naws_secret_access_key = {aws}\napi_token = {hex}\ncommit = {hex}\n"
    );
    let doc = kanso::Document {
        path: "AGENTS.md".into(),
        source,
    };
    let found =
        kanso::rules::analyze(&doc, &kanso::Config::default(), std::path::Path::new(".")).unwrap();
    let lines: Vec<_> = found
        .iter()
        .filter(|d| d.rule_id == "KS007")
        .map(|d| d.line)
        .collect();
    assert!(lines.contains(&3), "AWS missing: {lines:?}");
    assert!(lines.contains(&4), "hex missing: {lines:?}");
    assert!(!lines.contains(&5), "unrelated hex flagged: {lines:?}");
    for d in found {
        assert!(!d.message.contains(aws) && !d.message.contains(hex));
    }
}

#[test]
fn m7_pem_message_stays_on_one_text_line() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("AGENTS.md"),
        "# Keys\n\njson: \"-----BEGIN PRIVATE KEY-----\\nYWJjZGVmZ2g=\\n\"\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap()
    };
    let json = run(&["check", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    let count = report["diagnostics"].as_array().unwrap().len();
    assert!(count > 0);
    let text = String::from_utf8(run(&["check"]).stdout).unwrap();
    assert_eq!(text.lines().count(), count + 1, "{text}");
}

#[test]
fn contextual_keywords_respect_identifier_segments() {
    let hex = "0123456789abcdef0123456789abcdef";
    for name in [
        "monkey",
        "MONKEY",
        "TURKEY",
        "DONKEY",
        "HOCKEY",
        "MONKEY-PATCHING",
        "Monkey",
        "keyboard",
        "Keyboard",
        "KEYBOARD",
        "tokenizer",
        "Tokenizer",
        "TOKENIZER",
        "tokens",
        "secretary",
        "keyring",
    ] {
        let text = format!("{name}: {hex}");
        assert_eq!(kanso::rules::redact(&text), text, "{name}");
    }
    for name in [
        "api_key",
        "APIKEY",
        "apikey",
        "x-apikey",
        "APIKey",
        "fooAPIKey",
        "authtoken",
        "accesstoken",
        "secretkey",
        "privatekey",
        "clientsecret",
        "awssecretkey",
        "AWSSecretKey",
        "secretKey",
        "AWS_SECRET_ACCESS_KEY",
        "token",
        "client secret",
        "api_token budget",
        "token budget2",
        "token budget_suffix",
        "token budget; api key",
    ] {
        let text = format!("{name}: {hex}");
        assert_ne!(kanso::rules::redact(&text), text, "{name}");
    }
}

#[test]
fn ab_corpus_has_sixteen_secret_lines_and_no_prose_findings() {
    let h32 = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
    let h40 = "9fceb02d0ae598e95dc970b74767f19372d61af8";
    let h64 = "3f8a7c2e91b04d5ea6c1f0b29e7d4a8c5b3e2f1a0d9c8b7a6e5f4d3c2b1a0f9e";
    let a40 = &"Xk3pQ9vL2mN8rT5wY1zB/cD4fG7hJ0kL+aS6dF9gH2j"[..40];
    let lines = [
        (
            format!("- Monkey-patching is forbidden; see commit {h40} for the revert."),
            false,
        ),
        (
            format!("- Keyboard shortcut docs live in commit {h40}."),
            false,
        ),
        (
            format!("- The tokenizer vocabulary is verified against sha256 {h64}."),
            false,
        ),
        (
            format!("- Cache key format: deps-{h32} is computed by CI."),
            false,
        ),
        (format!("- the token budget check compares {h64}."), false),
        (format!("- api_key: {h32}"), true),
        (format!("- apiKey = {h32}"), true),
        (format!("- APIKEY={h32}"), true),
        (format!("- apikey: {h32}"), true),
        (format!("- x-apikey: {h32}"), true),
        (format!("- APIKey={h32}"), true),
        (format!("- authtoken={h64}"), true),
        (format!("- accessToken: {h64}"), true),
        (format!("- secretkey={h40}"), true),
        (format!("- client_secret={h40}"), true),
        (format!("- TOKEN={h64}"), true),
        (format!("- AWS_SECRET_ACCESS_KEY={a40}"), true),
        (format!("- AwsSecretAccessKey: {a40}"), true),
        (format!("- awssecretkey={a40}"), true),
        (format!("- AWSSecretKey={a40}"), true),
        (format!("- aws secret key {a40}"), true),
        (format!("- tokens used: {h64}"), false),
    ];
    for (line, expected) in lines {
        assert_eq!(kanso::rules::redact(&line) != line, expected, "{line}");
    }
}

#[test]
fn compact_aws_labels_detect_forty_character_keys() {
    let aws = "Xk3pQ9vL2mN8rT5wY1zB/cD4fG7hJ0kL+aS6dF9g";
    for label in [
        "awssecretkey",
        "AWSSecretKey",
        "AWS_SECRET_ACCESS_KEY",
        "AwsSecretAccessKey",
    ] {
        let text = format!("{label}: {aws}");
        assert_ne!(kanso::rules::redact(&text), text, "{label}");
    }
}

#[test]
fn release_prose_corpus_replays_each_line() {
    let lines = [
        "- Pin every action by full SHA, e.g. `actions/checkout@11d5960a3eeb1b1fa5ac4e1e7ad87c43b9a7b0d1` (keep this pinned; never use a moving key).",
        "- The tokenizer vocabulary is verified against sha256 `3f8a7c2e91b04d5ea6c1f0b29e7d4a8c5b3e2f1a0d9c8b7a6e5f4d3c2b1a0f9e`.",
        "- Cache key format: `deps-0123456789abcdef0123456789abcdef` is computed by CI, do not edit.",
        "- Monkey-patching is forbidden; see commit 9fceb02d0ae598e95dc970b74767f19372d61af8 for the revert.",
        "- Keyboard shortcut docs live in commit a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.",
        "- Secrets are rotated by the platform team; the KMS key id is 1234abcd-12ab-34cd-56ef-1234567890ab.",
        "- AWS keys are never stored here; the bucket policy lives at infra/aws/policies/readonlyBucketAccessKeyPolicy.json.",
        "- Docker image digest: sha256:9b2c1d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c (key: base image).",
        "- Use `git log --format=%H` to get the commit id; the token budget check compares e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.",
    ];
    for value in [
        "deps-0123456789abcdef0123456789abcde",
        "deps-Z8gP2mQ7vL4xN9aR6sT3wY5bC1dF0hKj",
    ] {
        let text = format!("Cache key format: {value}");
        assert_ne!(kanso::rules::redact(&text), text, "{value}");
    }
    let with_credential_label = "api key: Cache key format: deps-0123456789abcdef0123456789abcdef";
    assert_ne!(
        kanso::rules::redact(with_credential_label),
        with_credential_label
    );
    for (index, line) in lines.iter().enumerate() {
        let changed = kanso::rules::redact(line) != *line;
        assert!(!changed, "line {}", index + 1);
    }
}
