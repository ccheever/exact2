#!/usr/bin/env python3
"""Summarize heavybench probe JSON files.

fling / ladder: per segment fps, hitch ms/s, p95 frame, main-thread busy (ms/frame, ms/s),
CPU ms/s (process, main thread), blank frames; ladder breaking point.
jump: time to a blank-free viewport.  coldstart: process start -> first blank-free list frame.
Every file: live on/off and memory (phys_footprint at load, start, peak, end).
agg.py imports this module."""
import json, sys, statistics as st

BREAK_FPS = 110  # ladder breaking point: first speed where fps < this (on a 120 Hz screen), or any blank frame


def break_fps(max_fps):
    """110 at 120 Hz; the same fraction of a slower screen's rate (55 on a 60 Hz simulator)."""
    return BREAK_FPS * (max_fps or 120) / 120


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p / 100 * len(xs)))] if xs else float('nan')


def mb(b):
    return b / 1048576 if b else float('nan')


def segments(d):
    """Split the per-frame arrays at the segment markers (-1)."""
    segs, cur = [], None
    busy = d.get('busyFrameMs') or [None] * len(d['dts'])
    for dt, ex, bz in zip(d['dts'], d['expected'], busy):
        if dt < 0:
            cur = {'dts': [], 'exp': [], 'busy': []}
            segs.append(cur)
            continue
        if cur is not None:
            cur['dts'].append(dt); cur['exp'].append(ex)
            if bz is not None: cur['busy'].append(bz)
    return segs


def fling_rows(d):
    """One row per measured (non-warm-up) segment, keyed by signed speed."""
    blank = d.get('blank') or []
    stats = d.get('segStats') or []
    bi, rows = 0, []
    for i, (meta, s) in enumerate(zip(d['segments'], segments(d))):
        n = len(s['dts'])
        b = [x for x in blank[bi:bi + n] if x >= 0]; bi += n
        if meta.get('warm') or not n:
            continue
        dur = sum(s['dts'])
        hitch = sum(max(0.0, a - e) for a, e in zip(s['dts'], s['exp']))
        ss = stats[i] if i < len(stats) else {}
        rows.append({
            'speed': meta['v'] * meta['dir'],
            'fps': n / dur,
            'hitch': 1000 * hitch / dur,
            'p95': 1000 * pct(s['dts'], 95),
            'max': 1000 * max(s['dts']),
            'busy_f': ss.get('busyMsPerFrame', float('nan')),
            'busy_s': ss.get('busyMsPerSec', float('nan')),
            'busy_p95': pct(s['busy'], 95) if s['busy'] else float('nan'),
            'cpu_s': ss.get('cpuMsPerSec', float('nan')),
            'main_s': ss.get('mainCpuMsPerSec', float('nan')),
            'travel': ss.get('travelPt'),
            'target': meta['v'] * meta.get('dur', 2.0),
            'label': meta.get('inner') or (f"{meta['kind']} {meta['trip']}" if meta.get('trip') else ''),
            'blank': sum(1 for x in b if x > 0) if b else None,
            'severe': sum(1 for x in b if x >= 250) if b else None,
            'sampled': len(b) if b else None,
            'bandmax': max(b) if b else None,
        })
    return rows


def breaking_point(rows, max_fps=120):
    """First speed (by |v|, down before up) where fps < break_fps(max_fps) or blank > 0."""
    for r in sorted(rows, key=lambda r: (abs(r['speed']), -r['speed'])):
        why = []
        if r['fps'] < break_fps(max_fps): why.append(f"fps {r['fps']:.0f}")
        if r['blank']: why.append(f"blank {r['blank']}/{r['sampled']}")
        if why:
            return r['speed'], ', '.join(why)
    return None, 'none'


def header(path, d):
    m = d.get('mem', {})
    print(f"== {path}  bundle={d['bundle']} scenario={d['scenario']} live={'on' if d.get('live') else 'off'} "
          f"sample={d['sample']} render={d.get('render')} scroll={d.get('scrollClass')} maxFps={d.get('maxFps')} "
          f"sampleMs={d.get('sampleMs', 0):.1f} {d.get('error', '')}")
    if m:
        print(f"   memory MB: load {mb(m.get('load')):.1f}  start {mb(m.get('start')):.1f}  peak {mb(m.get('peak')):.1f}  "
              f"end {mb(m.get('end')):.1f}  (kernel lifetime peak {mb(m.get('lifetimePeak')):.1f})")
    r = d.get('run')
    if r and r.get('sec', 0) >= 0.5:
        print(f"   whole run: {r['sec']:.1f} s, busy {r['busyMsPerSec']:.0f} ms/s ({r['busyMsPerFrame']:.2f} ms/frame), "
              f"CPU {r['cpuMsPerSec']:.0f} ms/s, main {r['mainCpuMsPerSec']:.0f} ms/s")


def show_fling(d):
    rows = fling_rows(d)
    inner = d['scenario'] in ('innerfling', 'innerkeep')
    if inner: print(f"   inner lists: {d.get('innerLists')}")
    print(f"{'what':>14} " if inner else '', end='')
    print(f"{'speed':>7} {'fps':>6} {'hitch':>6} {'p95':>5} {'max':>6} {'busy/f':>7} {'bz p95':>6} {'busy/s':>6} "
          f"{'cpu/s':>6} {'main/s':>6} {'travel%':>7} {'blankF':>8} {'>=250':>5} {'maxPt':>5}")
    for r in rows:
        bf = '' if r['blank'] is None else f"{r['blank']}/{r['sampled']}"
        sv = '' if r['severe'] is None else str(r['severe'])
        xp = '' if r['bandmax'] is None else f"{r['bandmax']:.0f}"
        tr = '' if r['travel'] is None or not r['target'] else f"{100 * r['travel'] / r['target']:.0f}"
        if inner: print(f"{r['label']:>14} ", end='')
        print(f"{r['speed']:>7} {r['fps']:>6.1f} {r['hitch']:>6.1f} {r['p95']:>5.1f} {r['max']:>6.1f} {r['busy_f']:>7.2f} "
              f"{r['busy_p95']:>6.1f} {r['busy_s']:>6.0f} {r['cpu_s']:>6.0f} {r['main_s']:>6.0f} {tr:>7} {bf:>8} {sv:>5} {xp:>5}")
    for k in d.get('keep') or []:
        print(f"   keep {k['kind']:>9} {k['trip']:>4}: mark {k['mark']:.0f} (read back {k['markRead'] if k['markRead'] is None else round(k['markRead'])}) "
              f"-> got {k['got'] if k['got'] is None else round(k['got'])}  kept={k['kept']}  sameView={k.get('sameView')} {k.get('note', '')}")
    if d['scenario'] == 'ladder':
        sp, why = breaking_point(rows, d.get('maxFps'))
        print(f"breaking point (fps < {break_fps(d.get('maxFps')):.0f} or blank > 0): {sp if sp is not None else 'none'} ({why})")


def show_jump(d):
    ms = [j['ms'] for j in d['jumps']]
    ok = [m for m in ms if m is not None]
    print('jump ms:', ' '.join('timeout' if m is None else f'{m:.0f}' for m in ms))
    if ok:
        print(f"median {st.median(ok):.0f} max {max(ok):.0f} timeouts {len(ms) - len(ok)}")
    dts = [x for x in d['dts'] if x > 0]
    if dts:
        print(f"frames p95 {1000 * pct(dts, 95):.1f} ms, max {1000 * max(dts):.1f} ms")


def show_cold(d):
    c = d.get('coldstart')
    if not c:
        print('coldstart: no result', d.get('error', '')); return
    print(f"cold start {c['ms']:.0f} ms to a blank-free list frame (on screen at {c['targetMs']:.0f} ms), "
          f"start = {c['source']}; process start -> probe constructor {c['procToCtorMs']:.0f} ms")
    print(f"   first display-link tick {c['firstTickMs']:.0f} ms, list found {c['scrollFoundMs']:.0f} ms, "
          f"{c['frames']} frames sampled (leading top gap ignored {c.get('leadingGapPt', 0):.0f} pt); by then CPU {c['cpuMs']:.0f} ms (main {c['mainCpuMs']:.0f}), "
          f"main busy since constructor {c['busyMsSinceCtor']:.0f} ms, footprint {mb(c['footprint']):.1f} MB")


def main(paths):
    for path in paths:
        d = json.load(open(path))
        header(path, d)
        sc = d['scenario']
        if sc in ('fling', 'ladder', 'rest', 'innerfling', 'innerkeep'): show_fling(d)
        elif sc == 'jump': show_jump(d)
        elif sc == 'coldstart': show_cold(d)


if __name__ == '__main__':
    main(sys.argv[1:])
