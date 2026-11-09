#!/usr/bin/env python3
"""Print the changelog section for a tag matching the package version."""
import re
import sys
import tomllib
from pathlib import Path

if len(sys.argv) != 2:
    sys.exit("usage: release_notes.py vVERSION")
version = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
if sys.argv[1] != f"v{version}":
    sys.exit("tag does not match Cargo.toml version")
changelog = Path("CHANGELOG.md").read_text(encoding="utf-8")
match = re.search(r"(?ms)^## \[" + re.escape(version) + r"\][^\n]*\n(.*?)(?=^## |\Z)", changelog)
if not match or not match.group(1).strip():
    sys.exit("missing changelog section")
print(match.group(1).strip())
