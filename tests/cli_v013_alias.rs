#[cfg(unix)]
use std::fs;
use std::process::Command;

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_kanso"))
        .env_clear()
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn alias_explanation_is_available_in_text_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["explain", "alias"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("symbolic link"), "{text}");
    let out = run(dir.path(), &["--json", "explain", "alias"]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["rule"], "alias");
    assert!(
        json["explanation"]
            .as_str()
            .unwrap()
            .contains("symbolic link")
    );
}

#[cfg(unix)]
#[test]
fn alias_is_in_json_but_not_sarif_without_affecting_score_or_exit() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("kanso.toml"), "").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "# Instructions\n\nContent.\n").unwrap();
    symlink("AGENTS.md", dir.path().join("CLAUDE.md")).unwrap();
    let out = run(dir.path(), &["check", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["score"], 100);
    assert!(
        json["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["rule_id"] == "alias")
    );
    let out = run(dir.path(), &["check", "--format", "sarif"]);
    assert_eq!(out.status.code(), Some(0));
    let sarif: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["ruleId"] != "alias")
    );
    assert!(
        sarif["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["id"] == "alias")
    );
    fs::write(
        dir.path().join("AGENTS.md"),
        "# Instructions\n\n[missing](missing.md)\n",
    )
    .unwrap();
    let out = run(dir.path(), &["check", "--format", "sarif"]);
    assert_eq!(out.status.code(), Some(1));
    let sarif: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["ruleId"] == "KS004")
    );
}
