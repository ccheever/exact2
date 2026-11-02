#!/usr/bin/env python3
"""Summarize listbench probe JSON (analyze.py <result.json> ...): per fling segment fps, hitch ms/s, p95 frame,
blank frames; per jump time to content."""
import json, sys, statistics as st

def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p / 100 * len(xs)))] if xs else float('nan')

def fling(d):
    segs, cur = [], None
    blank = d.get('blank') or []
    bi = 0
    for dt, ex in zip(d['dts'], d['expected']):
        if dt < 0:
            cur = {'dts': [], 'exp': [], 'blank': []}
            segs.append(cur)
            continue
        if cur is None:
            continue
        cur['dts'].append(dt); cur['exp'].append(ex)
    # blank samples are one per driven frame; split them by frame counts
    if blank:
        for s in segs:
            n = len(s['dts'])
            s['blank'] = blank[bi:bi + n]; bi += n
    rows = []
    for meta, s in zip(d['segments'], segs):
        if not s['dts'] or meta.get('warm'):
            continue
        dur = sum(s['dts'])
        hitch = sum(max(0.0, dt - ex) for dt, ex in zip(s['dts'], s['exp']))
        b = [x for x in s['blank'] if x >= 0]
        rows.append({
            'speed': meta['v'] * meta['dir'],
            'fps': len(s['dts']) / dur,
            'hitch_ms_s': 1000 * hitch / dur,
            'p95_ms': 1000 * pct(s['dts'], 95),
            'max_ms': 1000 * max(s['dts']),
            'blank_frames': sum(1 for x in b if x > 0) if b else None,
            'severe': sum(1 for x in b if x >= 250) if b else None,
            'sampled': len(b) if b else None,
            'blank_pt_mean': st.mean(b) if b else None,
            'blank_pt_max': max(b) if b else None,
        })
    return rows

def main():
    for path in sys.argv[1:]:
        d = json.load(open(path))
        print(f"== {path}  bundle={d['bundle']} scenario={d['scenario']} sample={d['sample']} scroll={d.get('scrollClass')} maxFps={d.get('maxFps')} render={d.get('render')} sampleMs={d.get('sampleMs',0):.1f} {d.get('error','')}")
        if d['scenario'] == 'fling':
            print(f"{'speed':>7} {'fps':>6} {'hitch':>7} {'p95':>6} {'max':>6} {'blankF':>7} {'>=250':>6} {'meanPt':>7} {'maxPt':>6}")
            for r in fling(d):
                bf = '' if r['blank_frames'] is None else f"{r['blank_frames']}/{r['sampled']}"
                mp = '' if r['blank_pt_mean'] is None else f"{r['blank_pt_mean']:.0f}"
                xp = '' if r['blank_pt_max'] is None else f"{r['blank_pt_max']:.0f}"
                sv = '' if r['severe'] is None else str(r['severe'])
                print(f"{r['speed']:>7} {r['fps']:>6.1f} {r['hitch_ms_s']:>7.1f} {r['p95_ms']:>6.1f} {r['max_ms']:>6.1f} {bf:>7} {sv:>6} {mp:>7} {xp:>6}")
        else:
            ms = [j['ms'] for j in d['jumps']]
            ok = [m for m in ms if m is not None]
            print('jump ms:', ' '.join('timeout' if m is None else f'{m:.0f}' for m in ms))
            if ok:
                print(f"median {st.median(ok):.0f} max {max(ok):.0f} timeouts {len(ms) - len(ok)}")
            dts = [x for x in d['dts'] if x > 0]
            if dts:
                print(f"frames p95 {1000 * pct(dts, 95):.1f} ms, max {1000 * max(dts):.1f} ms")

main()
