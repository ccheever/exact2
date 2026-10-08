#!/usr/bin/env python3
"""board.py --tag <tag> --rev <commit> [options] — writes SCOREBOARD.md from the board's series (bench lane, 2026-09-30).

One table per benchmark per device, medians of the rounds on disk, one row per app, the columns of
bench/extra-heavy/summarize.py (its arithmetic, copied); under each table every metric where exact2 is worse than the
better of UIKit and SwiftUI (with the ratio) and every metric where it is better. xheavy also gets the inner-list
tables, a second exact2 build's row and delta (--tip-tag/--tip-rev) and the movement against an earlier series
(--prior). Peak MB is the probe's own peak through the fling (`mem.peak` of fling-t), never a segment-end sample.
A result whose `build` stamp is not the expected one is reported at the top (no stale builds).

The series it reads are chain.sh's: <xheavy>/<tag>-19-<dev>, <crypto>/<dev>-<tag>, <heavy>/<dev>-<tag>, where the
three roots default to <checkout>/target/bench/{extra-heavy,crypto-list,heavy-list}/results (--xheavy, --crypto,
--heavy move them). Expected stamps: exact2's are xheavy@{rev}, crypto-svgi@{rev}, heavy@{rev}; --expect
BENCH.APP=STAMP sets one (also SwiftUI's and UIKit's; {rev} is replaced). Output: <checkout>/target/bench/board/
SCOREBOARD.md (--out), or --stdout; --head FILE puts a hand-written header (what was built and measured) on top,
--notes FILE after the ranked losses."""
import json, glob, os, statistics, sys, collections, math, argparse

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
ap = argparse.ArgumentParser(description='Write the scoreboard from the board\'s series.')
ap.add_argument('--tag', required=True, help='the build tag the series are named for (chain.sh <dev> <tag>)')
ap.add_argument('--rev', required=True, help="exact2's commit in those builds (their stamps carry it)")
ap.add_argument('--tip-tag', help='a second exact2 xheavy build in the same rotation (chain.sh TIP_TAG)')
ap.add_argument('--tip-rev', help="that build's commit")
ap.add_argument('--prior', help='an earlier xheavy series tag to compare against (<xheavy>/<prior>-19-<dev>)')
ap.add_argument('--xheavy', default=f'{ROOT}/target/bench/extra-heavy/results', help='Extra Heavy results root')
ap.add_argument('--crypto', default=f'{ROOT}/target/bench/crypto-list/results', help='crypto list results root')
ap.add_argument('--heavy', default=f'{ROOT}/target/bench/heavy-list/results', help='plain heavy list results root')
ap.add_argument('--apps', action='append', default=[], metavar='BENCH=A,B,EXACT2',
                help="a bench's apps, exact2's last (defaults xheavy=swiftui,uikit,exact2 crypto=swiftui,uikit,svgi heavy=swiftui,uikit,exact)")
ap.add_argument('--devices', default='iphone=iPhone,ipad=iPad', help='series suffix=display name, comma-separated, in order')
ap.add_argument('--expect', action='append', default=[], metavar='BENCH.APP=STAMP', help='an expected build stamp ({rev} replaced)')
ap.add_argument('--head', help='a Markdown file put on top of the board')
ap.add_argument('--notes', help='a Markdown file put after the ranked losses')
ap.add_argument('--out', default=f'{ROOT}/target/bench/board/SCOREBOARD.md')
ap.add_argument('--stdout', action='store_true', help='print the board instead of writing it')
ap.add_argument('--summary', action='store_true', help='also print the ranked losses')
A = ap.parse_args()
if bool(A.tip_tag) != bool(A.tip_rev):
    ap.error('--tip-tag and --tip-rev go together')
TAG, REV, TIP_TAG, TIP_REV = A.tag, A.rev, A.tip_tag, A.tip_rev
DEVS = [tuple(x.split('=', 1)) for x in A.devices.split(',')]
APPS = {'xheavy': ['swiftui', 'uikit', 'exact2'], 'crypto': ['swiftui', 'uikit', 'svgi'], 'heavy': ['swiftui', 'uikit', 'exact']}
for x in A.apps:
    b, v = x.split('=', 1); APPS[b] = v.split(',')
EXPECT = {'xheavy': {APPS['xheavy'][-1]: 'xheavy@{rev}'}, 'crypto': {APPS['crypto'][-1]: 'crypto-svgi@{rev}'},
          'heavy': {APPS['heavy'][-1]: 'heavy@{rev}'}}
for x in A.expect:
    k, v = x.split('=', 1); b, a = k.split('.', 1); EXPECT.setdefault(b, {})[a] = v
stamps = lambda b, rev: {a: v.replace('{rev}', rev) for a, v in EXPECT.get(b, {}).items()}
HOME = os.path.expanduser('~')
show = lambda p: os.path.relpath(p, ROOT) if os.path.abspath(p).startswith(ROOT + os.sep) else p.replace(HOME, '~')
med = lambda xs: statistics.median(xs) if xs else float('nan')
nan = float('nan')
isnum = lambda v: isinstance(v, (int, float)) and v == v
problems = []


def load(d, app, kind):
    out = []
    for f in sorted(glob.glob(f'{d}/{app}-{kind}-[0-9].json')):
        try:
            out.append(json.load(open(f)))
        except Exception as e:
            problems.append(f'unreadable result {f}: {e}')
    return out


THERMAL_NAMES = {0: 'nominal', 1: 'fair', 2: 'serious', 3: 'critical'}
ALL_KINDS = ('fling-t', 'fling-live', 'rest', 'ladder-t', 'jump-layer', 'coldstart', 'fling-b', 'innerfling-t', 'innerfling-b', 'innerkeep')


def thermal_of(r):
    """The worst thermal state one result saw (probe 6b10a15abb37 on: thermalStart/End, per-segment thermal, the
    change notifications), or None when the probe recorded none (older probe)."""
    vals = [r.get('thermalStart'), r.get('thermalEnd')] + [x.get('thermal') for x in r.get('segStats', [])] \
        + [c[1] for c in r.get('thermalChanges', []) or []]
    vals = [v for v in vals if isinstance(v, (int, float)) and not isinstance(v, bool) and v >= 0]
    return int(max(vals)) if vals else None


def low_power_of(r):
    return any(v is True for v in [r.get('lowPowerStart'), r.get('lowPowerEnd')] + [x.get('lowPower') for x in r.get('segStats', [])])


def thermal_by_kind(d, app):
    """{kind: (worst state over the rounds that recorded one or None, rounds recorded, rounds, low power seen)}"""
    out = {}
    for k in ALL_KINDS:
        rs = load(d, app, k)
        if not rs:
            continue
        ts = [thermal_of(r) for r in rs]
        rec = [t for t in ts if t is not None]
        out[k] = (max(rec) if rec else None, len(rec), len(rs), any(low_power_of(r) for r in rs))
    return out


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


def list_rows(d, apps):
    rows = {}
    for app in apps:
        t = [fling_stats(r) for r in load(d, app, 'fling-t') if 'run' in r]
        if not t:
            continue
        live = [fling_stats(r) for r in load(d, app, 'fling-live') if 'run' in r]
        row = {k: med([x[k] for x in t]) for k in t[0]}
        row['live fps'] = med([x['fps'] for x in live])
        bl = load(d, app, 'fling-b')
        row['blank'] = sum(sum(1 for b in r.get('blank', []) if b and b > 0) for r in bl) if bl else nan
        rest = load(d, app, 'rest')
        row['rest cpu'] = med([r['run']['cpuMsPerSec'] for r in rest])
        row['rest main'] = med([r['run']['mainCpuMsPerSec'] for r in rest])
        # the rounds' values, printed beside the median: exact2's 19-kind rest swings run to run (the rest lane)
        row['rest cpu all'] = [r['run']['cpuMsPerSec'] for r in rest]
        row['rest main all'] = [r['run']['mainCpuMsPerSec'] for r in rest]
        cold = load(d, app, 'coldstart')
        row['cold'] = med([r['coldstart']['ms'] for r in cold if r.get('coldstart')])
        breaks = []
        for r in load(d, app, 'ladder-t'):
            b = None
            for seg, st in zip(r['segments'], r['segStats']):
                if seg.get('warm'):
                    continue
                if st['frames'] / st['sec'] < 110:
                    b = seg['v'] * seg['dir']; break
            breaks.append(b)
        row['ladder'] = ','.join(str(b) for b in breaks)
        row['ladder n'] = med([abs(b) if b is not None else math.inf for b in breaks]) if breaks else nan
        js = []
        for r in load(d, app, 'jump-layer'):
            js += [j.get('ms') for j in r.get('jumps', []) if j.get('ms') is not None]
        row['jump p50'] = med(js)
        row['jump timeouts'] = sum(1 for r in load(d, app, 'jump-layer') for j in r.get('jumps', []) if j.get('ms') is None)
        sp = collections.defaultdict(list)
        for r in load(d, app, 'fling-t'):
            for v, f in speeds(r).items():
                sp[v].append(f)
        row['speeds'] = {v: med(f) for v, f in sp.items()}
        row['rounds'] = {k: len(glob.glob(f'{d}/{app}-{k}-[0-9].json')) for k in
                         ('fling-t', 'fling-live', 'rest', 'ladder-t', 'jump-layer', 'coldstart', 'fling-b')}
        row['builds'] = sorted({json.load(open(f)).get('build', '?') for f in glob.glob(f'{d}/{app}-*-[0-9].json')})
        row['thermal'] = thermal_by_kind(d, app)
        rows[app] = row
    return rows


COLS = [('fps', '%.1f'), ('live fps', '%.1f'), ('late', '%.1f'), ('worst', '%.0f'), ('busy', '%.2f'), ('cpu', '%.0f'),
        ('main', '%.0f'), ('peak', '%.0f'), ('end', '%.0f'), ('blank', '%d'), ('ladder', '%s'), ('jump p50', '%.0f'),
        ('cold', '%.0f'), ('rest cpu', '%.0f'), ('rest main', '%.0f')]
NAMES = {'fps': 'fps', 'live fps': 'live fps', 'late': 'late/s', 'worst': 'worst ms', 'busy': 'busy/f', 'cpu': 'cpu ms/s',
         'main': 'main ms/s', 'peak': 'peak MB', 'end': 'end MB', 'blank': 'blanks', 'ladder': 'ladder <110',
         'jump p50': 'jump p50', 'cold': 'cold ms', 'rest cpu': 'rest cpu', 'rest main': 'rest main'}
HIGHER = {'fps', 'live fps', 'ladder'}


def fmt(v, f):
    if v is None or (isinstance(v, float) and v != v):
        return '—'
    try:
        return f % v
    except (TypeError, ValueError):
        return str(v)


def table(rows, order):
    out = ['| app | ' + ' | '.join(NAMES[c] for c, _ in COLS) + ' |', '|---' * (len(COLS) + 1) + '|']
    for app in order:
        if app in rows:
            cells = []
            for c, f in COLS:
                cell = fmt(rows[app].get(c), f)
                if c in ('rest cpu', 'rest main') and rows[app].get(c + ' all'):
                    cell += ' (' + ', '.join('%.0f' % x for x in rows[app][c + ' all']) + ')'
                cells.append(cell)
            out.append(f'| {app} | ' + ' | '.join(cells) + ' |')
    allv = sorted({v for r in rows.values() for v in r['speeds']}, key=lambda v: (abs(v), -v))
    out += ['', 'rest cpu / rest main: the median, then each round\'s value.']
    out += ['', '| fling fps by speed (pt/s) | ' + ' | '.join(str(v) for v in allv) + ' |', '|---' * (len(allv) + 1) + '|']
    for app in order:
        if app in rows:
            out.append(f'| {app} | ' + ' | '.join('%.1f' % rows[app]['speeds'].get(v, nan) for v in allv) + ' |')
    return out


def kind_of(key):
    """The scenario a table metric is measured in (for the thermal rule)."""
    if key in ('live fps',): return 'fling-live'
    if key in ('blank',): return 'fling-b'
    if key in ('rest cpu', 'rest main'): return 'rest'
    if key in ('ladder n',): return 'ladder-t'
    if key in ('jump p50',): return 'jump-layer'
    if key in ('cold',): return 'coldstart'
    if key in ('inner blanks',): return 'innerfling-b'
    if key in ('keep anchor', 'keep raw'): return 'innerkeep'
    if key.startswith(('filmstrip', 'inbox', 'inner ')): return 'innerfling-t'
    return 'fling-t'


def thermal_split(metrics, rows, apps_cmp):
    """-> (metrics that may be ranked, [(label, {app: state})] not ranked: the compared apps' worst thermal states in
    that metric's scenario differ. A state nobody recorded (older probe) is unknown, not different.)"""
    ok, no = [], []
    for m in metrics:
        k = kind_of(m[1])
        st = {a: rows[a]['thermal'].get(k, (None,))[0] for a in apps_cmp if a in rows}
        rec = {v for v in st.values() if v is not None}
        (no if len(rec) > 1 else ok).append(m if len(rec) <= 1 else (m[0], st))
    return ok, no


def thermal_table(rows, order):
    kinds = [k for k in ALL_KINDS if any(k in rows[a]['thermal'] for a in order if a in rows)]
    out = ['', '| thermal state, worst over the rounds (0 nominal, 1 fair, 2 serious, 3 critical; — = the probe recorded none; n/m = rounds that recorded it; LP = Low Power Mode seen) | ' + ' | '.join(kinds) + ' |', '|---' * (len(kinds) + 1) + '|']
    for a in order:
        if a not in rows:
            continue
        cells = []
        for k in kinds:
            t = rows[a]['thermal'].get(k)
            if not t:
                cells.append(''); continue
            w, n, m, lp = t
            cells.append(('—' if w is None else str(w)) + (f' ({n}/{m})' if 0 < n < m else '') + (' LP' if lp else ''))
        out.append(f'| {a} | ' + ' | '.join(cells) + ' |')
    return out


def verdicts(metrics, ex, others):
    """metrics: [(label, key, higher_is_better, fmt)], ex: exact2's values, others: {app: values}.
    -> (losses, wins, ties); a loss/win is (ratio, text); ratio >= 1 is how many times worse/better."""
    losses, wins, ties = [], [], []
    for label, key, higher, f in metrics:
        e = ex.get(key)
        comp = {a: o.get(key) for a, o in others.items() if isnum(o.get(key))}
        if not isnum(e) or not comp:
            continue
        best_app = (max if higher else min)(comp, key=lambda a: comp[a])
        b = comp[best_app]
        rest = ', '.join(f'{a} {fmt(v, f)}' for a, v in comp.items())
        if e == b or (e and b and abs(e / b - 1) < 0.005):  # equal, or within 0.5%
            ties.append(f'{label} {fmt(e, f)} ({rest})')
            continue
        worse = (e < b) if higher else (e > b)
        if worse:
            num, den = (b, e) if higher else (e, b)
            ratio = num / den if den else math.inf
            rs = f'{ratio:.2f}×' if ratio != math.inf else 'from zero'
            losses.append((ratio, f'**{label}**: exact2 {fmt(e, f)} vs {best_app} {fmt(b, f)} — {rs} ({rest})'))
        else:
            # better than the better of the two: the margin over the best competitor
            num, den = (e, b) if higher else (b, e)
            ratio = num / den if den else math.inf
            rs = f'{ratio:.2f}×' if ratio != math.inf else 'to zero'
            wins.append((ratio, f'{label}: exact2 {fmt(e, f)} vs {best_app} {fmt(b, f)} — {rs} ({rest})'))
    return losses, wins, ties


def verdict_lines(losses, wins, ties):
    out = ['', '**exact2 worse than the better of UIKit and SwiftUI** (ratio = how many times worse; *noise* marks a gap under 3%):', '']
    out += [f"- {t}{' *(noise)*' if r < 1.03 else ''}" for r, t in sorted(losses, key=lambda x: -x[0])] or ['- none']
    out += ['', '**exact2 better than both:**', '']
    out += [f"- {t}{' *(noise)*' if r < 1.03 else ''}" for r, t in sorted(wins, key=lambda x: -x[0])] or ['- none']
    if ties:
        out += ['', '**Equal to the better one (within 0.5%):** ' + '; '.join(ties)]
    return out


def list_metrics(rows, ex):
    m = [(NAMES[c], c, c in HIGHER, f) for c, f in COLS if c != 'ladder']
    m.append(('ladder (first speed under 110 fps, median; inf = never)', 'ladder n', True, '%.0f'))
    allv = sorted({v for r in rows.values() for v in r['speeds']}, key=lambda v: (abs(v), -v))
    flat = {a: dict(r, **{f'sp{v}': r['speeds'].get(v, nan) for v in allv}) for a, r in rows.items()}
    m += [(f'fling fps at {v} pt/s', f'sp{v}', True, '%.1f') for v in allv]
    return m, flat


ALL_LOSSES = collections.defaultdict(list)  # device -> [(ratio, bench, text)]


def order_note(d, apps, first='fling-t', extra=()):
    """The app order each round actually ran, from the result files' times (PROGRAM.md rule 4: the order rotates).
    extra: [(label, dir, app)] rows whose results live in another series dir (exact2's tip build)."""
    out = []
    for r in (1, 2, 3):
        ts = []
        for label, dd, a in [(a, d, a) for a in apps] + list(extra):
            f = f'{dd}/{a}-{first}-{r}.json'
            if os.path.exists(f):
                ts.append((os.path.getmtime(f), label))
        if ts:
            out.append(f'round {r}: ' + ', '.join(a for _, a in sorted(ts)))
    return 'Order run: ' + '; '.join(out) + '.' if out else ''


def rounds_note(rows, want=3):
    short = [f"{a} {k} {n}/{want}" for a, r in rows.items() for k, n in r['rounds'].items() if n != want]
    return ('Rounds short of three: ' + ', '.join(short) + '.') if short else 'Three rounds of every scenario for every app.'


def check_builds(rows, expect, where):
    for a, r in rows.items():
        if expect.get(a) and r['builds'] != [expect[a]]:
            problems.append(f'{show(where)}: {a} results carry build stamps {r["builds"]}, expected {expect[a]}')


def inner_tables(sources):
    """sources: [(label, dir, app)] — a row per source, so a second exact2 build from another series dir can sit beside the first."""
    out = []
    apps = [l for l, _, _ in sources]
    inner = {l: (load(d, a, 'innerfling-t'), load(d, a, 'innerfling-b'), load(d, a, 'innerkeep')) for l, d, a in sources}
    vals = {a: {} for a in apps}
    if not any(v[0] for v in inner.values()):
        return out, vals
    for kind in ('filmstrip', 'inbox'):
        sp = {}
        for a, (t, _, _) in inner.items():
            acc = collections.defaultdict(list)
            for r in t:
                for seg, st in zip(r['segments'], r['segStats']):
                    if seg.get('warm') or seg.get('inner') != kind:
                        continue
                    acc[seg['v'] * seg['dir']].append(st['frames'] / st['sec'])
            sp[a] = {v: med(f) for v, f in acc.items()}
            for v, f in sp[a].items():
                vals[a][f'{kind}{v}'] = f
        vs = sorted({v for x in sp.values() for v in x}, key=lambda v: (abs(v), -v))
        out += ['', f'| innerfling {kind} fps | ' + ' | '.join(str(v) for v in vs) + ' |', '|---' * (len(vs) + 1) + '|']
        for a in apps:
            out.append(f'| {a} | ' + ' | '.join('%.1f' % sp[a].get(v, nan) for v in vs) + ' |')
        vals['_speeds_' + kind] = vs
    out += ['', '| app | inner busy/f | inner cpu ms/s | inner main ms/s | inner blanks (innerfling 4) | innerkeep kept (same item, same place ±1 pt) | shifts pt | raw offset kept | raw offset errors pt |',
            '|---|---|---|---|---|---|---|---|---|']
    for a, (t, b, k) in inner.items():
        t = [r for r in t if 'run' in r]
        busy = med([r['run']['busyMsPerFrame'] for r in t]); cpu = med([r['run']['cpuMsPerSec'] for r in t])
        main = med([r['run']['mainCpuMsPerSec'] for r in t])
        blanks = sum(sum(1 for x in r.get('blank', []) if x and x > 0) for r in b)
        keeps = [e for r in k for e in r.get('keep', [])]
        anch = [e for e in keeps if 'keptAnchor' in e]
        ka = f"{sum(1 for e in anch if e['keptAnchor'])}/{len(anch)}" if anch else 'n/a'
        sh = ','.join(('?' if e.get('anchorMatch', 0) < 0.9 else str(e['anchorShiftPt'])) for e in anch if not e['keptAnchor']) or '-'
        kept = sum(1 for e in keeps if e.get('kept'))
        errs = ','.join(str(round(e['err'])) for e in keeps if not e.get('kept') and e.get('err') is not None) or '-'
        out.append(f'| {a} | {busy:.2f} | {cpu:.0f} | {main:.0f} | {blanks} | {ka} | {sh} | {kept}/{len(keeps)} | {errs} |')
        vals[a].update({'inner busy': busy, 'inner cpu': cpu, 'inner main': main, 'inner blanks': blanks if b else nan,
                        'keep anchor': (sum(1 for e in anch if e['keptAnchor']) / len(anch)) if anch else nan,
                        'keep raw': (kept / len(keeps)) if keeps else nan,
                        'rounds': (len(t), len(b), len(k))})
    return out, vals


def list_section(title, d, apps, exname, expect, dev, devname, bench, prior=None, extra=()):
    """extra: [(label, dir, app, expected stamp)] — further exact2 builds rerun alone (their SwiftUI/UIKit rows are this
    table's); the verdicts are against the LAST build listed (the newest code), and a delta table follows."""
    out = [f'### {title} — {devname}', '']
    if not os.path.isdir(d):
        return out + [f'Not run (`{d}` missing).', '']
    rows = list_rows(d, apps)
    if exname not in rows or len(rows) < 2:
        return out + [f'Incomplete: results for {sorted(rows)} only in `{d}`.', '']
    check_builds(rows, expect, d)
    base_ex = exname; labels = list(apps); srcs = [(a, d, a) for a in apps if a in rows]
    for label, xd, xa, xstamp in extra:
        xr = list_rows(xd, [xa]) if os.path.isdir(xd) else {}
        if xa in xr:
            check_builds({xa: xr[xa]}, {xa: xstamp}, xd)
            rows[label] = xr[xa]; labels.append(label); srcs.append((label, xd, xa)); exname = label
            out.insert(1, f'`{label}`: exact2 rerun alone at that commit (`{show(xd)}`, {rounds_note({xa: xr[xa]})}); '
                          f'the SwiftUI and UIKit rows are this table\'s. The verdicts below are against this newest build; the delta table is the day\'s landings.')
    out += [f'`{show(d)}` · {rounds_note({a: rows[a] for a in apps if a in rows})} {order_note(d, [a for a in apps if a in rows], extra=[x for x in srcs if x[0] not in apps])}', '']
    out += table(rows, labels)
    out += thermal_table(rows, labels)
    m, flat = list_metrics(rows, rows[exname])
    others = {a: flat[a] for a in rows if a not in (exname, base_ex)} if exname != base_ex else {a: flat[a] for a in rows if a != exname}
    m_rank, unranked = thermal_split(m, rows, [exname] + list(others))
    losses, wins, ties = verdicts(m_rank, flat[exname], others)
    inner = []
    if bench == 'xheavy':
        inner, iv = inner_tables(srcs)
        if inner:
            im = []
            for kind in ('filmstrip', 'inbox'):
                im += [(f'{kind} inner fling fps at {v} pt/s', f'{kind}{v}', True, '%.1f') for v in iv.get('_speeds_' + kind, [])]
            im += [('inner busy/f', 'inner busy', False, '%.2f'), ('inner cpu ms/s', 'inner cpu', False, '%.0f'),
                   ('inner main ms/s', 'inner main', False, '%.0f'), ('inner blanks', 'inner blanks', False, '%d'),
                   ('innerkeep kept (anchor), share', 'keep anchor', True, '%.2f')]
            # 'raw offset kept' stays in the table, unranked: it is not a loss (see the note under the table)
            im_rank, un2 = thermal_split(im, rows, [exname] + list(others))
            unranked += un2
            l2, w2, t2 = verdicts(im_rank, iv[exname], {a: iv[a] for a in rows if a not in (exname, base_ex) or (a == base_ex and exname == base_ex)})
            losses += l2; wins += w2; ties += t2
            if exname != base_ex:
                flat[base_ex].update(iv[base_ex]); flat[exname].update(iv[exname]); m = m + im
            for a in rows:
                if iv[a].get('rounds') and iv[a]['rounds'] != (3, 3, 3):
                    out.insert(3, f'Inner-list rounds for {a} (innerfling-t, innerfling-b, innerkeep): {iv[a]["rounds"]}.')
    out += inner
    if inner:
        out += ['', "innerkeep's primary column is `kept (same item, same place ±1 pt)`, the probe's `keptAnchor`: exact2 keeps both "
                "inner lists 12/12. `raw offset kept` is in the table but not ranked as a loss: exact2's raw offset moves because "
                "rows above the anchor are measured after the jump (scroll anchoring, LLP 1070 H4/N2), while UIKit's app hands "
                "its layout exact heights for all 1,000 messages, so its offset space never changes (the inner lane)."]
    jt = {a: r['jump timeouts'] for a, r in rows.items() if r['jump timeouts']}
    if jt:
        out += ['', 'Jump targets that timed out (not in the p50): ' + ', '.join(f'{a} {n}' for a, n in jt.items()) + '.']
    out += verdict_lines(losses, wins, ties)
    if unranked:
        out += ['', '**Not ranked — the compared apps ran at different thermal states in that scenario** (a throttled phone is not a slower app): '
                + '; '.join(f"{lab} ({', '.join(f'{a} {THERMAL_NAMES.get(v, v) if v is not None else chr(8212)}' for a, v in st.items())})" for lab, st in unranked)]
    ALL_LOSSES[dev] += [(r, title, t) for r, t in losses]
    if exname != base_ex:
        out += delta_table(m, flat[base_ex], flat[exname], base_ex, exname)
    if prior and os.path.isdir(prior[1]):
        out += moved(rows, list_rows(prior[1], apps), apps, base_ex, prior[0])
    return out + ['']


def delta_table(metrics, a, b, la, lb):
    """Every metric of two exact2 builds side by side with the change, higher-is-better metrics marked so a reader
    sees the sign right; ±3% is noise."""
    out = ['', f'**{la} → {lb}** (the day\'s landings as one delta; a change under 3% is noise; ↑ = higher is better):', '',
           f'| metric | {la} | {lb} | change |', '|---|---|---|---|']
    for label, key, higher, f in metrics:
        x, y = a.get(key), b.get(key)
        if not isnum(x) or not isnum(y):
            continue
        if x == 0 and y == 0:
            ch = '0 → 0'
        elif x == 0 or x == math.inf or y == math.inf:
            ch = f'{fmt(x, f)} → {fmt(y, f)}'
        else:
            pct = (y / x - 1) * 100
            better = (pct > 0) == higher
            ch = f'{pct:+.0f}%' + ('' if abs(pct) < 3 else (' better' if better else ' worse'))
        out.append(f"| {label}{' ↑' if higher else ''} | {fmt(x, f)} | {fmt(y, f)} | {ch} |")
    return out


def moved(now, old, apps, exname, label):
    """Metrics that moved by more than 10% against an earlier series, for every app, so a change that also shows in
    the unchanged SwiftUI/UIKit bundles (the device: leftover processes, thermal state) is told from an exact2 one."""
    out = ['', f'**Against {label}** (metrics where any app moved more than 10%; the SwiftUI and UIKit bundles are the same builds in both series, so their movement is the device, not the code):', '',
           '| metric | ' + ' | '.join(f'{a} {label} → now' for a in apps if a in now and a in old) + ' | reading |', '|---' * (len([a for a in apps if a in now and a in old]) + 2) + '|']
    n = 0
    for c, f in COLS:
        if c == 'ladder':
            continue
        cells, mv = [], {}
        for a in apps:
            if a not in now or a not in old:
                continue
            o, v = old[a].get(c), now[a].get(c)
            if isnum(o) and isnum(v) and o:
                mv[a] = v / o
                cells.append(f'{fmt(o, f)} → {fmt(v, f)} ({(v / o - 1) * 100:+.0f}%)')
            elif isnum(o) and isnum(v):
                mv[a] = 1.0 if v == o else math.inf
                cells.append(f'{fmt(o, f)} → {fmt(v, f)}')
            else:
                cells.append('—')
        if not any(abs(r - 1) > 0.10 for r in mv.values()):
            continue
        comp = [r for a, r in mv.items() if a != exname]
        e = mv.get(exname)
        if e is None:
            reading = ''
        elif abs(e - 1) <= 0.10:
            reading = 'exact2 steady; the competitors moved (device or run-to-run)'
        elif comp and all(abs(r - 1) > 0.10 and (r > 1) == (e > 1) for r in comp):
            reading = 'all three moved the same way: the device'
        elif comp and all(abs(r - 1) <= 0.10 for r in comp):
            reading = f'only exact2 moved: the code since {label}'
        else:
            reading = 'mixed: part device, part code'
        out.append(f'| {NAMES[c]} | ' + ' | '.join(cells) + f' | {reading} |')
        n += 1
    if not n:
        out = ['', f'**Against {label}:** no table metric of any app moved by more than 10%.']
    return out


def main():
    body = []
    body += ['## 1. Extra Heavy feed, 19 kinds', '']
    xa = APPS['xheavy']
    for dev, name in DEVS:
        extra = [(f'exact2 @{TIP_REV}', f'{A.xheavy}/{TIP_TAG}-19-{dev}', xa[-1], stamps('xheavy', TIP_REV)[xa[-1]])] if TIP_TAG else []
        body += list_section('Extra Heavy feed (19 kinds)', f'{A.xheavy}/{TAG}-19-{dev}', xa, xa[-1], stamps('xheavy', REV),
                             dev, name, 'xheavy', prior=(A.prior, f'{A.xheavy}/{A.prior}-19-{dev}') if A.prior else None, extra=extra)
    body += ['## 2. Crypto list', '', 'exact2 is `exact-svgi` (the SVG app: rows hold the series, the view maps it to points).', '']
    for dev, name in DEVS:
        body += list_section('Crypto list', f'{A.crypto}/{dev}-{TAG}', APPS['crypto'], APPS['crypto'][-1],
                             stamps('crypto', REV), dev, name, 'crypto')
    body += ['## 3. Plain heavy list', '']
    for dev, name in DEVS:
        body += list_section('Plain heavy list', f'{A.heavy}/{dev}-{TAG}', APPS['heavy'], APPS['heavy'][-1],
                             stamps('heavy', REV), dev, name, 'heavy')
    notes = open(A.notes).read().rstrip().split('\n') if A.notes else []
    head = open(A.head).read().rstrip().split('\n') if A.head else [f'# exact2 scoreboard — {TAG} ({REV}) against UIKit and SwiftUI', '',
                                                                      'Written by `bench/board/board.py` from the results on disk.']
    summ = ['## Losses ranked by ratio', '', 'exact2 against the better of UIKit and SwiftUI. Gaps under 3% are left out here as noise; they are listed under each table.', '']
    for dev, name in DEVS:
        ls = sorted([x for x in ALL_LOSSES[dev] if x[0] >= 1.03], key=lambda x: -x[0])
        summ += [f'### {name}', ''] + ([f'{i + 1}. [{b}] {t}' for i, (r, b, t) in enumerate(ls)] or ['Nothing measured yet.']) + ['']
    prob = (['## Problems found while building this board', ''] + [f'- {p}' for p in problems] + ['']) if problems else []
    # a competitor bundle with no --expect is not checked (an unstamped one carries no BenchBuild)
    text = '\n'.join(head + [''] + prob + summ + notes + [''] + body) + '\n'
    if A.stdout:
        sys.stdout.write(text)
    else:
        os.makedirs(os.path.dirname(os.path.abspath(A.out)), exist_ok=True)
        open(A.out, 'w').write(text)
        print(f'wrote {A.out} ({len(text.splitlines())} lines); problems: {len(problems)}')
    if A.summary:
        print('\n'.join(summ))


main()
