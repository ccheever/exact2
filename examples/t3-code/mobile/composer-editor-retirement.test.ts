// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {MobileDraftClient} from './mobile-draft-recovery';
import {mobileComposerTarget} from './composer-target';
import {obj,type Obj} from './shared/domain';
import type {Files,Native} from './shared/protocol';
import type {Pending} from './shared/client';
import {mobileEditorOwnerAdmit as admit,mobileEditorOwner,mobileEditorCaptureDocumentIntent as capture,mobileEditorCommitDocumentIntent as commit,type EditorRouteInput} from './composer-editor-owner';
import {mobileEditorDocumentEnroll as enroll,mobileEditorDocumentWritten as written,mobileEditorSendCapture,mobileEditorSendConfirmed,
 mobileEditorPersistSnapshot,mobileEditorPersistDocument,mobileEditorDocumentsHydrate,mobileEditorRetirementSnapshot,mobileEditorRetirementReceipt} from './composer-editor-persistence';
import {mobileEditorFlushRetirement as flush} from './composer-editor-retirement';
import {mobileComposerContextCapture,mobileComposerContextCommit,mobileComposerContextRead} from './composer-command-context';
let sequence=0;
function fixture(value='draft'){
 const client=new MobileDraftClient();Object.assign(client,{origin:'https://retirement.test',environmentId:`retirement-${++sequence}`,threadId:'a',projectId:'p',generation:1});
 client.local.drafts[client.draftKey]=value;const target=mobileComposerTarget(client),route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'editor',environmentId:client.environmentId,threadId:'a',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const calls:unknown[]=[],writes:Obj[]=[];const native:Native={available:true,watch(){},async later(r){calls.push(r);throw new Error('unexpected native call')}};
 const storage:Files={fs:{async mkdir(){},async readFile(){return new ArrayBuffer(0)},async atomicWriteFile(_path,bytes){writes.push(obj(JSON.parse(new TextDecoder().decode(bytes))))}}};
 const pending:Pending={method:'orchestration.dispatchCommand',payload:{type:'message.dispatch',commandId:'send',threadId:'a',text:value,attachments:[]},threadId:'a',text:value,description:'Send',uncertain:false};
 return {client,target,route,calls,writes,native,storage,pending};
}
test('named unmounted intent captures and commits A while focused on B, including its own context pruning and caret',()=>{
 const value='[Other](t3-context://v1/thread/thread_other)',f=fixture(value);enroll(f.client,f.target);
 const context=mobileComposerContextCapture(f.client,f.target)!;expect(mobileComposerContextCommit(f.client,context,value,{version:1,kind:'thread',contextId:'thread_other',label:'Other',title:'Other',environmentId:f.client.environmentId,threadId:'other'})).toBe(true);
 f.client.threadId='b';f.client.local.drafts[f.client.draftKey]='B untouched';const c=capture(f.client,f.target,'review',{start:2,end:2})!;
 expect(c).not.toBeNull();expect(commit(f.client,c,{value:'replacement',selection:{start:3,end:3}})).toBe(true);expect(f.client.draft).toBe('B untouched');expect(f.client.local.drafts[f.target.key]).toBe('replacement');
 expect(mobileComposerContextRead(f.client,f.target.key,'replacement')).toMatchObject({ok:true,context:undefined});expect(commit(f.client,c,{value:'replay',selection:{start:0,end:0}})).toBe(false);
 f.client.threadId='a';expect(admit(f.client,f.target,f.route,'catalog')?.state.selection).toEqual({start:3,end:3});
});
test('mount appearing after unmounted capture requires CAS instead of direct slot mutation',()=>{
 const f=fixture();enroll(f.client,f.target);const c=capture(f.client,f.target,'voice')!;const owner=admit(f.client,f.target,f.route,'catalog')!;owner.state={...owner.state,mountId:'native'};
 expect(commit(f.client,c,{value:'unsafe',selection:{start:0,end:0}})).toBe(false);expect(f.client.draft).toBe('draft');
});
test('named intent refuses changed connection and exact ABA application revision',()=>{
 const f=fixture('A');enroll(f.client,f.target);const c=capture(f.client,f.target,'terminal')!;
 f.client.local.drafts[f.target.key]='B';written(f.client,f.target,'A','B');f.client.local.drafts[f.target.key]='A';written(f.client,f.target,'B','A');expect(commit(f.client,c,{value:'bad',selection:{start:0,end:0}})).toBe(false);
 const fresh=capture(f.client,f.target,'voice')!;f.client.generation++;expect(commit(f.client,fresh,{value:'bad',selection:{start:0,end:0}})).toBe(false);
});
test('confirmed production preserve receipt does not call native or clear mounted draft',async()=>{
 const f=fixture();const owner=admit(f.client,f.target,f.route,'catalog')!;owner.state={...owner.state,mountId:'native'};
 const c=mobileEditorSendCapture(f.client,f.pending,false)!;mobileEditorSendConfirmed(f.client,f.pending,f.client.environmentId);
 expect((await flush(f.client,c.id,f.route,f.native,f.storage)).message).toContain('kept');expect(f.calls).toEqual([]);expect(f.client.draft).toBe('draft');expect(f.writes).toHaveLength(1);
});
test('even restored forged complete coverage becomes preserved without native IO; letGo on persist is propagated',async()=>{
 const f=fixture();enroll(f.client,f.target);const c=mobileEditorSendCapture(f.client,f.pending,false)!;const snapshot=mobileEditorPersistSnapshot(f.client);
 const r=obj(obj(obj(snapshot.raw).sends)[c.id]);r.phase='confirmed';r.authority='complete-v1';
 const saved=mobileEditorPersistDocument(snapshot,{drafts:f.client.local.drafts,pending:{}}),client=new MobileDraftClient();Object.assign(client,{origin:f.client.origin,environmentId:f.client.environmentId,threadId:'a',generation:1});client.local.drafts={...f.client.local.drafts};mobileEditorDocumentsHydrate(client,saved);
 const sentinel={name:'FetchError',kind:'Aborted'},storage:Files={fs:{...f.storage.fs,async atomicWriteFile(){throw sentinel}}};
 await expect(flush(client,c.id,f.route,f.native,storage)).rejects.toBe(sentinel);expect(f.calls).toEqual([]);expect(client.draft).toBe('draft');expect(mobileEditorRetirementReceipt(client,c.id)?.phase).toBe('preserved');
 await flush(client,c.id,f.route,f.native,f.storage);expect(f.calls).toEqual([]);expect(f.writes).toHaveLength(1);
 const savedAgain=f.writes[0]!,restarted=new MobileDraftClient();Object.assign(restarted,{origin:client.origin,environmentId:client.environmentId,threadId:'a',generation:2});restarted.local.drafts={...client.local.drafts};mobileEditorDocumentsHydrate(restarted,savedAgain);expect(mobileEditorRetirementReceipt(restarted,c.id)?.phase).toBe('preserved');
});
test('retirement refuses a merely pending request and never reruns server Send',async()=>{
 const f=fixture();enroll(f.client,f.target);const c=mobileEditorSendCapture(f.client,f.pending,false)!;
 await expect(flush(f.client,c.id,f.route,f.native,f.storage)).rejects.toMatchObject({kind:'retained'});expect(f.calls).toEqual([]);expect(mobileEditorRetirementSnapshot(f.client).items[0]?.phase).toBe('pending');
});
