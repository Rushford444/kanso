use kanso::{Config, Document, rules};

#[test]
fn absolute_links_are_not_checked_as_relative_files() {
    let root = tempfile::tempdir().unwrap();
    let doc = Document {
        path: root.path().join("AGENTS.md"),
        source: "# Guide\n\n[absolute](/kanso-nonexistent-document.md)\n".into(),
    };
    let diagnostics = rules::analyze(&doc, &Config::default(), root.path()).unwrap();
    assert!(!diagnostics.iter().any(|d| d.rule_id == "KS004"));
}

#[test]
fn invalid_uri_scheme_does_not_hide_a_missing_relative_file() {
    let root = tempfile::tempdir().unwrap();
    let doc = Document {
        path: root.path().join("AGENTS.md"),
        source: "# Guide\n\n[missing](abc!:missing-file.md)\n".into(),
    };
    let diagnostics = rules::analyze(&doc, &Config::default(), root.path()).unwrap();
    assert!(diagnostics.iter().any(|d| d.rule_id == "KS004"));
}
