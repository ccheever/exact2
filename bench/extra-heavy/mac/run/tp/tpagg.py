import xml.etree.ElementTree as ET, collections, sys
root=ET.parse('/tmp/tp.xml').getroot()
ids={}
def reg(e):
    i=e.get('id')
    if i: ids[i]=e
    for c in e: reg(c)
reg(root)
def res(e): return ids.get(e.get('ref'), e) if e.get('ref') else e
incl=collections.Counter(); leaf=collections.Counter(); total=0
t0=float(sys.argv[1]) if len(sys.argv)>1 else 0; t1=float(sys.argv[2]) if len(sys.argv)>2 else 1e18
for row in root.iter('row'):
    th=res(row.find('thread'))
    if 'Main Thread' not in (th.get('fmt') or ''): continue
    t=int(res(row.find('sample-time')).text)/1e9
    if not (t0<=t<=t1): continue
    bt=row.find('tagged-backtrace') or row.find('backtrace')
    if bt is None: continue
    bt=res(bt)
    b=bt.find('backtrace'); b=res(b) if b is not None else bt
    frames=[]
    for f in b.iter('frame'):
        f=res(f); frames.append(f.get('name') or '?')
    if not frames: continue
    total+=1; leaf[frames[0]]+=1
    for n in set(frames): incl[n]+=1
print('main-thread samples',total)
print('--- inclusive top')
for n,c in incl.most_common(300): print(f"{c:5d} {100*c/total:5.1f}% {n[:150]}")
print('--- leaf top')
for n,c in leaf.most_common(25): print(f"{c:5d} {100*c/total:5.1f}% {n[:150]}")
