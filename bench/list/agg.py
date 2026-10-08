#!/usr/bin/env python3
"""Aggregate a paired series directory: medians across rounds per app and speed.
Usage: agg.py <results dir> (devseries.sh / series.sh write one)."""
import glob, json, os, statistics as st, sys

def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p / 100 * len(xs)))] if xs else float('nan')

def fling_rows(d):
    segs, cur = [], None
    for dt, ex in zip(d['dts'], d['expected']):
        if dt < 0:
            cur = {'dts': [], 'exp': []}; segs.append(cur); continue
        if cur is not None:
            cur['dts'].append(dt); cur['exp'].append(ex)
    blank = d.get('blank') or []
    bi, out = 0, {}
    for meta, s in zip(d['segments'], segs):
        n = len(s['dts']); b = [x for x in blank[bi:bi + n] if x >= 0]; bi += n
        if meta.get('warm') or not n:
            continue
        dur = sum(s['dts'])
        hitch = sum(max(0.0, a - e) for a, e in zip(s['dts'], s['exp']))
        out[meta['v'] * meta['dir']] = dict(fps=n / dur, hitch=1000 * hitch / dur, p95=1000 * pct(s['dts'], 95),
                                            blank=sum(1 for x in b if x > 0), severe=sum(1 for x in b if x >= 250),
                                            sampled=len(b), bandmax=max(b) if b else 0)
    return out

def load(pattern):
    return [json.load(open(p)) for p in sorted(glob.glob(pattern))]

APPS = [a for a in ('expo', 'exact', 'swiftui')]
def main(dirpath):
    global APPS
    APPS = [a for a in APPS if glob.glob(f'{dirpath}/{a}-*.json')]
    for kind, label in (('fling-t', 'TIMING (no sampling)'), ('fling-b', 'BLANK (layer sample every 4th frame)')):
        print(f'\n## {label}')
        apps = {app: [fling_rows(d) for d in load(f'{dirpath}/{app}-{kind}-*.json')] for app in APPS}
        speeds = sorted({s for rs in apps.values() for r in rs for s in r}, key=lambda s: (abs(s), -s))
        if kind == 'fling-t':
            print(f"{'speed':>7} | " + " | ".join(f"{a+' fps':>11} {'hitch':>6} {'p95':>5}" for a in APPS) + "   (medians; runs " + "/".join(str(len(apps[a])) for a in APPS) + ")")
            for s in speeds:
                row = []
                for app in APPS:
                    rs = [r[s] for r in apps[app] if s in r]
                    row.append((st.median(x['fps'] for x in rs), st.median(x['hitch'] for x in rs), st.median(x['p95'] for x in rs)) if rs else (float('nan'),) * 3)
                print(f"{s:>7} | " + " | ".join(f"{r[0]:>11.1f} {r[1]:>6.0f} {r[2]:>5.1f}" for r in row))
        else:
            print(f"{'speed':>7} | " + " | ".join(f"{a+' blank/n':>16} {'>=250':>5} {'maxPt':>5}" for a in APPS) + "   (summed over runs)")
            for s in speeds:
                row = []
                for app in APPS:
                    rs = [r[s] for r in apps[app] if s in r]
                    row.append((sum(x['blank'] for x in rs), sum(x['sampled'] for x in rs), sum(x['severe'] for x in rs), max((x['bandmax'] for x in rs), default=0)))
                print(f"{s:>7} | " + " | ".join(f"{str(r[0])+'/'+str(r[1]):>16} {r[2]:>5} {r[3]:>5.0f}" for r in row))
            for app in APPS:
                ms = [d.get('sampleMs', 0) for d in load(f'{dirpath}/{app}-{kind}-*.json')]
                if ms: print(f'  {app}: sampler cost {st.median(ms):.1f} ms per sample')
    for jk in sorted({os.path.basename(p).rsplit('-', 1)[0] for p in glob.glob(f'{dirpath}/*-jump*.json')}):
        ds = load(f'{dirpath}/{jk}-*.json')
        ms = [j['ms'] for d in ds for j in d['jumps']]
        ok = [m for m in ms if m is not None]
        cost = st.median(d.get('sampleMs', 0) for d in ds)
        if ok:
            print(f"{jk:>18}: time to full viewport p50 {st.median(ok):.0f} ms, p90 {pct(ok, 90):.0f}, max {max(ok):.0f}, timeouts {len(ms) - len(ok)}/{len(ms)} (sampler {cost:.1f} ms/frame)")

main(sys.argv[1])
