#!/usr/bin/env python3
"""Aggregate a series directory of <app>-<kind>-<round>.json files: medians across rounds.

Any apps (swiftui, exact, expo, ...) and any kinds. A kind's scenario comes from the JSON.
Paired fling kinds <X>-t (timing, unsampled) and <X>-b (blank-sampled) are joined into one table.
Usage: agg.py <results dir>"""
import glob, json, os, re, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze import fling_rows, breaking_point, break_fps, pct, mb

ORDER = ('swiftui', 'exact', 'expo')
NAME = re.compile(r'^([^-]+)-(.+)-(\d+)\.json$')
nan = float('nan')


def med(xs):
    xs = [x for x in xs if x is not None and x == x]
    return st.median(xs) if xs else nan


def load(dirpath):
    runs = {}  # app -> kind -> [json]
    for p in sorted(glob.glob(os.path.join(dirpath, '*.json'))):
        m = NAME.match(os.path.basename(p))
        if not m: continue
        app, kind, _ = m.groups()
        runs.setdefault(app, {}).setdefault(kind, []).append(json.load(open(p)))
    return runs


def speed_table(timing, blank, title):
    """timing / blank: lists of per-run fling_rows keyed by speed (blank may be the same runs)."""
    t = [{r['speed']: r for r in rows} for rows in timing]
    b = [{r['speed']: r for r in rows} for rows in blank]
    speeds = sorted({s for rs in t + b for s in rs}, key=lambda s: (abs(s), -s))
    print(f"\n### {title}")
    print(f"{'speed':>7} {'fps':>6} {'hitch':>6} {'p95':>5} {'busy/f':>6} {'busy/s':>6} {'cpu/s':>6} {'main/s':>6} | "
          f"{'blank/n':>9} {'>=250':>5} {'maxPt':>5}")
    for s in speeds:
        rs = [r[s] for r in t if s in r]
        bs = [r[s] for r in b if s in r and r[s]['sampled']]
        vals = [med(x[k] for x in rs) for k in ('fps', 'hitch', 'p95', 'busy_f', 'busy_s', 'cpu_s', 'main_s')]
        bl = f"{sum(x['blank'] for x in bs)}/{sum(x['sampled'] for x in bs)}" if bs else '-'
        sv = str(sum(x['severe'] for x in bs)) if bs else '-'
        mx = f"{max(x['bandmax'] for x in bs):.0f}" if bs else '-'
        print(f"{s:>7} {vals[0]:>6.1f} {vals[1]:>6.0f} {vals[2]:>5.1f} {vals[3]:>6.2f} {vals[4]:>6.0f} {vals[5]:>6.0f} "
              f"{vals[6]:>6.0f} | {bl:>9} {sv:>5} {mx:>5}")


def app_report(app, kinds, summary):
    print(f"\n## {app}   (medians across rounds; blank summed; busy/f = main-thread busy ms per frame; "
          f"cpu/s, main/s = CPU ms per second of scrolling)")
    s = summary.setdefault(app, {})
    done = set()
    for kind in sorted(kinds):
        if kind in done: continue
        ds = kinds[kind]
        sc = ds[0]['scenario']
        n = len(ds)
        if sc in ('fling', 'ladder', 'rest'):
            base, pair = (kind[:-2], None)
            if kind.endswith('-t') or kind.endswith('-b'):
                other = base + ('-b' if kind.endswith('-t') else '-t')
                pair = other if other in kinds else None
            if pair:
                tk, bk = (kind, pair) if kind.endswith('-t') else (pair, kind)
                done |= {tk, bk}
                timing = [fling_rows(d) for d in kinds[tk]]
                blank = [fling_rows(d) for d in kinds[bk]]
                live = 'on' if kinds[tk][0].get('live') else 'off'
                speed_table(timing, blank, f"{base} — timing {tk} ({len(timing)} runs), blank {bk} "
                            f"({len(blank)} runs, sampler {med(d.get('sampleMs') for d in kinds[bk]):.1f} ms), live {live}")
            else:
                done.add(kind)
                rows = [fling_rows(d) for d in ds]
                live = 'on' if ds[0].get('live') else 'off'
                sm = ds[0]['sample']
                speed_table(rows, rows, f"{kind} — {n} runs, scenario {sc}, live {live}, "
                            f"sample {'every ' + str(sm) + ' (' + ds[0].get('render', '') + ')' if sm else 'off'}")
                timing = rows
            if sc == 'ladder':
                mf = ds[0].get('maxFps')
                runs = timing
                if pair:
                    # fps from the unsampled run, blank from the sampled run of the same round
                    runs = []
                    for i, t in enumerate(timing):
                        bl = {r['speed']: r for r in (blank[i] if i < len(blank) else [])}
                        runs.append([dict(r, blank=bl.get(r['speed'], {}).get('blank'),
                                          sampled=bl.get(r['speed'], {}).get('sampled')) for r in t])
                bps = [breaking_point(r, mf) for r in runs]
                print(f"breaking point (fps < {break_fps(mf):.0f} or blank > 0), per run: "
                      + '; '.join(f"{sp if sp is not None else 'none'} ({why})" for sp, why in bps))
                s['ladder'] = [sp for sp, _ in bps]
            if base == 'fling' or kind == 'fling':
                all_rows = [r for rows in timing for r in rows]
                s['fps'] = med(r['fps'] for r in all_rows)
                s['busy_f'] = med(r['busy_f'] for r in all_rows)
                s['cpu_s'] = med(r['cpu_s'] for r in all_rows)
                s['main_s'] = med(r['main_s'] for r in all_rows)
                if pair:
                    s['blank'] = sum(r['blank'] or 0 for rows in blank for r in rows)
                    s['sampled'] = sum(r['sampled'] or 0 for rows in blank for r in rows)
            if kind.startswith('fling-live'):
                all_rows = [r for rows in timing for r in rows]
                s['live_fps'] = med(r['fps'] for r in all_rows)
        elif sc == 'jump':
            done.add(kind)
            ms = [j['ms'] for d in ds for j in d['jumps']]
            ok = [m for m in ms if m is not None]
            if ok:
                print(f"\n### {kind}: time to full viewport p50 {st.median(ok):.0f} ms, p90 {pct(ok, 90):.0f}, max {max(ok):.0f}, "
                      f"timeouts {len(ms) - len(ok)}/{len(ms)} (sampler {med(d.get('sampleMs') for d in ds):.1f} ms/frame)")
                s['jump'] = st.median(ok)
        elif sc == 'coldstart':
            done.add(kind)
            cs = [d['coldstart'] for d in ds if d.get('coldstart')]
            errs = [d.get('error') for d in ds if not d.get('coldstart')]
            print(f"\n### {kind}: {len(cs)}/{n} runs")
            if cs:
                srcs = sorted({c['source'] for c in cs})
                print(f"cold start to blank-free list frame: median {med(c['ms'] for c in cs):.0f} ms "
                      f"(runs: {', '.join('%.0f' % c['ms'] for c in cs)}; start = {'/'.join(srcs)}); "
                      f"list found {med(c['scrollFoundMs'] for c in cs):.0f} ms, first tick {med(c['firstTickMs'] for c in cs):.0f} ms, "
                      f"process->constructor {med(c['procToCtorMs'] for c in cs):.0f} ms; CPU by then {med(c['cpuMs'] for c in cs):.0f} ms "
                      f"(main {med(c['mainCpuMs'] for c in cs):.0f}), footprint {mb(med(c['footprint'] for c in cs)):.1f} MB")
                s['cold'] = med(c['ms'] for c in cs)
            for e in errs: print('  error:', e)
    # memory, per kind
    print(f"\n### memory (phys_footprint MB, medians): kind  start  peak  end  lifetime-peak")
    for kind in sorted(kinds):
        ms = [d.get('mem') for d in kinds[kind] if d.get('mem')]
        if not ms: continue
        v = [mb(med(m.get(k) for m in ms)) for k in ('start', 'peak', 'end', 'lifetimePeak')]
        print(f"  {kind:>14}  {v[0]:6.1f} {v[1]:6.1f} {v[2]:6.1f} {v[3]:6.1f}")
        s['mem_peak'] = max(s.get('mem_peak', 0), v[3] if v[3] == v[3] else v[1])


def main(dirpath):
    runs = load(dirpath)
    apps = [a for a in ORDER if a in runs] + sorted(a for a in runs if a not in ORDER)
    summary = {}
    for app in apps:
        app_report(app, runs[app], summary)
    print(f"\n## SUMMARY  (fling medians over all measured speeds; ladder = breaking point per run; "
          f"cold = ms from process start)")
    print(f"{'app':>8} {'fps':>6} {'busy/f':>6} {'cpu/s':>6} {'main/s':>6} {'blank/n':>9} {'live fps':>8} "
          f"{'jump':>5} {'cold':>6} {'memMB':>6}  ladder")
    for app in apps:
        s = summary.get(app, {})
        g = lambda k: s.get(k, nan)
        bl = f"{s['blank']}/{s['sampled']}" if 'blank' in s else '-'
        lad = ','.join('none' if x is None else str(x) for x in s.get('ladder', [])) or '-'
        print(f"{app:>8} {g('fps'):>6.1f} {g('busy_f'):>6.2f} {g('cpu_s'):>6.0f} {g('main_s'):>6.0f} {bl:>9} "
              f"{g('live_fps'):>8.1f} {g('jump'):>5.0f} {g('cold'):>6.0f} {g('mem_peak'):>6.1f}  {lad}")


if __name__ == '__main__':
    main(sys.argv[1])
