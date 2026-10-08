#!/usr/bin/env python3
"""check-m2.py <results-dir> [apps] [rounds] — is every result of a mac-lane series a whole run?

The runner's "ok" means a result file exists; a `partial` file (the probe's checkpoint, moved into place when the
app was killed at the limit or died) has a 2-second `run` that summarize.py would read as a fling. This lists,
per app and scenario, the rounds that are missing, partial, or short of their segments, and exits 1 if any is.
Also refused: a result whose machine-state reading (<json>.state, written by run.sh from tmp/speed.py) was below
the machine's floor before or after the run, and one with no reading at all unless --ungated says the series
predates the gate. Run it before summarize.py; a series with holes is not a series."""
import json, glob, os, sys

UNGATED = '--ungated' in sys.argv   # a series from before run.sh recorded the machine's state
LOADED = '--loaded' in sys.argv     # a series taken on a deliberately loaded machine (STATE_GATE=off)
argv = [a for a in sys.argv if not a.startswith('--')]
d = argv[1]
apps = argv[2].split(',') if len(argv) > 2 else None
rounds = int(argv[3]) if len(argv) > 3 else 3
feed = ['fling-t', 'fling-live', 'rest', 'ladder-t', 'jump-layer', 'coldstart', 'fling-b', 'innerfling-t', 'innerfling-b', 'innerkeep']
live = ['fling-t', 'ladder-t']
files = glob.glob(f'{d}/*.json') + glob.glob(f'{d}/live/*.json')
if apps is None:
    apps = sorted({os.path.basename(f).split('-')[0] for f in files})
bad = []


def state(path):
    """The machine's state beside a result (run.sh's <json>.state): None if healthy, else why not."""
    sp = path + '.state'
    if not os.path.exists(sp):
        return None if UNGATED else 'no machine-state reading (a run from before the gate: pass --ungated to allow)'
    vals = {}
    for line in open(sp):
        w = line.split()
        if not w:
            continue
        for k in ('speed1', 'speed4', 'min1', 'min4', 'gate'):
            if k in w:
                vals[(w[0], k)] = w[w.index(k) + 1]
    if vals.get(('before', 'gate')) == 'off' and not LOADED:
        return 'taken with the gate off (a loaded machine: pass --loaded to allow)'
    if LOADED:
        return None
    m1, m4 = float(vals.get(('before', 'min1'), 0)), float(vals.get(('before', 'min4'), 0))
    if m1 <= 0 or m4 <= 0:
        return 'the machine had no floor (~/xhm/machine.state missing): its state was read but not held to anything'
    for when in ('before', 'after'):
        s1, s4 = float(vals.get((when, 'speed1'), 0)), float(vals.get((when, 'speed4'), 0))
        if s1 < m1 or s4 < m4:
            return f'machine below its floor {when} the run (speed {s1:.0f}/{s4:.0f}, floor {m1:.0f}/{m4:.0f})'
    return None


def check(path):
    if not os.path.exists(path):
        return 'missing'
    why = state(path)
    if why:
        return why
    try:
        r = json.load(open(path))
    except Exception as e:
        return f'unreadable ({e})'
    if r.get('partial'):
        return f"partial ({r.get('segmentsDone')} of {len(r.get('segments', []))} segments, {r.get('run', {}).get('sec', 0):.1f} s)"
    sc = r.get('scenario')
    segs, st = r.get('segments'), r.get('segStats')
    # Only the scenarios that run segments keep a stat per segment; a jump lists the ladder's segments and
    # fills `jumps`, a cold start fills `coldstart`, innerkeep fills `keep` (probe.m:846).
    if sc in ('fling', 'ladder', 'innerfling') and segs and st is not None and len(st) != len(segs):
        return f'{len(st)} of {len(segs)} segments'
    if sc == 'jump' and not r.get('jumps'):
        return 'no jumps recorded'
    if sc == 'coldstart' and not (r.get('coldstart') or {}).get('ms'):
        return 'no cold start recorded'
    if sc == 'innerkeep' and not r.get('keep'):
        return 'no innerkeep checks recorded'
    if sc in ('fling', 'ladder', 'innerfling', 'rest') and not (r.get('run') or {}).get('frames'):
        return 'no frames'
    w = r.get('window') or {}
    # `visible` is the window's occlusion bit when the result is written. A cold start writes at its first
    # frame: exact2 on bones gets there 254–274 ms after launch, before AppKit has delivered the new window's
    # occlusion state (2 of 3 rounds read 0 there, 2026-09-30; their times equal the round that read 1, and
    # SwiftUI at 550+ ms always reads 1). So a cold start needs key + active only; everything else, written
    # seconds later, needs the bit too.
    front = w.get('key') and w.get('appActive') and (w.get('visible') or sc == 'coldstart')
    if w and not front:
        return f'window not frontmost ({w})'
    return None


n = 0
for app in apps:
    for sub, kinds in (('', feed), ('live/', live)):
        if not glob.glob(f'{d}/{sub}{app}-*.json'):
            continue
        for k in kinds:
            if sub == '' and not glob.glob(f'{d}/{app}-{k}-*.json'):
                continue  # a scenario the series did not ask for
            for rd in range(1, rounds + 1):
                n += 1
                why = check(f'{d}/{sub}{app}-{k}-{rd}.json')
                if why:
                    bad.append(f'{sub}{app}-{k}-{rd}: {why}')
print(f'{n - len(bad)} of {n} results whole ({", ".join(apps)}; {rounds} rounds)')
for b in bad:
    print('  NOT A RUN:', b)
sys.exit(1 if bad else 0)
