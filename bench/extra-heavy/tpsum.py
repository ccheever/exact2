#!/usr/bin/env python3
"""tpsum.py <time-profile.xml> <t0> <t1> — CPU ms/s per thread, top leaf and inclusive frames, in a window."""
import sys, collections, xml.etree.ElementTree as ET
t0, t1 = float(sys.argv[2]), float(sys.argv[3])
root = ET.parse(sys.argv[1]).getroot(); ids = {}
for e in root.iter():
    if e.get('id'): ids[e.get('id')] = e
R = lambda e: ids[e.get('ref')] if e.get('ref') else e
thr = collections.Counter(); incl = collections.defaultdict(collections.Counter)
for row in root.iter('row'):
    ch = list(row)
    tm = int(R(ch[0]).text) / 1e9
    if not (t0 <= tm <= t1) or len(ch) < 7: continue
    th = R(ch[1]).get('fmt').split(' (0x')[0]; w = int(R(ch[5]).text) / 1e6
    thr[th] += w
    for f in set(R(f).get('name') or '?' for f in R(ch[6]).iter('frame')): incl[th][f[:100]] += w
span = t1 - t0
for th, w in thr.most_common(6): print(f'{w/span:7.0f} ms/s  {th}')
pat = sys.argv[4] if len(sys.argv) > 4 else None
for th in [t for t, _ in thr.most_common(2)]:
    print('--', th)
    for f, w in incl[th].most_common(200):
        if pat is None or any(p.lower() in f.lower() for p in pat.split('|')):
            print(f'{w/span:7.1f} ms/s  {f}')
