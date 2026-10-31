"""M10 startup cohorts: ten quiet seconds before each launch; during-launch load is reported."""
import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
import re
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


def wait_file(path, child, seconds=35, accept=lambda value: True):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if path.exists():
            value = read(path)
            if accept(value):
                return value
        if child.poll() is not None:
            raise RuntimeError(f'exit {child.returncode} before {path.name}')
        time.sleep(.02)
    raise RuntimeError(f'timed out waiting for {path.name}')


class Activator:
    """Use the same NSRunningApplication activation as measure.m, before focus confirmation."""
    def __init__(self):
        self.appkit = ctypes.CDLL('/System/Library/Frameworks/AppKit.framework/AppKit')
        objc = self.objc = ctypes.CDLL('/usr/lib/libobjc.A.dylib')
        pointer = ctypes.c_void_p
        objc.objc_getClass.argtypes = objc.sel_registerName.argtypes = [ctypes.c_char_p]
        objc.objc_getClass.restype = objc.sel_registerName.restype = pointer
        objc.objc_autoreleasePoolPush.restype = pointer
        objc.objc_autoreleasePoolPop.argtypes = [pointer]
        self.application = objc.objc_getClass(b'NSRunningApplication')
        self.lookup = objc.sel_registerName(b'runningApplicationWithProcessIdentifier:')
        self.activate = objc.sel_registerName(b'activateWithOptions:')
        self.find = ctypes.CFUNCTYPE(pointer, pointer, pointer, ctypes.c_int)(('objc_msgSend', objc))
        self.send = ctypes.CFUNCTYPE(ctypes.c_bool, pointer, pointer, ctypes.c_ulong)(('objc_msgSend', objc))

    def __call__(self, pid):
        pool = self.objc.objc_autoreleasePoolPush()
        try:
            app = self.find(self.application, self.lookup, pid)
            return bool(app and self.send(app, self.activate, 1 << 1))
        finally:
            self.objc.objc_autoreleasePoolPop(pool)


def wait_for_lane(out, seconds=3600):
    deadline = time.monotonic() + seconds
    while True:
        rows = [line.split(None, 2) for line in subprocess.check_output(
            ['/bin/ps', '-ww', '-axo', 'pid=,ppid=,args='], text=True).splitlines()]
        parents = {int(pid): int(parent) for pid, parent, _ in rows}
        ours, pid = set(), os.getpid()
        while pid and pid not in ours:
            ours.add(pid)
            pid = parents.get(pid, 0)
        blockers = [dict(pid=int(pid), command=command) for pid, parent, command in rows
                    if int(pid) not in ours
                    and not (int(parent) in ours - {1} and command.startswith('/usr/bin/caffeinate '))
                    and (re.search(
                        r'target/g0/\S*run-[^ /]+\.py(?:\s|$)', command) or re.search(
                        r'(?:^|/)(?:measure|gpui-heavy-list|egui-heavy-list|HeavyExact2|Heavy list)(?:\s|$)', command))]
        with (out / 'ownership.jsonl').open('a') as stream:
            stream.write(json.dumps(dict(wall=time.time(), blockers=blockers)) + '\n')
        if not blockers:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError('another measurement lane remains active; no launch')
        time.sleep(5)


def run(args):
    base, out = args.base.resolve(), args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    provenance = read(base / 'provenance.json')
    provenance['startupDriverSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    arms = args.arms.split(',')
    if args.scoreboard:
        arms = ['native', 'gpui', 'ink', 'egui']
    paths = [provenance[a]['executable'] for a in arms]
    write(out / 'provenance.json', provenance)
    write(out / 'power-mode.json', {
        name: subprocess.check_output(['/usr/bin/pmset', '-g', *extra], text=True)
        for name, extra in [('settings', []), ('source', ['batt'])]})
    assert subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1
    activate = Activator()
    spec = importlib.util.spec_from_file_location('quiet', Path(__file__).with_name('run-hook.py'))
    quiet = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(quiet)
    class StartupQuiet(quiet.QuietMonitor):
        def observe(self):
            row = super().observe()
            absent = subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1
            with (out / 'power-observations.jsonl').open('a') as stream:
                stream.write(json.dumps(dict(time=self.now(), absent=absent)) + '\n')
            row['quiet'] = row['quiet'] and absent
            row['powerAbsent'] = absent
            return row
    monitor = StartupQuiet(out / 'quiet.jsonl', paths + [base / 'measure', base / 'exact-wheel'])
    wait_for_lane(out)
    # Five prior amendments admitted no startup cohort: launches wake XProtect
    # and other daemons. Gate before each launch and retain load during it.
    schedule = []
    child = None
    try:
        for round_number in range(1, args.rounds + 1):
            order = arms[(round_number-1) % len(arms):] + arms[:(round_number-1) % len(arms)]
            if args.scoreboard:
                for arm in order:
                    wait_for_lane(out)
                    gate = quiet.launch_gate(monitor, args.gate_seconds)
                    write(out / f'{arm}-{round_number}.gate.json', gate)
                    assert gate['passed'], 'ten-second prelaunch quiet gate not met'
                    env = {k: v for k, v in os.environ.items()
                           if not k.startswith(('EXACT_', 'G1_', 'G3_', 'G4_', 'G5_', 'BENCH_', 'DYLD_'))}
                    env.update(G1_GPU_HOOK='0', EXACT_INK_DEVICE='metal', G1_ONLY_ARM=arm,
                               G3_PAINTED_BIN=provenance['ink']['executable'],
                               G4_EGUI_BIN=provenance['egui']['executable'])
                    command = [str(base / 'measure'), str(out), provenance['native']['executable'],
                               provenance['gpui']['executable'], str(base / 'exact-wheel'),
                               '1', str(round_number), 'cold-only']
                    first_sample = len(monitor.rows)
                    record = dict(arm=arm, round=round_number, cohort='cold', trace=False,
                                  start=monitor.now(), gate=gate,
                                  output=str(out / f'{arm}-{round_number}.json'))
                    schedule.append(record)
                    write(out / 'schedule.json', schedule)
                    monitor.begin_launch()
                    try:
                        with (out / f'driver-{arm}-{round_number}.log').open('w') as log:
                            child = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
                            record['pid'] = child.pid
                            record['exit'] = child.wait(timeout=300)
                        child = None
                        output = Path(record['output'])
                        record['accepted'] = output.exists() and read(output).get('accepted') is True
                        assert record['exit'] == 0 and record['accepted'], 'failed original launch; no replacement table'
                    finally:
                        record['end'] = monitor.now()
                        monitor.end_launch()
                        record['duringLoad'] = quiet.launch_load(monitor.rows[first_sample:])
                        write(out / 'schedule.json', schedule)
                    print('SCOREBOARD', arm, round_number, flush=True)
                continue
            for arm in order:
                for cohort in ['cold', 'warm']:
                    wait_for_lane(out)
                    assert subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1, 'power sampler started'
                    name = f'{arm}-{round_number}-{cohort}'
                    gate = quiet.launch_gate(monitor, args.gate_seconds)
                    write(out / f'{name}.gate.json', gate)
                    assert gate['passed'], 'ten-second prelaunch quiet gate not met'
                    first_sample = len(monitor.rows)
                    output = out / f'{name}.json'
                    env = {k: v for k, v in os.environ.items()
                           if not k.startswith(('EXACT_', 'G1_', 'G3_', 'G4_', 'G5_', 'BENCH_', 'DYLD_'))}
                    probe_arm = arm.split('-')[0]
                    env.update(G1_OUTPUT=str(output), G1_ARM=probe_arm, G1_COLD_ONLY='1',
                               G1_GPU_HOOK='0', BENCH_LIVE='0', EXACT_INK_DEVICE='metal',
                               DYLD_PRINT_STATISTICS='1', DYLD_PRINT_STATISTICS_DETAILS='1',
                               EXACT_WINDOW_WIDTH='800', EXACT_WINDOW_HEIGHT='768' if probe_arm == 'ink' else '800')
                    if not args.no_trace:
                        env['G1_TRACE'] = '1'
                    record = dict(arm=arm, round=round_number, cohort=cohort,
                                  trace=not args.no_trace, start=monitor.now(), output=str(output), gate=gate)
                    schedule.append(record)
                    monitor.begin_launch()
                    with (out / f'{name}.log').open('w') as log:
                        child = subprocess.Popen([provenance[arm]['executable']], env=env,
                                                 stdout=log, stderr=subprocess.STDOUT)
                        record['pid'] = child.pid
                        write(out / 'schedule.json', schedule)
                        try:
                            activate(child.pid)
                            wait_file(Path(str(output) + '.window'), child)
                            focus_deadline, last_focus = time.monotonic() + 5, 0
                            record['focusChecks'] = []
                            while time.monotonic() < focus_deadline:
                                activate(child.pid)
                                child.send_signal(signal.SIGUSR1)
                                focus = wait_file(Path(str(output) + '.focus'), child,
                                                  seconds=max(.01, focus_deadline-time.monotonic()),
                                                  accept=lambda value: value['time'] > last_focus)
                                record['focusChecks'].append(focus)
                                last_focus = focus['time']
                                if focus['active'] and focus['frontmostPid'] == child.pid and focus['keyWindow'] == focus['window']:
                                    break
                                time.sleep(.05)
                            assert focus['active'] and focus['frontmostPid'] == child.pid and focus['keyWindow'] == focus['window'], 'focus not confirmed'
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
                            record['dyldStatisticsLines'] = [line for line in (out / f'{name}.log').read_text().splitlines()
                                                            if re.search(r'pre-main|dylib loading|rebase/binding|initializer time', line)]
                            if not args.no_trace:
                                trace = read(Path(str(output) + '.trace.json'))
                                assert trace['dropped'] == 0
                                names = {e['name'] for e in trace['traceEvents']}
                                assert names & {'probe present commit', 'probe direct present request'}, 'presentation request not observed'
                                assert 'probe presented' in names
                            record['accepted'] = True
                        except BaseException as error:
                            record.update(accepted=False, error=str(error))
                            raise
                        finally:
                            record['end'] = monitor.now()
                            monitor.end_launch()
                            record['duringLoad'] = quiet.launch_load(monitor.rows[first_sample:])
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
        write(out / 'run.json', dict(launches=len(schedule),
              qualification='m10-cut1d-prelaunch',
              arms=arms, cohorts=['cold'] if args.scoreboard else ['cold', 'warm'],
              expectedLaunches=args.rounds * len(arms) * (1 if args.scoreboard else 2),
              quiet=bool(schedule) and all(r['gate']['passed'] for r in schedule),
              powerAbsent=all(r['powerAbsent'] for r in monitor.rows),
              monitorError=str(monitor.error) if monitor.error else None))
    assert all(r['powerAbsent'] for r in monitor.rows), 'power sampler overlapped cohort'


def distribution(values):
    return dict(n=len(values), p50=statistics.median(values), minimum=min(values), maximum=max(values)) if values else None


def sample(path, traced=True):
    result = read(path)
    assert result.get('accepted') is not False, f'{path}: rejected original launch'
    if not traced:
        return dict(file=str(path), milestones={'constructor': result['preMainMs'],
                    'first presented': result.get('firstDrawableMs'), 'complete screen': result['coldMs']},
                    exclusive={}, inclusive={}, startup=result.get('startup', {}),
                    clocks=launch_clocks(result.get('startup', {})), arm=result['arm'])
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
    present_commit = min((v for v in [first('probe present commit', True),
                         first('probe direct present preceding commit', True)] if v is not None), default=None)
    if result['arm'] == 'ink' and first('probe direct present request') is not None:
        # The copy commits on the worker; the direct present is on main. Its
        # thread-local preceding-commit marker can name the earlier compose pass.
        encodes = [e for e in events if e['name'] == 'present encode' and start <= e['start'] <= end]
        encode = min(encodes, key=lambda e: e['start'], default=None)
        present_commit = min((e['end'] - start for e in events if encode
            and e['thread'] == encode['thread'] and encode['end'] <= e['start'] <= end
            and e['name'] in {'probe command commit', 'probe present commit'}), default=None)
    milestones = {'constructor': result['preMainMs'], 'main': first('boot main'), 'host entry': first('boot host entry'),
                  'AppKit finished launching': first('AppKit did finish launching'),
                  'window created': first('boot window created'),
                  'data ready': first('boot data ready'), 'app creation': first('boot app creation'),
                  'app fonts ready': first('boot fonts ready'),
                  'first window update': first('probe first window update'),
                  'visible window observed': first('probe visible window'),
                  'plan loaded': first('plan-ready'), 'fonts ready': first('fonts-ready'),
                  'data source bound': first('boot data bound'),
                  'first commit completed (runner returned)': first('runner-ready'),
                  'first layout completed': first('layout-ready'),
                  'first text shape start': first('text shape'),
                  'first text shape end': first('text shape', True),
                  'layer attached': first('boot layer attached'),
                  'window show returned': first('boot window shown'),
                  'first paint record': first('paint walk'),
                  'first paint record completed': first('paint walk', True),
                  'first present committed': present_commit,
                  'first drawable present request': min((v for v in [first('probe present commit', True),
                       first('probe direct present request')] if v is not None), default=None),
                  'first presented': result['firstDrawableMs'], 'complete screen': result['coldMs']}
    startup = result['startup']
    clocks = launch_clocks(startup)
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
                startup=startup, clocks=clocks, arm=result['arm'])


def launch_clocks(startup):
    clocks = {}
    for phase, lower, upper in [('process to constructor', None, 'constructor'),
                                ('process to main', None, 'main'),
                                ('main to complete callback', 'main', 'complete'),
                                ('constructor to complete callback', 'constructor', 'complete'),
                                ('process to complete callback', None, 'complete')]:
        if any(prefix + suffix not in startup for prefix in [lower, upper] if prefix
               for suffix in ['CpuSeconds', 'Cycles', 'Instructions']):
            continue
        def delta(suffix):
            return startup[upper + suffix] - (startup[lower + suffix] if lower else 0)
        cpu, cycles, instructions = delta('CpuSeconds'), delta('Cycles'), delta('Instructions')
        clocks[phase] = dict(cpuMs=cpu*1000, cycles=cycles, instructions=instructions,
                             effectiveGHz=cycles/cpu/1e9, ipc=instructions/cycles)
    return clocks


def summarize(args):
    directory = args.directory.resolve()
    schedule = read(directory / 'schedule.json')
    run_record = read(directory / 'run.json')
    assert run_record['quiet'] and not run_record['monitorError'], 'cohort is not quiet'
    assert len(schedule) == run_record['launches'] == run_record['expectedLaunches'], 'incomplete cohort'
    arms = run_record.get('arms', ['ink', 'gpui', 'egui'])
    cohorts = run_record.get('cohorts', ['cold', 'warm'])
    expected = {(arm, cohort, r) for arm in arms
                for cohort in cohorts for r in range(1, len(schedule) // (len(arms) * len(cohorts)) + 1)}
    assert {(r['arm'], r['cohort'], r['round']) for r in schedule} == expected, 'unbalanced cohort'
    assert all(r.get('accepted') and r.get('exit') == 0 for r in schedule), 'failed attempts retained; incomplete cohort'
    if run_record.get('qualification') == 'm10-cut1d-prelaunch':
        assert run_record['powerAbsent'], 'power sampler overlapped cohort'
        assert all(r['gate']['passed'] and r['gate']['seconds'] >= 10 and r['duringLoad']['observations'] for r in schedule)
    samples = [dict(sample(directory / Path(r['output']).name, r['trace']), arm=r['arm'], cohort=r['cohort'], round=r['round'], duringLoad=r.get('duringLoad')) for r in schedule]
    tables = {}
    for arm in arms:
        for cohort in cohorts:
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
            tables[f'{arm} {cohort}']['clocks'] = {
                phase: {metric: distribution([s['clocks'][phase][metric] for s in selected])
                        for metric in selected[0]['clocks'][phase]}
                for phase in selected[0].get('clocks', {})}
    paired = {}
    for arm in arms if 'warm' in cohorts else []:
        cold = {s['round']: s for s in samples if s['arm'] == arm and s['cohort'] == 'cold'}
        warm = {s['round']: s for s in samples if s['arm'] == arm and s['cohort'] == 'warm'}
        paired[arm] = {name: distribution([cold[r]['milestones'][name]-warm[r]['milestones'][name]
                                          for r in cold if cold[r]['milestones'][name] is not None
                                          and warm[r]['milestones'][name] is not None])
                       for name in cold[next(iter(cold))]['milestones']}
    write(directory / 'summary.json', dict(samples=samples, tables=tables, coldMinusWarm=paired))
    for row in samples:
        if row.get('duringLoad'):
            print('During-launch load (cores):', row['arm'], row['round'], row['cohort'], json.dumps(row['duringLoad']))
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
    launch.add_argument('--arms', default='egui,ink,gpui', help='provenance keys; ink-* are paired Ink variants')
    launch.add_argument('--scoreboard', action='store_true', help='measure.m cold-only, native/GPUI/Ink/egui')
    analyze = commands.add_parser('summarize')
    analyze.add_argument('directory', type=Path)
    args = parser.parse_args()
    (run if args.command == 'run' else summarize)(args)
