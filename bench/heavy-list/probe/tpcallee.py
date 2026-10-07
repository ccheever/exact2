#!/usr/bin/env python3
"""tpcallee.py <tp.xml> <name>… — for each function whose name contains <name>, what it calls on the main thread
(the frame below it in each sample)."""
import xml.etree.ElementTree as ET, collections, sys
root=ET.parse(sys.argv[1]).getroot()
ids={}
def reg(e):
    i=e.get('id')
    if i: ids[i]=e
    for c in e: reg(c)
reg(root)
def res(e): return ids.get(e.get('ref'), e) if e.get('ref') else e
targets=sys.argv[2:]
for target in targets:
    callee=collections.Counter(); n=0
    for row in root.iter('row'):
        th=res(row.find('thread'))
        if 'Main Thread' not in (th.get('fmt') or ''): continue
        bt=row.find('tagged-backtrace') or row.find('backtrace')
        if bt is None: continue
        bt=res(bt); b=bt.find('backtrace'); b=res(b) if b is not None else bt
        frames=[res(f).get('name') or '?' for f in b.iter('frame')]
        for i,f in enumerate(frames):
            if target in f:
                n+=1; callee[frames[i-1] if i>0 else '<self>']+=1; break
    print(f'=== {target}: {n}')
    for k,c in callee.most_common(14): print(f"  {c:5d} {k[:140]}")
