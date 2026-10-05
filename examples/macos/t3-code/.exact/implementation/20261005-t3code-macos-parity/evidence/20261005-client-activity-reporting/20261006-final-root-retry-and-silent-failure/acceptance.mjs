import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
const base='target/t3-repair/activity-final';mkdirSync(base,{recursive:true});
function requests(){return readFileSync('target/t3-verify/trace/rpc.jsonl','utf8').trim().split('\n').map(x=>JSON.parse(x)).filter(x=>x.method==='server.refreshProviders'&&x.direction==='client-to-server');}
function selected(state){return {clock:state.clock,slots:Object.fromEntries(Object.entries(state.slots).filter(([key])=>key.startsWith('workspace'))),workspaceDiscovery:state.resources.workspaceDiscovery,pending:state.pending};}
function check(v,m){if(!v)throw new Error(m);}
export async function rootRetry(s){
 const steps=[]; const take=async name=>{const raw=await s.state();writeFileSync(`${base}/${name}-state.json`,JSON.stringify(raw));const value={name,at:new Date().toISOString(),...selected(raw),requests:requests()};steps.push(value);writeFileSync(`${base}/capture.json`,JSON.stringify({steps},null,2));return value;};
 const before=await take('before');check(before.workspaceDiscovery?.needed,'Selected provider/workspace needs an incomplete snapshot');
 // An absolute future clock avoids s.now being stale in a live host. Native input tests must be finished before this freezes the runner clock.
 await s.clock(Math.max(before.clock+1000,Number(before.slots.workspaceRetryAt||0)+1000));
 let start=await take('initial-completion');
 for(let i=0;i<30&&(start.pending.some(x=>x.name==='workspaceRefreshed')||start.slots.workspaceRetryAt!==start.clock+10000);i++){await new Promise(r=>setTimeout(r,100));start=await take('initial-completion-'+i);}
 check(start.slots.workspaceRefreshed?.retry===true,'Incomplete response requires retry');
 check(start.slots.workspaceRetryAt===start.clock+10000,'Deadline is completionclock+10000');
 const startClock=start.clock;const initialCount=start.requests.length;
 await s.clock(startClock+9000);const early=await take('before-deadline');
 check(early.requests.length===initialCount,'No request before deadline');
 check(early.slots.workspaceRetryAt===startClock+10000,'Deadline unchanged before due');
 await s.clock(startClock+10000);let due=await take('at-deadline');
 for(let i=0;i<30&&(due.pending.some(x=>x.name==='workspaceRefreshed')||due.slots.workspaceRetryAt!==due.clock+10000);i++){await new Promise(r=>setTimeout(r,100));due=await take('at-deadline-'+i);}
 check(due.requests.length===initialCount+1,'Exactly one request at deadline');
 check(due.slots.workspaceRetryAt===due.clock+10000,'Nextdeadline starts at completion');
 check(JSON.stringify(due.requests.at(-1).frame.payload)===JSON.stringify(start.requests.at(-1).frame.payload),'Sameworkspace/provider');
 const result={passed:true,startClock,beforeDeadline:early.clock,atDeadline:due.clock,initialCount,finalCount:due.requests.length,nextDeadline:due.slots.workspaceRetryAt};writeFileSync(`${base}/result.json`,JSON.stringify(result,null,2));return result;
}
export async function silenceBefore(s){
 const state=await s.state(),logs=await s.logs();check(state.resources.data.connected,'Server must be connected before controlled shutdown');
 const value={at:new Date().toISOString(),clock:state.clock,connected:state.resources.data.connected,toasts:state.resources.shell?.toasts??[],logs};
 writeFileSync(`${base}/silent-before.json`,JSON.stringify(value,null,2));return {connected:value.connected,at:value.at,toastIds:value.toasts.map(x=>x.id)};
}
export async function silenceAfter(s){
 const before=JSON.parse(readFileSync(`${base}/silent-before.json`));await new Promise(r=>setTimeout(r,27000));
 const state=await s.state(),logs=await s.logs(),tree=await s.tree();
 writeFileSync(`${base}/silent-after-state.json`,JSON.stringify(state));writeFileSync(`${base}/silent-after-tree.json`,JSON.stringify(tree));
 const toasts=state.resources.shell?.toasts??[],priorIds=new Set(before.toasts.map(x=>x.id));
 const value={at:new Date().toISOString(),elapsedMs:Date.now()-Date.parse(before.at),connected:state.resources.data.connected,error:state.resources.data.error,toasts,newToasts:toasts.filter(x=>!priorIds.has(x.id)),logs};
 writeFileSync(`${base}/silent-after.json`,JSON.stringify(value,null,2));
 check(!value.connected,'Actual backend shutdown must disconnect app');check(value.elapsedMs>=25000,'Observe at least one real heartbeat interval');check(value.newToasts.length===0,'Report failure must not add toast');
 check(![...(logs.lines??[]),...(logs.host??[])].some(line=>/reportClientActivity/i.test(String(line))&&/refus|fail|error/i.test(String(line))),'No activity-report refusal logged');
 return {passed:true,elapsedMs:value.elapsedMs,connected:value.connected,newToasts:value.newToasts.length};
}
