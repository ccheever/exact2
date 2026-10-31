"""M10 startup cohorts and phase tables, using the existing probe and quiet monitor."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import time
import sys

sys.dont_write_bytecode = True


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def read(path):
    return json.loads(path.read_text())


def wait_file(path, child, seconds=35):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if path.exists():
            return read(path)
        if child.poll() is not None:
            raise RuntimeError(f'exit {child.returncode} before {path.name}')
        time.sleep(.02)
    raise RuntimeError(f'timed out waiting for {path.name}')


def run(args):
    base, out = args.base.resolve(), args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    provenance = read(base / 'provenance.json')
    arms = ['ink', 'gpui', 'egui']
    paths = [provenance[a]['executable'] for a in arms]
    write(out / 'provenance.json', provenance)
    write(out / 'power-mode.json', {
        name: subprocess.check_output(['/usr/bin/pmset', '-g', *extra], text=True)
        for name, extra in [('settings', []), ('source', ['batt'])]})
    assert subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1
    spec = importlib.util.spec_from_file_location('quiet', Path(__file__).with_name('run-hook.py'))
    quiet = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(quiet)
    monitor = quiet.QuietMonitor(out / 'quiet.jsonl', paths)
    consecutive = 0
    gate_deadline = time.monotonic() + args.gate_seconds
    while time.monotonic() < gate_deadline and consecutive < 12:
        time.sleep(5)
        observation = monitor.observe()
        consecutive = consecutive + 1 if observation['quiet'] else 0
        print('QUIET', consecutive, observation['busyCores'], flush=True)
    write(out / 'gate.json', dict(passed=consecutive == 12, consecutive=consecutive))
    if consecutive != 12:
        raise RuntimeError('sixty-second quiet gate not met; no launches')
    first_sample = len(monitor.rows)
    monitor.thread.start()
    schedule = []
    child = None
    try:
        for round_number in range(1, args.rounds + 1):
            order = arms[(round_number-1) % 3:] + arms[:(round_number-1) % 3]
            for arm in order:
                for cohort in ['cold', 'warm']:
                    assert monitor.error is None, monitor.error
                    assert all(r['quiet'] for r in monitor.rows[first_sample:]), 'loaded cohort retained; stop'
                    name = f'{arm}-{round_number}-{cohort}'
                    output = out / f'{name}.json'
                    env = {k: v for k, v in os.environ.items()
                           if not k.startswith(('EXACT_', 'G1_', 'G3_', 'G4_', 'G5_', 'BENCH_', 'DYLD_'))}
                    env.update(G1_OUTPUT=str(output), G1_ARM=arm, G1_COLD_ONLY='1',
                               G1_GPU_HOOK='0', BENCH_LIVE='0', EXACT_INK_DEVICE='metal',
                               EXACT_WINDOW_WIDTH='800', EXACT_WINDOW_HEIGHT='768' if arm == 'ink' else '800')
                    if not args.no_trace:
                        env['G1_TRACE'] = '1'
                    record = dict(arm=arm, round=round_number, cohort=cohort,
                                  trace=not args.no_trace, start=monitor.now(), output=str(output))
                    schedule.append(record)
                    with (out / f'{name}.log').open('w') as log:
                        child = subprocess.Popen([provenance[arm]['executable']], env=env,
                                                 stdout=log, stderr=subprocess.STDOUT)
                        record['pid'] = child.pid
                        write(out / 'schedule.json', schedule)
                        try:
                            wait_file(Path(str(output) + '.window'), child)
                            child.send_signal(signal.SIGUSR1)
                            focus = wait_file(Path(str(output) + '.focus'), child)
                            assert focus['active'] and focus['frontmostPid'] == child.pid
                            assert focus['keyWindow'] == focus['window']
                            wait_file(Path(str(output) + '.ready'), child)
                            child.send_signal(signal.SIGUSR2)
                            result = wait_file(output, child)
                            record['exit'] = child.wait(timeout=5)
                            assert record['exit'] == 0
                            assert result['activeAtStart'] and result['activeAtEnd']
                            assert not result['focusLosses']
                            assert result['thermalStart'] == result['thermalEnd'] == 0
                            assert not result['lowPower'] and not result['gpuHookEnabled']
                            assert result['size'] == [800, 768] and result['scale'] == 2
                            assert result['coldMs'] > 0
                            if not args.no_trace:
                                trace = read(Path(str(output) + '.trace.json'))
                                assert trace['dropped'] == 0
                                names = {e['name'] for e in trace['traceEvents']}
                                assert names & {'probe present commit', 'probe direct present preceding commit'}, 'presentation commit not observed'
                                assert 'probe presented' in names
                            record['accepted'] = True
                        except BaseException as error:
                            record.update(accepted=False, error=str(error))
                            raise
                        finally:
                            record['end'] = monitor.now()
                            write(out / 'schedule.json', schedule)
                    child = None
                    print('LAUNCH', name, result['coldMs'], flush=True)
    finally:
        if child and child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        # Complete the observation that contains the final launch.
        time.sleep(5)
        monitor.stop.set()
        monitor.thread.join()
        write(out / 'run.json', dict(launches=len(schedule), expectedLaunches=args.rounds * 6,
              quiet=all(r['quiet'] for r in monitor.rows[first_sample:]),
              monitorError=str(monitor.error) if monitor.error else None))
    assert monitor.error is None, monitor.error
    assert all(r['quiet'] for r in monitor.rows[first_sample:]), 'loaded observations; no qualified table'


def distribution(values):
    return dict(n=len(values), p50=statistics.median(values), minimum=min(values), maximum=max(values)) if values else None


def sample(path, traced=True):
    result = read(path)
    if not traced:
        return dict(file=str(path), milestones={'constructor': result['preMainMs'],
                    'first presented': result['firstDrawableMs'], 'complete screen': result['coldMs']},
                    exclusive={}, inclusive={}, startup={}, arm=result['arm'])
    trace = read(Path(str(path) + '.trace.json'))
    assert trace['dropped'] == 0
    start = result['startup']['processStart'] * 1000
    end = start + result['coldMs']
    events = [dict(name=e['name'], start=e['ts']/1000, end=(e['ts']+e['dur'])/1000,
                   thread=e['tid'], value=e.get('args', {}).get('value', 0)) for e in trace['traceEvents']]
    def first(name, finish=False):
        values = [e['end' if finish else 'start'] - start for e in events
                  if e['name'] == name and start <= e['start'] <= end]
        return min(values) if values else None
    milestones = {'constructor': result['preMainMs'], 'main': first('boot main'), 'host entry': first('boot host entry'),
                  'AppKit finished launching': first('AppKit did finish launching'),
                  'window created': first('boot window created'),
                  'first window update': first('probe first window update'),
                  'visible window observed': first('probe visible window'),
                  'plan loaded': first('plan-ready'), 'fonts ready': first('fonts-ready'),
                  'data source bound': first('boot data bound'),
                  'first commit completed (runner returned)': first('runner-ready'),
                  'first layout completed': first('layout-ready'),
                  'first text shape start': first('text shape'),
                  'first text shape end': first('text shape', True),
                  'layer attached': first('boot layer attached'),
                  'first paint record': first('paint walk'),
                  'first paint record completed': first('paint walk', True),
                  'first present committed': min((v for v in [first('probe present commit', True),
                       first('probe direct present preceding commit', True)] if v is not None), default=None),
                  'first presented': result['firstDrawableMs'], 'complete screen': result['coldMs']}
    startup = result['startup']
    milestones['complete callback observed'] = startup['completeObserved'] * 1000 - start if startup.get('completeObserved') else None
    # The main-thread intervals nest. Divide at every boundary and charge each
    # slice to the innermost interval; off-thread work is kept in its own table.
    main = [e for e in events if e['thread'] == trace['mainThread'] and e['end'] > e['start']
            and e['start'] < end and e['end'] > start]
    points = sorted({start, end, start+result['preMainMs'],
                     *(max(start, e['start']) for e in main), *(min(end, e['end']) for e in main)})
    exclusive = {}
    for a, b in zip(points, points[1:]):
        containing = [e for e in main if e['start'] <= a and e['end'] >= b]
        name = min(containing, key=lambda e: e['end']-e['start'])['name'] if containing else (
            'process to constructor' if b <= start+result['preMainMs'] else 'unattributed / waiting')
        exclusive[name] = exclusive.get(name, 0) + b-a
    assert abs(sum(exclusive.values())-result['coldMs']) < .001
    inclusive = {}
    for e in events:
        duration = max(0, min(end, e['end'])-max(start, e['start']))
        if duration:
            key = ('main: ' if e['thread'] == trace['mainThread'] else 'other: ') + e['name']
            inclusive[key] = inclusive.get(key, 0) + duration
    return dict(file=str(path), milestones=milestones, exclusive=exclusive, inclusive=inclusive,
                startup=startup, arm=result['arm'])


def summarize(args):
    directory = args.directory.resolve()
    schedule = read(directory / 'schedule.json')
    run_record = read(directory / 'run.json')
    assert run_record['quiet'] and not run_record['monitorError'], 'cohort is not quiet'
    assert len(schedule) == run_record['launches'] == run_record['expectedLaunches'], 'incomplete cohort'
    expected = {(arm, cohort, r) for arm in ['ink', 'gpui', 'egui']
                for cohort in ['cold', 'warm'] for r in range(1, len(schedule) // 6 + 1)}
    assert {(r['arm'], r['cohort'], r['round']) for r in schedule} == expected, 'unbalanced cohort'
    assert all(r.get('accepted') and r.get('exit') == 0 for r in schedule), 'failed attempts retained; incomplete cohort'
    samples = [dict(sample(directory / Path(r['output']).name, r['trace']), cohort=r['cohort'], round=r['round']) for r in schedule]
    tables = {}
    for arm in ['ink', 'gpui', 'egui']:
        for cohort in ['cold', 'warm']:
            selected = [s for s in samples if s['arm'] == arm and s['cohort'] == cohort]
            tables[f'{arm} {cohort}'] = {
                kind: {name: distribution([s[kind].get(name, 0) for s in selected
                                           if kind != 'milestones' or s[kind].get(name) is not None])
                       for name in sorted({n for s in selected for n in s[kind]})}
                for kind in ['milestones', 'exclusive', 'inclusive']}
            tables[f'{arm} {cohort}']['counters'] = {
                name: distribution([s['startup'][name] for s in selected if name in s['startup']])
                for name in ['constructorMinorFaults', 'constructorMajorFaults', 'constructorPageins',
                             'constructorInstructions', 'constructorCycles', 'completeMinorFaults',
                             'completeMajorFaults', 'completePageins', 'completeInstructions',
                             'completeCycles', 'completeCpuSeconds', 'loadedImages', 'registeredObjcClasses']}
    paired = {}
    for arm in ['ink', 'gpui', 'egui']:
        cold = {s['round']: s for s in samples if s['arm'] == arm and s['cohort'] == 'cold'}
        warm = {s['round']: s for s in samples if s['arm'] == arm and s['cohort'] == 'warm'}
        paired[arm] = {name: distribution([cold[r]['milestones'][name]-warm[r]['milestones'][name]
                                          for r in cold if cold[r]['milestones'][name] is not None
                                          and warm[r]['milestones'][name] is not None])
                       for name in cold[next(iter(cold))]['milestones']}
    write(directory / 'summary.json', dict(samples=samples, tables=tables, coldMinusWarm=paired))
    for name, table in tables.items():
        print('\n' + name + '\n\n| Process age, ms | p50 | min–max | n |\n| --- | ---: | ---: | ---: |')
        for phase, value in table['milestones'].items():
            if value:
                print(f"| {phase} | {value['p50']:.3f} | {value['minimum']:.3f}–{value['maximum']:.3f} | {value['n']} |")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    launch = commands.add_parser('run')
    launch.add_argument('base', type=Path)
    launch.add_argument('out', type=Path)
    launch.add_argument('--rounds', type=int, default=10)
    launch.add_argument('--gate-seconds', type=int, default=300)
    launch.add_argument('--no-trace', action='store_true', help='same-launch-path observer control')
    analyze = commands.add_parser('summarize')
    analyze.add_argument('directory', type=Path)
    args = parser.parse_args()
    (run if args.command == 'run' else summarize)(args)
