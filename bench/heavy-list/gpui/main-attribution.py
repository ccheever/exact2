"""Stage 0 Python reducer. Run from the repository root; inputs and outputs are retained under target/s0."""
import json,collections,pathlib
p=pathlib.Path('target/s0/cpu-profiler');ss=json.load(open(p/'samples.json'));end=json.load(open(p/'ink.json'));main=next(x['tid'] for x in ss if 'Main Thread' in x['thread']);summary={}
for phase in ['fling','rest']:
 rows=collections.Counter();details={};d=end[phase]['end']['time']-end[phase]['start']['time']
 for s in ss:
  if s['phase']!=phase or s['tid']!=main:continue
  cat=None
  for f in s['stack']:
   n=f['name'];b=f['binary'];c=None
   if 'benchmark-probe' in b:c='probe'
   elif 'exact_ink::counters' in n or 'counter_state' in n:c='counter observer'
   elif 'exact_ink::path' in n or 'exact_linux::ink::clips' in n:c='path / clips'
   elif 'exact_ink::atlas' in n or 'exact_linux::ink::glyph' in n:c='glyph / atlas'
   elif 'exact_ink::scene' in n or 'exact_ink::instance' in n:c='scene / instance encoding'
   elif 'exact_kernel::layout' in n:c='layout'
   elif 'exact_ink::device::metal' in n:c='Metal device / texture'
   elif 'exact_linux::paint::Painter' in n:c='paint walk'
   elif 'exact_linux::ink::Ink' in n:c='Ink drawing'
   elif 'exact_linux::presenter' in n:c='presenter service'
   elif 'exact_apple_painted::window::App' in n and ('::draw' in n or '::about_to_wait' in n):c='carrier draw / dispatch'
   elif 'winit::' in n:c='winit dispatch'
   if c:cat=c;break
  if cat is None:cat='AppKit / CoreFoundation / other platform'
  rows[cat]+=s['weight_ms'];details.setdefault(cat,[]).append(s['stack'][0] if s['stack'] else {})
 summary[phase]=dict(seconds=d,weight_ms=sum(rows.values()),weighted_ms_per_s={k:v/d for k,v in rows.most_common()},counts=dict(rows),sample_leaves={k:v[:12] for k,v in details.items()})
(p/'main-attribution.json').write_text(json.dumps(summary,indent=2))
