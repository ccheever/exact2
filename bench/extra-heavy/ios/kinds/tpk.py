#!/usr/bin/env python3
"""tpk.py <tp.xml> [skip_s=16.5] [dur_s=28] [focus…] — Time Profiler export → ms/s in the fling window.
Window: [first sample of the app + skip, + dur]. Prints per-thread ms/s, per-binary self ms/s, top inclusive and
self symbols (all threads and main), and for each focus substring its callees (main+all)."""
import xml.etree.ElementTree as ET, collections, sys, re
path = sys.argv[1]; skip = float(sys.argv[2]) if len(sys.argv) > 2 else 16.5; dur = float(sys.argv[3]) if len(sys.argv) > 3 else 28
focus = sys.argv[4:]
ids = {}
rows = []
for ev, e in ET.iterparse(path, events=('end',)):
    i = e.get('id')
    if i: ids[i] = e
    if e.tag == 'row': rows.append(e)
res = lambda e: ids.get(e.get('ref'), e) if e is not None and e.get('ref') else e
samples = []
for row in rows:
    th = res(row.find('thread')); tname = th.get('fmt') or '?'
    proc = res(row.find('process')); pname = proc.get('fmt') if proc is not None else ''
    t = int(res(row.find('sample-time')).text) / 1e9
    w = int(res(row.find('weight')).text) / 1e6 if row.find('weight') is not None else 1.0
    bt = row.find('tagged-backtrace')
    if bt is None: bt = row.find('backtrace')
    frames = []
    if bt is not None:
        bt = res(bt)
        for f in bt.iter('frame'):
            f = res(f); b = f.find('binary'); b = res(b)
            frames.append((f.get('name') or '?', b.get('name') if b is not None else '?'))
    samples.append((t, tname, w, frames))
t0 = min(s[0] for s in samples) + skip; t1 = t0 + dur
win = [s for s in samples if t0 <= s[0] <= t1]
secs = dur
def tclass(n):
    n = re.sub(r'\(0x[0-9a-f]+\).*', '', n).strip()
    return re.sub(r'\d+', '#', n) or '?'
per_thread = collections.Counter(); per_bin = collections.Counter()
incl = collections.Counter(); selfc = collections.Counter(); mincl = collections.Counter(); mself = collections.Counter()
for t, tn, w, fr in win:
    main = 'Main Thread' in tn
    per_thread[tclass(tn)] += w
    if not fr: continue
    per_bin[fr[0][1]] += w; selfc[fr[0][0]] += w
    if main: mself[fr[0][0]] += w
    for n in set(x[0] for x in fr):
        incl[n] += w
        if main: mincl[n] += w
ms = lambda c: c / secs
tot = sum(per_thread.values())
print(f'window {secs:.0f}s, total {ms(tot):.0f} ms/s')
print('--- threads (ms/s)')
for n, c in per_thread.most_common(14): print(f'{ms(c):7.1f} {n[:90]}')
print('--- self by binary (ms/s)')
for n, c in per_bin.most_common(14): print(f'{ms(c):7.1f} {n}')
def top(title, c, k=45):
    print('---', title)
    for n, v in c.most_common(k): print(f'{ms(v):7.1f} {n[:160]}')
top('inclusive, all threads', incl); top('self, all threads', selfc, 30)
top('inclusive, main', mincl); top('self, main', mself, 30)
for fcs in focus:
    for label, mainonly in (('main', True), ('all', False)):
        ch = collections.Counter(); n = 0
        for t, tn, w, fr in win:
            if mainonly and 'Main Thread' not in tn: continue
            names = [x[0] for x in fr]
            for i, f in enumerate(names):
                if fcs in f:
                    n += w; ch[names[i - 1] if i > 0 else '<self>'] += w; break
        print(f'--- callees of {fcs} ({label}): {ms(n):.1f} ms/s')
        for k, v in ch.most_common(15): print(f'{ms(v):7.1f} {k[:150]}')
