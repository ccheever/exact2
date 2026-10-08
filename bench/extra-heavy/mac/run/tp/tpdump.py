"""tpdump.py <tp.xml> <out.json.gz> — a Time Profiler export (xctrace export --xpath …time-profile) as a compact
file: {"names": [...], "threads": [...], "cores": [...], "samples": [[t_ns, thread_index, weight_ns, [name_index, … leaf
first], core_index], …]}; cores are the export's labels ("CPU 4 (P Core)"), so a sample says which kind of core ran it.
Every thread, not only main; the analysis (tpx.py) runs anywhere afterwards."""
import xml.etree.ElementTree as ET, sys, gzip, json

src, out = sys.argv[1], sys.argv[2]
ids = {}
names, nidx = [], {}
threads, tidx = [], {}
cores, cidx = [], {}
samples = []


def intern(table, index, s):
    i = index.get(s)
    if i is None:
        i = index[s] = len(table)
        table.append(s)
    return i


def res(e):
    r = e.get('ref')
    return ids[r] if r else e


for ev, e in ET.iterparse(src, events=('start', 'end')):
    if ev == 'start':
        i = e.get('id')
        if i:
            ids[i] = e
        continue
    if e.tag != 'row':
        continue
    th = res(e.find('thread'))
    st = e.find('sample-time')
    w = e.find('weight')
    co = e.find('core')
    bt = e.find('tagged-backtrace')
    if bt is None:
        bt = e.find('backtrace')
    if th is None or st is None or bt is None:
        continue
    bt = res(bt)
    b = bt.find('backtrace')
    b = res(b) if b is not None else bt
    frames = [intern(names, nidx, res(f).get('name') or '?') for f in b.iter('frame')]
    if not frames:
        continue
    t = int(res(st).text)
    wn = int(res(w).text) if w is not None else 1000000
    core = intern(cores, cidx, res(co).get('fmt') or '?') if co is not None else -1
    samples.append([t, intern(threads, tidx, th.get('fmt') or '?'), wn, frames, core])
with gzip.open(out, 'wt') as f:
    json.dump({'names': names, 'threads': threads, 'cores': cores, 'samples': samples}, f)
print('samples', len(samples), 'threads', len(threads), 'names', len(names))
