// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,spyOn,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Files} from './shared/protocol';
import {mobileComposerTarget} from './composer-target';
import * as durable from './composer-editor-persistence';
import {mobileEditorContextInsertionPreflight as preflight,mobileEditorContextTargetCurrent as targetCurrent,mobileEditorCaptureContextInsertion as capture,mobileEditorContextInsertionCurrent as current,mobileEditorCommitContextInsertion as commit,
 mobileEditorCaptureDocumentIntent as strictCapture,mobileEditorCommitDocumentContextIntent as strictCommit,
 mobileEditorOwnerAdmit as admit,mobileEditorOwner as owner,mobileEditorOwnerRevision as ownerRevision,type EditorRouteInput} from './composer-editor-owner';
import {mobileComposerContextCaptureTarget as contextCapture,mobileComposerContextCommitBatch as batch,mobileComposerContextRead as read,
 mobileComposerContextsPersisted as contexts,mobileComposerContextsHydrate as hydrate} from './composer-command-context';
const uuid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const link=(id:string,kind='terminal')=>`[${id}](t3-context://v1/${kind}/${id})`;
const terminal=(contextId='term'):Obj=>({version:1,kind:'terminal',contextId,label:contextId,terminalId:'pty',terminalLabel:'Shell',lineStart:1,lineEnd:2,text:'output'});
const review=(contextId='comment'):Obj=>({version:1,kind:'review-comment',contextId,label:contextId,sectionId:'s',sectionTitle:'Worktree',filePath:'a.ts',startIndex:0,endIndex:0,rangeLabel:'L1',text:'Please change',diff:'-old\n+new'});
const content=(record=terminal())=>({text:link(String(record.contextId),String(record.kind)),context:{version:1 as const,records:[record]}});
let serial=0;
function fixture(text='ab') {
 const client=new MobileDraftClient();Object.assign(client,{origin:'https://semantic.test',environmentId:`semantic-${++serial}`,threadId:'a',generation:4});
 client.local.drafts[client.draftKey]=text;const target=mobileComposerTarget(client);fleet.saved.push({environmentId:target.environmentId,origin:client.origin});
 const document=durable.mobileEditorDocumentEnroll(client,target)!;
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:target.environmentId,threadId:'a',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 return {client,target,document,route};
}
type Fixture=ReturnType<typeof fixture>;
const local=(f:Fixture)=>f.client.local as typeof f.client.local & {composerFiles?:any;mobileAttachmentOrder?:any;mobileNewTaskDrafts?:any};
function write(f:Fixture,text:string,records:Obj[]=[],selection={start:text.length,end:text.length}) {
 return strictCommit(f.client,strictCapture(f.client,f.target,'test')!,{value:text,selection},records);
}
function seed(f:Fixture,records:Obj[]){expect(batch(f.client,contextCapture(f.client,f.target)!,records)).toBe(true)}
function saved(f:Fixture){return structuredClone({drafts:f.client.local.drafts,document:f.document,contexts:contexts(f.client),snapshotDrafts:f.client.local.snapshotDrafts,
 composerFiles:local(f).composerFiles,orders:local(f).mobileAttachmentOrder,cleanup:local(f).mobileNewTaskDrafts,snapshotReleases:f.client.local.snapshotReleases,
 revision:f.client.revision,ownerRevision:ownerRevision(f.client)})}
function file(f:Fixture,n=1){return {id:uuid(n),contextId:`f${n}`,draftKey:f.target.key,environmentId:f.target.environmentId,name:`f${n}.txt`,mimeType:'text/plain',sizeBytes:n,source:'attached',attachmentId:`upload${n}`,status:'ready' as const}}
function fileRecord(n=1):Obj{return {version:1,kind:'file',contextId:`f${n}`,label:`f${n}`,attachmentId:uuid(n),name:`f${n}.txt`,mimeType:'text/plain',sizeBytes:n}}

test('same text uses captured UTF16 range despite later caret and revision changes; success is single use',()=>{
 const f=fixture('😀ab'),c=capture(f.client,f.target,'terminal','route',{start:2,end:3})!;
 expect(write(f,'😀ab',[],{start:4,end:4}).ok).toBe(true);expect(current(f.client,c,'route')).toBe(true);
 const r=commit(f.client,c,content(),'route');expect(r.ok).toBe(true);expect(f.client.draft).toBe('😀 '+link('term')+' b');
 expect(f.document.selection).toEqual({start:3+link('term').length+1,end:3+link('term').length+1});
 expect(read(f.client)).toMatchObject({context:{records:[terminal()]}});expect(current(f.client,c,'route')).toBe(false);
 const before=saved(f);expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'superseded'});expect(saved(f)).toEqual(before);
});
test('text changed during producer await appends to latest; returning to same text preserves source ABA range',()=>{
 for(const aba of [false,true]) {
  const f=fixture('ab'),c=capture(f.client,f.target,'review','route',{start:1,end:1})!;
  expect(write(f,'newer text').ok).toBe(true);if(aba)expect(write(f,'ab').ok).toBe(true);
  expect(current(f.client,c,'route')).toBe(true);expect(commit(f.client,c,content(review()),'route').ok).toBe(true);
  expect(f.client.draft).toBe(aba?'a '+link('comment','review-comment')+' b':'newer text '+link('comment','review-comment')+' ');
 }
});
test('latest context-only and inventory changes are incorporated, not stale capture rejection',()=>{
 const f=fixture(link('old')),c=capture(f.client,f.target,'terminal','route')!;seed(f,[terminal('old')]);
 f.client.local.snapshotDrafts[f.target.key]=[{id:uuid(8),name:'new.png',mimeType:'image/png',sizeBytes:8,opaque:{keep:true}}];
 local(f).composerFiles=[file(f,2)];local(f).mobileAttachmentOrder={[f.target.key]:[uuid(2),uuid(8)]};
 expect(commit(f.client,c,content(),'route').ok).toBe(true);expect(read(f.client)).toMatchObject({context:{records:[terminal('old'),terminal()]}});
 expect(local(f).composerFiles).toEqual([file(f,2)]);expect(local(f).mobileAttachmentOrder[f.target.key]).toEqual([uuid(2),uuid(8)]);
 expect(f.client.local.snapshotDrafts[f.target.key]![0]).toHaveProperty('opaque',{keep:true});
});
test('selection replacement prunes context file, preserves images/noncontext files/foreign metadata and queues cleanup atomically',()=>{
 const text=link('f1','file'),f=fixture(text);seed(f,[fileRecord()]);const c=capture(f.client,f.target,'terminal','route',{start:0,end:text.length})!;
 const foreign={...file(f,9),draftKey:'foreign:thread',environmentId:'foreign',opaque:{retained:[1,2]}};
 local(f).composerFiles=[file(f),file(f,2),foreign];f.client.local.snapshotDrafts[f.target.key]=[{id:uuid(8),name:'i.png',mimeType:'image/png',sizeBytes:3}];
 f.client.local.snapshotDrafts['foreign:thread']=[{id:uuid(10),name:'foreign.png',mimeType:'image/png',sizeBytes:7}];
 local(f).mobileAttachmentOrder={[f.target.key]:[uuid(8),uuid(1),uuid(2)],'foreign:thread':[uuid(9),uuid(10)]};
 local(f).mobileNewTaskDrafts={version:1,records:{},receipts:{unknown:{raw:true}},claims:{},fileReleases:[uuid(7)]};f.client.local.snapshotReleases=[uuid(6)];
 const prior=f.client.revision,r=commit(f.client,c,content(),'route');expect(r).toMatchObject({ok:true,removedFileIds:[uuid(1)]});
 expect(f.client.revision).toBe(prior+1);expect(local(f).composerFiles).toEqual([file(f,2),foreign]);expect(local(f).mobileAttachmentOrder).toEqual({[f.target.key]:[uuid(8),uuid(2)],'foreign:thread':[uuid(9),uuid(10)]});
 expect(local(f).mobileNewTaskDrafts).toEqual({version:1,records:{},receipts:{unknown:{raw:true}},claims:{},fileReleases:[uuid(7),uuid(1)]});expect(f.client.local.snapshotReleases).toEqual([uuid(6)]);
 expect(f.client.local.snapshotDrafts['foreign:thread']).toHaveLength(1);expect(read(f.client)).toMatchObject({context:{records:[terminal()]}});
});
test('one concrete commit follows all preparation; its refusal leaves text/history/inventory untouched',()=>{
 const f=fixture('before'),c=capture(f.client,f.target,'terminal','route')!,before=saved(f),refuse=spyOn(durable,'mobileEditorDocumentCommit').mockReturnValue(false);
 try {expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'superseded'});expect(refuse).toHaveBeenCalledTimes(1);expect(saved(f)).toEqual(before)}finally{refuse.mockRestore()}
 expect(current(f.client,c,'route')).toBe(true);expect(commit(f.client,c,content(),'route').ok).toBe(true);
});
test('failure does not seed rejected context in undo history',()=>{
 const f=fixture('before'),c=capture(f.client,f.target,'terminal','route')!,before=saved(f);
 local(f).composerFiles=[{...file(f),status:['staged']}];const invalid=saved(f);expect(commit(f.client,c,content(),'route').ok).toBe(false);expect(saved(f)).toEqual(invalid);
 delete local(f).composerFiles;expect(write(f,link('term')).ok).toBe(true);const r=read(f.client);expect(r.ok&&r.context?.records.some(v=>v.contextId==='term')).not.toBe(true);
 expect(before.document.value).toBe('before');
});
test('limits and malformed unused raw context refuse before actual document write',()=>{
 const f=fixture('before'),c=capture(f.client,f.target,'review','route')!,before=saved(f),original=durable.mobileEditorDocumentCommit,call=spyOn(durable,'mobileEditorDocumentCommit').mockImplementation(original);
 try {
  for(const body of [{...content(),text:'x'.repeat(1_000_001)},content({...terminal(),lineEnd:-1}),{text:'x',context:{version:1 as const,records:[{version:1,kind:'mention',contextId:'m',label:'m',path:'m'}]}}]){
   expect(commit(f.client,c,body,'route').ok).toBe(false);expect(saved(f)).toEqual(before);
  }
  expect(call).not.toHaveBeenCalled();
 }finally{call.mockRestore()}
 const raw={version:1,entries:{[JSON.stringify([f.client.origin,f.target.environmentId,f.target.key])]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:0,text:'before',context:{version:1,records:[{...terminal('bad'),text:null}]}}}};
 hydrate(f.client,{mobileComposerContexts:raw});const invalid=saved(f);expect(commit(f.client,c,content(),'route').ok).toBe(false);expect(saved(f)).toEqual(invalid);
});
test('malformed owning cleanup store is retained rather than treated as empty',()=>{
 for(const raw of [null,[],{version:2,records:{},receipts:{},claims:{},fileReleases:[]},{version:1,records:[],receipts:{},claims:{},fileReleases:[]},{version:1,records:{},receipts:{},claims:{owner:3},fileReleases:[]}]){
  const f=fixture(),c=capture(f.client,f.target,'terminal','route')!;local(f).mobileNewTaskDrafts=raw;const before=saved(f);
  expect(preflight(f.client)).toBe(false);expect(targetCurrent(f.client,f.target)).toBe(true);expect(current(f.client,c,'route')).toBe(false);expect(capture(f.client,f.target,'terminal','route')).toBeNull();
  expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'invalid-inventory'});expect(saved(f)).toEqual(before);
 }
});
test('unsupported uploader binding refuses whole publication; no synthetic history restore for missing local file',()=>{
 for(const missing of [false,true]) {
  const f=fixture(link('alternate','file')),c=capture(f.client,f.target,'terminal','route')!;seed(f,[{...fileRecord(),contextId:'alternate',label:'alternate'}]);
  if(!missing)local(f).composerFiles=[file(f)];const before=saved(f);
  expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'unsupported'});expect(saved(f)).toEqual(before);
 }
});
test('original catalog, connection, named route policy and durable incarnation cannot be recaptured',()=>{
 for(const change of [
  (f:Fixture)=>{f.client.generation++},(f:Fixture)=>{f.client.origin='https://other.test'},(f:Fixture)=>{f.client.threadId='b'},
  (f:Fixture)=>{fleet.saved.find(v=>v.environmentId===f.target.environmentId)!.origin='https://replacement.test'},
  (f:Fixture)=>{f.document.incarnation='replacement'},(f:Fixture)=>{f.document.blocked=true},
 ]){
  const f=fixture(),c=capture(f.client,f.target,'terminal','route')!;change(f);const before=saved(f);
  expect(current(f.client,c,'route')).toBe(false);expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'superseded'});expect(saved(f)).toEqual(before);
 }
 const f=fixture(),c=capture(f.client,f.target,'terminal','route')!;expect(commit(f.client,c,content(),'different')).toEqual({ok:false,reason:'superseded'});
});
test('unenrolled/unavailable capture never enrolls or permits fallback; mount blocks independent of incarnation',()=>{
 const client=new MobileDraftClient();Object.assign(client,{origin:'https://none.test',environmentId:'none',threadId:'t',generation:1});const target=mobileComposerTarget(client);
 expect(capture(client,target,'terminal','route')).toBeNull();expect(durable.mobileEditorDocumentMembership(client,target)).toBe('unenrolled');
 const f=fixture(),c=capture(f.client,f.target,'terminal','route')!,active=admit(f.client,f.target,f.route,'catalog')!;active.state.mountId='native';active.document.incarnation='different';
 expect(capture(f.client,f.target,'terminal','route')).toBeNull();const before=saved(f);expect(commit(f.client,c,content(),'route').ok).toBe(false);expect(saved(f)).toEqual(before);
});
test('new same-producer capture supersedes old; mutations to plain receipt cannot alter captured authority',()=>{
 const f=fixture(),old=capture(f.client,f.target,'terminal','route')!,latest=capture(f.client,f.target,'terminal','route')!;
 expect(current(f.client,old,'route')).toBe(false);const changed=structuredClone(latest);changed.insertion.start=1;expect(current(f.client,changed,'route')).toBe(false);
 expect(commit(f.client,latest,content(),'route').ok).toBe(true);
});
test('durable selection fallback and owner ledger are prepared before accepted publication',()=>{
 const f=fixture('abc');expect(write(f,'abc',[],{start:1,end:2}).ok).toBe(true);const active=admit(f.client,f.target,f.route,'catalog')!,c=capture(f.client,f.target,'review','route')!;
 expect(c.insertion).toEqual({text:'abc',start:1,end:2});const r=commit(f.client,c,content(review()),'route');expect(r.ok).toBe(true);expect(owner(f.client)).toBeNull();
 expect(active.document.value).toBe(f.client.draft);expect(active.document.revision).toBe(f.document.revision);
 if(r.ok){r.selection.start=0;expect(active.document.selection).toEqual(f.document.selection)}
});
test('actual MobileDraftClient persist contains accepted text/context/revision/inventory/order/cleanup in one snapshot',async()=>{
 const text=link('f1','file'),f=fixture(text);seed(f,[fileRecord()]);local(f).composerFiles=[file(f)];local(f).mobileAttachmentOrder={[f.target.key]:[uuid(1)]};
 const c=capture(f.client,f.target,'terminal','route',{start:0,end:text.length})!;expect(commit(f.client,c,content(),'route').ok).toBe(true);
 let stored:Obj={};const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_p,bytes){stored=obj(JSON.parse(new TextDecoder().decode(bytes)))}}};
 await f.client.persist(storage);expect(obj(stored.drafts)[f.target.key]).toBe(link('term')+' ');expect(stored.composerFiles).toEqual([]);
 expect(obj(stored.mobileNewTaskDrafts).fileReleases).toEqual([uuid(1)]);expect(obj(stored.mobileAttachmentOrder)[f.target.key]).toBeUndefined();
 expect(obj(Object.values(obj(obj(stored.mobileComposerContexts).entries))[0]).text).toBe(f.client.draft);
 expect(obj(Object.values(obj(obj(stored.mobileComposerEditor).documents))[0]).revision).toBe(f.document.revision);
});

test('final context cap refuses without seeding, while a captured replacement can free one slot',()=>{
 const records=Array.from({length:200},(_,i)=>terminal(`t${i}`)),text=records.map(r=>link(String(r.contextId))).join(' '),f=fixture(text);seed(f,records);
 const rejected=capture(f.client,f.target,'terminal','route')!,before=saved(f);expect(commit(f.client,rejected,content(),'route')).toEqual({ok:false,reason:'limit'});expect(saved(f)).toEqual(before);
 const accepted=capture(f.client,f.target,'terminal','route',{start:0,end:link('t0').length})!;expect(commit(f.client,accepted,content(),'route').ok).toBe(true);
 const r=read(f.client);expect(r.ok&&r.context?.records.length).toBe(200);expect(r.ok&&r.context?.records.some(row=>row.contextId==='t0')).toBe(false);
});

// Raw unsupported recovery fields must survive refusal, never become null-filled JSON.
test('sparse arrays with extra enumerable fields in unrelated recovery metadata refuse without laundering',()=>{
 const f=fixture(),c=capture(f.client,f.target,'terminal','route')!;const opaque:any[]=[];opaque.length=1;Object.assign(opaque,{extra:'must survive'});
 local(f).mobileNewTaskDrafts={version:1,records:{},receipts:{other:opaque},claims:{},fileReleases:[]};
 const before=saved(f);expect(commit(f.client,c,content(),'route')).toEqual({ok:false,reason:'invalid-inventory'});expect(saved(f)).toEqual(before);
 expect(Object.keys(local(f).mobileNewTaskDrafts.receipts.other)).toEqual(['extra']);expect(0 in opaque).toBe(false);
});
