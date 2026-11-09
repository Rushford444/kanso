#!/usr/bin/env python3
import os
import re
import tarfile
import tomllib
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parent

class ReleaseTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        for name in ("hygiene.py", "prepare_release.py", "prepare-release.sh"):
            shutil.copy2(SCRIPTS / name, self.root / "scripts" / name)
        self.git("init", "-q")
        self.git("config", "user.name", "Tester")
        self.git("config", "user.email", "test@example.invalid")
        (self.root / ".gitignore").write_text("__pycache__/\n")
        (self.root / "README.md").write_text("public text\n")
        for name in ("AGENTS.md", "SKILL.md", "llms.txt", "README.fr.md"):
            (self.root / name).write_text("public documentation\n")
        (self.root / "Cargo.toml").write_text('[package]\nname = "fixture"\nversion = "0.1.4"\n')
        (self.root / ".internal" / "nested" / "more").mkdir(parents=True)
        (self.root / ".internal" / "nested" / "more" / "private.txt").write_text("private")
        (self.root / "sub" / ".internal" / "deep").mkdir(parents=True)
        (self.root / "sub" / ".internal" / "deep" / "private.txt").write_text("private")
        (self.root / "docs").mkdir()
        (self.root / "docs" / "VERIFICATION.md").write_text("private\n")
        (self.root / "docs" / "DELIVERY-READY-refused.json").write_text("private\n")
        self.commit()

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True, text=True).stdout.strip()

    def commit(self):
        self.git("add", "-A")
        self.git("commit", "-qm", "candidate")

    def prepare(self, name):
        return subprocess.run([str(self.root / "scripts" / "prepare-release.sh"), "HEAD", name], cwd=self.root, capture_output=True, text=True)

    def test_release_notes_require_matching_tag_and_version(self):
        notes = SCRIPTS / "release_notes.py"
        (self.root / "CHANGELOG.md").write_text("# Changelog\n\n## [0.1.4] - 2026-11-06\n\n### Changed\n\n- Fixture.\n\n## [0.1.3] - old\n\n- Older.\n")
        good = subprocess.run(["python3", str(notes), "v0.1.4"], cwd=self.root, capture_output=True, text=True)
        self.assertEqual(good.returncode, 0, good.stderr)
        self.assertIn("- Fixture.", good.stdout)
        self.assertNotIn("Older", good.stdout)
        bad = subprocess.run(["python3", str(notes), "v0.1.3"], cwd=self.root, capture_output=True, text=True)
        self.assertNotEqual(bad.returncode, 0)
        (self.root / "CHANGELOG.md").write_text("# Changelog\n\n## [0.1.3] - old\n\n- Older.\n")
        missing = subprocess.run(["python3", str(notes), "v0.1.4"], cwd=self.root, capture_output=True, text=True)
        self.assertNotEqual(missing.returncode, 0)

    def test_release_asset_checks_cover_all_archives(self):
        import hashlib
        assets = self.root / "assets"
        assets.mkdir()
        names = ["kanso-0.1.4-x86_64-unknown-linux-gnu.tar.gz", "kanso-0.1.4-x86_64-apple-darwin.tar.gz", "kanso-0.1.4-aarch64-apple-darwin.tar.gz", "kanso-0.1.4-x86_64-pc-windows-msvc.zip"]
        for name in names:
            (assets / name).write_bytes(name.encode())
            digest = hashlib.sha256(name.encode()).hexdigest()
            (assets / (name + ".sha256")).write_text(f"{digest}  {name}\n")
        def check(*args):
            return subprocess.run(["python3", str(SCRIPTS / "check_release_assets.py"), "0.1.4", str(assets), *args], capture_output=True, text=True)
        self.assertEqual(check("--write-sums").returncode, 0)
        sums = assets / "SHA256SUMS"
        expected = "".join(f"{hashlib.sha256((assets / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(names))
        self.assertEqual(sums.read_text(), expected)
        self.assertEqual(check().returncode, 0)
        sums.write_text("wrong\n")
        self.assertNotEqual(check().returncode, 0)
        sums.unlink()
        self.assertNotEqual(check().returncode, 0)
        sums.write_text(expected)
        (assets / names[2]).unlink()
        self.assertNotEqual(check().returncode, 0)
        (assets / names[2]).write_bytes(names[2].encode())
        (assets / "unexpected").write_text("extra")
        self.assertNotEqual(check().returncode, 0)
        (assets / "unexpected").unlink()
        (assets / names[1]).write_bytes(b"corrupt")
        self.assertNotEqual(check().returncode, 0)
        (assets / names[1]).write_bytes(names[1].encode())
        (assets / (names[2] + ".sha256")).write_text((assets / (names[0] + ".sha256")).read_text())
        self.assertNotEqual(check().returncode, 0)

    def test_apple_silicon_matrix_and_archive_uses_real_host(self):
        workflow = (SCRIPTS.parent / ".github/workflows/dist.yml").read_text()
        self.assertIn("- os: macos-15\n            target: aarch64-apple-darwin", workflow)
        self.assertIn("- os: macos-15-intel\n            target: x86_64-apple-darwin", workflow)
        dist = self.root / "dist"
        dist.mkdir()
        shutil.copy2(SCRIPTS.parent / "dist/build.py", dist / "build.py")
        (self.root / "LICENSE").write_text("license")
        for name in ("README.md", "README.fr.md", "CONTRIBUTING.md", "SECURITY.md", "docs/configuration.md", "docs/usage.md", "docs/fr/configuration.md", "docs/fr/usage.md", "docs/demo.tape", "docs/assets/banner-dark.jpg", "docs/assets/banner-light.jpg"):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(SCRIPTS.parent / name, dest)
        binary = self.root / "target/release/kanso"
        binary.parent.mkdir(parents=True)
        binary.write_text("binary")
        bindir = self.root / "bin"
        bindir.mkdir()
        for name, script in (("cargo", "exit 0"), ("rustc", "echo 'host: aarch64-apple-darwin'")):
            executable = bindir / name
            executable.write_text("#!/bin/sh\n" + script + "\n")
            executable.chmod(0o755)
        env = os.environ.copy()
        env["PATH"] = str(bindir) + os.pathsep + env["PATH"]
        result = subprocess.run(["python3", str(dist / "build.py")], env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.root / "target/dist/kanso-0.1.4-aarch64-apple-darwin.tar.gz"
        self.assertTrue(archive.is_file())
        self.assertTrue(archive.with_name(archive.name + ".sha256").is_file())
        with tarfile.open(archive) as bundle:
            members = set(bundle.getnames())
            for name in ("README.md", "README.fr.md", "docs/configuration.md", "docs/usage.md", "docs/fr/configuration.md", "docs/fr/usage.md"):
                self.assertIn(name, members)
                text = bundle.extractfile(name).read().decode()
                targets = re.findall(r"\[[^]]*\]\(([^)]+)\)", text)
                for value in re.findall(r'<(?:img|source)\b[^>]*?\b(?:src|srcset)=["\']([^"\']+)', text):
                    targets.extend(candidate.strip().split()[0] for candidate in value.split(","))
                for target in targets:
                    target = target.split()[0].split("#")[0]
                    if target and not re.match(r"(?:[a-zA-Z][a-zA-Z0-9+.-]*:|//|/)", target):
                        resolved = os.path.normpath(os.path.join(os.path.dirname(name), target))
                        self.assertIn(resolved, members, f"{name}: {target}")

    def test_gitleaks_allowlisted_paths_are_tracked(self):
        config = tomllib.loads((SCRIPTS.parent / ".gitleaks.toml").read_text())
        self.assertTrue(config["extend"]["useDefault"])
        tracked = subprocess.run(["git", "ls-files"], cwd=SCRIPTS.parent, check=True, capture_output=True, text=True).stdout.splitlines()
        for pattern in config["allowlist"]["paths"]:
            self.assertTrue(any(re.search(pattern, path) for path in tracked), pattern)

    def test_parentless_and_checkout_untouched(self):
        branch = self.git("branch", "--show-current")
        head = self.git("rev-parse", "HEAD")
        result = self.prepare("public")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("rev-list", "--count", "public"), "1")
        self.assertEqual(self.git("branch", "--show-current"), branch)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        tree = self.git("ls-tree", "-r", "--name-only", "public")
        self.assertNotIn("VERIFICATION.md", tree)
        self.assertNotIn("DELIVERY-READY", tree)
        self.assertNotIn(".internal/", tree)
        for name in ("AGENTS.md", "SKILL.md", "llms.txt", "README.fr.md"):
            self.assertIn(name, tree.splitlines())
        previous = self.git("rev-parse", "public")
        (self.root / "README.md").write_text("updated public text\n")
        self.commit()
        result = self.prepare("public")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("rev-list", "--parents", "-n", "1", "public").split()[1:], [previous])
        self.git("merge-base", "--is-ancestor", previous, "public")
        self.assertEqual(self.git("show", "-s", "--format=%an <%ae>|%cn <%ce>|%s", "public"),
                         "Kodawari <237834781+Rushford444@users.noreply.github.com>|Kodawari <237834781+Rushford444@users.noreply.github.com>|Release v0.1.4")

    def test_nested_artifacts_are_excluded_case_insensitively(self):
        for name in ("docs/STATUS-x.md", "docs/notes/handoff-x.md", "notes/ArReT-x.md", "nested/.INTERNAL/private.txt"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("private")
        self.commit()
        result = self.prepare("clean")
        self.assertEqual(result.returncode, 0, result.stderr)
        tree = self.git("ls-tree", "-r", "--name-only", "clean").lower()
        for name in ("status-x", "handoff-x", "arret-x", ".internal"):
            self.assertNotIn(name, tree)

    def test_plural_and_names_are_rejected(self):
        for index, word in enumerate(["la" + "nes", "Wor" + "kers", "Lu" + "is", "Miel" + "anis", "pR" + "iMe"]):
            (self.root / "README.md").write_text(word)
            self.commit()
            self.assertNotEqual(self.prepare(f"private-{index}").returncode, 0)
        (self.root / "README.md").write_text("Kodawari")
        self.commit()
        result = self.prepare("legitimate")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_package_excludes_nested_artifacts(self):
        manifest = (SCRIPTS.parent / "Cargo.toml").read_text()
        exclude = next(line for line in manifest.splitlines() if line.startswith("exclude = "))
        (self.root / "Cargo.toml").write_text('[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2024"\n' + exclude + '\n')
        (self.root / "src").mkdir()
        (self.root / "src" / "lib.rs").write_text("")
        for name in ("docs/STATUS-x.md", "docs/notes/handoff-x.md", "notes/ArReT-x.md", "nested/.INTERNAL/private.txt"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("private")
        self.commit()
        result = subprocess.run(["cargo", "package", "--list", "--allow-dirty", "--offline"], cwd=self.root, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ("status-x", "handoff-x", "arret-x", ".internal"):
            self.assertNotIn(name, result.stdout.lower())

    def test_failures_do_not_show_tracebacks(self):
        self.assertEqual(self.prepare("public").returncode, 0)
        previous = self.git("rev-parse", "public")
        (self.root / "README.md").write_text("/home/" + "node")
        self.commit()
        result = self.prepare("public")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("rev-parse", "public"), previous)
        self.assertNotIn("Traceback", result.stderr)

    def test_concurrent_ref_change_is_not_overwritten(self):
        self.assertEqual(self.prepare("public").returncode, 0)
        (self.root / "README.md").write_text("changed\n")
        self.commit()
        competing = self.git("rev-parse", "HEAD")
        bindir = self.root / "bin"
        bindir.mkdir()
        real_git = shutil.which("git")
        wrapper = bindir / "git"
        wrapper.write_text(f'''#!/bin/sh
case " $* " in
  *" update-ref refs/heads/public "*) "{real_git}" update-ref refs/heads/public {competing} ;;
esac
exec "{real_git}" "$@"
''')
        wrapper.chmod(0o755)
        env = os.environ.copy()
        env["PATH"] = str(bindir) + os.pathsep + env["PATH"]
        result = subprocess.run([str(self.root / "scripts" / "prepare-release.sh"), "HEAD", "public"], cwd=self.root, capture_output=True, text=True, env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("rev-parse", "public"), competing)
        self.assertNotIn("Traceback", result.stderr)

    def test_rejects_private_paths(self):
        for index, value in enumerate(["/home/" + "node", "/Us" + "ers/"]):
            (self.root / "README.md").write_text(value)
            self.commit()
            self.assertNotEqual(self.prepare(f"bad-path-{index}").returncode, 0)
        self.assertEqual(self.git("branch", "--list", "bad-path-*"), "")

    def test_rejects_content_and_names(self):
        words = ["ar" + "gos", "jar" + "vis", "Pr" + "ime", "la" + "ne", "RO" + "OT", "wor" + "ker"]
        for index, word in enumerate(words):
            with self.subTest(word=index, location="content"):
                (self.root / "README.md").write_text(word)
                self.commit()
                self.assertNotEqual(self.prepare(f"bad-content-{index}").returncode, 0)
                (self.root / "README.md").write_text("public")
            with self.subTest(word=index, location="name"):
                path = self.root / (word + "_note")
                path.write_text("public")
                self.commit()
                self.assertNotEqual(self.prepare(f"bad-name-{index}").returncode, 0)
                path.unlink()
        self.assertEqual(self.git("branch", "--list", "bad-*"), "")

if __name__ == "__main__":
    unittest.main()
