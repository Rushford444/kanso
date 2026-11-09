#![cfg(unix)]
use std::{fs, os::unix::fs::symlink};

#[test]
fn regular_files_keep_input_spelling_and_deduplicate_by_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(real.join("nested")).unwrap();
    symlink(&real, tmp.path().join("alias")).unwrap();
    let input = tmp.path().join("alias/nested");
    fs::write(
        input.join("AGENTS.md"),
        "# Guide\n\n- Run tests now\n- Run tests now\n\n[missing](missing.md)\n",
    )
    .unwrap();
    let expected = input.join("AGENTS.md");
    let config = kanso::Config::default();
    for paths in [
        vec![input.clone()],
        vec![expected.clone()],
        vec![input.clone(), real.join("nested")],
    ] {
        assert_eq!(
            kanso::discovery::discover(&paths, &config).unwrap(),
            vec![expected.clone()]
        );
        let report = kanso::engine::check(&paths, &config).unwrap();
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].path, expected);
        for id in ["KS004", "KS005"] {
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|d| d.rule_id == id && d.path == expected),
                "{id}: {:?}",
                report.diagnostics
            );
        }
    }
}

#[test]
fn cli_formats_keep_the_alias_path() {
    use std::process::Command;
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(real.join("nested")).unwrap();
    symlink(&real, tmp.path().join("alias")).unwrap();
    let input = tmp.path().join("alias/nested");
    fs::write(
        input.join("AGENTS.md"),
        "# Guide\n\n[missing](missing.md)\n",
    )
    .unwrap();
    let expected = input.join("AGENTS.md").to_string_lossy().into_owned();
    for flags in [vec![], vec!["--json"], vec!["--format", "sarif"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .arg("check")
            .arg(&input)
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains(&expected), "{text}");
        assert!(
            !text.contains(&real.canonicalize().unwrap().to_string_lossy().to_string()),
            "{text}"
        );
    }
}

#[test]
fn aliased_root_still_runs_combined_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    fs::create_dir_all(real.join("nested")).unwrap();
    symlink(&real, tmp.path().join("alias")).unwrap();
    let input = tmp.path().join("alias/nested");
    fs::write(input.join("kanso.toml"), "").unwrap();
    fs::write(input.join("AGENTS.md"), "# A\n\n- Run all the tests\n").unwrap();
    fs::write(input.join("CLAUDE.md"), "# B\n\n- Run all the tests\n").unwrap();
    let report =
        kanso::engine::check(std::slice::from_ref(&input), &kanso::Config::default()).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.rule_id == "KS005" && d.path == input.join("CLAUDE.md")),
        "{:?}",
        report.diagnostics
    );
}
