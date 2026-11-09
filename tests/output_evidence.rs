use std::{path::PathBuf, process::Command};

fn run(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ks008-bad"))
        .args(arguments)
        .output()
        .expect("run kanso");
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

#[test]
fn snapshot_human_json_and_sarif_and_validate_official_schema() {
    let text = run(&["check", "AGENTS.md"]);
    let json = run(&["check", "AGENTS.md", "--json"]);
    let sarif = run(&["check", "AGENTS.md", "--format", "sarif"]);
    let normalize = |s: &str| {
        let root = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
        let uri_root = root.replace(':', "%3A");
        s.replace("\\\\", "/")
            .replace('\\', "/")
            .replace(&uri_root, "PROJECT")
            .replace(&root, "PROJECT")
    };
    insta::assert_snapshot!("human", normalize(&text));
    insta::assert_json_snapshot!(
        "json",
        serde_json::from_str::<serde_json::Value>(&normalize(&json)).unwrap()
    );
    let value: serde_json::Value = serde_json::from_str(&sarif).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("schema/sarif-schema-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "SARIF schema validation: {errors:?}");
    insta::assert_json_snapshot!(
        "sarif",
        serde_json::from_str::<serde_json::Value>(&normalize(&sarif)).unwrap()
    );
}
