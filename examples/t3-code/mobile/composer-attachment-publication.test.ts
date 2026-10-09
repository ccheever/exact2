// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerAttachmentInventoryRead as read,mobileComposerAttachmentPublicationPrepare as prepare,
  type OrdinaryAttachmentInventory,type OrdinaryAttachmentPublicationInput} from './composer-attachment-publication';
import type {DraftFile} from './shared/composer-editor-files';
import type {Obj} from './shared/domain';
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const target={environmentId:'env',threadId:'thread',draftKey:'env:thread'};
const file=(n:number,extra:Record<string,unknown>={}):DraftFile=>({id:id(n),contextId:`file_${n}`,draftKey:target.draftKey,environmentId:'env',
  name:`${n}.txt`,mimeType:'text/plain',sizeBytes:11,source:'attached',attachmentId:'',status:'staged',...extra});
const image=(n:number):Obj=>({id:id(n),name:`${n}.png`,mimeType:'image/png',sizeBytes:10});
const record=(n:number,extra:Record<string,unknown>={}):Obj=>({version:1,kind:'file',contextId:`file_${n}`,label:`${n}.txt`,
  attachmentId:id(n),name:`${n}.txt`,mimeType:'text/plain',sizeBytes:11,...extra});
const context=(...records:Obj[])=>({version:1,records});
const link=(n:number)=>`[${n}.txt](t3-context://v1/file/file_${n})`;
const clone=<T>(v:T):T=>structuredClone(v);
function fixture():OrdinaryAttachmentPublicationInput {
  return {target:clone(target),inventory:{snapshotDrafts:{[target.draftKey]:[image(1)],'other:thread':[image(11)]},
    composerFiles:[file(2),file(3),file(12,{draftKey:'other:thread',environmentId:'other',nested:{keep:['all',7]}})],
    mobileAttachmentOrder:{[target.draftKey]:[id(2),id(1),id(3)],'other:thread':[id(12),id(11),'foreign-stale']},
    snapshotReleases:['prior-image'],fileReleases:['prior-file']},previousContext:context(record(2)),nextContext:undefined,nextText:'terminal'};
}
function ready(input:OrdinaryAttachmentPublicationInput){const result=prepare(input);expect(result.ok).toBe(true);if(!result.ok)throw Error(result.reason);return result}
function frozen<T>(value:T):T {if(value&&typeof value==='object'){Object.freeze(value);for(const child of Object.values(value))frozen(child)}return value}

test('prunes only former context files, preserves mixed order/foreign metadata and queues canonical cleanup',()=>{
  const input=frozen(fixture()),before=clone(input),result=ready(input);
  expect(result.composerFiles.map(f=>f.id)).toEqual([id(3),id(12)]);
  expect(result.snapshotDrafts).toEqual(input.inventory.snapshotDrafts);
  expect(result.mobileAttachmentOrder).toEqual({[target.draftKey]:[id(1),id(3)],'other:thread':[id(12),id(11),'foreign-stale']});
  expect(result.fileReleases).toEqual(['prior-file',id(2)]);expect(result.snapshotReleases).toEqual(['prior-image']);
  expect(result.removedFileIds).toEqual([id(2)]);expect(result.composerFiles[1]).toEqual((input.inventory.composerFiles as DraftFile[])[2]);
  expect(input).toEqual(before);
});
test('read mixed list honors saved order then deterministic image/file tail without normalizing foreign order',()=>{
  const input=fixture();input.inventory={...input.inventory,mobileAttachmentOrder:{[target.draftKey]:['stale',id(3)],foreign:['x']}};
  const result=read(input.inventory,target);expect(result.ok).toBe(true);if(!result.ok)return;
  expect(result.attachments.map(a=>[a.type,a.id])).toEqual([['file',id(3)],['image',id(1)],['file',id(2)]]);
  (result.attachments.find(a=>a.type==='file') as {file:DraftFile}).file.name='changed';
  expect((input.inventory.composerFiles as DraftFile[])[1]!.name).toBe('3.txt');
});
test('retained context file keeps local and upload identity plus all descriptive fields',()=>{
  const input=fixture();(input.inventory.composerFiles as DraftFile[])[0]=file(2,{status:'ready',attachmentId:'remote-upload',videoWidth:23,videoHeight:17,metadata:{tag:'keep'}});
  input.nextContext=context(record(2));input.nextText=link(2);const result=ready(input);
  expect(result.composerFiles).toEqual(input.inventory.composerFiles);expect(result.fileReleases).toEqual(['prior-file']);expect(result.removedFileIds).toEqual([]);
});
test('images survive losing context and non-context files survive even without a text reference',()=>{
  const input=fixture();input.previousContext=context(record(1,{kind:'image',name:'1.png',mimeType:'image/png',sizeBytes:10}));
  const result=ready(input);expect(result.snapshotDrafts[target.draftKey]).toEqual([image(1)]);
  expect(result.composerFiles).toEqual(input.inventory.composerFiles);expect(result.removedFileIds).toEqual([]);
});
test('canonical shared local ID in another named draft is preserved and guarded release remains only an obligation',()=>{
  const input=fixture();(input.inventory.composerFiles as DraftFile[]).push(file(2,{draftKey:'elsewhere:thread',environmentId:'elsewhere'}));
  const result=ready(input);expect(result.composerFiles.some(f=>f.id===id(2)&&f.draftKey==='elsewhere:thread')).toBe(true);
  expect(result.fileReleases).toContain(id(2));
});
test('existing release queue order and duplicate receipts survive; new removal is queued only once',()=>{
  const input=fixture();input.inventory={...input.inventory,fileReleases:['prior-file',id(2),'prior-file']};
  expect(ready(input).fileReleases).toEqual(['prior-file',id(2),'prior-file']);
});
test('multiple source records for one file can all disappear with one cleanup; repeated same reference can survive',()=>{
  const input=fixture();input.previousContext=context(record(2),record(2,{contextId:'alternate'}));
  expect(ready(input).removedFileIds).toEqual([id(2)]);
  input.nextContext=context(record(2));input.nextText=`${link(2)} ${link(2)}`;
  expect(ready(input).removedFileIds).toEqual([]);
});
test('alternate context ID bound to the same local file is refused by legacy uploader compatibility',()=>{
  const input=fixture();input.nextContext=context(record(2,{contextId:'alternate'}));input.nextText='[alt](t3-context://v1/file/alternate)';
  expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('unresolved historical context never restores an attachment or grants cleanup authority',()=>{
  const input=fixture();input.nextContext=context(record(99));input.nextText=link(99);
  expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('legacy file reference without canonical context uses actual row binding; wrong local-vs-upload ID refuses',()=>{
  const input=fixture();input.previousContext=undefined;input.nextText=link(2);expect(ready(input).removedFileIds).toEqual([]);
  input.nextContext=context(record(2,{attachmentId:'uploaded_id'}));expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('legacy uploader must recognize every source file reference',()=>{
  const input=fixture();input.nextContext=context(record(2));input.nextText='[escaped \\] label](t3-context://v1/file/file_2)';
  expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('matching context ID cannot point to an image instead of a file',()=>{
  const input=fixture();input.nextContext=context(record(1));input.nextText=link(1);
  expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('ready outputs are fully detached, including foreign metadata and queue arrays',()=>{
  const input=fixture(),before=clone(input),result=ready(input);
  result.snapshotDrafts['other:thread']![0]!.name='changed';(result.composerFiles[1] as unknown as {nested:{keep:unknown[]}}).nested.keep.push('changed');
  result.mobileAttachmentOrder['other:thread']!.push('changed');result.fileReleases.push('changed');expect(input).toEqual(before);
});
test('absent optional files/order produces detached empty replacements; missing required inventory/queues refuses',()=>{
  const raw:OrdinaryAttachmentInventory={snapshotDrafts:{},composerFiles:undefined,mobileAttachmentOrder:undefined,snapshotReleases:[],fileReleases:[]};
  const result=ready({inventory:raw,target,previousContext:undefined,nextContext:undefined,nextText:''});
  expect(result.composerFiles).toEqual([]);expect(result.mobileAttachmentOrder).toEqual({[target.draftKey]:[]});
  for(const key of ['snapshotDrafts','snapshotReleases','fileReleases'] as const)expect(read({...raw,[key]:undefined},target)).toEqual({ok:false,reason:'invalid-inventory'});
});
const malformed:Record<string,(input:OrdinaryAttachmentPublicationInput)=>void>={
  'snapshot dictionary null':x=>{x.inventory={...x.inventory,snapshotDrafts:null}},
  'snapshot foreign slot primitive':x=>{(x.inventory.snapshotDrafts as Record<string,unknown>).poison=7},
  'snapshot foreign row null':x=>{(x.inventory.snapshotDrafts as Record<string,unknown>).poison=[null]},
  'snapshot missing ID':x=>{delete (x.inventory.snapshotDrafts as Record<string,Obj[]>)[target.draftKey]![0]!.id},
  'snapshot boolean size':x=>{(x.inventory.snapshotDrafts as Record<string,Obj[]>)[target.draftKey]![0]!.sizeBytes=true},
  'snapshot upload object':x=>{(x.inventory.snapshotDrafts as Record<string,Obj[]>)[target.draftKey]![0]!.uploadId={}},
  'file array primitive sibling':x=>{(x.inventory.composerFiles as unknown[]).push(7)},
  'file wrong named environment':x=>{(x.inventory.composerFiles as DraftFile[])[0]!.environmentId='other'},
  'file duplicate same named ID':x=>{(x.inventory.composerFiles as DraftFile[]).push(file(2,{contextId:'different'}))},
  'file duplicate named context':x=>{(x.inventory.composerFiles as DraftFile[]).push(file(99,{contextId:'file_2'}))},
  'file image ID collision':x=>{(x.inventory.composerFiles as DraftFile[]).push(file(1))},
  'file missing foreign metadata':x=>{delete (x.inventory.composerFiles as Obj[])[2]!.name},
  'file fractional size':x=>{(x.inventory.composerFiles as DraftFile[])[0]!.sizeBytes=1.5},
  'file unsafe size':x=>{(x.inventory.composerFiles as DraftFile[])[0]!.sizeBytes=Number.MAX_SAFE_INTEGER+1},
  'file NaN dimension':x=>{(x.inventory.composerFiles as DraftFile[])[0]!.videoWidth=NaN},
  'file infinity metadata':x=>{Object.assign((x.inventory.composerFiles as DraftFile[])[0]!,{extra:Infinity})},
  'file array status':x=>{Object.assign((x.inventory.composerFiles as DraftFile[])[0]!,{status:['staged']})},
  'foreign file array status':x=>{Object.assign((x.inventory.composerFiles as DraftFile[])[2]!,{status:['staged']})},
  'array hidden missing index':x=>{const rows=Array(1);Object.assign(rows,{extra:image(9)});(x.inventory.snapshotDrafts as Record<string,unknown>).bad=rows},
  'file ready without upload':x=>{(x.inventory.composerFiles as DraftFile[])[0]!.status='ready'},
  'order primitive':x=>{x.inventory={...x.inventory,mobileAttachmentOrder:[]}},
  'foreign order nonarray':x=>{(x.inventory.mobileAttachmentOrder as Record<string,unknown>).bad=3},
  'foreign order mixed':x=>{(x.inventory.mobileAttachmentOrder as Record<string,unknown>).bad=['id',null]},
  'order duplicates':x=>{(x.inventory.mobileAttachmentOrder as Record<string,unknown>).bad=['id','id']},
  'snapshot release primitive':x=>{x.inventory={...x.inventory,snapshotReleases:null}},
  'snapshot release invalid ID':x=>{x.inventory={...x.inventory,snapshotReleases:['']}},
  'file release mixed':x=>{x.inventory={...x.inventory,fileReleases:['valid',false]}},
  'file metadata undefined':x=>{Object.assign((x.inventory.composerFiles as DraftFile[])[0]!,{extra:undefined})},
  'file metadata cycle':x=>{const row=(x.inventory.composerFiles as DraftFile[])[0]!;Object.assign(row,{cycle:row})},
  'sparse snapshot array':x=>{(x.inventory.snapshotDrafts as Record<string,unknown>).bad=Array(2)},
};
for(const [name,mutate]of Object.entries(malformed))test(`fails closed for ${name}`,()=>{
  const input=fixture();mutate(input);expect(prepare(input)).toEqual({ok:false,reason:'invalid-inventory'});
});
for(const [name,changed]of Object.entries({newTask:{threadId:'new:p',draftKey:'env:new:p'},queued:{threadId:'t~queued-edit~x',draftKey:'env:t~queued-edit~x'},wrongKey:{draftKey:'foreign:t'},empty:{environmentId:''}}))test(`refuses ${name} target`,()=>{
  expect(read(fixture().inventory,{...target,...changed})).toEqual({ok:false,reason:'invalid-target'});
});
for(const kind of ['previousContext','nextContext'] as const)test(`validates all ${kind} records before pruning even unused`,()=>{
  const input=fixture();input[kind]=context(record(2),{version:1,kind:'file',contextId:'bad',label:'bad'});
  expect(prepare(input)).toEqual({ok:false,reason:'invalid-context'});
  input[kind]=context(record(2),record(2));expect(prepare(input)).toEqual({ok:false,reason:'invalid-context'});
});
for(const change of [{source:'pasted-text'},{source:'future-import'},{id:'not-canonical'}])test(`cannot queue unsupported removed bytes ${JSON.stringify(change)}`,()=>{
  const input=fixture();Object.assign((input.inventory.composerFiles as DraftFile[])[0]!,change);
  if(change.id)input.previousContext=context(record(2,{attachmentId:change.id}));
  expect(prepare(input)).toEqual({ok:false,reason:'unsupported'});
});
test('preparation uses latest image/file/order state rather than a stale producer capture',()=>{
  const old=fixture(),latest=clone(old);(latest.inventory.snapshotDrafts as Record<string,Obj[]>)[target.draftKey]!.push(image(5));
  (latest.inventory.composerFiles as DraftFile[]).push(file(6));
  (latest.inventory.mobileAttachmentOrder as Record<string,string[]>)[target.draftKey]!.unshift(id(6));
  const result=ready(latest);expect(result.mobileAttachmentOrder[target.draftKey]).toEqual([id(6),id(1),id(3),id(5)]);
  expect((old.inventory.composerFiles as DraftFile[]).map(f=>f.id)).toEqual([id(2),id(3),id(12)]);
});

test('valid oversized previous recovery context can prune before the live bound; oversized future refuses',()=>{
  const input=fixture();input.previousContext=context(...Array.from({length:205},(_,n)=>record(n+20)),record(2));
  expect(ready(input).removedFileIds).toEqual([id(2)]);
  input.nextContext=input.previousContext;expect(prepare(input)).toEqual({ok:false,reason:'invalid-context'});
});
