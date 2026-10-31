"""Stage 0 Python driver. Run from the repository root; inputs and outputs are retained under target/s0."""
import argparse, hashlib, json, os, pathlib, signal, socket, subprocess, time
ROOT=pathlib.Path(__file__).resolve().parents[3]
P=ROOT/'target/s0'
ap=argparse.ArgumentParser();ap.add_argument('name');ap.add_argument('--binary',required=True);ap.add_argument('--counters',action='store_true');ap.add_argument('--profile',action='store_true');ap.add_argument('--vmmap',action='store_true');ap.add_argument('--cpu-only',action='store_true');ap.add_argument('--dispatch',action='store_true');ap.add_argument('--clear',action='store_true');ap.add_argument('--busy',action='store_true');ap.add_argument('--delta',type=float,default=11.3);ap.add_argument('--vmmap-at',type=float,default=.2);ap.add_argument('--no-probe-display',action='store_true');ap.add_argument('--no-probe-timer',action='store_true');ap.add_argument('--wake-trace',action='store_true');ap.add_argument('--time-profiler',action='store_true');ap.add_argument('--hz',type=int,default=120);ap.add_argument('--busy-binary',default='metal-busy');ap.add_argument('--out-root',default=str(P));ap.add_argument('--device',default='metal',choices=['metal','wgpu']);ap.add_argument('--cache-dir');a=ap.parse_args()
out=pathlib.Path(a.out_root).resolve()/a.name;out.mkdir(exist_ok=False)
base=out/'ink.json';sockpath=f'/tmp/exact-s0-{os.getpid()}.sock'
env=os.environ.copy()
for k in list(env):
 if k.startswith(('EXACT_','G1_','BENCH_')):env.pop(k)
env.update(EXACT_AGENT='1',EXACT_AGENT_LIVE='1',EXACT_AGENT_SOCKET=sockpath,EXACT_INK_DEVICE=a.device,EXACT_PAINTER='ink',EXACT_WINDOW_WIDTH='800',EXACT_WINDOW_HEIGHT='768',EXACT_ASSETS=str(ROOT/'bench/heavy-list/exact-heavylist'),BENCH_LIVE='0',G1_TRACE='1',G1_ARM='ink',G1_OUTPUT=str(base),EXACT_AGENT_STORAGE='s0-'+a.name,EXACT_AGENT_STORAGE_FRESH='1')
if a.cache_dir:env['EXACT_CACHE']=str(pathlib.Path(a.cache_dir).resolve())
if a.counters:env['EXACT_INK_COUNTERS']='1'
if a.cpu_only:env['G1_AGENT_CPU_ONLY']='1'
env['G1_REST_TRACE']='1'
if a.dispatch:env['G1_DISPATCH_TRACE']='1'
if a.wake_trace:env['G1_WAKE_TRACE']='1'
if a.clear:env['G1_PRESENT_CLEAR']='1'
env['G1_THREAD_CPU']='1'
if a.no_probe_display:env['G1_NO_PROBE_DISPLAY']='1'
if a.no_probe_timer:env['G1_NO_PROBE_TIMER']='1'
if a.busy:env['G1_GPU_BUSY']='1'
assert pathlib.Path(a.binary).is_file(), a.binary
busy=None
if a.busy:
 busy=subprocess.Popen([str(P/a.busy_binary)],stdout=(out/'busy.jsonl').open('wb'),stderr=subprocess.STDOUT);(out/'busy.pid').write_text(str(busy.pid));time.sleep(1)
child=subprocess.Popen([a.binary],env=env,stdout=(out/'process.log').open('wb'),stderr=subprocess.STDOUT)
(out/'pid').write_text(str(child.pid)+'\n')
(out/'provenance.json').write_text(json.dumps(dict(binary=a.binary,sha256=hashlib.sha256(pathlib.Path(a.binary).read_bytes()).hexdigest(),probe_sha256=hashlib.sha256((pathlib.Path(a.binary).parent.parent/'Resources/benchmark-probe.dylib').read_bytes()).hexdigest(),git_head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),env={k:v for k,v in env.items() if k.startswith(('EXACT_','G1_','BENCH_'))}),indent=2))
def wait(test,timeout=40):
 end=time.monotonic()+timeout
 while not test():
  if child.poll() is not None:raise RuntimeError(f'child exited {child.returncode}')
  if time.monotonic()>end:raise RuntimeError('condition timed out')
  time.sleep(.05)
connection=None
import threading
finished=threading.Event()
def quiet():
 with (out/'quiet.jsonl').open('w',buffering=1) as f:
  while not finished.is_set():
   rows=subprocess.run(['ps','-axo','pid,pcpu,comm'],capture_output=True,text=True).stdout
   compilers=[l for l in rows.splitlines() if l.split() and pathlib.Path(l.split()[-1]).name in {'cargo','rustc','swiftc','clang','clang++','xcodebuild','metal','swift-frontend'}]
   f.write(json.dumps(dict(time=time.time(),load=os.getloadavg(),compilers=compilers))+'\n');finished.wait(1)
monitor=threading.Thread(target=quiet);monitor.start()
try:
 wait(lambda:pathlib.Path(sockpath).exists())
 connection=socket.socket(socket.AF_UNIX);connection.settimeout(20);connection.connect(sockpath);io=connection.makefile('rwb')
 ready=json.loads(io.readline());(out/'agent-ready.json').write_text(json.dumps(ready))
 def request(v):
  io.write((json.dumps(v)+'\n').encode());io.flush();r=json.loads(io.readline())
  if isinstance(r,dict) and 'error' in r:raise RuntimeError(r)
  return r
 (out/'state-boot.json').write_text(json.dumps(request({'op':'state','counters':True})))
 wait(lambda:pathlib.Path(str(base)+'.ready').exists())
 if a.counters:(out/'state-start.json').write_text(json.dumps(request({'op':'state','counters':True})))
 if a.time_profiler:
  (out/'clock.json').write_text(json.dumps(dict(wall=time.time(),monotonic=time.monotonic(),wall_minus_monotonic=time.time()-time.monotonic())))
  profiler=subprocess.Popen(['xcrun','xctrace','record','--template','Time Profiler','--attach',str(child.pid),'--time-limit','40s','--no-prompt','--output',str(out/'cpu.trace')],stdout=(out/'xctrace.log').open('wb'),stderr=subprocess.STDOUT)
  (out/'xctrace.pid').write_text(str(profiler.pid))
  wait(lambda:'Ctrl-C to stop' in (out/'xctrace.log').read_text(errors='replace') or 'Recording started' in (out/'xctrace.log').read_text(errors='replace'),30)
 child.send_signal(signal.SIGUSR2);wait(lambda:pathlib.Path(str(base)+'.fling').exists())
 if a.profile:
  profile=subprocess.Popen(['/usr/bin/sample',str(child.pid),'10','5','-file',str(out/'sample.txt')],stdout=(out/'sample.log').open('wb'),stderr=subprocess.STDOUT)
  (out/'sample.pid').write_text(str(profile.pid))
 start=time.perf_counter();events=[];gauges=[];mapped=False
 for i in range(a.hz*10):
  due=start+i/a.hz;delay=due-time.perf_counter()
  if delay>0:time.sleep(delay)
  phase='began' if i==0 else 'ended' if i==a.hz*10-1 else 'changed'
  before=time.perf_counter();answer=request({'op':'tap','at':[400,400],'wheel':[0,a.delta],'phase':phase});after=time.perf_counter()
  events.append(dict(due=due,start=before,end=after,answer=answer))
  if a.counters and i%12==0:gauges.append(dict(time=time.perf_counter(),state=request({'op':'state','counters':True})))
  if a.vmmap and i>=a.vmmap_at*a.hz and not mapped:
   vm=subprocess.Popen(['/usr/bin/vmmap','-summary',str(child.pid)],stdout=(out/'vmmap.txt').open('wb'),stderr=subprocess.STDOUT);mapped=True;(out/'vmmap.pid').write_text(str(vm.pid))
 end=time.perf_counter();(out/'drive.json').write_text(json.dumps(dict(start=start,end=end,events=events,gauges=gauges)))
 if a.counters:(out/'state-fling-end.json').write_text(json.dumps(request({'op':'state','counters':True})))
 child.send_signal(signal.SIGUSR2);wait(lambda:pathlib.Path(str(base)+'.rest').exists())
 rest=time.perf_counter()
 while time.perf_counter()-rest<10:time.sleep(min(.2,10-(time.perf_counter()-rest)))
 child.send_signal(signal.SIGUSR2);child.wait(timeout=20)
 if a.profile:profile.wait(timeout=30)
 if a.vmmap:vm.wait(timeout=30)
 if a.time_profiler:
  if profiler.poll() is None:profiler.send_signal(signal.SIGINT)
  profiler.wait(timeout=60)
 print(json.dumps(dict(name=a.name,code=child.returncode,seconds=end-start,events=len(events))))
 if child.returncode:raise RuntimeError(f'carrier exited {child.returncode}')
finally:
 finished.set();monitor.join()
 if busy:
  busy.terminate();busy.wait(timeout=15)
 if connection:connection.close()
 if child.poll() is None:
  child.terminate()
  try:child.wait(timeout=15)
  except subprocess.TimeoutExpired:child.kill();child.wait()
