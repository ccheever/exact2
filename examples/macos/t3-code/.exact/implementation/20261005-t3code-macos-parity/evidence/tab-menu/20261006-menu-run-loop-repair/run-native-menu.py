"""Run from repository root on an unlocked Mac; briefly opens three native menus."""
import hashlib
import json
from pathlib import Path
import subprocess

here = Path(__file__).resolve().parent
root = Path.cwd()
source = root / "examples/macos/t3-code/modules/apple/T3ContextMenu.swift"
out = root / "target/t3-menu-run-loop-repair"
out.mkdir(parents=True, exist_ok=True)
exe = out / "menu-check"
subprocess.run(["xcrun", "swiftc", str(source), str(here / "main.swift"), "-o", str(exe)], check=True)
results = []
for mode in ["select", "escape", "settle"]:
    run = subprocess.run([str(exe), mode], capture_output=True, text=True, timeout=12)
    (here / (mode + ".log")).write_text(run.stdout + run.stderr)
    results.append({"mode": mode, "exit_code": run.returncode, "output": run.stdout})
report = {"source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
          "fixture_sha256": hashlib.sha256((here / "main.swift").read_bytes()).hexdigest(),
          "checks": results}
(here / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
assert all(row["exit_code"] == 0 for row in results), "Native menu regression failed"
