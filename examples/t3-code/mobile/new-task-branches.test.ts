// Regression cases from the captured draft/branch ownership review.
import { expect, test } from 'bun:test';
import { noteNow } from './shared/composer-controls';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskPrepare, mobileNewTaskAction, mobileNewTaskCachedPrepare, mobileNewTask } from './new-task';
import { fleet as productionFleet } from './shared/settings-b-fleet';
import { mobileCacheClear, mobileCacheClearKind } from './mobile-client-cache';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { draftContext, patchDraftContext } from './shared/composer-controls-branch';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as act } from './new-task-flow';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind, mobileNewTaskDraftRetarget,
  mobileNewTaskDraftPresentation, mobileNewTaskDraftSelectedBranch } from './mobile-new-task-drafts';

async function fixture() {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('not saved'); }, async atomicWriteFile() {} } };
  await client.command('dismiss-error', '', '', 0, { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } }, files);
  Object.assign(client, { environmentId: 'env', origin: 'https://example.test', projectId: 'a', connection: 'connected',
    configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
  noteNow(client, 1791420000000);
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
  client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  client.local.drafts['env:new:a'] = 'A original'; client.local.drafts['env:new:b'] = 'B original';
  let serial = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.op === 'ids' ? Array.from({length: Number(request.count)}, () => `id-${++serial}`) : request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] }
      : request.method === 'vcs.switchRef' ? { refName: 'normalized-branch' } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const snapshot = (location: string, visit = 'visit', session = 'flow', ready = true) => view(session, visit, location, true, ready, client, fleet);
  const action = (owner: string, kind: string, id = '', value = '', visit = 'visit') => act(owner, visit, kind, id, value, native, files, client, fleet);
  return { client, fleet, files, native, calls, snapshot, action };
}

async function selected() {
 const f=await fixture(),chooser=f.snapshot('/new');await f.action(chooser.owner,'project','["env","a"]');
 const original=f.native.later;
 f.native.later=async input=>{const request=obj(input);
  if(request.method==='vcs.refreshStatus')return {ok:true,generation:f.client.generation,value:{isRepo:true,refName:'main'}};
  if(request.method==='vcs.listRefs')return {ok:true,generation:f.client.generation,value:{refs:[{name:'main',current:true,isDefault:true}],total:1,nextCursor:null}};
  return original(input);
 };
 return f;
}
const branch=(f:Awaited<ReturnType<typeof selected>>)=>mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client,f.client.draftKey)!);

test('older prepare cannot demote newer same-branch explicit selection',async()=>{
 const f=await selected(),base=f.native.later,entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let first=true;
 f.native.later=async input=>{if(first&&obj(input).op==='http'){first=false;entered.resolve();await release.promise;}return base(input);};
 const older=mobileNewTaskPrepare('',f.native,f.client);await entered.promise;
 await mobileNewTaskPrepare('',f.native,f.client);await mobileNewTaskAction('branch','main','',f.native,f.files,f.client,f.fleet);
 expect(branch(f)).toBe('main');release.resolve();await older;
 expect(branch(f)).toBe('main');
});
test('explicit local branch survives actual switch to worktree mode',async()=>{
 const f=await selected();await mobileNewTaskPrepare('',f.native,f.client);await mobileNewTaskAction('branch','main','',f.native,f.files,f.client,f.fleet);
 await mobileNewTaskAction('workspace','','worktree',f.native,f.files,f.client,f.fleet);
 expect(draftContext(f.client).envMode).toBe('worktree');expect(draftContext(f.client).branch).toBe('main');
 expect(branch(f)).toBe('main');
});
test('default worktree base follows source selectBranch policy instead of null checkout fallback',async()=>{
 const f=await selected();patchDraftContext(f.client,{envMode:'worktree',branch:'',worktreePath:''});await mobileNewTaskPrepare('',f.native,f.client);
 expect(draftContext(f.client).branch).toBe('main');expect(branch(f)).toBe('main');
});
test('stale refs response cannot mutate another independent draft workspace',async()=>{
 const f=await selected(),base=f.native.later,entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let first=true;
 f.native.later=async input=>{if(first&&obj(input).method==='vcs.listRefs'){first=false;entered.resolve();await release.promise;}return base(input);};
 const older=mobileNewTaskPrepare('',f.native,f.client);await entered.promise;
 mobileNewTaskDraftCreate(f.client,{id:'other',environmentId:'env',projectId:'b',origin:f.client.origin,createdAt:'2026-10-08T00:00:00Z'});
 f.client.projectId='b';mobileNewTaskDraftBind(f.client,'new-task:other','other-flow');
 release.resolve();const answer=await older.catch(error=>{expect(error.kind).toBe('superseded');return {loaded:false};});expect(answer.loaded).toBe(false);
 expect(draftContext(f.client).branch).toBe('');
});

test('stale prepare cannot publish after same-key retarget or project root change',async()=>{
 for(const change of ['retarget','root']){
  const f=await selected(),base=f.native.later,entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let first=true;
  f.native.later=async input=>{if(first&&obj(input).method==='vcs.listRefs'){first=false;entered.resolve();await release.promise;}return base(input);};
  const key=f.client.draftKey,older=mobileNewTaskPrepare('',f.native,f.client);await entered.promise;
  if(change==='retarget'){
   mobileNewTaskDraftRetarget(f.client,key,{environmentId:'env',projectId:'b',origin:f.client.origin});f.client.projectId='b';
  }else{f.client.shell.projects[0].workspaceRoot='/new-root';}
  release.resolve();const answer=await older.catch(error=>{expect(error.kind).toBe('superseded');return {loaded:false};});
  expect(answer.loaded).toBe(false);expect(draftContext(f.client).branch).toBe('');
 }
});

test('worktree to local chooses actual current checkout rather than old feature base',async()=>{
 const f=await selected(),base=f.native.later,refs=[{name:'main',current:true,isDefault:true},{name:'feature',current:false}];
 f.native.later=async input=>obj(input).method==='vcs.listRefs'?{ok:true,generation:f.client.generation,value:{refs,total:2,nextCursor:null}}:base(input);
 patchDraftContext(f.client,{envMode:'worktree',branch:'',worktreePath:''});await mobileNewTaskPrepare('',f.native,f.client);
 expect((await mobileNewTaskAction('branch','feature','',f.native,f.files,f.client,f.fleet)).message).toBe('');expect(branch(f)).toBe('feature');
 expect((await mobileNewTaskAction('workspace','','local',f.native,f.files,f.client,f.fleet)).message).toBe('');
 // Pinned resolveNewTaskLocalWorkspaceSelection chooses the current main ref.
 const expected={branch:'main',worktreePath:null};
 expect(draftContext(f.client)).toEqual({envMode:'local',branch:expected.branch??'',worktreePath:expected.worktreePath??''});
 expect(branch(f)).toBe('main');
});

async function cachedBranches() {
 const f=await selected(),base=f.native.later,rows=new Map<string,Obj>(),calls:Obj[]=[];let epoch=0;
 f.fleet.saved=[{environmentId:f.client.environmentId,origin:f.client.origin}];
 const prior=productionFleet.saved;productionFleet.saved=f.fleet.saved;
 f.native.later=async input=>{
  const request=obj(input);calls.push(request);
  if(request.op==='mobileClientCache'){
   const key=JSON.stringify([request.environmentId,request.kind,request.key]);let value:Obj={};
   if(request.action==='ticket')value={ticket:String(epoch)};
   if(request.action==='write'){const stale=request.ticket!==String(epoch);if(!stale)rows.set(key,{environmentId:request.environmentId,kind:request.kind,key:request.key,payload:request.payload,schemaVersion:1,updatedAt:10});value={written:!stale,stale};}
   if(request.action==='read')value={record:rows.get(key)??null};
   if(request.action==='clear'||request.action==='clearKind'){epoch++;const removed=rows.size;rows.clear();value={removed};}
   if(request.action==='remove'){const removed=rows.get(key)?.payload===request.expectedPayload?1:0;if(removed)rows.delete(key);value={removed};}
   return {ok:true,generation:0,value};
  }
  if(request.method==='vcs.listRefs')return {ok:true,generation:f.client.generation,value:{refs:[{name:'main',current:true,isDefault:true,worktreePath:'/a'}],totalCount:1,nextCursor:null,isRepo:true,hasPrimaryRemote:true}};
  return base(input);
 };
 return {...f,rows,cacheCalls:calls,restore:()=>{productionFleet.saved=prior;}};
}
test('actual new-task lexical list persists once then offline cache phase renders read-only rows',async()=>{
 const f=await cachedBranches();try{
  await mobileNewTaskPrepare('',f.native,f.client);
  expect(f.cacheCalls.filter(row=>row.method==='vcs.listRefs')).toHaveLength(1);expect(f.rows.size).toBe(1);
  f.client.connection='disconnected';f.client.shellLive=false;f.cacheCalls.length=0;
  const before={context:draftContext(f.client),scopes:[...f.client.scopes],ready:f.client.ready};
  const prepared=await mobileNewTaskCachedPrepare('',f.native,f.client,()=>true,f.fleet);
  expect(prepared.loaded).toBe(true);
  const view=mobileNewTask('',f.client,f.fleet);expect(view.branches.map(row=>[row.id,row.disabled])).toEqual([['main',true]]);
  expect(f.cacheCalls.map(row=>[row.op,row.action])).toEqual([['mobileClientCache','read']]);
  expect({context:draftContext(f.client),scopes:[...f.client.scopes],ready:f.client.ready}).toEqual(before);
  expect((await mobileNewTaskAction('branch','main','',f.native,f.files,f.client,f.fleet)).message).toContain('cannot check out');
 }finally{f.restore();}
});
test('cached new-task rows disappear synchronously after cache clear or catalog replacement',async()=>{
 for(const transition of ['clear','catalog']){
  const f=await cachedBranches();try{
   await mobileNewTaskPrepare('',f.native,f.client);f.client.connection='disconnected';
   await mobileNewTaskCachedPrepare('',f.native,f.client,()=>true,f.fleet);
   expect(mobileNewTask('',f.client,f.fleet).branches).toHaveLength(1);
   if(transition==='clear')await mobileCacheClear(f.native,{environmentId:f.client.environmentId});
   else f.fleet.saved=[{environmentId:f.client.environmentId,origin:'https://replacement.test'}];
   expect(mobileNewTask('',f.client,f.fleet).branches).toEqual([]);
  }finally{f.restore();}
 }
});
test('cache phase obeys route ownership and never substitutes a filtered offline list',async()=>{
 const f=await cachedBranches();try{
  await mobileNewTaskPrepare('',f.native,f.client);f.client.connection='disconnected';f.cacheCalls.length=0;
  expect((await mobileNewTaskCachedPrepare('',f.native,f.client,()=>false,f.fleet)).loaded).toBe(false);
  expect((await mobileNewTaskCachedPrepare('main',f.native,f.client,()=>true,f.fleet)).loaded).toBe(false);
  expect(f.cacheCalls).toEqual([]);
  expect(mobileNewTask('',f.client,f.fleet).branches).toEqual([]);
  let current=true;const base=f.native.later;
  f.native.later=async input=>{const result=await base(input);if(obj(input).action==='read')current=false;return result;};
  expect((await mobileNewTaskCachedPrepare('',f.native,f.client,()=>current,f.fleet)).loaded).toBe(false);
 }finally{f.restore();}
});
test('connected new-task cache phase displays rows before delayed real refresh finishes',async()=>{
 const f=await cachedBranches();try{
  await mobileNewTaskPrepare('',f.native,f.client);f.client.generation++;
  expect((await mobileNewTaskCachedPrepare('',f.native,f.client,()=>true,f.fleet)).loaded).toBe(true);
  const base=f.native.later,entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();
  f.native.later=async input=>{if(obj(input).method==='vcs.listRefs'){entered.resolve();await release.promise;}return base(input);};
  const live=mobileNewTaskPrepare('',f.native,f.client);await entered.promise;
  expect(mobileNewTask('',f.client,f.fleet).branches.map(row=>[row.id,row.disabled])).toEqual([['main',true]]);
  release.resolve();await live;
  expect(mobileNewTask('',f.client,f.fleet).branches[0]?.disabled).toBe(false);
 }finally{f.restore();}
});
test.each(['none','environment','all','kind'] as const)('cold actual-client cached rows survive status invalidation; explicit %s clear still evicts',async clear=>{
 const f=await cachedBranches();try{
  // Populate disk through the real subtype, leaving private shared repository
  // state cold. A prior mobileNewTaskPrepare would mask refreshStatus here.
  await f.client.restAccess(f.native).request('vcs.listRefs',{cwd:'/a',limit:100});
  expect((await mobileNewTaskCachedPrepare('',f.native,f.client,()=>true,f.fleet)).loaded).toBe(true);
  const base=f.native.later,entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();
  f.native.later=async input=>{if(obj(input).method==='vcs.listRefs'){entered.resolve();await release.promise;}return base(input);};
  const live=mobileNewTaskPrepare('',f.native,f.client);await entered.promise;
  expect(f.cacheCalls.some(row=>row.method==='vcs.refreshStatus')).toBe(true);
  expect(f.cacheCalls.some(row=>row.action==='clear'&&row.kind==='vcs-refs')).toBe(true);
  expect(f.rows.size).toBe(0);
  expect(mobileNewTask('',f.client,f.fleet).branches.map(row=>[row.id,row.disabled])).toEqual([['main',true]]);
  if(clear==='environment')await mobileCacheClear(f.native,{environmentId:f.client.environmentId});
  if(clear==='all')await mobileCacheClear(f.native);
  if(clear==='kind')await mobileCacheClearKind(f.native,'vcs-refs');
  if(clear!=='none')expect(mobileNewTask('',f.client,f.fleet).branches).toEqual([]);
  release.resolve();await live;
  if(clear==='none')expect(mobileNewTask('',f.client,f.fleet).branches[0]?.disabled).toBe(false);
  else{expect(mobileNewTask('',f.client,f.fleet).branches).toEqual([]);expect(f.rows.size).toBe(0);}
 }finally{f.restore();}
});
