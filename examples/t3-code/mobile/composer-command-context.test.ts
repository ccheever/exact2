// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect,test } from 'bun:test';
import { MobileDraftClient,mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileComposerContextCapture as captureContext,mobileComposerContextCommit as commit,mobileComposerContextRead as read,
 mobileComposerContextObserve as observe,mobileComposerContextsHydrate as hydrate,mobileComposerContextsPersisted as persisted,
 mobileComposerContextForSend as send } from './composer-command-context';
import { obj,arr,type Obj } from './shared/domain';
import type { Files,Native } from './shared/protocol';
const capture=(client:MobileDraftClient)=>captureContext(client,{kind:'ordinary',key:client.draftKey,origin:client.origin,generation:client.generation});
const record=(id='one')=>({version:1,kind:'mention',contextId:id,label:id,path:`src/${id}.ts`});
const link=(id='one')=>`[${id}](t3-context://v1/mention/${id})`;
function fixture(document:Obj={version:1}) {
 const client=new MobileDraftClient(); const identity={origin:'https://context.test',environmentId:'e',threadId:'t',projectId:'p',generation:3};Object.assign(client,identity);
 let disk=JSON.stringify(document), uncertain=false;const writes:Obj[]=[],calls:Obj[]=[];
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(disk).buffer},async atomicWriteFile(_path,bytes){disk=new TextDecoder().decode(bytes);writes.push(obj(JSON.parse(disk)))}}};
 const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);return request.op==='request'&&uncertain
  ?{ok:false,generation:3,error:{kind:'transport',message:'reply lost',uncertain:true}}:{ok:true,generation:3,value:request.op==='status'?{phase:'disconnected'}:{}}}};
 return {client,storage,native,writes,calls,disk:()=>obj(JSON.parse(disk)),uncertain:()=>{uncertain=true},async load(){const h=mobileDraftRecoveryHandles(client,native,storage);await client.refresh(h.native,h.storage);Object.assign(client,identity)}};
}
function insert(f:ReturnType<typeof fixture>,id='one'){expect(commit(f.client,capture(f.client)!,link(id),record(id))).toBe(true)}
test('ordinary producer atomically commits text/records and fences stale or generation-replaced guards',()=>{
 const f=fixture(),g=capture(f.client)!;expect(commit(f.client,g,link(),record())).toBe(true);expect(f.client.draft).toBe(link());expect(read(f.client)).toMatchObject({ok:true,context:{records:[record()]}});
 expect(commit(f.client,g,'stale',record())).toBe(false);const current=capture(f.client)!;f.client.generation++;expect(commit(f.client,current,'late')).toBe(false);
 expect(f.client.draft).toBe(link());
});
test('observed deletion and undo restore records but a captured ABA guard remains stale',()=>{
 const f=fixture();insert(f);const old=capture(f.client)!;f.client.local.drafts['e:t']='';observe(f.client);expect(read(f.client)).toMatchObject({context:undefined});
 f.client.local.drafts['e:t']=link();observe(f.client);expect(read(f.client)).toMatchObject({context:{records:[record()]}});expect(commit(f.client,old,'stale')).toBe(false);
});
test('server replacement isolates context even when environment and draft IDs repeat',()=>{
 const f=fixture();insert(f);const old=capture(f.client)!;f.client.origin='https://replacement.test';expect(read(f.client)).toMatchObject({context:undefined});expect(commit(f.client,old,'late')).toBe(false);
 f.client.origin='https://context.test';expect(read(f.client)).toMatchObject({context:{records:[record()]}});
});
test('new-task and missing thread identities never enter the ordinary store',()=>{
 const f=fixture();f.client.threadId='';expect(capture(f.client)).toBeNull();expect(send(f.client,'e:new:p','x',{old:true})).toEqual({old:true});
});
test('actual persistence and loader roundtrip context and do not replay after cleared restart',async()=>{
 const f=fixture();insert(f);await f.client.persist(f.storage);const next=fixture(f.disk());await next.load();expect(read(next.client)).toMatchObject({context:{records:[record()]}});
 next.client.local.drafts['e:t']='';await next.client.persist(next.storage);const cleared=fixture(next.disk());await cleared.load();expect(read(cleared.client)).toMatchObject({context:undefined});
});
test('persist captures context beside the same text before an awaited disk write',async()=>{
 const f=fixture();insert(f);let release!:()=>void;const held=new Promise<void>(r=>release=r);let saved:Obj={};
 const waiting=f.client.persist({fs:{...f.storage.fs,async atomicWriteFile(_p,bytes){saved=obj(JSON.parse(new TextDecoder().decode(bytes)));await held}}});
 f.client.local.drafts['e:t']='';observe(f.client);release();await waiting;expect(obj(saved.drafts)['e:t']).toBe(link());
 const restarted=fixture(saved);await restarted.load();expect(read(restarted.client)).toMatchObject({context:{records:[record()]}});
});
test('malformed saved metadata is retained, blocks send and cannot be laundered by clearing text',()=>{
 const f=fixture();insert(f);const saved=obj(persisted(f.client)),entry=obj(Object.values(obj(saved.entries))[0]);entry.context={version:1,records:[{future:true}]};
 const fresh=fixture();hydrate(fresh.client,{mobileComposerContexts:saved});f.client=fresh.client;f.client.local.drafts['e:t']='';observe(f.client);expect(read(f.client).ok).toBe(false);expect(()=>send(f.client,'e:t','')).toThrow('unsupported');
 expect(obj(Object.values(obj(obj(persisted(f.client)).entries))[0]).context).toEqual(entry.context);
});
test('unrelated shared contexts pass unchanged without an ordinary context owner',()=>{
 const f=fixture(),shared={version:1,records:[{kind:'review-comment',contextId:'legacy',comment:'shared'}]};expect(send(f.client,'e:t','legacy',shared)).toBe(shared);
});
test('actual dispatch persists exactly the merged context sent over the wire',async()=>{
 const f=fixture();insert(f);await f.client.dispatch(f.native,f.storage,{type:'message.dispatch',commandId:'cmd',threadId:'t',text:link()},'Send');
 const wire=obj(f.calls.find(c=>c.op==='request')?.payload);expect(obj(wire.context).records).toEqual([record()]);expect(obj(obj(obj(f.writes[0]?.pending).e).payload)).toEqual(wire);
});
test('actual retry never recomposes an uncertain original command from newer records',async()=>{
 const f=fixture();insert(f);f.uncertain();await expect(f.client.dispatch(f.native,f.storage,{type:'message.dispatch',commandId:'cmd',threadId:'t',text:link()},'Send')).rejects.toThrow('reply lost');
 const wire=f.calls.find(c=>c.op==='request')?.payload;const restarted=fixture(f.disk());await restarted.load();
 expect(commit(restarted.client,capture(restarted.client)!,link(),{...record(),path:'changed.ts'})).toBe(true);
 await restarted.client.write(restarted.native,restarted.storage,{...restarted.client.local.pending.e!,uncertain:false});expect(restarted.calls.filter(c=>c.op==='request').at(-1)?.payload).toEqual(wire);
});
test('send rebinds local images only to attachments in its actual outgoing payload',()=>{
 const f=fixture(),image={version:1,kind:'image',contextId:'photo',label:'Photo',attachmentId:'local',name:'photo.png',mimeType:'image/png',sizeBytes:5};
 const text='[Photo](t3-context://v1/image/photo)';expect(commit(f.client,capture(f.client)!,text,image)).toBe(true);f.client.local.snapshotDrafts['e:t']=[{id:'local',uploadId:'uploaded'}];
 expect(arr(send(f.client,'e:t',text,undefined,[{id:'uploaded'}])?.records)[0]?.attachmentId).toBe('uploaded');expect(()=>send(f.client,'e:t',text,undefined,[])).toThrow('unavailable');
});
test('invalid producer never changes draft/context and hydration replaces stale guards',()=>{
 const f=fixture();insert(f);const g=capture(f.client)!,before=persisted(f.client);expect(commit(f.client,g,'x',{...record(),path:''})).toBe(false);expect(persisted(f.client)).toEqual(before);
 const fresh=fixture({version:1,drafts:{'e:t':link()}});fresh.client.local.drafts['e:t']=link();const old=capture(fresh.client)!;hydrate(fresh.client,{mobileComposerContexts:before});expect(commit(fresh.client,old,'late')).toBe(false);
});

test('delayed applied context survives in undo history after newer typing already removed its link',()=>{
 const f=fixture();f.client.local.drafts['e:t']='newer text';
 // Runtime has admitted the newest native snapshot before consuming retained terminal.
 expect(commit(f.client,capture(f.client)!,'newer text',record())).toBe(true);
 expect(f.client.draft).toBe('newer text');expect(read(f.client)).toMatchObject({context:undefined});
 expect(obj(persisted(f.client)).entries).toEqual({});
 f.client.local.drafts['e:t']=link();observe(f.client);expect(read(f.client)).toMatchObject({context:{records:[record()]}});
});
test('producer enforces the source 200-record cap without changing the current draft',()=>{
 const seed=fixture();insert(seed);const saved=obj(persisted(seed.client)),entry=obj(Object.values(obj(saved.entries))[0]);
 const records=Array.from({length:200},(_,i)=>record(`item${i}`)),text=records.map(r=>link(r.contextId)).join(' ');
 entry.context={version:1,records};entry.text=text;const f=fixture();f.client.local.drafts['e:t']=text;hydrate(f.client,{mobileComposerContexts:saved});
 expect(commit(f.client,capture(f.client)!,text+' '+link('extra'),record('extra'))).toBe(false);
 expect(f.client.draft).toBe(text);expect(read(f.client)).toMatchObject({context:{records}});
 expect(captureContext(f.client,{kind:'queued-edit',key:'e:t',origin:f.client.origin,generation:3})).toBeNull();
});
