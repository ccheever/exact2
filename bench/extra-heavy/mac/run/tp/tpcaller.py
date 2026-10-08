import xml.etree.ElementTree as ET, collections, sys
root=ET.parse('/tmp/tp.xml').getroot()
ids={}
def reg(e):
    i=e.get('id')
    if i: ids[i]=e
    for c in e: reg(c)
reg(root)
def res(e): return ids.get(e.get('ref'), e) if e.get('ref') else e
for target in sys.argv[1:]:
    ch=collections.Counter()
    for row in root.iter('row'):
        th=res(row.find('thread'))
        if 'Main Thread' not in (th.get('fmt') or ''): continue
        bt=row.find('tagged-backtrace') or row.find('backtrace')
        if bt is None: continue
        bt=res(bt); b=bt.find('backtrace'); b=res(b) if b is not None else bt
        frames=[res(f).get('name') or '?' for f in b.iter('frame')]
        for i,f in enumerate(frames):
            if target in f:
                ch[' <- '.join(x[:60] for x in frames[i+1:i+4])]+=1; break
    print('===',target)
    for k,c in ch.most_common(6): print(f"  {c:5d} {k}")
