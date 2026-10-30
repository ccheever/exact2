"""Stage 0 Python driver. Run from the repository root; inputs and outputs are retained under target/s0."""
import importlib,json,os,subprocess,sys,time
from pathlib import Path
root=Path(__file__).resolve().parents[3];os.chdir(root)
sys.path.insert(0,str(root/'bench/heavy-list/egui'));m=importlib.import_module('windows-measure')
out=root/'target/s0/windows';out.mkdir(exist_ok=True)
original_popen=subprocess.Popen
original_snapshot=m.snapshot
for mode in ['attempt']:
 dest=out/mode;dest.mkdir(exist_ok=False);calls=0;active=False
 def popen(args,*rest,**kwargs):
  if Path(str(args[0])).name=='Heavy list.exe' and mode!='off':
   env=kwargs['env'].copy();env.update(EXACT_INK_COUNTERS='1',EXACT_INK_COUNTER_OUTPUT=str(dest/'counters.json'));kwargs['env']=env
  return original_popen(args,*rest,**kwargs)
 subprocess.Popen=popen
 def snapshot(process,tid):
  global calls,active
  calls+=1
  if mode=='sampled' and calls==1:
   with (dest/'xperf-start.log').open('wb') as f:subprocess.run(['xperf','-on','PROC_THREAD+LOADER+PROFILE','-stackwalk','Profile','-buffersize','1024','-minbuffers','128','-maxbuffers','256','-f',str(dest/'kernel.etl')],stdout=f,stderr=subprocess.STDOUT,check=True)
   active=True
  result=original_snapshot(process,tid);result['wallUnix']=time.time();result['threads']=[dict(id=t.id,cpu=t.user_time+t.system_time) for t in process.threads()]
  if mode=='sampled' and calls==2:
   with (dest/'xperf-stop.log').open('wb') as f:subprocess.run(['xperf','-d',str(dest/'fling.etl')],stdout=f,stderr=subprocess.STDOUT,check=True)
   active=False
  return result
 m.snapshot=snapshot
 m.wait_quiet(dest)
 try:m.run('d3d11',0,dest,True,True,False)
 finally:
  if active:subprocess.run(['xperf','-d',str(dest/'fling.etl')],check=True)
  subprocess.Popen=original_popen
 print(mode,'complete',flush=True)
