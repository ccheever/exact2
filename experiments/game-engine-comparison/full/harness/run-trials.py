import os,sys,json,time,shutil,subprocess,signal,hashlib,pathlib
ROOT=pathlib.Path('/tmp/exact-full-games-20260917')
engine=sys.argv[1]
base=ROOT/engine
frozen=ROOT/'frozen'/engine
outroot=ROOT/'trials'/engine
outroot.mkdir(parents=True,exist_ok=True)
ignore=shutil.ignore_patterns('node_modules','dist','runtime-libs','screenshots','evidence','.git','*.log','*.pid')
if frozen.exists(): raise SystemExit('Frozen directory already exists; refusing to overwrite')
shutil.copytree(base,frozen,ignore=ignore)
if (base/'node_modules').exists(): (frozen/'node_modules').symlink_to(base/'node_modules',target_is_directory=True)
def hashes(root):
 return {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*')) if p.is_file() and not any(x in p.parts for x in ['node_modules','.godot','dist'])}
(outroot/'baseline-hashes.json').write_text(json.dumps(hashes(frozen),indent=2))
def run(cmd,cwd,stdout,stderr,limit,env=None,stdin=None):
 t=time.time(); proc=subprocess.Popen(cmd,cwd=cwd,stdout=stdout,stderr=stderr,stdin=stdin,env=env,start_new_session=True)
 timeout=False
 try: code=proc.wait(timeout=limit)
 except subprocess.TimeoutExpired:
  timeout=True;os.killpg(proc.pid,signal.SIGTERM)
  try: code=proc.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);code=proc.wait()
 return {'pid':proc.pid,'started':t,'finished':time.time(),'exitCode':code,'timedOut':timeout}
for task,rep in [('a',1),('b',1),('b',2),('a',2)]:
 name=f'{task}{rep}'; work=outroot/name/'game'; evidence=outroot/name/'evidence'; evidence.mkdir(parents=True)
 shutil.copytree(frozen,work,symlinks=True)
 prompt=(ROOT/f'task-{task}.txt').read_text()
 prompt+='\nYour working directory is '+str(work)+'. Dependencies are warm; do not modify node_modules. Use bun run build. Shared game contract is BRIEF.md in this working directory.\n'
 shutil.copyfile(ROOT/'BRIEF.md',work/'BRIEF.md')
 env=os.environ.copy()
 if engine=='godot':
  env['LD_LIBRARY_PATH']=str(base/'runtime-libs/root/usr/lib/x86_64-linux-gnu')
  env['CHROMIUM']='/home/ccheever/.cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell'
  env['GODOT']='/tmp/exact-engine-comparison-20260917/native/Godot_v4.7.2-stable_linux.x86_64'
 (evidence/'prompt.txt').write_text(prompt)
 record={'engine':engine,'task':task,'replicate':rep,'model':'gpt-5.6-sol','effort':'high','dispatch':time.time()}
 print(json.dumps({'event':'dispatch',**record}),flush=True)
 with open(evidence/'prompt.txt') as inp,open(evidence/'agent.jsonl','w') as log,open(evidence/'agent.err','w') as err:
  record['agent']=run(['codex','exec','--skip-git-repo-check','--dangerously-bypass-approvals-and-sandbox','-m','gpt-5.6-sol','-c','model_reasoning_effort="high"','--json','-o',str(evidence/'final.md'),'-'],work,log,err,900,env,inp)
 with open(evidence/'build.log','w') as log:
  record['build']=run(['bun','run','build'],work,log,subprocess.STDOUT,180,env)
 if record['build']['exitCode']==0:
  server=subprocess.Popen(['bun',str(ROOT/'harness/serve.mjs'),str(work/'dist')],stdout=subprocess.PIPE,stderr=open(evidence/'server.err','w'),env={**env,'PORT':'0'},text=True,start_new_session=True)
  try:
   info=json.loads(server.stdout.readline());record['server']=info
   with open(evidence/'evaluation.log','w') as log:
    record['evaluation']=run(['bun','acceptance.mjs',f'url=http://127.0.0.1:{info["port"]}/?agent=1',f'engine={engine}',f'task={"extra" if task=="a" else "home"}',f'out={evidence}/evaluation'],ROOT/'harness',log,subprocess.STDOUT,600,env)
  except Exception as e:record['evaluationError']=str(e)
  finally:
   if server.poll() is None:os.killpg(server.pid,signal.SIGTERM);server.wait(timeout=10)
 record['sourceHashes']=hashes(work)
 record['finished']=time.time()
 with open(evidence/'changes.diff','w') as log:subprocess.run(['diff','-ru','--exclude=node_modules','--exclude=dist','--exclude=.godot',str(frozen),str(work)],stdout=log)
 (evidence/'record.json').write_text(json.dumps(record,indent=2))
 print(json.dumps({'event':'finished','engine':engine,'attempt':name,'agent':record['agent'],'evaluation':record.get('evaluation')}),flush=True)
