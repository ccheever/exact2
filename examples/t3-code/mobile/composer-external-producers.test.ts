// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {obj,type Obj} from './shared/domain';
import type {Files,Native} from './shared/protocol';
import {mobileComposerTarget} from './composer-target';
import {mobileEditorDocumentEnroll,mobileEditorDocumentWritten,mobileEditorDocumentsHydrate} from './composer-editor-persistence';
import {mobileComposerContextRead,mobileComposerContextsPersisted} from './composer-command-context';
import {mobileTerminalAttachOutput} from './terminal-mobile';
import {mobileReviewRead,mobileReviewAction,mobileReviewSnapshot} from './review-data';
import {mobileExternalContextCapture,mobileExternalContextCurrent} from './composer-external-context';
import {terminalDraftRecords} from './shared/terminal-integrations';
import {fleet} from './shared/settings-b-fleet';
const patch='diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\n';
let serial=0;
function fixture(text='hello world',enroll=true){
  const client=new MobileDraftClient();Object.assign(client,{origin:'https://external-producer.test',environmentId:`producer-${++serial}`,threadId:'a',projectId:'p',generation:3,
    connection:'connected',configLive:true,shellLive:true,threadLive:true});
  client.shell.projects=[{id:'p',title:'Repo',workspaceRoot:'/repo'}];client.shell.threads=[{id:'a',projectId:'p',title:'Thread'}];
  client.thread={sequence:1,hasMore:false,historyCursor:null,latestLocalTurnOrdinal:null,projection:{thread:{id:'a'},checkpoints:[],runs:[]}};
  client.local.drafts[client.draftKey]=text;const target=mobileComposerTarget(client),document=enroll?mobileEditorDocumentEnroll(client,target)!:null;
  fleet.saved.push({environmentId:client.environmentId,origin:client.origin});
  const calls:Obj[]=[],writes:Obj[]=[];let nativeHook:((request:Obj)=>Promise<void>|void)|undefined,writeHook:((saved:Obj)=>Promise<void>|void)|undefined;
  const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);await nativeHook?.(request);
    const value=request.op==='ids'?['4f138455-8a22-42fb-85ad-9a7ae2d22766']:request.op==='http'?{authenticated:true,permissions:['filesystem:read']}
      :request.method==='review.getDiffPreview'?{cwd:'/repo',sources:[{kind:'branch-range',title:'Changes',baseRef:'main',headRef:'feature',diffHash:'one',diff:patch}]}:{};
    return {ok:true,generation:client.generation,value};}};
  const storage:Files={fs:{async mkdir(){},async readFile(){return new ArrayBuffer(0)},async atomicWriteFile(_path,bytes){const saved=obj(JSON.parse(new TextDecoder().decode(bytes)));writes.push(saved);await writeHook?.(saved)}}};
  const key=JSON.stringify([client.environmentId,'a','term-1']);
  const attach=()=>mobileTerminalAttachOutput(key,'first\nsecond',1,1,1000,native,storage,client);
  const review=(op:string,id='',value='',n=0)=>mobileReviewAction(mobileReviewSnapshot(false,client).owner,op,id,value,n,native,storage,false,client);
  const pick=async()=>{await mobileReviewRead(native,'',false,false,client);await review('line','a.ts','',1)};
  return {client,target,document,calls,writes,native,storage,attach,review,pick,onNative:(fn:typeof nativeHook)=>nativeHook=fn,onWrite:(fn:typeof writeHook)=>writeHook=fn};
}
function edit(f:ReturnType<typeof fixture>,value:string){const before=f.client.draft;f.client.local.drafts[f.target.key]=value;expect(mobileEditorDocumentWritten(f.client,f.target,before,value)).toBe(true)}
const records=(f:ReturnType<typeof fixture>)=>{const read=mobileComposerContextRead(f.client);return read.ok?read.context?.records??[]:[]};
function gate(){let resolve!:()=>void;const promise=new Promise<void>(r=>resolve=r);return {promise,resolve}}

test('Terminal publishes remembered selection, canonical context and one complete saved snapshot',async()=>{
  const f=fixture();f.document!.selection={start:6,end:11};const answer=await f.attach();
  expect(answer).toMatchObject({accepted:true,message:''});expect(f.client.draft).toMatch(/^hello \[.*\]\(t3-context:\/\/v1\/terminal\//);
  expect(f.client.draft).not.toContain('world');expect(records(f)).toMatchObject([{kind:'terminal',text:'second',lineStart:2,lineEnd:2}]);
  expect(terminalDraftRecords(f.client,f.client.draft)).toEqual([]);expect(f.writes).toHaveLength(1);
  expect(obj(f.writes[0]!.drafts)[f.target.key]).toBe(f.client.draft);expect(f.writes[0]!.mobileComposerContexts).toEqual(mobileComposerContextsPersisted(f.client));
  expect(f.calls.some(c=>c.op==='editorInsert'||c.op==='composerAttachRemove'||c.method==='orchestration.dispatchCommand')).toBe(false);
});
test('Terminal preserves typing during ID allocation and appends to latest text',async()=>{
  const f=fixture();f.document!.selection={start:6,end:11};f.onNative(request=>{if(request.op==='ids')edit(f,'new typing')});
  expect((await f.attach()).accepted).toBe(true);expect(f.client.draft).toStartWith('new typing [');
});
test('Terminal text ABA keeps the originally captured selection',async()=>{
  const f=fixture();f.document!.selection={start:6,end:11};f.onNative(request=>{if(request.op==='ids'){edit(f,'temporary');edit(f,'hello world')}});
  expect((await f.attach()).accepted).toBe(true);expect(f.client.draft).toStartWith('hello [');expect(f.client.draft).not.toContain('world');
});
test('Terminal refuses changed route, project and connection during ID allocation',async()=>{
  for(const change of [(f:ReturnType<typeof fixture>)=>f.client.threadId='other',(f:ReturnType<typeof fixture>)=>f.client.projectId='other',
    (f:ReturnType<typeof fixture>)=>f.client.generation++,(f:ReturnType<typeof fixture>)=>f.client.threadEpoch++]){
    const f=fixture();f.onNative(request=>{if(request.op==='ids')change(f)});const answer=await f.attach();
    expect(answer.accepted).toBe(false);expect(answer.message).toContain('changed');expect(f.client.local.drafts[f.target.key]).toBe('hello world');expect(f.writes).toHaveLength(0);
    expect(mobileComposerContextsPersisted(f.client)).toEqual({version:1,entries:{}});
  }
});
test('legacy Terminal cannot fall through after ordinary enrollment while IDs are pending',async()=>{
  const f=fixture('legacy',false);f.onNative(request=>{if(request.op==='ids')mobileEditorDocumentEnroll(f.client,f.target)});
  expect((await f.attach()).accepted).toBe(false);expect(f.client.draft).toBe('legacy');expect(terminalDraftRecords(f.client,f.client.draft)).toEqual([]);expect(f.writes).toHaveLength(0);
});
test('unavailable saved ownership refuses before the Terminal ID call',async()=>{
  const f=fixture();mobileEditorDocumentsHydrate(f.client,{mobileComposerEditor:{version:99,documents:{}}});
  expect((await f.attach()).accepted).toBe(false);expect(f.calls).toEqual([]);expect(f.client.draft).toBe('hello world');
});
test('accepted Terminal edit survives storage failure and reports acceptance for sheet dismissal',async()=>{
  const f=fixture();f.onWrite(()=>{throw new Error('disk full')});const answer=await f.attach();
  expect(answer.accepted).toBe(true);expect(answer.message).toContain('attached');expect(answer.message).toContain('could not be saved');
  expect(records(f)).toHaveLength(1);expect(f.document!.revision).toBe(1);expect(f.calls.some(c=>c.op==='composerAttachRemove')).toBe(false);
});
test('Terminal suppresses overlapping taps without a second ID request or insertion',async()=>{
  const f=fixture(),started=gate(),finish=gate();f.onWrite(async()=>{started.resolve();await finish.promise});
  const first=f.attach();await started.promise;const second=await f.attach();expect(second.accepted).toBe(false);expect(second.message).toContain('Wait');
  expect(f.calls.filter(c=>c.op==='ids')).toHaveLength(1);finish.resolve();expect((await first).accepted).toBe(true);expect(records(f)).toHaveLength(1);
});
test('Review accepts canonical context and closes its form before persistence settles',async()=>{
  const f=fixture();await f.pick();const started=gate(),finish=gate();f.onWrite(async()=>{started.resolve();await finish.promise});
  const saving=f.review('save','','Explain');await started.promise;
  expect(mobileReviewSnapshot(false,f.client)).toMatchObject({commentOpen:false,commentCount:1});expect(records(f)).toMatchObject([{kind:'review-comment',text:'Explain'}]);
  try { await expect(f.review('save','','Duplicate')).rejects.toMatchObject({kind:'superseded'}); }
  finally { finish.resolve(); }
  expect((await saving).message).toBe('');
  expect(records(f)).toHaveLength(1);expect(f.writes).toHaveLength(1);
});
test('Review failed save retains the accepted card once and exact canonical record',async()=>{
  const f=fixture();await f.pick();f.onWrite(()=>{throw new Error('disk full')});const answer=await f.review('save','','Explain');
  expect(answer.message).toContain('could not be saved');expect(answer.data).toMatchObject({commentOpen:false,commentCount:1});expect(records(f)).toHaveLength(1);
  await expect(f.review('save','','Explain')).rejects.toMatchObject({kind:'superseded'});expect(records(f)).toHaveLength(1);
});
test('Review refusal leaves form and draft unchanged; owned removal refuses desktop global deletion',async()=>{
  const f=fixture();await f.pick();f.document!.revision=Number.MAX_SAFE_INTEGER;
  expect((await f.review('save','','Explain')).message).not.toBe('');expect(mobileReviewSnapshot(false,f.client)).toMatchObject({commentOpen:true,commentCount:0});expect(f.client.draft).toBe('hello world');
  const good=fixture();await good.pick();expect((await good.review('save','','Explain')).message).toBe('');const prior=good.client.draft;
  expect((await good.review('delete-comment',String(records(good)[0]!.contextId))).message).toContain('not available yet');expect(good.client.draft).toBe(prior);expect(records(good)).toHaveLength(1);
});
test('a late Review save cannot clear a new route selection or create its card',async()=>{
  const f=fixture();await f.pick();const started=gate(),finish=gate();f.onWrite(async()=>{started.resolve();await finish.promise});
  const saving=f.review('save','','Original');await started.promise;f.client.threadId='other';await f.pick();
  expect(mobileReviewSnapshot(false,f.client)).toMatchObject({commentOpen:true,commentCount:0});finish.resolve();await saving;
  expect(mobileReviewSnapshot(false,f.client)).toMatchObject({commentOpen:true,commentCount:0});expect(f.client.draft).toBe('');expect(f.client.local.drafts[f.target.key]).toContain('review-comment');
});
test('legacy capture keeps its membership class and never enrolls merely to inspect it',()=>{
  const f=fixture('legacy',false),capture=mobileExternalContextCapture(f.client,f.target,'terminal','route')!;
  expect(capture.kind).toBe('legacy');expect(mobileExternalContextCurrent(f.client,capture,'route')).toBe(true);expect(mobileExternalContextCurrent(f.client,capture,'other')).toBe(false);
  mobileEditorDocumentEnroll(f.client,f.target);expect(mobileExternalContextCurrent(f.client,capture,'route')).toBe(false);
});
test('abandoned persistence propagates its exact error after accepted Terminal and Review edits',async()=>{
  for(const producer of ['terminal','review']){
    const f=fixture(),abandoned=Object.assign(new Error('Answer left'),{name:'FetchError',kind:'Aborted'});
    if(producer==='review')await f.pick();f.onWrite(()=>{throw abandoned});let caught:unknown;
    try{await (producer==='terminal'?f.attach():f.review('save','','Accepted'))}catch(error){caught=error}
    expect(caught).toBe(abandoned);expect(records(f)).toHaveLength(1);expect(f.document!.revision).toBe(1);
    if(producer==='review')expect(mobileReviewSnapshot(false,f.client)).toMatchObject({commentOpen:false,commentCount:1});
    expect(f.calls.some(c=>c.op==='composerAttachRemove')).toBe(false);
  }
});
test('Terminal rejects malformed raw cleanup before any target lookup and after pending IDs',async()=>{
  for(const timing of ['before','during']){
    const f=fixture(),local=f.client.local as unknown as Obj;
    if(timing==='before')local.mobileNewTaskDrafts=null;else f.onNative(request=>{if(request.op==='ids')local.mobileNewTaskDrafts=null});
    expect((await f.attach()).accepted).toBe(false);expect(local.mobileNewTaskDrafts).toBeNull();expect(f.client.draft).toBe('hello world');expect(f.writes).toHaveLength(0);
    expect(f.calls.filter(c=>c.op==='ids')).toHaveLength(timing==='before'?0:1);
  }
});
test('Review rejects malformed cleanup before projection can default the raw value',async()=>{
  const f=fixture();await f.pick();const owner=mobileReviewSnapshot(false,f.client).owner,local=f.client.local as unknown as Obj;local.mobileNewTaskDrafts=null;
  await expect(mobileReviewAction(owner,'save','','Explain',0,f.native,f.storage,false,f.client)).rejects.toMatchObject({kind:'retained'});
  expect(local.mobileNewTaskDrafts).toBeNull();expect(f.client.draft).toBe('hello world');expect(f.writes).toHaveLength(0);
});
