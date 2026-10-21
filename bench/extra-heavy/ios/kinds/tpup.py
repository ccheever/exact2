#!/usr/bin/env python3
"""tpup.py <tp.xml> <substr> [main|all] [depth=8] — caller chains of a symbol (top stacks) in the 16.5 s+28 s window."""
import xml.etree.ElementTree as ET, collections, sys
path, target = sys.argv[1], sys.argv[2]; which = sys.argv[3] if len(sys.argv) > 3 else 'all'; depth = int(sys.argv[4]) if len(sys.argv) > 4 else 8
ids = {}; rows = []
for ev, e in ET.iterparse(path, events=('end',)):
    i = e.get('id')
    if i: ids[i] = e
    if e.tag == 'row': rows.append(e)
res = lambda e: ids.get(e.get('ref'), e) if e is not None and e.get('ref') else e
S = []
for row in rows:
    th = res(row.find('thread')).get('fmt') or ''
    t = int(res(row.find('sample-time')).text) / 1e9
    bt = row.find('tagged-backtrace')
    if bt is None: continue
    S.append((t, th, [res(f).get('name') or '?' for f in res(bt).iter('frame')]))
t0 = min(s[0] for s in S) + 16.5
c = collections.Counter()
for t, th, fr in S:
    if not (t0 <= t <= t0 + 28): continue
    if which == 'main' and 'Main Thread' not in th: continue
    for i, f in enumerate(fr):
        if target in f:
            c[' < '.join(x[:60] for x in fr[i:i + depth])] += 1; break
for k, v in c.most_common(12): print(f'{v/28:6.1f} {k}\n')
