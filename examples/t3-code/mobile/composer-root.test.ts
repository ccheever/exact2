// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerRootSnapshot as snapshot,mobileComposerRootAction as action,type ComposerRootInput} from './composer-root';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileThreadLocalCommandsSnapshot} from './thread-send-local-commands';
import {mobileEditorFilesSnapshot} from './composer-file-runtime';
import {fleet} from './shared/settings-b-fleet';
import {setDraftFiles} from './shared/composer-editor-files';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
let serial=0;
const uuid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
function deferred<T>(){let resolve!:(v:T)=>void,reject!:(e:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
async function fixture(text='/mo') {
  const client=new MobileDraftClient(),environmentId=`composer-root-${++serial}`,calls:Obj[]=[],writes:Obj[]=[];
  let response:((request:Obj)=>Promise<unknown>)|null=null,writeHook:(()=>Promise<void>)|null=null;
  const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_path,bytes){
    writes.push(obj(JSON.parse(new TextDecoder().decode(bytes))));await writeHook?.();}}};
  const hydration=mobileDraftRecoveryHandles(client,{available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline fixture'}}}},storage);
  await client.refresh(hydration.native,hydration.storage);
  Object.assign(client,{origin:'https://composer-root.test',environmentId,threadId:'t',projectId:'p',generation:1,connection:'connected',providerId:'codex'});
  fleet.saved.push({environmentId,origin:client.origin});client.shell.projects=[{id:'p',workspaceRoot:'/repo',repositoryIdentity:{owner:'org',name:'repo'}}];
  client.shell.threads=[{id:'t',projectId:'p',title:'Current',latestUserMessageAt:null,modelSelection:{instanceId:'codex',model:'m'},latestRun:null}];
  client.config={providers:[{instanceId:'codex',driver:'codex',skills:[],slashCommands:[{name:'compact'}],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[{name:'compact'}]}]}]};
  client.local.drafts[client.draftKey]=text;
  const input:ComposerRootInput={active:true,visit:'visit',url:`/threads/${environmentId}/t`,environmentId,threadId:'t',editorId:'thread-composer',
    preferencesReady:true,preferencesJSON:JSON.stringify({planModeEnabled:false,followUpBehavior:'queue'}),scheme:'light',themeId:'t3-code',enterBehavior:'send',
    focusSerial:'sheet',focusAttempt:3,focusOperation:'focus',now:1000};
  const reply=(request:Obj):unknown=>{
    if(request.op==='composerEditorApply'){const c=obj(request.command),next=obj(c.next);return {ok:true,generation:1,value:{event:{...obj(request.identity),kind:'commandApplied',eventCount:Number(obj(c.expected).eventCount)+1,
      commandId:c.commandId,commandRevision:c.commandRevision,reason:'',value:next.value,selection:next.selection,composing:false,focused:true}}}}
    if(request.op==='composerFileHold')return {ok:true,generation:1,value:request.action==='acquire'?{status:'held',receipt:{identity:request.identity,requestId:request.requestId,
      holdId:uuid(900),fileIdentity:uuid(901),id:obj(request.file).id,sizeBytes:obj(request.file).sizeBytes}}:{status:'released',requestId:request.requestId}};
    if(request.op==='http')return {ok:true,generation:1,value:{authenticated:true,permissions:['filesystem:read']}};
    if(request.method==='projects.searchEntries')return {ok:true,generation:1,value:{entries:[{path:'src/a.ts',kind:'file'}]}};
    if(request.method==='pullRequests.detail')return {ok:true,generation:1,value:{}};
    if(request.method==='pullRequests.list')return {ok:true,generation:1,value:{entries:[],errors:[]}};
    if(request.method==='server.refreshProviders')return {ok:true,generation:1,value:client.config};
    throw Error(`Unexpected native request ${JSON.stringify(request)}`);
  };
  const native:Native={available:true,watch(){},async later(raw){const r=obj(raw);calls.push(r);return response?response(r):reply(r)}};
  const view=(raw='')=>snapshot(client,input,raw);
  const run=(op:string,key='',payload='',admission=view().admission,handle:Native|null=native)=>action(client,input,{op,key,payload,admission},handle,storage);
  const event=(kind:string,value=mobileEditorOwner(client)!.state.value,patch:Obj={})=>{const state=mobileEditorOwner(client)!.state;return JSON.stringify({...state.identity,mountId:state.mountId||'mount',kind,eventCount:state.eventCount+1,
    value,selection:{start:value.length,end:value.length},focused:true,composing:false,...patch})};
  const ready=async()=>{view();await run('event','',event('ready'));return view()};
  return {client,input,native,storage,calls,writes,view,run,event,ready,reply,respond:(fn:typeof response)=>{response=fn},holdWrite:(fn:typeof writeHook)=>{writeHook=fn}};
}

test('actual hydrated ordinary route supports offline draft, ready file gate and source native theme/focus props',async()=>{
  const f=await fixture('text');f.client.connection='disconnected';const first=f.view();expect(first.enabled).toBe(true);expect(first.files.ready).toBe(false);
  expect(JSON.parse(first.configuration)).toMatchObject({editable:false,readOnly:false,focusIntent:{serial:'sheet',attempt:3,operation:'focus'}});
  const v=await f.ready(),config=JSON.parse(v.configuration);expect(v.files.ready).toBe(true);expect(config.editable).toBe(true);
  expect(config.presentation).toMatchObject({fontSize:16,lineHeight:23,fontFamily:'DMSans-Regular',placeholder:'Ask the repo agent, or run a command…',enterBehavior:'send'});
  expect(typeof JSON.parse(config.presentation.themeJson)).toBe('object');expect(v.owner).toBe(mobileEditorOwner(f.client)!.target.owner);expect(f.calls).toHaveLength(0);
});
test('exact ordinary path, loaded preferences and catalog required; same-thread remount has new admission',async()=>{
  const f=await fixture();let v=await f.ready();const old=v.admission;
  for(const url of [`/threads/${f.input.environmentId}/t/files/x`,`/threads/${f.input.environmentId}/t/`,`https://foreign/threads/${f.input.environmentId}/t`,`/threads/${f.input.environmentId}/other`]){
    f.input.url=url;expect(f.view().enabled).toBe(false);
  }
  f.input.url=`/threads/${f.input.environmentId}/t`;f.input.preferencesReady=false;expect(f.view().enabled).toBe(false);
  f.input.preferencesReady=true;f.input.preferencesJSON='{}';expect(f.view().enabled).toBe(false);
  f.input.preferencesJSON=JSON.stringify({planModeEnabled:false,followUpBehavior:'queue'});f.input.visit='new-visit';v=f.view();expect(v.admission).not.toBe(old);
  await expect(f.run('event','', '{}',old)).rejects.toMatchObject({kind:'superseded'});expect(f.view().admission).toBe(v.admission);
  fleet.saved.find(s=>s.environmentId===f.input.environmentId)!.origin='https://replaced.test';expect(f.view().enabled).toBe(false);expect(f.calls).toHaveLength(0);
});
test('real UI pick uses captured menu revision and item ID, mode commands follow provider toggle without plan preference',async()=>{
  const f=await fixture('/');let v=await f.ready();expect(v.menu.rows.some(r=>r.id==='cmd:plan')).toBe(true);
  obj((f.client.config.providers as Obj[])[0]).showInteractionModeToggle=false;v=f.view();expect(v.menu.rows.some(r=>r.id==='cmd:plan')).toBe(false);
  await f.run('pick',v.menuRevision,'cmd:model',v.admission);expect(f.client.draft).toBe('/model ');expect(f.calls.map(c=>c.op)).toEqual(['composerEditorApply']);
  expect(f.view().effectId).toBe('');await expect(f.run('pick',v.menuRevision,'cmd:model',v.admission)).rejects.toMatchObject({kind:'superseded'});
});
test('raw event observation precedes persistence; independent text event publishes while query is held',async()=>{
  const f=await fixture('@src');const v=await f.ready(),query=deferred<unknown>(),started=deferred<void>();
  f.respond(async r=>{if(r.method==='projects.searchEntries'){started.resolve();return query.promise}return f.reply(r)});
  const pending=f.run('immediate',v.queries.immediateKey).then(value=>({value}),error=>({error}));await started.promise;
  const write=deferred<void>();f.holdWrite(()=>write.promise);const raw=f.event('text','new text');f.view(raw);
  expect(f.client.draft).toBe('@src');expect(JSON.parse(f.view().configuration).document.value).toBe('new text');
  const changed=f.run('event','',raw);expect(f.client.draft).toBe('new text');write.resolve();await changed;
  query.resolve({ok:true,generation:1,value:{entries:[{path:'old.ts',kind:'file'}]}});expect(await pending).toMatchObject({error:{kind:'superseded'}});expect(f.client.draft).toBe('new text');expect(f.view().menu.visible).toBe(false);
});
test('actual native submit alternate is claimed once; unrelated payload cannot forge effect',async()=>{
  const f=await fixture('text');await f.ready();await f.run('event','',f.event('submit',undefined,{alternate:true}));const v=f.view();
  expect(v.effectKind).toBe('submit');const claimed=await f.run('claim-effect',v.effectId,'false');expect(claimed).toMatchObject({effectKind:'submit',alternate:true,visit:'visit',admission:v.admission});
  expect((await f.run('claim-effect',v.effectId,'true')).effectId).toBe('');expect(f.calls).toHaveLength(0);
  await f.run('event','',f.event('blur',undefined,{focused:false}));const blur=f.view();expect((await f.run('claim-effect',blur.effectId)).effectKind).toBe('blur');
});
test('route change and catalog replacement fence held native command replies; exact letGo is preserved',async()=>{
  for(const change of ['route','catalog','permission'] as const){const f=await fixture();const v=await f.ready(),held=deferred<unknown>();f.respond(()=>held.promise);
    const work=f.run('pick',v.menuRevision,'cmd:model');if(change==='route'){f.input.visit='other';f.view()}
    else if(change==='catalog')fleet.saved.find(s=>s.environmentId===f.input.environmentId)!.origin='https://new.test';else f.client.scopes=['new-permission'];
    held.resolve(f.reply(f.calls[0]!));await expect(work).rejects.toMatchObject({kind:'superseded'});expect(f.client.draft).toBe('/mo');}
  const f=await fixture(),v=await f.ready(),sentinel={name:'FetchError',kind:'Aborted'};f.respond(async()=>{throw sentinel});
  await expect(f.run('pick',v.menuRevision,'cmd:model')).rejects.toBe(sentinel);expect(f.calls).toHaveLength(1);
});
test('unavailable native and unsupported rich callback report truthfully without claiming effects',async()=>{
  const f=await fixture();const v=await f.ready();expect((await f.run('pick',v.menuRevision,'cmd:model',v.admission,null)).message).toContain('unavailable');
  expect((await f.run('rich','','{}')).message).toContain('not integrated');expect(f.calls).toHaveLength(0);
  await expect(f.run('made-up')).rejects.toThrow('Unknown composer action');
});
test('source compact command uses visible V2 user feed and history, not hidden message storage',async()=>{
  const f=await fixture('/compact');f.client.thread={projection:{thread:{id:'t'},messages:[{role:'user',text:'hidden'}],visibleTurnItems:[]},sequence:0,historyCursor:null,hasMore:false,latestLocalTurnOrdinal:null};
  await f.ready();expect(f.view().menu.rows.some(r=>r.id==='pcmd:compact')).toBe(false);
  f.client.thread.projection.visibleTurnItems=[{sourceThreadId:'t',sourceItemId:'m',item:{type:'user_message',text:'/compact',attachments:[]}}];expect(f.view().menu.rows.some(r=>r.id==='pcmd:compact')).toBe(false);
  obj((f.client.thread.projection.visibleTurnItems as Obj[])[0]).item={type:'user_message',text:'hello',attachments:[]};expect(f.view().menu.rows.some(r=>r.id==='pcmd:compact')).toBe(true);
  f.client.thread.projection.visibleTurnItems=[];f.client.thread.hasMore=true;f.client.shell.threads[0]!.latestUserMessageAt='2026-10-09T00:00:00Z';expect(f.view().menu.rows.some(r=>r.id==='pcmd:compact')).toBe(true);
});
test('existing file icon resolver supplies restricted native identifiers for real mention tokens',async()=>{
  const f=await fixture('@"src/a.ts" ');await f.ready();const c=JSON.parse(f.view().configuration),tokens=JSON.parse(c.document.tokensJson);
  expect(tokens).toHaveLength(1);expect(tokens[0].iconUri).toBe('t3-bundled-icon:typescript');
  await f.run('event','',f.event('text','@"src/a.ts"'));const next=JSON.parse(JSON.parse(f.view().configuration).document.tokensJson);
  expect(next[0]?.iconUri).toBe('t3-bundled-icon:typescript');
});
test('saved file readiness requires actual native hold; explicit retry reuses uncertain request and retirement delegates only JS bookkeeping',async()=>{
  const f=await fixture('text');setDraftFiles(f.client.local,[{id:uuid(1),contextId:'file-context',draftKey:f.client.draftKey,environmentId:f.client.environmentId,name:'file.txt',mimeType:'text/plain',sizeBytes:5,source:'attached',attachmentId:'',status:'staged'}]);
  let first=true;f.respond(async r=>{if(r.op==='composerFileHold'&&first){first=false;throw Error('uncertain hold reply')}return f.reply(r)});
  let v=await f.ready();expect(v.files.ready).toBe(false);expect(v.files.needsPrepare).toBe(true);expect(JSON.parse(v.configuration).readOnly).toBe(false);
  expect((await f.run('prepare-files',v.files.prepareKey)).message).toContain('uncertain hold reply');v=f.view();expect(v.files.needsPrepare).toBe(false);
  await f.run('retry','files');v=f.view();expect(v.files.needsPrepare).toBe(true);await f.run('prepare-files',v.files.prepareKey);
  expect(f.view().files.ready).toBe(true);expect(f.calls[0]).toEqual(f.calls[1]);expect(JSON.parse(f.view().configuration).editable).toBe(true);
  const old=f.view().admission;f.input.active=false;v=f.view();expect(v.enabled).toBe(false);expect(v.files.cleanupKey).not.toBe('');expect(v.files).not.toHaveProperty('retiredAdmissions');
  await f.run('cleanup-files',v.files.cleanupKey,'','');expect(mobileEditorFilesSnapshot(f.client).retiredAdmissions).not.toContain(old);
  // Delegation neither sends a release nor claims native teardown. The native owner still protects until its real destroy.
  expect(f.calls).toHaveLength(2);expect(f.view().files.cleanupKey).toBe('');
});
test('actual usage capability controls menu and live native read uses real repository and permission scope',async()=>{
  const f=await fixture('/usage');await f.ready();expect(f.view().menu.rows.some(r=>r.id==='pcmd:usage-limits')).toBe(false);
  Object.assign(obj((f.client.config.providers as Obj[])[0]),{slashCommands:[{name:'usage-limits'}],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[{name:'usage-limits'}]}],enabled:true,installed:true,availability:'available',auth:{status:'authenticated'},
    usageLimits:{checkedAt:'2026-10-09T00:00:00Z',windows:[{id:'daily',kind:'session',label:'Daily',usedPercent:30}]}});
  expect(f.view().menu.rows.some(r=>r.id==='pcmd:usage-limits')).toBe(true);
  const usage=f.view();await f.run('pick',usage.menuRevision,'pcmd:usage-limits');expect(f.client.draft).toBe('');expect(f.calls).toHaveLength(1);
  expect(mobileThreadLocalCommandsSnapshot(f.client,{scope:{origin:f.client.origin,environmentId:f.client.environmentId,threadId:'t',draftKey:f.client.draftKey},
    usageKey:`${f.client.environmentId}:t:codex::`,instanceId:'codex',config:f.client.config,userInputActive:false}).usage).not.toBeNull();
  const g=await fixture('#7');let v=await g.ready();await g.run('immediate',v.queries.immediateKey);
  const query=g.calls.find(r=>r.method==='pullRequests.list');expect(query).toBeDefined();expect(obj(query?.payload).projectId).toBe('p');
  expect(obj(g.calls.find(r=>r.method==='pullRequests.detail')?.payload).repository).toBe('org/repo');
  await g.run('event','',g.event('text','#two'));v=g.view();expect(v.queries.pullRequests.delayMs).toBe(180);
  const before=g.calls.length;g.input.now=v.queries.pullRequests.dueAt-500;await g.run('wake',v.queries.pullRequests.key);expect(g.calls).toHaveLength(before);
  g.input.now=v.queries.pullRequests.dueAt+1;await g.run('wake',v.queries.pullRequests.key);expect(g.calls.length).toBeGreaterThan(before);
});
test('explicit root clock advances held query settlement and never regresses to an older minute snapshot',async()=>{
  const f=await fixture('text');obj((f.client.config.providers as Obj[])[0]).workspaceSnapshots=[];
  const v=await f.ready(),held=deferred<unknown>(),started=deferred<Obj>();expect(v.queries.immediateKey).not.toBe('');
  expect(v.queries.immediateClockOffset).toBe(0);
  f.respond(async r=>{if(r.method==='server.refreshProviders'){started.resolve(r);return held.promise}return f.reply(r)});
  const pending=f.run('immediate',v.queries.immediateKey).then(value=>({value}),error=>({error}));const request=await started.promise;
  f.input.now=7000;await f.run('claim-effect','absent');f.input.now=1000;f.view();
  // Only an explicitly pending workspace snapshot schedules the source timer;
  // absent snapshots retain a cooldown but wait for a new prompt/provider wake.
  held.resolve({ok:true,generation:request.generation,value:{providers:[{...obj((f.client.config.providers as Obj[])[0]),
    workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[],slashCommandsPending:true}]}]}});
  expect(await pending).not.toHaveProperty('error');
  const later=f.view();expect(later.admission).toBe(v.admission);expect(later.queries.discovery.dueAt).toBe(17000);
  expect(later.queries.discovery.delayMs).toBe(10000);
});
