#!/usr/bin/env python3
"""Run all requested measurements/checks without fail-fast; keep artifacts in cache."""
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
OUT = Path.home() / "Library/Caches/exact2-cluster-lod/out/L2b"
OUT.mkdir(parents=True, exist_ok=True)

def run(name, command):
    started = time.monotonic()
    with (OUT / (name + ".log")).open("w") as log:
        child = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        record = {"name": name, "pid": child.pid, "command": command}
        with (OUT / "processes.jsonl").open("a") as history:
            history.write(json.dumps(record) + "\n")
        print(json.dumps(record), flush=True)
        code = child.wait()
    record.update(exit=code, seconds=time.monotonic() - started)
    with (OUT / "runs.jsonl").open("a") as history:
        history.write(json.dumps(record) + "\n")
    print(json.dumps(record), flush=True)
    return code

mode = sys.argv[1] if len(sys.argv) > 1 else "verify"
failures = []
commands = []
if mode == "verify":
    commands = [
        ("tests", ["cargo", "test", "--workspace", "--no-fail-fast", "--", "--nocapture"]),
        ("clippy", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]),
        ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
        ("wasm-format", ["cargo", "build", "-p", "clod-format", "--target", "wasm32-unknown-unknown"]),
        ("wasm-view", ["cargo", "build", "-p", "clod-view", "--lib", "--target", "wasm32-unknown-unknown"]),
    ]
elif mode in ("sweep", "oracles"):
    for asset in ("gaul", "washington"):
        source = str(OUT.parent / (asset + "-1.clod"))
        if mode == "oracles":
            commands.append((asset + "-oracle", ["target/debug/clod-view", "oracle", source, "--steps", "64", "--size", "256x256"]))
            continue
        for layout in ("single", "ring:12", "grid:400", "field:5000,1"):
            for selector in ("gpu", "brute", "cpu", "naive"):
                name = "-".join((asset, layout.replace(":", "-").replace(",", "-"), selector))
                command = ["target/debug/clod-view", "time", source, "--layout", layout, "--frames", "7", "--size", "2560x1440", "--threshold-px", "1", "--out", str(OUT / (name + ".png"))]
                command += ["--mode", "naive"] if selector == "naive" else ["--select", selector]
                commands.append((name, command))
else:
    raise SystemExit("usage: measure.py verify|sweep|oracles")
for name, command in commands:
    if run(name, command):
        failures.append(name)
lengths = [(len(p.read_text().splitlines()), str(p.relative_to(ROOT))) for p in ROOT.rglob("*") if p.suffix in (".rs", ".wgsl", ".py", ".c", ".cpp", ".h") and "target" not in p.parts and "vendor" not in p.parts]
for count, path in lengths:
    if count > 1500:
        failures.append(f"{path}: {count} lines")
print(json.dumps({"mode": mode, "commands": len(commands), "source_files": len(lengths), "max_lines": max(lengths), "failures": failures}), flush=True)
sys.exit(bool(failures))
