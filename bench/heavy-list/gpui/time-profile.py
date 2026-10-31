"""Stage 0 Python reducer. Run from the repository root; inputs and outputs are retained under target/s0."""
from pathlib import Path
import xml.etree.ElementTree as E, json,datetime,subprocess,collections
p=Path('target/s0/cpu-profiler');root=E.parse(p/'cpu.xml').getroot();ids={e.get('id'):e for e in root.iter() if e.get('id')}
def resolve(e):
 while e is not None and e.get('ref'):e=ids[e.get('ref')]
 return e
frames={e.get('id'):e.get('name','') for e in root.iter('frame') if e.get('id')}
keys=list(frames);names=subprocess.check_output(['rustfilt'],input='\n'.join(frames[k] for k in keys)+'\n',text=True).splitlines();frames=dict(zip(keys,names))
clock=json.load(open(p/'clock.json'));toc=E.parse(p/'toc.xml').getroot();start=datetime.datetime.fromisoformat(toc.find('.//start-date').text).timestamp()-clock['wall_minus_monotonic']
probe=json.load(open(p/'ink.json'));samples=[]
for row in root.iter('row'):
 fields=list(row);t=start+int(resolve(fields[0]).text)/1e9
 phase=next((n for n in ['fling','rest'] if probe[n]['start']['time']<=t<probe[n]['end']['time']),None)
 if phase is None:continue
 thread=resolve(fields[1]);tid=int(resolve(thread.find('tid')).text);weight=int(resolve(fields[5]).text)/1e6;stack=resolve(fields[6]);trace=[]
 for node in stack.findall('frame'):
  node=resolve(node);binary=resolve(node.find('binary'));trace.append(dict(name=frames[node.get('id')],binary=binary.get('name') if binary is not None else '?'))
 samples.append(dict(time=t,phase=phase,tid=tid,thread=thread.get('fmt'),state=resolve(fields[4]).text,weight_ms=weight,stack=trace))
(p/'samples.json').write_text(json.dumps(samples))
summary={}
for phase in ['fling','rest']:
 ss=[s for s in samples if s['phase']==phase];seconds=probe[phase]['end']['time']-probe[phase]['start']['time'];threads={};leaves=collections.Counter();owners=collections.Counter();unresolved=0
 for s in ss:
  w=s['weight_ms'];tid=s['tid'];t=threads.setdefault(tid,dict(name=s['thread'],samples=0,weight_ms=0,leaves=collections.Counter(),binaries=collections.Counter()));t['samples']+=1;t['weight_ms']+=w
  stack=s['stack'];leaf=stack[0] if stack else dict(name='unknown',binary='?');key=leaf['binary']+'!'+leaf['name'];t['leaves'][key]+=w;t['binaries'][leaf['binary']]+=w;leaves[key]+=w
  names='\n'.join(f['name'] for f in stack);bins={f['binary'] for f in stack};owner='platform'
  if not stack or leaf['name'].startswith('0x') or leaf['name']=='unknown':owner='unresolved';unresolved+=w
  elif any('benchmark-probe' in b for b in bins):owner='probe'
  elif 'Main Thread' in s['thread']:owner='main carrier/platform'
  elif 'exact_linux::image' in names:owner='image'
  elif 'exact_ink::present' in names:owner='present worker'
  elif 'agent' in names:owner='agent'
  elif any('Metal' in b or 'IOGPU' in b for b in bins):owner='Metal driver'
  elif 'CoreVideo' in bins:owner='display link'
  owners[owner]+=w
 for t in threads.values():
  t['sampled_ms_per_s']=t['weight_ms']/seconds;t['leaves']=t['leaves'].most_common(12);t['binaries']=t['binaries'].most_common()
 summary[phase]=dict(seconds=seconds,samples=len(ss),states=dict(collections.Counter(s['state'] for s in ss)),weight_ms=sum(s['weight_ms'] for s in ss),unresolved_weight_ms=unresolved,owners_ms_per_s={k:v/seconds for k,v in owners.items()},threads=threads,leaves=leaves.most_common(25))
(p/'profile-summary.json').write_text(json.dumps(summary,indent=2))
for phase,s in summary.items():
 print(phase,'seconds',s['seconds'],'samples',s['samples'],'states',s['states'],'weight',s['weight_ms'],'owners',s['owners_ms_per_s'])
 print('threads',[(k,t['name'],round(t['sampled_ms_per_s'],3),t['binaries'][:3]) for k,t in s['threads'].items()]);print('leaves',s['leaves'][:12])
