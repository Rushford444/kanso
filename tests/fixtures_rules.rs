use kanso::{Config, Document, rules};
use std::path::PathBuf;

fn document(group: &str, name: &str) -> (PathBuf, Document) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(group);
    let path = root.join(name);
    let source = std::fs::read_to_string(&path).expect("read fixture");
    (root, Document { path, source })
}

#[test]
fn individual_rule_fixtures_have_positive_and_negative_examples() {
    let cases = [
        ("KS001", "ks001-bad", "SKILL.md", "ks001-good", "SKILL.md"),
        ("KS002", "ks002-bad", "AGENTS.md", "ks002-good", "AGENTS.md"),
        ("KS004", "ks004-bad", "AGENTS.md", "ks004-good", "AGENTS.md"),
        ("KS005", "ks005-bad", "AGENTS.md", "ks005-good", "AGENTS.md"),
        ("KS006", "ks006-bad", "AGENTS.md", "ks006-good", "AGENTS.md"),
        ("KS007", "ks007-bad", "AGENTS.md", "ks007-good", "AGENTS.md"),
        ("KS008", "ks008-bad", "AGENTS.md", "ks008-good", "AGENTS.md"),
        ("KS009", "ks009-bad", "AGENTS.md", "ks009-good", "AGENTS.md"),
    ];
    for (rule, positive, positive_name, negative, negative_name) in cases {
        for (group, name, expected) in [
            (positive, positive_name, true),
            (negative, negative_name, false),
        ] {
            let (root, doc) = document(group, name);
            let diagnostics =
                rules::analyze(&doc, &Config::default(), &root).expect("analyze fixture");
            assert_eq!(
                diagnostics.iter().any(|d| d.rule_id == rule),
                expected,
                "{rule} in {group}: {diagnostics:?}"
            );
        }
    }
}

#[test]
fn combined_root_budget_has_positive_and_negative_fixtures() {
    for (group, expected) in [("ks003-bad", true), ("ks003-good", false)] {
        let (root, doc) = document(group, "AGENTS.md");
        let config = Config {
            total_token_warn: 20,
            total_token_error: 50,
            ..Config::default()
        };
        let diagnostics =
            rules::analyze_combined(&[doc], &config, &root).expect("analyze root fixture");
        assert_eq!(
            diagnostics.iter().any(|d| d.rule_id == "KS003"),
            expected,
            "{group}: {diagnostics:?}"
        );
    }
}

#[test]
fn realistic_fixture_corpus_is_present_and_secret_free() {
    for (group, name) in [
        ("realistic-agents", "AGENTS.md"),
        ("realistic-claude", "CLAUDE.md"),
        ("realistic-cursor", ".cursor/rules/project.mdc"),
        ("realistic-skill", "SKILL.md"),
        ("realistic-project", ".github/copilot-instructions.md"),
    ] {
        let (root, doc) = document(group, name);
        assert!((1024..=6144).contains(&doc.source.len()), "{group} size");
        let diagnostics = rules::analyze(&doc, &Config::default(), &root).expect("analyze fixture");
        assert!(
            !diagnostics.iter().any(|d| d.rule_id == "KS007"),
            "{group}: {diagnostics:?}"
        );
        if group == "realistic-skill" {
            assert!(
                !diagnostics.iter().any(|d| d.rule_id == "KS004"),
                "{diagnostics:?}"
            );
            assert!(root.join("checklist.md").is_file());
        }
    }
}

#[test]
fn edge_fixtures_are_exercised_through_the_cli() {
    let cases = [
        ("ks001-missing", "KS001", true),
        ("ks002-edge", "KS002", false),
        ("ks003-edge", "KS003", false),
        ("ks004-anchor", "KS004", true),
        ("ks005-normalized", "KS005", true),
        // SimHash similarity is 52/64 = 0.8125, below the default 0.85.
        ("ks006-neighbor", "KS006", false),
        ("ks007-aws", "KS007", true),
        ("ks008-notitle", "KS008", true),
        ("ks009-custom", "KS009", false),
    ];
    for (group, rule, expected) in cases {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(group);
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(&dir)
            .args(["check", ".", "--json"])
            .output()
            .expect("run CLI");
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("parse output");
        let diagnostics = value["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(
            diagnostics.iter().any(|d| d["rule_id"] == rule),
            expected,
            "{group}: {diagnostics:?}"
        );
    }
}

#[test]
fn cursor_rule_is_discovered_by_cli() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/realistic-cursor");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(&dir)
        .args(["check", ".", "--json"])
        .output()
        .expect("run CLI");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("parse output");
    let files = value["files"].as_array().expect("files");
    assert!(
        files.iter().any(|file| file["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("project.mdc"))),
        "{files:?}"
    );
}
