#!/usr/bin/env python3
"""abtable.py <results-dir> [apps] — an A/B series read as medians over rounds per app and scenario, with every
round's value beside them (the order alternated each round: an order effect shows as an odd/even split).
Scenarios: fling-t, innerfling-t, rest (whatever is present). Columns: fps, busy ms/frame, process CPU ms/s,
main-thread CPU ms/s, peak footprint MB. Requires check-m2.py to have passed on the directory."""
import json, glob, os, statistics, sys, collections

d = sys.argv[1]
apps = sys.argv[2].split(',') if len(sys.argv) > 2 else None
files = sorted(glob.glob(f'{d}/*-*-*.json'))
if apps is None:
    apps = []
    for f in files:
        a = os.path.basename(f).split('-')[0]
        if a not in apps:
            apps.append(a)
med = statistics.median


def load(app, kind):
    out = {}
    for f in glob.glob(f'{d}/{app}-{kind}-*.json'):
        r = int(os.path.basename(f).rsplit('-', 1)[1].split('.')[0])
        out[r] = json.load(open(f))
    return out


def stat(r, key):
    run = r['run']
    if key == 'fps':
        return run['frames'] / run['sec']
    if key == 'peak':
        return r['mem']['peak'] / 2**20
    return run[key]


keys = [('fps', 'fps', '%.1f'), ('busyMsPerFrame', 'busy/f', '%.2f'), ('cpuMsPerSec', 'cpu ms/s', '%.0f'),
        ('mainCpuMsPerSec', 'main ms/s', '%.0f'), ('peak', 'peak MB', '%.0f')]
for kind in ('fling-t', 'innerfling-t', 'rest'):
    have = {a: load(a, kind) for a in apps}
    if not any(have.values()):
        continue
    print(f'### {kind}')
    print('| app | ' + ' | '.join(f'{name} (rounds)' for _, name, _ in keys) + ' |')
    print('|---' * (len(keys) + 1) + '|')
    for a in apps:
        rs = have[a]
        if not rs:
            continue
        cells = []
        for key, name, fmt in keys:
            vals = [stat(rs[r], key) for r in sorted(rs)]
            cells.append((fmt % med(vals)) + ' (' + ', '.join(fmt % v for v in vals) + ')')
        print(f'| {a} | ' + ' | '.join(cells) + ' |')
    print()
# The fling by speed: main-thread CPU ms/s per app, medians (where the report cycle shows: 1000 pt/s)
have = {a: load(a, 'fling-t') for a in apps}
if any(have.values()):
    acc = {a: collections.defaultdict(list) for a in apps}
    for a in apps:
        for r in have[a].values():
            for seg, st in zip(r['segments'], r['segStats']):
                if not seg.get('warm'):
                    acc[a][seg['v'] * seg['dir']].append(st['mainCpuMsPerSec'])
    vs = sorted({v for a in apps for v in acc[a]}, key=lambda v: (abs(v), -v))
    print('| fling main ms/s by speed | ' + ' | '.join(str(v) for v in vs) + ' |')
    print('|---' * (len(vs) + 1) + '|')
    for a in apps:
        if acc[a]:
            print(f'| {a} | ' + ' | '.join('%.0f' % med(acc[a][v]) for v in vs) + ' |')
    print()
    acc = {a: collections.defaultdict(list) for a in apps}
    for a in apps:
        for r in have[a].values():
            for seg, st in zip(r['segments'], r['segStats']):
                if not seg.get('warm'):
                    acc[a][seg['v'] * seg['dir']].append(st['cpuMsPerSec'])
    print('| fling process cpu ms/s by speed | ' + ' | '.join(str(v) for v in vs) + ' |')
    print('|---' * (len(vs) + 1) + '|')
    for a in apps:
        if acc[a]:
            print(f'| {a} | ' + ' | '.join('%.0f' % med(acc[a][v]) for v in vs) + ' |')
