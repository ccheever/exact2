"""Summarize all ten startup pairs and retain every quiet-rule observation."""
import argparse
import json
from pathlib import Path
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
root = parser.parse_args().directory
samples = {arm: sorted([json.loads(f.read_text()) for f in (root/arm).glob('d3d11-*.json')],
                      key=lambda row: row['number']) for arm in ['before', 'after']}

def distribution(values):
    return dict(median=statistics.median(values), min=min(values), max=max(values))

report = {}
for arm, rows in samples.items():
    assert [r['number'] for r in rows] == list(range(1, 11))
    assert all(r['status'] == 'ok' for r in rows)
    report[arm] = {key: distribution([r[key] for r in rows]) for key in ['dwmMs', 'dxgiMs']}
report['paired'] = {}
for key in ['dwmMs', 'dxgiMs']:
    values = [after[key]-before[key] for after, before in zip(samples['after'], samples['before'])]
    report['paired'][key] = dict(**distribution(values), deltas=values)
quiet = [json.loads(line) for line in (root/'quiet.jsonl').read_text().splitlines()]
report['quiet'] = dict(n=len(quiet), maxCpu=max(q['cpuPercent'] for q in quiet),
                       loaded=[q for q in quiet if not q['quiet']])
print(json.dumps(report, indent=2))
