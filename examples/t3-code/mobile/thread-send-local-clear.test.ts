// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadLocalCommandClear as clear} from './thread-send-local-clear';
import {mobileComposerTarget} from './composer-target';
import {mobileComposerContextsHydrate,mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileEditorDocument,mobileEditorDocumentKey,mobileEditorDocumentEnroll} from './composer-editor-persistence';
import {mobileDraftChanged} from './draft';
import {fleet} from './shared/settings-b-fleet';
import type {ThreadSendSnapshot} from './thread-send-admission';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
let serial=0;
async function fixture(text='/feedback detail',emptyContext=false) {
 const client=new MobileDraftClient();let disk:Obj={},failure:unknown=null,hook:(()=>Promise<void>)|undefined;
 const writes:Obj[]=[],calls:unknown[]=[];
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(disk)).buffer},async atomicWriteFile(_p,bytes){
  if(failure)throw failure;await hook?.();disk=obj(JSON.parse(new TextDecoder().decode(bytes)));writes.push(disk);
 }}};
 const native:Native={available:true,watch(){},async later(r){calls.push(r);return {ok:false,generation:0,error:{message:'Offline fixture'}}}};
 const handles=mobileDraftRecoveryHandles(client,native,storage);await client.refresh(handles.native,handles.storage);
 Object.assign(client,{origin:'https://clear.test',environmentId:`clear-${++serial}`,threadId:'thread',projectId:'project',generation:3});
 fleet.saved.push({origin:client.origin,environmentId:client.environmentId});client.local.drafts[client.draftKey]=text;
 const target=mobileComposerTarget(client),context={version:1,records:emptyContext?[]:[{version:1,kind:'skill',contextId:'unused',label:'Unused',name:'unused'}]};
 mobileComposerContextsHydrate(client,{mobileComposerContexts:{version:1,entries:{[JSON.stringify([client.origin,client.environmentId,target.key])]:{
  origin:client.origin,environmentId:client.environmentId,key:target.key,revision:1,text,context}}}});
 const snapshot:ThreadSendSnapshot={origin:client.origin,environmentId:client.environmentId,projectId:'project',threadId:'thread',draftKey:target.key,
  rawText:text,context,attachments:[],modelSelection:{instanceId:'p',model:'m'},runtimeMode:'full-access',interactionMode:'plan',providerDriver:'codex',
  activeProviderThreadId:'provider-thread',showInteractionModeToggle:true};
 writes.length=0;calls.length=0;
 const doc=()=>mobileEditorDocument(client,mobileEditorDocumentKey({origin:client.origin,environmentId:client.environmentId,draftKey:target.key}));
 return {client,target,snapshot,native,storage,writes,calls,doc,disk:()=>disk,fail(value:unknown){failure=value},hold(value:()=>Promise<void>){hook=value}};
}
for(const mode of ['text','content'] as const)test(`local ${mode} clear happens before persistence yields without a queued marker`,async()=>{
 const f=await fixture(mode==='text'?'/usage-limits':'/feedback detail');let release!:()=>void;
 const held=new Promise<void>(done=>{release=done});f.hold(()=>held);
 const answer=clear(f.client,f.target,f.snapshot,mode,f.native,f.storage);
 expect(f.client.draft).toBe('');expect(f.calls).toEqual([]);expect(f.writes).toEqual([]);
 release();expect(await answer).toEqual({applied:true,message:''});
 expect(obj(f.disk().drafts)[f.target.key]).toBe('');expect(Object.keys(obj(f.disk().mobileOutboxTransferCompletions))).toHaveLength(0);
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context?.records??[]).toEqual([]);
 if(mode==='content')expect(f.doc()).toMatchObject({revision:1,value:''});
});
test('full clear advances an existing enrolled document and tracks later plain typing',async()=>{
 const f=await fixture(),before=mobileEditorDocumentEnroll(f.client,f.target)!;
 const incarnation=before.incarnation;
 expect(await clear(f.client,f.target,f.snapshot,'content',f.native,f.storage)).toEqual({applied:true,message:''});
 await mobileDraftChanged(f.client,'next',f.native,f.storage,f.target.owner);
 expect(f.doc()).toMatchObject({incarnation,value:'next',revision:2});
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context).toBeNull();
});
test('changed text, context, route or saved catalog cannot clear captured command',async()=>{
 for(const mode of ['text','context','route','catalog'] as const){const f=await fixture();
  if(mode==='text')await mobileDraftChanged(f.client,'new text',f.native,f.storage,f.target.owner);
  if(mode==='context')f.snapshot.context={version:1,records:[]};
  if(mode==='route')f.client.threadId='other';
  if(mode==='catalog')fleet.saved.find(row=>row.environmentId===f.client.environmentId)!.origin='https://other.test';
  const before=structuredClone(f.client.local);f.writes.length=0;
  expect((await clear(f.client,f.target,f.snapshot,'content',f.native,f.storage)).applied).toBe(false);
  expect(f.client.local).toEqual(before);expect(f.writes).toEqual([]);
 }
});
test('attachments and malformed raw stores refuse without enrollment or normalization',async()=>{
 for(const mode of ['attachment','malformed'] as const){const f=await fixture();
  if(mode==='attachment')f.client.local.snapshotDrafts[f.target.key]=[{id:'11111111-1111-4111-8111-111111111111',name:'image.png',mimeType:'image/png',sizeBytes:3}];
  else Object.assign(f.client.local,{composerFiles:null});
  const before=structuredClone(f.client.local);
  expect((await clear(f.client,f.target,f.snapshot,'content',f.native,f.storage)).applied).toBe(false);
  expect(f.client.local).toEqual(before);expect(f.doc()).toBeNull();expect(f.writes).toEqual([]);
 }
});
for(const mode of ['text','content'] as const)test(`${mode} save failure truthfully retains applied clear and never restores command`,async()=>{
 const f=await fixture();f.fail(Error('save failed'));
 expect(await clear(f.client,f.target,f.snapshot,mode,f.native,f.storage)).toEqual({applied:true,message:'save failed'});
 expect(f.client.draft).toBe('');expect(f.calls).toEqual([]);
});
test('cancellation after content clear propagates while preserving the accepted state',async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'};f.fail(sentinel);
 await expect(clear(f.client,f.target,f.snapshot,'content',f.native,f.storage)).rejects.toBe(sentinel);
 expect(f.client.draft).toBe('');expect(f.calls).toEqual([]);
});

test('empty context envelope clears without inventing context records',async()=>{
 const f=await fixture('/feedback detail',true);
 expect(await clear(f.client,f.target,f.snapshot,'content',f.native,f.storage)).toEqual({applied:true,message:''});
 expect(mobileComposerContextSendSnapshot(f.client,f.target)?.context?.records??[]).toEqual([]);
});
test('blocked or exhausted enrolled document refuses local clear before changing content',async()=>{
 for(const mode of ['blocked','limit'] as const){const f=await fixture(),document=mobileEditorDocumentEnroll(f.client,f.target)!;
  if(mode==='blocked')document.blocked=true;else document.revision=Number.MAX_SAFE_INTEGER;
  for(const clearMode of ['text','content'] as const){expect((await clear(f.client,f.target,f.snapshot,clearMode,f.native,f.storage)).applied).toBe(false);
   expect(f.client.draft).toBe('/feedback detail');expect(f.writes).toEqual([])}
 }
});
