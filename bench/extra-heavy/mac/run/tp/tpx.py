#!/usr/bin/env python3
"""tpx.py <dump.json.gz> [cmd] [args] — analysis of a tpdump.py file.

  threads [t0 t1]                 per-thread CPU: ms and ms/s over the window (seconds since trace start)
  top <thread-substr> [n] [t0 t1] inclusive and leaf tops of the threads whose name contains the substring
  callee <thread-substr> <fn>     what <fn> calls (one level), by weight
  caller <thread-substr> <fn>     who calls <fn>
  buckets <thread-substr> <file>  exclusive partition: each sample goes to the first rule (substring of a frame,
                                  searched root-first … one rule per line "label<TAB>substr[|substr]") that matches
"""
import gzip, json, sys, collections

d = json.load(gzip.open(sys.argv[1]))
names, threads, samples = d['names'], d['threads'], d['samples']
cmd = sys.argv[2] if len(sys.argv) > 2 else 'threads'
T0 = min(s[0] for s in samples)
T1 = max(s[0] for s in samples)


def window(args):
    if len(args) >= 2:
        return float(args[0]), float(args[1])
    return 0.0, (T1 - T0) / 1e9


def short(t):
    # "Main Thread 0x1234 (XHeavy, pid: 1)" -> name without ids
    import re
    t = re.sub(r'0x[0-9a-f]+', '', t)
    t = re.sub(r'\(.*?pid: \d+\)', '', t)
    return ' '.join(t.split())


def sel(sub, t0, t1):
    for smp in samples:
        t, th, w, fr = smp[0], smp[1], smp[2], smp[3]
        ts = (t - T0) / 1e9
        if ts < t0 or ts > t1:
            continue
        # '*' every thread; '!x' every thread whose name lacks x; else those whose name contains it
        if sub == '*' or (sub[0] == '!' and sub[1:].lower() not in threads[th].lower()) or (sub[0] != '!' and sub.lower() in threads[th].lower()):
            yield th, w, fr


if cmd == 'segs':
    # segs <run.json> — the probe run's segments as windows of this trace. The fling starts at the first
    # main-thread sample inside the probe's scroll setter (setOff, under -[LBProbe tick:]); each segment lasts
    # its `sec` in the run's JSON. Prints "label t0 t1" (seconds since trace start) for top/buckets/threads.
    run = json.load(open(sys.argv[3]))
    probe = [i for i, n in enumerate(names) if 'setOff' in n]
    ps = set(probe)
    first = min((s_[0] for s_ in samples if 'Main Thread' in threads[s_[1]] and ps.intersection(s_[3])), default=None)
    last = max((s_[0] for s_ in samples if 'Main Thread' in threads[s_[1]] and ps.intersection(s_[3])), default=None)
    if first is None:
        print('no setOff sample: the trace missed the scroll'); sys.exit(1)
    t = (first - T0) / 1e9
    print(f'# scroll in trace {t:.2f}–{(last - T0) / 1e9:.2f} s; run sec {run["run"]["sec"]:.2f}; trace {((T1 - T0) / 1e9):.1f} s')
    for seg, st in zip(run['segments'], run['segStats']):
        label = ('warm' if seg.get('warm') else '') + str(seg['v'] * seg['dir'])
        print(f'{label} {t:.2f} {t + st["sec"]:.2f}  fps {st["frames"] / st["sec"]:.1f} busy/f {st["busyMsPerFrame"]:.1f} main {st["mainCpuMsPerSec"]:.0f}')
        t += st['sec']
    sys.exit(0)
if cmd == 'cores':
    # cores <thread-substr> [t0 t1] — which cores ran the thread's samples (P or E), by weight
    sub = sys.argv[3]
    t0, t1 = window(sys.argv[4:])
    cores = d.get('cores') or []
    per = collections.Counter(); kind = collections.Counter()
    for smp in samples:
        ts = (smp[0] - T0) / 1e9
        if ts < t0 or ts > t1 or len(smp) < 5 or sub.lower() not in threads[smp[1]].lower():
            continue
        label = cores[smp[4]] if smp[4] >= 0 else '?'
        per[label] += smp[2]; kind['E' if '(E' in label else 'P' if '(P' in label else '?'] += smp[2]
    tot = sum(per.values()) or 1
    print(f'{sub} {t0:.1f}–{t1:.1f} s: ' + ', '.join(f'{k} {100 * w / tot:.0f}% ({w / 1e6 / (t1 - t0):.0f} ms/s)' for k, w in kind.most_common()))
    sys.exit(0)
if cmd == 'threads':
    t0, t1 = window(sys.argv[3:])
    per = collections.Counter()
    for th, w, fr in sel('*', t0, t1):
        per[short(threads[th])] += w
    dur = t1 - t0
    tot = sum(per.values())
    print(f'window {t0:.1f}–{t1:.1f} s; all threads {tot/1e6/dur:.0f} ms/s')
    for n, w in per.most_common(40):
        print(f'{w/1e6/dur:8.1f} ms/s {100*w/tot:5.1f}%  {n}')
elif cmd == 'top':
    sub = sys.argv[3]
    n = int(sys.argv[4]) if len(sys.argv) > 4 else 80
    t0, t1 = window(sys.argv[5:])
    incl = collections.Counter(); leaf = collections.Counter(); tot = 0
    for th, w, fr in sel(sub, t0, t1):
        tot += w; leaf[fr[0]] += w
        for f in set(fr):
            incl[f] += w
    dur = t1 - t0
    print(f'{sub}: {tot/1e6/dur:.0f} ms/s')
    print('--- inclusive')
    for f, w in incl.most_common(n):
        print(f'{w/1e6/dur:7.1f} {100*w/tot:5.1f}% {names[f][:170]}')
    print('--- leaf')
    for f, w in leaf.most_common(40):
        print(f'{w/1e6/dur:7.1f} {100*w/tot:5.1f}% {names[f][:170]}')
elif cmd == 'topin':
    # topin <thread-substr> <fn-substr> [n] [t0 t1] — inclusive and leaf tops of only the samples whose stack has <fn>
    sub, target = sys.argv[3], sys.argv[4]
    n = int(sys.argv[5]) if len(sys.argv) > 5 else 50
    t0, t1 = window(sys.argv[6:])
    hits = {i for i, nm in enumerate(names) if target in nm}
    incl = collections.Counter(); leaf = collections.Counter(); tot = 0
    for th, w, fr in sel(sub, t0, t1):
        if not hits.intersection(fr):
            continue
        tot += w; leaf[fr[0]] += w
        for f in set(fr):
            incl[f] += w
    dur = t1 - t0
    print(f'{sub} within {target}: {tot/1e6/dur:.1f} ms/s')
    for f, w in incl.most_common(n):
        print(f'{w/1e6/dur:7.1f} {100*w/max(tot,1):5.1f}% {names[f][:170]}')
    print('--- leaf')
    for f, w in leaf.most_common(15):
        print(f'{w/1e6/dur:7.1f} {100*w/max(tot,1):5.1f}% {names[f][:170]}')
elif cmd in ('callee', 'caller'):
    sub, target = sys.argv[3], sys.argv[4]
    t0, t1 = window(sys.argv[5:])
    out = collections.Counter(); tot = 0
    for th, w, fr in sel(sub, t0, t1):
        # frames are leaf first; the outermost match for callee, the innermost for caller
        idx = [i for i, f in enumerate(fr) if target in names[f]]
        if not idx:
            continue
        tot += w
        if cmd == 'callee':
            i = idx[0]
            out[names[fr[i - 1]] if i > 0 else '<self>'] += w
        else:
            i = idx[-1]
            out[names[fr[i + 1]] if i + 1 < len(fr) else '<root>'] += w
    dur = t1 - t0
    print(f'=== {cmd} of {target}: {tot/1e6/dur:.1f} ms/s')
    for k, w in out.most_common(25):
        print(f'  {w/1e6/dur:7.1f} {k[:170]}')
elif cmd == 'buckets':
    sub, rules_file = sys.argv[3], sys.argv[4]
    t0, t1 = window(sys.argv[5:])
    rules = []
    for line in open(rules_file):
        line = line.rstrip('\n')
        if not line or line.startswith('#'):
            continue
        label, pats = line.split('\t')
        rules.append((label, pats.split('|')))
    hit = {}
    for i, n in enumerate(names):
        for ri, (label, pats) in enumerate(rules):
            if any(p in n for p in pats):
                hit.setdefault(i, []).append(ri)
    out = collections.Counter(); tot = 0
    rest = collections.Counter()
    for th, w, fr in sel(sub, t0, t1):
        tot += w
        best = None
        for f in fr:
            for ri in hit.get(f, ()):
                if best is None or ri < best:
                    best = ri
        if best is None:
            out['(other)'] += w
            # the outermost app-ish frame, to see what is unclassified
            rest[names[fr[min(len(fr) - 1, 6)]] if len(fr) > 6 else names[fr[-1]]] += w
        else:
            out[rules[best][0]] += w
    dur = t1 - t0
    print(f'{sub}: {tot/1e6/dur:.0f} ms/s')
    for label, w in out.most_common():
        print(f'{w/1e6/dur:7.1f} ms/s {100*w/tot:5.1f}%  {label}')
    if '--rest' in sys.argv:
        print('--- unclassified by a deep frame')
        for k, w in rest.most_common(30):
            print(f'  {w/1e6/dur:7.1f} {k[:150]}')
