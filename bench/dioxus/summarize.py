#!/usr/bin/env python3
"""summarize.py — medians over rounds for the Dioxus comparison's web and macOS results.

  summarize.py web <web.json> [...]            bench.mjs output: load, scroll per speed, taps, per app
  summarize.py mac <dir> [...]                 macbench output (<app>-cold-<r>.json, <app>-scroll-<r>.json): every app
                                               found, its files pooled over the directories
  summarize.py mac <label>=<app>@<dir>[,<dir>...] ...
                                               one row per label: app <app>'s files from those directories

Every number is a median over the runs it pools; the n beside each says how many."""
import glob, json, os, re, statistics as st, sys

med = lambda xs: st.median(xs) if xs else float('nan')


def web(files):
    d = [r for f in files for r in json.load(open(f))]
    print("== WEB (Chrome, medians over rounds) ==")
    for app in dict.fromkeys(r['app'] for r in d):
        rs = [r for r in d if r['app'] == app]
        L = lambda k: med([r['load'][k] for r in rs])
        print(f"{app:7} firstRow {L('firstRowMs'):6.0f} ms  LCP {L('lcpMs'):5.0f}  bytes(non-img) {L('bytesNonImage')/1e6:5.2f} MB  heap {L('jsHeapMB'):5.1f} MB  rendererRSS {L('rendererRssMB'):5.0f} MB  nodes {L('nodes')}  endRSS {med([r['end']['rendererRssMB'] for r in rs]):5.0f}  (n={len(rs)})")
        for i in range(min(len(r['scroll']) for r in rs)):
            S = lambda k: med([r['scroll'][i][k] for r in rs])
            print(f"   {S('pxPerS'):6.0f} px/s fps {S('fps'):5.1f} late {S('late'):3.0f} worst {S('worstFrameMs'):5.1f} p99 {S('p99FrameMs'):4.1f} blank {S('blankPct'):.1f}%  main {S('mainTaskMsPerS'):4.0f} (script {S('scriptMsPerS'):3.0f} layout {S('layoutMsPerS'):3.0f} style {S('styleMsPerS'):3.0f}) ms/s  chromeCPU {S('chromeCpuMsPerS'):4.0f} ms/s")
        taps = [x for r in rs if 'tap' in r for x in r['tap']['eventMs']]
        print(f"   tap: counted {[r.get('tap', {}).get('counted') for r in rs]}  event durations ms median {med(taps) if taps else 'none>16'} n={len(taps)} (Event Timing reports only >=16 ms)")


def mac(args):
    rows = []
    for a in args:
        m = re.fullmatch(r'([^=]+)=([^@]+)@(.+)', a)
        if m:
            rows.append((m[1], m[2], m[3].split(',')))
    if not rows:
        apps = sorted({re.sub(r'-(cold|scroll)-\d+\.json$', '', os.path.basename(f)) for d in args for f in glob.glob(f'{d}/*-*-*.json')})
        rows = [(a, a, args) for a in apps]
    print("== macOS (medians over rounds) ==")
    for label, app, dirs in rows:
        cold = [json.load(open(f)) for d in dirs for f in glob.glob(f"{d}/{app}-cold-*.json")]
        sc = [json.load(open(f)) for d in dirs for f in glob.glob(f"{d}/{app}-scroll-*.json")]
        print(f"{label:13} cold firstInk {med([c['firstInkMs'] for c in cold]):5.0f} ms (n={len(cold)}: {sorted(round(c['firstInkMs']) for c in cold)})  footprint 1.5 s after launch {med([c['footprintMB'] for c in cold]):5.0f} MB")
        speeds = max((len(s['runs']) for s in sc), default=0)
        for i in range(speeds):
            runs = [s['runs'][i] for s in sc if len(s['runs']) > i]
            M = lambda k: med([r[k] for r in runs])
            sp = runs[0].get('speed', runs[0].get('ptPerS', '?'))
            print(f"   {sp:6} pt/s fps {M('fps'):5.1f} late {M('late'):3.0f} worst {M('worstMs'):5.1f} blankFrames {M('blankFrames'):.0f}  cpu {M('cpuMsPerS'):4.0f} ms/s  footprint {M('footprintMB'):5.0f} MB (n={len(runs)})")


if len(sys.argv) < 3 or sys.argv[1] not in ('web', 'mac'):
    sys.exit(__doc__)
(web if sys.argv[1] == 'web' else mac)(sys.argv[2:])
