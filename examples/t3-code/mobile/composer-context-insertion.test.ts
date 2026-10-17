// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobileComposerInsertionRange as range,mobileComposerAfterSelection as after,mobileComposerCountAttachmentsAfterSelection as count,
 mobileComposerInsertContext as rich,mobileComposerInsertText as plain,type MobileComposerInsertionDraft} from './composer-context-insertion';
import type {Obj} from './shared/domain';
type File={id:string;type:'file'|'image';name:string;uploadedAttachmentId?:string};
const file=(id:string,type:'file'|'image'='file'):File=>({id,type,name:id,uploadedAttachmentId:`upload-${id}`});
const link=(id:string,kind='file')=>`[${id}](t3-context://v1/${kind}/${id})`;
const record=(id:string,kind='file'):Obj=>({version:1,kind,contextId:id,label:id,attachmentId:id,name:id,mimeType:'text/plain',sizeBytes:1});
const draft=(text='',records:Obj[]=[],attachments:File[]=[]):MobileComposerInsertionDraft<File>=>({text,context:records.length?{version:1,records}:undefined,attachments});
const content=(text:string,records:Obj[]=[],attachments:File[]=[])=>({text,context:{version:1 as const,records},attachments});
const target=(text:string,start=0,end=start)=>({text,start,end});
test('captured UTF16 range is clamped, caret motion is irrelevant, changed text appends and text ABA restores captured range',()=>{
 const text='😀 old suffix',capture=target(text,3,6);expect(range(text,capture)).toEqual({start:3,end:6});expect(range('changed',capture)).toEqual({start:7,end:7});expect(range(text,capture)).toEqual({start:3,end:6});
 expect(range(text,target(text,-9,999))).toEqual({start:0,end:text.length});expect(range(text,target(text,8,2))).toEqual({start:8,end:8});expect(range(text)).toEqual({start:text.length,end:text.length});
 expect(plain(draft(text),'new',capture).draft.text).toBe('😀 new suffix');
});
test('retained selection counting inserts one literal space and keeps all images/non-context files',()=>{
 const f=file('bound'),i=file('image','image'),free=file('free'),text=link('bound')+' '+link('image','image'),d=draft(text,[record('bound'),record('image','image')],[f,i,free]);
 const capture=target(text,0,text.length),result=after(d,capture);expect(result.text).toBe(' ');expect(result.context).toBeUndefined();expect(result.attachments).toEqual([i,free]);expect(count(d,capture)).toBe(2);expect(d.attachments).toEqual([f,i,free]);
});
test('rich separators use source whitespace boundaries, including empty rich text',()=>{
 for(const [text,value,at,expected]of [['ab','x',1,'a x b'],['a b','x',2,'a x b'],['a\nb','x',2,'a\nx b'],['ab',' x ',1,'a x b'],['','x',0,'x '],['ab','',1,'a  b']] as const){
  const r=rich(draft(text),content(value),target(text,at))!;expect(r.draft.text).toBe(expected);expect(r.selection.start).toBe(r.selection.end);
 }
 expect(rich(draft('ab'),content('x'),target('ab',1))?.selection).toEqual({start:4,end:4});
});
test('plain fallback has no rich separators and removes only context-bound files',()=>{
 const f=file('bound'),image=file('img','image'),free=file('free'),text='a'+link('bound')+'b';const d=draft(text,[record('bound')],[f,image,free]);
 const r=plain(d,'Z',target(text,1,text.length-1));expect(r.draft.text).toBe('aZb');expect(r.selection).toEqual({start:2,end:2});expect(r.removed).toEqual([f]);expect(r.draft.attachments).toEqual([image,free]);expect(r.draft.attachments[0]).toBe(image);
});
test('rich reference merge replaces matching IDs, prunes old records, and keeps annotation screenshot dependencies',()=>{
 const screenshot={...record('shot','image')},annotation={version:1,kind:'preview-annotation',contextId:'annotation',label:'note',screenshotContextId:'shot'};
 const r=rich(draft('',[record('gone')]),content(link('annotation','preview-annotation'),[annotation,screenshot]))!;
 expect(r.draft.context?.records).toEqual([annotation,screenshot]);expect(r.draft.text.endsWith(' ')).toBe(true);
 const old=record('same'),replacement={...record('same'),name:'replacement'},d=draft(link('same'),[old]);
 expect(rich(d,content(link('same'),[replacement]))?.draft.context?.records).toEqual([replacement]);
});
test('rich insertion rejects whole nonempty batch above live attachment cap after replacement',()=>{
 const attachments=Array.from({length:100},(_,i)=>file(`i${i}`,'image'));expect(rich(draft('',[],attachments),content('x',[],[file('new')]))).toBeNull();
 expect(rich(draft('',[],attachments),content('text only'))).not.toBeNull();
 const old=file('old'),text=link('old'),replace=draft(text,[record('old')],[old,...attachments.slice(0,99)]),incoming=file('new');
 const r=rich(replace,content(link('new'),[record('new')],[incoming]),target(text,0,text.length))!;
 expect(r.draft.attachments.length).toBe(100);expect(r.removed).toEqual([old]);expect(r.draft.attachments.at(-1)).toBe(incoming);
 expect(rich(replace,content(link('new'),[record('new')],[incoming,file('overflow')]),target(text,0,text.length))).toBeNull();
});
test('context cap applies after merge/pruning and text-only rich insertion does not trim recovery attachments',()=>{
 const records=Array.from({length:201},(_,i)=>record(`r${i}`)),text=records.map(r=>link(String(r.contextId))).join(' ');
 expect(rich(draft(),content(text,records))).toBeNull();expect(rich(draft(),content(records.slice(0,200).map(r=>link(String(r.contextId))).join(' '),records))?.draft.context?.records.length).toBe(200);
 const attachments=Array.from({length:101},(_,i)=>file(`image${i}`,'image'));expect(rich(draft('',[],attachments),content('text'))?.draft.attachments.length).toBe(101);
});
test('source uses previous context bindings, not every new record, when retaining imported file metadata',()=>{
 const orphan=file('orphan'),d=draft(),r=rich(d,content('no reference',[record('orphan')],[orphan]))!;
 expect(r.draft.context).toBeUndefined();expect(r.draft.attachments).toEqual([orphan]);expect(r.removed).toEqual([]);
});
test('inputs and attachment metadata are immutable; projection returns cleanup facts only',()=>{
 const attachment=Object.freeze(file('old')),d=Object.freeze(draft(link('old'),[Object.freeze(record('old'))],[attachment])),input=content('plain');Object.freeze(d.attachments);Object.freeze(d.context!.records);Object.freeze(input.attachments);
 const before=JSON.stringify(d),r=rich(d,input,target(d.text,0,d.text.length))!;expect(JSON.stringify(d)).toBe(before);expect(r.removed[0]).toBe(attachment);expect(r.draft.text).toBe('plain ');
});

test('plain empty readback drops empty context unless source settings or project retain its row',()=>{
 const d:MobileComposerInsertionDraft<File>={text:'erase',context:{version:1,records:[]},attachments:[]},captured=target(d.text,0,d.text.length);
 expect(plain(d,'',captured).draft).toEqual({text:'',attachments:[],context:undefined});
 for(const property of ['modelSelection','runtimeMode','interactionMode','workspaceSelection','project']){
  const retained={...d,[property]:property.endsWith('Mode')?'default':{opaque:true}};
  expect(plain(retained,'',captured).draft.context).toEqual({version:1,records:[]});expect(plain(retained,'',captured).draft[property as keyof typeof retained]).toEqual(retained[property as keyof typeof retained]);
 }
 expect(rich({...d,text:''},content('',[]))?.draft.context).toEqual({version:1,records:[]});
});
