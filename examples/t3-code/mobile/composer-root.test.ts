// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerRootSnapshot as snapshot,mobileComposerRootAction as action,type ComposerRootInput} from './composer-root';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileComposerContextCapture,mobileComposerContextCommit} from './composer-command-context';
import {mobileComposerTarget} from './composer-target';
import {mobileThreadLocalCommandsSnapshot} from './thread-send-local-commands';
import {mobileEditorFilesSnapshot} from './composer-file-runtime';
import {fleet} from './shared/settings-b-fleet';
import {setDraftFiles} from './shared/composer-editor-files';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
import {contractComponent,type Env} from '../tests/support/contract-action-fixture';
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
  const ready=async()=>{view();await run('rich','',event('ready'));return view()};
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
  expect(f.view().eventKey).not.toBe('');
  const changed=f.run('event','',raw);expect(f.client.draft).toBe('new text');write.resolve();await changed;
  expect(f.view().eventKey).toBe('');
  query.resolve({ok:true,generation:1,value:{entries:[{path:'old.ts',kind:'file'}]}});expect(await pending).toMatchObject({error:{kind:'superseded'}});expect(f.client.draft).toBe('new text');expect(f.view().menu.visible).toBe(false);
});
test('actual Root retains the latest native text while the mutation queue refuses more sends',async()=>{
  const f=await fixture('');await f.ready();const root=contractComponent('mobile/app.contract','App');
  const state:Env={richComposerLatch:''},work:Promise<unknown>[]=[];
  let full=false,sends=0;
  const scope=()=>({richComposerInput:f.input,richComposer:f.view(),wallTime:{epochAtZero:0},performanceNow:()=>f.input.now,
    composerRootAction:(input:ComposerRootInput,op:string,admission:string,key:string,payload:string,now:number)=>{
      if(full)throw Error('QueueFull');sends++;
      const pending=action(f.client,{...input,now},{op,admission,key,payload},f.native,f.storage);work.push(pending);return pending;
    }});
  const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
  let text='a',latest=f.event('text',text);root.call('richComposerEvent',state,scope(),[latest]);f.view(String(state.richComposerLatch));
  root.call('flushRichComposer',state,scope(),[]);await entered.promise;full=true;
  for(let index=1;index<124;index++){
    text+='x';latest=f.event('text',text);root.call('richComposerEvent',state,scope(),[latest]);f.view(String(state.richComposerLatch));
  }
  expect(state.richComposerLatch).toBe(latest);expect(sends).toBe(1);expect(f.view().eventKey).not.toBe('');
  held.resolve();await work[0];full=false;f.holdWrite(null);
  root.call('flushRichComposer',state,scope(),[]);await work.at(-1);
  expect(f.client.draft).toBe(text);expect(obj(f.writes.at(-1)?.drafts)[f.client.draftKey]).toBe(text);
  expect(f.view().eventKey).toBe('');f.input.active=false;expect(f.view().eventKey).toBe('');
});
test('actual native submit alternate is claimed once; unrelated payload cannot forge effect',async()=>{
  const f=await fixture('text');await f.ready();await f.run('rich','',f.event('submit',undefined,{alternate:true}));const v=f.view();
  expect(v.effectKind).toBe('submit');const claimed=await f.run('claim-effect',v.effectId,'false');expect(claimed).toMatchObject({effectKind:'submit',alternate:true,visit:'visit',admission:v.admission});
  expect((await f.run('claim-effect',v.effectId,'true')).effectId).toBe('');expect(f.calls).toHaveLength(0);
  await f.run('rich','',f.event('blur',undefined,{focused:false}));const blur=f.view();expect((await f.run('claim-effect',blur.effectId)).effectKind).toBe('blur');
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

test('inline file context opens the owned draft preview through Root and refuses stale token callbacks',async()=>{
  const source='[file.txt](t3-context://v1/file/file-context)',text=`😀 ${source} `;
  const f=await fixture(text),id=uuid(1);
  setDraftFiles(f.client.local,[{id,contextId:'file-context',draftKey:f.client.draftKey,environmentId:f.client.environmentId,
    name:'file.txt',mimeType:'text/plain',sizeBytes:5,source:'attached',attachmentId:'',status:'staged'}]);
  await f.ready();const view=f.view();await f.run('prepare-files',view.files.prepareKey);
  const latest=`${text}rapid edit 😀`,observation=f.event('text',latest);f.view(observation);
  expect(f.client.draft).toBe(text);expect(f.view().eventKey).not.toBe('');
  const owner=mobileEditorOwner(f.client)!,callback={...owner.state.identity,mountId:owner.state.mountId,kind:'contextPress',
    richEventId:'tap-file',eventCount:owner.state.eventCount,value:latest,payload:{source,start:3,end:3+source.length}};
  const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
  let finished=false;const opening=f.run('rich','',JSON.stringify(callback)).then(result=>{finished=true;return result});
  await entered.promise;expect(finished).toBe(false);expect(f.client.draft).toBe(latest);
  held.resolve();const result=await opening;f.holdWrite(null);
  expect(obj(f.writes.at(-1)?.drafts)[f.client.draftKey]).toBe(latest);
  expect(result).toMatchObject({attachmentId:id,message:'',owner:owner.target.owner,visit:f.input.visit});
  const root=contractComponent('mobile/app.contract','App'),opened:string[][]=[];
  const scope={richComposerChanged:result,richComposer:f.view(),composerEditor:f.input.editorId,composerOwner:owner.target.owner,
    nav:{id:f.input.visit},top:(v:unknown)=>v,nativeNewTask:false,showMobileNotice:()=>{},showDraftMedia:(...args:string[])=>opened.push(args),
    composerReply:(reply:unknown)=>reply};
  root.call('didRichComposerChange',{},scope,[]);expect(opened).toEqual([[id,owner.target.owner]]);
  root.call('didRichComposerChange',{}, {...scope,nav:{id:'other-visit'}},[]);expect(opened).toHaveLength(1);
  for(const patch of [{mountId:'retired'},{eventCount:callback.eventCount-1},{value:'changed'},
    {payload:{...callback.payload,start:2}},{payload:{...callback.payload,end:source.length}},
    {payload:{...callback.payload,source:'[foreign](t3-context://v1/file/foreign)'}}]){
    await expect(f.run('rich','',JSON.stringify({...callback,...patch}))).rejects.toMatchObject({kind:'superseded'});
  }
  setDraftFiles(f.client.local,[]);await expect(f.run('rich','',JSON.stringify(callback))).rejects.toMatchObject({kind:'superseded'});
  expect(f.client.draft).toBe(latest);expect(f.writes).toHaveLength(1);
  f.input.active=false;f.view();f.input.active=true;f.input.visit='returned';await f.ready();
  expect(JSON.parse(f.view().configuration).document.value).toBe(latest);
  expect(f.calls.every(call=>call.op==='composerFileHold')).toBe(true);
});

test('preview revalidates typing, route and file ownership after saving observed text',async()=>{
  for(const change of ['typing','route','file'] as const){
    const source='[file.txt](t3-context://v1/file/file-context)',f=await fixture(`${source} `),id=uuid(1);
    setDraftFiles(f.client.local,[{id,contextId:'file-context',draftKey:f.client.draftKey,environmentId:f.client.environmentId,
      name:'file.txt',mimeType:'text/plain',sizeBytes:5,source:'attached',attachmentId:'',status:'staged'}]);
    await f.ready();const view=f.view();await f.run('prepare-files',view.files.prepareKey);
    const value=`${source} latest`;f.view(f.event('text',value));const owner=mobileEditorOwner(f.client)!;
    const callback={...owner.state.identity,mountId:owner.state.mountId,kind:'contextPress',richEventId:'tap',
      eventCount:owner.state.eventCount,value,payload:{source,start:0,end:source.length}};
    const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
    const opening=f.run('rich','',JSON.stringify(callback));await entered.promise;
    if(change==='typing')f.view(f.event('text',`${value} newer`));
    else if(change==='route'){f.input.active=false;f.view()}
    else setDraftFiles(f.client.local,[]);
    held.resolve();await expect(opening).rejects.toMatchObject({kind:'superseded'});
  }
});

async function imagePreviewFixture(storage:'snapshot'|'file'){
  const source='![Photo](t3-context://v1/image/photo-reference)',text=`😀 ${source} `,f=await fixture(text),id=uuid(2);
  if(storage==='snapshot')f.client.local.snapshotDrafts[f.client.draftKey]=[{id,name:'photo.png',mimeType:'image/png',sizeBytes:5}];
  else setDraftFiles(f.client.local,[{id,contextId:'file-storage',draftKey:f.client.draftKey,environmentId:f.client.environmentId,
    name:'photo.png',mimeType:'image/png',sizeBytes:5,source:'attached',attachmentId:'',status:'staged'}]);
  expect(mobileComposerContextCommit(f.client,mobileComposerContextCapture(f.client,mobileComposerTarget(f.client))!,text,
    {version:1,kind:'image',contextId:'photo-reference',label:'Photo',attachmentId:id,name:'photo.png',mimeType:'image/png',sizeBytes:5})).toBe(true);
  await f.ready();const view=f.view();if(view.files.needsPrepare)await f.run('prepare-files',view.files.prepareKey);
  return {...f,source,text,id};
}
test('inline snapshot and file-backed images save observed text before opening their bound attachment',async()=>{
  for(const storage of ['snapshot','file'] as const){
    const f=await imagePreviewFixture(storage),value=`${f.text}rapid edit 😀`;f.view(f.event('text',value));
    const owner=mobileEditorOwner(f.client)!,callback={...owner.state.identity,mountId:owner.state.mountId,kind:'contextPress',
      richEventId:'tap-photo',eventCount:owner.state.eventCount,value,payload:{source:f.source,start:3,end:3+f.source.length}};
    const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
    let finished=false;const opening=f.run('rich','',JSON.stringify(callback)).then(result=>{finished=true;return result});
    await entered.promise;expect(finished).toBe(false);expect(f.client.draft).toBe(value);
    held.resolve();const result=await opening;
    expect(obj(f.writes.at(-1)?.drafts)[f.client.draftKey]).toBe(value);
    expect(result).toMatchObject({attachmentId:f.id,message:'',owner:owner.target.owner,visit:f.input.visit});
  }
});
test('inline image preview rejects attachment removal while observed text is saving',async()=>{
  for(const storage of ['snapshot','file'] as const){
    const f=await imagePreviewFixture(storage),value=`${f.text}latest`;f.view(f.event('text',value));
    const owner=mobileEditorOwner(f.client)!,callback={...owner.state.identity,mountId:owner.state.mountId,kind:'contextPress',
      richEventId:'tap-photo',eventCount:owner.state.eventCount,value,payload:{source:f.source,start:3,end:3+f.source.length}};
    const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
    const opening=f.run('rich','',JSON.stringify(callback));await entered.promise;
    if(storage==='snapshot')f.client.local.snapshotDrafts[f.client.draftKey]=[];else setDraftFiles(f.client.local,[]);
    held.resolve();await expect(opening).rejects.toMatchObject({kind:'superseded'});
    expect(f.client.draft).toBe(value);
  }
});
test('inline image preview rereads context ownership after saving while its inventory remains',async()=>{
  for(const storage of ['snapshot','file'] as const){
    const f=await imagePreviewFixture(storage),value=`${f.text}latest`;f.view(f.event('text',value));
    const owner=mobileEditorOwner(f.client)!,callback={...owner.state.identity,mountId:owner.state.mountId,kind:'contextPress',
      richEventId:'tap-photo',eventCount:owner.state.eventCount,value,payload:{source:f.source,start:3,end:3+f.source.length}};
    const held=deferred<void>(),entered=deferred<void>();f.holdWrite(async()=>{entered.resolve();await held.promise});
    const opening=f.run('rich','',JSON.stringify(callback));await entered.promise;
    expect(mobileComposerContextCommit(f.client,mobileComposerContextCapture(f.client,mobileComposerTarget(f.client))!,value,
      undefined,'photo-reference')).toBe(true);
    held.resolve();await expect(opening).rejects.toMatchObject({kind:'superseded'});
    expect(storage==='snapshot'?f.client.local.snapshotDrafts[f.client.draftKey]:f.client.local.composerFiles).toHaveLength(1);
  }
});

test('confirmed NewTask resume clears pre-picker focus before Root schedules restoration', () => {
  const root = contractComponent('mobile/app.contract', 'App');
  const result = { visit: 'visit', effectKind: 'new-task', navigation: 'focus', message: '' };
  const state: Env = { composerFocusIdentity: 'editor:owner', composerReturn: {} };
  root.call('didRichComposerChange', state, { richComposerChanged: result, composerReply: (value: unknown) => value,
    nav: { id: 'visit' }, top: (value: unknown) => value, nativeNewTask: true, showMobileNotice: () => {},
    composerEditor: 'editor', composerOwner: 'owner', ComposerFocusReturn: (value: unknown) => value }, []);
  expect(state.composerFocusIdentity).toBe('');
  expect(state.composerReturn).toMatchObject({ editorId: 'editor', owner: 'owner', phase: 'pending', attempts: 0 });
});
