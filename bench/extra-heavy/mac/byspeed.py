#!/usr/bin/env python3
"""byspeed.py <results-dir> [apps] — fling-t per speed, medians over rounds: fps, run-loop busy ms/frame and
ms/s, main-thread CPU ms/s, process CPU ms/s. What the whole-run columns of summarize.py average away: a cost
paid per scroll frame shows at 1000 pt/s, where almost no row is owed."""
import json, glob, statistics, sys, collections
d = sys.argv[1]
apps = sys.argv[2].split(',') if len(sys.argv) > 2 else ['swiftui', 'exact2']
med = statistics.median
for key, title, fmt in (('fps', 'fps', '%.1f'), ('busyMsPerFrame', 'busy ms/frame', '%.1f'), ('busyMsPerSec', 'busy ms/s', '%.0f'),
                        ('mainCpuMsPerSec', 'main CPU ms/s', '%.0f'), ('cpuMsPerSec', 'process CPU ms/s', '%.0f')):
    acc = {a: collections.defaultdict(list) for a in apps}
    for a in apps:
        for f in sorted(glob.glob(f'{d}/{a}-fling-t-*.json')):
            r = json.load(open(f))
            for seg, st in zip(r['segments'], r['segStats']):
                if seg.get('warm'):
                    continue
                acc[a][seg['v'] * seg['dir']].append(st['frames'] / st['sec'] if key == 'fps' else st[key])
    vs = sorted({v for a in apps for v in acc[a]}, key=lambda v: (abs(v), -v))
    print(f'| fling {title} by speed | ' + ' | '.join(str(v) for v in vs) + ' |')
    print('|---' * (len(vs) + 1) + '|')
    for a in apps:
        print(f'| {a} | ' + ' | '.join(fmt % med(acc[a][v]) for v in vs) + ' |')
    print()
