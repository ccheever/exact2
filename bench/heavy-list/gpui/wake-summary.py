"""Stage 0 Python reducer. Run from the repository root; inputs and outputs are retained under target/s0."""
import json,collections,pathlib,subprocess
results={}
for name in ['wake-sources-2','wake-callers','rest-final']:
 p=pathlib.Path('target/s0')/name;j=json.load(open(p/'ink.json'));trace=json.load(open(p/'ink.json.trace.json'));lo=j['rest']['start']['time']*1e6;hi=j['rest']['end']['time']*1e6;e=[x for x in trace['traceEvents'] if lo<=x['ts']<hi];service=[x for x in e if x['name']=='carrier service'];dispatch=[x for x in service if x['args']['value']==1];arms=[x for x in e if x['name']=='run loop timer arm'];requests=[x for x in e if x['name']=='run loop wake request'];calls={}
 for kind,events in [('arm',arms),('wake',requests)]:
  raw=collections.Counter(x['args'].get('caller','unmarked') for x in events);keys=list(raw);names=subprocess.check_output(['rustfilt'],input='\n'.join(keys)+'\n',text=True).splitlines();calls[kind]=dict(zip(names,raw.values()))
 after_arm=[x for x in dispatch if any(0<=x['ts']-(y['ts']+y['dur'])<1000 for y in arms)];other=[x for x in dispatch if x not in after_arm]
 result=dict(seconds=(hi-lo)/1e6,dropped=trace['dropped'],marks=dict(collections.Counter(x['name'] for x in e)),service_causes=dict(collections.Counter(x['args']['value'] for x in service)),dispatch_within_1ms_after_arm=len(after_arm),caller_counts=calls,other_dispatch_times_ms=[(x['ts']-lo)/1000 for x in other],wake_request_times_ms=[(x['ts']-lo)/1000 for x in requests],arm_delays_ms=[x['args']['value'] for x in arms])
 (p/'wake-summary.json').write_text(json.dumps(result,indent=2));results[name]=result
 for phase in ['fling','rest']:
  for fmt in ['json','md']:
   args=['bun','bench/heavy-list/gpui/exclusive.mjs',str(p/'ink.json.trace.json')]+(['--rest'] if phase=='rest' else [])+(['--json'] if fmt=='json' else [])
   (p/('exclusive'+('-rest' if phase=='rest' else '')+'.'+fmt)).write_text(subprocess.check_output(args,text=True))
print(json.dumps(results,indent=2))
