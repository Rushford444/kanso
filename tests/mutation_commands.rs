use kanso::{Config, Document, rules};
use std::path::Path;

fn ks009_count(root: &Path, command: &str) -> usize {
    let document = Document {
        path: root.join("AGENTS.md"),
        source: format!("# Commands\n\n```sh\n{command}\n```\n"),
    };
    let config = Config {
        disabled_rules: (1..=8).map(|n| format!("KS{n:03}")).collect(),
        ..Config::default()
    };
    rules::analyze(&document, &config, root)
        .expect("analyze command")
        .iter()
        .filter(|diagnostic| diagnostic.rule_id == "KS009")
        .count()
}

#[test]
fn just_recipe_parameters_are_not_targets() {
    let root = tempfile::tempdir().expect("temporary project");
    std::fs::write(root.path().join("justfile"), "build arg:\n").expect("write justfile");
    assert_eq!(ks009_count(root.path(), "just build"), 0);
    assert_eq!(ks009_count(root.path(), "just arg"), 1);
}

#[test]
fn direct_package_scripts_distinguish_flags_missing_and_declared_names() {
    let root = tempfile::tempdir().expect("temporary project");
    std::fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"build":"echo ok"}}"#,
    )
    .expect("write package manifest");
    assert_eq!(ks009_count(root.path(), "pnpm --version"), 0);
    assert_eq!(ks009_count(root.path(), "pnpm missing"), 1);
    assert_eq!(ks009_count(root.path(), "pnpm build"), 0);
}

#[test]
fn tool_options_are_not_command_targets() {
    let root = tempfile::tempdir().expect("temporary project");
    for command in ["make --version", "just --version", "cargo --version"] {
        assert_eq!(ks009_count(root.path(), command), 0, "{command}");
    }
}

#[test]
fn assignment_names_must_be_nonempty_alphanumeric_or_underscore() {
    let root = tempfile::tempdir().expect("temporary project");
    for (command, count) in [
        ("=x cargo missing", 0),
        ("BAD-NAME=x cargo missing", 0),
        ("A_B=x cargo missing", 1),
    ] {
        assert_eq!(ks009_count(root.path(), command), count, "{command}");
    }
}
