#!/usr/bin/env python3
"""table.py <agg.json> — markdown per-kind table (SwiftUI vs exact2, Δ = exact2 − SwiftUI; Expo fps for context) + a ranked list."""
import json, sys, math
a = json.load(open(sys.argv[1]))
order = ['17', 'photo+markdown+typeface'] + sorted(k for k in a if k not in ('17', 'photo+markdown+typeface'))
f = lambda v, p=1: '—' if v is None or (isinstance(v, float) and math.isnan(v)) else f'{v:.{p}f}'
d = lambda e, s, p=1: '—' if any(isinstance(x, float) and math.isnan(x) for x in (e, s)) else f'{e - s:+.{p}f}'
print('| kind | rounds | fps S / E (Δ) | 12k S / E | 24k S / E | ladder 48k S / E | 96k S / E | late/s S / E | busy ms/f S / E | CPU ms/s S / E (Δ) | main ms/s S / E (Δ) | peak MB S / E | Expo fps |')
print('|---|---|---|---|---|---|---|---|---|---|---|---|---|')
for k in order:
    s, e, x = a[k].get('swiftui', {}), a[k].get('exact2', {}), a[k].get('expo', {})
    g = lambda r, m: r.get(m, float('nan'))
    print(f"| {k} | {e.get('n', 0)} | {f(g(s,'fps'))} / {f(g(e,'fps'))} ({d(g(e,'fps'), g(s,'fps'))}) | {f(g(s,'f12k'),0)} / {f(g(e,'f12k'),0)} | {f(g(s,'f24k'),0)} / {f(g(e,'f24k'),0)} | "
          f"{f(g(s,'l48k'),0)} / {f(g(e,'l48k'),0)} | {f(g(s,'l96k'),0)} / {f(g(e,'l96k'),0)} | {f(g(s,'late'))} / {f(g(e,'late'))} | {f(g(s,'busy'))} / {f(g(e,'busy'))} | "
          f"{f(g(s,'cpu'),0)} / {f(g(e,'cpu'),0)} ({d(g(e,'cpu'), g(s,'cpu'),0)}) | {f(g(s,'main'),0)} / {f(g(e,'main'),0)} ({d(g(e,'main'), g(s,'main'),0)}) | {f(g(s,'peak'),0)} / {f(g(e,'peak'),0)} | {f(g(x,'fps'))} |")
