#!/usr/bin/env python3
"""Create a reviewed release snapshot and advance its local ref."""
import fnmatch
import io
import os
import re
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import tomllib

from hygiene import problems

root = Path(__file__).resolve().parent.parent
if len(sys.argv) != 3:
    sys.exit("usage: prepare-release.sh COMMIT BRANCH")
revision, branch = sys.argv[1:]
ref = "refs/heads/" + branch

def git(*args, **kwargs):
    result = subprocess.run(["git", *args], cwd=root, capture_output=True, **kwargs)
    if result.returncode:
        sys.exit(result.stderr.decode(errors="replace").strip() or "git command failed")
    return result.stdout

git("check-ref-format", ref)
commit = git("rev-parse", "--verify", revision + "^{commit}").decode().strip()
old_result = subprocess.run(["git", "rev-parse", "--verify", "--quiet", ref], cwd=root, capture_output=True)
if old_result.returncode not in (0, 1):
    sys.exit(old_result.stderr.decode(errors="replace").strip() or "git command failed")
old = old_result.stdout.decode().strip() if old_result.returncode == 0 else None
# Only these development artifacts are omitted; all remaining files are scanned.
excluded = [".internal/*", "STATUS-*", "ARRET-*", "DELIVERY-READY-*", "docs/HANDOFF-*", "docs/ARRET-*", "docs/DELIVERY-READY-*", "docs/VERIFICATION*", "docs/MUTATION-*", "docs/PACKAGING-FINAL.md", "docs/GATES-EVIDENCE.md", "docs/IMPLEMENTATION-PLAN.md"]
data = git("archive", "--format=tar", commit)
with tempfile.TemporaryDirectory(prefix="kanso-release-") as tmp:
    target = Path(tmp) / "tree"
    target.mkdir()
    issues = []
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        for member in archive:
            name = member.name.rstrip("/")
            if not name or any(part.lower() == ".internal" or part.lower().startswith(("status-", "handoff", "arret")) for part in Path(name).parts) or any(fnmatch.fnmatchcase(name, pattern) for pattern in excluded):
                continue
            if member.isdir():
                issues.extend(problems(name, b""))
                continue
            if not member.isfile() or Path(name).is_absolute() or ".." in Path(name).parts:
                sys.exit(f"unsupported archive entry: {name}")
            content = archive.extractfile(member).read()
            issues.extend(problems(name, content))
            path = target / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            path.chmod(member.mode & 0o777)
    if issues:
        sys.exit("\n".join(issues))
    try:
        version = tomllib.loads((target / "Cargo.toml").read_text())["package"]["version"]
    except (OSError, UnicodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        sys.exit(f"invalid candidate manifest: {error}")
    if not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        sys.exit("invalid candidate version")
    env = os.environ.copy()
    env.update({"GIT_AUTHOR_NAME": "Kodawari", "GIT_AUTHOR_EMAIL": "237834781+Rushford444@users.noreply.github.com",
                "GIT_COMMITTER_NAME": "Kodawari", "GIT_COMMITTER_EMAIL": "237834781+Rushford444@users.noreply.github.com"})
    env["GIT_INDEX_FILE"] = str(Path(tmp) / "index")
    gitdir = git("rev-parse", "--absolute-git-dir").decode().strip()
    def stage(*args, input=None):
        result = subprocess.run(["git", "--git-dir", gitdir, "--work-tree", str(target), *args], env=env, input=input, capture_output=True)
        if result.returncode:
            sys.exit(result.stderr.decode(errors="replace").strip() or "git command failed")
        return result.stdout
    stage("add", "--force", "--all")
    tree = stage("write-tree").decode().strip()
    parent = ["-p", old] if old else []
    new = stage("commit-tree", tree, *parent, input=f"Release v{version}\n".encode()).decode().strip()
    stage("update-ref", ref, new, old or "0" * len(new))
    print(f"Updated {ref} at {new} from {commit}; no checkout or push performed.")
