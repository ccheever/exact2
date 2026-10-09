// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {T3Client} from './shared/client';
import type {MobileComposerTarget} from './composer-target';
import {mobileEditorOwnerAdmit as admit,mobileEditorOwnerWritten as written,mobileEditorCaptureIntent as capture,
 mobileEditorIntentCurrent as current,mobileEditorPublishCommitted as publish,mobileEditorOwner as owner,type EditorRouteInput} from './composer-editor-owner';
function fixture(){const client=new T3Client();Object.assign(client,{origin:'https://owner.test',environmentId:'e',threadId:'t',generation:1});
 const target:MobileComposerTarget={kind:'ordinary',owner:'ordinary-one',key:'e:t',editorOwner:'e:t',editOwner:'',origin:client.origin,environmentId:'e',generation:1,projectId:'p',threadId:'t',incarnation:''};
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId:'e',threadId:'t',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 client.local.drafts['e:t']='initial';return {client,target,route}}
test('same admission ignores stale caller draft; fresh route gets a new epoch',()=>{const f=fixture(),a=admit(f.client,f.target,f.route,'catalog')!;
 a.state={...a.state,value:'observed'};expect(admit(f.client,f.target,f.route,'catalog')).toBe(a);expect(a.state.value).toBe('observed');
 const b=admit(f.client,f.target,{...f.route,routeVisit:'next'},'catalog')!;expect(b.state.identity.renderEpoch).not.toBe(a.state.identity.renderEpoch)});
test('explicit reducer boundaries count ABA and reject stale intents',()=>{const f=fixture();admit(f.client,f.target,f.route,'catalog');const old=capture(f.client,f.target,'voice')!;
 f.client.local.drafts['e:t']='B';expect(written(f.client,f.target,'initial','B')).toBe(true);f.client.local.drafts['e:t']='initial';expect(written(f.client,f.target,'B','initial')).toBe(true);
 expect(owner(f.client)?.document.revision).toBe(2);expect(current(f.client,old)).toBe(false)});
test('generation and queued/new-task scopes never publish ordinary writes',()=>{const f=fixture();admit(f.client,f.target,f.route,'catalog');f.client.generation=2;
 expect(written(f.client,f.target,'initial','initial')).toBe(false);expect(admit(f.client,{...f.target,kind:'queued-edit'},f.route,'catalog')).toBeNull();
 expect(admit(f.client,{...f.target,key:'new-task:x'},f.route,'catalog')).toBeNull()});
test('unmounted named publication keeps explicit interior return caret and refuses replay',()=>{const f=fixture();admit(f.client,f.target,f.route,'catalog');const intent=capture(f.client,f.target,'review',{start:2,end:2})!;
 admit(f.client,f.target,{...f.route,active:false},'catalog');f.client.local.drafts['e:t']='replacement';expect(publish(f.client,intent,{value:'replacement',selection:{start:3,end:3}})).toBe(true);
 expect(publish(f.client,intent,{value:'replacement',selection:{start:3,end:3}})).toBe(false);const next=admit(f.client,f.target,f.route,'catalog')!;expect(next.state.selection).toEqual({start:3,end:3})});
test('unrecorded external write never becomes a newly stamped controlled overwrite',()=>{const f=fixture();admit(f.client,f.target,f.route,'catalog');admit(f.client,f.target,{...f.route,active:false},'catalog');
 f.client.local.drafts['e:t']='unannounced';expect(admit(f.client,f.target,f.route,'catalog')).toBeNull()});
