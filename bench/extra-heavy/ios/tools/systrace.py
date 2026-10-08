#!/usr/bin/env python3
"""systrace.py <run.trace> <qos|hangs|blocked|threads> [process-substring] [thread-substring] [--min ms]

Reads a System Trace by exporting its tables with xctrace:
  qos      each thread's requested/effective QoS intervals (ThreadQoSTable), summed by (thread, requested, effective)
  hangs    potential-hangs: the main thread's hangs with their durations
  blocked  thread-state: a thread's longest non-running intervals and what they waited on (default: the main thread)
  threads  thread-info: thread ids and names of the process
"""
import subprocess, sys, os, re, collections
import xml.etree.ElementTree as ET

trace = sys.argv[1]; what = sys.argv[2]
proc = sys.argv[3] if len(sys.argv) > 3 and not sys.argv[3].startswith('--') else ''
thr = sys.argv[4] if len(sys.argv) > 4 and not sys.argv[4].startswith('--') else ''
min_ms = float(sys.argv[sys.argv.index('--min') + 1]) if '--min' in sys.argv else 10.0

def export(schema):
    out = os.path.join(os.path.dirname(trace) or '.', f'.{schema}.xml')
    if not os.path.exists(out) or os.path.getsize(out) == 0:
        with open(out, 'w') as f:
            subprocess.run(['xcrun', 'xctrace', 'export', '--input', trace, '--xpath',
                            f'/trace-toc/run[@number="1"]/data/table[@schema="{schema}"]'], stdout=f, stderr=subprocess.DEVNULL)
    root = ET.parse(out).getroot()
    seen = {}
    def val(e):
        if e.get('ref') and e.get('ref') in seen: return seen[e.get('ref')]
        t = e.get('fmt') if e.get('fmt') is not None else ''.join(e.itertext()).strip()
        if e.get('id'): seen[e.get('id')] = t
        return t
    rows = []
    for r in root.findall('.//row'):
        rows.append([val(c) for c in r])
    return rows

def ms(text):
    """'4.01 s' | '203.63 ms' | '41 ns' | '696.92 µs' -> milliseconds"""
    m = re.match(r'([\d.]+)\s*(ns|µs|us|ms|s)', text)
    if not m: return 0.0
    v = float(m.group(1)); u = m.group(2)
    return v * {'ns': 1e-6, 'µs': 1e-3, 'us': 1e-3, 'ms': 1, 's': 1000}[u]

def tsec(text):
    m = re.match(r'(\d+):(\d+)\.(\d+)\.(\d+)', text)
    if not m: return 0.0
    return int(m.group(1)) * 60 + int(m.group(2)) + int(m.group(3)) / 1000 + int(m.group(4)) / 1e6

if what == 'threads':
    for r in export('thread-info'):
        if proc in ' '.join(r): print(r)
elif what == 'qos':
    names = {}
    for r in export('thread-info'):
        # thread-info rows: tid, name, process... (shapes vary): keep the strings
        names[r[0]] = ' | '.join(r)
    total = collections.defaultdict(float)
    for r in export('ThreadQoSTable'):
        start, dur, process, thread, requested, effective, note = (r + [''] * 7)[:7]
        if proc and proc not in process: continue
        if thr and thr not in thread and thr not in names.get(thread, ''): continue
        total[(thread, requested, effective)] += ms(dur)
    for (thread, requested, effective), t in sorted(total.items(), key=lambda kv: -kv[1])[:40]:
        print(f'{t:10.1f} ms  requested {requested:16s} effective {effective:16s} {thread}')
elif what == 'hangs':
    for r in export('potential-hangs'):
        print(r)
elif what == 'blocked':
    rows = export('thread-state')
    out = []
    for r in rows:
        line = ' | '.join(r)
        if proc and proc not in line: continue
        if thr and thr not in line: continue
        # columns: start, duration, thread, state, ..., reason (shapes vary): report the long ones
        durs = [ms(c) for c in r if re.match(r'[\d.]+\s*(ns|µs|us|ms|s)$', c)]
        d = max(durs) if durs else 0
        if d >= min_ms and not any(s in line for s in ('Running', 'Unknown')):
            out.append((d, tsec(r[0]), line[:260]))
    for d, t, line in sorted(out, key=lambda x: -x[0])[:40]:
        print(f'{d:9.1f} ms at {t:8.3f} s  {line}')
