// @ref llp/1109.008-mobile-voice.decision.md#draft-and-selection
// Actual document/context owners and copied voice controller; native speech/selection are mocked.
import { beforeEach, expect, test } from 'bun:test';
import { MobileDraftClient } from './mobile-draft-recovery';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { mobileComposerTarget } from './composer-target';
import { mobileVoiceAction, mobileVoiceSnapshot } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';
import { mobileEditorOwnerAdmit, type EditorRouteInput } from './composer-editor-owner';
import { mobileComposerEditorAccept } from './composer-editor-state';
import { mobileEditorDocument, mobileEditorDocumentKey, mobileEditorDocumentMembership as membership,
  mobileEditorDocumentWritten, mobileEditorDocumentsHydrate, mobileEditorPersistSnapshot, mobileEditorPersistDocument } from './composer-editor-persistence';
import { mobileComposerContextsHydrate, mobileComposerContextRead } from './composer-command-context';

beforeEach(resetVoiceInputGlobalsForTests);
let serial=0;
function fixture(value='Hello world') {
  const client=new MobileDraftClient();Object.assign(client,{origin:'https://voice-document.test',environmentId:`voice-${++serial}`,threadId:'one',projectId:'p',generation:1});
  client.local.drafts[client.draftKey]=value;
  const target=mobileComposerTarget(client),calls:Obj[]=[],writes:Obj[]=[];
  const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'thread-composer',environmentId:target.environmentId,threadId:'one',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
  let onNative:((request:Obj)=>Promise<void>|void)|undefined,onWrite:((saved:Obj)=>Promise<void>|void)|undefined,session='';
  const storage:Files={fs:{async mkdir(){},async readFile(){return new ArrayBuffer(0)},async atomicWriteFile(_path,bytes){const saved=obj(JSON.parse(new TextDecoder().decode(bytes)));writes.push(saved);await onWrite?.(saved)}}};
  const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);await onNative?.(request);
    if(request.action!=='status')session=String(request.session);
    const value=request.action==='status'?{available:true,locale:'en-US',event:0,session,elapsed:0,levels:[]}
      :request.action==='permission'?{granted:true,canAskAgain:true}:request.action==='prepare'?{locale:'en-US'}
      :request.action==='selection'?{start:6,end:11}:request.action==='stop'||request.action==='recorder-prepare'?{uri:'file:///voice-document.m4a'}
      :request.action==='transcribe'?{transcript:'spoken words'}:{};
    return {ok:true,generation:client.generation,value};}};
  const key=mobileEditorDocumentKey({origin:client.origin,environmentId:client.environmentId,draftKey:target.key});
  const enroll=(mount=false)=>{const owner=mobileEditorOwnerAdmit(client,target,route,'catalog')!;
    if(mount){const ready=mobileComposerEditorAccept(owner.state,{...owner.state.identity,mountId:'mount',eventCount:0,kind:'ready',value:owner.state.value,selection:owner.state.selection,focused:false,composing:false});
      if(!ready.accepted)throw new Error('Fixture ready rejected');owner.state=ready.state;
    }else mobileEditorOwnerAdmit(client,target,{...route,active:false},'catalog');
    return mobileEditorDocument(client,key)!;};
  const action=(op:string,start=6,end=11,bridge=native)=>mobileVoiceAction(op,start,end,'Voice draft',client.draftKey,bridge,storage,client);
  return {client,target,key,route,calls,writes,storage,native,enroll,action,document:()=>mobileEditorDocument(client,key),
    onNative:(fn:typeof onNative)=>{onNative=fn},onWrite:(fn:typeof onWrite)=>{onWrite=fn}};
}
function observedWrite(f:ReturnType<typeof fixture>,value:string) {
  const before=f.client.local.drafts[f.target.key]??'';f.client.local.drafts[f.target.key]=value;
  expect(mobileEditorDocumentWritten(f.client,f.target,before,value)).toBe(true);
}
const selectionCalls=(f:ReturnType<typeof fixture>)=>f.calls.filter(c=>c.action==='selection-commit');
function cold(f:ReturnType<typeof fixture>,saved:Obj) {
  const client=new MobileDraftClient();Object.assign(client,{origin:f.client.origin,environmentId:f.target.environmentId,threadId:'one',projectId:'p',generation:2});
  client.local.drafts=structuredClone(saved.drafts) as Record<string,string>;
  mobileEditorDocumentsHydrate(client,saved);mobileComposerContextsHydrate(client,saved);return client;
}

test('read-only membership distinguishes absent, off-focus, blocked, mismatched and stale connection without enrollment',()=>{
  const f=fixture();expect(membership(f.client,f.target)).toBe('unenrolled');expect(f.document()).toBeNull();
  const d=f.enroll();expect(membership(f.client,f.target)).toBe('enrolled');f.client.threadId='other';
  expect(membership(f.client,f.target)).toBe('enrolled');f.client.local.drafts[f.target.key]='unobserved';
  expect(membership(f.client,f.target)).toBe('unavailable');expect(d.blocked).toBeUndefined();
  f.client.local.drafts[f.target.key]=d.value;d.blocked=true;expect(membership(f.client,f.target)).toBe('unavailable');
  delete d.blocked;f.client.generation++;expect(membership(f.client,f.target)).toBe('unavailable');
});
test('invalid persisted ownership refuses before native status and never falls through to legacy',async()=>{
  const f=fixture();mobileEditorDocumentsHydrate(f.client,{mobileComposerEditor:{version:99,documents:{}}});
  expect(membership(f.client,f.target)).toBe('unavailable');expect((await f.action('start')).message).toContain('not available');
  expect(f.calls).toHaveLength(0);expect(f.client.draft).toBe('Hello world');expect(f.writes).toHaveLength(0);
});
test('legacy ordinary Voice remains unenrolled after its successful plain write',async()=>{
  const f=fixture();expect((await f.action('start')).data.phase).toBe('recording');expect((await f.action('stop')).data.phase).toBe('idle');
  expect(f.client.draft).toBe('Hello spoken words');expect(f.document()).toBeNull();expect(membership(f.client,f.target)).toBe('unenrolled');
  expect(f.writes.every(saved=>saved.mobileComposerEditor===undefined)).toBe(true);expect(selectionCalls(f)).toHaveLength(1);
});
test('enrolled unmounted Voice atomically commits text, source UTF16 caret, revision and durable context',async()=>{
  const link='[Issue](t3-context://v1/thread/ctx)',text=`😀 world ${link}`,f=fixture(text);f.enroll();
  const slot=JSON.stringify([f.client.origin,f.target.environmentId,f.target.key]);
  mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{[slot]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,
    revision:0,text,context:{version:1,records:[{version:1,kind:'thread',contextId:'ctx',environmentId:f.target.environmentId,threadId:'linked',title:'Issue',label:'Issue'}]}}}}});
  expect((await f.action('start',3,8)).data.phase).toBe('recording');expect((await f.action('stop')).message).toBe('');
  const next=`😀 spoken words ${link}`;
  expect(f.client.draft).toBe(next);expect(f.document()).toMatchObject({value:next,revision:1,selection:{start:15,end:15}});
  expect(mobileComposerContextRead(f.client)).toMatchObject({ok:true,context:{records:[{contextId:'ctx'}]}});
  expect(selectionCalls(f)[0]).toMatchObject({owner:f.target.editorOwner,text:next,start:15,end:15});
  const restored=cold(f,f.writes.at(-1)!);expect(restored.draft).toBe(next);
  expect(mobileEditorDocument(restored,f.key)).toMatchObject({value:next,revision:1,selection:{start:15,end:15}});
  expect(membership(restored,mobileComposerTarget(restored))).toBe('enrolled');
});
test('offscreen completion updates only the captured document using the finishing answer',async()=>{
  const f=fixture();f.enroll();let active=true;
  const starting:Native={...f.native,later:input=>{if(!active)throw new Error('Expired starting answer');return f.native.later(input)}};
  await f.action('start',6,11,starting);active=false;f.client.threadId='two';f.client.local.drafts[f.client.draftKey]='Untouched';
  expect((await f.action('stop')).message).toBe('');expect(f.client.draft).toBe('Untouched');
  expect(f.document()).toMatchObject({value:'Hello spoken words',revision:1,selection:{start:18,end:18}});
  expect(obj(f.writes.at(-1)?.drafts)[f.target.key]).toBe('Hello spoken words');
});
for(const phase of ['status','selection','prepare','transcribe'])test(`captured revision rejects ordinary ABA during ${phase}`,async()=>{
  const f=fixture();f.enroll();let once=false;
  f.onNative(request=>{if(request.action===phase&&!once){once=true;observedWrite(f,'Changed');observedWrite(f,'Hello world')}});
  const started=await f.action('start',-1,-1);const result=phase==='transcribe'?await f.action('stop'):started;
  expect(once).toBe(true);expect(result.message||result.data.error).not.toBe('');expect(f.client.draft).toBe('Hello world');
  expect(f.document()?.revision).toBe(2);expect(selectionCalls(f)).toHaveLength(0);expect(f.writes).toHaveLength(0);
});
test('offscreen ABA is rejected without a voice observer callback',async()=>{
  const f=fixture();f.enroll();await f.action('start');f.client.threadId='two';
  observedWrite(f,'Changed');observedWrite(f,'Hello world');expect((await f.action('stop')).data.error).toContain('draft changed');
  expect(f.client.local.drafts[f.target.key]).toBe('Hello world');expect(f.writes).toHaveLength(0);
});
for(const change of ['enrolled','blocked','incarnation','connection','missing','invalid'] as const)test(`recording membership cannot fall through after ${change}`,async()=>{
  const f=fixture();if(change!=='enrolled')f.enroll();await f.action('start');
  if(change==='enrolled')f.enroll();
  else if(change==='blocked')f.document()!.blocked=true;
  else if(change==='incarnation')f.document()!.incarnation='replacement';
  else if(change==='connection')f.client.generation++;
  else if(change==='missing')f.client.local={...f.client.local,drafts:{...f.client.local.drafts}};
  else mobileEditorDocumentsHydrate(f.client,{mobileComposerEditor:{version:99}});
  expect((await f.action('stop')).data.error).toContain('draft changed');expect(f.client.local.drafts[f.target.key]).toBe('Hello world');
  expect(selectionCalls(f)).toHaveLength(0);expect(f.writes).toHaveLength(0);
});
test('mounted rich owner refuses initial recording and a late mount before transcript commit',async()=>{
  const f=fixture();f.enroll(true);expect((await f.action('start')).message).toContain('not available');expect(f.calls).toHaveLength(0);
  mobileEditorOwnerAdmit(f.client,f.target,{...f.route,active:false},'catalog');await f.action('start');
  f.onNative(request=>{if(request.action==='transcribe')f.enroll(true)});
  expect((await f.action('stop')).data.error).toContain('draft changed');expect(f.client.draft).toBe('Hello world');expect(selectionCalls(f)).toHaveLength(0);
});
test('unavailable context and exhausted context revision refuse before named text mutation',async()=>{
  for(const exhausted of [false,true]){
    resetVoiceInputGlobalsForTests();const f=fixture();f.enroll();const slot=JSON.stringify([f.client.origin,f.target.environmentId,f.target.key]);
    mobileComposerContextsHydrate(f.client,{mobileComposerContexts:exhausted?{version:1,entries:{[slot]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,
      revision:Number.MAX_SAFE_INTEGER,text:'Hello world'}}}:{version:99}});
    await f.action('start');expect((await f.action('stop')).data.phase).toBe('error');expect(f.client.draft).toBe('Hello world');
    expect(f.document()?.revision).toBe(0);expect(selectionCalls(f)).toHaveLength(0);expect(f.writes).toHaveLength(0);
  }
});
test('new retry records a fresh ordinary revision and does not reuse the rejected capture',async()=>{
  const f=fixture();f.enroll();await f.action('start');observedWrite(f,'Changed');observedWrite(f,'Hello world');
  expect((await f.action('stop')).data.phase).toBe('error');expect((await f.action('retry')).data.phase).toBe('recording');
  expect((await f.action('stop')).data.phase).toBe('idle');expect(f.document()).toMatchObject({value:'Hello spoken words',revision:3});
});
test('accepted write survives persistence refusal and repeated Stop cannot insert twice',async()=>{
  const f=fixture();f.enroll();await f.action('start');f.onWrite(()=>{throw new Error('Disk refused')});
  expect((await f.action('stop')).message).toContain('Disk refused');expect(f.document()).toMatchObject({value:'Hello spoken words',revision:1});
  f.onWrite(undefined);await f.client.persist(f.storage);await f.action('stop');
  expect(f.document()?.revision).toBe(1);expect(selectionCalls(f)).toHaveLength(1);expect(membership(f.client,f.target)).toBe('enrolled');
});
test('abandoned persistence propagates letGo identity after accepted document mutation',async()=>{
  const f=fixture();f.enroll();await f.action('start');const abandoned=Object.assign(new Error('Answer left'),{name:'FetchError',kind:'Aborted'});
  f.onWrite(()=>{throw abandoned});let caught:unknown;try{await f.action('stop')}catch(error){caught=error}
  expect(caught).toBe(abandoned);expect(f.document()).toMatchObject({value:'Hello spoken words',revision:1});
});
test('late caret reply cannot retain a token after durable ABA without voice observations',async()=>{
  const f=fixture();f.enroll();await f.action('start');f.onNative(request=>{if(request.action==='selection-commit'){observedWrite(f,'Other');observedWrite(f,'Hello spoken words')}});
  let caught:unknown;try{await f.action('stop')}catch(error){caught=error}
  expect(caught||mobileVoiceSnapshot(f.target.editorOwner,f.client).error).toBeTruthy();
  expect(mobileVoiceSnapshot(f.target.editorOwner,f.client).selectionOwner).toBe('');expect(f.document()?.revision).toBe(3);
});
test('read-only membership does not stamp a blocked flag while serialization still detects unsupported writes',()=>{
  const f=fixture(),document=f.enroll();f.client.local.drafts[f.target.key]='Unsupported';
  expect(membership(f.client,f.target)).toBe('unavailable');expect(document.blocked).toBeUndefined();
  const saved=mobileEditorPersistDocument(mobileEditorPersistSnapshot(f.client),{drafts:f.client.local.drafts,pending:{}});
  expect(obj(obj(obj(saved.mobileComposerEditor).documents)[f.key]).blocked).toBe(true);
});

for(const mutation of ['aba','enrollment'] as const)test(`held status reply refuses ${mutation} made while its answer waits`,async()=>{
  const f=fixture();if(mutation==='aba')f.enroll();
  let release!:()=>void,entered!:()=>void;
  const gate=new Promise<void>(resolve=>{release=resolve}),seen=new Promise<void>(resolve=>{entered=resolve});
  f.onNative(async request=>{if(request.action==='status'){entered();await gate}});
  const starting=f.action('start');await seen;
  if(mutation==='aba'){observedWrite(f,'Intervening');observedWrite(f,'Hello world')}else f.enroll();
  release();expect((await starting).message).toContain('draft changed');expect(f.client.draft).toBe('Hello world');
  expect(f.calls.some(call=>call.action==='record')).toBe(false);expect(f.writes).toHaveLength(0);
});
test('held caret reply releases its stale token after a later accepted document edit',async()=>{
  const f=fixture();f.enroll();await f.action('start');let release!:()=>void,entered!:()=>void;
  const gate=new Promise<void>(resolve=>{release=resolve}),seen=new Promise<void>(resolve=>{entered=resolve});
  f.onNative(async request=>{if(request.action==='selection-commit'){entered();await gate}});
  const stopping=f.action('stop');await seen;expect(f.document()?.value).toBe('Hello spoken words');
  observedWrite(f,'Later accepted edit');release();let caught:unknown;
  try{await stopping}catch(error){caught=error}
  expect(caught).toBeTruthy();expect(f.client.draft).toBe('Later accepted edit');
  expect(f.document()?.revision).toBe(2);expect(mobileVoiceSnapshot(f.target.editorOwner,f.client).selectionOwner).toBe('');
});
