#!/usr/bin/env python3
"""Check the complete set of release assets against their checksums."""
import hashlib
from pathlib import Path
import re
import sys

if (len(sys.argv) not in (3, 4) or (len(sys.argv) == 4 and sys.argv[3] != "--write-sums")) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", sys.argv[1]):
    sys.exit("usage: check_release_assets.py VERSION DIRECTORY [--write-sums]")
version, directory = sys.argv[1], Path(sys.argv[2])
names = [f"kanso-{version}-x86_64-unknown-linux-gnu.tar.gz",
         f"kanso-{version}-x86_64-apple-darwin.tar.gz",
         f"kanso-{version}-aarch64-apple-darwin.tar.gz",
         f"kanso-{version}-x86_64-pc-windows-msvc.zip"]
if len(sys.argv) == 4:
    if not all((directory / name).is_file() for name in names):
        sys.exit("unexpected or missing release assets")
    sums = "".join(f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(names))
    (directory / "SHA256SUMS").write_text(sums, encoding="utf-8")
expected = set(names) | {name + ".sha256" for name in names} | {"SHA256SUMS"}
if {path.name for path in directory.iterdir()} != expected:
    sys.exit("unexpected or missing release assets")
for name in names:
    digest = hashlib.sha256((directory / name).read_bytes()).hexdigest()
    if (directory / (name + ".sha256")).read_text(encoding="utf-8") != f"{digest}  {name}\n":
        sys.exit(f"invalid checksum: {name}")
sums = "".join(f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(names))
if (directory / "SHA256SUMS").read_bytes() != sums.encode():
    sys.exit("invalid checksum: SHA256SUMS")
print("All release assets and checksums verified")
