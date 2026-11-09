#!/usr/bin/env python3
"""Build a local binary archive; never upload or publish."""
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tomllib
import zipfile

root = Path(__file__).resolve().parent.parent
subprocess.run(["cargo", "build", "--release", "--locked"], cwd=root, check=True)
subprocess.run(["cargo", "package", "--locked", "--allow-dirty", "--no-verify"], cwd=root, check=True)
host_info = subprocess.run(["rustc", "-vV"], cwd=root, check=True, capture_output=True, text=True).stdout
host = next(line.split(": ", 1)[1] for line in host_info.splitlines() if line.startswith("host: "))
binary = root / "target" / "release" / ("kanso.exe" if os.name == "nt" else "kanso")
out = root / "target" / "dist"
out.mkdir(parents=True, exist_ok=True)
version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
archive = out / f"kanso-{version}-{host}{'.zip' if os.name == 'nt' else '.tar.gz'}"
files = ["LICENSE", "README.md", "README.fr.md", "CONTRIBUTING.md", "SECURITY.md",
         "docs/configuration.md", "docs/usage.md", "docs/fr/configuration.md", "docs/fr/usage.md",
         "docs/demo.tape", "docs/assets/banner-dark.jpg", "docs/assets/banner-light.jpg"]
entries = [(binary, binary.name)] + [(root / name, name) for name in files]
if os.name == "nt":
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as result:
        for path, name in entries:
            result.write(path, name)
else:
    with tarfile.open(archive, "w:gz") as result:
        for path, name in entries:
            result.add(path, arcname=name, recursive=False)
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
checksum = archive.with_name(archive.name + ".sha256")
checksum.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
print(f"archive: {archive}\nsha256: {checksum}")
