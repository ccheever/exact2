#!/usr/bin/env python3
"""summarize.py <results-dir> [apps] — the per-app table (swiftui, expo, exact2, uikit by default; the heavy list names its exact2 app `exact`) for a series of the heavy list or the extra-heavy feed (medians over rounds).

From each app's JSONs (<app>-<kind>-<round>.json, the probe's): fling-t whole-run fps, late frames
(dt > 1.5 × the display period) per second, worst frame, busy ms/frame, process CPU and main-thread
CPU ms/s, peak / end footprint; fling-b blank frames (summed); ladder breaking point (first speed
with fps < 110 or a blank); jump p50; cold start; rest CPU and main CPU. Per-speed fps is agg.py's."""
import json, os, statistics, sys, glob, re, collections

d = sys.argv[1]
apps = sys.argv[2].split(',') if len(sys.argv) > 2 else ['swiftui', 'expo', 'exact2', 'uikit']
# An app with no result files in this series is left out of the tables.
apps = [a for a in apps if glob.glob(f'{d}/{a}-*.json')]
med = lambda xs: statistics.median(xs) if xs else float('nan')


def load(app, kind):
    return [json.load(open(f)) for f in sorted(glob.glob(f'{d}/{app}-{kind}-*.json'))]


def fling_stats(r):
    exp = r['expected']
    dts = [x for x in r['dts'][1:] if x and x > 0]
    per = [e for e in exp[1:] if e and e > 0]
    period = med(per) if per else 1 / 120
    late = sum(1 for x in dts if x > 1.5 * period)
    run = r['run']
    return dict(fps=run['frames'] / run['sec'], late=late / run['sec'], worst=max(dts) * 1000,
                busy=run['busyMsPerFrame'], cpu=run['cpuMsPerSec'], main=run['mainCpuMsPerSec'],
                peak=r['mem']['peak'] / 2**20, end=r['mem']['end'] / 2**20)


def speeds(r):
    out = {}
    for seg, st in zip(r['segments'], r['segStats']):
        if seg.get('warm'):
            continue
        out[seg['v'] * seg['dir']] = st['frames'] / st['sec']
    return out


rows = {}
for app in apps:
    t = [fling_stats(r) for r in load(app, 'fling-t')]
    live = [fling_stats(r) for r in load(app, 'fling-live')]
    row = {k: med([x[k] for x in t]) for k in (t[0] if t else {})}
    row['live fps'] = med([x['fps'] for x in live])
    bl = load(app, 'fling-b')
    row['blank'] = sum(sum(1 for b in r.get('blank', []) if b and b > 0) for r in bl)
    rest = load(app, 'rest')
    row['rest cpu'] = med([r['run']['cpuMsPerSec'] for r in rest])
    row['rest main'] = med([r['run']['mainCpuMsPerSec'] for r in rest])
    cold = load(app, 'coldstart')
    row['cold'] = med([r['coldstart']['ms'] for r in cold if r.get('coldstart')])
    breaks = []
    for r in load(app, 'ladder-t'):
        b = None
        for seg, st in zip(r['segments'], r['segStats']):
            if seg.get('warm'):
                continue
            if st['frames'] / st['sec'] < 110:
                b = seg['v'] * seg['dir']; break
        breaks.append(str(b))
    row['ladder'] = ','.join(breaks)
    js = []
    for r in load(app, 'jump-layer'):
        js += [j.get('ms') for j in r.get('jumps', []) if j.get('ms') is not None]
    row['jump p50'] = med(js)
    sp = collections.defaultdict(list)
    for r in load(app, 'fling-t'):
        for v, f in speeds(r).items():
            sp[v].append(f)
    row['speeds'] = {v: med(f) for v, f in sp.items()}
    rows[app] = row

cols = [('fps', '%.1f'), ('live fps', '%.1f'), ('late', '%.1f'), ('worst', '%.0f'), ('busy', '%.2f'), ('cpu', '%.0f'),
        ('main', '%.0f'), ('peak', '%.0f'), ('end', '%.0f'), ('blank', '%d'), ('ladder', '%s'), ('jump p50', '%.0f'),
        ('cold', '%.0f'), ('rest cpu', '%.0f'), ('rest main', '%.0f')]
names = {'fps': 'fps', 'live fps': 'live fps', 'late': 'late/s', 'worst': 'worst ms', 'busy': 'busy/f', 'cpu': 'cpu ms/s',
         'main': 'main ms/s', 'peak': 'peak MB', 'end': 'end MB', 'blank': 'blanks', 'ladder': 'ladder <110',
         'jump p50': 'jump p50', 'cold': 'cold ms', 'rest cpu': 'rest cpu', 'rest main': 'rest main'}
print('| app | ' + ' | '.join(names[c] for c, _ in cols) + ' |')
print('|---' * (len(cols) + 1) + '|')
for app, row in rows.items():
    cells = []
    for c, f in cols:
        v = row.get(c)
        try:
            cells.append(f % v)
        except TypeError:
            cells.append(str(v))
    print(f'| {app} | ' + ' | '.join(cells) + ' |')
allv = sorted({v for r in rows.values() for v in r['speeds']}, key=lambda v: (abs(v), -v))
print()
print('| fling fps by speed | ' + ' | '.join(str(v) for v in allv) + ' |')
print('|---' * (len(allv) + 1) + '|')
for app, row in rows.items():
    print(f'| {app} | ' + ' | '.join('%.1f' % row['speeds'].get(v, float('nan')) for v in allv) + ' |')

# Inner lists (19-kind series only): innerfling fps per inner kind and speed, blanks inside the
# inner list (innerfling 4), innerkeep marks kept.
inner = {app: (load(app, 'innerfling-t'), load(app, 'innerfling-b'), load(app, 'innerkeep')) for app in apps}
if any(v[0] for v in inner.values()):
    for kind in ('filmstrip', 'inbox'):
        sp = {}
        for app, (t, _, _) in inner.items():
            acc = collections.defaultdict(list)
            for r in t:
                for seg, st in zip(r['segments'], r['segStats']):
                    if seg.get('warm') or seg.get('inner') != kind:
                        continue
                    acc[seg['v'] * seg['dir']].append(st['frames'] / st['sec'])
            sp[app] = {v: med(f) for v, f in acc.items()}
        vs = sorted({v for d_ in sp.values() for v in d_}, key=lambda v: (abs(v), -v))
        print()
        print(f'| innerfling {kind} fps | ' + ' | '.join(str(v) for v in vs) + ' |')
        print('|---' * (len(vs) + 1) + '|')
        for app in apps:
            print(f'| {app} | ' + ' | '.join('%.1f' % sp[app].get(v, float('nan')) for v in vs) + ' |')
    print()
    print('| app | inner busy/f | inner cpu ms/s | inner main ms/s | inner blanks (innerfling 4) | innerkeep kept (same item, same place ±1 pt) | shifts pt | raw offset kept | raw offset errors pt |')
    print('|---|---|---|---|---|---|---|---|---|')
    for app, (t, b, k) in inner.items():
        busy = med([r['run']['busyMsPerFrame'] for r in t]); cpu = med([r['run']['cpuMsPerSec'] for r in t])
        main = med([r['run']['mainCpuMsPerSec'] for r in t])
        blanks = sum(sum(1 for x in r.get('blank', []) if x and x > 0) for r in b)
        keeps = [e for r in k for e in r.get('keep', [])]
        # primary: the anchor (probe `keptAnchor`: the box shows the same content within 1 pt); runs from
        # before the anchor check have no keptAnchor and print "n/a"
        anch = [e for e in keeps if 'keptAnchor' in e]
        ka = f"{sum(1 for e in anch if e['keptAnchor'])}/{len(anch)}" if anch else 'n/a'
        sh = ','.join(('?' if e.get('anchorMatch', 0) < 0.9 else str(e['anchorShiftPt'])) for e in anch if not e['keptAnchor']) or '-'
        kept = sum(1 for e in keeps if e.get('kept'))
        errs = ','.join(str(round(e['err'])) for e in keeps if not e.get('kept') and e.get('err') is not None) or '-'
        print(f'| {app} | {busy:.2f} | {cpu:.0f} | {main:.0f} | {blanks} | {ka} | {sh} | {kept}/{len(keeps)} | {errs} |')

# Thermal state (probe 6b10a15abb37, 2026-09-30, on): the worst state each app saw in each scenario over the rounds
# (thermalStart/thermalEnd, each segment's `thermal`, every change notification); "-" = the probe recorded none.
def thermal_of(r):
    vals = [r.get('thermalStart'), r.get('thermalEnd')] + [x.get('thermal') for x in r.get('segStats', [])] + [c[1] for c in r.get('thermalChanges', []) or []]
    vals = [v for v in vals if isinstance(v, (int, float)) and not isinstance(v, bool) and v >= 0]
    return int(max(vals)) if vals else None


kinds = [k for k in ('fling-t', 'fling-live', 'rest', 'ladder-t', 'jump-layer', 'coldstart', 'fling-b', 'innerfling-t', 'innerfling-b', 'innerkeep')
         if any(glob.glob(f'{d}/{a}-{k}-*.json') for a in apps)]
print()
print('| thermal, worst state per scenario (0 nominal, 1 fair, 2 serious, 3 critical; - = not recorded; LP = Low Power Mode) | ' + ' | '.join(kinds) + ' |')
print('|---' * (len(kinds) + 1) + '|')
states = {}
for app in apps:
    cells = []
    for k in kinds:
        rs = load(app, k)
        rec = [t for t in (thermal_of(r) for r in rs) if t is not None]
        lp = any(v is True for r in rs for v in [r.get('lowPowerStart'), r.get('lowPowerEnd')] + [x.get('lowPower') for x in r.get('segStats', [])])
        states[(app, k)] = max(rec) if rec else None
        cells.append(('-' if not rec else str(max(rec))) + (f' ({len(rec)}/{len(rs)} rounds)' if 0 < len(rec) < len(rs) else '') + (' LP' if lp else ''))
    print(f'| {app} | ' + ' | '.join(cells) + ' |')
diff = [k for k in kinds if len({states[(a, k)] for a in apps if states[(a, k)] is not None}) > 1]
if diff:
    print()
    print('The apps ran at DIFFERENT thermal states in: ' + ', '.join(diff) + ' - do not compare those rows across apps.')
