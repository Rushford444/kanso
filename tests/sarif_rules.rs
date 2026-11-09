use std::{collections::HashSet, process::Command};

const AGENTS: &str = "\
# Workflow

### Deep section

See [missing](missing.md#install).

- Run unit tests before review.
- Run unit tests before review.

- Run all database migration tests before merging changes.
- Run all database migration tests before merging code.

Example token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890

```sh
cargo abracadabra-build --release
```
";

const SKILL: &str = "\
---
name: Invalid Name
description: \"\"
---
# Release review
Review release notes.
";

fn sarif() -> serde_json::Value {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("kanso.toml"), "").expect("config");
    std::fs::write(dir.path().join("AGENTS.md"), AGENTS).expect("agents");
    std::fs::write(dir.path().join("SKILL.md"), SKILL).expect("skill");
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(dir.path())
        .args(["check", ".", "--format", "sarif"])
        .output()
        .expect("run kanso");
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("SARIF JSON")
}

#[test]
fn sarif_driver_rules_describe_every_result() {
    let value = sarif();
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1);
    let rules = runs[0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules");
    assert!(!rules.is_empty(), "rules must not be empty");

    let mut ids = HashSet::new();
    for rule in rules {
        let id = rule["id"].as_str().expect("id");
        assert!(!id.is_empty(), "empty rule id");
        assert!(ids.insert(id), "duplicate rule id {id}");
        let name = rule["name"].as_str().expect("name");
        assert!(!name.is_empty(), "{id} has an empty name");
        assert!(
            name.bytes().all(|b| b.is_ascii_alphanumeric()),
            "{id} name {name} is not alphanumeric"
        );
        let short = rule["shortDescription"]["text"]
            .as_str()
            .expect("shortDescription.text");
        assert!(!short.is_empty(), "{id} has no short description");
        assert!(short.len() <= 120, "{id} short description is too long");
        let full = rule["fullDescription"]["text"]
            .as_str()
            .expect("fullDescription.text");
        assert!(!full.is_empty(), "{id} has no full description");
        assert!(
            full.len() > short.len(),
            "{id} full description is not longer than the short one"
        );
        let help_uri = rule["helpUri"].as_str().expect("helpUri");
        assert!(
            help_uri.starts_with("https://"),
            "{id} helpUri {help_uri} is not absolute"
        );
    }
    for id in [
        "alias", "KS000", "KS001", "KS002", "KS003", "KS004", "KS005", "KS006", "KS007", "KS008",
        "KS009",
    ] {
        assert!(ids.contains(id), "{id} is missing from rules");
    }

    let results = runs[0]["results"].as_array().expect("results");
    assert!(results.len() > 1, "results should exercise several rules");
    for result in results {
        let rule_id = result["ruleId"].as_str().expect("ruleId");
        assert!(ids.contains(rule_id), "{rule_id} is not described");
        let index = usize::try_from(result["ruleIndex"].as_u64().expect("ruleIndex"))
            .expect("ruleIndex fits usize");
        assert_eq!(rules[index]["id"], rule_id);
    }
}
