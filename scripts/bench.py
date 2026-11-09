#!/usr/bin/env python3
"""Measure a warm release binary over 1,000 generated instruction files."""
import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

root = Path(__file__).resolve().parent.parent
target = root / "target"
target.mkdir(exist_ok=True)
binary = target / "release" / ("kanso.exe" if __import__("os").name == "nt" else "kanso")
with tempfile.TemporaryDirectory(prefix="kanso-bench-", dir=target) as directory:
    tree = Path(directory)
    for index in range(1000):
        project = tree / f"unit-{index:04}"
        project.mkdir()
        (project / "AGENTS.md").write_text(
            f"# Module {index}\n\nRun the test suite before changing this module.\n",
            encoding="utf-8",
        )
    command = [str(binary), "check", str(tree), "--json"]
    warm = subprocess.run(command, capture_output=True, check=False)
    if warm.returncode not in (0, 1):
        raise SystemExit(f"warmup failed ({warm.returncode}): {warm.stderr.decode(errors='replace')}")
    times = []
    for _ in range(5):
        start = time.perf_counter()
        result = subprocess.run(command, capture_output=True, check=False)
        times.append(time.perf_counter() - start)
        if result.returncode not in (0, 1):
            raise SystemExit(f"run failed ({result.returncode}): {result.stderr.decode(errors='replace')}")
        report = json.loads(result.stdout)
        if len(report["files"]) != 1000:
            raise SystemExit(f"expected 1000 files, got {len(report['files'])}")
    print(f"seconds={times}; median={statistics.median(times):.6f}; max={max(times):.6f}")
