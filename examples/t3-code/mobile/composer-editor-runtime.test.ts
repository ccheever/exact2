// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileEditorSnapshot as snapshot,mobileEditorAction as action,mobileEditorQueryWake as wake,mobileEditorPrepareImmediate as immediate,
 mobileEditorClaimEffect as claim,type EditorPresentation,type EditorRouteInput} from './composer-editor-runtime';
import {MobileDraftClient} from './mobile-draft-recovery';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileComposerContextRead} from './composer-command-context';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
let sequence=0;
function fixture(text='/mo'){
 const client=new MobileDraftClient(),environmentId=`runtime-${++sequence}`;Object.assign(client,{origin:'https://runtime.test',environmentId,threadId:'t',projectId:'p',generation:1,connection:'connected',providerId:'codex'});
 fleet.saved.push({environmentId,origin:client.origin});client.shell.projects=[{id:'p',workspaceRoot:'/repo'}];client.shell.threads=[{id:'t',projectId:'p',title:'Current'},{id:'other',projectId:'p',title:'Other task'}];
 client.config={providers:[{instanceId:'codex',driver:'codex',skills:[],slashCommands:[],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[]}]}]};client.local.drafts[client.draftKey]=text;
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'thread-composer',environmentId,threadId:'t',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const presentation:EditorPresentation={themeJson:'{}',placeholder:'Message',fontSize:16,lineHeight:20,enterBehavior:'send',iconUris:{typescript:'file:///real-bundle/pierre_typescript.png'},
 hasCompactableConversation:true,offersUsageLimits:false,allowInteractionMode:true,repository:'org/repo',permissionRevision:'session1'};
 let now=1000;const calls:Obj[]=[],writes:Obj[]=[];let response:((request:Obj)=>Promise<unknown>)|null=null,writeHook:(()=>Promise<void>)|null=null;
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode('{}').buffer},async atomicWriteFile(_path,bytes){writes.push(obj(JSON.parse(new TextDecoder().decode(bytes))));await writeHook?.()}}};
 const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);if(response)return response(request);
  if(request.op==='composerEditorApply'){const c=obj(request.command),next=obj(c.next);return {ok:true,generation:1,value:{event:{...obj(request.identity),kind:'commandApplied',eventCount:Number(obj(c.expected).eventCount)+1,
   commandId:c.commandId,commandRevision:c.commandRevision,reason:'',value:next.value,selection:next.selection,composing:false,focused:true}}}}
  return {ok:true,generation:1,value:request.op==='http'?{authenticated:true,permissions:['filesystem:read']}:request.method==='projects.searchEntries'?{entries:[{path:'src/a.ts',kind:'file'}]}:request.method==='pullRequests.list'?{entries:[],errors:[]}:{providers:client.config.providers}};
 }};
 const view=(raw='')=>snapshot(client,route,raw,presentation,now);
 const event=(kind:string,value=mobileEditorOwner(client)!.state.value,patch:Obj={})=>{const state=mobileEditorOwner(client)!.state;return JSON.stringify({...state.identity,mountId:state.mountId||'mount',kind,eventCount:state.eventCount+1,value,
   selection:{start:value.length,end:value.length},focused:true,composing:false,...patch})};
 const send=(kind:'event'|'rich'|'pick'|'dismiss'|'retry',payload:string)=>action(client,route,kind,payload,native,storage,()=>now);
 const ready=async()=>{view();await send('event',event('ready'));return view()};
 return {client,route,presentation,native,storage,calls,writes,view,event,send,ready,now:()=>now,time:(n:number)=>{now=n},response:(fn:(r:Obj)=>Promise<unknown>)=>{response=fn},holdWrite:(fn:()=>Promise<void>)=>{writeHook=fn}};
}
function deferred<T>(){let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
const tick=()=>new Promise<void>(r=>queueMicrotask(r));
test('ready handshake supports offline drafts and source /model pick inserts text without submit/navigation',async()=>{
 const f=fixture();f.client.connection='disconnected';const view=await f.ready();expect(view.enabled).toBe(true);expect(JSON.parse(view.configuration).editable).toBe(true);
 const item=view.menu.rows.find(r=>r.id==='cmd:model')!;expect(item.label).toBe('/model');
 await f.send('pick',JSON.stringify({admission:view.admission,menuRevision:view.menuRevision,id:item.id}));expect(f.client.draft).toBe('/model ');expect(f.calls.map(r=>r.op)).toEqual(['composerEditorApply']);expect(f.view().effect).toBeNull();
});
test('pure observation precedes persistence and two actual synchronous native writes count ABA',async()=>{
 const f=fixture('A');await f.ready();const hold=deferred<void>();f.holdWrite(()=>hold.promise);
 const b=f.event('text','B');f.view(b);expect(f.client.draft).toBe('A');expect(JSON.parse(f.view().configuration).document.value).toBe('B');
 const first=f.send('event',b);expect(f.client.draft).toBe('B');const second=f.send('event',f.event('text','A'));expect(f.client.draft).toBe('A');expect(mobileEditorOwner(f.client)?.document.revision).toBe(2);
 hold.resolve();await Promise.all([first,second]);expect(JSON.parse(f.view().configuration).document.value).toBe('A');
});
test('event/reply duplicates apply context once and latest typing wins over retained terminal',async()=>{
 const f=fixture('@Other');const view=await f.ready(),row=view.menu.rows.find(r=>r.id.startsWith('thread:'))!;expect(row).toBeDefined();
 f.response(async request=>{const c=obj(request.command),next=obj(c.next);const terminal={kind:'commandApplied',commandId:c.commandId,commandRevision:c.commandRevision,eventCount:Number(obj(c.expected).eventCount)+1,
  value:next.value,selection:next.selection,composing:false,focused:true,reason:''};
  const newer={...obj(request.identity),kind:'text',eventCount:Number(terminal.eventCount)+1,value:'newer',selection:{start:5,end:5},composing:false,focused:true,pendingCommand:terminal};
  await f.send('event',JSON.stringify(newer));return {ok:true,generation:1,value:{event:{...obj(request.identity),...terminal}}};
 });
 await f.send('pick',JSON.stringify({admission:view.admission,menuRevision:view.menuRevision,id:row.id}));expect(f.client.draft).toBe('newer');expect(mobileComposerContextRead(f.client)).toMatchObject({context:undefined});
 const contextText='[Other task](t3-context://v1/thread/thread_other)';await f.send('event',f.event('text',contextText));expect(mobileComposerContextRead(f.client)).toMatchObject({context:{records:[{contextId:'thread_other'}]}});
});
test('selection-only caret changes use real UTF16 positions and stale menu selection is refused',async()=>{
 const f=fixture('😀\n/mo suffix');await f.ready();await f.send('event',f.event('selection',undefined,{selection:{start:6,end:6}}));const v=f.view();expect(v.menu.rows.some(r=>r.id==='cmd:model')).toBe(true);
 await f.send('event',f.event('selection',undefined,{selection:{start:0,end:2}}));expect(f.view().menu.visible).toBe(false);
 await expect(f.send('pick',JSON.stringify({admission:v.admission,menuRevision:v.menuRevision,id:'cmd:model'}))).rejects.toMatchObject({kind:'superseded'});expect(f.calls).toHaveLength(0);
});
test('native IME refusal acknowledges command without changing mode or draft',async()=>{
 const f=fixture('/plan');const v=await f.ready();f.response(async request=>{const c=obj(request.command),expected=obj(c.expected);return {ok:true,generation:1,value:{event:{...obj(request.identity),kind:'commandRejected',eventCount:Number(expected.eventCount)+1,
 commandId:c.commandId,commandRevision:c.commandRevision,reason:'composition',value:expected.value,selection:expected.selection,composing:true,focused:true}}}});
 await f.send('pick',JSON.stringify({admission:v.admission,menuRevision:v.menuRevision,id:'cmd:plan'}));expect(f.client.draft).toBe('/plan');expect(f.client.interactionMode).not.toBe('plan');expect(mobileEditorOwner(f.client)?.state.pendingCommand).toBeNull();
});
test('letGo propagates original sentinel and retains plain command for explicit retry without cleanup calls',async()=>{
 const f=fixture();const v=await f.ready(),sentinel={name:'FetchError',kind:'Aborted'};f.response(async()=>{throw sentinel});
 await expect(f.send('pick',JSON.stringify({admission:v.admission,menuRevision:v.menuRevision,id:'cmd:model'}))).rejects.toBe(sentinel);expect(f.calls).toHaveLength(1);expect(f.client.draft).toBe('/mo');expect(mobileEditorOwner(f.client)?.pending).not.toBeNull();
});
test('new route retires held native reply and does not adopt into same-thread replacement',async()=>{
 const f=fixture();const v=await f.ready(),hold=deferred<unknown>();f.response(()=>hold.promise);const work=f.send('pick',JSON.stringify({admission:v.admission,menuRevision:v.menuRevision,id:'cmd:model'}));
 f.route.routeVisit='replacement';f.view();hold.resolve({ok:true,generation:1,value:{event:{}}});await expect(work).rejects.toMatchObject({kind:'superseded'});expect(f.client.draft).toBe('/mo');
});
test('root-only submit effects are captured once; unsupported rich callbacks never pretend to succeed',async()=>{
 const f=fixture('text');await f.ready();const event=f.event('submit',undefined,{alternate:true});await f.send('event',event);await f.send('event',event);const v=f.view();expect(v.effect?.kind).toBe('submit');expect(JSON.parse(v.effect!.payload).alternate).toBe(true);
 expect(claim(f.client,v.admission,v.effect!.id)?.id).toBe(v.effect!.id);expect(claim(f.client,v.admission,v.effect!.id)).toBeNull();expect(f.calls).toHaveLength(0);
 expect((await f.send('rich','{}')).message).toContain('not integrated');
});
test('initial path runs directly, later path wake waits200ms and uses actual query owner',async()=>{
 const f=fixture('@src');let v=await f.ready();expect(v.queries.immediateKey).not.toBe('');await immediate(f.client,v.admission,v.queries.immediateKey,f.native,f.now);expect(f.calls.map(r=>r.method||r.path)).toEqual(['/api/auth/session','projects.searchEntries']);
 await f.send('event',f.event('text','@other'));v=f.view();expect(v.queries.path.delayMs).toBe(200);f.time(1199);await wake(f.client,v.queries.path.key,f.native,f.now);expect(f.calls).toHaveLength(2);
 f.time(1200);await wake(f.client,v.queries.path.key,f.native,f.now);expect(f.calls).toHaveLength(4);expect(obj(f.calls[3]?.payload).query).toBe('other');
});
test('PR scope changes preserve query timer deadline and use latest request project',async()=>{
 const f=fixture('#one');let v=await f.ready();await immediate(f.client,v.admission,v.queries.immediateKey,f.native,f.now);
 await f.send('event',f.event('text','#two'));v=f.view();const key=v.queries.pullRequests.key;expect(v.queries.pullRequests.dueAt).toBe(1180);
 f.time(1090);f.client.projectId='p2';f.client.shell.projects.push({id:'p2',workspaceRoot:'/repo'});const next=f.view();expect(next.queries.pullRequests.key).toBe(key);expect(next.queries.pullRequests.dueAt).toBe(1180);
 f.time(1180);await wake(f.client,key,f.native,f.now);expect(obj(f.calls.at(-1)?.payload).projectId).toBe('p2');
});
test('query current ownership rejects an old timer after route retirement',async()=>{
 const f=fixture('@src');await f.ready();await f.send('event',f.event('text','@next'));const v=f.view();f.route.active=false;f.view();f.time(1300);
 await expect(wake(f.client,v.queries.path.key,f.native,f.now)).rejects.toMatchObject({kind:'superseded'});expect(f.calls).toHaveLength(0);
});
test('focus bursts never discard an accepted pending submit and another submit is explicitly refused',async()=>{
 const f=fixture('text');await f.ready();await f.send('event',f.event('submit',undefined,{alternate:false}));const original=f.view().effect!.id;
 for(let i=0;i<40;i++)await f.send('event',f.event(i%2?'focus':'blur'));
 expect(f.view().effect?.id).toBe(original);await f.send('event',f.event('submit',undefined,{alternate:true}));expect(f.view().message).toContain('previous submit');
 expect(mobileEditorOwner(f.client)?.effects.filter(e=>e.kind==='submit')).toHaveLength(1);expect(mobileEditorOwner(f.client)?.effects).toHaveLength(2);
});
test('same mount receives source presentation geometry/preferences and actual attachment upload clipboard binding',async()=>{
 const f=fixture('[photo](t3-context://v1/image/photo)');
 f.client.local.snapshotDrafts[f.client.draftKey]=[{id:'local-photo',uploadId:'server-photo',name:'photo.png',mimeType:'image/png',sizeBytes:10}];
 const context=await import('./composer-command-context');const target=(await import('./composer-target')).mobileComposerTarget(f.client);
 expect(context.mobileComposerContextCommit(f.client,context.mobileComposerContextCapture(f.client,target)!,f.client.draft,
  {version:1,kind:'image',contextId:'photo',label:'photo',attachmentId:'local-photo',name:'photo.png',mimeType:'image/png',sizeBytes:10})).toBe(true);
 const first=JSON.parse((await f.ready()).configuration);expect(JSON.parse(first.presentation.clipboardFragment).records[0].attachmentId).toBe('server-photo');
 Object.assign(f.presentation,{fontFamily:'DMSans-Regular',fontSize:20,lineHeight:26,contentInsetVertical:0,scrollEnabled:true,autoCorrect:false,spellCheck:false,textPasteThresholdBytes:32768,maxInputChars:1_000_000,submitTitle:'Queue',alternateSubmitTitle:'Steer'});
 const next=JSON.parse(f.view().configuration);expect(next.renderEpoch).toBe(first.renderEpoch);expect(next.mountId).toBe(first.mountId);
 expect(next.presentation).toMatchObject({fontFamily:'DMSans-Regular',fontSize:20,lineHeight:26,contentInsetVertical:0,scrollEnabled:true,autoCorrect:false,spellCheck:false,textPasteThresholdBytes:32768,submitTitle:'Queue',alternateSubmitTitle:'Steer'});
});
test('late applied mode choice does not overwrite newer settings and explicit retry key remains reviewable',async()=>{
 const f=fixture('/plan');const v=await f.ready();f.response(async request=>{const c=obj(request.command),next=obj(c.next);f.client.modelId='newer-model';return {ok:true,generation:1,value:{event:{...obj(request.identity),kind:'commandApplied',
 eventCount:Number(obj(c.expected).eventCount)+1,commandId:c.commandId,commandRevision:c.commandRevision,reason:'',value:next.value,selection:next.selection,focused:true,composing:false}}}});
 await f.send('pick',JSON.stringify({admission:v.admission,menuRevision:v.menuRevision,id:'cmd:plan'}));expect(f.client.interactionMode).not.toBe('plan');expect(f.view().message).toContain('model or mode changed');
 const g=fixture();const before=await g.ready();g.response(async()=>{throw {name:'FetchError',kind:'Aborted'}});await expect(g.send('pick',JSON.stringify({admission:before.admission,menuRevision:before.menuRevision,id:'cmd:model'}))).rejects.toMatchObject({kind:'Aborted'});
 expect(g.view().pendingCommandKey).toBe(mobileEditorOwner(g.client)?.pending?.id);
});
test('retained submit cannot send newer text or an equal-text ABA draft',async()=>{
 for(const aba of [false,true]){const f=fixture('old');await f.ready();await f.send('event',f.event('submit',undefined,{alternate:false}));const v=f.view();
 await f.send('event',f.event('text','new'));if(aba)await f.send('event',f.event('text','old'));
 expect(claim(f.client,v.admission,v.effect!.id)).toBeNull();expect(f.view().message).toContain('draft changed');expect(f.calls).toHaveLength(0)}
});
test('immediate work captures root minus wall offset once and preserves projection-to-invocation elapsed time',async()=>{
 const f=fixture('@src');await f.ready();const first=snapshot(f.client,f.route,'',f.presentation,1000,5000);
 // Change document to create a fresh immediate discovery/query admission at known clocks.
 f.route.routeVisit='clock-visit';const admitted=snapshot(f.client,f.route,'',f.presentation,1000,5000);
 const ready=f.event('ready');await f.send('event',ready);const work=snapshot(f.client,f.route,'',f.presentation,1000,5000);
 expect(work.queries.immediateClockOffset).toBe(-4000);const later=snapshot(f.client,f.route,'',f.presentation,1100,5100);
 expect(later.queries.immediateKey).toBe(work.queries.immediateKey);expect(later.queries.immediateClockOffset).toBe(-4000);
 await immediate(f.client,work.admission,work.queries.immediateKey,f.native,()=>5500+work.queries.immediateClockOffset);expect(f.calls).toHaveLength(2);
});
test('old operation reply stays redundant after event-first command and a newer completed command',async()=>{
 const f=fixture();const firstView=await f.ready(),hold=deferred<unknown>();let firstReply:unknown,seen=0;
 f.response(async request=>{const c=obj(request.command),next=obj(c.next),event={...obj(request.identity),kind:'commandApplied',eventCount:Number(obj(c.expected).eventCount)+1,
 commandId:c.commandId,commandRevision:c.commandRevision,reason:'',value:next.value,selection:next.selection,composing:false,focused:true};
 const reply={ok:true,generation:1,value:{event}};if(++seen===1){firstReply=reply;await f.send('event',JSON.stringify(event));return hold.promise}return reply});
 const first=f.send('pick',JSON.stringify({admission:firstView.admission,menuRevision:firstView.menuRevision,id:'cmd:model'}));await tick();await tick();
 await f.send('event',f.event('text','/model '));await f.send('event',f.event('text','/plan'));const second=f.view();
 await f.send('pick',JSON.stringify({admission:second.admission,menuRevision:second.menuRevision,id:'cmd:plan'}));expect(f.client.interactionMode).toBe('plan');
 const before=f.client.draft;hold.resolve(firstReply);expect((await first).message).toBe('');expect(f.client.draft).toBe(before);expect(f.view().message).toBe('');expect(f.calls).toHaveLength(2);
});
test('effect claims advance observable revision and remembered native caret survives route return',async()=>{
 const f=fixture('abcdef');await f.ready();await f.send('event',f.event('selection',undefined,{selection:{start:2,end:2}}));
 await f.send('event',f.event('focus',undefined,{selection:{start:2,end:2}}));const before=f.view();expect(claim(f.client,before.admission,before.effect!.id)).not.toBeNull();expect(f.view().revision).toBeGreaterThan(before.revision);
 f.route.active=false;f.view();f.route.active=true;f.route.routeVisit='return';expect(JSON.parse(f.view().configuration).document.selection).toEqual({start:2,end:2});
});
test('identical menu rows cannot admit an old callback after provider or workspace scope changes',async()=>{
 const f=fixture();const prior=await f.ready();f.client.projectId='other-project';f.client.shell.projects.push({id:'other-project',workspaceRoot:'/other'});const next=f.view();
 expect(next.menu.rows).toEqual(prior.menu.rows);expect(next.menuRevision).not.toBe(prior.menuRevision);
 await expect(f.send('pick',JSON.stringify({admission:prior.admission,menuRevision:prior.menuRevision,id:'cmd:model'}))).rejects.toMatchObject({kind:'superseded'});expect(f.calls).toHaveLength(0);
});
test('invalid saved context remains blocked while display keeps canonical unavailable chips',async()=>{
 const f=fixture('[Saved](t3-context://v1/mention/item)');const context=await import('./composer-command-context');
 context.mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:9,entries:{}}});const view=await f.ready();
 expect(view.message).toContain('unsupported');const control=JSON.parse(view.configuration),tokens=JSON.parse(control.document.tokensJson);expect(tokens).toHaveLength(1);expect(tokens[0].label).toBe('Saved · unavailable');
 expect(context.mobileComposerContextRead(f.client).ok).toBe(false);
});
