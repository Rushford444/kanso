use std::{fs, process::Command};

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn secret_in_path_is_reported_without_changing_location() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let nested = dir.path().join(secret);
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    for args in [
        &["check", "--json"][..],
        &["check"][..],
        &["check", "--format", "sarif"][..],
    ] {
        let out = run(dir.path(), args);
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(text.contains(secret), "location must remain intact: {text}");
        assert!(text.contains("secret-like token in path"), "{text}");
        if args.contains(&"--json") {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            let d = v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["rule_id"] == "KS007")
                .unwrap();
            assert!(!d["message"].as_str().unwrap().contains(secret));
        }
    }
}

#[test]
fn invalid_config_is_usage_error_with_safe_validation_reason() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "max_file_bytes = 0\n").unwrap();
    for args in [&["check"][..], &["check", "--json"][..]] {
        let out = run(dir.path(), args);
        assert_eq!(out.status.code(), Some(2));
        let text = format!(
            "{}{}",
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap()
        );
        assert!(
            text.contains("max_file_bytes must be greater than zero"),
            "{text}"
        );
    }
}

#[test]
fn invalid_toml_does_not_disclose_values_or_misidentify_rule() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    fs::write(
        dir.path().join("kanso.toml"),
        format!("invalid_option = \"{secret}\"\n"),
    )
    .unwrap();
    for args in [&["check"][..], &["check", "--json"][..]] {
        let out = run(dir.path(), args);
        assert_eq!(out.status.code(), Some(2));
        let text = format!(
            "{}{}",
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap()
        );
        assert!(text.contains("invalid configuration"), "{text}");
        assert!(!text.contains("unknown disabled rule"), "{text}");
        assert!(!text.contains(secret), "secret leaked");
    }
}

#[cfg(unix)]
#[test]
fn local_instruction_alias_does_not_change_score_or_exit_code() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    symlink("AGENTS.md", dir.path().join("CLAUDE.md")).unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["score"], 100);
    let report: kanso::Report = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(kanso::score::score(&report.diagnostics), report.score);
    assert_eq!(v["files"].as_array().unwrap().len(), 1);
    let diagnostics = v["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["message"] == "symbolic link alias")
    );
    assert!(!diagnostics.iter().any(|d| d["rule_id"] == "KS000"));
}

#[test]
fn common_cargo_extensions_are_recognized() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\n\n```sh\ncargo nextest run\ncargo deny check\ncargo insta test\ncargo mutants\ncargo clippy\ncargo fmt\ncargo not-a-command\n```\n").unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let findings: Vec<_> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["rule_id"] == "KS009")
        .collect();
    assert_eq!(findings.len(), 1, "{findings:?}");
}

#[test]
fn disabled_rule_rejects_ks000_and_path_scan_can_be_disabled() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("kanso.toml"),
        "disabled_rules = [\"KS000\"]\n",
    )
    .unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown disabled rule")
    );
    fs::write(
        dir.path().join("kanso.toml"),
        "disabled_rules = [\"KS007\"]\n",
    )
    .unwrap();
    let nested = dir.path().join("ghp_abcdefghijklmnopqrstuvwxyz1234567890");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        !value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS007")
    );
}

#[cfg(unix)]
#[test]
fn fifo_has_explicit_non_regular_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(dir.path().join("AGENTS.md"))
            .status()
            .unwrap()
            .success()
    );
    let out = run(dir.path(), &["check", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS000" && d["message"] == "not a regular file")
    );
}

#[cfg(unix)]
#[test]
fn relative_alias_works_for_all_commands() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    symlink("AGENTS.md", dir.path().join("CLAUDE.md")).unwrap();
    let absolute = dir.path().join("CLAUDE.md");
    for name in ["CLAUDE.md", "./CLAUDE.md", absolute.to_str().unwrap()] {
        for command in ["check", "score", "fix"] {
            let out = run(dir.path(), &[command, name, "--json"]);
            assert_eq!(
                out.status.code(),
                Some(0),
                "{command} {name}: {}",
                String::from_utf8_lossy(&out.stdout)
            );
        }
    }
}

#[test]
fn ordinary_hyphenated_paths_are_not_secrets() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "release-2026-11-05-team-review-v2b-final",
        "my-project-2026-11-05-release-candidate",
        "target-aarch64-unknown-linux-gnu-abcdef1234567890",
    ] {
        let nested = dir.path().join(name);
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    }
    let out = run(dir.path(), &["check", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        !value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS007")
    );
}

#[test]
fn directory_named_as_instruction_is_traversal_failure() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("AGENTS.md")).unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS000" && d["message"] == "cannot inspect or traverse path")
    );
}

#[test]
fn fix_does_not_silently_convert_utf16() {
    for le in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        let mut original = if le {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in "# Instructions\n\n- item \n".encode_utf16() {
            original.extend(if le {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        fs::write(&path, &original).unwrap();
        let out = run(dir.path(), &["fix", "--write"]);
        let current = fs::read(&path).unwrap();
        assert!(out.status.success());
        assert!(current.starts_with(&original[..2]));
        assert_eq!(
            kanso::discovery::decode_regular_bytes(&current).unwrap(),
            "# Instructions\n\n- item\n"
        );
    }
}
