#[cfg(unix)]
mod unix {
    use serde_json::Value;
    use std::{fs, os::unix::fs::symlink, process::Command};
    fn run(root: &std::path::Path, args: &[&str]) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        serde_json::from_slice(&out.stdout).unwrap()
    }
    #[test]
    fn preview_symlink_and_directory_orders() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        fs::write(p.join("kanso.toml"), "").unwrap();
        fs::create_dir(p.join("plain")).unwrap();
        fs::write(
            p.join("plain/AGENTS.md"),
            "# Plain\n\n- repeat me\n- repeat me\n",
        )
        .unwrap();
        symlink("AGENTS.md", p.join("plain/CLAUDE.md")).unwrap();
        for (paths, expected) in [
            (vec!["plain/CLAUDE.md"], "plain/CLAUDE.md"),
            (
                vec!["plain/CLAUDE.md", "plain/AGENTS.md"],
                "plain/CLAUDE.md",
            ),
            (
                vec!["plain/AGENTS.md", "plain/CLAUDE.md"],
                "plain/AGENTS.md",
            ),
            (vec!["plain/CLAUDE.md", "plain"], "plain/CLAUDE.md"),
            (vec!["plain", "plain/CLAUDE.md"], "plain/AGENTS.md"),
        ] {
            let mut args = vec!["fix", "--dry-run", "--json"];
            args.extend(paths);
            let v = run(p, &args);
            assert_eq!(v["changed_files"], 1, "{v}");
            assert_eq!(v["files"][0], expected);
        }
        let out = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(p)
            .args(["fix", "--write", "plain/CLAUDE.md"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(3));
        assert!(
            fs::symlink_metadata(p.join("plain/CLAUDE.md"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let out = Command::new(env!("CARGO_BIN_EXE_kanso"))
            .current_dir(p)
            .args(["fix", "--write", "plain"])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(
            fs::symlink_metadata(p.join("plain/CLAUDE.md"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(p.join("plain/AGENTS.md")).unwrap(),
            "# Plain\n\n- repeat me\n"
        );
    }
    #[test]
    fn preview_directory_spelling_is_stable_whatever_creation_order() {
        for link_first in [true, false] {
            let t = tempfile::tempdir().unwrap();
            let p = t.path();
            fs::write(p.join("kanso.toml"), "").unwrap();
            fs::create_dir(p.join("plain")).unwrap();
            if link_first {
                symlink("AGENTS.md", p.join("plain/CLAUDE.md")).unwrap();
            }
            fs::write(
                p.join("plain/AGENTS.md"),
                "# Plain\n\n- repeat me\n- repeat me\n",
            )
            .unwrap();
            if !link_first {
                symlink("AGENTS.md", p.join("plain/CLAUDE.md")).unwrap();
            }
            for _ in 0..5 {
                let v = run(p, &["fix", "--dry-run", "--json", "plain"]);
                assert_eq!(v["changed_files"], 1, "{v}");
                assert_eq!(v["files"][0], "plain/AGENTS.md", "{v}");
            }
        }
    }
    #[test]
    fn outside_directory_target_has_visible_alias_not_secret() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
        fs::write(p.join("kanso.toml"), "").unwrap();
        fs::create_dir_all(p.join(format!("secret/{secret}"))).unwrap();
        fs::create_dir(p.join("safe")).unwrap();
        fs::write(
            p.join(format!("secret/{secret}/AGENTS.md")),
            "# Root\n\n[missing](missing.md)\n\n- repeat\n- repeat\n",
        )
        .unwrap();
        fs::write(
            p.join("safe/SKILL.md"),
            "---\nname: fine\ndescription: Fine\n---\n# Fine\n",
        )
        .unwrap();
        symlink(
            format!("../secret/{secret}/AGENTS.md"),
            p.join("safe/CLAUDE.md"),
        )
        .unwrap();
        for args in [
            vec!["check", "safe", "--json"],
            vec!["check", "safe/CLAUDE.md", "safe", "--json"],
            vec!["check", "safe", "safe/CLAUDE.md", "--json"],
        ] {
            let v = run(p, &args);
            let text = v.to_string();
            assert!(!text.contains(secret), "{v}");
            assert!(
                v["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["path"] == "safe/CLAUDE.md"),
                "{v}"
            );
            assert!(
                !v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["rule_id"] == "KS007"),
                "{v}"
            );
        }
        let v = run(p, &["fix", "--dry-run", "--json", "safe"]);
        assert_eq!(v["files"][0], "safe/CLAUDE.md");
    }
    #[test]
    fn excluded_directories_and_distinct_alias_parents() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        fs::write(p.join("kanso.toml"), "").unwrap();
        fs::create_dir(p.join("ok")).unwrap();
        fs::write(p.join("ok/AGENTS.md"), "# OK\n").unwrap();
        for dir in [".git", "target", "node_modules", "vendor", ".venv"] {
            fs::create_dir(p.join(dir)).unwrap();
            fs::write(p.join(dir).join("AGENTS.md"), "# excluded\n").unwrap();
        }
        let files =
            kanso::discovery::discover(&[p.to_path_buf()], &kanso::Config::default()).unwrap();
        assert_eq!(files, vec![p.join("ok/AGENTS.md")]);
        for dir in ["first", "second"] {
            fs::create_dir(p.join(dir)).unwrap();
            symlink("../ok/AGENTS.md", p.join(dir).join("CLAUDE.md")).unwrap();
        }
        let report = kanso::engine::check(
            &[p.join("first/CLAUDE.md"), p.join("second/CLAUDE.md")],
            &kanso::Config::default(),
        )
        .unwrap();
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.rule_id == "alias")
                .count(),
            2
        );
        for dir in ["first", "second"] {
            fs::remove_file(p.join(dir).join("CLAUDE.md")).unwrap();
            symlink("missing", p.join(dir).join("CLAUDE.md")).unwrap();
        }
        let report = kanso::engine::check(
            &[p.join("first/CLAUDE.md"), p.join("second/CLAUDE.md")],
            &kanso::Config::default(),
        )
        .unwrap();
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.rule_id == "KS000")
                .count(),
            2
        );
    }
    #[test]
    fn lexical_ancestor_alias_secret_stays_with_its_file() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("real");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("kanso.toml"), "").unwrap();
        let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
        fs::create_dir(root.join(secret)).unwrap();
        fs::create_dir(root.join("safe")).unwrap();
        fs::write(root.join(secret).join("AGENTS.md"), "# Secret\n\n- rule\n").unwrap();
        fs::write(root.join("safe/AGENTS.md"), "# Safe\n\n- rule\n").unwrap();
        let alias = t.path().join("alias");
        symlink(&root, &alias).unwrap();
        let secret_path = alias.join(secret).join("AGENTS.md");
        let safe_path = alias.join("safe/AGENTS.md");
        let report = kanso::engine::check(
            &[secret_path.clone(), safe_path.clone()],
            &kanso::Config::default(),
        )
        .unwrap();
        assert_eq!(report.files.len(), 2);
        assert_eq!(
            report
                .files
                .iter()
                .find(|f| f.path == safe_path)
                .unwrap()
                .score,
            100
        );
        assert_eq!(
            report
                .files
                .iter()
                .find(|f| f.path == secret_path)
                .unwrap()
                .score,
            95
        );
        for file in &report.files {
            let local: Vec<_> = report
                .diagnostics
                .iter()
                .filter(|d| d.path == file.path)
                .cloned()
                .collect();
            assert_eq!(
                file.score,
                kanso::score::score(&local),
                "{file:?}: {:?}",
                report.diagnostics
            );
        }
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.rule_id == "KS007" && d.path == secret_path),
            "{:?}",
            report.diagnostics
        );
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.rule_id == "KS007" && d.path == safe_path)
        );
    }
    #[test]
    fn visible_secret_is_scored_on_its_own_file() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path();
        fs::write(p.join("kanso.toml"), "").unwrap();
        fs::create_dir(p.join("safe")).unwrap();
        let secret = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
        fs::create_dir(p.join(secret)).unwrap();
        fs::write(p.join("safe/AGENTS.md"), "# Safe\n\n- rule\n").unwrap();
        fs::write(p.join(secret).join("AGENTS.md"), "# Secret\n\n- other\n").unwrap();
        symlink("AGENTS.md", p.join(secret).join("CLAUDE.md")).unwrap();
        let paths = [p.join("safe/AGENTS.md"), p.join(secret).join("CLAUDE.md")];
        let r = kanso::engine::check(&paths, &kanso::Config::default()).unwrap();
        for f in &r.files {
            let local: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| d.path == f.path)
                .cloned()
                .collect();
            assert_eq!(f.score, kanso::score::score(&local), "{:?}", r.diagnostics);
        }
        assert_eq!(r.score, kanso::score::score(&r.diagnostics));
        assert_eq!(
            r.diagnostics
                .iter()
                .filter(|d| d.rule_id == "KS007")
                .count(),
            1
        );
    }
}
