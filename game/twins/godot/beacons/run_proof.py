#!/usr/bin/env python3
"""Process orchestration only. Game, inputs and assertions all execute in Godot."""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parent
OUT = ROOT / 'artifacts'
GODOT = Path(os.environ.get('GODOT', '~/Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot')).expanduser()
DIARY = ROOT / '001-beacons-godot.md'


def stamp(message):
    line = f'{datetime.now(timezone.utc).isoformat()} — {message}'
    print(line, flush=True)
    with DIARY.open('a') as diary:
        diary.write('\n- ' + line + '\n')


def run(name, mode, headless=True):
    stamp(f'Run {name}: mode={mode}, headless={headless}, --fixed-fps 60.')
    command = [str(GODOT), '--path', str(ROOT), '--fixed-fps', '60',
               '--log-file', str(OUT / f'{name}.engine.log'), '--audio-driver', 'Dummy']
    if headless:
        command.append('--headless')
    command += ['--', f'--mode={mode}', f'--output=res://artifacts/{name}']
    report_path = OUT / f"{name}.json"
    if report_path.exists():
        report_path.unlink()
    started = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    (OUT / f'{name}.stdout.log').write_text(result.stdout + result.stderr)
    report_path = OUT / f'{name}.json'
    report = json.loads(report_path.read_text()) if report_path.exists() else {}
    ok = result.returncode == 0 and report and not report.get('failures')
    ok = ok and 'SCRIPT ERROR' not in result.stderr
    # Known OS user-data startup errors do not invalidate in-project game assertions.
    # Preserve stderr and report this limitation independently of functional proof.
    (OUT / f'{name}.launch.json').write_text(json.dumps({'exit': result.returncode, 'stderr': result.stderr}, indent=2))
    stamp(f'{name}: {"PASS" if ok else "FAIL"} in {time.monotonic() - started:.3f} s; exit={result.returncode}; {len(report.get("checks", []))} Godot assertions.')
    if not ok:
        print(result.stdout + result.stderr, flush=True)
    return bool(ok)


def main():
    started = time.monotonic()
    OUT.mkdir(exist_ok=True)
    for name in ['run-a.final', 'run-b.final', 'run-a.state', 'run-b.state', 'restored.state', 'playing.png']:
        (OUT / name).unlink(missing_ok=True)
    # Sequential: the second run writes the checkpoint restored by the third.
    results = [run('run-a', 'full'), run('run-b', 'full'), run('restored', 'restore')]
    equality = {}
    for label, names in [('same_script', ['run-a.final', 'run-b.final']),
                         ('fresh_process_continuation', ['run-a.state', 'run-b.state', 'restored.state'])]:
        blobs = [(OUT / name).read_bytes() if (OUT / name).exists() else None for name in names]
        equality[label] = blobs[0] is not None and all(blob == blobs[0] for blob in blobs)
        stamp(f'{label}: {"PASS" if equality[label] else "FAIL"}, byte-for-byte full serialized state.')
    results.append(run('screenshot', 'screenshot', headless=False))
    passed = all(results) and all(equality.values()) and (OUT / 'playing.png').exists()
    summary = {'passed': passed, 'equality': equality,
               'wall_seconds': time.monotonic() - started,
               'completed_utc': datetime.now(timezone.utc).isoformat(),
               'state_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.glob('*.state')}}
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    stamp(f'Aggregate proof {"PASS" if passed else "FAIL"}: {summary["wall_seconds"]:.3f} s.')
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
