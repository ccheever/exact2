// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadMountedLocalCommandClear as clear} from './thread-send-mounted-local-clear';
import {mobileThreadMountedSendAdmission as admission} from './thread-send-mounted';
import {mobileEditorSnapshot as snapshot,mobileEditorAction as action,type EditorPresentation,type EditorRouteInput} from './composer-editor-runtime';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileEditorDocument,mobileEditorDocumentKey} from './composer-editor-persistence';
import {formatComposerContextReference} from './composer-editor-document';
import {mobileComposerTarget} from './composer-target';
import {mobileComposerContextsHydrate,mobileComposerContextSendSnapshot,mobileComposerContextCaptureTarget,mobileComposerContextCommitBatch} from './composer-command-context';
import {mobileThreadSendLocalCommand as local,type ThreadLocalCommandContext} from './thread-send-local-commands';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
import type {ThreadSendSnapshot} from './thread-send-admission';
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const now=Date.parse('2026-10-09T12:00:00Z');let sequence=0;
async function fixture(text='/feedback detail',empty=false) {
 const client=new MobileDraftClient();let disk:Obj={},writeCount=0,failAt=0,failValue:unknown=Error('save failed');
 let hook:((request:Obj,event:Obj)=>Promise<void>|void)|undefined,writeHook:((wire:Obj)=>Promise<void>|void)|undefined;
 const requests:Obj[]=[],writes:Obj[]=[],order:string[]=[];
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(disk)).buffer},async atomicWriteFile(_p,bytes){
  const wire=obj(JSON.parse(new TextDecoder().decode(bytes)));order.push('persist');writeCount++;
  if(writeCount===failAt)throw failValue;await writeHook?.(wire);disk=copy(wire);writes.push(copy(wire));
 }}};
 const bootstrap:Native={available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline'}}}};
 const handles=mobileDraftRecoveryHandles(client,bootstrap,storage);await client.refresh(handles.native,handles.storage);
 Object.assign(client,{origin:'https://mounted-clear.test',environmentId:`mounted-clear-${++sequence}`,threadId:'thread',projectId:'project',generation:3,connection:'connected',providerId:'codex',modelId:'model',runtimeMode:'full-access',interactionMode:'plan'});
 fleet.saved.push({environmentId:client.environmentId,origin:client.origin});client.local.drafts[client.draftKey]=text;
 client.shell.projects=[{id:'project',workspaceRoot:'/repo'}];client.shell.threads=[{id:'thread',projectId:'project',modelSelection:{instanceId:'codex',model:'model'}}];
 client.config={providers:[{instanceId:'codex',driver:'codex',enabled:true,installed:true,availability:'available',auth:{email:'a@example.com'},
  usageLimits:{checkedAt:new Date(now).toISOString(),windows:[{id:'daily',kind:'primary',label:'Daily',usedPercent:25}]},skills:[],slashCommands:[],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[]}]}]};
 const target=mobileComposerTarget(client),key=JSON.stringify([client.origin,client.environmentId,target.key]);
 const context={version:1,records:empty?[]:[{version:1,kind:'skill',contextId:'unused',label:'Unused',name:'unused'}]};
 mobileComposerContextsHydrate(client,{mobileComposerContexts:{version:1,entries:{[key]:{origin:client.origin,environmentId:client.environmentId,key:target.key,revision:1,text,context}}}});
 const captured:ThreadSendSnapshot={origin:client.origin,environmentId:client.environmentId,projectId:'project',threadId:'thread',draftKey:target.key,rawText:text,context,attachments:[],
  modelSelection:{instanceId:'codex',model:'model'},runtimeMode:'full-access',interactionMode:'plan',providerDriver:'codex',activeProviderThreadId:'provider-thread',showInteractionModeToggle:true};
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:client.environmentId,threadId:'thread',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const presentation:EditorPresentation={themeJson:'{}',placeholder:'Message',fontSize:16,lineHeight:20,enterBehavior:'send',iconUris:{},hasCompactableConversation:true,offersUsageLimits:true,allowInteractionMode:true,repository:'repo',permissionRevision:'permission'};
 const native:Native={available:true,watch(){},async later(raw){const request=obj(raw);requests.push(copy(request));order.push(String(request.op));
  if(request.op!=='composerEditorApply')throw Error('Unexpected native operation');
  const command=obj(request.command),next=obj(command.next),event:Obj={...obj(request.identity),kind:'commandApplied',commandId:command.commandId,commandRevision:command.commandRevision,
   eventCount:Number(obj(command.expected).eventCount)+1,value:next.value,selection:next.selection,composing:false,focused:true,reason:''};
  await hook?.(request,event);return {ok:true,generation:request.generation,value:{event}};
 }};
 const view=()=>snapshot(client,route,'',presentation,now);
 const event=(kind:string,value=mobileEditorOwner(client)!.state.value,extra:Obj={})=>{const owner=mobileEditorOwner(client)!;return {...owner.state.identity,mountId:owner.state.mountId||'mount',kind,eventCount:owner.state.eventCount+1,
  value,selection:{start:value.length,end:value.length},composing:false,focused:true,...extra}};
 const send=(e:unknown)=>action(client,route,'event',JSON.stringify(e),native,storage,()=>now);
 view();await send(event('ready'));
 writeCount=0;writes.length=0;order.length=0;requests.length=0;
 const invoke=(mode:'text'|'content'='content')=>clear(client,target,captured,mode,admission(client)!,native,storage);
 const ctx:ThreadLocalCommandContext={snapshot:captured,facts:{connected:true,canOperate:true,pendingThreadCreation:false,queuedEdit:false,contextImporting:false,voiceBlocked:false,pendingPastedText:false,usageLimitsOffered:true,
  planModeEnabled:true,activeThreadBusy:false,canSteerActiveTurn:true,followUpBehavior:'queue',config:client.config,uploadStates:{},uploadOwners:{}},now,usageKey:'usage',current:()=>client.environmentId===target.environmentId,
  clearDraft:mode=>invoke(mode),uploadFeedback:async payload=>{order.push('rpc');expect(payload).toEqual({threadId:'thread',reason:'detail'});return {feedbackId:'remote'}}};
 return {client,target,key,captured,route,native,storage,requests,writes,order,view,event,send,invoke,ctx,disk:()=>copy(disk),
  hook:(h:typeof hook)=>{hook=h},writeHook:(h:typeof writeHook)=>{writeHook=h},fail:(at:number,value:unknown=Error('save failed'))=>{failAt=at;failValue=value},resetWrites:()=>{writeCount=0;writes.length=0}};
}
for(const empty of [false,true])test(`feedback atomically removes ${empty?'empty':'nonempty'} context and saves before one RPC`,async()=>{
 const f=await fixture('/feedback detail',empty),beforeMode=f.client.interactionMode;
 expect(await local(f.client,f.ctx)).toMatchObject({kind:'feedback',status:'sent',message:''});
 expect(f.order).toEqual(['composerEditorApply','persist','persist','rpc']);expect(f.client.draft).toBe('');expect(f.client.interactionMode).toBe(beforeMode);
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).toBeNull();
 expect(f.writes.every(w=>obj(w.drafts)[f.target.key]===''&&!Object.hasOwn(obj(obj(w.mobileComposerContexts).entries),f.key))).toBe(true);
 expect(f.requests).toHaveLength(1);expect(mobileEditorOwner(f.client)?.pending).toBeNull();
});
test('usage preserves explicit empty envelope; unavailable report never dispatches native clear',async()=>{
 const f=await fixture('/usage-limits',true);expect(await local(f.client,f.ctx)).toMatchObject({kind:'usage-limits',opened:true,message:''});
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).toEqual({version:1,records:[]});expect(f.order).not.toContain('rpc');
 const missing=await fixture('/usage-limits');missing.ctx.facts.config={providers:[]};
 expect(await local(missing.client,missing.ctx)).toMatchObject({kind:'usage-limits',opened:false});expect(missing.requests).toEqual([]);expect(missing.client.draft).toBe('/usage-limits');
});
test('rejected CAS preserves producer context and does not upload',async()=>{
 const f=await fixture();f.hook((_r,e)=>{Object.assign(e,{kind:'commandRejected',value:f.captured.rawText,selection:{start:2,end:2},reason:'composition',composing:true})});
 expect(await local(f.client,f.ctx)).toMatchObject({status:'failed'});expect(f.order).not.toContain('rpc');
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).toEqual(f.captured.context);
});
for(const condition of ['composition','readOnly','voice','catalog','malformed','attachment'] as const)test(`preflight ${condition} refuses before native or save`,async()=>{
 const f=await fixture();
 if(condition==='composition')await f.send(f.event('selection',undefined,{composing:true}));
 if(condition==='readOnly'){f.route.readOnly=true;f.view()}
 if(condition==='voice'){f.route.voiceBusy=true;f.view()}
 if(condition==='catalog')fleet.saved.find(row=>row.environmentId===f.client.environmentId)!.origin='https://replacement.test';
 if(condition==='malformed')Object.assign(f.client.local,{composerFiles:null});
 if(condition==='attachment')f.client.local.snapshotDrafts[f.target.key]=[{id:'11111111-1111-4111-8111-111111111111',name:'image.png',mimeType:'image/png',sizeBytes:3}];
 f.resetWrites();expect((await f.invoke()).applied).toBe(false);expect(f.requests).toEqual([]);expect(f.writes).toEqual([]);
});
test('synchronous event-first publication and failed event save are repaired by fresh invocation save',async()=>{
 const f=await fixture();let failed=false;f.fail(1);
 f.hook(async(_r,e)=>{const running=f.send(e);expect(mobileEditorOwner(f.client)?.pending).toBeNull();expect(f.client.draft).toBe('');
  try{const result=await running;failed=!!result.message}catch{failed=true}});
 expect(await f.invoke()).toEqual({applied:true,message:''});expect(failed).toBe(true);expect(f.order).toEqual(['composerEditorApply','persist','persist']);
 expect(f.requests).toHaveLength(1);expect(f.writes).toHaveLength(1);
});
for(const failure of ['ordinary','letGo'] as const)test(`final fresh save ${failure} retains applied clear without RPC`,async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'};f.fail(2,failure==='letGo'?sentinel:Error('final save failed'));
 if(failure==='letGo')await expect(local(f.client,f.ctx)).rejects.toBe(sentinel);
 else expect(await local(f.client,f.ctx)).toMatchObject({status:'failed',message:'final save failed'});
 expect(f.client.draft).toBe('');expect(f.order).not.toContain('rpc');expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).toBeNull();
});
for(const kind of ['text','selection','metadata','attachment'] as const)test(`new ${kind} before held terminal preserves producer ownership`,async()=>{
 const f=await fixture(kind==='metadata'?`/feedback ${formatComposerContextReference({kind:'skill',contextId:'new',label:'New'})}`:'/feedback detail');f.hook(async(_r,e)=>{
  if(kind==='metadata'){const guard=mobileComposerContextCaptureTarget(f.client,f.target)!;expect(mobileComposerContextCommitBatch(f.client,guard,[{version:1,kind:'skill',contextId:'new',label:'New',name:'new'}])).toBe(true)}
  else if(kind==='attachment')f.client.local.snapshotDrafts[f.target.key]=[{id:'11111111-1111-4111-8111-111111111111',name:'image.png',mimeType:'image/png',sizeBytes:3}];
  else {const later={...e,kind:kind==='text'?'text':'selection',eventCount:Number(e.eventCount)+1,value:kind==='text'?'new text':'',selection:kind==='text'?{start:8,end:8}:{start:0,end:0},pendingCommand:{...e}};
   await f.send(later)}
 });
 expect((await f.invoke()).applied).toBe(false);expect(f.order).not.toContain('rpc');
 if(kind==='text')expect(f.client.draft).toBe('new text');
 if(kind==='attachment')expect(f.client.local.snapshotDrafts[f.target.key]).toHaveLength(1);
 if(kind==='metadata'){expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context?.records.map(r=>r.contextId)).toEqual(['new']);
  expect(obj(obj(obj(f.disk().mobileComposerContexts).entries)[f.key]).context).toEqual(mobileComposerContextSendSnapshot(f.client,f.target)?.context)}
 if(kind==='attachment')expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).not.toBeNull();
});
test('typing after accepted clear persists latest text without repeating clear or blocking original feedback',async()=>{
 const f=await fixture();f.hook(async(_r,e)=>{await f.send(e);await f.send(f.event('text','newer'))});
 expect(await local(f.client,f.ctx)).toMatchObject({status:'sent'});expect(f.client.draft).toBe('newer');expect(obj(f.disk().drafts)[f.target.key]).toBe('newer');expect(f.requests).toHaveLength(1);
});
for(const kind of ['focus','submit'] as const)test(`retained terminal carried by ${kind} keeps its once-only root effect`,async()=>{
 const f=await fixture();f.hook(async(_r,e)=>{await f.send({...e,kind,eventCount:Number(e.eventCount)+1,pendingCommand:{...e},alternate:false})});
 expect((await f.invoke()).applied).toBe(false);expect(mobileEditorOwner(f.client)!.effects.filter(e=>e.kind===kind)).toHaveLength(1);
});
for(const change of ['route','environment','generation','catalog'] as const)test(`after publication ${change} keeps only original valid transport authority`,async()=>{
 const f=await fixture();f.hook(async(_r,e)=>{await f.send(e);
  if(change==='route'){f.client.threadId='other';f.route.active=false;f.view()}
  if(change==='environment')f.client.environmentId='other';
  if(change==='generation')f.client.generation++;
  if(change==='catalog')fleet.saved.find(row=>row.environmentId===f.client.environmentId)!.origin='https://replacement.test';
 });
 if(change==='route'){expect(await local(f.client,f.ctx)).toMatchObject({status:'sent'});expect(f.order.filter(x=>x==='rpc')).toHaveLength(1)}
 else {await expect(local(f.client,f.ctx)).rejects.toMatchObject({kind:'superseded'});expect(f.order).not.toContain('rpc')}
});
test('optional absent file inventory remains supported, explicit null does not',async()=>{
 const f=await fixture();delete (f.client.local as unknown as Obj).composerFiles;
 expect(await f.invoke()).toEqual({applied:true,message:''});
});
test('abandoned native reply retains only the exact command; explicit retry never resumes old feedback RPC',async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'};let once=true;
 f.hook(()=>{if(once){once=false;throw sentinel}});
 await expect(local(f.client,f.ctx)).rejects.toBe(sentinel);const id=mobileEditorOwner(f.client)!.pending!.id;
 expect(f.client.draft).toBe('/feedback detail');expect(f.order).not.toContain('rpc');
 const result=await action(f.client,f.route,'retry',id,f.native,f.storage,()=>now);
 expect(result.command).toEqual({id,applied:true,contentCleared:true});expect(f.client.draft).toBe('');expect(f.order).not.toContain('rpc');
 expect(f.requests[0]!.command).toEqual(f.requests[1]!.command);
});
test('ordinary publication save failure reports applied native clear and never uploads',async()=>{
 const f=await fixture();f.fail(1);
 expect(await f.invoke()).toEqual({applied:true,message:'save failed'});expect(f.client.draft).toBe('');
 expect(f.order).toEqual(['composerEditorApply','persist']);
});
test('contradictory terminal reply cannot change an event-first applied receipt',async()=>{
 const f=await fixture();f.hook(async(_r,e)=>{await f.send(e);e.kind='commandRejected';e.reason='contradictory'});
 await expect(local(f.client,f.ctx)).rejects.toMatchObject({kind:'superseded'});expect(f.order).not.toContain('rpc');expect(f.client.draft).toBe('');
});
for(const change of ['route','generation'] as const)test(`held publication save ${change} departure keeps original callback boundary`,async()=>{
 const f=await fixture();let release!:()=>void,entered!:()=>void,first=true;
 const gate=new Promise<void>(done=>{release=done}),started=new Promise<void>(done=>{entered=done});
 f.writeHook(async()=>{if(first){first=false;entered();await gate}});
 const pending=local(f.client,f.ctx);await started;expect(f.client.draft).toBe('');
 if(change==='route'){f.client.threadId='other';f.route.active=false;f.view()}else f.client.generation++;
 release();
 if(change==='route'){expect(await pending).toMatchObject({status:'sent'});expect(f.order.filter(v=>v==='rpc')).toHaveLength(1)}
 else {await expect(pending).rejects.toMatchObject({kind:'superseded'});expect(f.order).not.toContain('rpc')}
});

for(const change of ['incarnation','blocked','unobserved'] as const)test(`fresh save ${change} replacement revokes original named document authority`,async()=>{
 const f=await fixture();let count=0;
 f.writeHook(()=>{if(++count!==2)return;
  const document=mobileEditorDocument(f.client,mobileEditorDocumentKey({origin:f.captured.origin,environmentId:f.target.environmentId,draftKey:f.target.key}))!;
  if(change==='incarnation')document.incarnation='replacement';
  if(change==='blocked')document.blocked=true;
  if(change==='unobserved')f.client.local.drafts[f.target.key]='unobserved write';
 });
 await expect(local(f.client,f.ctx)).rejects.toMatchObject({kind:'superseded'});expect(f.order).not.toContain('rpc');
});
