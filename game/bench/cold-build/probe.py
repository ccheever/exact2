#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time, shutil, tempfile
mode = pathlib.Path(sys.argv[0]).name
folder = pathlib.Path(os.environ['D2_LOG'])
folder.mkdir(parents=True, exist_ok=True)
args = sys.argv[1:]
start = time.time()
load = os.getloadavg()
def save(row):
    row.update(start=start, end=time.time(), load=load, loadEnd=os.getloadavg(), cwd=os.getcwd(), pid=os.getpid())
    fd = os.open(folder / 'units.jsonl', os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
    os.write(fd, (json.dumps(row) + '\n').encode()); os.close(fd)
if mode == 'cargo':
    command = next((a for a in args if not a.startswith('+')), '')
    if command in ['build', 'rustc', 'check', 'test']:
        # A Cargo option; everything after the first `--` belongs to rustc or the test binary.
        tail = args.index('--') if '--' in args else len(args)
        args.insert(tail, '--timings')
    code = subprocess.call([os.environ['D2_CARGO']] + args, close_fds=False)
    save(dict(kind='cargo', args=args, code=code))
elif mode == 'rustc-probe' or args and pathlib.Path(args[0]).stem == 'rustc':
    mode = 'rustc-probe'
    compiled = '--crate-name' in args
    candidates = os.environ.get('D2_STOP_ON_REBUILD')
    if candidates and compiled:
        hashes = set(json.loads(pathlib.Path(candidates).read_text()))
        extra = next((arg.removeprefix('extra-filename=-') for arg in args if arg.startswith('extra-filename=-')), None)
        # Cargo may clear the completed fingerprint before launching rustc.
        # The caller snapshots completed copied hashes before starting the build.
        if extra in hashes:
            print('D2 CACHE REJECTED: Cargo invoked rustc for copied fingerprint ' + extra, file=sys.stderr, flush=True)
            save(dict(kind='cache-rejected', args=args, fingerprint=extra, code=86))
            sys.exit(86)
    cmd = args + (['-Ztime-passes', '-Ztime-passes-format=json'] if compiled else [])
    proc = subprocess.Popen(cmd, stderr=subprocess.PIPE, close_fds=False)
    phases = []
    for line in proc.stderr:
        try:
            row = json.loads(line.removeprefix(b'time: '))
            if 'pass' in row: phases.append(row); continue
        except (ValueError, TypeError): pass
        sys.stderr.buffer.write(line); sys.stderr.buffer.flush()
    code = proc.wait()
    save(dict(kind='rustc', args=args, phases=phases, code=code))
else:
    cargo = shutil.which('cargo')
    if not cargo: sys.exit('cargo must be on PATH')
    wrappers = tempfile.TemporaryDirectory(prefix='exact-d2-probe-')
    for name in ['cargo', 'rustc-probe']:
        pathlib.Path(wrappers.name, name).symlink_to(pathlib.Path(__file__).resolve())
    env = dict(os.environ, D2_CARGO=cargo, PATH=wrappers.name + ':' + os.environ['PATH'], RUSTC_WRAPPER=str(pathlib.Path(__file__).resolve()), RUSTC_BOOTSTRAP='1', CARGO_LOG='cargo::core::compiler::fingerprint=info')
    with open(folder / 'output.log', 'wb') as log:
        proc = subprocess.Popen(args, env=env, stdout=log, stderr=subprocess.STDOUT)
        (folder / 'pid.json').write_text(json.dumps(dict(monitor=os.getpid(), child=proc.pid,args=args)))
        print(json.dumps(dict(startedPid=proc.pid,monitorPid=os.getpid(),log=str(folder / 'output.log'))),flush=True)
        seen = {}
        while proc.poll() is None:
            now = time.time()
            rows = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,command='], text=True)
            processes = {}
            for line in rows.splitlines():
                bits = line.strip().split(None, 2)
                if len(bits) == 3: processes[int(bits[0])] = (int(bits[1]), bits[2])
            def ours(pid):
                visited = set()
                while pid in processes and pid not in visited:
                    if pid == proc.pid: return True
                    visited.add(pid); pid = processes[pid][0]
                return False
            for pid, (_, command) in processes.items():
                if 'wasm-opt ' in command and ours(pid):
                    if pid not in seen: seen[pid] = dict(first=now, last=now, command=command, sampleLoad=os.getloadavg())
                    seen[pid]['last'] = now
            time.sleep(0.2)
        code = proc.wait()
        for pid, row in seen.items(): save(dict(kind='wasm-opt-sampled', child=pid, **row))
    save(dict(kind='command', args=args, code=code))
    print(json.dumps(dict(code=code, elapsed=time.time()-start, load=load, loadEnd=os.getloadavg(), log=str(folder / 'output.log'))), flush=True)
if mode not in ['cargo', 'rustc-probe']: wrappers.cleanup()
sys.exit(code)
