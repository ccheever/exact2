import argparse,json,pathlib,statistics,subprocess
parser=argparse.ArgumentParser(description='Pair the retained hook/power experiment by arm and round.')
parser.add_argument('series',type=pathlib.Path)
parser.add_argument('historical',type=pathlib.Path)
parser.add_argument('--out',type=pathlib.Path,required=True)
args=parser.parse_args()
args.out.mkdir(parents=True,exist_ok=True)
aggregate=pathlib.Path(__file__).with_name('aggregate.mjs')
root=args.series.resolve(); historical=args.historical.resolve()
arms=['native','gpui','ink','egui']
def dist(xs): return dict(median=statistics.median(xs),min=min(xs),max=max(xs))
def cell(d,n=2): return f"{d['median']:.{n}f} ({d['min']:.{n}f}–{d['max']:.{n}f})"
old=json.loads(subprocess.check_output(['bun',str(aggregate),str(historical),'--json']))
quiet=[json.loads(line) for line in (root/'quiet.jsonl').read_text().splitlines()]
assert all(r['quiet'] for r in quiet)
schedule=json.loads((root/'schedule.json').read_text()); assert len(schedule)==15 and all('end' in r for r in schedule)
summary={}; samples={}; audit={}
for config in 'abc':
    summary[config]=json.loads(subprocess.check_output(['bun',str(aggregate),str(root/config),'--json']))
    samples[config]={}
    for arm in arms:
        rows=summary[config]['samples'][arm]; assert len(rows)==5
        samples[config][arm]={}
        for sample in rows:
            path=pathlib.Path(sample['path']); raw=json.loads(path.read_text())
            round=int(path.stem.split('-')[1]); samples[config][arm][round]=sample
            assert raw['gpuHookEnabled']==(config!='a')
            assert raw['thermalStart']==raw['thermalEnd']==0 and not raw['lowPower']
            assert raw['scale']==2 and raw['frameSize']==[800,800]
            assert raw['activeAtStart'] and raw['activeAtEnd'] and not raw['focusLosses']
            assert sample['posted']==424
            before=max(i for i,q in enumerate(quiet) if q['time']<=raw['fling']['start']['time'])
            after=next(i for i,q in enumerate(quiet) if q['time']>=raw['rest']['end']['time'])
            cover=quiet[before:after+1]; assert all(b['time']-a['time']<6 for a,b in zip(cover,cover[1:]))
            gpu=raw['gpu']['fling']; assert gpu['committed']==gpu['completed'] and gpu['invalid']==0
            if config=='a': assert gpu['completed']==0 and gpu['queueClass']==gpu['bufferClass']==''
            elif arm!='native': assert gpu['completed']>0
            sample['completed_buffers']=gpu['completed']; sample['buffer_rate']=gpu['completed']/sample['fling_seconds']
        assert set(samples[config][arm])==set(range(1,6))
    audit[config]={k:summary[config][k] for k in ['rejected','unused']}
# Historical G4 medians/ranges transcribed from its published tables.
g4={
'native':{'fling_cpu':[131.91,112.39,143.12],'fling_main':[84.51,76.30,92.87],'peak_mib':[228.39,211.35,232.45]},
'gpui':{'fling_cpu':[84.92,82.05,89.05],'fling_main':[39.56,38.97,41.52],'fling_fps':[88.59,88.38,89.00],'peak_mib':[406.42,391.64,413.84]},
'ink':{'fling_cpu':[86.41,84.98,87.07],'fling_main':[43.83,41.98,44.65],'fling_fps':[32.60,32.29,33.20],'peak_mib':[591.56,587.67,593.42]},
'egui':{'fling_cpu':[66.84,64.93,71.54],'fling_main':[31.33,30.28,33.69],'fling_fps':[40.90,40.49,41.28],'peak_mib':[507.81,504.14,515.89]}}
g4={arm:{k:dict(zip(['median','min','max'],v)) for k,v in row.items()} for arm,row in g4.items()}
rows=['| Arm; metric | G4 | G4m original (c) | (a) hook off, power off | (b) hook on, power off | (c) hook on, power on |','|---|---:|---:|---:|---:|---:|']
for arm in arms:
    for label,key in [('process CPU, ms/s','fling_cpu'),('main CPU, ms/s','fling_main'),('presented frames/s','fling_fps'),('peak footprint, MiB','peak_mib')]:
        values=[g4[arm].get(key),old['metrics'][arm].get(key)]+[summary[c]['metrics'][arm].get(key) for c in 'abc']
        rows.append('| '+arm+'; '+label+' | '+' | '.join(cell(v) if v else '—' for v in values)+' |')
paired={}; predictions={}
for arm in arms:
    paired[arm]={}
    for delta,before,after in [('hook','a','b'),('power','b','c')]:
        for key in ['fling_cpu','fling_main']:
            values=[samples[after][arm][r][key]-samples[before][arm][r][key] for r in range(1,6)]
            paired[arm][delta+'_'+key]=dict(**dist(values),values=values)
    buffers=[samples['b'][arm][r]['completed_buffers'] for r in range(1,6)]
    paired[arm]['completed_buffers']=dist(buffers)
    if all(buffers):
        values=[(samples['b'][arm][r]['fling_cpu']-samples['a'][arm][r]['fling_cpu'])*1000/samples['b'][arm][r]['buffer_rate'] for r in range(1,6)]
        paired[arm]['hook_us_buffer']=dict(**dist(values),values=values)
    lo,hi={'native':[-5,5],'gpui':[1,10],'ink':[1,10],'egui':[5,20]}[arm]
    h=paired[arm]['hook_fling_cpu']['median']; p=paired[arm]['power_fling_cpu']['median']
    predictions[arm]=dict(hook=dict(prediction=[lo,hi],result=h,met=lo<=h<=hi),power=dict(prediction=[0,5],result=p,met=0<=p<=5))
    for before,after in [('a','b'),('b','c')]:
        for key,bound in [('fling_fps',5),('peak_mib',10)]:
            if key not in summary[before]['metrics'][arm]:continue
            change=(summary[after]['metrics'][arm][key]['median']/summary[before]['metrics'][arm][key]['median']-1)*100
            predictions[arm][after+'-'+before+'_'+key]=dict(bound_percent=bound,result_percent=change,met=abs(change)<=bound)
rows+=['','| Arm | Hook process CPU, ms/s | Hook main CPU, ms/s | Completed buffers in (b), ten seconds | Hook process CPU, µs/buffer | Power process CPU, ms/s | Power main CPU, ms/s |','|---|---:|---:|---:|---:|---:|---:|']
for arm in arms:
    p=paired[arm]; rows.append('| '+arm+' | '+' | '.join(cell(p[k],0 if k=='completed_buffers' else 3) if k in p else '—' for k in ['hook_fling_cpu','hook_fling_main','completed_buffers','hook_us_buffer','power_fling_cpu','power_fling_main'])+' |')
result=dict(summary=summary,paired=paired,predictions=predictions,audit=audit,quiet=dict(count=len(quiet),max_load=[max(r['load'][i] for r in quiet) for i in range(3)]))
(args.out/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
(args.out/'comparison.md').write_text('\n'.join(rows)+'\n')
print('\n'.join(rows)); print(json.dumps(dict(predictions=predictions,audit=audit,quiet=result['quiet']),indent=2))
