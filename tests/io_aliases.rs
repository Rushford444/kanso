use std::fs;
#[cfg(unix)]
use std::path::Path;

#[test]
fn utf16_bom_is_decoded_and_bomless_is_diagnosed() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bom, le) in [("le", [0xff, 0xfe], true), ("be", [0xfe, 0xff], false)] {
        let path = dir.path().join(name);
        let mut bytes = bom.to_vec();
        for unit in "# Tést\n".encode_utf16() {
            bytes.extend(if le {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            kanso::discovery::read_regular(&path, 100).unwrap(),
            "# Tést\n"
        );
    }
    let invalid = dir.path().join("invalid");
    fs::write(&invalid, [0xff, 0xfe, 0x00, 0xd8]).unwrap();
    assert!(kanso::discovery::read_regular(&invalid, 100).is_err());
    let path = dir.path().join("bare");
    fs::write(&path, b"#\0 \0T\0i\0t\0l\0e\0\n\0").unwrap();
    assert!(
        kanso::discovery::read_regular(&path, 100)
            .unwrap_err()
            .to_string()
            .contains("UTF-16")
    );
}

#[test]
fn bomless_utf16_detection_distinguishes_binary_zero_distribution() {
    use kanso::discovery::decode_regular_bytes;
    for bytes in [
        b"#\0T\0".as_slice(),
        b"\0#\0T".as_slice(),
        b"\0abc".as_slice(),
        b"a\0bc".as_slice(),
        b"#\0T\0i\0t\0l\0e\0".as_slice(),
        b"\0#\0T\0i\0t\0l\0e".as_slice(),
    ] {
        assert_eq!(
            decode_regular_bytes(bytes).unwrap_err().to_string(),
            "UTF-16/unsupported encoding"
        );
    }
    for bytes in [b"\0abcde".as_slice(), b"a\0bcde".as_slice()] {
        assert_eq!(
            decode_regular_bytes(bytes).unwrap_err().to_string(),
            "binary file"
        );
    }
}

#[cfg(unix)]
#[test]
fn aliases_are_read_once_and_bad_links_and_fifo_are_errors() {
    use std::{os::unix::fs::symlink, process::Command};
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(
        dir.path().join("AGENTS.md"),
        "# Title\n\n- Run all checks\n",
    )
    .unwrap();
    symlink("AGENTS.md", dir.path().join("CLAUDE.md")).unwrap();
    fs::write(dir.path().join("other.txt"), "# Other\n").unwrap();
    symlink("other.txt", dir.path().join("GEMINI.md")).unwrap();
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.score, kanso::score::score(&report.diagnostics));
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| d.rule_id == "alias" && d.severity == kanso::Severity::Info)
            .count(),
        2
    );
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| ["KS005", "KS006"].contains(&d.rule_id.as_str()))
    );
    let explicit_good =
        kanso::engine::check(&[dir.path().join("CLAUDE.md")], &kanso::Config::default()).unwrap();
    assert_eq!(explicit_good.files.len(), 1);
    assert!(
        explicit_good
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "alias" && d.severity == kanso::Severity::Info)
    );
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("outside"), "# Outside").unwrap();
    symlink(outside.path().join("outside"), dir.path().join("SKILL.md")).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(dir.path().join(".cursorrules"))
            .status()
            .unwrap()
            .success()
    );
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    let explicit =
        kanso::engine::check(&[dir.path().join("SKILL.md")], &kanso::Config::default()).unwrap();
    assert!(
        explicit
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "KS000" && d.severity == kanso::Severity::Error)
    );
    for name in ["SKILL.md", ".cursorrules"] {
        assert!(report.diagnostics.iter().any(|d| d.rule_id == "KS000"
            && d.severity == kanso::Severity::Error
            && d.path.ends_with(Path::new(name))));
    }
}

#[cfg(windows)]
#[test]
fn windows_file_symlink_is_reported_as_an_alias() {
    use std::os::windows::fs::symlink_file;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\n").unwrap();
    let link = dir.path().join("CLAUDE.md");
    match symlink_file(dir.path().join("AGENTS.md"), &link) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(1314) => {
            eprintln!("skipped: Windows symlink privilege not available");
            return;
        }
        Err(error) => panic!("could not create Windows symlink: {error}"),
    }
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "alias" && d.path.ends_with("CLAUDE.md"))
    );
}

#[cfg(unix)]
#[test]
fn discovered_fifo_is_a_not_regular_file_error() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("AGENTS.md");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let report =
        kanso::engine::check(&[dir.path().to_path_buf()], &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == fifo && d.rule_id == "KS000" && d.message == "not a regular file")
    );
}

#[cfg(unix)]
#[test]
fn directory_ancestor_alias_preserves_special_file_diagnostic_paths() {
    use std::{
        os::unix::{fs::symlink, net::UnixListener},
        process::Command,
    };
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias = dir.path().join("alias");
    symlink(&real, &alias).unwrap();
    let input = alias.join("nested");
    fs::create_dir(&input).unwrap();
    let fifo = input.join("AGENTS.md");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let socket = input.join("CLAUDE.md");
    let _listener = UnixListener::bind(&socket).unwrap();
    let supported_dir = input.join("GEMINI.md");
    fs::create_dir(&supported_dir).unwrap();
    let report = kanso::engine::check(&[input], &kanso::Config::default()).unwrap();
    assert!(report.diagnostics.iter().any(|d| d.path == supported_dir
        && d.rule_id == "KS000"
        && d.message == "cannot inspect or traverse path"));
    for path in [&fifo, &socket] {
        assert!(
            report.diagnostics.iter().any(|d| d.path == *path
                && d.rule_id == "KS000"
                && d.message == "not a regular file"),
            "{path:?}: {:?}",
            report.diagnostics
        );
    }
    let device = std::path::PathBuf::from("/dev/null");
    let report =
        kanso::engine::check(std::slice::from_ref(&device), &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == device && d.rule_id == "KS000" && d.message == "not a regular file")
    );
}

#[cfg(unix)]
#[test]
fn equivalent_directory_inputs_deduplicate_discovery_failures() {
    use std::{
        os::unix::{fs::symlink, net::UnixListener},
        process::Command,
    };
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias_root = dir.path().join("alias");
    symlink(&real, &alias_root).unwrap();
    let real = real.join("nested");
    fs::create_dir(&real).unwrap();
    let alias = alias_root.join("nested");
    assert!(
        Command::new("mkfifo")
            .arg(real.join("AGENTS.md"))
            .status()
            .unwrap()
            .success()
    );
    for (first, second) in [(&alias, &real), (&real, &alias)] {
        let report =
            kanso::engine::check(&[first.clone(), second.clone()], &kanso::Config::default())
                .unwrap();
        let failures: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.rule_id == "KS000")
            .collect();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].path, first.join("AGENTS.md"));
        assert_eq!(report.score, 85);
    }
    let _socket = UnixListener::bind(real.join("CLAUDE.md")).unwrap();
    symlink("missing", real.join("GEMINI.md")).unwrap();
    for (first, second) in [(&alias, &real), (&real, &alias)] {
        let report =
            kanso::engine::check(&[first.clone(), second.clone()], &kanso::Config::default())
                .unwrap();
        let failures: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.rule_id == "KS000")
            .collect();
        assert_eq!(failures.len(), 3, "{failures:?}");
        for name in ["AGENTS.md", "CLAUDE.md", "GEMINI.md"] {
            assert!(failures.iter().any(|d| d.path == first.join(name)));
        }
        assert_eq!(report.score, 70);
    }
}

#[test]
fn supported_directory_input_preserves_exact_path() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("AGENTS.md");
    fs::create_dir(&input).unwrap();
    let report =
        kanso::engine::check(std::slice::from_ref(&input), &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "KS000" && d.path.as_os_str() == input.as_os_str())
    );
}

#[test]
fn cli_reports_bomless_utf16_as_encoding_error() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), b"#\0 \0T\0i\0t\0l\0e\0\n\0").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir.path())
        .args(["check", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "KS000" && d["message"] == "UTF-16/unsupported encoding")
    );
}
