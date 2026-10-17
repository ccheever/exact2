// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerFileHistoryProject as project,mobileComposerFileHistoryDispose as dispose,
 type ComposerFileHistory,type ComposerFileHistoryInput,type ComposerFileHistoryAttachment} from './composer-file-history';
import {mobileFileHoldsCreate as create,mobileFileHoldReserve as reserve,mobileFileHoldAcquire as acquire,
 mobileFileHoldReleaseNeeded as releaseNeeded,mobileFileHoldRelease as release,mobileFileHoldHeld as held,
 type HeldFile} from './composer-file-holds-io';
import type {DraftFile} from './shared/composer-editor-files';
import type {Native} from './shared/protocol';
import type {Obj} from './shared/domain';
const uuid=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const identity={owner:JSON.stringify(['ordinary','https://history.test','env',3,'env:thread']),editorId:'composer',routeVisit:'visit',renderEpoch:'epoch',mountId:'mount'};
const target={origin:'https://history.test',environmentId:'env',threadId:'thread',draftKey:'env:thread'};
const file=(n:number):DraftFile=>({id:uuid(n),contextId:`c${n}`,draftKey:target.draftKey,environmentId:'env',name:`file${n}.txt`,mimeType:'text/plain',sizeBytes:n,source:'attached',attachmentId:`uploaded-${n}`,status:'ready'});
const binding=(f:DraftFile,n:number):HeldFile=>({request:{op:'composerFileHold',action:'acquire',generation:3,identity,requestId:uuid(10000+n),target,file:structuredClone(f)},
 receipt:{identity,requestId:uuid(10000+n),holdId:uuid(20000+n),fileIdentity:uuid(30000+n),id:f.id,sizeBytes:f.sizeBytes}});
const entry=(n:number)=>{const f=file(n);return {file:f,hold:binding(f,n)}};
const history=(count=0):ComposerFileHistory=>({identity,target,revision:0,closed:false,entries:Array.from({length:count},(_,i)=>entry(i+1))});
const attachment=(n:number):ComposerFileHistoryAttachment=>({type:'file',...entry(n)});
const record=(n:number,contextId=`c${n}`,kind='file'):Obj=>({version:1,kind,contextId,label:contextId,attachmentId:uuid(n),name:`file${n}.txt`,mimeType:'text/plain',sizeBytes:n});
function input(h:ComposerFileHistory,attachments:ComposerFileHistoryAttachment[]=[],records?:Obj[]):ComposerFileHistoryInput {
 const holds=new Map(h.entries.map(e=>[e.hold.request.requestId,e.hold]));
 for(const a of attachments)if(a.type==='file'&&a.hold)holds.set(a.hold.request.requestId,a.hold);
 return {identity,target,attachments,context:records?{version:1,records}:undefined,usableHolds:[...holds.values()]};
}
function ready(h:ComposerFileHistory,i:ComposerFileHistoryInput){const r=project(h,i);if(r.status!=='ready')throw Error(JSON.stringify(r));return r}

test('refreshes latest metadata and recency without rewriting immutable native acquire payload',()=>{
 const h=history(3),before=structuredClone(h),one=h.entries[0]!,latest={...one.file,name:'renamed.pdf',mimeType:'application/pdf',attachmentId:'new-upload'};
 const r=ready(h,input(h,[{type:'file',file:latest,hold:one.hold}]));
 expect(r.history.entries.map(e=>e.file.id)).toEqual([uuid(2),uuid(3),uuid(1)]);expect(r.history.entries[2]!.file.name).toBe('renamed.pdf');
 expect(r.history.entries[2]!.hold.request.file.name).toBe('file1.txt');expect(r.release).toEqual([]);expect(h).toEqual(before);
 expect(r.expectedRevision).toBe(0);expect(r.history.revision).toBe(1);
});
test('new canonical binding replaces one local id and emits exact previous release only after publication',()=>{
 const h=history(1),f=h.entries[0]!.file,next=binding(f,99),i=input(h,[{type:'file',file:f,hold:next}]),r=ready(h,i);
 expect(r.release).toEqual([h.entries[0]!.hold]);expect(r.history.entries[0]!.hold).toEqual(next);expect(h.entries[0]!.hold.request.requestId).toBe(uuid(10001));
 r.release[0]!.request.file.name='consumer mutation';expect(h.entries[0]!.hold.request.file.name).toBe('file1.txt');
});
test('missing live holds return preflight needs without candidate or old-release side effects',()=>{
 const h=history(2),i=input(h,[{type:'file',file:file(3),hold:null}]);expect(project(h,i)).toEqual({status:'needs-holds',files:[file(3)]});expect(h.revision).toBe(0);
 expect(project(h,{...i,usableHolds:[]})).toEqual({status:'refused',reason:'unusable-hold'});
});
test('source capacity counts images and refreshes old live files before oldest eviction',()=>{
 const h=history(205),images:ComposerFileHistoryAttachment[]=Array.from({length:205},(_,n)=>({type:'image',id:`image${n}`}));
 expect(ready(h,input(h,images)).history.entries).toHaveLength(205);
 const r=ready(h,input(h,[attachment(1)]));expect(r.history.entries).toHaveLength(200);expect(r.release.map(v=>v.receipt.id)).toEqual([2,3,4,5,6].map(uuid));
 expect(r.history.entries.at(-1)!.file.id).toBe(uuid(1));expect(h.entries).toHaveLength(205);
});
test('overflow live files are retained; history limit never trims the live recovery draft',()=>{
 const h=history(),all=Array.from({length:205},(_,i)=>attachment(i+1)),r=ready(h,input(h,all));expect(r.history.entries).toHaveLength(205);expect(r.release).toEqual([]);
 const trimmed=ready(r.history,input(r.history));expect(trimmed.history.entries).toHaveLength(200);expect(trimmed.release.map(v=>v.receipt.id)).toEqual([1,2,3,4,5].map(uuid));
});
test('already-restored context order controls deduplication, unavailable ids and image exclusion',()=>{
 const h=history(4),records=[record(3),record(1),record(3,'duplicate'),record(8),record(8,'unavailable-duplicate'),record(2),record(4,'image','image')];
 const r=ready(h,input(h,[{type:'image',id:uuid(2)}],records));expect(r.restoredFiles.map(f=>f.id)).toEqual([uuid(3),uuid(1)]);expect(r.unavailable).toEqual([uuid(8)]);
 expect(r.restoredFiles.every(f=>f.attachmentId===''&&f.status==='staged')).toBe(true);expect(h.entries[0]!.file.attachmentId).toBe('uploaded-1');
});
test('restore resets upload only and preserves scope, local/context identities and dimensions',()=>{
 const old=entry(1),f={...old.file,mimeType:'video/mp4',videoWidth:1920,videoHeight:1080},h={...history(),entries:[{file:f,hold:old.hold}]};
 const restored=ready(h,input(h,[],[record(1)])).restoredFiles[0];expect(restored).toEqual({...f,attachmentId:'',status:'staged'});expect(restored!.contextId).toBe('c1');
});
test('dispose returns detached cleanup obligations, closes even exhausted revision and is idempotent',()=>{
 const h={...history(2),revision:Number.MAX_SAFE_INTEGER},r=dispose(h);if(r.status!=='ready')throw Error('dispose');
 expect(r.history.closed).toBe(true);expect(r.history.revision).toBe(h.revision);expect(r.history.entries).toEqual([]);expect(r.release).toEqual(h.entries.map(e=>e.hold));
 expect(project(r.history,input(r.history))).toEqual({status:'refused',reason:'owner'});expect(dispose(r.history)).toEqual({status:'ready',history:r.history,release:[]});
 expect(project(h,input(h))).toEqual({status:'refused',reason:'limit'});expect(h.closed).toBe(false);
});
test('owner target and mount are captured independently of revision',()=>{
 const h=history(1);expect(project(h,{...input(h),identity:{...identity,mountId:'other'}})).toEqual({status:'refused',reason:'owner'});
 expect(project(h,{...input(h),target:{...target,threadId:'other',draftKey:'env:other'}})).toEqual({status:'refused',reason:'invalid'});
});
for(const corrupt of [
 (h:any)=>{h.entries.push(structuredClone(h.entries[0]))},(h:any)=>{h.entries[0].file.sizeBytes++},(h:any)=>{h.entries[0].hold.receipt.requestId=uuid(99)},
 (h:any)=>{h.entries[0].hold.request.file.extra=true},(h:any)=>{h.entries[0].hold.request.generation=4},(h:any)=>{h.revision=Infinity},
 (h:any)=>{h.unused={bad:undefined}},(h:any)=>{h.entries[0].file.videoWidth=true},(h:any)=>{h.entries[0].hold.receipt.sizeBytes='1'},
 (h:any)=>{h.entries[0].file.circular=h},(h:any)=>{h.entries[0].file.status=['ready']},(h:any)=>{h.entries[0].file.videoWidth=undefined},
])test('invalid unused/history state refuses without pruning or laundering',()=>{
 const h=structuredClone(history(1));corrupt(h);expect(project(h,input(history()))).toEqual({status:'refused',reason:'invalid'});expect(dispose(h)).toEqual({status:'refused',reason:'invalid'});
});
test('valid missing history differs from malformed context and unsupported namespaces',()=>{
 const h=history();expect(ready(h,input(h,[],[record(99)])).unavailable).toEqual([uuid(99)]);
 expect(project(h,input(h,[],[{...record(99),sizeBytes:-1}]))).toEqual({status:'refused',reason:'invalid'});
 expect(project(h,input(h,[],[record(1),record(2,'c1')]))).toEqual({status:'refused',reason:'invalid'});
 expect(project(h,input(h,[{type:'file',file:{...file(1),source:'pasted-text'},hold:null}]))).toEqual({status:'refused',reason:'unsupported'});
 expect(project(h,input(h,[{type:'file',file:{...file(1),status:['staged']} as unknown as DraftFile,hold:binding(file(1),1)}]))).toEqual({status:'refused',reason:'invalid'});
 expect(project(h,{...input(h),extra:true} as ComposerFileHistoryInput)).toEqual({status:'refused',reason:'invalid'});
});
test('real IO adapter only supplies usable history while held; release-needed/releasing cannot resurrect it',async()=>{
 const ledger=create(identity,3,target),request=reserve(ledger,file(1),uuid(101)),calls:unknown[]=[];
 let finish!:(value:unknown)=>void;const wait=new Promise(resolve=>{finish=resolve});
 const native:Native={available:true,watch(){throw Error('unexpected watch')},async later(raw){calls.push(raw);return (raw as any).action==='acquire'
  ?{ok:true,generation:3,value:{status:'held',receipt:{identity,requestId:request.requestId,holdId:uuid(102),fileIdentity:uuid(103),id:file(1).id,sizeBytes:1}}}:wait}};
 expect(held(ledger,request.requestId)).toBeNull();await acquire(ledger,request.requestId,native);
 const usable=held(ledger,request.requestId)!;expect(usable).not.toBeNull();
 const h=ready(history(),{...input(history(),[{type:'file',file:file(1),hold:usable}]),usableHolds:[usable]}).history;
 expect(ready(h,{...input(h,[],[record(1)]),usableHolds:[held(ledger,request.requestId)!]}).restoredFiles).toHaveLength(1);
 releaseNeeded(ledger,request.requestId);expect(held(ledger,request.requestId)).toBeNull();expect(ledger.entries[request.requestId]!.phase).toBe('release-needed');
 expect(project(h,{...input(h),usableHolds:[]})).toEqual({status:'refused',reason:'unusable-hold'});
 const pending=release(ledger,request.requestId,native);expect(ledger.entries[request.requestId]!.phase).toBe('releasing');expect(held(ledger,request.requestId)).toBeNull();
 expect(project(h,{...input(h),usableHolds:[]})).toEqual({status:'refused',reason:'unusable-hold'});
 finish({ok:true,generation:3,value:{status:'released',requestId:request.requestId}});await pending;expect(calls).toHaveLength(2);expect(held(ledger,request.requestId)).toBeNull();
});
