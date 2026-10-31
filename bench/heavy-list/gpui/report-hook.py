"""Print reproducible hook-comparison tables without rewriting historical results."""
import argparse
from datetime import timezone
import json
from pathlib import Path
import plistlib
import re
import statistics
import subprocess


def background(series):
    quiet = [json.loads(line) for line in (series/'quiet.jsonl').read_text().splitlines()]
    offset = statistics.median(row['wall']-row['time'] for row in quiet)
    schedule = json.loads((series/'schedule.json').read_text())
    windows = [(r['start']+offset, r['end']+offset) for r in schedule if r['config'] == 'c']
    rows = []
    for path in sorted(series.glob('power-*.plist')):
        values = []
        for fragment in re.findall(rb'<\?xml.*?</plist>', path.read_bytes(), re.S):
            raw = plistlib.loads(fragment)
            end = raw['timestamp'].replace(tzinfo=timezone.utc).timestamp()
            seconds = raw['elapsed_ns']/1e9
            # Whole configuration windows, including startup and inter-arm gaps.
            if not any(a <= end and end <= b for a, b in windows):
                continue
            tasks = raw['tasks']
            cores = lambda task: task['cputime_ms_per_s']/1000
            named = lambda name: sum(cores(t) for t in tasks if t['name'] == name)
            values.append(dict(seconds=seconds, busy=cores(raw['all_tasks']),
                               perf=named('PerfPowerServices'), dead=named('DEAD_TASKS'),
                               fleet=sum(t['name'] == 'fleet' for t in tasks), claude=named('claude')))
        assert values, f'{path}: no interior power samples'
        mean = lambda key: sum(v[key]*v['seconds'] for v in values)/sum(v['seconds'] for v in values)
        rows.append(dict(file=str(path), count=len(values), busyMean=mean('busy'),
                         busyMax=max(v['busy'] for v in values), perfMedian=statistics.median(v['perf'] for v in values),
                         deadMean=mean('dead'), fleetMedian=statistics.median(v['fleet'] for v in values),
                         claudeMax=max(v['claude'] for v in values),
                         busyPass=sum(v['busy'] <= 2 for v in values),
                         perfPass=sum(v['perf'] <= .25 for v in values)))
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('series', type=Path)
    parser.add_argument('analysis', type=Path, help='analyze-hook.py output directory')
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    aggregate = Path(__file__).with_name('aggregate.mjs')
    baseline = subprocess.check_output(['bun', str(aggregate), str(args.series/'a')], text=True).strip()
    comparison = (args.analysis/'comparison.md').read_text().strip()
    data = json.loads((args.analysis/'analysis.json').read_text())
    prediction = ['| Arm | Predicted hook CPU, ms/s → result | Predicted power CPU, ms/s → result |', '|---|---:|---:|']
    bounds = ['| Arm | Frames/s change, b−a; c−b | Peak footprint change, b−a; c−b |', '|---|---:|---:|']
    for arm, name in [('native', 'Native'), ('gpui', 'GPUI'), ('ink', 'Ink'), ('egui', 'egui')]:
        p = data['predictions'][arm]
        fields = []
        for key in ['hook', 'power']:
            row = p[key]
            low, high = row['prediction']
            fields.append(f"{low:+g} to {high:+g} → {row['result']:+.3f}, **{'met' if row['met'] else 'miss'}**")
        prediction.append('| '+name+' | '+' | '.join(fields)+' |')
        fields = []
        for key in ['fling_fps', 'peak_mib']:
            fields.append('; '.join(f"{p[c+'_'+key]['result_percent']:+.3f}%" for c in ['b-a', 'c-b'])
                          if 'b-a_'+key in p else '—')
        bounds.append('| '+name+' | '+' | '.join(fields)+' |')
    background_rows = background(args.series)
    (args.analysis/'background.json').write_text(json.dumps(background_rows, indent=2)+'\n')
    bg = ['| (c) round | Round samples | Busy cores, mean / max | PerfPowerServices, median cores | DEAD_TASKS, mean cores | fleet count, median | claude, max cores | Pass ≤2 busy / ≤0.25 PerfPowerServices |',
          '|---|---:|---:|---:|---:|---:|---:|---:|']
    for r in background_rows:
        bg.append(f"| {Path(r['file']).stem} | {r['count']} | {r['busyMean']:.2f} / {r['busyMax']:.2f} | {r['perfMedian']:.2f} | {r['deadMean']:.2f} | {r['fleetMedian']:g} | {r['claudeMax']:.2f} | {r['busyPass']} / {r['perfPass']} |")
    blocks = [baseline, comparison, '\n'.join(prediction), '\n'.join(bounds), '\n'.join(bg)]
    report = '\n\n'.join(blocks)+'\n'
    (args.analysis/'report.md').write_text(report)
    if args.verify:
        record = Path(__file__).with_name('RESULTS.md').read_text().split("## G4m, the instrument's share", 1)[1]
        for block in blocks:
            assert block in record, f'record differs:\n{block}'
        print('Hook report reproduced exactly: complete (a) tables, comparison, predictions, bounds and background.')
    else:
        print(report, end='')


if __name__ == '__main__':
    main()
