#!/usr/bin/env python3
"""coldsum.py <dir> <apps,…> — cold start (no insertion, linked probe) per app: median ms of the rounds, runs, pre-main, busy."""
import json, glob, statistics as st, sys
d, apps = sys.argv[1], sys.argv[2].split(',')
print('| app | cold ms (median) | runs | exec→probe ctor | main busy to first ink |')
print('|---|---|---|---|---|')
for a in apps:
    cs = [json.load(open(f)).get('coldstart') for f in sorted(glob.glob(f'{d}/{a}-coldstart-*.json'))]
    cs = [c for c in cs if c]
    if not cs: print(f'| {a} | - | 0 | | |'); continue
    m = lambda k: st.median(c[k] for c in cs)
    print(f"| {a} | {m('ms'):.0f} | {', '.join(str(round(c['ms'])) for c in cs)} | {m('procToCtorMs'):.0f} | {m('busyMsSinceCtor'):.0f} |")
