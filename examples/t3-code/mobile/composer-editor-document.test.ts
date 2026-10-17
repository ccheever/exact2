// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { collectComposerInlineTokens, collectComposerContextReferences, formatComposerContextReference,
  mobileComposerDocument, mobileComposerClipboard, mobileComposerFileIcon, contextChipPresentation,
  encodeComposerContextClipboardHtml, encodeComposerContextFragment } from './composer-editor-document';
const link=(id:string,label='File')=>formatComposerContextReference({kind:'mention',contextId:id,label});
test('confirmed Unicode tokens preserve source UTF16 ranges and trailing delimiter history',()=>{
  const text='😀 @"src/main.ts" $deploy ', tokens=collectComposerInlineTokens(text);
  expect(tokens.map(t=>[t.type,t.value,t.start,t.end])).toEqual([['mention','src/main.ts',3,17],['skill','deploy',18,25]]);
  expect(collectComposerInlineTokens(text.trimEnd())).toHaveLength(1);
  expect(collectComposerInlineTokens(text.trimEnd(),{preserveTrailingFrom:tokens})).toEqual(tokens);
  expect(collectComposerInlineTokens('$20 $20k $1e6 @scope/package ')).toEqual([]);
});
test('context occurrences retain duplicate identities and override overlapping ordinary tokens',()=>{
  const value=`😀 ${link('one','@src/a.ts')} ${link('one','again')}`;
  const refs=collectComposerContextReferences(value); expect(refs).toHaveLength(2); expect(refs[0]?.start).toBe(3);
  const projected=mobileComposerDocument({value,environmentId:'e',attachments:[],skills:[],iconUri:()=>null});
  expect(projected.tokens.map(t=>t.type)).toEqual(['context','context']);
  expect(projected.tokens.map(t=>t.label)).toEqual(['@src/a.ts · unavailable','again · unavailable']);
  expect(JSON.parse(projected.tokensJson)).toEqual(projected.tokens);
});
test('projection resolves real icon assets and display names without retaining mutable caller records',()=>{
  const value=`@"src/a.ts" $deploy ${link('one','Saved')}`;
  const record={version:1,kind:'mention',contextId:'one',label:'Saved',path:'other/b.ts'};
  const input={value,environmentId:'e',context:{version:1 as const,records:[record]},attachments:[],skills:[{name:'deploy',displayName:' Ship '}],iconUri:(path:string)=>`asset:${path}`};
  const output=mobileComposerDocument(input);
  expect(output.tokens.map(t=>[t.label,t.iconUri])).toEqual([['a.ts','asset:src/a.ts'],['Ship',null],['Saved','asset:other/b.ts']]);
  record.path='changed'; expect(output.tokens[2]?.iconUri).toBe('asset:other/b.ts');
  expect(JSON.parse(output.clipboardFragment).records[0].path).toBe('other/b.ts');
});
test('clipboard exports all records and rebinds uploads only within the source environment',()=>{
  const records=[{attachmentId:'local',contextId:'a'},{attachmentId:'foreign',contextId:'b'},{contextId:'unused',kind:'mention'}];
  const encoded=mobileComposerClipboard('e',{version:1,records},[{id:'local',uploadedAttachmentId:'remote',uploadEnvironmentId:'e'},
    {id:'foreign',uploadedAttachmentId:'other',uploadEnvironmentId:'x'}]);
  expect(JSON.parse(encoded!).records).toEqual([{attachmentId:'remote',contextId:'a'},{attachmentId:'foreign',contextId:'b'},records[2]]);
  expect(records[0]?.attachmentId).toBe('local'); expect(mobileComposerClipboard('',{version:1,records},[])).toBe('');
  expect(mobileComposerClipboard('e',undefined,[])).toBe('');
});
test('clipboard encoding preserves source size and escaped HTML boundaries',()=>{
  expect(encodeComposerContextClipboardHtml('<x>&','{"x":"é"}')).toBe('<pre data-t3-context-fragment="%7B%22x%22%3A%22%C3%A9%22%7D">&lt;x&gt;&amp;</pre>');
  expect(encodeComposerContextClipboardHtml('ignored','x','<b>ok</b>')).toBe('<div data-t3-context-fragment="x"><b>ok</b></div>');
  expect(encodeComposerContextFragment({version:1,source:{environmentId:'e'},records:[{text:'a'.repeat(16_000_000)}]})).toBeNull();
});
test('file chips and pull request chips follow source MIME and state palettes',()=>{
  expect(contextChipPresentation('file',{name:'clip.mp4',mimeType:'application/octet-stream'})).toEqual({accent:'#d06217',symbol:'play.rectangle'});
  expect(contextChipPresentation('file',{name:'picture.png',mimeType:'application/octet-stream'})).toEqual({accent:'#d55665',symbol:'photo'});
  expect(contextChipPresentation('file',{name:'picture.heic',mimeType:'image/heic'})).toEqual({accent:'#0090cd',symbol:'doc'});
  for(const [state,isDraft,color] of [['open',false,'#009f6e'],['open',true,'#7f8793'],['merged',false,'#8a70dd'],['closed',false,'#d55665']] as const)
    expect(contextChipPresentation('review-comment',{sectionId:'pull-request:7',pullRequest:{state,isDraft}})).toEqual({accent:color,symbol:'git-pull-request'});
});
test('file icon resolver uses pinned mobile names, multipart suffixes, video and position suffixes',()=>{
  expect(['src/a.ts:12:2','tsconfig.app.json','clip.mov','Dockerfile','archive.tar.gz','foo.unknown','folder\\README.md','src/a.ts/'].map(mobileComposerFileIcon))
    .toEqual(['typescript','typescript','video','docker','zip','default','markdown','typescript']);
});
