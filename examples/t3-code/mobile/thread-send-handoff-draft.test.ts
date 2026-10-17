// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import {expect,spyOn,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {mobileThreadSendCaptureDraft as capture,mobileThreadSendApplyQueuedDraft as apply} from './thread-send-handoff-draft';
import {mobileComposerContextSendSnapshot as snapshot,mobileComposerContextsHydrate as hydrate,mobileComposerContextsPersisted as contexts,
  mobileComposerContextCaptureTarget,mobileComposerContextObserveTarget,mobileComposerContextCommitBatch} from './composer-command-context';
import {mobileEditorOwnerAdmit,type EditorRouteInput} from './composer-editor-owner';
import * as durable from './composer-editor-persistence';
import {fleet} from './shared/settings-b-fleet';
import type {MobileComposerTarget} from './composer-target';
import type {ThreadSendTransferClaim,ThreadSendTransferCapture} from './thread-send-transfer-model';
import type {MobileOutboxAttachment} from './mobile-outbox-model';
import {obj,type Obj} from './shared/domain';
import type {Files,Native} from './shared/protocol';
import {mobileDraftChanged} from './draft';
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
let serial=0;
async function fixture(text='  draft  '){
 const client=new MobileDraftClient();
 await client.refresh({available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Fixture offline'}}}},
  {fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(){}}});
 Object.assign(client,{origin:'https://handoff.test',environmentId:`send-${++serial}`,threadId:'thread',generation:4});
 const key=client.draftKey;client.local.drafts[key]=text;fleet.saved.push({origin:client.origin,environmentId:client.environmentId});
 const target:MobileComposerTarget={kind:'ordinary',origin:client.origin,environmentId:client.environmentId,generation:4,projectId:client.projectId,threadId:'thread',key,editorOwner:key,editOwner:'',incarnation:'',owner:JSON.stringify(['ordinary',client.origin,client.environmentId,4,key])};
 return {client,target,key};
}
type Fixture=Awaited<ReturnType<typeof fixture>>;
const local=(f:Fixture)=>f.client.local as typeof f.client.local & {composerFiles?:any;mobileAttachmentOrder?:any;mobileNewTaskDrafts?:any;mobileOutboxTransferCompletions?:any};
const file=(f:Fixture,n=2)=>({id:id(n),contextId:`f${n}`,draftKey:f.key,environmentId:f.target.environmentId,name:`${n}.txt`,mimeType:'text/plain',sizeBytes:5,source:'attached',attachmentId:'',status:'staged' as const,extra:{keep:true}});
const image=(n=1)=>({id:id(n),name:'image.png',mimeType:'image/png',sizeBytes:6,source:{kind:'capture',opaque:'kept'}});
const context=(f:Fixture)=>({version:1,records:[{version:1,kind:'file',contextId:'alternate',label:'2.txt',attachmentId:id(2),name:'2.txt',mimeType:'text/plain',sizeBytes:5},{version:1,kind:'skill',contextId:'unused',label:'Unused',name:'unused'}]});
function seed(f:Fixture){
 f.client.local.snapshotDrafts[f.key]=[image()];local(f).composerFiles=[file(f)];local(f).mobileAttachmentOrder={[f.key]:['stale',id(2)]};
 hydrate(f.client,{mobileComposerContexts:{version:1,entries:{[JSON.stringify([f.client.origin,f.target.environmentId,f.key])]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.key,revision:3,text:f.client.draft,context:context(f)}}}});
}
function captured(f:Fixture){const result=capture(f.client,f.target);expect(result.status).toBe('captured');if(result.status!=='captured')throw Error(result.reason);return result.capture}
function claim(c:ThreadSendTransferCapture):ThreadSendTransferClaim {
 const d=c.draft,attachments:MobileOutboxAttachment[]=d.attachmentIds.map(localId=>{const i=d.images.find(i=>i.id===localId);if(i)return {id:localId,kind:'image',name:String(i.name),mimeType:String(i.mimeType),sizeBytes:Number(i.sizeBytes),uploadId:'',status:'staged',...(i.source===undefined?{}:{source:i.source as Obj})};const f=d.files.find(f=>f.id===localId)!;return {id:f.id,kind:'file',name:f.name,mimeType:f.mimeType,sizeBytes:f.sizeBytes,uploadId:'',status:'staged',contextId:f.contextId,source:f.source}});
 return {kind:'ordinary',origin:d.origin,environmentId:d.environmentId,transferId:'message',draftKey:d.key,fingerprint:'a'.repeat(64),messageId:'message',threadId:d.threadId,commandId:'command',mutationId:'epoch:1',state:'queued',capture:c,
 record:{schemaVersion:1,origin:d.origin,environmentId:d.environmentId,threadId:d.threadId,messageId:'message',commandId:'command',text:d.text.trim(),attachments,...(d.context?{context:d.context}:{}),modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default',createdAt:'2026-10-09T00:00:00.000Z'}};
}
const document=(f:Fixture)=>durable.mobileEditorDocument(f.client,durable.mobileEditorDocumentKey({origin:f.client.origin,environmentId:f.target.environmentId,draftKey:f.key}));
function state(f:Fixture){return structuredClone({drafts:f.client.local.drafts,snapshotDrafts:f.client.local.snapshotDrafts,files:local(f).composerFiles,orders:local(f).mobileAttachmentOrder,releases:f.client.local.snapshotReleases,
 cleanup:local(f).mobileNewTaskDrafts,completions:local(f).mobileOutboxTransferCompletions,doc:document(f),context:snapshot(f.client,f.target),revision:f.client.revision})}
function write(f:Fixture,text:string){const guard=mobileComposerContextCaptureTarget(f.client,f.target)!,before=f.client.draft;f.client.local.drafts[f.key]=text;expect(durable.mobileEditorDocumentWritten(f.client,f.target,before,text)).toBe(true);expect(mobileComposerContextObserveTarget(f.client,guard,text)).toBe(true)}
function cleared(f:Fixture,q:ThreadSendTransferClaim){const r=apply(f.client,q);expect(r.status).toBe('applied');if(r.status==='blocked')throw Error(r.reason);expect(r.marker.disposition).toBe('cleared');return r}

test('capture explicitly enrolls only after complete validation and preserves raw text/full context/order',async()=>{
 const f=await fixture();seed(f);expect(document(f)).toBeNull();const c=captured(f);expect(c.draft.text).toBe('  draft  ');expect(c.draft.context).toEqual(context(f));expect(c.draft.contextRevision).toBe(3);
 expect(c.draft.attachmentIds).toEqual([id(2),id(1)]);expect(c.draft.attachmentOrder).toEqual(['stale',id(2)]);expect(c.draft.document.revision).toBe(0);expect(document(f)?.value).toBe('  draft  ');
 c.draft.files[0]!.name='mutated';expect(local(f).composerFiles[0].name).toBe('2.txt');
});
test('exact queued proof clears all named content once while preserving foreign raw rows and cleanup queues',async()=>{
 const f=await fixture();seed(f);const foreign={...file(f,8),draftKey:'foreign:thread',environmentId:'foreign',unknown:{x:[1,2]}};
 local(f).composerFiles.push(foreign);f.client.local.snapshotDrafts['foreign:thread']=[image(9)];local(f).mobileAttachmentOrder['foreign:thread']=['foreign-stale',id(9),id(8)];
 local(f).mobileNewTaskDrafts={version:1,records:{future:{opaque:true}},receipts:{opaque:[1,2]},claims:{x:'future'},fileReleases:[id(20)]};f.client.local.snapshotReleases=[id(21)];
 const q=claim(captured(f)),before=f.client.revision,r=cleared(f,q);expect(f.client.revision).toBe(before+1);expect(f.client.draft).toBe('');expect(document(f)?.revision).toBe(1);expect(document(f)?.selection).toEqual({start:0,end:0});
 expect(local(f).composerFiles).toEqual([foreign]);expect(f.client.local.snapshotDrafts[f.key]).toEqual([]);expect(f.client.local.snapshotDrafts['foreign:thread']).toEqual([image(9)]);
 expect(local(f).mobileAttachmentOrder).toEqual({[f.key]:[],'foreign:thread':['foreign-stale',id(9),id(8)]});expect(f.client.local.snapshotReleases).toEqual([id(21),id(1)]);
 expect(local(f).mobileNewTaskDrafts).toEqual({version:1,records:{future:{opaque:true}},receipts:{opaque:[1,2]},claims:{x:'future'},fileReleases:[id(20),id(2)]});expect(r.marker.after.context).toBeNull();
 const saved=state(f);expect(apply(f.client,q).status).toBe('already-applied');expect(state(f)).toEqual(saved);
});
test('changed document revision including text ABA preserves actual current projection without partial subtraction',async()=>{
 for(const aba of [false,true]){const f=await fixture(),q=claim(captured(f));write(f,'new');if(aba)write(f,'  draft  ');const before=state(f),r=apply(f.client,q);expect(r.status).toBe('applied');if(r.status==='blocked')continue;
 expect(r.marker.disposition).toBe('preserved');expect(f.client.draft).toBe(before.drafts[f.key]);expect(document(f)).toEqual(before.doc);expect(r.marker.after.document.revision).toBe(aba?2:1)}
});
test('context-only/inventory-only/raw order changes preserve, while unrelated foreign updates do not stop clear',async()=>{
 for(const kind of ['context','file','image','order','foreign'] as const){const f=await fixture();seed(f);const q=claim(captured(f));
 if(kind==='context'){expect(mobileComposerContextCommitBatch(f.client,mobileComposerContextCaptureTarget(f.client,f.target)!,[])).toBe(true)}
 if(kind==='file')local(f).composerFiles[0].extra.keep=false;if(kind==='image')f.client.local.snapshotDrafts[f.key]![0]!.name='changed.png';if(kind==='order')local(f).mobileAttachmentOrder[f.key]=['different-stale',id(2)];if(kind==='foreign')local(f).composerFiles.push({...file(f,8),draftKey:'other:thread',environmentId:'other'});
 const r=apply(f.client,q);expect(r.status).toBe('applied');if(r.status==='blocked')continue;expect(r.marker.disposition).toBe(kind==='foreign'?'cleared':'preserved');if(kind!=='foreign')expect(f.client.draft).toBe('  draft  ')}
});
test('different current enrolled incarnation preserves even lower revision, never clears replacement',async()=>{
 const f=await fixture(),q=claim(captured(f));document(f)!.incarnation='replacement';document(f)!.revision=0;const r=apply(f.client,q);expect(r.status).toBe('applied');if(r.status!=='blocked'){expect(r.marker.disposition).toBe('preserved');expect(r.marker.after.document.incarnation).toBe('replacement')}
});
test('blocked missing or unobserved document cannot forge a preserve completion',async()=>{
 for(const kind of ['blocked','unobserved'] as const){const f=await fixture(),q=claim(captured(f));if(kind==='blocked')document(f)!.blocked=true;else f.client.local.drafts[f.key]='unobserved';const before=state(f);expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(before)}
 const f=await fixture(),q=claim(captured(f)),other=await fixture();Object.assign(other.client,{origin:f.client.origin,environmentId:f.target.environmentId,threadId:'thread'});expect(apply(other.client,q).status).toBe('blocked');
});
test('queued claim must decode exactly; prepared/changed record or foreign catalog leaves everything untouched',async()=>{
 const f=await fixture(),q=claim(captured(f));for(const change of [(x:any)=>x.state='prepared',(x:any)=>x.record.text='changed',(x:any)=>x.capture.draft.document.revision=-1]){const input=structuredClone(q);change(input);const before=state(f);expect(apply(f.client,input).status).toBe('blocked');expect(state(f)).toEqual(before)}
 fleet.saved.find(v=>v.environmentId===f.target.environmentId)!.origin='https://replaced.test';const before=state(f);expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(before);
});
test('capture and completion refuse rich admission even before native ready or with different incarnation',async()=>{
 for(const mountId of ['', 'native']){const f=await fixture(),q=claim(captured(f));const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:f.target.environmentId,threadId:'thread',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const owner=mobileEditorOwnerAdmit(f.client,f.target,route,'catalog')!;owner.state.mountId=mountId;owner.document.incarnation='other';const before=state(f);expect(capture(f.client,f.target).status).toBe('blocked');expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(before)}
});
test('raw malformed stores never enroll, normalize or partially publish',async()=>{
 const changes:Array<(f:Fixture)=>void>=[f=>local(f).mobileNewTaskDrafts=null,f=>local(f).composerFiles=null,f=>local(f).mobileAttachmentOrder=null,
 f=>local(f).mobileOutboxTransferCompletions=[],f=>{f.client.local.snapshotDrafts.foreign=[{id:'x'} as never]},f=>{local(f).composerFiles=[{...file(f),status:['ready']}]},
 f=>{const a:any[]=[];a.length=1;(a as any).extra=3;local(f).mobileNewTaskDrafts={version:1,records:{opaque:a},receipts:{},claims:{},fileReleases:[]}}];
 for(const change of changes){const f=await fixture();change(f);const before=state(f);expect(capture(f.client,f.target).status).toBe('blocked');expect(document(f)).toBeNull();expect(state(f)).toEqual(before)}
 for(const change of changes){const f=await fixture(),q=claim(captured(f));change(f);const before=state(f);expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(before)}
});
test('one concrete text commit after preparation; injected refusal leaves context history marker and inventory untouched',async()=>{
 const f=await fixture();seed(f);const q=claim(captured(f)),before=state(f),mock=spyOn(durable,'mobileEditorDocumentCommit').mockReturnValue(false);
 try{expect(apply(f.client,q).status).toBe('blocked');expect(mock).toHaveBeenCalledTimes(1);expect(state(f)).toEqual(before)}finally{mock.mockRestore()}
 cleared(f,q);
});
test('completion can finish same named document after focus changes, but cannot borrow other environment',async()=>{
 const f=await fixture(),q=claim(captured(f));f.client.threadId='other';f.client.local.drafts[f.client.draftKey]='keep other';const r=apply(f.client,q);expect(r.status).toBe('applied');expect(f.client.local.drafts[f.key]).toBe('');expect(f.client.draft).toBe('keep other');
});
test('actual persist contains matching metadata/text/context/images/files/order/completion in one snapshot',async()=>{
 const f=await fixture();seed(f);const q=claim(captured(f)),r=cleared(f,q);let saved:Obj={};const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_p,bytes){saved=obj(JSON.parse(new TextDecoder().decode(bytes)))}}};
 await f.client.persist(storage);const key=JSON.stringify([f.client.origin,f.target.environmentId,f.key]);
 expect(obj(saved.drafts)[f.key]).toBe('');expect(obj(obj(saved.mobileComposerEditor).documents)[key]).toEqual(r.marker.after.document);
 expect(obj(obj(saved.mobileComposerContexts).entries)[key]??null).toBeNull();expect(obj(saved.snapshotDrafts)[f.key]).toEqual([]);expect(saved.composerFiles).toEqual([]);
 expect(obj(saved.mobileAttachmentOrder)[f.key]??null).toEqual(r.marker.after.attachmentOrder);expect(r.marker.after.attachmentOrder).toBeNull();expect(obj(saved.mobileOutboxTransferCompletions).message).toEqual(r.marker);
});

test('new input after accepted clear refreshes same-claim preserve proof and never clears twice',async()=>{
 const f=await fixture(),q=claim(captured(f));cleared(f,q);write(f,'new input');const r=apply(f.client,q);expect(r.status).toBe('applied');if(r.status==='blocked')return;
 expect(r.marker.disposition).toBe('preserved');expect(r.marker.before).toEqual(q.capture!.draft.document?{incarnation:q.capture!.draft.document.incarnation,revision:q.capture!.draft.document.revision}:null);
 expect(r.marker.after.text).toBe('new input');expect(f.client.draft).toBe('new input');const before=state(f);expect(apply(f.client,q).status).toBe('already-applied');expect(state(f)).toEqual(before);
 local(f).mobileOutboxTransferCompletions.message.fingerprint='b'.repeat(64);const corrupt=state(f);expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(corrupt);
});
test('revision exhaustion refuses before any document or marker publication',async()=>{
 const f=await fixture(),q=claim(captured(f));f.client.revision=Number.MAX_SAFE_INTEGER;const before=state(f);expect(apply(f.client,q).status).toBe('blocked');expect(state(f)).toEqual(before);
 const g=await fixture();captured(g);document(g)!.revision=Number.MAX_SAFE_INTEGER;const atLimit=claim(captured(g)),limit=state(g);expect(apply(g.client,atLimit).status).toBe('blocked');expect(state(g)).toEqual(limit);
});

test('actual plain input ledger remains current after queued clear and subsequent persisted typing',async()=>{
 const f=await fixture('');let saved:Obj={};
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_p,bytes){saved=obj(JSON.parse(new TextDecoder().decode(bytes)))}}};
 const native:Native={available:true,watch(){},async later(){return {ok:true,generation:f.client.generation,value:{}}}};
 expect((await mobileDraftChanged(f.client,'first',native,storage)).message).toBe('');
 const q=claim(captured(f));cleared(f,q);
 expect((await mobileDraftChanged(f.client,'next',native,storage)).message).toBe('');
 expect(document(f)?.value).toBe('next');expect(document(f)?.blocked).toBeUndefined();expect(document(f)?.revision).toBe(2);
 const key=JSON.stringify([f.client.origin,f.target.environmentId,f.key]);expect(obj(saved.drafts)[f.key]).toBe('next');
 expect(obj(obj(obj(saved.mobileComposerEditor).documents)[key]).blocked).toBeUndefined();
});
test('returned completion is detached from stored proof on initial publication and preserve refresh',async()=>{
 const f=await fixture(),q=claim(captured(f));const first=cleared(f,q);const stored=structuredClone(local(f).mobileOutboxTransferCompletions);
 first.marker.after.document.revision=900;expect(local(f).mobileOutboxTransferCompletions).toEqual(stored);
 write(f,'new');const next=apply(f.client,q);expect(next.status).toBe('applied');if(next.status==='blocked')return;
 const current=structuredClone(local(f).mobileOutboxTransferCompletions);next.marker.after.text='mutated';expect(local(f).mobileOutboxTransferCompletions).toEqual(current);
});

test('preserve proof records actual persisted effective order for stale, empty, and absent raw order',async()=>{
 for(const order of ['stale','empty','absent'] as const){
  const f=await fixture();seed(f);const q=claim(captured(f));
  if(order==='empty')local(f).mobileAttachmentOrder[f.key]=[];
  if(order==='absent')delete local(f).mobileAttachmentOrder[f.key];
  local(f).composerFiles[0].name='changed.txt';
  const result=apply(f.client,q);expect(result.status).toBe('applied');if(result.status==='blocked')continue;
  expect(result.marker.disposition).toBe('preserved');
  expect(result.marker.after.attachmentOrder).toEqual(order==='stale'?[id(2),id(1)]:[id(1),id(2)]);
  let saved:Obj={};await f.client.persist({fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_p,bytes){saved=obj(JSON.parse(new TextDecoder().decode(bytes)))}}});
  const scope=JSON.stringify([f.client.origin,f.target.environmentId,f.key]);
  expect({document:obj(obj(saved.mobileComposerEditor).documents)[scope],text:obj(saved.drafts)[f.key],
   context:obj(obj(saved.mobileComposerContexts).entries)[scope]??null,images:obj(saved.snapshotDrafts)[f.key]??[],
   files:(saved.composerFiles as Obj[]).filter(file=>file.draftKey===f.key),attachmentIds:obj(saved.mobileAttachmentOrder)[f.key]??[],
   attachmentOrder:obj(saved.mobileAttachmentOrder)[f.key]??null}).toEqual(result.marker.after);
  expect(obj(saved.mobileOutboxTransferCompletions).message).toEqual(result.marker);
  expect(apply(f.client,q).status).toBe('already-applied');
 }
});
