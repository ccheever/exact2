"""Summarize G6's retained JSON and PresentMon CSV; never alter raw observations."""
import csv
import json
from pathlib import Path
import statistics as S
import sys

root=Path(sys.argv[1])

def metric(values):
    values=[v for v in values if v is not None]
    return dict(median=S.median(values),min=min(values),max=max(values),n=len(values)) if values else None


def rate(before,after,key,frequency):
    return (after[key]-before[key])*frequency/(after['qpc']-before['qpc'])


def input_metrics(sample):
    gaps=[]
    for gesture in range(4):
        ticks=[r['qpc'] for r in sample['posts'] if r['gesture']==gesture]
        gaps.extend((b-a)*1000/sample['qpcFrequency'] for a,b in zip(ticks,ticks[1:]))
    return dict(posts=len(sample['posts']),maxPostGapMs=max(gaps),
                latePostGaps=sum(gap>25 for gap in gaps))


def frames(sample,path):
    frequency=sample['qpcFrequency']
    if not path.exists(): return {}
    with path.open(encoding='utf-8-sig') as f:
        rows=list(csv.DictReader(f))
    times=sorted(float(r['QPCTime'])+float(r['msUntilDisplayed'])*frequency/1000
                 for r in rows if r['Dropped']=='0' and float(r['msUntilDisplayed'])>0)
    if not times: return {}
    start,end=sample['flingStart']['qpc'],sample['flingEnd']['qpc']
    spans=sample['gestureWindows']
    intervals=[]
    count=0
    for a,b in spans:
        t=[v for v in times if a<=v<=b]
        count+=len(t)
        intervals.extend((y-x)*1000/frequency for x,y in zip(t,t[1:]))
    intervals.sort()
    seconds=sum(b-a for a,b in spans)/frequency
    if not intervals: return {}
    return dict(p50=intervals[min(int(.5*len(intervals)),len(intervals)-1)],
                p95=intervals[min(int(.95*len(intervals)),len(intervals)-1)],
                p99=intervals[min(int(.99*len(intervals)),len(intervals)-1)],
                latePerSecond=sum(x>25 for x in intervals)/seconds,
                gestureFps=count/seconds,phaseFps=sum(start<=v<=end for v in times)*frequency/(end-start),
                restPresents=sum(end<v<=sample['restEnd']['qpc'] for v in times),
                displayedFrames=len(times),csvRows=len(rows),
                modes=sorted(set(r['PresentMode'] for r in rows)))

report={}
for phase in ['warm','warm-startup','cold']:
    directory=root/phase
    if not directory.exists(): continue
    quiet=[json.loads(line) for line in (directory/'quiet.jsonl').read_text().splitlines()]
    phase_report=dict(quietObservations=len(quiet),loadedObservations=[x for x in quiet if not x['quiet']],
                      maxCpuPercent=max(x['cpuPercent'] for x in quiet),arms={})
    for arm in ['d3d11','wgpu','egui']:
        samples=[json.loads(p.read_text()) for p in sorted(directory.glob(arm+'-*.json'))]
        passed=[x for x in samples if x['status']=='ok']
        entry=dict(attempts=len(samples),failures=[x for x in samples if x['status']!='ok'],
                   dwmMs=metric([x['dwmMs'] for x in passed]),dxgiMs=metric([x['dxgiMs'] for x in passed]))
        full=[x for x in passed if 'flingEnd' in x]
        if full:
            measures={}
            for x in full:
                f=(frames(x,directory/f'{arm}-{x["number"]}-presents.csv')
                   if x.get('presentmonEventsLost',0)==0 and x.get('presentmonExit')==0 else {})
                f['eventsLost']=x.get('presentmonEventsLost',0)
                f.update(input_metrics(x))
                frequency=x['qpcFrequency']
                f.update(flingCpu=rate(x['flingStart'],x['flingEnd'],'processCpuMs',frequency),
                         flingMain=rate(x['flingStart'],x['flingEnd'],'mainCpuMs',frequency),
                         restCpu=rate(x['flingEnd'],x['restEnd'],'processCpuMs',frequency),
                         restMain=rate(x['flingEnd'],x['restEnd'],'mainCpuMs',frequency),
                         peakWS=x['restEnd']['peakWorkingSet']/2**20,
                         sampledFlingPeakWS=x['sampledPeakFling']/2**20,
                         flingEndWS=x['flingEnd']['workingSet']/2**20,
                         restEndWS=x['restEnd']['workingSet']/2**20,
                         focusLosses=len(x['focusLosses']))
                measures[str(x['number'])]=f
            entry['runs']=measures
            entry['metrics']={key:metric([m.get(key) for m in measures.values()]) for key in set().union(*(m.keys() for m in measures.values())) if key!='modes'}
        phase_report['arms'][arm]=entry
    report[phase]=phase_report
print(json.dumps(report,indent=2))
