#!/usr/bin/env python3
"""tptree.py <tp.xml> <skip_s> <dur_s> [--thread RE] [--root RE] [--min MS] [--depth N] [--flat] [--threads]
Time Profiler export -> per-thread ms/s, and for the chosen threads (default: Main Thread) a root-first call tree
of inclusive ms/s (children under --min pruned). --root RE starts each sample's stack at the outermost frame
matching RE (samples without one are dropped). --flat prints inclusive ms/s per symbol instead of the tree.
The window is [first sample + skip, + dur]."""
import xml.etree.ElementTree as ET, sys, re, collections, argparse
ap = argparse.ArgumentParser()
ap.add_argument('path'); ap.add_argument('skip', type=float); ap.add_argument('dur', type=float)
ap.add_argument('--thread', default='^Main Thread'); ap.add_argument('--root'); ap.add_argument('--min', type=float, default=1.0)
ap.add_argument('--depth', type=int, default=40); ap.add_argument('--flat', action='store_true'); ap.add_argument('--threads', action='store_true')
ap.add_argument('--self', dest='selfonly', action='store_true', help='flat by leaf (self) time')
a = ap.parse_args()
ids = {}; rows = []
for ev, e in ET.iterparse(a.path, events=('end',)):
    i = e.get('id')
    if i: ids[i] = e
    if e.tag == 'row': rows.append(e)
res = lambda e: ids.get(e.get('ref'), e) if e is not None and e.get('ref') else e
S = []
for row in rows:
    th = res(row.find('thread')); tn = (th.get('fmt') or '?') if th is not None else '?'
    t = int(res(row.find('sample-time')).text) / 1e9
    w = int(res(row.find('weight')).text) / 1e6 if row.find('weight') is not None else 1.0
    bt = row.find('tagged-backtrace')
    if bt is None: bt = row.find('backtrace')
    fr = [res(f).get('name') or '?' for f in res(bt).iter('frame')] if bt is not None else []
    S.append((t, tn, w, fr))
t0 = min(s[0] for s in S) + a.skip; t1 = t0 + a.dur
S = [s for s in S if t0 <= s[0] < t1]
thr = collections.Counter()
for t, tn, w, fr in S: thr[re.sub(r' \(0x[0-9a-f]+\).*', '', tn) if a.threads else tn.split(' (')[0] + ' ' + (re.search(r'0x[0-9a-f]+', tn) or re.match('', '')).group(0)] += w
print(f'window {a.dur}s; total {sum(thr.values())/a.dur:.1f} ms/s')
for k, v in thr.most_common(14 if not a.threads else 30): print(f'  {v/a.dur:7.1f}  {k}')
if a.threads: sys.exit()
tre = re.compile(a.thread); rre = re.compile(a.root) if a.root else None
def short(n): return n if len(n) <= 110 else n[:107] + '...'
tree = lambda: [0.0, collections.defaultdict(tree)]
root = tree(); flat = collections.Counter(); selft = collections.Counter(); tot = 0.0
for t, tn, w, fr in S:
    if not tre.search(tn): continue
    st = fr[::-1]
    if rre:
        k = next((i for i, f in enumerate(st) if rre.search(f)), None)
        if k is None: continue
        st = st[k:]
    tot += w; node = root; node[0] += w
    for f in st[:a.depth]:
        node = node[1][f]; node[0] += w
    for f in set(st): flat[f] += w
    if st: selft[st[-1]] += w
print(f'threads /{a.thread}/' + (f' under /{a.root}/' if a.root else '') + f': {tot/a.dur:.1f} ms/s')
if a.flat or a.selfonly:
    for f, w in (selft if a.selfonly else flat).most_common(60):
        if w / a.dur >= a.min: print(f'{w/a.dur:7.1f}  {short(f)}')
    sys.exit()
def show(node, depth):
    kids = sorted(node[1].items(), key=lambda kv: -kv[1][0])
    for name, k in kids:
        if k[0] / a.dur < a.min: continue
        print(f'{k[0]/a.dur:7.1f}  ' + '  ' * depth + short(name))
        show(k, depth + 1)
show(root, 0)
