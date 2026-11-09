use std::{fs, process::Command};

fn check(dir: &std::path::Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir)
        .args(["check", "--json"])
        .output()
        .unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn m2_configured_file_limit_reports_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "max_file_bytes = 32\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "a".repeat(33)).unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "# Title\n\nText.\n").unwrap();
    let result = check(dir.path());
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS000" && d["path"].as_str().unwrap().ends_with("AGENTS.md"))
    );
    assert!(
        result["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"].as_str().unwrap().ends_with("CLAUDE.md"))
    );
}

#[test]
fn m3_invalid_utf8_reports_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), [0xff, 0xfe, 0x00]).unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "# Title\n\nText.\n").unwrap();
    let result = check(dir.path());
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS000" && d["path"].as_str().unwrap().ends_with("AGENTS.md"))
    );
    assert!(
        result["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"].as_str().unwrap().ends_with("CLAUDE.md"))
    );
}

#[test]
fn m2_regular_reader_enforces_boundary_and_rejects_binary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "1234").unwrap();
    assert_eq!(kanso::discovery::read_regular(&path, 4).unwrap(), "1234");
    assert!(kanso::discovery::read_regular(&path, 3).is_err());
    fs::write(&path, b"a\0b").unwrap();
    assert!(kanso::discovery::read_regular(&path, 4).is_err());
    let config = kanso::Config {
        max_file_bytes: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[cfg(unix)]
#[test]
fn m3_explicit_symlink_loop_is_diagnostic_not_fatal() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("loop.md");
    symlink("loop.md", &link).unwrap();
    let report =
        kanso::engine::check(std::slice::from_ref(&link), &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "KS000" && d.path == link)
    );
}

#[cfg(unix)]
#[test]
fn m2_reader_refuses_symlink_and_fifo() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    let link = dir.path().join("link");
    fs::write(&path, "text").unwrap();
    symlink(&path, &link).unwrap();
    assert!(kanso::discovery::read_regular(&link, 4).is_err());
    let fifo = dir.path().join("pipe");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(kanso::discovery::read_regular(&fifo, 4).is_err());
}

#[test]
fn missing_input_is_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir.path())
        .args(["check", "missing"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn utf8_bom_heading_has_correct_offsets_and_no_ks008() {
    let source = "\u{feff}# Title\n\nText.\n";
    let parsed = kanso::parser::parse(source);
    assert_eq!(parsed.headings.len(), 1);
    let span = parsed.headings[0].span;
    assert_eq!(&source[span.start..span.end], "# Title\n");
    assert_eq!(span.line, 1);
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), source).unwrap();
    let result = check(dir.path());
    assert!(
        !result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS008")
    );
}

#[test]
fn fix_preview_identifies_first_change_without_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Title\n\n- secret-value \n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir.path())
        .args(["fix"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("AGENTS.md:3: changes start here"));
    assert!(!text.contains("secret-value"));
}

#[cfg(unix)]
#[test]
fn m3_discovered_instruction_symlink_is_reported() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    symlink("missing", dir.path().join("AGENTS.md")).unwrap();
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "KS000" && d.message.contains("symbolic link"))
    );
}

#[test]
fn help_only_lists_exit_codes_used_by_each_command() {
    let dir = tempfile::tempdir().unwrap();
    for command in ["explain", "init", "fix"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(dir.path())
            .args([command, "--help"])
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Exit codes: 0 success; 2 usage error; 3 runtime error."));
        assert!(!text.contains("1 findings"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir.path())
        .args(["check", "--help"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("1 findings")
    );
}

#[test]
fn discovery_rejects_missing_input() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        kanso::discovery::discover(&[dir.path().join("absent")], &kanso::Config::default())
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn discovery_records_non_not_found_input_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("plain");
    fs::write(&file, "x").unwrap();
    let config = kanso::Config::default();
    let other_error = file.join("child");
    assert_eq!(
        fs::symlink_metadata(&other_error)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ENOTDIR)
    );
    let report = kanso::engine::check(std::slice::from_ref(&other_error), &config);
    // Discovery records a non-NotFound input error; project-root selection can still fail.
    assert!(report.is_err());
    assert_eq!(
        kanso::discovery::discover(&[other_error], &config).unwrap(),
        Vec::<std::path::PathBuf>::new()
    );
}

// Windows maps ERROR_PATH_NOT_FOUND to NotFound, unlike Unix ENOTDIR.
#[cfg(windows)]
#[test]
fn discovery_treats_child_of_file_as_not_found_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("plain");
    fs::write(&file, "x").unwrap();
    let error =
        kanso::discovery::discover(&[file.join("child")], &kanso::Config::default()).unwrap_err();
    assert!(
        matches!(error, kanso::KansoError::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
    );
}

#[test]
fn discovery_excludes_unsupported_and_config_ignored_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Present\n").unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "# Ignored\n").unwrap();
    fs::write(dir.path().join("notes.md"), "# Unsupported\n").unwrap();
    let config = kanso::Config {
        ignore: vec!["CLAUDE.md".into()],
        ..kanso::Config::default()
    };
    let found = kanso::discovery::discover(&[dir.path().to_path_buf()], &config).unwrap();
    assert_eq!(found, vec![dir.path().join("AGENTS.md")]);
}

#[test]
fn project_root_missing_input_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(kanso::engine::project_root(&[dir.path().join("missing")]).is_err());
}

#[test]
fn m3_diagnostics_distinguish_size_binary_and_invalid_utf8() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "max_file_bytes = 32\n").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "x".repeat(33)).unwrap();
    fs::write(dir.path().join("CLAUDE.md"), b"a\0b").unwrap();
    fs::write(dir.path().join("GEMINI.md"), [0xff]).unwrap();
    let report = kanso::engine::check(
        &[dir.path().to_path_buf()],
        &kanso::Config {
            max_file_bytes: 32,
            ..kanso::Config::default()
        },
    )
    .unwrap();
    for (name, reason) in [
        ("AGENTS.md", "size limit"),
        ("CLAUDE.md", "binary"),
        ("GEMINI.md", "UTF-8"),
    ] {
        assert!(
            report.diagnostics.iter().any(|d| d.rule_id == "KS000"
                && d.path.ends_with(name)
                && d.message.contains(reason)),
            "missing {name}: {reason}"
        );
    }
}

#[cfg(unix)]
#[test]
fn m3_unreadable_file_reports_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    fs::write(&path, "# Title\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&path).is_ok() {
        eprintln!("SKIP: current user can read mode-000 files");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        return;
    }
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(report.diagnostics.iter().any(|d| d.rule_id == "KS000"
        && d.path == path
        && d.message.contains("permission denied")));
}

#[test]
fn combined_root_diagnostics_only_affect_their_own_file_score() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("AGENTS.md");
    let second = dir.path().join("CLAUDE.md");
    let text = "# Title\n\n- Always run all project tests\n";
    fs::write(&first, text).unwrap();
    fs::write(&second, text).unwrap();
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    let duplicate = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "KS005")
        .collect::<Vec<_>>();
    assert_eq!(duplicate.len(), 1);
    assert_eq!(duplicate[0].path, second);
    for file in &report.files {
        let local: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.path == file.path)
            .cloned()
            .collect();
        assert_eq!(
            file.score,
            kanso::score::score(&local),
            "{}",
            file.path.display()
        );
    }
    let first_score = report.files.iter().find(|f| f.path == first).unwrap().score;
    let second_score = report
        .files
        .iter()
        .find(|f| f.path == second)
        .unwrap()
        .score;
    assert_eq!(first_score, 100);
    assert_eq!(second_score, 95);
}
