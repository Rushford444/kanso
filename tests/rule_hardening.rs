use kanso::{Config, Document, rules};
use std::{fs, path::Path, process::Command};

#[cfg(unix)]
fn check_special(target: &Path) {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    if target == Path::new("fifo") {
        assert!(
            Command::new("mkfifo")
                .arg(dir.path().join("special"))
                .status()
                .unwrap()
                .success()
        );
    } else {
        symlink(target, dir.path().join("special")).unwrap();
    }
    fs::write(
        dir.path().join("AGENTS.md"),
        "# Title\n\n[x](special#heading)\n",
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .args(["check", "--json"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("special file caused a hang");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("KS004"));
}
#[cfg(unix)]
#[test]
fn fifo_anchor_never_blocks() {
    check_special(Path::new("fifo"));
}
#[cfg(unix)]
#[test]
fn device_anchor_never_reads_unbounded() {
    check_special(Path::new("/dev/zero"));
}

#[test]
fn nearest_package_builtins_and_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let sub = root.join("packages/web");
    fs::create_dir_all(&sub).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"workspaces":["packages/*"],"scripts":{"rootonly":"true"}}"#,
    )
    .unwrap();
    fs::write(
        sub.join("package.json"),
        r#"{"scripts":{"lint":"true","dev":"true"}}"#,
    )
    .unwrap();
    let doc = Document {path: sub.join("AGENTS.md"), source: "# T\n\n```sh\npnpm install\npnpm dev\nnpm run lint\nyarn add react\npnpm exec vitest\nnpm run missing\n```\n".into()};
    let found = rules::analyze(&doc, &Config::default(), root).unwrap();
    assert_eq!(
        found.iter().filter(|d| d.rule_id == "KS009").count(),
        1,
        "{found:?}"
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ks009-custom");
    let doc = Document {
        path: fixture.join("AGENTS.md"),
        source: fs::read_to_string(fixture.join("AGENTS.md")).unwrap(),
    };
    assert!(
        !rules::analyze(&doc, &Config::default(), &fixture)
            .unwrap()
            .iter()
            .any(|d| d.rule_id == "KS009")
    );
}

#[test]
fn oversized_anchor_is_not_parsed() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("large.md"), "# heading\n".repeat(200)).unwrap();
    let doc = Document {
        path: dir.path().join("AGENTS.md"),
        source: "# T\n\n[x](large.md#heading)\n".into(),
    };
    let config = Config {
        max_file_bytes: 100,
        ..Config::default()
    };
    assert!(
        rules::analyze(&doc, &config, dir.path())
            .unwrap()
            .iter()
            .any(|d| d.rule_id == "KS004")
    );
}

#[test]
fn workspace_layouts_find_nearest_package() {
    for workspace in [
        r#"{"workspaces":["packages/*"]}"#,
        r#"{"workspaces":{"packages":["packages/*"]}}"#,
        "{}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("package.json"), workspace).unwrap();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n",
        )
        .unwrap();
        let package = root.join("packages/web");
        let nested = package.join("docs");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"scripts":{"lint":"true"}}"#,
        )
        .unwrap();
        let doc = Document {
            path: nested.join("AGENTS.md"),
            source: "# T\n\n```sh\nnpm run lint\npnpm lint\nyarn lint\n```\n".into(),
        };
        assert!(
            !rules::analyze(&doc, &Config::default(), root)
                .unwrap()
                .iter()
                .any(|d| d.rule_id == "KS009")
        );
    }
}

#[test]
fn root_workspace_scripts_and_excluded_packages() {
    for pnpm in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let manifest = if pnpm {
            "{}"
        } else {
            r#"{"workspaces":["packages/*"]}"#
        };
        fs::write(root.join("package.json"), manifest).unwrap();
        if pnpm {
            fs::write(
                root.join("pnpm-workspace.yaml"),
                "packages:\n  - packages/*\n",
            )
            .unwrap();
        }
        for (path, name) in [("packages/web", "dev"), ("excluded", "secretcmd")] {
            let path = root.join(path);
            fs::create_dir_all(&path).unwrap();
            fs::write(
                path.join("package.json"),
                format!(r#"{{"scripts":{{"{name}":"true"}}}}"#),
            )
            .unwrap();
        }
        let doc = Document {
            path: root.join("AGENTS.md"),
            source: "# T\n\n```sh\npnpm dev\npnpm secretcmd\n```\n".into(),
        };
        let found = rules::analyze(&doc, &Config::default(), root).unwrap();
        assert_eq!(
            found
                .iter()
                .filter(|d| d.rule_id == "KS009")
                .map(|d| d.line)
                .collect::<Vec<_>>(),
            vec![5],
            "{found:?}"
        );
    }
}

#[test]
fn ks000_explains_unreadable_input() {
    let explanation = rules::explain("KS000").expect("KS000 must be explainable");
    assert!(explanation.contains("read"));
}

#[test]
fn cli_explains_ks000_and_rejects_unknown() {
    let known = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .args(["explain", "KS000"])
        .output()
        .unwrap();
    assert_eq!(known.status.code(), Some(0));
    assert_ne!(known.stdout, b"", "explanation must not be empty");
    let unknown = Command::new(env!("CARGO_BIN_EXE_kanso"))
        .args(["explain", "KS999"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
}

#[test]
fn workspace_excludes_dependency_directories_and_keeps_nested_package_scope() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("package.json"), r#"{"workspaces":["**"]}"#).unwrap();
    for (path, script) in [
        ("packages/web", "webcmd"),
        ("packages/api", "apicmd"),
        ("node_modules/bad", "dependencycmd"),
    ] {
        let path = root.join(path);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("package.json"),
            format!(r#"{{"scripts":{{"{script}":"true"}}}}"#),
        )
        .unwrap();
    }
    let root_doc = Document {
        path: root.join("AGENTS.md"),
        source: "# T\n\n```sh\npnpm dependencycmd\n```\n".into(),
    };
    assert_eq!(
        rules::analyze(&root_doc, &Config::default(), root)
            .unwrap()
            .iter()
            .filter(|d| d.rule_id == "KS009")
            .count(),
        1
    );
    let nested_doc = Document {
        path: root.join("packages/web/AGENTS.md"),
        source: "# T\n\n```sh\npnpm apicmd\n```\n".into(),
    };
    assert_eq!(
        rules::analyze(&nested_doc, &Config::default(), root)
            .unwrap()
            .iter()
            .filter(|d| d.rule_id == "KS009")
            .count(),
        1
    );
}

#[test]
fn workspace_empty_pattern_does_not_select_all_packages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("package.json"), r#"{"workspaces":[""]}"#).unwrap();
    let sub = root.join("unselected");
    fs::create_dir_all(&sub).unwrap();
    fs::write(
        sub.join("package.json"),
        r#"{"scripts":{"unexpected":"true"}}"#,
    )
    .unwrap();
    let doc = Document {
        path: root.join("AGENTS.md"),
        source: "# T\n\n```sh\npnpm unexpected\n```\n".into(),
    };
    assert_eq!(
        rules::analyze(&doc, &Config::default(), root)
            .unwrap()
            .iter()
            .filter(|d| d.rule_id == "KS009")
            .count(),
        1
    );
}

#[test]
fn workspace_parent_pattern_cannot_select_a_project_package() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("package.json"),
        r#"{"workspaces":["packages/../unselected"]}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("packages")).unwrap();
    let sub = root.join("unselected");
    fs::create_dir_all(&sub).unwrap();
    fs::write(
        sub.join("package.json"),
        r#"{"scripts":{"unexpected":"true"}}"#,
    )
    .unwrap();
    let doc = Document {
        path: root.join("AGENTS.md"),
        source: "# T\n\n```sh\npnpm unexpected\n```\n".into(),
    };
    assert_eq!(
        rules::analyze(&doc, &Config::default(), root)
            .unwrap()
            .iter()
            .filter(|d| d.rule_id == "KS009")
            .count(),
        1
    );
}
