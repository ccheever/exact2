"""Run the hook comparison, or one unqualified instrument sample, on the silver Mac."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import subprocess
import threading
import time

BUSY_LIMIT = 2.0
PROCESS_LIMIT = 0.25
COMPILERS = {'cargo', 'rustc', 'swiftc', 'swift-frontend', 'clang', 'clang++', 'xcodebuild', 'metal', 'ld'}


def quiet_series(rows):
    """WindowServer is benchmark work; bounded daemon bursts are allowed per series."""
    if not rows:
        return False
    crossings = 0
    for row in rows:
        if max(row['load']) >= 30 or row['compilers'] or row['busyCores'] > BUSY_LIMIT:
            return False
        over = [p for p in row['topNonBenchmark'] if Path(p['command']).name != 'WindowServer'
                and p['cpuCores'] > PROCESS_LIMIT]
        crossings += bool(over)
        if any(p['cpuCores'] > 1.0 or not p['command'].startswith(
                ('/System/Library/', '/usr/libexec/', '/usr/sbin/', '/sbin/')) for p in over):
            return False
    return crossings / len(rows) <= 0.1


class QuietMonitor:
    def __init__(self, output, benchmark_paths):
        self.output = output
        self.benchmark_paths = set(map(str, benchmark_paths))
        self.quartz = ctypes.CDLL('/System/Library/Frameworks/QuartzCore.framework/QuartzCore')
        self.quartz.CACurrentMediaTime.restype = ctypes.c_double
        self.mach = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
        self.mach.mach_host_self.restype = ctypes.c_uint
        self.host = self.mach.mach_host_self()
        self.previous = self.ticks()
        self.previous_time = self.now()
        self.rows = []
        self.stop = threading.Event()
        self.error = None
        self.thread = threading.Thread(target=self.monitor)

    def now(self):
        return self.quartz.CACurrentMediaTime()

    def ticks(self):
        # HOST_CPU_LOAD_INFO sums ticks across CPUs, including tasks that exited.
        ticks = (ctypes.c_uint * 4)()
        count = ctypes.c_uint(4)
        status = self.mach.host_statistics(self.host, 3, ticks, ctypes.byref(count))
        assert status == 0 and count.value == 4, 'host_statistics CPU ticks unavailable'
        return list(ticks)

    def observe(self):
        current = self.ticks()
        now = self.now()
        delta = [(a - b) % 2**32 for a, b in zip(current, self.previous)]
        assert sum(delta) > 0, 'CPU observation interval too short'
        busy = os.cpu_count() * (sum(delta) - delta[2]) / sum(delta)
        elapsed = now - self.previous_time
        self.previous, self.previous_time = current, now
        lines = subprocess.check_output(['/bin/ps', '-axo', 'pid=,pcpu=,comm='], text=True)
        processes = []
        for line in lines.splitlines():
            pid, pcpu, command = line.strip().split(None, 2)
            processes.append(dict(pid=int(pid), cpuCores=float(pcpu)/100, command=command))
        builds = [p for p in processes if Path(p['command']).name in COMPILERS]
        largest = sorted((p for p in processes if p['command'] not in self.benchmark_paths
                          and Path(p['command']).name != 'WindowServer'),
                         key=lambda p: p['cpuCores'], reverse=True)[:5]
        load = list(os.getloadavg())
        row = dict(wall=time.time(), time=now, intervalSeconds=elapsed, load=load,
                   busyCores=busy, topNonBenchmark=largest, compilers=builds,
                   busyLimit=BUSY_LIMIT, nonBenchmarkLimit=PROCESS_LIMIT,
                   quiet=max(load) < 30 and not builds and busy <= BUSY_LIMIT
                   and largest[0]['cpuCores'] <= PROCESS_LIMIT)
        self.rows.append(row)
        with self.output.open('a') as stream:
            stream.write(json.dumps(row) + '\n')
        return row

    def monitor(self):
        try:
            while not self.stop.wait(5):
                self.observe()
        except BaseException as error:
            self.error = error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base', type=Path, help='prepared bundles, measure, exact-wheel and provenance.json')
    parser.add_argument('out', type=Path, help='fresh output directory')
    parser.add_argument('--single', action='store_true', help='one default Ink fling; retain even if loaded; no table')
    args = parser.parse_args()
    base, out = args.base.resolve(), args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    provenance = json.loads((base/'provenance.json').read_text())
    (out/'provenance.json').write_text(json.dumps(provenance, indent=2)+'\n')
    power_mode = {name: subprocess.check_output(['/usr/bin/pmset', '-g', *extra], text=True)
                  for name, extra in [('settings', []), ('source', ['batt'])]}
    (out/'power-mode.json').write_text(json.dumps(dict(wall=time.time(), **power_mode), indent=2)+'\n')
    assert subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1, 'powermetrics already running'
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(('EXACT_', 'G1_', 'G3_', 'G4_', 'G5_', 'BENCH_')) or key == 'DYLD_INSERT_LIBRARIES':
            env.pop(key)
    env.update(EXACT_INK_DEVICE='metal', BENCH_LIVE='0', G3_PAINTED_BIN=provenance['ink']['executable'])
    if args.single:
        env['G3_ONLY_INK'] = '1'
    else:
        env['G4_EGUI_BIN'] = provenance['egui']['executable']
    paths = [provenance[arm]['executable'] for arm in ['native', 'gpui', 'ink', 'egui']]
    monitor = QuietMonitor(out/'quiet.jsonl', paths + [base/'measure', base/'exact-wheel'])
    # First observation covers a full interval, rather than since boot.
    time.sleep(5)
    initial = monitor.observe()
    if not args.single:
        assert initial['quiet'], 'machine is not quiet; owner must stop the background work'
    monitor.thread.start()
    power = child = error_stream = None
    schedule = []
    try:
        runs = [(1, 'a')] if args.single else [(r, c) for r in range(1, 6)
                  for c in 'abc'[(r-1)%3:]+'abc'[:(r-1)%3]]
        for round_number, config in runs:
            assert monitor.error is None, monitor.error
            if not args.single:
                # The daemon allowance is assessed over the complete series.
                assert all(max(row['load']) < 30 and not row['compilers']
                           and row['busyCores'] <= BUSY_LIMIT for row in monitor.rows), 'loaded observations retained; stop series'
                env['G1_GPU_HOOK'] = '0' if config == 'a' else '1'
            folder = out/'sample' if args.single else out/config/'series/fling'
            folder.mkdir(parents=True, exist_ok=True)
            if not args.single:
                (out/config/'series/cold').mkdir(parents=True, exist_ok=True)
            record = dict(round=round_number, config=config, hook=env.get('G1_GPU_HOOK', 'driver default'),
                          power=config == 'c', start=monitor.now(), wall=time.time(), instrumentOnly=args.single)
            if config == 'c':
                command = ['sudo', '-n', '/usr/bin/powermetrics', '--samplers', 'cpu_power,gpu_power,tasks',
                           '--show-process-energy', '--show-process-gpu', '-i', '1000', '-n', '600',
                           '-f', 'plist', '-o', str(out/f'power-{round_number}.plist')]
                error_stream = (out/f'power-{round_number}.stderr').open('w')
                power = subprocess.Popen(command, stdout=error_stream, stderr=error_stream)
                record.update(powerPid=power.pid, powerCommand=command)
                deadline = time.monotonic()+5
                power_path = out/f'power-{round_number}.plist'
                while not power_path.exists() or power_path.stat().st_size == 0:
                    assert power.poll() is None and time.monotonic() < deadline, 'power sampler unavailable'
                    time.sleep(.05)
            command = [str(base/'measure'), str(folder), provenance['native']['executable'],
                       provenance['gpui']['executable'], str(base/'exact-wheel'), '1', str(round_number)]
            record['command'] = command
            with (out/f'{config}-{round_number}-driver.log').open('w') as log:
                child = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
                record['driverPid'] = child.pid
                schedule.append(record)
                (out/'schedule.json').write_text(json.dumps(schedule, indent=2)+'\n')
                print('START', config, round_number, 'pid', child.pid, flush=True)
                status = child.wait(timeout=360)
                assert status == 0, f'driver exit {status}; retain all attempts'
                child = None
            if power:
                assert power.poll() is None, 'power sampler stopped early'
                subprocess.run(['sudo', '-n', '/bin/launchctl', 'asuser', str(os.getuid()),
                                '/bin/kill', '-TERM', str(power.pid)], check=True)
                record['powerExit'] = power.wait(timeout=10)
                error_stream.close()
                power = None
            assert subprocess.run(['pgrep', '-x', 'powermetrics'], capture_output=True).returncode == 1, 'power sampler remains'
            record['end'] = monitor.now()
            (out/'schedule.json').write_text(json.dumps(schedule, indent=2)+'\n')
            print('FINISHED', config, round_number, flush=True)
    finally:
        if child and child.poll() is None:
            child.terminate()
            child.wait(timeout=5)
        if power and power.poll() is None:
            subprocess.run(['sudo', '-n', '/bin/launchctl', 'asuser', str(os.getuid()),
                            '/bin/kill', '-TERM', str(power.pid)], check=True)
            power.wait(timeout=10)
        if error_stream:
            error_stream.close()
        # Keep a complete five-second observation beyond the last phase endpoint.
        time.sleep(5)
        monitor.stop.set()
        monitor.thread.join()
    assert monitor.error is None, monitor.error
    if args.single:
        print('Instrument sample retained; not entered into a table.', flush=True)
    else:
        assert quiet_series(monitor.rows), 'loaded observations retained; no qualified table'
        print('Complete: five rounds of each configuration; amended quiet rule passes.', flush=True)


if __name__ == '__main__':
    main()
