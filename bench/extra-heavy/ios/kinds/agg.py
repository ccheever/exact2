#!/usr/bin/env python3
"""agg.py [rounds…] — per-kind table from $KROOT/r<round>/<tag>/<app>-{fling,ladder}-t-1.json (median over rounds).
KROOT defaults to run.sh's default output, <checkout>/target/bench/extra-heavy/results/kinds."""
import json, glob, os, statistics, sys, collections
R = os.environ.get('KROOT') or os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..', '..', 'target', 'bench', 'extra-heavy', 'results', 'kinds')
rounds = sys.argv[1:] or sorted(os.path.basename(d) for d in glob.glob(f'{R}/r*'))
med = lambda xs: statistics.median(xs) if xs else float('nan')

def stats(r):
    exp = r['expected']; dts = [x for x in r['dts'][1:] if x and x > 0]
    per = [e for e in exp[1:] if e and e > 0]; period = med(per) if per else 1 / 120
    run = r['run']
    sp = {}
    for seg, st in zip(r['segments'], r['segStats']):
        if not seg.get('warm'): sp[seg['v'] * seg['dir']] = st['frames'] / st['sec']
    bys = collections.defaultdict(list)
    for v, f in sp.items(): bys[abs(v)].append(f)
    return dict(fps=run['frames'] / run['sec'], late=sum(1 for x in dts if x > 1.5 * period) / run['sec'],
                busy=run['busyMsPerFrame'], cpu=run['cpuMsPerSec'], main=run['mainCpuMsPerSec'],
                peak=r['mem']['peak'] / 2**20, sp={v: med(f) for v, f in bys.items()})

data = collections.defaultdict(lambda: collections.defaultdict(list))  # (tag, app, scen) -> [stats]
for rd in rounds:
    for f in glob.glob(f'{R}/{rd}/*/*-t-1.json'):
        tag = os.path.basename(os.path.dirname(f)); app, scen = os.path.basename(f).split('-')[:2]
        try: data[(tag, scen)][app].append(stats(json.load(open(f))))
        except Exception as e: print('skip', f, e, file=sys.stderr)

def m(lst, k, v=None):
    return med([x['sp'].get(v, float('nan')) for x in lst]) if v else med([x[k] for x in lst])

out = {}
tags = sorted({t for t, _ in data})
for tag in tags:
    row = {}
    for app in ('swiftui', 'exact2', 'expo'):
        f = data[(tag, 'fling')].get(app, []); l = data[(tag, 'ladder')].get(app, [])
        if not f and not l: continue
        d = {k: m(f, k) for k in ('fps', 'late', 'busy', 'cpu', 'main', 'peak')} if f else {}
        for v in (1000, 3000, 6000, 12000, 24000): d[f'f{v//1000}k'] = m(f, None, v) if f else float('nan')
        for v in (3000, 6000, 12000, 24000, 48000, 96000): d[f'l{v//1000}k'] = m(l, None, v) if l else float('nan')
        d['n'] = len(f)
        row[app] = d
    out[tag] = row
json.dump(out, open(f'{R}/agg.json', 'w'), indent=1)
cols = ['fps', 'f12k', 'f24k', 'l24k', 'l48k', 'l96k', 'late', 'busy', 'cpu', 'main', 'peak']
print('| kind | n | ' + ' | '.join(f'{c} S / E / Δ' for c in cols) + ' |')
print('|---' * (len(cols) + 2) + '|')
for tag, row in out.items():
    s, e = row.get('swiftui', {}), row.get('exact2', {})
    cells = []
    for c in cols:
        a, b = s.get(c, float('nan')), e.get(c, float('nan'))
        fmt = '%.0f' if c in ('cpu', 'main', 'peak') else '%.1f'
        cells.append(f'{fmt % a} / {fmt % b} / {fmt % (b - a):>s}')
    print(f'| {tag} | {e.get("n", 0)} | ' + ' | '.join(cells) + ' |')
