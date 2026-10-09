// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import {expect,test} from 'bun:test';
import {T3Client} from './shared/client';
import {mobileDraftAttachmentOrdersPersisted} from './draft-attachment-order';
import {threadSendTransferDecodeCapture as capture,threadSendTransferDecodeClaim as claim,threadSendTransferDecodeCompletion as completion,
  threadSendTransferDecodeDraftContent as content,type ThreadSendTransferCapture,type ThreadSendTransferClaim,type ThreadSendTransferCompletion} from './thread-send-transfer-model';
import type {MobileOutboxRecord} from './mobile-outbox-model';
import type {Obj} from './shared/domain';
const id=(n:number)=>`abcdef00-0000-4000-8000-${String(n).padStart(12,'0')}`;
const clone=<T>(v:T):T=>structuredClone(v);
const scope={origin:'https://send.test',environmentId:'env',threadId:'thread',key:'env:thread'};
const image={id:id(1),name:'image.png',mimeType:'image/png',sizeBytes:4,source:{kind:'capture',future:{keep:true}}};
const file={id:id(2),name:'file.txt',mimeType:'text/plain',sizeBytes:3,draftKey:scope.key,environmentId:'env',contextId:'legacy-unreferenced',source:'attached',attachmentId:'',status:'staged' as const,future:['preserve',{n:2}]};
const recordContext={version:1 as const,records:[
  {version:1,contextId:'first-canonical',kind:'file',label:'First',attachmentId:file.id,name:file.name,mimeType:file.mimeType,sizeBytes:file.sizeBytes},
  {version:1,contextId:'second-canonical',kind:'file',label:'Second',attachmentId:file.id,name:file.name,mimeType:file.mimeType,sizeBytes:file.sizeBytes},
  {version:1,contextId:'unused-skill',kind:'skill',label:'Skill',name:'unreferenced'}] as Obj[]};
function fixture(){
  const c:ThreadSendTransferCapture={version:2,kind:'ordinary',draft:{...scope,document:{incarnation:'doc-a',revision:7,selection:{start:1,end:2}},
    text:'  raw send  ',context:clone(recordContext),contextRevision:9,images:[clone(image)],files:[clone(file)],attachmentIds:[file.id,image.id],attachmentOrder:['stale',file.id]}};
  const r:MobileOutboxRecord={schemaVersion:1,origin:scope.origin,environmentId:'env',threadId:'thread',messageId:'message',commandId:'command',text:'raw send',context:clone(recordContext),
    attachments:[{id:file.id,kind:'file',name:file.name,mimeType:file.mimeType,sizeBytes:3,uploadId:'',status:'staged',contextId:file.contextId,source:'attached'},
      {id:image.id,kind:'image',name:image.name,mimeType:image.mimeType,sizeBytes:4,uploadId:'',status:'staged',source:clone(image.source)}],
    modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default',createdAt:'2026-10-09T00:00:00.000Z'};
  const q:ThreadSendTransferClaim={kind:'ordinary',origin:scope.origin,environmentId:'env',transferId:'message',draftKey:scope.key,fingerprint:'a'.repeat(64),
    messageId:'message',threadId:'thread',commandId:'command',mutationId:'epoch:1',state:'queued',capture:c,record:r};
  const preserved:ThreadSendTransferCompletion={version:2,kind:'ordinary',draftKey:scope.key,fingerprint:q.fingerprint,disposition:'preserved',before:{incarnation:'doc-a',revision:7},
    after:{document:{origin:scope.origin,environmentId:'env',threadId:'thread',draftKey:scope.key,incarnation:'doc-a',revision:8,selection:{start:5,end:5}},
      text:'newer',context:{origin:scope.origin,environmentId:'env',key:scope.key,revision:10,text:'newer',context:clone(recordContext)},
      images:clone(c.draft.images),files:clone(c.draft.files),attachmentIds:clone(c.draft.attachmentIds),attachmentOrder:clone(c.draft.attachmentIds)}};
  const cleared:ThreadSendTransferCompletion={...clone(preserved),disposition:'cleared',after:{document:{...preserved.after.document,selection:{start:0,end:0}},
    text:'',context:null,images:[],files:[],attachmentIds:[],attachmentOrder:null}};
  return {c,r,q,preserved,cleared};
}
function set(raw:unknown,path:string,value:unknown){const keys=path.split('.');let current=raw as Record<string,unknown>;for(const k of keys.slice(0,-1))current=current[k] as Record<string,unknown>;current[keys.at(-1)!]=value}

test('complete raw capture preserves metadata/order and two context records sharing one real file',()=>{
  const f=fixture(),next=capture(f.c,f.r);expect(next).toEqual(f.c);expect(next).not.toBe(f.c);expect(next.draft.files[0]).not.toBe(f.c.draft.files[0]);
  expect(next.draft.context!.records).toHaveLength(3);expect(next.draft.files[0]!.contextId).toBe('legacy-unreferenced');
  expect(claim(f.q)).not.toBe(f.q);f.c.draft.files[0]!.name='changed';expect(next.draft.files[0]!.name).toBe('file.txt');
});
test('raw content preflight needs no fabricated enrolled document',()=>{
  const {document,...raw}=fixture().c.draft;expect(content(raw)).toEqual(raw);expect(content(raw)).not.toHaveProperty('document');
  expect(()=>content({...raw,document})).toThrow();
});
test('null context and nonzero context revision remain distinct from explicit empty source context',()=>{
  const f=fixture();f.c.draft.context=null;delete f.r.context;expect(capture(f.c,f.r).draft.contextRevision).toBe(9);
  f.c.draft.context={version:1,records:[]};expect(()=>capture(f.c,f.r)).toThrow();f.r.context={version:1,records:[]};expect(capture(f.c,f.r).draft.context).toEqual(f.r.context);
});
test('absent and stale raw order are captured exactly while canonical order appends unranked rows',()=>{
  const f=fixture();f.c.draft.attachmentOrder=null;f.c.draft.attachmentIds=[image.id,file.id];f.r.attachments.reverse();expect(capture(f.c,f.r).draft.attachmentOrder).toBeNull();
  f.c.draft.attachmentOrder=['no-longer-present'];expect(capture(f.c,f.r).draft.attachmentOrder).toEqual(['no-longer-present']);
});
test('already uploaded references retain same-environment original binding without local/upload aliasing',()=>{
  const f=fixture();f.c.draft.files[0]!.attachmentId='remote-id';f.c.draft.files[0]!.status='ready';Object.assign(f.r.attachments[0]!,{uploadId:'remote-id',uploadEnvironmentId:'env',status:'ready'});
  expect(capture(f.c,f.r).draft.files[0]!.attachmentId).toBe('remote-id');f.c.draft.context!.records[0]!.attachmentId='remote-id';expect(()=>capture(f.c,f.r)).toThrow();
});
for(const [path,value]of [
  ['version',1],['kind','new-task'],['draft.key','new-task:fake'],['draft.threadId','new:thread'],['draft.origin','https://send.test/'],
  ['draft.document.revision',true],['draft.document.revision',1.5],['draft.document.revision',Infinity],['draft.document.incarnation',''],
  ['draft.document.selection.end',100],['draft.document.selection.start',-1],['draft.contextRevision',NaN],['draft.contextRevision',-1],
  ['draft.files.0.status',['staged']],['draft.files.0.source','pasted-text'],['draft.files.0.environmentId','foreign'],['draft.files.0.draftKey','env:other'],
  ['draft.files.0.sizeBytes',false],['draft.files.0.videoWidth',true],['draft.files.0.attachmentId',{}],['draft.images.0.uploadId',42],
  ['draft.context.records.0.attachmentId',file.contextId],['draft.context.records.0.name','changed'],['draft.context.records.0.kind','image'],
  ['draft.attachmentIds',[file.id]],['draft.attachmentIds',[image.id,file.id]],['draft.attachmentOrder',[file.id,file.id]],
  ['draft.document.extra','discarded?'],['draft.extra','discarded?']
] as [string,unknown][])test(`capture refuses malformed exact field ${path}=${JSON.stringify(value)}`,()=>{const f=fixture();set(f.c,path,value);expect(()=>capture(f.c,f.r)).toThrow()});
test('mixed duplicate UUIDs cannot hide behind case, type or different metadata',()=>{
  const f=fixture();f.c.draft.images[0]!.id=file.id.toUpperCase();f.c.draft.attachmentIds=[file.id,file.id.toUpperCase()];expect(()=>capture(f.c)).toThrow();
});
for(const change of [
  (c:ThreadSendTransferCapture)=>{c.draft.images.length=2},
  (c:ThreadSendTransferCapture)=>{Object.assign(c.draft.images,{extra:'lost'})},
  (c:ThreadSendTransferCapture)=>{Object.defineProperty(c.draft,'text',{get(){throw new Error('getter executed')},enumerable:true})},
  (c:ThreadSendTransferCapture)=>{Object.assign(c.draft,{symbol:Symbol('bad')})},
  (c:ThreadSendTransferCapture)=>{Object.assign(c.draft,{bad:undefined})},
  (c:ThreadSendTransferCapture)=>{Object.assign(c.draft.images[0]!,{cycle:c})},
  (c:ThreadSendTransferCapture)=>{Object.setPrototypeOf(c.draft.images[0]!,{inherited:true})},
  (c:ThreadSendTransferCapture)=>{Object.defineProperty(c.draft.images[0]!,'hidden',{value:1})}
])test('raw JSON refuses erased, executable or cyclic structure',()=>{const f=fixture();change(f.c);expect(()=>capture(f.c)).toThrow()});
for(const [path,value]of [['text','different'],['creation',{}],['context.records',[]],['attachments.0.id',image.id],['attachments.0.status','uploading'],
  ['attachments.0.uploadEnvironmentId','foreign'],['attachments.0.contextId','synthetic'],['attachments.1.source',{changed:true}],['modelSelection',undefined],
  ['runtimeMode',undefined],['interactionMode',undefined],['attachments.0.extra','future'],['environmentId','foreign']] as [string,unknown][])
  test(`queued record is bound at ${path}`,()=>{const f=fixture();set(f.r,path,value);expect(()=>capture(f.c,f.r)).toThrow()});
test('raw text is preserved; only source trim is admitted into outgoing record',()=>{const f=fixture();expect(capture(f.c,f.r).draft.text).toBe('  raw send  ');f.r.text=f.c.draft.text;expect(()=>capture(f.c,f.r)).toThrow()});
test('terminal ordinary claim retains full owner and kind with no original content',()=>{
  const f=fixture();for(const state of ['completed','released'] as const){const q={...f.q,state,record:null,capture:null};expect(claim(q)).toEqual(q);expect(()=>claim({...q,origin:undefined})).toThrow();expect(()=>claim({...q,kind:undefined})).toThrow();expect(()=>claim({...q,record:f.r})).toThrow()}
});
for(const [path,value]of [['state',['queued']],['kind','new-task'],['fingerprint','bad'],['transferId','other'],['capture',null],['record',null],['origin','https://other.test'],['draftKey','env:other'],['commandId','other']] as [string,unknown][])
  test(`claim refuses contradictory ${path}`,()=>{const f=fixture();set(f.q,path,value);expect(()=>claim(f.q)).toThrow()});
test('clear and preserve are explicit detached projections bound to original queued claim',()=>{
  const f=fixture();expect(completion(f.cleared,f.q)).toEqual(f.cleared);expect(completion(f.preserved,f.q)).toEqual(f.preserved);
  expect(completion(f.preserved,f.q).after.files[0]).not.toBe(f.preserved.after.files[0]);
  expect(()=>completion(f.preserved,{...f.q,state:'prepared'})).toThrow();
});
test('preservation allows new enrolled incarnation; never permits it to clear original draft',()=>{
  const f=fixture();Object.assign(f.preserved.after.document,{incarnation:'new-doc',revision:0});expect(completion(f.preserved,f.q).after.document.revision).toBe(0);
  f.cleared.after.document.incarnation='new-doc';expect(()=>completion(f.cleared,f.q)).toThrow();
});
test('same revision may change selection/context but cannot claim changed durable text',()=>{
  const f=fixture();f.preserved.after.document.revision=7;expect(()=>completion(f.preserved,f.q)).toThrow();
  f.preserved.after.text=f.c.draft.text;f.preserved.after.context!.text=f.c.draft.text;expect(completion(f.preserved,f.q).after.document.revision).toBe(7);
});
test('capture exhaustion may preserve but cannot clear with unsafe next revision',()=>{
  const f=fixture();f.c.draft.document.revision=Number.MAX_SAFE_INTEGER;f.preserved.before.revision=Number.MAX_SAFE_INTEGER;f.preserved.after.document.revision=Number.MAX_SAFE_INTEGER;
  f.preserved.after.text=f.c.draft.text;f.preserved.after.context!.text=f.c.draft.text;expect(completion(f.preserved,f.q)).toEqual(f.preserved);
  f.cleared.before.revision=Number.MAX_SAFE_INTEGER;expect(()=>completion(f.cleared,f.q)).toThrow();
});
for(const [path,value]of [['fingerprint','b'.repeat(64)],['before.revision',6],['before.incarnation','other'],['after.document',null],['after.document.blocked',true],
  ['after.document.origin','https://other.test'],['after.document.draftKey','env:other'],['after.document.revision',6],['after.document.selection.end',100],
  ['after.context.text','foreign'],['after.context.revision',false],['after.context.key','env:other'],['after.context.context',null],['after.context.extra',1],
  ['after.document.value','duplicated'],['disposition',['preserved']]] as [string,unknown][])
  test(`completion refuses invalid saved projection ${path}`,()=>{const f=fixture();set(f.preserved,path,value);expect(()=>completion(f.preserved,f.q)).toThrow()});
for(const [path,value]of [['after.document.revision',9],['after.document.selection',null],['after.text','keep'],['after.images',[image]],['after.attachmentOrder',[]],
  ['after.context',{origin:scope.origin,environmentId:'env',key:scope.key,revision:10,text:'',context:{version:1,records:[]}}]] as [string,unknown][])
  test(`clear cannot preserve partial projection at ${path}`,()=>{const f=fixture();set(f.cleared,path,value);expect(()=>completion(f.cleared,f.q)).toThrow()});
test('preserved recovery overflow is validated completely without applying new Send limits',()=>{
  const f=fixture();f.preserved.after.images=Array.from({length:101},(_,i)=>({...image,id:id(i+10)}));f.preserved.after.files=[];
  f.preserved.after.attachmentIds=f.preserved.after.images.map(x=>String(x.id));f.preserved.after.attachmentOrder=clone(f.preserved.after.attachmentIds);f.preserved.after.context=null;
  expect(completion(f.preserved,f.q).after.images).toHaveLength(101);
  Object.assign(f.c.draft,{images:f.preserved.after.images,files:[],attachmentIds:f.preserved.after.attachmentIds,attachmentOrder:null,context:null});expect(()=>capture(f.c)).toThrow();
  f.preserved.after.images[100]!.sizeBytes=true;expect(()=>completion(f.preserved,f.q)).toThrow();
});

test('preserve records current private staged files without granting canonical upload authority',()=>{
  const f=fixture();f.preserved.after.files=[{...file,id:'folded-text',source:'pasted-text'}];f.preserved.after.images=[];
  f.preserved.after.attachmentOrder=['folded-text'];f.preserved.after.attachmentIds=['folded-text'];f.preserved.after.context=null;
  expect(completion(f.preserved,f.q).after.files[0]!.source).toBe('pasted-text');
});

test('accessor rejection does not execute the accessor while validating raw JSON',()=>{
  const f=fixture();let called=false;Object.defineProperty(f.c.draft,'text',{enumerable:true,get(){called=true;return 'raw send'}});
  expect(()=>capture(f.c)).toThrow();expect(called).toBe(false);
});

test('source context byte budget counts records JSON rather than envelope overhead',()=>{
  const f=fixture(),record:Obj={version:1,contextId:'boundary',kind:'skill',label:'Skill',name:'skill',metadata:''};
  record.metadata='x'.repeat(16_000_000-JSON.stringify([record]).length);f.c.draft.context={version:1,records:[record]};
  expect(JSON.stringify(f.c.draft.context.records).length).toBe(16_000_000);
  expect(capture(f.c).draft.context!.records).toHaveLength(1);
  record.metadata=String(record.metadata)+'x';expect(()=>capture(f.c)).toThrow();
});

// Compare against the established serializer, not a second implementation of its normalization.
test('completion order matches actual persisted order while capture retains raw live order',()=>{
  const f=fixture(),client=new T3Client();
  Object.assign(client.local,{snapshotDrafts:{[scope.key]:clone(f.c.draft.images)},composerFiles:clone(f.c.draft.files),
    mobileAttachmentOrder:{[scope.key]:clone(f.c.draft.attachmentOrder)}});
  const saved=mobileDraftAttachmentOrdersPersisted(client)[scope.key]??null;
  expect(saved).toEqual(f.c.draft.attachmentIds);expect(capture(f.c).draft.attachmentOrder).toEqual(['stale',file.id]);
  f.preserved.after.attachmentOrder=saved;expect(completion(f.preserved,f.q).after.attachmentOrder).toEqual(saved);
  for(const bad of [null,[],['stale',file.id],[file.id]]){
    f.preserved.after.attachmentOrder=bad;expect(()=>completion(f.preserved,f.q)).toThrow();
  }
  Object.assign(client.local,{snapshotDrafts:{},composerFiles:[],mobileAttachmentOrder:{[scope.key]:[]}});
  const empty=mobileDraftAttachmentOrdersPersisted(client)[scope.key]??null;expect(empty).toBeNull();
  f.cleared.after.attachmentOrder=empty;expect(completion(f.cleared,f.q).after.attachmentOrder).toBeNull();
  const preservedEmpty={...clone(f.cleared),disposition:'preserved' as const};
  expect(completion(preservedEmpty,f.q).after.attachmentOrder).toBeNull();
  preservedEmpty.after.attachmentOrder=[];expect(()=>completion(preservedEmpty,f.q)).toThrow();
});
