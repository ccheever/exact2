// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileThreadSendPlan as plan,type ThreadSendSnapshot,type ThreadSendFacts} from './thread-send-admission';
import {mobileComposerAttachmentInventoryRead} from './composer-attachment-publication';
import {mobileOutboxMessageContent} from './mobile-outbox-wire';
import type {MobileOutboxRecord} from './mobile-outbox-model';
import type {DraftFile} from './shared/composer-editor-files';
import type {Obj} from './shared/domain';
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const file=(n:number,extra:Partial<DraftFile>={}):DraftFile=>({id:id(n),contextId:`file_${n}`,draftKey:'env:thread',environmentId:'env',name:`${n}.txt`,mimeType:'text/plain',sizeBytes:21,source:'attached',attachmentId:'',status:'staged',...extra});
const image=(n:number):Obj=>({id:id(n),name:`${n}.png`,mimeType:'image/png',sizeBytes:11});
const record=(n:number,contextId=`file_${n}`):Obj=>({version:1,kind:'file',contextId,label:'file',attachmentId:id(n),name:`${n}.txt`,mimeType:'text/plain',sizeBytes:21});
function snapshot():ThreadSendSnapshot{return {origin:'http://localhost:4321',environmentId:'env',projectId:'project',threadId:'thread',draftKey:'env:thread',rawText:'  hello\n',attachments:[],modelSelection:{instanceId:'codex',model:'gpt',options:[{id:'x',value:true}]},runtimeMode:'full-access',interactionMode:'plan',providerDriver:'codex',activeProviderThreadId:'provider-thread',showInteractionModeToggle:true}}
function facts():ThreadSendFacts{return {connected:true,canOperate:true,pendingThreadCreation:false,queuedEdit:false,contextImporting:false,voiceBlocked:false,pendingPastedText:false,usageLimitsOffered:false,planModeEnabled:true,activeThreadBusy:false,canSteerActiveTurn:true,followUpBehavior:'steer',config:{environment:{capabilities:{attachmentUploads:true,fileAttachments:{maxUploadBytes:50*1024*1024}}}},uploadStates:{},uploadOwners:{}}}
function ready(s=snapshot(),f=facts()){const r=plan(s,f);expect(r.kind).toBe('message');if(r.kind!=='message')throw Error(JSON.stringify(r));return r.record}
const frozen=<T>(v:T):T=>{if(v&&typeof v==='object'){Object.freeze(v);Object.values(v).forEach(frozen)}return v};

test('trim is outgoing only; original settings and supplied context survive with no desktop pruning',()=>{
  const s=snapshot();s.rawText=' \t😀 hello\n ';s.context={version:1,records:[{version:1,kind:'mention',contextId:'unused',label:'file',path:'src/a.ts'}]};
  const before=structuredClone(s),result=ready(frozen(s),frozen(facts()));
  expect(result.text).toBe('😀 hello');expect(result.context).toEqual(s.context);expect(result.modelSelection).toEqual(s.modelSelection);
  expect(s).toEqual(before);expect(result).not.toHaveProperty('creation');expect(result).not.toHaveProperty('messageId');expect(result.threadId).toBe('thread');
});
test('actual inventory read feeds full mixed list including unreferenced file and file-only send',()=>{
  const s=snapshot();const read=mobileComposerAttachmentInventoryRead({snapshotDrafts:{'env:thread':[image(1)],foreign:[image(5)]},composerFiles:[file(2),file(3)],mobileAttachmentOrder:{'env:thread':[id(2),id(1),id(3)]},snapshotReleases:[],fileReleases:[]},{environmentId:'env',threadId:'thread',draftKey:'env:thread'});
  expect(read.ok).toBe(true);if(!read.ok)return;s.attachments=read.attachments;s.rawText='  ';
  expect(ready(s).attachments.map(a=>a.id)).toEqual([id(2),id(1),id(3)]);
  s.attachments=[{type:'file',id:id(2),file:file(2)}];expect(ready(s).text).toBe('');expect(ready(s).attachments).toHaveLength(1);
});
test('two context IDs share one canonical local file and existing wire remaps both exactly once',()=>{
  const s=snapshot();s.attachments=[{type:'file',id:id(1),file:file(1,{attachmentId:'remote',status:'ready'})}];
  s.context={version:1,records:[record(1,'alternate_a'),record(1,'alternate_b')]};
  const f=facts();f.uploadOwners[id(1)]={origin:s.origin,environmentId:s.environmentId};
  const r=ready(s,f),captured:MobileOutboxRecord={...r,messageId:'message',commandId:'command',createdAt:'2026-10-09T00:00:00.000Z'};
  const wire=mobileOutboxMessageContent(captured,[{kind:'reference',localId:id(1),attachment:{type:'file',id:'remote',name:'1.txt',mimeType:'text/plain',sizeBytes:21}}],true);
  expect(r.context).toEqual(s.context);expect(wire.attachments).toHaveLength(1);expect((wire.context!.records as Obj[]).map(r=>r.attachmentId)).toEqual(['remote','remote']);
});
test('offline queues all content despite absent transport permissions; connected denial refuses',()=>{
  const f=facts();f.canOperate=false;expect(plan(snapshot(),f).kind).toBe('refused');f.connected=false;expect(ready(snapshot(),f).text).toBe('hello');
});
test('source only failed upload states accepted by this server block; uploading and unsupported states queue',()=>{
  const s=snapshot();s.attachments=[{type:'file',id:id(1),file:file(1)}];const f=facts();
  for(const state of ['ready','uploading'] as const){f.uploadStates[id(1)]=state;expect(plan(s,f).kind).toBe('message')}
  f.uploadStates[id(1)]='failed';expect(plan(s,f).kind).toBe('refused');f.connected=false;expect(plan(s,f).kind).toBe('message');
  f.connected=true;f.config={environment:{capabilities:{}}};expect(plan(s,f).kind).toBe('message');
});
test('source caps allow exactly100 mixed attachments and reject recovery overflow without dropping files',()=>{
  const s=snapshot();s.attachments=Array.from({length:100},(_,i)=>({type:'file',id:id(i+1),file:file(i+1)}));expect(ready(s).attachments).toHaveLength(100);
  s.attachments.push({type:'image',id:id(101),image:image(101)});const before=structuredClone(s);expect(plan(s,facts()).kind).toBe('refused');expect(s).toEqual(before);
});
test('context full-record validation and limits occur without pruning stale references',()=>{
  const s=snapshot();s.context={version:1,records:Array.from({length:200},(_,i)=>({version:1,kind:'mention',contextId:`c${i}`,label:'x',path:'x'}))};expect(ready(s).context!.records).toHaveLength(200);
  (s.context as {records:Obj[]}).records.push({version:1,kind:'mention',contextId:'overflow',label:'x',path:'x'});expect(plan(s,facts()).kind).toBe('refused');
  s.context={version:1,records:[record(1)]};expect(plan(s,facts()).kind).toBe('refused');
});
test('local commands have exact case, whitespace, provider, attachment and offered semantics',()=>{
  const s=snapshot(),f=facts();f.usageLimitsOffered=true;
  for(const text of [' /USAGE-LIMITS\n','/usage-limits']){s.rawText=text;expect(plan(s,f)).toEqual({kind:'usage-limits'})}
  f.canOperate=false;expect(plan(s,f)).toEqual({kind:'usage-limits'});f.canOperate=true;f.usageLimitsOffered=false;expect(plan(s,f).kind).toBe('message');
  s.rawText=' /FeEdBaCk  useful\n feedback ';expect(plan(s,f)).toEqual({kind:'feedback',reason:'useful\n feedback'});
  s.rawText='/feedback';expect(plan(s,f)).toEqual({kind:'feedback'});s.rawText='/feedback-more';expect(plan(s,f).kind).toBe('message');
  s.rawText='/feedback';s.activeProviderThreadId=null;expect(plan(s,f).kind).toBe('refused');s.activeProviderThreadId='x';f.canOperate=false;f.connected=false;expect(plan(s,f).kind).toBe('refused');
  f.canOperate=true;s.providerDriver='claude-agent';expect(plan(s,f).kind).toBe('message');s.providerDriver='codex';s.attachments=[{type:'file',id:id(1),file:file(1)}];expect(plan(s,f).kind).toBe('message');
  s.rawText='/usage-limits';f.usageLimitsOffered=true;expect(plan(s,f).kind).toBe('message');
});
test('plan/model/provider commands remain literal text; effective plan settings are captured',()=>{
  const s=snapshot(),f=facts();for(const text of ['/model x','/plan','/compact','ultrathink hi']){s.rawText=text;expect(ready(s,f).text).toBe(text)}
  expect(ready(s,f).interactionMode).toBe('plan');f.planModeEnabled=false;expect(ready(s,f).interactionMode).toBe('plan');f.planModeEnabled=true;s.showInteractionModeToggle=false;expect(ready(s,f).interactionMode).toBe('default');
  s.rawText='';expect(plan(s,f).kind).toBe('refused');
});
test('follow-up captures source queue/steer/alternate/restart fallback, only active and steer-capable',()=>{
  const f=facts();f.activeThreadBusy=true;for(const mode of ['queue','steer','restart'] as const){f.followUpBehavior=mode;delete f.followUpOverride;
    expect(ready(snapshot(),f).dispatchMode).toBe(mode==='queue'?'queue':'auto');f.followUpOverride=mode==='queue'?'steer':'queue';expect(ready(snapshot(),f).dispatchMode).toBe(mode==='queue'?'auto':'queue')}
  f.canSteerActiveTurn=false;expect(ready(snapshot(),f)).not.toHaveProperty('dispatchMode');f.canSteerActiveTurn=true;f.activeThreadBusy=false;expect(ready(snapshot(),f)).not.toHaveProperty('dispatchMode');
});
test('source blockers refuse ordinary send, while empty ready source is not an implicit implement-plan',()=>{
  for(const key of ['pendingThreadCreation','queuedEdit','contextImporting','voiceBlocked','pendingPastedText'] as const){const f=facts();f[key]=true;expect(plan(snapshot(),f).kind).toBe('refused')}
  const s=snapshot();s.rawText=' \n';expect(plan(s,facts()).kind).toBe('refused');
});
test('connected Antigravity unavailable blocks but non-Antigravity catalog absence and offline selection survive',()=>{
  const s=snapshot(),f=facts();s.modelSelection={instanceId:'a',model:'x'};f.config={settings:{providerInstances:{a:{driver:'antigravity'}}}};expect(plan(s,f).kind).toBe('refused');f.connected=false;expect(plan(s,f).kind).toBe('message');f.connected=true;f.config=null;expect(plan(s,f).kind).toBe('message');
});
test('legacy memory files and missing remote provenance refuse; no artificial ID aliases',()=>{
  const s=snapshot(),f=facts();s.attachments=[{type:'file',id:id(1),file:file(1,{source:'pasted-text'})}];expect(plan(s,f).kind).toBe('refused');
  s.attachments=[{type:'file',id:id(1),file:file(1,{status:'ready',attachmentId:'remote'})}];expect(plan(s,f).kind).toBe('refused');
  f.uploadOwners[id(1)]={origin:'http://elsewhere',environmentId:'env'};expect(plan(s,f).kind).toBe('refused');
});
test('strict snapshot and fact validation refuses malformed supplied fields without coercion or mutation',()=>{
  const changes:Array<(s:ThreadSendSnapshot,f:ThreadSendFacts)=>void>=[s=>{s.draftKey='new-task:x'},s=>{s.context=null},s=>{s.modelSelection.options=[{id:'x',value:42 as never}]},
    s=>{s.attachments=[{type:'file',id:id(1),file:file(1,{status:['ready'] as never})}]},s=>{s.attachments=[{type:'image',id:id(1),image:{...image(1),source:[]}}]},
    s=>{s.attachments=[{type:'file',id:id(1),file:file(1)},{type:'file',id:id(1),file:file(1)}]},s=>{s.rawText='x'.repeat(1_000_001)},
    (_,f)=>{f.connected='true' as never},(_,f)=>{f.uploadStates[id(1)]='unknown' as never},s=>{s.context={version:1,records:[{kind:'unknown'}]}}];
  for(const change of changes){const s=snapshot(),f=facts();change(s,f);const before=structuredClone([s,f]);expect(plan(s,f).kind).toBe('refused');expect([s,f]).toEqual(before)}
  const s=snapshot(),sparse:any[]=[];sparse.length=1;(sparse as any).extra='x';s.attachments=sparse;expect(plan(s,facts()).kind).toBe('refused');
});

test('offered usage opens before source context cap and attachment binding; invalid raw context still refuses',()=>{
  const s=snapshot(),f=facts();s.rawText='/usage-limits';f.usageLimitsOffered=true;
  s.context={version:1,records:Array.from({length:201},(_,i)=>({version:1,kind:'mention',contextId:`c${i}`,label:'x',path:'x'}))};expect(plan(s,f)).toEqual({kind:'usage-limits'});
  s.context={version:1,records:[record(1)]};expect(plan(s,f)).toEqual({kind:'usage-limits'});
  s.context={version:1,records:[{kind:'invalid'}]};expect(plan(s,f).kind).toBe('refused');
});
