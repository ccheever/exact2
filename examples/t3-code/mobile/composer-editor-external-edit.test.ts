// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,spyOn,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {obj,type Obj} from './shared/domain';
import type {Files} from './shared/protocol';
import {mobileComposerTarget} from './composer-target';
import * as durable from './composer-editor-persistence';
import {mobileEditorOwnerAdmit as admit,mobileEditorOwner as owner,mobileEditorOwnerRevision as ownerRevision,
  mobileEditorCaptureDocumentIntent as capture,mobileEditorCommitDocumentContextIntent as commit,type EditorRouteInput} from './composer-editor-owner';
import {mobileComposerContextCaptureTarget as contextCapture,mobileComposerContextCommitBatch as batch,
  mobileComposerContextRead as read,mobileComposerContextsPersisted as contexts,mobileComposerContextsHydrate as hydrate} from './composer-command-context';
const record=(contextId='one'):Obj=>({version:1,kind:'mention',contextId,label:contextId,path:`${contextId}.ts`});
const link=(id='one',kind='mention')=>`[${id}](t3-context://v1/${kind}/${id})`;
const next=(value:string,start=value.length,end=start)=>({value,selection:{start,end}});
let sequence=0;
function fixture(text='initial') {
  const client=new MobileDraftClient();Object.assign(client,{origin:'https://external.test',environmentId:`edit-${++sequence}`,threadId:'a',generation:4});
  client.local.drafts[client.draftKey]=text;const target=mobileComposerTarget(client),document=durable.mobileEditorDocumentEnroll(client,target)!;
  const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:target.environmentId,threadId:'a',readOnly:false,voiceBusy:false,
    focusIntent:{serial:'',attempt:0,operation:'none'}};
  return {client,target,document,route};
}
type Fixture=ReturnType<typeof fixture>;
function write(f:Fixture,text:string,records:Obj[]=[]) {return commit(f.client,capture(f.client,f.target,'review')!,next(text),records)}
function snapshot(f:Fixture) {return {drafts:structuredClone(f.client.local.drafts),document:structuredClone(f.document),context:contexts(f.client),
  revision:f.client.revision,ownerRevision:ownerRevision(f.client)}};
function seed(f:Fixture,records:Obj[]) {expect(batch(f.client,contextCapture(f.client,f.target)!,records)).toBe(true)}

test('public unmounted transaction publishes exact named text, caret and batch before persistence',async()=>{
  const f=fixture(),old=admit(f.client,f.target,f.route,'catalog')!,intent=capture(f.client,f.target,'review')!;
  f.client.threadId='other';f.client.local.drafts[f.client.draftKey]='unrelated';const revision=f.client.revision,owned=ownerRevision(f.client);
  const value=link('a')+' '+link('b'),result=commit(f.client,intent,next(value,3),[record('a'),record('b')]);
  expect(result).toEqual({ok:true,documentRevision:1,contextRevision:1});expect(f.client.revision).toBe(revision+1);expect(ownerRevision(f.client)).toBe(owned+1);
  expect(f.client.draft).toBe('unrelated');expect(f.document).toMatchObject({value,revision:1,selection:{start:3,end:3}});
  expect(old.document).toMatchObject({value,revision:1,selection:{start:3,end:3}});expect(owner(f.client)).toBeNull();
  expect(read(f.client,f.target.key)).toMatchObject({context:{records:[record('a'),record('b')]}});
  let saved:Obj={};const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_path,bytes){saved=obj(JSON.parse(new TextDecoder().decode(bytes)))}}};
  await f.client.persist(storage);expect(obj(saved.drafts)[f.target.key]).toBe(value);
  const row=obj(Object.values(obj(obj(saved.mobileComposerEditor).documents))[0]);expect(row).toMatchObject({revision:1,selection:{start:3,end:3}});expect(row.value).toBeUndefined();
  expect(obj(Object.values(obj(obj(saved.mobileComposerContexts).entries))[0]).text).toBe(value);
  f.client.threadId='a';const mounted=admit(f.client,f.target,{...f.route,routeVisit:'return'},'catalog')!;expect(mounted.state.selection).toEqual({start:3,end:3});
  expect(commit(f.client,intent,next('duplicate'),[]).ok).toBe(false);
});
test('same-text explicit context and selection edit advances each document/context once',()=>{
  const f=fixture(link()),intent=capture(f.client,f.target,'review')!,revision=f.client.revision;
  expect(commit(f.client,intent,next(link(),1,2),[record()])).toEqual({ok:true,documentRevision:1,contextRevision:1});
  expect(f.document.selection).toEqual({start:1,end:2});expect(f.client.revision).toBe(revision+1);
  expect(commit(f.client,intent,next(link(),0),[record()]).ok).toBe(false);
});
test('future live capacity refusal leaves text, selection and history recency unchanged',()=>{
  const live=Array.from({length:200},(_,i)=>record(`live${i}`)),text=live.map(r=>link(String(r.contextId))).join(' '),f=fixture(text);seed(f,live);
  expect(write(f,'').ok).toBe(true);const before=snapshot(f);
  expect(write(f,text+' '+link('rejected'),[record('rejected')])).toEqual({ok:false,reason:'limit'});expect(snapshot(f)).toEqual(before);
  expect(write(f,link('rejected')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:undefined});
  expect(write(f,text).ok).toBe(true);expect(read(f.client)).toMatchObject({context:{records:live}});
});
test('new annotation merges an undo-only screenshot before the ordered dependency pass',()=>{
  const image:Obj={version:1,kind:'image',contextId:'photo',label:'Photo',attachmentId:'asset',name:'a.png',mimeType:'image/png',sizeBytes:1};
  const annotation:Obj={version:1,kind:'preview-annotation',contextId:'note',label:'Note',annotationId:'n',pageUrl:'',pageTitle:null,
    comment:'',targetSummary:'',styleChanges:[],elements:[],screenshotContextId:'photo'};
  const f=fixture(link('photo','image'));seed(f,[image]);expect(write(f,'').ok).toBe(true);
  const images=structuredClone(f.client.local.snapshotDrafts);
  expect(write(f,link('note','preview-annotation'),[annotation]).ok).toBe(true);
  expect(read(f.client)).toMatchObject({context:{records:[image,annotation]}});expect(f.client.local.snapshotDrafts).toEqual(images);
});
test('accepted unreferenced batch seeds every record for Undo while latest text stays authoritative',()=>{
  const f=fixture('latest'),items=[record('a'),record('b')];expect(write(f,'latest',items).ok).toBe(true);
  items[0]!.path='mutated';expect(f.client.draft).toBe('latest');expect(read(f.client)).toMatchObject({context:undefined});
  expect(write(f,link('a')+' '+link('b')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:{records:[record('a'),record('b')]}});
});
test('malformed unused saved records refuse without laundering raw metadata',()=>{
  const f=fixture('before'),raw={version:1,entries:{[JSON.stringify([f.client.origin,f.target.environmentId,f.target.key])]:{
    origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:0,text:'before',context:{version:1,records:[{...record(),path:''}]}}}};
  hydrate(f.client,{mobileComposerContexts:raw});const before=snapshot(f);
  expect(write(f,'after',[]).ok).toBe(false);expect(snapshot(f)).toEqual(before);expect(contexts(f.client)).toEqual(raw);
});
test('invalid later batch record, duplicates, oversized and cyclic payloads never partially seed history',()=>{
  const f=fixture('before'),before=snapshot(f),cyclic:Obj=record('cycle');cyclic.extra=cyclic;
  for(const records of [[record('a'),{...record('bad'),path:''}],[record('a'),record('a')],[cyclic],
    Array.from({length:201},(_,i)=>record(String(i))),[{...record('large'),extra:'x'.repeat(16_000_000)}]]) {
    expect(write(f,link('a'),records).ok).toBe(false);expect(snapshot(f)).toEqual(before);
  }
  expect(write(f,link('a')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:{version:1,records:[]}});
});
test('actual document callback refusal discards a prepared history fork without changing eviction order',()=>{
  const f=fixture(link('a')+' '+link('b'));seed(f,[record('a'),record('b')]);expect(write(f,'').ok).toBe(true);
  const before=snapshot(f),refuse=spyOn(durable,'mobileEditorDocumentCommit').mockReturnValue(false);
  try {expect(write(f,link('a'),[record('a')]).ok).toBe(false);expect(refuse).toHaveBeenCalledTimes(1);expect(snapshot(f)).toEqual(before)}
  finally {refuse.mockRestore()}
  expect(write(f,'',Array.from({length:199},(_,i)=>record(`new${i}`))).ok).toBe(true);
  expect(write(f,link('b')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:{records:[record('b')]}});
  expect(write(f,link('a')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:undefined});
});
test('mounted same named document refuses even with a different volatile incarnation',()=>{
  const f=fixture(),intent=capture(f.client,f.target,'review')!,active=admit(f.client,f.target,f.route,'catalog')!;
  active.state.mountId='native-mount';active.document.incarnation='different';const before=snapshot(f);
  expect(commit(f.client,intent,next('after'),[]).ok).toBe(false);expect(snapshot(f)).toEqual(before);expect(owner(f.client)).toBe(active);
});
test('unrelated mounted document does not block an off-focus exact named transaction',()=>{
  const f=fixture(),intent=capture(f.client,f.target,'terminal')!;f.client.threadId='b';f.client.local.drafts[f.client.draftKey]='other';
  const target=mobileComposerTarget(f.client),active=admit(f.client,target,{...f.route,threadId:'b'},'catalog')!;active.state.mountId='b-mount';
  expect(commit(f.client,intent,next('after',2),[]).ok).toBe(true);expect(owner(f.client)).toBe(active);expect(active.state.value).toBe('other');expect(f.client.draft).toBe('other');
});
test('document ABA, unobserved text, connection and canonical-origin replacement refuse captured intents',()=>{
  const f=fixture('A'),intent=capture(f.client,f.target,'terminal')!;expect(write(f,'B').ok).toBe(true);expect(write(f,'A').ok).toBe(true);
  expect(commit(f.client,intent,next('stale'),[]).ok).toBe(false);
  const fresh=capture(f.client,f.target,'terminal')!;
  for(const change of ['text','generation','origin','incarnation'] as const) {
    const before=snapshot(f),oldOrigin=f.client.origin;
    if(change==='text')f.client.local.drafts[f.target.key]='unobserved';
    if(change==='generation')f.client.generation++;
    if(change==='origin')f.client.origin='https://other.test';
    if(change==='incarnation')f.document.incarnation='replacement';
    const current=snapshot(f);expect(commit(f.client,fresh,next('refused'),[]).ok).toBe(false);expect(snapshot(f)).toEqual(current);
    f.client.local.drafts=before.drafts;f.client.origin=oldOrigin;f.client.generation=f.target.generation;Object.assign(f.document,before.document);
  }
});
test('invalid selection and exhausted document/context/client revisions refuse before mutation',()=>{
  const f=fixture('before'),intent=capture(f.client,f.target,'review')!,before=snapshot(f);
  for(const value of [next('after',-1),next('after',2,1),next('after',0,6),next('after',0.5),next('x'.repeat(1_000_001))]) {
    expect(commit(f.client,intent,value,[]).ok).toBe(false);expect(snapshot(f)).toEqual(before);
  }
  f.document.revision=Number.MAX_SAFE_INTEGER;const exhausted=capture(f.client,f.target,'review')!,max=snapshot(f);
  expect(commit(f.client,exhausted,next('after'),[]).ok).toBe(false);expect(snapshot(f)).toEqual(max);
  f.document.revision=0;f.client.revision=Number.MAX_SAFE_INTEGER;expect(commit(f.client,intent,next('after'),[]).ok).toBe(false);
  f.client.revision=before.revision;
  hydrate(f.client,{mobileComposerContexts:{version:1,entries:{[JSON.stringify([f.client.origin,f.target.environmentId,f.target.key])]:{
    origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:Number.MAX_SAFE_INTEGER,text:'before'}}}});
  const contextMax=snapshot(f);expect(commit(f.client,intent,next('after'),[]).ok).toBe(false);expect(snapshot(f)).toEqual(contextMax);
});
test('valid raw recovery overflow prunes before live capacity, while exact record byte cap succeeds',()=>{
  const f=fixture('before'),records=Array.from({length:201},(_,i)=>record(`old${i}`));
  hydrate(f.client,{mobileComposerContexts:{version:1,entries:{[JSON.stringify([f.client.origin,f.target.environmentId,f.target.key])]:{
    origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:0,text:'before',context:{version:1,records}}}}});
  expect(write(f,link('old0')).ok).toBe(true);expect(read(f.client)).toMatchObject({context:{records:[records[0]]}});
  const large={...record('large'),extra:''};large.extra='x'.repeat(16_000_000-JSON.stringify([large]).length);
  expect(write(f,link('large'),[large]).ok).toBe(true);expect(read(f.client).ok).toBe(true);
});
test('letGo during actual one-snapshot persistence preserves committed state and retry does not repeat insertion',async()=>{
  const f=fixture(),intent=capture(f.client,f.target,'review')!,value=link();expect(commit(f.client,intent,next(value,2),[record()]).ok).toBe(true);
  const sentinel={name:'FetchError',kind:'Aborted'},writes:Obj[]=[];let fail=true;
  const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_path,bytes){writes.push(obj(JSON.parse(new TextDecoder().decode(bytes))));if(fail)throw sentinel}}};
  await expect(f.client.persist(storage)).rejects.toBe(sentinel);expect(f.document.revision).toBe(1);expect(f.client.draft).toBe(value);
  expect(commit(f.client,intent,next(value+value),[record()]).ok).toBe(false);fail=false;await f.client.persist(storage);
  expect(writes).toHaveLength(2);expect(writes[1]).toEqual(writes[0]);expect(f.document.selection).toEqual({start:2,end:2});
});
