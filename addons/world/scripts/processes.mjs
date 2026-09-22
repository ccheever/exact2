// Record only our children; every long-lived child is SIGKILLed and awaited.
import cp from 'node:child_process';
import {syncBuiltinESMExports} from 'node:module';
import {appendFileSync} from 'node:fs';
if (process.versions.bun) throw new Error("Run measurement/proof scripts with node: Bun does not synchronize patched spawn exports");
const children = new Map();
const original = cp.spawn;
cp.spawn = function(...args) {
  const child = original(...args);
  if (child.pid) {
    const row = {pid:child.pid, command:String(args[0]), args:args[1], detached:!!args[2]?.detached, at:new Date().toISOString()};
    appendFileSync(new URL('../evidence/pids.jsonl',import.meta.url),JSON.stringify(row)+'\n');
    const exited = new Promise(resolve=>child.once('exit',(code,signal)=>{row.code=code;row.signal=signal;resolve();}));
    children.set(child.pid,{child,exited,row});
  }
  return child;
};
const originalSync = cp.spawnSync;
cp.spawnSync = function(...args) {
  const result=originalSync(...args);
  if(result.pid)appendFileSync(new URL('../evidence/pids.jsonl',import.meta.url),JSON.stringify({pid:result.pid,command:String(args[0]),synchronous:true,reaped:result.pid,code:result.status,signal:result.signal})+'\n');
  return result;
};
syncBuiltinESMExports();
export async function killChildren() {
  for (const {child,exited,row} of children.values()) {
    if (child.exitCode===null && child.signalCode===null) {
      try {process.kill(row.detached ? -child.pid : child.pid,'SIGKILL');} catch(e) {if(e.code!=='ESRCH')throw e;}
    }
    await exited;
    appendFileSync(new URL('../evidence/pids.jsonl',import.meta.url),JSON.stringify({reaped:child.pid,signal:row.signal,code:row.code})+'\n');
  }
  children.clear();
}
