use std::fs;

#[test]
fn regular_inputs_keep_first_spelling() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("dir");
    fs::create_dir(&dir).unwrap();
    let file = dir.join("AGENTS.md");
    fs::write(&file, "# Guide\n").unwrap();
    for paths in [
        vec![file.clone()],
        vec![dir.clone()],
        vec![dir.clone(), file.clone()],
    ] {
        assert_eq!(
            kanso::discovery::discover(&paths, &kanso::Config::default()).unwrap(),
            vec![file.clone()]
        );
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::{os::unix::fs::symlink, path::PathBuf, process::Command};
    fn check(paths: &[PathBuf]) -> kanso::Report {
        kanso::engine::check(paths, &kanso::Config::default()).unwrap()
    }
    #[test]
    fn broken_input_does_not_change_other_spelling() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("kanso.toml"), "").unwrap();
        fs::create_dir(t.path().join("docs")).unwrap();
        fs::write(t.path().join("AGENTS.md"), "# A\n").unwrap();
        fs::write(t.path().join("docs/AGENTS.md"), "# D\n").unwrap();
        let broken = t.path().join("BROKEN");
        symlink("missing", &broken).unwrap();
        for good in [t.path().join("AGENTS.md"), t.path().join("docs")] {
            let expected = if good.is_dir() {
                good.join("AGENTS.md")
            } else {
                good.clone()
            };
            for paths in [
                vec![broken.clone(), good.clone()],
                vec![good.clone(), broken.clone()],
            ] {
                let r = check(&paths);
                assert_eq!(r.files[0].path, expected);
                assert_eq!(
                    r.diagnostics
                        .iter()
                        .filter(|d| d.rule_id == "KS000")
                        .count(),
                    1
                );
            }
        }
    }
    #[test]
    fn broken_relative_input_never_contaminates_other_inputs() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("kanso.toml"), "").unwrap();
        fs::create_dir(t.path().join("docs")).unwrap();
        fs::write(t.path().join("AGENTS.md"), "# A\n").unwrap();
        fs::write(t.path().join("docs/AGENTS.md"), "# D\n").unwrap();
        symlink("missing", t.path().join("BROKEN")).unwrap();
        for (good, expected) in [("AGENTS.md", "AGENTS.md"), ("docs", "docs/AGENTS.md")] {
            for paths in [["BROKEN", good], [good, "BROKEN"]] {
                let output = Command::new(env!("CARGO_BIN_EXE_kanso"))
                    .current_dir(t.path())
                    .arg("check")
                    .args(paths)
                    .arg("--json")
                    .output()
                    .unwrap();
                let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(json["files"][0]["path"], expected);
            }
        }
    }
    #[test]
    fn terminal_symlink_uses_input_but_resolves_links_at_target() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join("sub")).unwrap();
        fs::write(t.path().join("kanso.toml"), "").unwrap();
        fs::write(t.path().join("valid.md"), "ok").unwrap();
        fs::write(
            t.path().join("package.json"),
            r#"{"scripts":{"lint":"true"}}"#,
        )
        .unwrap();
        fs::write(t.path().join("sub/package.json"), r#"{"scripts":{}}"#).unwrap();
        fs::write(
            t.path().join("AGENTS.md"),
            "# Guide\n\n[valid](valid.md)\n[missing](missing.md)\n\n```sh\nnpm run lint\n```\n",
        )
        .unwrap();
        symlink("AGENTS.md", t.path().join("CLAUDE.md")).unwrap();
        symlink("../AGENTS.md", t.path().join("sub/CLAUDE.md")).unwrap();
        for name in ["CLAUDE.md", "sub/CLAUDE.md"] {
            let input = t.path().join(name);
            let r = check(std::slice::from_ref(&input));
            assert_eq!(r.files[0].path, input);
            let links: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| d.rule_id == "KS004")
                .collect();
            assert_eq!(links.len(), 1, "{links:?}");
            assert_eq!(links[0].path, input);
            assert!(
                !r.diagnostics.iter().any(|d| d.rule_id == "KS009"),
                "{:?}",
                r.diagnostics
            );
            assert!(
                r.diagnostics.iter().all(|d| d.path == input),
                "{:?}",
                r.diagnostics
            );
            for flags in [
                vec!["check"],
                vec!["check", "--json"],
                vec!["check", "--format", "sarif"],
                vec!["score"],
            ] {
                let o = Command::new(env!("CARGO_BIN_EXE_kanso"))
                    .args(flags)
                    .arg(&input)
                    .output()
                    .unwrap();
                let text = String::from_utf8(o.stdout).unwrap();
                assert!(text.contains(input.to_str().unwrap()), "{text}");
                assert!(
                    !text.contains(
                        t.path()
                            .join("AGENTS.md")
                            .canonicalize()
                            .unwrap()
                            .to_str()
                            .unwrap()
                    ),
                    "{text}"
                );
            }
        }
    }
    #[test]
    fn explicit_fifo_and_directory_are_one_failure() {
        let t = tempfile::tempdir().unwrap();
        let real = t.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(t.path().join("kanso.toml"), "").unwrap();
        let fifo = real.join("AGENTS.md");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        symlink(&real, t.path().join("alias")).unwrap();
        let alias = t.path().join("alias/AGENTS.md");
        for broken in [false, true] {
            if broken {
                fs::remove_file(&fifo).unwrap();
                symlink("missing", &fifo).unwrap();
            }
            for (paths, expected) in [
                (vec![alias.clone(), real.clone()], alias.clone()),
                (vec![real.clone(), alias.clone()], fifo.clone()),
            ] {
                let r = check(&paths);
                let failures: Vec<_> = r
                    .diagnostics
                    .iter()
                    .filter(|d| d.rule_id == "KS000")
                    .collect();
                assert_eq!(failures.len(), 1, "{failures:?}");
                assert_eq!(failures[0].path, expected);
                assert_eq!(r.score, 85);
            }
        }
    }
    #[test]
    fn explicit_alias_and_walk_report_alias_once() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("kanso.toml"), "").unwrap();
        fs::write(t.path().join("AGENTS.md"), "# Guide\n").unwrap();
        let alias = t.path().join("CLAUDE.md");
        symlink("AGENTS.md", &alias).unwrap();
        let parent_alias = t.path().join("parent-alias");
        symlink(t.path(), &parent_alias).unwrap();
        for paths in [
            vec![parent_alias.join("CLAUDE.md"), t.path().to_path_buf()],
            vec![t.path().to_path_buf(), parent_alias.join("CLAUDE.md")],
        ] {
            let r = check(&paths);
            assert_eq!(
                r.diagnostics
                    .iter()
                    .filter(|d| d.rule_id == "alias")
                    .count(),
                1
            );
        }
    }
}
