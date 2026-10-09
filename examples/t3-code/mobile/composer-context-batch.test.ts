// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { mobileComposerTarget } from './composer-target';
import { mobileCreateContextHistory } from './mobile-new-task-context';
import { mobileComposerContextCaptureTarget as capture, mobileComposerContextObserveTarget as observe,
  mobileComposerContextPrepareBatch as prepare, mobileComposerContextCommitBatch as commit,
  mobileComposerContextRead as read, mobileComposerContextsPersisted as persisted,
  mobileComposerContextsHydrate as hydrate } from './composer-command-context';
const record=(contextId='one'):Obj=>({version:1,kind:'mention',contextId,label:contextId,path:`${contextId}.ts`});
const link=(id='one',kind='mention')=>`[${id}](t3-context://v1/${kind}/${id})`;
function fixture(text='') {
  const client=new T3Client(); Object.assign(client,{origin:'https://batch.test',environmentId:'e',threadId:'a',generation:7});
  client.local.drafts['e:a']=text;
  return {client,target:mobileComposerTarget(client)};
}
function edit(f:ReturnType<typeof fixture>,text:string) {
  const before=capture(f.client,f.target)!; expect(before).not.toBeNull();
  f.client.local.drafts['e:a']=text; expect(observe(f.client,before,text)).toBe(true);
}
test('history snapshot is detached, ordered and does not change callable restoration or recency',()=>{
  const history=mobileCreateContextHistory(); history('',{version:1,records:[record('a'),record('b')]});
  const snapshot=history.snapshot(); expect(snapshot.map(r=>r.contextId)).toEqual(['a','b']);
  snapshot[0]!.path='changed';snapshot.reverse();
  expect(history.snapshot()).toEqual([record('a'),record('b')]);
  const nested=mobileCreateContextHistory(), value={...record('nested'),extra:{rows:[{value:'original'}]}};
  nested('',{version:1,records:[value]});obj((obj(nested.snapshot()[0]!.extra).rows as Obj[])[0]).value='mutated';
  expect(nested.snapshot()).toEqual([value]);
  history('',{version:1,records:Array.from({length:199},(_,i)=>record(`new${i}`))});
  expect(history.snapshot()[0]).toEqual(record('b'));
  expect(history(link('b'))?.records).toEqual([record('b')]);
});
test('batch preflight detaches records and metadata commit leaves latest text and focused thread untouched',()=>{
  const f=fixture(link('a')+' '+link('b')), input=[record('a'),record('b')];
  const ready=prepare(f.client,f.target,input);expect(ready.ok).toBe(true);
  if(!ready.ok) throw Error('preflight refused');
  input[0]!.path='mutated';expect(ready.records[0]!.path).toBe('a.ts');
  f.client.threadId='other';f.client.local.drafts['e:other']='focused';
  const guard=capture(f.client,f.target)!, revision=f.client.revision;
  expect(commit(f.client,guard,ready.records)).toBe(true);
  expect(f.client.local.drafts['e:a']).toBe(link('a')+' '+link('b'));expect(f.client.draft).toBe('focused');
  expect(read(f.client,'e:a')).toMatchObject({context:{records:[record('a'),record('b')]}});
  expect(f.client.revision).toBe(revision+1);expect(commit(f.client,guard,ready.records)).toBe(false);
});
test('late applied batch seeds every imported record for undo without restoring terminal text',()=>{
  const f=fixture('captured'), ready=prepare(f.client,f.target,[record('a'),record('b')]);expect(ready.ok).toBe(true);
  edit(f,'later text');expect(commit(f.client,capture(f.client,f.target)!,[record('a'),record('b')])).toBe(true);
  expect(f.client.draft).toBe('later text');expect(read(f.client)).toMatchObject({context:undefined});
  expect(obj(persisted(f.client)).entries).toEqual({});
  edit(f,link('a')+' '+link('b'));expect(read(f.client)).toMatchObject({context:{records:[record('a'),record('b')]}});
});
test('new annotation retrieves a screenshot held only by prior undo history',()=>{
  const image={version:1,kind:'image',contextId:'photo',label:'Photo',attachmentId:'asset',name:'a.png',mimeType:'image/png',sizeBytes:1};
  const annotation={version:1,kind:'preview-annotation',contextId:'note',label:'Note',annotationId:'n',pageUrl:'',pageTitle:null,
    comment:'',targetSummary:'',styleChanges:[],elements:[],screenshotContextId:'photo'};
  const f=fixture(link('photo','image'));expect(commit(f.client,capture(f.client,f.target)!,[image])).toBe(true);
  const saved=obj(persisted(f.client)), row=obj(Object.values(obj(saved.entries))[0]);row.text=link('note','preview-annotation');
  const loaded=fixture(row.text as string);hydrate(loaded.client,{mobileComposerContexts:saved});
  const ready=prepare(loaded.client,loaded.target,[annotation]);expect(ready.ok).toBe(true);
  if(ready.ok) expect(ready.context?.records).toEqual([image]);
  expect(commit(loaded.client,capture(loaded.client,loaded.target)!,[annotation])).toBe(true);
  expect(read(loaded.client)).toMatchObject({context:{records:[image,annotation]}});
  edit(f,'');edit(f,link('note','preview-annotation'));expect(read(f.client)).toMatchObject({context:undefined});
  expect(commit(f.client,capture(f.client,f.target)!,[annotation])).toBe(true);
  expect(read(f.client)).toMatchObject({context:{records:[image,annotation]}});
});
test('full malformed batch and duplicates refuse before pruning and leave history unchanged',()=>{
  const f=fixture('latest'), guard=capture(f.client,f.target)!, before=f.client.revision;
  for(const records of [[record('bad'),{...record('bad'),path:'different'}],[{...record('bad'),path:''}],
    Array.from({length:201},(_,i)=>record(`bad${i}`))]) {
    expect(prepare(f.client,f.target,records).ok).toBe(false);expect(commit(f.client,guard,records)).toBe(false);
  }
  expect(f.client.revision).toBe(before);expect(f.client.draft).toBe('latest');
  edit(f,link('bad'));const after=read(f.client);expect(after.ok).toBe(true);
  if(after.ok) expect(after.context?.records??[]).toEqual([]);
});
test('capacity refusal does not seed failed records or change accepted metadata',()=>{
  const records=Array.from({length:200},(_,i)=>record(`live${i}`));
  const text=records.map(r=>link(String(r.contextId))).join(' '), f=fixture(text);
  expect(commit(f.client,capture(f.client,f.target)!,records)).toBe(true);edit(f,text+' '+link('rejected'));
  const before=persisted(f.client), revision=f.client.revision;
  expect(commit(f.client,capture(f.client,f.target)!,[record('rejected')])).toBe(false);
  expect(persisted(f.client)).toEqual(before);expect(f.client.revision).toBe(revision);
  edit(f,link('rejected'));expect(read(f.client)).toMatchObject({context:undefined});
});
test('late unreferenced batch cannot evict the current200live records from undo',()=>{
  const live=Array.from({length:200},(_,i)=>record(`live${i}`));
  const text=live.map(r=>link(String(r.contextId))).join(' '), f=fixture(text);
  expect(commit(f.client,capture(f.client,f.target)!,live)).toBe(true);
  expect(commit(f.client,capture(f.client,f.target)!,Array.from({length:200},(_,i)=>record(`late${i}`)))).toBe(true);
  edit(f,'');edit(f,text);expect(read(f.client)).toMatchObject({context:{records:live}});
});
test('malformed raw saved context cannot be laundered by a linkless batch',()=>{
  const f=fixture(link());expect(commit(f.client,capture(f.client,f.target)!,[record()])).toBe(true);
  const saved=obj(persisted(f.client)), row=obj(Object.values(obj(saved.entries))[0]);
  row.context={version:1,records:[{...record(),path:''}]};row.text='';
  const next=fixture();hydrate(next.client,{mobileComposerContexts:saved});
  expect(prepare(next.client,next.target,[]).ok).toBe(false);expect(capture(next.client,next.target)).toBeNull();
  expect(persisted(next.client)).toEqual(saved);
});
test('named guard refuses unobserved edits, ABA, hydration and connection replacement',()=>{
  const f=fixture('a'), guard=capture(f.client,f.target)!;
  f.client.local.drafts['e:a']='unobserved';expect(commit(f.client,guard,[record()])).toBe(false);
  f.client.local.drafts['e:a']='a';edit(f,'b');edit(f,'a');expect(commit(f.client,guard,[record()])).toBe(false);
  const fresh=capture(f.client,f.target)!;f.client.generation++;expect(commit(f.client,fresh,[])).toBe(false);f.client.generation--;
  f.client.origin='https://other.test';expect(commit(f.client,fresh,[])).toBe(false);f.client.origin='https://batch.test';
  const next=fixture('a'), old=capture(next.client,next.target)!;
  const seed=fixture(link());expect(commit(seed.client,capture(seed.client,seed.target)!,[record()])).toBe(true);
  hydrate(next.client,{mobileComposerContexts:persisted(seed.client)});expect(commit(next.client,old,[])).toBe(false);
});
test('incoming batch limits and cyclic payloads refuse before a projection can remove them',()=>{
  const f=fixture(), huge={...record(),extra:'x'.repeat(16_000_000)};
  const guard=capture(f.client,f.target)!;expect(prepare(f.client,f.target,[huge]).ok).toBe(false);expect(commit(f.client,guard,[huge])).toBe(false);
  const circular:Obj=record();circular.extra=circular;expect(prepare(f.client,f.target,[circular]).ok).toBe(false);
  expect(commit(f.client,guard,[circular])).toBe(false);expect(f.client.draft).toBe('');
});
test('valid recovery overflow can prune to live capacity but exhausted revisions never publish',()=>{
  const seed=fixture(link());expect(commit(seed.client,capture(seed.client,seed.target)!,[record()])).toBe(true);
  const saved=obj(persisted(seed.client)), row=obj(Object.values(obj(saved.entries))[0]);
  const records=Array.from({length:201},(_,i)=>record(`recovery${i}`));row.context={version:1,records};row.text=link('recovery0');
  const f=fixture(row.text as string);hydrate(f.client,{mobileComposerContexts:saved});
  const ready=prepare(f.client,f.target,[]);expect(ready.ok).toBe(true);if(ready.ok) expect(ready.context?.records.length).toBe(201);
  expect(commit(f.client,capture(f.client,f.target)!,[])).toBe(true);expect(read(f.client)).toMatchObject({context:{records:[records[0]]}});
  row.revision=Number.MAX_SAFE_INTEGER;const exhausted=fixture(row.text as string);hydrate(exhausted.client,{mobileComposerContexts:saved});
  const guard=capture(exhausted.client,exhausted.target)!;expect(guard).not.toBeNull();
  expect(prepare(exhausted.client,exhausted.target,[]).ok).toBe(false);expect(commit(exhausted.client,guard,[])).toBe(false);
  expect(persisted(exhausted.client)).toEqual(saved);
});
test('failed publication preserves undo eviction order',()=>{
  const f=fixture(link('a')+' '+link('b'));expect(commit(f.client,capture(f.client,f.target)!,[record('a'),record('b')])).toBe(true);
  edit(f,'');expect(commit(f.client,capture(f.client,f.target)!,[record('a'),{...record('bad'),path:''}])).toBe(false);
  expect(commit(f.client,capture(f.client,f.target)!,Array.from({length:199},(_,i)=>record(`next${i}`)))).toBe(true);
  edit(f,link('b'));expect(read(f.client)).toMatchObject({context:{records:[record('b')]}});
  edit(f,link('a'));expect(read(f.client)).toMatchObject({context:undefined});
});
test('live context limit counts record bytes without charging its envelope twice',()=>{
  const base={...record(),extra:''}, overhead=JSON.stringify([base]).length;
  base.extra='x'.repeat(16_000_000-overhead);
  const f=fixture(link());expect(prepare(f.client,f.target,[base]).ok).toBe(true);
  expect(commit(f.client,capture(f.client,f.target)!,[base])).toBe(true);
  expect(read(f.client).ok).toBe(true);
});
