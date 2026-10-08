#!/usr/bin/env python3
"""Four-way release benchmark: untraced cold-process ready, then native scroll trace.

Run only against an explicitly selected physical device. Cold means a new process,
not cleared OS/storage caches. Startup is renderer-ready after a Window draw, not
an independently observed display presentation. analyze.py owns presented FPS.
"""
import argparse
import hashlib
import json
import pathlib
import re
import shlex
import subprocess
import threading
import time
import uuid
import zipfile

RENDERERS = ('c9', 'main', 'views', 'compose')


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def save(path, value):
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value if isinstance(value, str) else json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def fields(data, partial=False):
    """Uncompressed Trace/TracePacket protobuf; optional incomplete trailing packet."""
    offset = 0
    def varint(pos):
        value = shift = 0
        while pos < len(data):
            byte = data[pos]; pos += 1
            value |= (byte & 127) << shift
            if byte < 128:
                return value, pos
            shift += 7
            if shift >= 70:
                raise ValueError('oversized protobuf varint')
        raise EOFError('truncated protobuf varint')
    while offset < len(data):
        try:
            key, pos = varint(offset)
            field, wire = key >> 3, key & 7
            if not field:
                raise ValueError('protobuf field zero')
            if wire == 0:
                value, pos = varint(pos)
            elif wire == 2:
                count, pos = varint(pos)
                if pos + count > len(data):
                    raise EOFError('truncated protobuf message')
                value = data[pos:pos + count]; pos += count
            elif wire in (1, 5):
                count = 8 if wire == 1 else 4
                if pos + count > len(data):
                    raise EOFError('truncated fixed protobuf field')
                value = data[pos:pos + count]; pos += count
            else:
                raise ValueError('unsupported protobuf wire type')
        except EOFError:
            if partial:
                return
            raise
        offset = pos
        yield field, value


def service_counters(path, partial=False):
    values = []
    for field, packet in fields(pathlib.Path(path).read_bytes(), partial):
        if field != 1:
            raise ValueError('unexpected trace field')
        packet = dict(fields(packet))
        if 35 in packet:
            stats = dict(fields(packet[35]))
            values.append({str(k): stats.get(k, 0) for k in (8, 9, 10)})
    return values


def load_products(folder):
    products = {}
    for variant in RENDERERS:
        path = pathlib.Path(folder) / variant / 'build-receipt.json'
        p = json.loads(path.read_text())
        assert p['release']['debuggable'] is False and p['release']['minified'] is True
        assert p['apk_sha256'] == sha(p['apk']), variant + ' APK differs from build receipt'
        with zipfile.ZipFile(p['apk']) as apk:
            descriptor = apk.read('assets/benchmark-assets.sha256')
            assert hashlib.sha256(descriptor).hexdigest() == p['dataset_sha256']
            dataset = json.loads(descriptor)
            assert dataset['messages_sha256'] == p['messages_sha256'] and dataset['images'] == p['images']
            assert len(p['images']) == 104, 'canonical heavy dataset requires104 JPEGs'
            for name, digest in p['images'].items():
                prefix = 'assets/assets/' if variant in ('c9', 'main') else 'assets/images/'
                assert hashlib.sha256(apk.read(prefix + name)).hexdigest() == digest
            if variant in ('views', 'compose'):
                assert hashlib.sha256(apk.read('assets/messages.json')).hexdigest() == p['messages_sha256']
        products[variant] = {**p, 'receipt_path': str(path.resolve()), 'receipt_sha256': sha(path)}
    assert len({p['appId'] for p in products.values()}) == 4
    assert len({p['dataset_sha256'] for p in products.values()}) == 1
    assert len({p['messages_sha256'] for p in products.values()}) == 1
    return products


def source_manifest():
    root = pathlib.Path(__file__).resolve().parents[3]
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--',
                                     'bench/heavy-list', 'host/android'], cwd=root).decode().split('\0')
    return {'root': str(root), 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(),
            'files': {p: sha(root / p) for p in sorted(set(paths)) if p and (root / p).is_file() and '__pycache__' not in pathlib.Path(p).parts and not p.endswith('.pyc')},
            'scope': 'Current tracked + nonignored untracked heavy benchmark and Android host files; APK binds exact executable. Main renderer build receipt has its separate source_head; generated bake inputs are not inferred from Git HEAD.'}


def markers(lines, token):
    result = {}
    for line in lines:
        match = re.search(r'\bHeavyBench\s*:\s*(Startup|Begin|Measure):(.*)$', line)
        if not match or token not in match.group(2):
            continue
        try:
            payload = json.loads(match.group(2))
        except json.JSONDecodeError as error:
            raise RuntimeError('Incomplete HeavyBench JSON (possibly Android log line limit); retain raw log') from error
        if payload.get('token') != token:
            continue
        prefix = re.match(r'^\d\d-\d\d\s+\S+\s+(\d+)\s+(\d+)\s+[A-Z]\s+', line)
        if not prefix:
            raise RuntimeError('Cannot bind HeavyBench line to raw logcat PID')
        payload['_capture_log_pid'] = int(prefix.group(1))
        result.setdefault(match.group(1), []).append(payload)
    for kind, rows in result.items():
        if len(rows) != 1:
            raise RuntimeError('Duplicate ' + kind + ' for ' + token)
    return {kind: rows[0] for kind, rows in result.items()}


class Device:
    def __init__(self, adb, serial):
        self.adb = [adb, '-s', serial]
    def run(self, *args, check=True, timeout=90):
        r = subprocess.run(self.adb + list(map(str, args)), capture_output=True, text=True, timeout=timeout)
        if check and r.returncode:
            raise RuntimeError(str(args) + '\n' + r.stdout + r.stderr)
        return r
    def shell(self, *args, **kw):
        return self.run('shell', ' '.join(shlex.quote(str(a)) for a in args), **kw)
    def stopped(self, products):
        for p in products.values():
            self.shell('am', 'force-stop', p['appId'])
        for p in products.values():
            assert not self.shell('pidof', p['appId'], check=False).stdout.strip(), 'app still running'
    def sessions(self, path):
        result = self.shell('perfetto', '--query').stdout
        save(path, result)
        match = re.search(r'Tracing sessions:\s*(\d+)', result)
        assert match, 'Cannot establish Perfetto session count; raw query retained'
        return int(match.group(1))
    def installed(self, product, path):
        paths = self.shell('pm', 'path', product['appId']).stdout.strip().splitlines()
        assert len(paths) == 1 and paths[0].startswith('package:'), 'require one monolithic APK'
        remote = paths[0][8:]
        digest = self.shell('sha256sum', remote).stdout.split()[0]
        assert digest == product['apk_sha256'], 'installed APK hash differs'
        result = {'apk_path': remote, 'apk_sha256': digest}
        save(path, result)
        return result
    def environment(self, folder):
        folder.mkdir()
        for label, command in [('thermal', ('dumpsys', 'thermalservice')), ('display', ('dumpsys', 'display')),
                               ('window', ('dumpsys', 'window', 'displays'))]:
            r = self.shell(*command, check=False)
            save(folder / (label + '.txt'), r.stdout + r.stderr)
        return {p.name: sha(p) for p in folder.iterdir()}
    def identity(self, pid):
        stat = self.shell('cat', f'/proc/{pid}/stat').stdout
        status = self.shell('cat', f'/proc/{pid}/status').stdout
        tail = stat[stat.rfind(')') + 2:].split()
        assert len(tail) >= 20
        return {'pid': pid, 'proc_start_ticks': int(tail[19]),
                'uid': int(re.search(r'^Uid:\s*(\d+)', status, re.M).group(1)),
                'stat': stat, 'status': status,
                'cmdline': self.shell('cat', f'/proc/{pid}/cmdline').stdout.replace('\0', ' ')}


def launch(device, products, variant, folder, token, args, speed=None):
    folder.mkdir()
    device.stopped(products)
    product = products[variant]
    installed = device.installed(product, folder / 'installed.json')
    environment = device.environment(folder / 'environment-before')
    proc = subprocess.Popen(device.adb + ['logcat', '-T', '1', '-v', 'threadtime', '-b', 'all', '-s',
                                         'HeavyBench:I', 'ActivityManager:I', 'AndroidRuntime:E', '*:S'],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    lines = []
    thread = threading.Thread(target=lambda: lines.extend(iter(proc.stdout.readline, '')), daemon=True)
    thread.start()
    result = {'renderer': variant, 'token': token, 'installed': installed, 'environment': environment}
    try:
        command = ['am', 'start', '-W', '-a', 'android.intent.action.MAIN', '-c', 'android.intent.category.LAUNCHER',
                   '-n', product['appId'] + '/' + product['activity'], '--es', 'BENCH_TOKEN', token,
                   '--ez', 'BENCH_LIVE', str(args.live).lower()]
        if speed is not None:
            command += ['--ef', 'BENCH_SPEED_DP_S', speed, '--el', 'BENCH_DURATION_MS', args.duration_ms,
                        '--el', 'BENCH_WARMUP_MS', args.warmup_ms]
        before = int(float(device.shell('cat', '/proc/uptime').stdout.split()[0]) * 1e9)
        result['launch_before_elapsed_ns_lower_bound'] = before
        started = device.shell(*command)
        save(folder / 'am-start.txt', started.stdout + started.stderr)
        assert 'Error:' not in started.stdout and 'Warning: Activity not started' not in started.stdout
        assert re.search(r'LaunchState:\s*COLD', started.stdout), 'am did not report a cold launch'
        pid_text = device.shell('pidof', product['appId']).stdout.strip()
        assert pid_text.isdecimal(), 'expected one application process'
        pid = int(pid_text)
        identity = device.identity(pid); save(folder / 'identity.json', identity)
        result.update(identity)
        until = time.monotonic() + args.timeout_s + (args.duration_ms + args.warmup_ms) / 1000
        required = 'Startup' if speed is None else 'Measure'
        while time.monotonic() < until:
            parsed = markers(list(lines), token)
            if required in parsed:
                break
            if any('FATAL EXCEPTION' in l and re.search(r'\s' + str(pid) + r'\s', l) for l in lines):
                raise RuntimeError('application fatal exception')
            time.sleep(.1)
        else:
            raise RuntimeError(required + ' timeout')
        parsed = markers(list(lines), token)
        assert parsed['Startup']['pid'] == pid and parsed['Startup']['renderer'] == variant
        assert all(m['_capture_log_pid'] == pid for m in parsed.values()), 'raw HeavyBench log PID differs'
        assert any(re.search(r'Start proc\s+' + str(pid) + ':' + re.escape(product['appId']) + r'/', l) for l in lines), 'missing ActivityManager Start proc proof'
        startup = parsed['Startup']; start = startup['process_start_elapsed_ms'] * 1_000_000
        assert start >= before - 20_000_000, 'process predates cold launch window'
        assert start <= startup['on_create_elapsed_ns'] <= startup['first_content_elapsed_ns'] <= startup['report_fully_drawn_elapsed_ns']
        assert startup['last_window_draw_elapsed_ns'] <= startup['first_content_elapsed_ns']
        result['markers'] = parsed
        result['process_to_ready_ms'] = (startup['first_content_elapsed_ns'] - start) / 1e6
        result['on_create_to_ready_ms'] = (startup['first_content_elapsed_ns'] - startup['on_create_elapsed_ns']) / 1e6
        if speed is not None:
            assert {'Startup', 'Begin', 'Measure'} <= parsed.keys()
            assert parsed['Begin']['start_elapsed_ns'] == parsed['Measure']['start_elapsed_ns']
            assert parsed['Measure']['start_elapsed_ns'] < parsed['Measure']['end_elapsed_ns']
            assert parsed['Measure']['speed_dp_s'] == speed and parsed['Begin']['speed_dp_s'] == speed
            assert parsed['Measure']['measured_duration_ms'] >= args.duration_ms - 2
        after = device.identity(pid)
        assert (after['uid'], after['proc_start_ticks']) == (identity['uid'], identity['proc_start_ticks'])
        result['status'] = 'COMPLETE'
        return result
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill(); proc.wait(timeout=5)
        thread.join(timeout=3)
        save(folder / 'logcat.txt', ''.join(lines)); save(folder / 'logcat-stderr.txt', proc.stderr.read())
        result['logcat_sha256'] = sha(folder / 'logcat.txt')
        if (folder / 'identity.json').exists():
            result['identity_sha256'] = sha(folder / 'identity.json')
        if (folder / 'am-start.txt').exists():
            result['am_start_sha256'] = sha(folder / 'am-start.txt')
        device.stopped(products)
        if args.cooldown_ms:
            time.sleep(args.cooldown_ms / 1000)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--products', required=True, type=pathlib.Path)
    p.add_argument('--serial', required=True)
    p.add_argument('--out', required=True, type=pathlib.Path)
    p.add_argument('--adb', default='adb')
    p.add_argument('--rounds', type=int, default=3)
    p.add_argument('--speeds', default='1000,3000,6000,12000,24000')
    p.add_argument('--duration-ms', type=int, default=8000)
    p.add_argument('--startup-reps', type=int, default=3)
    p.add_argument('--warmup-ms', type=int, default=1000)
    p.add_argument('--timeout-s', type=int, default=120)
    p.add_argument('--cooldown-ms', type=int, default=0)
    p.add_argument('--live', action='store_true', help='Enable the same authored live mutations in all products')
    p.add_argument('--plan-only', action='store_true', help='Verify local artifacts and write plan; never contact device')
    args = p.parse_args()
    assert args.rounds > 0 and args.startup_reps > 0 and 100 <= args.duration_ms <= 60000
    speeds = [float(x) for x in args.speeds.split(',')]
    assert speeds and all(x > 0 for x in speeds)
    assert 0 <= args.warmup_ms <= 30000 and 0 <= args.cooldown_ms <= 60000 and args.timeout_s > 0
    products = load_products(args.products)
    args.out.mkdir(parents=True, exist_ok=False)
    save(args.out / 'source-manifest.json', source_manifest())
    receipt = {'status': 'PREPARED', 'products': products, 'serial': args.serial, 'config': vars(args).copy(),
               'script_sha256': sha(__file__), 'source_manifest_sha256': sha(args.out / 'source-manifest.json'),
               'startup': [], 'scroll': [], 'art': {}, 'position_balance_note': 'Defaults of 3 rounds/repetitions are exploratory and not fully position-balanced. Use multiples of 4 for balanced renderer positions; actual order is retained.', 'scope': 'Four release apps; new process, warm OS caches; speed compile requested. Startup is untraced renderer-ready, scrolling uses only FrameTimeline + filtered log + process stats. No CPU sampling or video.'}
    receipt['config'] = {k: str(v) if isinstance(v, pathlib.Path) else v for k, v in receipt['config'].items()}
    scroll_plan = [(r, s, v) for r in range(args.rounds) for si, s in enumerate(speeds)
                   for v in RENDERERS[(r + si) % 4:] + RENDERERS[:(r + si) % 4]]
    receipt['planned_scroll_phases'] = len(scroll_plan)
    save(args.out / 'receipt.json', receipt)
    if args.plan_only:
        receipt['status'] = 'PLAN_ONLY'; save(args.out / 'receipt.json', receipt); return
    device = Device(args.adb, args.serial)
    recorder = None
    remote_config = '/data/misc/perfetto-configs/heavy-' + uuid.uuid4().hex + '.txtpb'
    remote_trace = remote_config.replace('perfetto-configs', 'perfetto-traces').replace('.txtpb', '.pftrace')
    boot = device.shell('cat', '/proc/sys/kernel/random/boot_id').stdout.strip()
    receipt['boot_id'] = boot
    seen = set()
    def record_identity(result):
        identity = (result['uid'], result['pid'], result['proc_start_ticks'])
        assert identity not in seen, 'process identity reused; not a fresh launch'
        seen.add(identity)
    try:
        assert device.sessions(args.out / 'service-before.txt') == 0, 'existing trace; do not interfere'
        save(args.out / 'device-properties.txt', device.shell('getprop').stdout)
        assert device.shell('getprop', 'ro.kernel.qemu').stdout.strip() != '1', 'physical Android device required'
        for variant, product in products.items():
            save(args.out / (variant + '-install.txt'), device.run('install', '--no-incremental', '-r', product['apk']).stdout)
            device.installed(product, args.out / (variant + '-installed.json'))
            compiled = device.shell('cmd', 'package', 'compile', '-f', '-m', 'speed', product['appId'])
            save(args.out / (variant + '-compile.txt'), compiled.stdout + compiled.stderr)
            assert 'Success' in compiled.stdout, 'ART speed compile did not report success'
            art = device.shell('cmd', 'art', 'dump', product['appId'], check=False)
            save(args.out / (variant + '-art-after.txt'), art.stdout + art.stderr)
            receipt['art'][variant] = {'compile_command_reported_success': True,
                'readback_exit_code': art.returncode,
                'speed_readback_observed': art.returncode == 0 and bool(re.search(r'status\s*[=:]\s*speed(?:[\s\]])', art.stdout)),
                'scope': 'Readback stored after requested speed compile; unavailable or nonmatching dump does not prove full speed AOT.'}
            save(args.out / 'receipt.json', receipt)
        for r in range(args.startup_reps):
            order = RENDERERS[r % 4:] + RENDERERS[:r % 4]
            for variant in order:
                token = 'heavy-start-' + uuid.uuid4().hex
                folder = args.out / f'startup-{r}-{variant}'
                result = launch(device, products, variant, folder, token, args)
                result.update(round=r, folder=folder.name); record_identity(result)
                receipt['startup'].append(result); save(args.out / 'receipt.json', receipt)
                print('startup', r, variant, result['process_to_ready_ms'], flush=True)
        duration = len(scroll_plan) * (args.duration_ms + args.warmup_ms + args.timeout_s * 1000 + args.cooldown_ms) + 60000
        config = f'''duration_ms: {duration}
write_into_file: true
file_write_period_ms: 1000
flush_period_ms: 1000
buffers {{ size_kb: 65536 fill_policy: RING_BUFFER }}
data_sources {{ config {{ name: "android.surfaceflinger.frametimeline" }} }}
data_sources {{ config {{ name: "android.log" android_log_config {{ log_ids: LID_DEFAULT filter_tags: "HeavyBench" }} }} }}
data_sources {{ config {{ name: "linux.process_stats" process_stats_config {{ scan_all_processes_on_start: true proc_stats_poll_ms: 1000 }} }} }}
'''
        save(args.out / 'config.txtpb', config)
        assert device.sessions(args.out / 'service-before-scroll.txt') == 0
        device.stopped(products); device.run('push', args.out / 'config.txtpb', remote_config)
        start = device.shell('perfetto', '--background-wait', '--txt', '-c', remote_config, '-o', remote_trace)
        save(args.out / 'recorder-start.txt', start.stdout + start.stderr)
        ids = re.findall(r'^\d+$', start.stdout, re.M); assert len(ids) == 1
        recorder = device.identity(int(ids[0])); assert 'perfetto' in recorder['cmdline'] and remote_trace in recorder['cmdline']
        receipt['recorder'] = recorder; receipt['remote_trace'] = remote_trace
        save(args.out / 'receipt.json', receipt)
        for attempt in range(6):
            time.sleep(1)
            device.run('pull', remote_trace, args.out / 'initial-prefix.pftrace')
            counters = service_counters(args.out / 'initial-prefix.pftrace', partial=True)
            if counters:
                break
        assert counters and all(c == counters[0] for c in counters), 'no stable pre-app service counter'
        receipt['service_counter_baseline'] = counters[0]
        receipt['initial_prefix_sha256'] = sha(args.out / 'initial-prefix.pftrace')
        save(args.out / 'receipt.json', receipt)
        for index, (r, speed, variant) in enumerate(scroll_plan):
            folder = args.out / f'scroll-{index:03d}-{variant}'
            result = launch(device, products, variant, folder, 'heavy-scroll-' + uuid.uuid4().hex, args, speed)
            result.update(round=r, speed_dp_s=speed, slot=index % 4, folder=folder.name); record_identity(result)
            receipt['scroll'].append(result); save(args.out / 'receipt.json', receipt)
            print('scroll', r, speed, variant, 'complete', flush=True)
        assert device.shell('cat', '/proc/sys/kernel/random/boot_id').stdout.strip() == boot
        receipt['status'] = 'CAPTURE_COMPLETE_PENDING_TIMELINE_AUDIT'
    except BaseException as error:
        receipt['status'] = 'FAILED_PRESERVE_EVIDENCE'; receipt['error'] = repr(error)
        raise
    finally:
        try:
            if recorder:
                stat = device.shell('cat', f"/proc/{recorder['pid']}/stat", check=False)
                if stat.returncode == 0 and stat.stdout[stat.stdout.rfind(')') + 2:].split()[0] != 'Z':
                    now = device.identity(recorder['pid'])
                    assert now['proc_start_ticks'] == recorder['proc_start_ticks'] and remote_trace in now['cmdline'], 'recorder PID changed; refuse signal'
                    device.shell('kill', '-INT', recorder['pid'])
                until = time.monotonic() + 30
                while time.monotonic() < until:
                    stat = device.shell('cat', f"/proc/{recorder['pid']}/stat", check=False)
                    if stat.returncode or stat.stdout[stat.stdout.rfind(')') + 2:].split()[0] == 'Z':
                        break
                    time.sleep(.2)
                else:
                    raise RuntimeError('owned recorder did not exit')
                receipt['recorder_stopped'] = True
                device.run('pull', remote_trace, args.out / 'timeline.pftrace')
                receipt['trace_sha256'] = sha(args.out / 'timeline.pftrace')
                device.shell('rm', '-f', remote_config, remote_trace)
            device.stopped(products)
            receipt['service_after_sessions'] = device.sessions(args.out / 'service-after.txt')
            assert receipt['service_after_sessions'] == 0
        except BaseException as error:
            receipt['cleanup_error'] = repr(error); receipt['status'] = 'FAILED_PRESERVE_EVIDENCE'
            raise
        finally:
            receipt['finished_unix_s'] = time.time(); save(args.out / 'receipt.json', receipt)


if __name__ == '__main__':
    main()
