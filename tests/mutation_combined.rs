use kanso::{config::Config, model::Document, rules};
use std::path::Path;

fn doc(path: &str, source: &str) -> Document {
    Document {
        path: path.into(),
        source: source.into(),
    }
}

fn combined(first: &str, second: &str, threshold: f64, disable_similarity: bool) -> Vec<String> {
    let mut config = Config {
        similarity_threshold: threshold,
        disabled_rules: vec!["KS003".into()],
        ..Config::default()
    };
    if disable_similarity {
        config.disabled_rules.push("KS006".into());
    }
    rules::analyze_combined(
        &[doc("a.md", first), doc("b.md", second)],
        &config,
        Path::new("."),
    )
    .unwrap()
    .into_iter()
    .filter(|d| d.rule_id == "KS006")
    .map(|d| format!("{}:{}:{}:{}", d.path.display(), d.line, d.column, d.message))
    .collect()
}

#[test]
fn first_level_two_heading_is_not_a_skipped_level() {
    let diagnostics = rules::analyze(
        &doc("AGENTS.md", "## Child\ncontent\n"),
        &Config::default(),
        Path::new("."),
    )
    .unwrap();
    let structure: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.rule_id == "KS008")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(structure, ["document has no level-one title"]);
}

#[test]
fn similarity_requires_enabled_rule_and_a_prior_match() {
    assert_eq!(
        combined("- alpha beta gamma\n", "", 0.0, false),
        Vec::<String>::new()
    );
    assert_eq!(
        combined("- alpha beta gamma\n", "- delta epsilon zeta\n", 0.0, true),
        Vec::<String>::new()
    );
}

#[test]
fn similarity_requires_three_words_in_both_items_and_threshold_match() {
    assert_eq!(
        combined("- alpha beta\n", "- gamma delta epsilon\n", 0.0, false),
        Vec::<String>::new()
    );
    assert_eq!(
        combined("- alpha beta gamma\n", "- delta epsilon\n", 0.0, false),
        Vec::<String>::new()
    );
    assert_eq!(
        combined("- alpha beta gamma\n", "- delta epsilon zeta\n", 1.0, false),
        Vec::<String>::new()
    );
    assert_eq!(
        combined("- alpha beta gamma\n", "- delta epsilon zeta\n", 0.0, false),
        ["b.md:1:1:similar list instruction in root context"]
    );
}
