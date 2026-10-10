// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerPickerFileLimit} from './composer-picker';
import {mobileComposerAttachmentAction,mobileComposerAttachments} from './composer-attachments';
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
 let nativeBefore:((request:Obj)=>Promise<Obj|undefined>|Obj|undefined)|undefined;
 let saveHook:((wire:Obj)=>Promise<void>|void)|undefined,nativeHook:((request:Obj,result:Obj)=>Promise<Obj>|Obj)|undefined;
 const intakes=new Map<string,{request:Obj;status:string;files:Obj[];finish?:Obj}>();let picked:Obj[]=[];
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
 const native:Native={available:true,watch(){},async later(raw){const request=obj(raw);calls.push(copy(request));events.push(String(request.action||request.op));const override=await nativeBefore?.(request);if(override)return override;let value:Obj={};
  if(request.op==='composerPickerIntake'){
   const operationId=String(request.operationId);let intake=intakes.get(operationId);
   if(request.action==='pick'){
    if(intake)expect(intake.request).toEqual(request);
    else {intake={request:copy(request),status:'staged',files:copy(picked)};intakes.set(operationId,intake);picked.forEach(f=>bytes.add(String(f.id)))}
   }
   if(!intake)throw Error('Unknown issued picker');
   if(request.action==='hold'){
    const inner=obj(request.request),file=obj(inner.file),issued=intake.files.find(f=>f.id===file.id&&f.kind==='file');
    if(!issued||!bytes.has(String(file.id))||['name','mimeType','sizeBytes'].some(k=>issued[k]!==file[k]))throw Error('Not an actual issued member');
    const prior=holds.get(String(inner.requestId));
    if(prior){expect(prior.request).toEqual(inner);if(prior.released)throw Error('released hold');value={status:'held',receipt:prior.receipt}}
    else {expect((disk.composerFiles as Obj[]).some(f=>f.id===file.id)).toBe(false);
     const receipt={identity:inner.identity,requestId:inner.requestId,holdId:uuid(10000+holds.size),fileIdentity:uuid(20000+holds.size),id:file.id,sizeBytes:file.sizeBytes};
     holds.set(String(inner.requestId),{request:copy(inner),receipt:copy(receipt),released:false});value={status:'held',receipt};}
   }else{
    if(request.action==='finish'){
     const publication=obj(request.publication),after=obj(publication.after),doc=obj(after.document),key=String(doc.draftKey),documentKey=JSON.stringify([doc.origin,doc.environmentId,key]);
     if(intake.status==='finished')expect(intake.finish).toEqual(request);
     else {
      expect(obj(disk.drafts)[key]??'').toBe(after.text);
      expect(obj(obj(disk.mobileComposerEditor).documents)[documentKey]).toMatchObject(doc);
      expect(obj(obj(disk.mobileComposerContexts).entries)[documentKey]??null).toEqual(after.context);
      expect(obj(disk.snapshotDrafts)[key]??[]).toEqual(after.images);
      expect((disk.composerFiles as Obj[]).filter(f=>f.draftKey===key)).toEqual(after.files);
      expect(obj(disk.mobileAttachmentOrder)[key]??null).toEqual(after.attachmentOrder);
      const ids=[...(after.images as Obj[]),...(after.files as Obj[])].map(f=>f.id);
      expect(publication.acceptedIds).toEqual(intake.files.filter(f=>ids.includes(f.id)).map(f=>f.id));
      expect(publication.discardedIds).toEqual(intake.files.filter(f=>!ids.includes(f.id)).map(f=>f.id));
      intake.status='finished';intake.finish=copy(request);
     }
    }else if(request.action==='cancel')intake.status='cancelled';
    value={operationId,identity:intake.request.identity,status:intake.status,files:copy(intake.files),error:''};
   }
  }else if(request.op==='composerFileHold'){
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
 return {client,target,route,presentation,native,storage,calls,writes,events,bytes,holds,intakes,pick:(rows:Obj[])=>{picked=copy(rows)},makeFile,view,event,send,mount,protect,read,disk:()=>copy(disk),
  beforeNative:(h:typeof nativeBefore)=>{nativeBefore=h},saveHook:(h:typeof saveHook)=>{saveHook=h},nativeHook:(h:typeof nativeHook)=>{nativeHook=h}};
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


const incoming=(n:number,kind:'file'|'image'='file'):Obj=>({kind,id:uuid(n),name:kind==='image'?`picked${n}.png`:`picked${n}.txt`,mimeType:kind==='image'?'image/png':'text/plain',sizeBytes:n});
async function pickerFixture(text='left right') {
 const f=await fixture(0,text);f.client.config.environment={capabilities:{attachmentUploads:true,fileAttachments:{maxUploadBytes:1024}}};
 await f.mount();return f;
}
const pickAction=async(f:Awaited<ReturnType<typeof pickerFixture>>,source='files',id='')=>(await mobileComposerAttachmentAction(source,id,f.native,f.storage,f.client,f.target.owner)).message;
test('mounted picker inserts real issued mixed bytes at captured selection and finishes exact saved projection',async()=>{
 const f=await pickerFixture();await f.send(f.event('selection','left right',{selection:{start:5,end:5}}));f.pick([incoming(71),incoming(72,'image')]);
 expect(await pickAction(f)).toBe('');const saved=f.read();
 expect(saved.text.startsWith('left [picked71.txt]')).toBe(true);expect(saved.text.endsWith(' right')).toBe(true);
 expect(saved.attachmentIds).toEqual([uuid(71),uuid(72)]);expect(saved.context?.records.map(r=>r.attachmentId)).toEqual([uuid(71),uuid(72)]);
 expect([...f.intakes.values()][0]?.status).toBe('finished');expect(f.calls.find(c=>c.action==='pick')?.remaining).toBe(100);
 expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);expect(f.holds.size).toBe(1);
 expect(f.events.indexOf('hold')).toBeLessThan(f.events.indexOf('composerEditorApply'));
});
test('event-first native picker terminal and later text publish once and preserve imported Undo history',async()=>{
 const f=await pickerFixture('');f.pick([incoming(73)]);let terminal=false;
 f.nativeHook(async(request,result)=>{if(request.op==='composerEditorApply'&&!terminal){terminal=true;await f.send(obj(result.value).event);await f.send(f.event('text','later text'));}return result});
 expect(await pickAction(f)).toBe('');expect(f.read().text).toBe('later text');expect(f.read().files).toEqual([]);
 const command=obj(f.calls.find(c=>c.op==='composerEditorApply')!.command);await f.send(f.event('text',String(obj(command.next).value)));
 expect(f.read().files[0]?.id).toBe(uuid(73));expect(f.read().files[0]?.attachmentId).toBe('');expect(f.bytes.has(uuid(73))).toBe(true);
});
test('lost successful finish reply retries the old terminal after later text without a second picker or edit',async()=>{
 const f=await pickerFixture('');f.pick([incoming(74,'image')]);let lost=false;
 f.nativeHook((request,result)=>{if(request.op==='composerPickerIntake'&&request.action==='finish'&&!lost){lost=true;throw Error('lost finish reply')}return result});
 expect(await pickAction(f)).toContain('lost finish reply');await f.send(f.event('text',f.read().text+'newer'));
 expect(await pickAction(f)).toBe('');expect(f.read().text.endsWith('newer')).toBe(true);
 expect(f.calls.filter(c=>c.action==='pick')).toHaveLength(1);expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);
 const finishes=f.calls.filter(c=>c.action==='finish');expect(finishes).toHaveLength(2);expect(finishes[1]).toEqual(finishes[0]);
});
test('file removal uses native edit then Undo restores same issued file and image removal restores context only',async()=>{
 const f=await pickerFixture('');f.pick([incoming(75),incoming(76,'image')]);expect(await pickAction(f)).toBe('');const both=f.read().text;
 const tile=mobileComposerAttachments(f.client).items.find(a=>a.id===uuid(75))!;
 // Exact composer-attachments.contract press mapping, driven through the public action.
 expect(await pickAction(f,tile.removeOperation==='remove-snapshot'?'remove-image':'remove-file',tile.id)).toBe('');expect(f.read().attachmentIds).toEqual([uuid(76)]);
 await f.send(f.event('text',both));expect(f.read().files[0]?.id).toBe(uuid(75));
 expect(await pickAction(f,'remove-image',uuid(76))).toBe('');await f.send(f.event('text',both));
 expect(f.read().images).toEqual([]);expect(f.read().context?.records.some(r=>r.kind==='image')).toBe(true);
});
test('mobile picker capability has no unknown-capability video fallback and Files does not require upload flag',async()=>{
 const f=await pickerFixture();f.client.config.environment={capabilities:{}};
 expect(mobileComposerPickerFileLimit(f.client,'photos')).toBe(0);expect(await pickAction(f)).toContain('does not support');expect(f.intakes.size).toBe(0);
 f.client.config.environment={capabilities:{fileAttachments:{maxUploadBytes:123}}};
 expect(mobileComposerPickerFileLimit(f.client,'files')).toBe(123);expect(mobileComposerPickerFileLimit(f.client,'photos')).toBe(0);
});

test('typing while actual issued hold awaits appends to latest text; returning to captured text uses original range',async()=>{
 for(const aba of [false,true]){
  const f=await pickerFixture();await f.send(f.event('selection','left right',{selection:{start:5,end:5}}));f.pick([incoming(77)]);let changed=false;
  f.nativeHook(async(request,result)=>{if(request.op==='composerPickerIntake'&&request.action==='hold'&&!changed){changed=true;await f.send(f.event('text','changed'));if(aba)await f.send(f.event('text','left right'));}return result});
  expect(await pickAction(f)).toBe('');expect(f.read().text.startsWith(aba?'left [picked77.txt]':'changed [picked77.txt]')).toBe(true);
  expect([...f.holds.values()].every(h=>!h.released)).toBe(true);
 }
});
test('metadata conflict after native application preserves raw text without inventing incoming attachment ownership',async()=>{
 const f=await pickerFixture('');f.pick([incoming(78)]);let changed=false;
 f.nativeHook((request,result)=>{if(request.op==='composerEditorApply'&&!changed){changed=true;
  const key=JSON.stringify([f.client.origin,f.client.environmentId,f.target.key]);
  mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{[key]:{origin:f.client.origin,environmentId:f.client.environmentId,key:f.target.key,revision:10,text:'',context:{version:1,records:[]}}}}});
 }return result});
 expect(await pickAction(f)).toContain('metadata changed');expect(f.read().files).toEqual([]);expect(f.read().context?.records??[]).toEqual([]);
 const raw=f.read().text;expect(raw).toContain(uuid(78));expect(mobileEditorOwner(f.client)?.pending).toBeNull();
 await f.send(f.event('text',raw+' later'));expect(f.read().text).toBe(raw+' later');expect(f.read().files).toEqual([]);
 expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);
});
test('successful image publication with failed save is saved in its original named draft after navigation, never cancelled',async()=>{
 const f=await pickerFixture('');f.pick([incoming(79,'image')]);let failSave=true;
 f.saveHook(()=>{if(failSave){failSave=false;throw Error('save failed')}});
 expect(await pickAction(f)).toContain('save failed');expect(f.read().images).toHaveLength(1);expect(obj(f.disk().snapshotDrafts)[f.target.key]??[]).toEqual([]);
 f.client.threadId='other';f.route.active=false;f.view();
 expect(await pickAction(f)).toContain('previous editor closed');
 expect((obj(f.disk().snapshotDrafts)[f.target.key] as Obj[])[0]?.id).toBe(uuid(79));expect([...f.intakes.values()][0]?.status).toBe('finished');
 expect(f.calls.some(c=>c.action==='cancel')).toBe(false);expect(f.client.local.drafts[f.client.draftKey]??'').toBe('');
});

test('lost applied CAS reply retries only its exact command and issued operation',async()=>{
 const f=await pickerFixture('');f.pick([incoming(80)]);let lost=false;
 f.nativeHook((request,result)=>{if(request.op==='composerEditorApply'&&!lost){lost=true;throw Error('lost CAS')}return result});
 expect(await pickAction(f)).toContain('lost CAS');expect(f.read().files).toEqual([]);
 expect(await pickAction(f)).toBe('');expect(f.read().files[0]?.id).toBe(uuid(80));
 const commands=f.calls.filter(c=>c.op==='composerEditorApply');expect(commands).toHaveLength(2);expect(commands[1]).toEqual(commands[0]);
 expect(f.calls.filter(c=>c.action==='pick')).toHaveLength(1);
});
test('lost CAS then retired admission reconciles saved original inventory without replaying the command into another thread',async()=>{
 const f=await pickerFixture('');f.pick([incoming(81,'image')]);let lost=false;
 f.nativeHook((request,result)=>{if(request.op==='composerEditorApply'&&!lost){lost=true;throw Error('lost CAS')}return result});
 expect(await pickAction(f)).toContain('lost CAS');f.client.threadId='other';f.route.active=false;f.view();
 expect(await pickAction(f)).toContain('previous editor closed');expect([...f.intakes.values()][0]?.status).toBe('finished');
 expect(f.read().images).toEqual([]);expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);
 const finish=f.calls.find(c=>c.action==='finish')!;expect(obj(finish.publication).discardedIds).toEqual([uuid(81)]);
});
test('native rejection adds no imported metadata and cancellation settles issued ownership',async()=>{
 const f=await pickerFixture('');f.pick([incoming(82)]);
 f.nativeHook((request,result)=>request.op==='composerEditorApply'?{...result,value:{event:{...obj(obj(result.value).event),kind:'commandRejected',reason:'document-changed',value:'',selection:{start:0,end:0}}}}:result);
 expect(await pickAction(f)).not.toBe('');expect(f.read().files).toEqual([]);expect(f.read().text).toBe('');expect(f.read().context).toBeNull();
 await pickAction(f);expect([...f.intakes.values()][0]?.status).toBe('cancelled');expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);
});
test('staged status permits a fresh saved finish proof after a newer draft save overtook the first proof',async()=>{
 const f=await pickerFixture('');f.pick([incoming(83,'image')]);let rejected=false;
 f.beforeNative(async request=>{if(request.op==='composerPickerIntake'&&request.action==='finish'&&!rejected){rejected=true;await f.send(f.event('text',f.read().text+' later'));return {ok:false,generation:1,error:{kind:'PickerIntake',message:'saved projection changed'}}}return undefined});
 expect(await pickAction(f)).toContain('saved projection changed');expect([...f.intakes.values()][0]?.status).toBe('staged');
 expect(await pickAction(f)).toBe('');const finishes=f.calls.filter(c=>c.action==='finish');expect(finishes).toHaveLength(2);expect(finishes[1]).not.toEqual(finishes[0]);
 expect(obj(obj(finishes[1]!.publication).after).text).toBe(f.read().text);expect(f.calls.filter(c=>c.op==='composerEditorApply')).toHaveLength(1);
});
test('cold context-only image is not laundered into byte or historical authority by a first edit',async()=>{
 const text=reference({kind:'image',contextId:uuid(84),label:'missing.png'}),f=await fixture(0,text);
 const key=JSON.stringify([f.client.origin,f.client.environmentId,f.target.key]);
 mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{[key]:{origin:f.client.origin,environmentId:f.client.environmentId,key:f.target.key,revision:1,text,
  context:{version:1,records:[{version:1,kind:'image',contextId:uuid(84),label:'missing.png',attachmentId:uuid(84),name:'missing.png',mimeType:'image/png',sizeBytes:1}]}}}}});
 await f.mount();const before=f.read();const answer=await f.send(f.event('text',text+' later'));
 expect(answer.message).not.toBe('');expect(f.read()).toEqual(before);expect(f.holds.size).toBe(0);expect(f.bytes.size).toBe(0);
});

test('temporary voice inhibition of the same unresolved CAS cannot release its incoming image authority',async()=>{
 const f=await pickerFixture('');f.pick([incoming(85,'image')]);let lost=false;
 f.nativeHook((request,result)=>{if(request.op==='composerEditorApply'&&!lost){lost=true;throw Error('lost CAS')}return result});
 expect(await pickAction(f)).toContain('lost CAS');f.route.voiceBusy=true;f.view();
 expect(await pickAction(f)).toContain('original editor still owns');expect(f.calls.some(c=>c.action==='finish'||c.action==='cancel')).toBe(false);
 f.route.voiceBusy=false;f.view();expect(await pickAction(f)).toBe('');expect(f.read().images[0]?.id).toBe(uuid(85));
});

test('lost empty picker finish reconciles its adopted operation after editor retirement without cancelling it',async()=>{
 for(const displaced of [false,true]){
 const f=await pickerFixture('');f.pick([]);let lost=false;
 f.nativeHook((request,result)=>{if(request.op==='composerPickerIntake'&&request.action==='finish'&&!lost){lost=true;throw Error('lost empty finish')}return result});
 expect(await pickAction(f)).toContain('lost empty finish');expect([...f.intakes.values()][0]?.status).toBe('finished');
 f.beforeNative(request=>request.op==='composerPickerIntake'&&request.action==='cancel'?{ok:false,generation:1,error:{kind:'PickerIntake',message:'An adopted picker cannot be cancelled.'}}:undefined);
 const origin=f.client.origin;if(displaced)f.client.origin='https://replacement.test';f.route.active=false;f.view();
 expect(await pickAction(f)).toContain('previous editor closed');expect(f.calls.some(c=>c.action==='cancel')).toBe(false);
 f.client.origin=origin;f.route.active=true;await f.mount();f.pick([incoming(86)]);
 expect(await pickAction(f)).toBe('');expect(f.read().files[0]?.id).toBe(uuid(86));
 }
});
