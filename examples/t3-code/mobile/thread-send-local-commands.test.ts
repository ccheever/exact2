// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
// Controlled invocation callbacks test the local owner; real draft/RPC/root acceptance is separate.
import {expect,test} from 'bun:test';
import {mobileThreadSendLocalCommand as send,mobileThreadLocalCommandsSnapshot as view,
  mobileThreadLocalFeedbackDismiss as dismiss,mobileThreadLocalUsageDismiss as close,mobileThreadLocalUsageOpen as open,
  type ThreadLocalCommandContext,type ThreadLocalPresentationInput} from './thread-send-local-commands';
import type {Obj} from './shared/domain';
import {hasProviderUsageLimits,collectProviderUsageLimits} from './thread-local-usage-model';
const now=Date.parse('2026-10-09T12:00:00Z');
const limits=(used=25):Obj=>({checkedAt:new Date(now).toISOString(),windows:[{id:'daily',kind:'primary',label:'Daily',usedPercent:used}]});
const provider=(more:Obj={}):Obj=>({instanceId:'codex',driver:'codex',enabled:true,installed:true,availability:'available',auth:{email:'a@example.com'},usageLimits:limits(),...more});
function context(text='/feedback useful'):ThreadLocalCommandContext {
  return {snapshot:{origin:'http://localhost:4321',environmentId:'env',projectId:'project',threadId:'a',draftKey:'env:a',rawText:text,
    attachments:[],modelSelection:{instanceId:'codex',model:'gpt'},runtimeMode:'full-access',interactionMode:'default',
    providerDriver:'codex',activeProviderThreadId:'provider-a',showInteractionModeToggle:true},
    facts:{connected:true,canOperate:true,pendingThreadCreation:false,queuedEdit:false,contextImporting:false,voiceBlocked:false,
      pendingPastedText:false,usageLimitsOffered:true,planModeEnabled:true,activeThreadBusy:false,canSteerActiveTurn:true,
      followUpBehavior:'queue',config:{providers:[provider()]},uploadStates:{},uploadOwners:{}},
    usageKey:'env:a|codex|run|pending',now,current:()=>true,clearDraft:async()=>({message:''}),uploadFeedback:async()=>({feedbackId:'remote-id'})};
}
function presentation(c:ThreadLocalCommandContext):ThreadLocalPresentationInput {
  const {origin,environmentId,threadId,draftKey}=c.snapshot;
  return {scope:{origin,environmentId,threadId,draftKey},usageKey:c.usageKey,instanceId:c.snapshot.modelSelection.instanceId,
    config:c.facts.config,userInputActive:false};
}
function deferred<T>(){let resolve!:(value:T)=>void,reject!:(reason:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no});return {promise,resolve,reject}}

// Source branch order and clear modes are observable before the callback promise resolves.
test('usage opens from config and starts text clear synchronously, before save with no RPC',async()=>{
  const owner={},c=context(' /USAGE-LIMITS\n'),saving=deferred<{message:string}>(),calls:string[]=[];
  c.facts.canOperate=false;c.facts.pendingThreadCreation=true;c.facts.queuedEdit=true;
  c.clearDraft=mode=>{calls.push(mode);return saving.promise};c.uploadFeedback=async()=>{calls.push('rpc');return {feedbackId:'wrong'}};
  const answer=send(owner,c);expect(calls).toEqual(['text']);expect(view(owner,presentation(c)).usage?.accounts).toHaveLength(1);
  saving.resolve({message:''});expect(await answer).toEqual({kind:'usage-limits',opened:true,message:''});expect(calls).toEqual(['text']);
});
test('unavailable report retains draft, closes prior report and returns source detail',async()=>{
  const owner={},c=context('/usage-limits');await send(owner,c);c.facts.config={providers:[]};let clears=0;c.clearDraft=async()=>{clears++;return {message:''}};
  expect(await send(owner,c)).toEqual({kind:'usage-limits',opened:false,message:'This provider does not currently report limits.'});
  expect(clears).toBe(0);expect(view(owner,presentation(c)).usage).toBeNull();
});
test('usage reprojects latest config at fixed time, user input hides it and key changes retire it',async()=>{
  const owner={},c=context('/usage-limits');await send(owner,c);const input=presentation(c);
  input.config={providers:[provider({usageLimits:limits(90)})]};const visible=view(owner,input);expect(visible.usage?.createdAt).toBe(new Date(now).toISOString());
  expect((visible.usage?.accounts[0]?.limits.windows as Obj[])[0]?.usedPercent).toBe(90);
  expect(view(owner,{...input,userInputActive:true}).usage).toBeNull();expect(view(owner,input).usage).not.toBeNull();
  expect(view(owner,{...input,usageKey:'changed-run'}).usage).toBeNull();expect(view(owner,input).usage).toBeNull();
});
test('usage close is exact owner and key; feedback never closes limits',async()=>{
  const owner={},c=context('/usage-limits');await send(owner,c);const input=presentation(c);
  expect(close(owner,{...input.scope,threadId:'b',draftKey:'env:b'},c.usageKey)).toBe(false);
  expect(close(owner,input.scope,'old-key')).toBe(false);c.snapshot.rawText='/feedback';await send(owner,c);
  expect(view(owner,input).usage).not.toBeNull();expect(close(owner,input.scope,c.usageKey)).toBe(true);expect(view(owner,input).usage).toBeNull();
});
test('usage save warning leaves shown report and never retries the edit',async()=>{
  const owner={},c=context('/usage-limits');let clears=0;c.clearDraft=async()=>{clears++;return {message:'Disk full'}};
  expect(await send(owner,c)).toEqual({kind:'usage-limits',opened:true,message:'Disk full'});expect(view(owner,presentation(c)).usage).not.toBeNull();expect(clears).toBe(1);
});
test('feedback publishes uploading then content clear synchronously and dispatches exact payload after save',async()=>{
  const owner={},c=context(' \n/FeEdBaCk  useful\n detail \n'),saved=deferred<{message:string}>(),uploaded=deferred<{feedbackId:string}>(),calls:unknown[]=[];
  c.clearDraft=mode=>{calls.push([mode,view(owner,presentation(c)).feedback[0]?.status]);return saved.promise};
  c.uploadFeedback=payload=>{calls.push(payload);return uploaded.promise};const answer=send(owner,c);
  expect(calls).toEqual([['content','uploading']]);const pending=view(owner,presentation(c));expect(pending.busy).toBe(true);expect(pending.feedback[0]?.title).toBe('Sending feedback to OpenAI...');
  expect(dismiss(owner,presentation(c).scope,pending.feedback[0]!.id)).toBe(false);
  saved.resolve({message:''});await Promise.resolve();expect(calls).toEqual([['content','uploading'],{threadId:'a',reason:'useful\n detail'}]);
  uploaded.resolve({feedbackId:'remote-123'});const result=await answer;expect(result.kind).toBe('feedback');
  const row=view(owner,presentation(c)).feedback[0]!;expect(row.title).toBe('Feedback sent to OpenAI');expect(row.description).toBe('Thread ID: remote-123');expect(row.feedbackId).toBe('remote-123');
  expect(view(owner,presentation(c)).busy).toBe(false);expect(dismiss(owner,presentation(c).scope,row.id)).toBe(true);expect(view(owner,presentation(c)).feedback).toEqual([]);
});
test('feedback has no reason field for bare command, and nonattachment source context is permitted',async()=>{
  const owner={},c=context('/feedback');c.snapshot.context={version:1,records:[{version:1,kind:'mention',contextId:'m',label:'M',path:'src/a.ts'}]};let payload:unknown;
  c.uploadFeedback=async value=>{payload=value;return {feedbackId:'ok'}};expect((await send(owner,c)).kind).toBe('feedback');expect(payload).toEqual({threadId:'a'});
});
test('one in-flight feedback per exact thread; second tap does not clear or upload',async()=>{
  const owner={},c=context(),upload=deferred<{feedbackId:string}>();let clear=0,rpc=0;c.clearDraft=async()=>{clear++;return {message:''}};c.uploadFeedback=()=>{rpc++;return upload.promise};
  const first=send(owner,c);expect(await send(owner,c)).toEqual({kind:'busy',message:''});expect(clear).toBe(1);expect(rpc).toBe(1);
  upload.resolve({feedbackId:'one'});await first;expect(view(owner,presentation(c)).feedback).toHaveLength(1);
});
test('late reply remains on A after viewing B; payload is captured before awaits',async()=>{
  const owner={},a=context(),upload=deferred<{feedbackId:string}>();let payload:unknown;a.uploadFeedback=value=>{payload=value;return upload.promise};const first=send(owner,a);await Promise.resolve();
  const b=context();b.snapshot.threadId='b';b.snapshot.draftKey='env:b';b.usageKey='b';expect(view(owner,presentation(b)).feedback).toEqual([]);
  a.snapshot.threadId='changed';a.snapshot.rawText='newer';upload.resolve({feedbackId:'a-result'});await first;
  const original=context();expect(payload).toEqual({threadId:'a',reason:'useful'});expect(view(owner,presentation(original)).feedback[0]?.feedbackId).toBe('a-result');expect(view(owner,presentation(b)).feedback).toEqual([]);
});
test('same thread under another home does not display or dismiss old feedback',async()=>{
  const owner={},c=context();await send(owner,c);const original=presentation(c),row=view(owner,original).feedback[0]!;
  const foreign={...original,scope:{...original.scope,origin:'http://localhost:9999'}};expect(view(owner,foreign).feedback).toEqual([]);expect(dismiss(owner,foreign.scope,row.id)).toBe(false);
  expect(view(owner,original).feedback).toHaveLength(1);
});
test('post-clear admission loss interrupts without dispatch or rewriting newer text',async()=>{
  const owner={},c=context(),saving=deferred<{message:string}>();let current=true,clears=0,rpc=0;
  c.current=()=>current;c.clearDraft=()=>{clears++;return saving.promise};c.uploadFeedback=async()=>{rpc++;return {feedbackId:'bad'}};
  const answer=send(owner,c);current=false;saving.resolve({message:''});expect(await answer).toMatchObject({kind:'feedback',status:'interrupted'});
  expect([clears,rpc]).toEqual([1,0]);expect(view(owner,presentation(c)).feedback).toEqual([]);expect(view(owner,presentation(c)).busy).toBe(false);
});
for(const during of ['clear','upload'] as const)test(`exact letGo from ${during} is rethrown, hidden and not retried`,async()=>{
  const owner={},c=context(),aborted={name:'FetchError',kind:'Aborted',identity:Symbol('actual')};let clear=0,rpc=0;
  c.clearDraft=async()=>{clear++;if(during==='clear')throw aborted;return {message:''}};
  c.uploadFeedback=async()=>{rpc++;throw aborted};let caught:unknown;try{await send(owner,c)}catch(e){caught=e}
  expect(caught).toBe(aborted);expect(view(owner,presentation(c)).feedback).toEqual([]);expect(view(owner,presentation(c)).busy).toBe(false);expect([clear,rpc]).toEqual([1,during==='clear'?0:1]);
});
test('save failure skips upload; ordinary upload failure retains cleared state and shows source failure',async()=>{
  const owner={},c=context();let rpc=0,clears=0;c.clearDraft=async()=>{clears++;return {message:'Draft save failed'}};c.uploadFeedback=async()=>{rpc++;throw Error('Provider rejected')};
  expect(await send(owner,c)).toMatchObject({status:'failed',message:'Draft save failed'});expect(rpc).toBe(0);
  c.clearDraft=async()=>{clears++;return {message:''}};expect(await send(owner,c)).toMatchObject({status:'failed',message:'Provider rejected'});
  const rows=view(owner,presentation(c)).feedback;expect(rows.map(row=>row.description)).toEqual(['Draft save failed','Provider rejected']);expect(rows[1]?.title).toBe('Could not send feedback to OpenAI');expect(clears).toBe(2);expect(rpc).toBe(1);
});
test('refused clear and malformed upload response are failures with no retry or success badge',async()=>{
  const owner={},c=context();let rpc=0;c.clearDraft=async()=>{throw Error('Draft owner changed')};c.uploadFeedback=async()=>{rpc++;return {feedbackId:''}};
  expect(await send(owner,c)).toMatchObject({status:'failed',message:'Draft owner changed'});expect(rpc).toBe(0);
  c.clearDraft=async()=>({message:''});expect(await send(owner,c)).toMatchObject({status:'failed',message:'The feedback server returned an invalid response.'});expect(rpc).toBe(1);
});
test('local owner uses ordinary source classification and refuses before mutation',async()=>{
  for(const edit of [(c:ThreadLocalCommandContext)=>{c.snapshot.activeProviderThreadId=null},(c:ThreadLocalCommandContext)=>{c.facts.canOperate=false},
    (c:ThreadLocalCommandContext)=>{c.facts.voiceBlocked=true},(c:ThreadLocalCommandContext)=>{c.facts.pendingPastedText=true},
    (c:ThreadLocalCommandContext)=>{c.current=()=>false}]){
    const c=context();let clears=0;c.clearDraft=async()=>{clears++;return {message:''}};edit(c);expect((await send({},c)).kind).toBe('refused');expect(clears).toBe(0);
  }
  for(const text of ['hello','/feedback-more','/usage-limits more']){const c=context(text);expect((await send({},c)).kind).toBe('not-local')}
  const c=context();c.snapshot.providerDriver='claudeAgent';expect((await send({},c)).kind).toBe('not-local');
});
test('mixed attachment command falls through to ordinary send without clear or local RPC',async()=>{
  const c=context(),id='00000000-0000-4000-8000-000000000001';let calls=0;c.clearDraft=async()=>{calls++;return {message:''}};c.uploadFeedback=async()=>{calls++;return {feedbackId:'bad'}};
  c.snapshot.attachments=[{type:'image',id,image:{id,name:'x.png',mimeType:'image/png',sizeBytes:1}}];expect((await send({},c)).kind).toBe('not-local');expect(calls).toBe(0);
});
test('report reuse excludes unavailable native providers, dedupes usable accounts and prefers hub reset path',()=>{
  expect(hasProviderUsageLimits('codex',[provider({enabled:false})],[])).toBe(false);
  expect(hasProviderUsageLimits('codex',[provider({installed:false})],[])).toBe(false);
  expect(hasProviderUsageLimits('codex',[provider({availability:'unavailable'})],[])).toBe(false);
  expect(hasProviderUsageLimits('codex',[],[{id:'hub',label:'Hub',accounts:[],error:'Offline'}])).toBe(true);
  const source={id:'hub',label:'Hub',accounts:[{id:'account',driver:'codex',email:' A@EXAMPLE.COM ',usageLimits:{...limits(70),resetCredits:{availableCount:1,nextCreditId:'credit'}}}]};
  const value=collectProviderUsageLimits('codex',[provider()],[source],now)!;expect(value.accounts).toHaveLength(1);
  expect(value.accounts[0]?.resetCreditInput).toEqual({sourceId:'hub',accountId:'account',creditId:'credit'});
});
test('returned report is detached from config and later views',async()=>{
  const c=context('/usage-limits'),owner={};await send(owner,c);const first=view(owner,presentation(c));first.usage!.accounts[0]!.limits.windows=[];
  expect(view(owner,presentation(c)).usage!.accounts[0]!.limits.windows).toHaveLength(1);
});

test('menu usage open shares panel owner without clearing or uploading a draft',()=>{
 const owner={},c=context('/us suffix'),input=presentation(c);
 expect(open(owner,{...input,now})).toEqual({opened:true,message:''});expect(view(owner,input).usage?.accounts).toHaveLength(1);
 expect(view(owner,input).busy).toBe(false);expect(view(owner,input).feedback).toEqual([]);
 expect(close(owner,input.scope,input.usageKey)).toBe(true);expect(view(owner,input).usage).toBeNull();
});
test('invalid menu ownership or time leaves an existing report unchanged',()=>{
 const owner={},c=context('/us'),input=presentation(c);open(owner,{...input,now});
 for(const patch of [{usageKey:''},{now:NaN},{scope:{...input.scope,draftKey:'foreign'}}]) {
  expect(open(owner,{...input,now,...patch})).toEqual({opened:false,message:'The selected usage report changed.'});
  expect(view(owner,input).usage?.accounts).toHaveLength(1);
 }
});
test('menu usage unavailable replaces the old panel without a typed-Send clear callback',()=>{
 const owner={},c=context('/us'),input=presentation(c);open(owner,{...input,now});
 expect(open(owner,{...input,now,config:{providers:[]}})).toEqual({opened:false,message:'This provider does not currently report limits.'});
 expect(view(owner,input).usage).toBeNull();
});
