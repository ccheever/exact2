import os,sys,pathlib,subprocess,json,time,hashlib,signal
root=pathlib.Path('/tmp/exact-full-games-20260917');engine,attempt=sys.argv[1:3]
work=root/'trials'/engine/attempt/'game';out=work.parent/'evidence'/'evaluation-v3'
out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy()
if engine=='godot':
 env['LD_LIBRARY_PATH']=str(root/'godot/runtime-libs/root/usr/lib/x86_64-linux-gnu');env['CHROMIUM']='/home/ccheever/.cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell'
server=subprocess.Popen(['bun',str(root/'harness/serve.mjs'),str(work/'dist')],stdout=subprocess.PIPE,stderr=open(out/'server.err','w'),text=True,env={**env,'PORT':'0'},start_new_session=True)
record={'engine':engine,'attempt':attempt,'started':time.time(),'evaluatorSha256':hashlib.sha256((root/'harness/acceptance.mjs').read_bytes()).hexdigest()}
try:
 info=json.loads(server.stdout.readline());record['server']=info
 with open(out/'evaluation.log','w') as log:
  p=subprocess.Popen(['bun','acceptance.mjs',f'url=http://127.0.0.1:{info["port"]}/?agent=1',f'engine={engine}',f'task={"extra" if attempt[0]=="a" else "home"}',f'out={out}'],cwd=root/'harness',stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
  record['pid']=p.pid
  try:record['exitCode']=p.wait(timeout=600)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);record['exitCode']=p.wait(timeout=10);record['timedOut']=True
finally:
 if server.poll() is None:os.killpg(server.pid,signal.SIGTERM);server.wait(timeout=10)
 record['finished']=time.time();(out/'rerun.json').write_text(json.dumps(record,indent=2));print(json.dumps(record))
