// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {ClientError,type Files,type Native} from './shared/protocol';
import type {Pending} from './shared/client';
import {obj,type Obj} from './shared/domain';
import {mobileComposerTarget} from './composer-target';
import {mobileEditorDocumentEnroll as enroll,mobileEditorDocumentCapture as capture,mobileEditorDocumentWritten as written,
 mobileEditorDocumentsHydrate as hydrate,mobileEditorPersistSnapshot as snap,mobileEditorPersistDocument as serialize,
 mobileEditorSendCapture as sendCapture,mobileEditorSendFailed as failed,mobileEditorSendConfirmed as confirm,
 mobileEditorRetirementSnapshot as retirements,mobileEditorRetirementReceipt as receipt,mobileEditorRetirementAllowed as allowed,
 mobileEditorPastePublish as pastePublish,mobileEditorPasteRemoveMarker as pasteRemove,mobileEditorPasteRetirements as pasteRetirements,
 mobileEditorPasteRetired as pasteRetired,mobileEditorDocumentKey as key,editorRetirementDecision,type EditorPasteMarker} from './composer-editor-persistence';
class Client extends MobileDraftClient {
 requests=0; response:()=>Promise<Obj>=async()=>({});
 override async request(){this.requests++;return this.response()}
 override async flushSnapshotReleases(){}
 complete(p:Pending,environmentId=this.environmentId){this.finishPending(p,environmentId)}
}
let serial=0;
function fixture(text='draft'){
 const client=new Client();Object.assign(client,{origin:'https://durable.test',environmentId:`durable-${++serial}`,threadId:'t',projectId:'p',generation:1,connection:'connected'});
 client.local.drafts[client.draftKey]=text;const target=mobileComposerTarget(client),writes:Obj[]=[];
 let write:((doc:Obj)=>Promise<void>)|undefined;
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_path,bytes){const doc=obj(JSON.parse(new TextDecoder().decode(bytes)));writes.push(doc);await write?.(doc)}}};
 const native:Native={available:true,watch(){},async later(){return {ok:true,generation:1,value:{}}}};
 const pending=(id='command',outgoing=text):Pending=>({method:'orchestration.dispatchCommand',payload:{type:'message.dispatch',commandId:id,threadId:'t',messageId:'m',text:outgoing,attachments:[]},threadId:'t',text:outgoing,description:'Send',uncertain:false});
 return {client,target,storage,native,writes,pending,hold:(fn:(doc:Obj)=>Promise<void>)=>{write=fn}};
}
const extension=(doc:Obj)=>obj(doc.mobileComposerEditor);
const first=(value:unknown)=>Object.values(obj(value))[0] as Obj;
function restore(f:ReturnType<typeof fixture>,saved:Obj){const c=new Client();Object.assign(c,{origin:f.client.origin,environmentId:f.client.environmentId,threadId:'t',projectId:'p',generation:2});
 c.local.drafts=structuredClone(saved.drafts) as Record<string,string>;c.local.pending=structuredClone(saved.pending??{}) as Record<string,Pending>;hydrate(c,saved);return c}

test('actual MobileDraftClient write commits receipt with exact pending and shared draft; preserves text and normal attachment cleanup',async()=>{
 const f=fixture();enroll(f.client,f.target);const p=f.pending();f.client.local.snapshotDrafts[f.target.key]=[{id:'local',uploadId:'wire',name:'image',mimeType:'image/png',sizeBytes:8}];p.payload.attachments=[{id:'wire'}];
 await f.client.write(f.native,f.storage,p);expect(f.client.requests).toBe(1);expect(f.client.draft).toBe('draft');expect(f.client.local.snapshotDrafts[f.target.key]).toEqual([]);expect(f.client.local.snapshotReleases).toEqual(['local']);expect(f.client.local.pending[f.client.environmentId]).toBeUndefined();
 const before=f.writes[0]!,d=first(extension(before).documents),r=first(extension(before).sends);expect(d.value).toBeUndefined();expect(obj(before.drafts)[f.target.key]).toBe('draft');expect(r.request).toBe('pending');expect(r.fingerprint).toBeUndefined();
 expect(first(extension(f.writes.at(-1)!).sends).phase).toBe('preserved');expect(first(extension(f.writes.at(-1)!).sends).request).toBe('terminal');
});
test('unenrolled legacy Send behavior remains unchanged',async()=>{const f=fixture();await f.client.write(f.native,f.storage,f.pending());expect(f.client.draft).toBe('');expect(f.writes[0]?.mobileComposerEditor).toBeUndefined()});
test('composed outgoing differs from captured document and remains actual frozen request',async()=>{
 const f=fixture('visible [context]');enroll(f.client,f.target);const p=f.pending('composed','serialized outgoing');await f.client.write(f.native,f.storage,p);
 const restored=restore(f,f.writes[0]!);const r=receipt(restored,retirements(restored).items[0]!.key)!;expect(r.before).toBe('visible [context]');expect(r.outgoing).toBe('serialized outgoing');expect(f.client.draft).toBe('visible [context]');
});
test('unobserved shared Send normalization persists text but blocks later authority without guessing a revision',async()=>{
 const f=fixture('before');const d=enroll(f.client,f.target)!;f.client.local.drafts[f.target.key]='normalized';await f.client.write(f.native,f.storage,f.pending('normalize','normalized'));
 const row=first(extension(f.writes[0]!).documents);expect(row.blocked).toBe(true);expect(row.revision).toBe(0);expect(obj(f.writes[0]!.drafts)[f.target.key]).toBe('normalized');expect(d.value).toBe('before');
 const c=restore(f,f.writes.at(-1)!);expect(enroll(c,mobileComposerTarget(c))).toBeNull();expect(c.draft).toBe('normalized');
});
test('corrupt managed membership preserves ALL ordinary completion text and original bytes',async()=>{
 const f=fixture();const raw={version:99,unknown:'must retain'};hydrate(f.client,{mobileComposerEditor:raw});const p=f.pending();f.client.local.pending[f.client.environmentId]=p;f.client.complete(p);
 expect(f.client.draft).toBe('draft');expect(f.client.pending).toBeUndefined();await f.client.persist(f.storage);expect(f.writes[0]?.mobileComposerEditor).toEqual(raw);
});
test('managed legacy pending without receipt preserves text after confirmation',()=>{const f=fixture();enroll(f.client,f.target);const p=f.pending();f.client.complete(p);expect(f.client.draft).toBe('draft');expect(retirements(f.client).items).toEqual([])});
test('lost initial atomic-write reply sends no request and retains inline exact command ownership',async()=>{
 const f=fixture();enroll(f.client,f.target);const p=f.pending(),sentinel={name:'FetchError',kind:'Aborted'};f.hold(async()=>{throw sentinel});
 await expect(f.client.write(f.native,f.storage,p)).rejects.toBe(sentinel);expect(f.client.requests).toBe(0);expect(f.client.pending).toBeUndefined();
 const r=receipt(f.client,retirements(f.client).items[0]!.key)!;expect(r.failure).toBe(true);expect(r.phase).toBe('pending');
 const saved=serialize(snap(f.client),{drafts:f.client.local.drafts,pending:{}}),wire=first(extension(saved).sends);expect(wire.request).toBe('inline');expect(wire.fingerprint).toBe(r.fingerprint);
 const c=restore(f,saved);expect(receipt(c,r.id)?.fingerprint).toBe(r.fingerprint);expect(sendCapture(c,p,false)?.created).toBe(false);
 // The possibly committed first snapshot independently reconstructs the exact original pending.
 expect(receipt(restore(f,f.writes[0]!),r.id)?.fingerprint).toBe(r.fingerprint);
});
test('same-command retry never captures newer ABA revision and refuses changed payload or terminal reuse',()=>{
 const f=fixture('A');enroll(f.client,f.target);const p=f.pending(),c=sendCapture(f.client,p,false)!;
 f.client.local.drafts[f.target.key]='B';written(f.client,f.target,'A','B');f.client.local.drafts[f.target.key]='A';written(f.client,f.target,'B','A');
 expect(sendCapture(f.client,p,true)?.id).toBe(c.id);expect(receipt(f.client,c.id)?.revision).toBe(0);expect(()=>sendCapture(f.client,{...p,payload:{...p.payload,text:'different'}},true)).toThrow();
 confirm(f.client,p,f.client.environmentId);expect(()=>sendCapture(f.client,p,false)).toThrow();expect(allowed(f.client,c.id)).toBe(false);
});
test('captured persistence snapshot is independent of a later write and saved row reloads exact source text',async()=>{
 const f=fixture('A');enroll(f.client,f.target);let release!:()=>void;const held=new Promise<void>(r=>release=r);f.hold(async()=>held);
 const save=f.client.persist(f.storage);f.client.local.drafts[f.target.key]='B';written(f.client,f.target,'A','B');release();await save;
 const c=restore(f,f.writes[0]!);expect(c.draft).toBe('A');expect(enroll(c,mobileComposerTarget(c))?.revision).toBe(0);
 const newer=serialize(snap(f.client),{drafts:f.client.local.drafts,pending:{}});expect(enroll(restore(f,newer),mobileComposerTarget(restore(f,newer)))?.value).toBe('B');
});
test('serialization refuses mismatched captured bytes rather than stamping later text',()=>{
 const f=fixture('A');enroll(f.client,f.target);const captured=snap(f.client);expect(()=>serialize(captured,{drafts:{[f.target.key]:'B'},pending:{}})).toThrow();
});
test('receipt compaction never reconstructs an unresolved request from missing or different pending',async()=>{
 const f=fixture();enroll(f.client,f.target);await f.client.write(f.native,f.storage,f.pending());const saved=structuredClone(f.writes[0]!);saved.pending={};const c=restore(f,saved);c.complete(f.pending());expect(c.draft).toBe('draft');expect(enroll(c,mobileComposerTarget(c))).toBeNull();
});
test('incomplete production policy rejects persisted complete authority; pure policy also rejects ABA and blocked docs',()=>{
 const f=fixture('A'),d=enroll(f.client,f.target)!,p=f.pending(),c=sendCapture(f.client,p,false)!,r=receipt(f.client,c.id)!;
 const ready={...r,phase:'confirmed' as const,authority:'complete-v1' as const};expect(editorRetirementDecision(ready,d,true)).toBe('clear');expect(editorRetirementDecision(ready,{...d,revision:2},true)).toBe('preserve');expect(editorRetirementDecision(ready,{...d,blocked:true},true)).toBe('preserve');
 const raw=snap(f.client);obj(obj(raw.raw).sends)[r.id]=ready;const saved=serialize(raw,{drafts:f.client.local.drafts,pending:{}}),restored=restore(f,saved);expect(allowed(restored,r.id)).toBe(false);
});
function marker(f:ReturnType<typeof fixture>):EditorPasteMarker{
 const d=enroll(f.client,f.target)!;f.client.local.snapshotDrafts[f.target.key]=[{id:'00000000-0000-4000-8000-000000000001',name:'pasted-image.png',mimeType:'image/png',sizeBytes:4}];
 return {proof:{version:1,operationId:'00000000-0000-4000-8000-000000000002',identity:{owner:'owner',editorId:'editor',routeVisit:'visit',renderEpoch:'epoch',mountId:'mount'},richEventId:'00000000-0000-4000-8000-000000000003',
  target:{origin:d.origin,environmentId:d.environmentId,draftKey:d.draftKey,incarnation:d.incarnation,capturedRevision:d.revision},
  files:[{leaseId:'00000000-0000-4000-8000-000000000004',id:'00000000-0000-4000-8000-000000000001',kind:'image',name:'pasted-image.png',mimeType:'image/png',sizeBytes:4,sha256:'a'.repeat(64)}]},
  publication:{origin:d.origin,environmentId:d.environmentId,draftKey:d.draftKey,incarnation:d.incarnation,revision:d.revision}};
}
test('paste publication requires actual metadata, remains historical after typing, and cleanup survives marker removal snapshot',async()=>{
 const f=fixture();const m=marker(f);expect(pastePublish(f.client,m)).toBe(true);expect(pastePublish(f.client,{...m,publication:{...m.publication,revision:99}})).toBe(false);
 f.client.local.drafts[f.target.key]='new text';written(f.client,f.target,'draft','new text');await f.client.persist(f.storage);
 expect(obj(f.writes[0]?.composerPasteAdoptions)[m.proof.operationId]).toEqual(m);expect(first(extension(f.writes[0]!).documents).revision).toBe(1);
 expect(pasteRemove(f.client,m.proof,{status:'staged',proof:m.proof})).toBe(false);expect(pasteRemove(f.client,m.proof,{status:'adopted',proof:m.proof})).toBe(true);
 await f.client.persist(f.storage);const saved=f.writes.at(-1)!;expect(saved.composerPasteAdoptions).toBeUndefined();expect(pasteRetirements(restore(f,saved))).toEqual([m.proof]);
 expect(pasteRetired(f.client,m.proof,{status:'retired',proof:{}})).toBe(false);expect(pasteRetired(f.client,m.proof,{status:'retired',proof:m.proof})).toBe(true);expect(pasteRetirements(f.client)).toEqual([]);
});
test('wrong paste inventory cannot produce a durable adoption marker',()=>{const f=fixture(),m=marker(f);f.client.local.snapshotDrafts[f.target.key]=[];expect(pastePublish(f.client,m)).toBe(false)});
test('bounded enrollment refuses new ownership without evicting an unresolved receipt',()=>{
 const f=fixture();enroll(f.client,f.target);const r=sendCapture(f.client,f.pending(),false)!;
 for(let n=1;n<256;n++){const target={...f.target,threadId:`t${n}`,key:`${f.client.environmentId}:t${n}`};expect(enroll(f.client,target)).not.toBeNull()}
 expect(()=>enroll(f.client,{...f.target,threadId:'overflow',key:`${f.client.environmentId}:overflow`})).toThrow();expect(receipt(f.client,r.id)).not.toBeNull();
});

test('invalid wire metadata cannot be laundered through an internal decode during persist',async()=>{
 const f=fixture();enroll(f.client,f.target);await f.client.persist(f.storage);const saved=structuredClone(f.writes[0]!);
 first(extension(saved).documents).value='draft';const c=restore(f,saved);expect(enroll(c,mobileComposerTarget(c))).toBeNull();
 const output=serialize(snap(c),{drafts:c.local.drafts,pending:{}});expect(output.mobileComposerEditor).toEqual(saved.mobileComposerEditor);
 const again=restore(f,output);expect(enroll(again,mobileComposerTarget(again))).toBeNull();again.complete(f.pending());expect(again.draft).toBe('draft');
});

test('native-issued paste proof shape is preserved; malformed UUID, MIME and names refuse',()=>{
 const f=fixture(),m=marker(f);
 for(const file of [{...m.proof.files[0]!,id:'arbitrary-path'},{...m.proof.files[0]!,mimeType:'image/jpeg'},{...m.proof.files[0]!,name:'other.png'}]){
  expect(pastePublish(f.client,{...m,proof:{...m.proof,files:[file]}})).toBe(false);
 }
 expect(pastePublish(f.client,m)).toBe(true);
});
