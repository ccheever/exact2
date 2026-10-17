// Source365aa87982 append/remove behavioral fixtures; no native intake authority asserted.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerPickerAppend as append,mobileComposerPickerRemove as remove,type ComposerPickerDraft} from './composer-picker-plan';
import type {OrdinaryInventoryAttachment as Attachment} from './composer-attachment-publication';
import {formatComposerContextReference as link,collectComposerContextReferences} from './composer-editor-document';
import type {Obj} from './shared/domain';
const file=(id:string,name=id+'.txt',mimeType='text/plain'):Attachment=>({type:'file',id,file:{id,contextId:id,
  draftKey:'env:thread',environmentId:'env',name,mimeType,sizeBytes:7,source:'attached',attachmentId:'',status:'staged'}});
const image=(id:string):Attachment=>({type:'image',id,image:{id,name:id+'.png',mimeType:'image/png',sizeBytes:9,opaque:{keep:true}}});
const record=(id:string,kind='file',contextId=id):Obj=>kind==='thread'
  ?{version:1,kind,contextId,label:contextId,environmentId:'env',threadId:id,title:id}
  :{version:1,kind,contextId,attachmentId:id,label:contextId,name:id+'.txt',mimeType:'text/plain',sizeBytes:7};
const ref=(id:string,kind='file')=>link({contextId:id,kind,label:id});
const draft=(text='',attachments:Attachment[]=[],records:Obj[]=[]):ComposerPickerDraft=>({text,attachments,context:records.length?{version:1,records}:undefined});
const capture=(text:string,start=text.length,end=start)=>({text,start,end});

test('every accepted incoming object gets a reference in mixed source order and exact metadata survives',()=>{
  const incoming=[image('photo'),file('doc'),file('movie','movie.mp4','video/mp4')];
  const result=append(draft('intro'),incoming,capture('intro'));
  expect(result.applied).toBe(true);expect(result.accepted).toEqual(incoming);expect(result.rejected).toEqual([]);
  expect(result.draft.attachments).toEqual(incoming);expect(result.draft.attachments[0]).toBe(incoming[0]);
  expect(collectComposerContextReferences(result.draft.text).map(r=>[r.kind,r.contextId])).toEqual([['image','photo'],['file','doc'],['file','movie']]);
  expect(result.draft.text.startsWith('intro !')).toBe(true);expect(result.draft.text.endsWith(' ')).toBe(true);
  expect(result.selection).toEqual({start:result.draft.text.length,end:result.draft.text.length});
});
test('same text ABA uses captured UTF16 interior range, changed text appends',()=>{
  const before='😀 old tail',target=capture(before,3,6),incoming=[file('new')];
  const first=append(draft(before),incoming,target),changed=append(draft('changed'),incoming,target);
  expect(first.draft.text).toBe('😀 '+ref('new').replace('[new]','[new.txt]')+' tail');
  expect(changed.draft.text).toBe('changed '+ref('new').replace('[new]','[new.txt]')+' ');
  expect(append(draft(before),incoming,target)).toEqual(first);
});
test('range replacement frees only context-owned files, keeps images and free files, takes prefix',()=>{
  const old=file('old'),photo=image('photo'),free=file('free'),text=ref('old')+' '+ref('photo','image');
  const current=draft(text,[old,photo,free],[record('old'),record('photo','image')]),incoming=[file('a'),image('b'),file('c')];
  const result=append(current,incoming,capture(text,0,text.length),4);
  expect(result.accepted).toEqual(incoming.slice(0,2));expect(result.rejected).toEqual([incoming[2]]);
  expect(result.removed).toEqual([old]);expect(result.draft.attachments).toEqual([photo,free,incoming[0],incoming[1]]);
  expect(current.attachments).toEqual([old,photo,free]);expect(result.draft.context?.records.map(r=>r.contextId)).toEqual(['a','b']);
});
test('hard attachment100 and context200 capacities use latest post-range state',()=>{
  const images=Array.from({length:99},(_,i)=>image('i'+i)),incoming=[file('first'),file('second')];
  expect(append(draft('',images),incoming,capture(''),1000).accepted).toEqual([incoming[0]]);
  const records=Array.from({length:200},(_,i)=>record('r'+i,'thread'));
  const text=records.map(r=>ref(String(r.contextId),'thread')).join(' ');
  const current=draft(text,[],records),none=append(current,incoming,capture(text));
  expect(none.applied).toBe(false);expect(none.draft).toBe(current);expect(none.selection).toBeNull();expect(none.rejected).toEqual(incoming);
  const firstRef=ref('r0','thread'),replace=append(current,incoming,capture(text,0,firstRef.length));
  expect(replace.accepted).toEqual([incoming[0]]);expect(replace.draft.context?.records.length).toBe(200);
});
test('zero acceptance never applies the hypothetical selection deletion or removes rows',()=>{
  const old=file('old'),text=ref('old'),current=draft(text,[old],[record('old')]),incoming=[file('new')];
  const result=append(current,incoming,capture(text,0,text.length),0);
  expect(result.draft).toBe(current);expect(result.removed).toEqual([]);expect(result.rejected).toEqual(incoming);expect(result.selection).toBeNull();
  expect(append(current,[],capture(text,0,text.length)).draft).toBe(current);
});
test('recovery overflow is retained without granting new picker capacity',()=>{
  const images=Array.from({length:101},(_,i)=>image('i'+i)),current=draft('',images),incoming=[file('new')];
  const result=append(current,incoming,capture(''));
  expect(result.draft).toBe(current);expect(result.draft.attachments.length).toBe(101);
  expect(result.rejected).toEqual(incoming);expect(result.removed).toEqual([]);
});
test('Files images preserve file bytes while source semantic MIME classification stays exact',()=>{
  const cases=[['a.PNG','application/octet-stream','image'],['a.jpg',' IMAGE/JPEG ;charset=x','image'],
    ['a.avif','image/avif','file'],['a.png','application/pdf','file'],['a.webp','binary/octet-stream','image'],['a.png','text/plain','file']];
  for(const [name,mime,kind]of cases){const incoming=file('new',name,mime),result=append(draft(),[incoming],capture(''));
    expect(result.draft.attachments[0]).toBe(incoming);expect(result.draft.attachments[0].type).toBe('file');
    expect(result.draft.context?.records[0].kind).toBe(kind);expect(result.draft.context?.records[0].mimeType).toBe(mime);
  }
});
test('remove all references bound to actual ID, preserve whitespace and unrelated rows',()=>{
  const selected=file('actual'),other=file('other'),unreferenced=file('free');
  const text=' \n'+ref('one')+'  '+ref('two','image')+'\t'+ref('one')+' '+ref('other')+'\n';
  const current=draft(text,[selected,other,unreferenced],[record('actual','file','one'),record('actual','image','two'),record('other')]);
  const result=remove(current,'actual');
  expect(result.draft.text).toBe(' \n'+'  '+'\t'+' '+ref('other')+'\n');
  expect(result.draft.context?.records).toEqual([record('other')]);expect(result.draft.attachments).toEqual([other,unreferenced]);
  expect(result.removed).toEqual([selected]);expect(result.selection).toBeNull();expect(result.accepted).toEqual([]);
});
test('removal never assumes contextId equals attachmentId and preserves dependency records',()=>{
  const imageRow=image('shot'),selected=file('remove');
  const records=[record('shot','image','shot'),{version:1,kind:'preview-annotation',contextId:'note',label:'note',screenshotContextId:'shot'},record('remove')];
  const current=draft(ref('note','preview-annotation')+' '+ref('remove'),[imageRow,selected],records);
  const result=remove(current,'remove');expect(result.draft.context?.records).toEqual(records.slice(0,2));expect(result.draft.attachments).toEqual([imageRow]);
  expect(remove(result.draft,'unknown').draft).toBe(result.draft);
});
test('empty removal normalization mirrors source settings retention and leaves frozen inputs unchanged',()=>{
  const selected=file('only'),text=ref('only'),current=draft(text,[selected],[record('only')]);
  Object.freeze(selected);Object.freeze(current.attachments);Object.freeze(current.context!.records);Object.freeze(current);
  const saved=JSON.stringify(current),result=remove(current,'only');expect(result.draft).toEqual({text:'',attachments:[],context:undefined});expect(JSON.stringify(current)).toBe(saved);
  for(const key of ['modelSelection','runtimeMode','interactionMode','workspaceSelection','project']){
    const preserved=remove({...current,[key]:key.endsWith('Mode')?'default':{opaque:true}},'only');expect(preserved.draft[key as keyof ComposerPickerDraft]).toBeDefined();
  }
});
