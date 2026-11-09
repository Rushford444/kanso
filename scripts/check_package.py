#!/usr/bin/env python3
"""Check the crate archive and all tracked working-tree files."""
import pathlib
import re
import subprocess
import sys
import tarfile

from hygiene import problems

root = pathlib.Path(__file__).resolve().parent.parent
listed = subprocess.run(["cargo", "package", "--list", "--allow-dirty", "--offline"], cwd=root, check=True, capture_output=True, text=True).stdout.splitlines()
subprocess.run(["cargo", "package", "--allow-dirty", "--no-verify", "--offline"], cwd=root, check=True)
manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
version = re.search(r'^version = "([^"]+)"', manifest, re.M).group(1)
archive = root / "target" / "package" / f"kdw-kanso-{version}.crate"
issues = []
with tarfile.open(archive, "r:gz") as crate:
    members = crate.getmembers()
    archived = {member.name.split("/", 1)[1] for member in members if member.isfile()}
    if archived != set(listed):
        issues.append(f"package list/archive mismatch: {archived ^ set(listed)}")
    for member in members:
        if member.isfile():
            issues.extend(problems(member.name, crate.extractfile(member).read()))
        else:
            issues.extend(problems(member.name, b""))
if sys.argv[1:] not in ([], ["--tree"]):
    sys.exit("usage: check_package.py [--tree]")
if sys.argv[1:]:
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True).stdout.split(b"\0")
    for entry in filter(None, tracked):
        name = entry.decode("utf-8", "surrogateescape")
        path = root / name
        if path.is_symlink():
            issues.extend(problems(name, str(path.readlink()).encode()))
        elif path.is_file():
            issues.extend(problems(name, path.read_bytes()))
        else:
            issues.append(f"{name}: tracked file missing")
if issues:
    print("\n".join(issues), file=sys.stderr)
    sys.exit(1)
print("Package hygiene OK" + ("; tracked tree hygiene OK" if sys.argv[1:] else ""))
