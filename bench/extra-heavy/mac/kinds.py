#!/usr/bin/env python3
"""kinds.py <kinds-results-dir> [apps] — the per-kind table: medians over rounds of fling-t (whole-run fps, late/s,
busy ms/frame, main CPU ms/s, peak MB, fps at ±24k) and ladder-t's breaking point, SwiftUI vs exact2."""
import json, glob, os, sys, statistics
d = sys.argv[1]; apps = sys.argv[2].split(",") if len(sys.argv) > 2 else ["swiftui", "exact2"]; med = lambda xs: statistics.median(xs) if xs else float('nan')
def load(k, app, sc): return [json.load(open(f)) for f in sorted(glob.glob(f'{d}/{k}/{app}-{sc}-*.json'))]
def st(r):
    dts = [x for x in r['dts'][1:] if x and x > 0]; per = med([e for e in r['expected'][1:] if e and e > 0])
    run = r['run']; sp = {}
    for s, ss in zip(r['segments'], r['segStats']):
        if not s.get('warm'): sp[s['v'] * s['dir']] = ss['frames'] / ss['sec']
    return dict(fps=run['frames'] / run['sec'], late=sum(1 for x in dts if x > 1.5 * per) / run['sec'],
                busy=run['busyMsPerFrame'], main=run['mainCpuMsPerSec'], cpu=run['cpuMsPerSec'], peak=r['mem']['peak'] / 2**20,
                v24=(sp.get(24000, float('nan')) + sp.get(-24000, float('nan'))) / 2)
def brk(r):
    for s, ss in zip(r['segments'], r['segStats']):
        if not s.get('warm') and ss['frames'] / ss['sec'] < 110 / 120 * r.get('maxFps', 120): return str(s['v'] * s['dir'])
    return '-'
print('| kind | app | fps | late/s | fps ±24k | busy ms/f | main ms/s | cpu ms/s | peak MB | ladder break (3 rounds) |')
print('|---|---|---|---|---|---|---|---|---|---|')
for k in sorted(os.listdir(d)):
    if not os.path.isdir(f'{d}/{k}'): continue
    for app in apps:
        s = [st(r) for r in load(k, app, 'fling-t')]
        if not s: continue
        m = {x: med([y[x] for y in s]) for x in s[0]}
        b = ','.join(brk(r) for r in load(k, app, 'ladder-t'))
        print(f"| {k} | {app} | {m['fps']:.1f} | {m['late']:.1f} | {m['v24']:.1f} | {m['busy']:.2f} | {m['main']:.0f} | {m['cpu']:.0f} | {m['peak']:.0f} | {b} |")
