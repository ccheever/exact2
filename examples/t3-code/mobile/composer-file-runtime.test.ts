// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileEditorSnapshot as snapshot,mobileEditorAction as action,mobileEditorPrepareFiles as prepare,mobileEditorCleanupFiles as cleanup,
 mobileEditorFilesReady as readyFiles,mobileEditorFilesRetry as retry,mobileEditorFilesDelegateRetired as delegate,mobileEditorRequestIntent,
 type EditorRouteInput,type EditorPresentation} from './composer-editor-runtime';
import {mobileEditorOwner,mobileEditorCaptureIntent} from './composer-editor-owner';
import {mobileComposerContextsHydrate,mobileComposerContextSendSnapshot,mobileComposerContextRead} from './composer-command-context';
import {mobileComposerTarget} from './composer-target';
import {formatComposerContextReference as reference} from './composer-editor-document';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {DraftFile} from './shared/composer-editor-files';
import type {Native,Files} from './shared/protocol';
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v));
const uuid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const token=(n:number)=>reference({kind:'file',contextId:`f${n}`,label:`file${n}.txt`});
const skill=(n:number):Obj=>({version:1,kind:'skill',contextId:`s${n}`,label:`Skill${n}`,name:`skill${n}`});
const skillText=(n:number)=>reference({kind:'skill',contextId:`s${n}`,label:`Skill${n}`});
function deferred<T>(){let resolve!:(v:T)=>void,reject!:(e:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
let sequence=0;
/** The native boundary independently checks actual saved inventory and a canonical-byte map.
 * It never accepts a row merely because the TypeScript caller supplied a valid shape. */
async function fixture(count=1,text=Array.from({length:count},(_,i)=>token(i+1)).join(' ')) {
 const client=new MobileDraftClient(),calls:Obj[]=[],writes:Obj[]=[],events:string[]=[],bytes=new Set<string>();let disk:Obj={};
 let saveHook:((wire:Obj)=>Promise<void>|void)|undefined,nativeHook:((request:Obj,result:Obj)=>Promise<Obj>|Obj)|undefined;
 const holds=new Map<string,{request:Obj;receipt:Obj;released:boolean}>();
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(disk)).buffer},async atomicWriteFile(_p,data){
  const wire=obj(JSON.parse(new TextDecoder().decode(data)));events.push('save');await saveHook?.(wire);disk=copy(wire);writes.push(copy(wire));
 }}};
 const bootstrap:Native={available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline'}}}};
 const handles=mobileDraftRecoveryHandles(client,bootstrap,storage);await client.refresh(handles.native,handles.storage);
 Object.assign(client,{origin:'https://file-runtime.test',environmentId:`file-runtime-${++sequence}`,threadId:'t',projectId:'p',generation:1,connection:'disconnected',providerId:'codex'});
 fleet.saved.push({environmentId:client.environmentId,origin:client.origin});client.shell.projects=[{id:'p',workspaceRoot:'/repo'}];
 client.shell.threads=[{id:'t',projectId:'p',title:'Current'},{id:'other',projectId:'p',title:'Other'}];
 client.config={providers:[{instanceId:'codex',driver:'codex',skills:[],slashCommands:[],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[]}]}]};
 const target=mobileComposerTarget(client),makeFile=(n:number):DraftFile=>({id:uuid(n),contextId:`f${n}`,draftKey:target.key,environmentId:target.environmentId,
  name:`file${n}.txt`,mimeType:'text/plain',sizeBytes:n,source:'attached',attachmentId:'',status:'staged'});
 const files=Array.from({length:count},(_,i)=>makeFile(i+1));files.forEach(f=>bytes.add(f.id));
 Object.assign(client.local,{composerFiles:files,mobileAttachmentOrder:{[target.key]:files.map(f=>f.id)}});client.local.drafts[target.key]=text;
 // A real prior picker/storage operation, before editor admission, owns these canonical rows.
 await client.persist(storage);calls.length=0;writes.length=0;events.length=0;
 const native:Native={available:true,watch(){},async later(raw){const request=obj(raw);calls.push(copy(request));events.push(String(request.action||request.op));let value:Obj={};
  if(request.op==='composerFileHold'){
   const id=String(request.requestId),prior=holds.get(id);
   if(request.action==='acquire'){
    if(prior){expect(prior.request).toEqual(request);if(prior.released)throw Error('released hold cannot reacquire');value={status:'held',receipt:prior.receipt}}
    else {const file=obj(request.file),saved=(disk.composerFiles as Obj[]).find(f=>f.id===file.id);
     if(!saved||JSON.stringify(saved)!==JSON.stringify(file)||!bytes.has(String(file.id)))throw Error('File is not saved in native canonical inventory');
     const receipt={identity:request.identity,requestId:request.requestId,holdId:uuid(10000+holds.size),fileIdentity:uuid(20000+holds.size),id:file.id,sizeBytes:file.sizeBytes};
     holds.set(id,{request:copy(request),receipt:copy(receipt),released:false});value={status:'held',receipt};
    }
   }else {if(prior)prior.released=true;value={status:'released',requestId:id}}
  }else if(request.op==='composerEditorApply'){
   const command=obj(request.command),next=obj(command.next);value={event:{...obj(request.identity),kind:'commandApplied',eventCount:Number(obj(command.expected).eventCount)+1,
    commandId:command.commandId,commandRevision:command.commandRevision,reason:'',value:next.value,selection:next.selection,focused:true,composing:false}};
  }else if(request.op==='mobileQueuedEdit'&&request.action==='release'){
   const pending=obj(disk.mobileNewTaskDrafts).fileReleases as string[]??[];
   for(const id of pending)if(!(disk.composerFiles as Obj[]).some(f=>f.id===id)&&![...holds.values()].some(h=>!h.released&&obj(h.request.file).id===id))bytes.delete(id);
   value={released:true};
  }else throw Error(`Unexpected native operation ${JSON.stringify(request)}`);
  const response={ok:true,generation:request.generation??1,value};return nativeHook?nativeHook(request,response):response;
 }};
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:client.environmentId,threadId:'t',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const presentation:EditorPresentation={themeJson:'{}',placeholder:'Message',fontSize:16,lineHeight:20,enterBehavior:'send',iconUris:{},hasCompactableConversation:true,
  offersUsageLimits:false,allowInteractionMode:true,repository:'org/repo',permissionRevision:'session'};
 const view=(raw='')=>snapshot(client,route,raw,presentation,1000);
 const event=(kind:string,value=mobileEditorOwner(client)!.state.value,extra:Obj={})=>{const state=mobileEditorOwner(client)!.state;return {...state.identity,mountId:state.mountId||'mount',kind,
  eventCount:state.eventCount+1,value,selection:{start:value.length,end:value.length},composing:false,focused:true,...extra}};
 const send=(e:unknown)=>action(client,route,'event',JSON.stringify(e),native,storage,()=>1000);
 const mount=async()=>{view();await send(event('ready'));return view()};
 const protect=async()=>{const v=view();await prepare(client,v.admission,v.files.prepareKey,native,storage);return view()};
 const read=()=>mobileComposerContextSendSnapshot(client,target)!;
 return {client,target,route,presentation,native,storage,calls,writes,events,bytes,holds,makeFile,view,event,send,mount,protect,read,disk:()=>copy(disk),
  saveHook:(h:typeof saveHook)=>{saveHook=h},nativeHook:(h:typeof nativeHook)=>{nativeHook=h}};
}

test('offline native ready stays noneditable until actual saved canonical hold, with no synthetic save',async()=>{
 const f=await fixture(),v=await f.mount();expect(v.files.ready).toBe(false);expect(v.files.needsPrepare).toBe(true);
 expect(JSON.parse(v.configuration)).toMatchObject({editable:false,readOnly:false});expect(readyFiles(f.client,v.admission)).toBe(false);
 const next=await f.protect();expect(next.files.ready).toBe(true);expect(JSON.parse(next.configuration).editable).toBe(true);
 expect(f.events).toEqual(['acquire']);expect(f.writes).toHaveLength(0);expect(f.calls[0]?.file).toEqual(f.read().files[0]);
 expect(mobileComposerContextRead(f.client).context).toBeUndefined();
});
test('real saved picker token with no context entry deletes then Undo restores file bytes and resets upload binding',async()=>{
 const f=await fixture();await f.mount();await f.protect();const saved=f.read().files[0]!;
 // An accepted upload changes metadata on the same held canonical file.
 Object.assign((f.client.local.composerFiles as DraftFile[])[0]!,{attachmentId:'server-upload',status:'ready'});
 await f.send(f.event('text',token(1)));await f.send(f.event('text',''));
 expect(f.read().files).toEqual([]);expect(f.read().attachmentIds).toEqual([]);expect(f.read().context).toBeNull();
 expect(obj(f.disk().mobileNewTaskDrafts).fileReleases).toContain(saved.id);expect(f.bytes.has(saved.id)).toBe(true);expect([...f.holds.values()][0]?.released).toBe(false);
 await f.send(f.event('text',token(1)));expect(f.read().files).toEqual([{...saved,attachmentId:'',status:'staged'}]);
 expect(f.read().context?.records[0]).toMatchObject({kind:'file',attachmentId:saved.id,contextId:'f1'});
 expect(f.read().attachmentIds).toEqual([saved.id]);expect(f.calls.filter(c=>c.action==='acquire')).toHaveLength(1);expect(f.bytes.has(saved.id)).toBe(true);
 expect(f.disk().composerFiles).toEqual(f.read().files);
});
test('mixed image and unrelated file remain in order while only context-owned file is pruned and restored last',async()=>{
 const f=await fixture(2,token(1));f.client.local.snapshotDrafts[f.target.key]=[{id:uuid(30),name:'image.png',mimeType:'image/png',sizeBytes:1}];
 Object.assign(f.client.local,{mobileAttachmentOrder:{[f.target.key]:[uuid(1),uuid(30),uuid(2)]}});await f.client.persist(f.storage);
 await f.mount();await f.protect();await f.send(f.event('text','plain'));
 expect(f.read().attachmentIds).toEqual([uuid(30),uuid(2)]);expect(f.read().images).toHaveLength(1);
 await f.send(f.event('text',token(1)));expect(f.read().attachmentIds).toEqual([uuid(30),uuid(2),uuid(1)]);expect(f.read().files.map(x=>x.id)).toEqual([uuid(2),uuid(1)]);
});
test('duplicate exact native text envelope does not advance document/context/history or persist again',async()=>{
 const f=await fixture();await f.mount();await f.protect();const e=f.event('text','');await f.send(e);
 const before=copy(f.read()),revision=mobileEditorOwner(f.client)!.document.revision,writes=f.writes.length;
 await f.send(e);expect(f.read()).toEqual(before);expect(mobileEditorOwner(f.client)!.document.revision).toBe(revision);expect(f.writes).toHaveLength(writes);
 await f.send(f.event('text',token(1)));expect(f.read().files).toHaveLength(1);
});
test('saved explicit file record wins derived picker metadata, and contradictory binding refuses atomically',async()=>{
 const f=await fixture(),key=JSON.stringify([f.client.origin,f.target.environmentId,f.target.key]);
 const context={version:1,records:[{version:1,kind:'file',contextId:'f1',label:'Explicit',attachmentId:uuid(1),name:'original.pdf',mimeType:'application/pdf',sizeBytes:1}]};
 mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{[key]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:1,text:f.client.draft,context}}}});
 await f.mount();await f.protect();await f.send(f.event('text',token(1)));expect(f.read().context?.records[0]?.label).toBe('Explicit');
 const g=await fixture(),gkey=JSON.stringify([g.client.origin,g.target.environmentId,g.target.key]);
 mobileComposerContextsHydrate(g.client,{mobileComposerContexts:{version:1,entries:{[gkey]:{origin:g.client.origin,environmentId:g.target.environmentId,key:g.target.key,revision:1,text:g.client.draft,
  context:{version:1,records:[{...context.records[0],attachmentId:uuid(999)}]}}}}});
 await g.mount();const v=await g.protect();expect(v.files.ready).toBe(false);expect(g.client.draft).toBe(token(1));expect(g.writes).toHaveLength(0);
});
for(const bad of ['unsaved','pasted','malformed-foreign'] as const)test(`${bad} inventory never gains editable native authority`,async()=>{
 const f=await fixture();
 if(bad==='unsaved'){const row=f.makeFile(2);(f.client.local.composerFiles as DraftFile[]).push(row);f.bytes.add(row.id)}
 if(bad==='pasted')(f.client.local.composerFiles as DraftFile[])[0]!.source='pasted-text';
 if(bad==='malformed-foreign')f.client.local.snapshotDrafts.foreign=[{id:'foreign',name:'bad',mimeType:'image/png',sizeBytes:-1}];
 const before=copy(f.client.local),v=await f.mount();if(v.files.needsPrepare)await f.protect();
 expect(f.view().files.ready).toBe(false);expect(JSON.parse(f.view().configuration).editable).toBe(false);
 expect(f.client.local.composerFiles).toEqual(before.composerFiles);expect(f.client.local.snapshotDrafts).toEqual(before.snapshotDrafts);expect(f.writes).toHaveLength(0);
});
test('lost acquire response retains exact request for explicit retry; abandoned handle makes no cleanup calls',async()=>{
 const f=await fixture();await f.mount();const abort={name:'FetchError',kind:'Aborted'};f.nativeHook(()=>{throw abort});
 await expect(f.protect()).rejects.toBe(abort);expect(f.calls).toHaveLength(1);expect(f.view().files.needsPrepare).toBe(false);expect(f.view().files.ready).toBe(false);
 const request=copy(f.calls[0]);f.nativeHook(undefined);retry(f.client,f.view().admission);await f.protect();expect(f.calls[1]).toEqual(request);expect(f.view().files.ready).toBe(true);
});
test('retirement invalidates late acquire; delegation drops JS authority without pretending native release',async()=>{
 const f=await fixture();await f.mount();const hold=deferred<Obj>();let reply:Obj={};f.nativeHook((_r,r)=>{reply=r;return hold.promise});
 const old=f.view().admission,work=f.protect();f.route.active=false;const retired=f.view();expect(retired.files.retiredAdmissions).toContain(old);
 delegate(f.client,old);hold.resolve(reply);await expect(work).rejects.toMatchObject({kind:'superseded'});
 expect(f.view().files.retiredAdmissions).toEqual([]);expect([...f.holds.values()][0]?.released).toBe(false);expect(f.calls).toHaveLength(1);
 f.route.active=true;f.route.routeVisit='new-visit';f.nativeHook(undefined);await f.mount();expect(f.view().files.ready).toBe(false);await f.protect();expect(f.calls).toHaveLength(2);
});
test('cleanup only releases after accepted publication and a fresh successful save; native guarded drain owns deletion',async()=>{
 const f=await fixture();await f.mount();await f.protect();await f.send(f.event('text',''));
 f.route.active=false;let v=f.view();expect(v.files.cleanupKey).not.toBe('');f.saveHook(()=>{throw Error('disk full')});
 await cleanup(f.client,v.files.cleanupKey,f.native,f.storage);expect(f.calls.filter(c=>c.action==='release')).toEqual([]);expect(f.bytes.has(uuid(1))).toBe(true);
 // A fresh current owner exposes explicit retry while old native port still owns its release claim.
 f.route.active=true;f.route.routeVisit='return';await f.mount();retry(f.client,f.view().admission);v=f.view();f.saveHook(undefined);
 await cleanup(f.client,v.files.cleanupKey,f.native,f.storage);expect(f.events.slice(-3)).toEqual(['save','release','release']);
 expect(f.bytes.has(uuid(1))).toBe(false);expect([...f.holds.values()][0]?.released).toBe(true);
});
test('menu terminal with later native text seeds context Undo without replacing latest text or file inventory',async()=>{
 const f=await fixture(1,`${token(1)} @Other`);await f.mount();await f.protect();const view=f.view(),item=view.menu.rows.find(r=>r.id.startsWith('thread:'))!;
 expect(item).toBeDefined();f.nativeHook(async(r,reply)=>{if(r.op!=='composerEditorApply')return reply;const terminal=obj(obj(reply.value).event);
 await f.send({...obj(r.identity),kind:'text',eventCount:Number(terminal.eventCount)+1,value:token(1),selection:{start:token(1).length,end:token(1).length},focused:true,composing:false,pendingCommand:terminal});return reply});
 await action(f.client,f.route,'pick',JSON.stringify({admission:view.admission,menuRevision:view.menuRevision,id:item.id}),f.native,f.storage,()=>1000);
 expect(f.client.draft).toBe(token(1));expect(f.read().files).toHaveLength(1);
 await f.send(f.event('text',`${token(1)} [Other](t3-context://v1/thread/thread_other)`));expect(f.read().context?.records.map(r=>r.contextId)).toEqual(['thread_other','f1']);
});
test('actual context history refresh protects recently live record at200-entry eviction boundary',async()=>{
 const f=await fixture(0,skillText(0)),key=JSON.stringify([f.client.origin,f.target.environmentId,f.target.key]);
 mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{[key]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:1,text:f.client.draft,
  context:{version:1,records:Array.from({length:200},(_,i)=>skill(i))}}}}});
 await f.mount();await f.send(f.event('text',skillText(0)));await f.send(f.event('text',''));
 const capture=mobileEditorCaptureIntent(f.client,f.target,'test-import')!;await mobileEditorRequestIntent(f.client,capture,{value:skillText(200),selection:{start:skillText(200).length,end:skillText(200).length}},skill(200),f.native,f.storage);
 await f.send(f.event('text',skillText(0)));expect(f.read().context?.records.map(r=>r.contextId)).toEqual(['s0']);
 await f.send(f.event('text',skillText(1)));expect(f.read().context).toBeNull();
});
test('held save does not replay accepted publication and later text wins',async()=>{
 const f=await fixture();await f.mount();await f.protect();const held=deferred<void>();let saves=0;f.saveHook(()=>++saves===1?held.promise:undefined);
 const first=f.send(f.event('text','')),second=f.send(f.event('text',token(1)));expect(f.client.draft).toBe(token(1));expect(f.read().files).toHaveLength(1);
 held.resolve();await Promise.all([first,second]);expect(f.client.draft).toBe(token(1));expect(f.read().files).toHaveLength(1);expect(f.calls).toHaveLength(1);
});

test('live overflow and source200 Undo bound release evicted holds only after a saved publication',async()=>{
 const f=await fixture(205,Array.from({length:200},(_,i)=>token(i+1)).join(' '));await f.mount();await f.protect();expect(f.holds.size).toBe(205);
 await f.send(f.event('text',''));expect(f.read().files.map(row=>row.id)).toEqual([201,202,203,204,205].map(uuid));
 expect(f.view().files.cleanupKey).toBe('');await f.send(f.event('text','next'));
 const key=f.view().files.cleanupKey;expect(key).not.toBe('');expect(f.calls.filter(c=>c.action==='release')).toEqual([]);
 await cleanup(f.client,key,f.native,f.storage);expect([...f.holds.values()].filter(h=>h.released).map(h=>obj(h.request.file).id)).toEqual([1,2,3,4,5].map(uuid));
 expect(f.bytes.has(uuid(1))).toBe(false);expect(f.bytes.has(uuid(6))).toBe(true);expect(f.bytes.has(uuid(205))).toBe(true);
 await f.send(f.event('text',token(6)));expect(f.read().files.at(-1)?.id).toBe(uuid(6));
});
