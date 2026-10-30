"""Stage 0 Python reducer. Run from the repository root; inputs and outputs are retained under target/s0."""
import bisect,csv,json,pathlib,subprocess
root=pathlib.Path('target/s0/windows');summary={}
for p in sorted(root.iterdir()):
 if not p.is_dir() or not (p/'d3d11-0.json').exists():continue
 j=json.load(open(p/'d3d11-0.json'));f=j['qpcFrequency'];base=j['flingStart'];lo=base['qpc'];hi=j['flingEnd']['qpc'];end=j['restEnd']['qpc']
 rows=list(csv.DictReader((p/'d3d11-0-presents.csv').open(encoding='utf-8-sig')))
 shown=sorted(float(r['QPCTime'])+float(r['msUntilDisplayed'])*f/1000 for r in rows if r['Dropped']=='0' and float(r['msUntilDisplayed'])>0)
 def unix(q):return base['wallUnix']+(q-lo)/f
 c=json.load(open(p/'counters.json')) if (p/'counters.json').exists() else None
 def endpoint(key):
  s=j[key];t=unix(s['qpc']);r=dict(time=t,cpu=s['processCpuMs']/1000,main=s['mainCpuMs']/1000,threads=[dict(id=x['id'],name='main' if x['id']==j['mainThread'] else 'thread '+str(x['id']),cpu=x['cpu']) for x in s['threads']])
  if c:
   candidates=[v for v in c['samples'] if v['timeUnixMs']<=t*1000];selected=candidates[-1] if candidates else c['samples'][0]
   r['counters']={**selected,'displayedFrames':bisect.bisect_right(shown,s['qpc'])};r['counterSampleTime']=selected['timeUnixMs']/1000;r['counterLagMs']=(t-r['counterSampleTime'])*1000
  return r
 a,b,z=[endpoint(k) for k in ['flingStart','flingEnd','restEnd']]
 raw=dict(screenHz=60,cpuOnly=False,fling=dict(start=a,end=b),rest=dict(start=b,end=z),source='d3d11-0.json and counters.json; displayed gauge from PresentMon; counter samples may precede CPU endpoints')
 d=b['time']-a['time'];rd=z['time']-b['time'];result=dict(seconds=d,cpu=(b['cpu']-a['cpu'])*1000/d,main=(b['main']-a['main'])*1000/d,rest_cpu=(z['cpu']-b['cpu'])*1000/rd,rest_seconds=rd,displayed_fling=bisect.bisect_right(shown,hi)-bisect.bisect_left(shown,lo),displayed_rest=bisect.bisect_right(shown,end)-bisect.bisect_right(shown,hi),peak_working_set=j['sampledPeakFling'],endpoint_counter_lags_ms=[x.get('counterLagMs') for x in [a,b,z]])
 if c:
  trace={**c,'traceEvents':c['traceEvents']+[dict(name='presented',ph='X',ts=unix(t)*1e6,dur=0,pid=j['pid'],tid=1,args=dict(value=0)) for t in shown]}
  (p/'probe.json').write_text(json.dumps(raw,indent=2));(p/'probe.json.trace.json').write_text(json.dumps(trace))
  for flags,suffix in [([],''),(['--rest'],'-rest')]:
   for fmt,extra in [('json',['--json']),('md',[])]:
    with (p/('exclusive'+suffix+'.'+fmt)).open('w') as out:subprocess.run(['bun','bench/heavy-list/gpui/exclusive.mjs',str(p/'probe.json.trace.json'),*flags,*extra],stdout=out,check=True)
  result['counter_sample_window_seconds']=b['counterSampleTime']-a['counterSampleTime']
 summary[p.name]=result
(root/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2))
