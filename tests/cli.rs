use std::{fs, process::Command};

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn version_and_help_use_binary_name() {
    let dir = tempfile::tempdir().unwrap();
    let version = run(dir.path(), &["--version"]);
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(version.starts_with(&format!("kanso {}", env!("CARGO_PKG_VERSION"))));
    assert!(!version.contains("kdw-kanso"));
    let help = run(dir.path(), &["--help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    let usage = help
        .lines()
        .find(|line| line.starts_with("Usage:"))
        .unwrap();
    assert!(usage.contains("kanso"));
    assert!(!usage.contains("kdw-kanso"));
}

#[test]
fn init_matches_configuration_in_both_readmes() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
    let content = String::from_utf8(fs::read(dir.path().join("kanso.toml")).unwrap())
        .unwrap()
        .replace("\r\n", "\n");
    for name in ["README.md", "README.fr.md"] {
        let readme =
            fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
                .unwrap()
                .replace("\r\n", "\n");
        let section = readme
            .split_once("## Configuration")
            .unwrap_or_else(|| panic!("{name}: missing Configuration heading"))
            .1;
        let block = section
            .split_once("```toml\n")
            .unwrap_or_else(|| panic!("{name}: missing TOML block"))
            .1;
        let block = block
            .split_once("```")
            .unwrap_or_else(|| panic!("{name}: unclosed TOML block"))
            .0;
        assert_eq!(content, block, "{name}: configuration differs from init");
    }
}

#[test]
fn fix_is_dry_run_then_safe_idempotent_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    let original = "# Title \n\n- repeat\n- repeat\n\n- parent\n  - child\n- parent\n  - different child\n\nHard break  \nnext\n\n```txt\nkeep  \n```\n";
    fs::write(&path, original).unwrap();
    let dry = run(dir.path(), &["fix", "--json"]);
    assert!(dry.status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let write = run(dir.path(), &["fix", "--write", "--json"]);
    assert!(write.status.success());
    let fixed = fs::read_to_string(&path).unwrap();
    assert!(fixed.contains("- repeat\n\n"));
    assert!(!fixed.contains("- repeat\n- repeat"));
    assert!(fixed.contains("- parent\n  - child\n- parent\n  - different child"));
    assert!(fixed.contains("Hard break  \nnext"));
    assert!(fixed.contains("```txt\nkeep  \n```"));
    assert!(run(dir.path(), &["fix", "--write"]).status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), fixed);
}

#[test]
fn json_errors_and_help_do_not_echo_secret_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "sk-test-abcdefghijklmnopqrstuv123456789";
    let output = run(dir.path(), &["check", "--json", "--unknown", secret]);
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stdout.contains(secret) && !stderr.contains(secret));
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["schema"], "kanso/v1");
}

#[test]
fn fix_preserves_lazy_and_nested_duplicate_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    let original = "# Title\n\n- same\n- same\ncontinuation\n\n- again\n- again\n\n  child\n";
    fs::write(&path, original).unwrap();
    assert!(run(dir.path(), &["fix", "--write"]).status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn sarif_and_init_are_machine_readable_and_init_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "plain text\n").unwrap();
    let output = run(dir.path(), &["check", "--format", "sarif"]);
    let sarif: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(sarif["runs"][0]["tool"]["driver"]["name"], "kanso");
    let first = run(dir.path(), &["init", "--json"]);
    let second = run(dir.path(), &["init", "--json"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&first.stdout).unwrap()["created"],
        true
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&second.stdout).unwrap()["created"],
        false
    );
}

#[test]
fn check_preserves_paths_and_redacts_messages() {
    let dir = tempfile::tempdir().unwrap();
    let path_secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let content_secret = "AKIAIOSFODNN7EXAMPLE";
    let path = dir.path().join(format!("{path_secret}.md"));
    fs::write(&path, format!("# Title\n\nkey {content_secret}\n")).unwrap();
    for args in [
        vec!["check", "--json", path.to_str().unwrap()],
        vec!["check", "--format", "sarif", path.to_str().unwrap()],
        vec!["check", path.to_str().unwrap()],
    ] {
        let output = run(dir.path(), &args);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(path_secret));
        assert!(!stdout.contains(content_secret));
    }
}
#[test]
fn fix_does_not_change_html_block_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("AGENTS.md");
    let original = "# Title\n\n<pre>\n- repeat\n- repeat\n\n</pre>\n";
    fs::write(&path, original).unwrap();
    assert!(run(dir.path(), &["fix", "--write"]).status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn explicit_dry_run_and_write_conflict() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Title\n\n- a \n").unwrap();
    assert!(run(dir.path(), &["fix", "--dry-run"]).status.success());
    assert_eq!(
        run(dir.path(), &["fix", "--dry-run", "--write"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn score_lists_files_and_badge_is_markdown() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Title\n\nText.\n").unwrap();
    let output = run(dir.path(), &["score", "--json"]);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["score"].is_number())
    );
    let human = run(dir.path(), &["score"]);
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("AGENTS.md")
    );
    let badge = run(dir.path(), &["score", "--badge"]);
    assert!(String::from_utf8(badge.stdout).unwrap().starts_with("!["));
}

#[test]
fn init_lists_all_configuration_keys() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
    let content = fs::read_to_string(dir.path().join("kanso.toml")).unwrap();
    for name in [
        "token_warn",
        "token_error",
        "total_token_warn",
        "total_token_error",
        "similarity_threshold",
        "max_file_bytes",
        "disabled_rules",
        "ignore",
    ] {
        assert!(content.contains(name));
    }
}

#[cfg(unix)]
#[test]
fn fix_refuses_explicit_symlink_write() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.md");
    let link = dir.path().join("AGENTS.md");
    fs::write(&target, "# Title\n\n- a \n").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(
        !run(dir.path(), &["fix", "--write", link.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "# Title\n\n- a \n");
}

#[test]
fn disabled_duplicate_rule_does_not_delete_lines() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Title\n\n- same\n- same\n").unwrap();
    fs::write(
        dir.path().join("kanso.toml"),
        "disabled_rules = [\"KS005\"]\n",
    )
    .unwrap();
    assert!(run(dir.path(), &["fix", "--write"]).status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(),
        "# Title\n\n- same\n- same\n"
    );
}

#[test]
fn project_config_is_loaded_and_custom_init_respected() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("project");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("AGENTS.md"), "# Title\n\n- same\n- same\n").unwrap();
    fs::write(sub.join("kanso.toml"), "disabled_rules = [\"KS005\"]\n").unwrap();
    let result = run(dir.path(), &["fix", "project", "--write"]);
    assert!(result.status.success());
    assert!(
        fs::read_to_string(sub.join("AGENTS.md"))
            .unwrap()
            .contains("- same\n- same")
    );
    assert!(
        run(dir.path(), &["init", "--config", "custom.toml"])
            .status
            .success()
    );
    assert!(dir.path().join("custom.toml").exists());
    assert!(!dir.path().join("kanso.toml").exists());
}

#[test]
fn fix_preview_identifies_files_and_config_errors_explain_cause() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Title\n\n- one \n").unwrap();
    let out = run(dir.path(), &["fix", "--dry-run", "--json"]);
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        val["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().contains("AGENTS.md"))
    );
    fs::write(
        dir.path().join("kanso.toml"),
        "token_warn = 6000\ntoken_error = 100\n",
    )
    .unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        val["error"]["message"]
            .as_str()
            .unwrap()
            .contains("threshold")
    );
}

#[test]
fn sarif_without_input_uses_os_current_dir_and_help_describes_options() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "text\n").unwrap();
    let out = run(dir.path(), &["check", "--format", "sarif"]);
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let json = run(dir.path(), &["check", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    let path = json["files"][0]["path"].as_str().unwrap();
    assert!(std::path::Path::new(path).is_absolute());
    assert!(path.ends_with("AGENTS.md"));
    let uri = val["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
        ["uri"]
        .as_str()
        .unwrap();
    assert!(uri.starts_with("file:///"), "{uri}");
    assert!(uri.ends_with("/AGENTS.md"), "{uri}");
    #[cfg(windows)]
    assert_eq!(
        uri.as_bytes()[9],
        b':',
        "Windows drive must be in the file URI"
    );
    let help = run(dir.path(), &["fix", "--help"]);
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("Examples:") && text.contains("Exit codes:") && text.contains("JSON"));
}
