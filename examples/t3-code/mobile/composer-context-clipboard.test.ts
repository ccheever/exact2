// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { decodeMobileComposerContextRecord as decode, decodeMobileComposerClipboardValue as fragmentValue } from './composer-context-schema';
import { decodeMobileComposerContextFragment as fragment, decodeMobileComposerContextHtml as html,
  encodeMobileComposerContextHtml as encodeHtml, importMobileComposerContextClipboard as importContext,
  reidentifyMobileComposerContext as reidentify, type MobileComposerClipboardImportPorts } from './composer-context-clipboard';
import type { Obj } from './shared/domain';
const base = { version: 1, contextId: 'one', label: 'One' };
const image = { ...base, kind: 'image', attachmentId: 'asset', name: 'a.png', mimeType: 'image/png', sizeBytes: 4 };
const mention = { ...base, kind: 'mention', path: 'src/a.ts' };
const element = { pageUrl: '', pageTitle: null, tagName: 'div', selector: null, htmlPreview: '', componentName: null, source: null, styles: '' };
const preview = { ...base, kind: 'preview-annotation', annotationId: '', pageUrl: '', pageTitle: null, comment: '', targetSummary: '', styleChanges: [] };
const review = { ...base, kind: 'review-comment', sectionId: 'a', sectionTitle: '', filePath: 'a.ts', startIndex: 0, endIndex: 0, rangeLabel: '', text: '', diff: '' };
const wrap = (records: Obj[]) => ({ version: 1, source: { environmentId: 'source' }, records });
const link = (id = 'one', kind = 'mention') => `[${id}](t3-context://v1/${kind}/${id})`;
test('all known kinds decode required fields and discard excess properties', () => {
  const records = [image, { ...image, kind: 'file' }, mention, { ...base, kind: 'skill', name: 'build' },
    { ...base, kind: 'thread', environmentId: 'env', threadId: 'thread', title: '' },
    { ...base, kind: 'terminal', terminalId: 'a', terminalLabel: 'A', lineStart: 0, lineEnd: 1, text: '' },
    { ...base, kind: 'element', ...element }, preview, review];
  for (const record of records) expect(decode({ ...record, extra: 'drop' })).toEqual(record);
  expect(decode({ ...mention, contextId: ' one ', path: ' src/a.ts ' })).toEqual(mention);
  expect(decode({ ...mention, label: ' One ' })?.label).toBe(' One ');
});
test('known records cannot escape their schema as future records', () => {
  for (const patch of [{ path: '' }, { path: ' ' }, { contextId: '../one' }, { label: 'x'.repeat(201) },
    { kind: ' mention ' }, { version: 2 }]) expect(decode({ ...mention, ...patch, payload: {} })).toBeNull();
  expect(decode({ ...image, sizeBytes: Number.MAX_SAFE_INTEGER + 1 })).toBeNull();
  expect(decode({ ...image, sizeBytes: 0.5 })).toBeNull();
  expect(decode({ ...review, startIndex: 2, endIndex: 1 })).toBeNull();
  expect(decode({ ...base, kind: 'element', ...element, source: {} })).toBeNull();
});
test('optionalKey rejects present undefined while source optional allows it', () => {
  expect(decode({ ...preview, screenshotContextId: undefined })).toBeNull();
  expect(decode({ ...review, fenceLanguage: undefined })).toBeNull();
  expect(fragmentValue({ ...wrap([]), source: { environmentId: ' env ', threadId: undefined } })).toEqual({
    version: 1, source: { environmentId: 'env', threadId: undefined }, records: [],
  });
  expect(decode({ ...preview, elements: [{ ...element, unknown: 1 }] })?.elements).toEqual([element]);
  expect(decode({ ...preview, elements: Array(51).fill(element) })).toBeNull();
});
test('future records preserve bounded JSON and reject values the wire cannot carry', () => {
  const future = { ...base, kind: ' future-v2 ', payload: { nested: [null, 1, 'x', true] } };
  expect(decode(future)).toEqual({ ...future, kind: 'future-v2' });
  const cycle: Record<string, unknown> = {}; cycle.self = cycle;
  for (const payload of [undefined, NaN, Infinity, new Date(), { a: undefined }, [undefined], cycle, 1n, 'x'.repeat(64000)])
    expect(decode({ ...future, payload })).toBeNull();
  expect(decode({ ...future, payload: 'x'.repeat(63998) })).not.toBeNull();
});
test('fragment caps raw records before dropping malformed values and retains duplicate IDs', () => {
  expect(fragment(JSON.stringify(wrap([mention, { ...mention, path: '' }, image])))?.records).toEqual([mention, image]);
  expect(fragmentValue(wrap(Array(201).fill({})))).toBeNull();
  expect(fragmentValue(wrap([mention, mention]))?.records).toHaveLength(2);
  expect(fragment('x'.repeat(16000001))).toBeNull();
  expect(fragment('{')).toBeNull();
});
test('HTML roundtrip, raw limits and malformed URI fallback match the clipboard flavor', () => {
  const raw = JSON.stringify(wrap([mention]));
  expect(html(encodeHtml('<>&😀', raw))).toEqual(wrap([mention]));
  expect(encodeHtml('<>&', raw)).toContain('&lt;&gt;&amp;');
  expect(encodeHtml('ignored', raw, '<b>same</b>')).toContain('><b>same</b></div>');
  expect(html("<pre data-t3-context-fragment='%broken'>x</pre>")).toBeNull();
});
test('reidentification rewrites linked screenshots, duplicates and missing references exactly once per record', () => {
  let count = 0;
  const records = [{ ...preview, contextId: 'annotation', screenshotContextId: 'photo' }, { ...image, contextId: 'photo' }, mention, mention];
  const result = reidentify(link('annotation', 'preview-annotation') + link('missing') + link(), records, () => `fresh${++count}`);
  expect(count).toBe(4);
  expect(result.context.records.map(r => r.contextId)).toEqual(['fresh1', 'fresh2', 'fresh4', 'fresh4']);
  expect(result.context.records[0]?.screenshotContextId).toBe('fresh2');
  expect(result.text).toContain('/mention/missing)');
  expect(result.text).toContain('/mention/fresh4)');
  for (const id of ['', ' one ', '../one', 'x'.repeat(129)]) expect(() => reidentify('', [mention], () => id)).toThrow();
});
function ports() {
  let sequence = 0;
  const calls: string[] = [], discarded: string[] = [];
  const p: MobileComposerClipboardImportPorts<{ id: string }> = { createId: () => `id${++sequence}`, assertCurrent() {},
    async importAttachment(record, environment) { calls.push(`${environment}:${record.attachmentId}`); return { id: `file${sequence}` }; },
    async discardAttachment(file) { discarded.push(file.id); } };
  return { p, calls, discarded };
}
test('valid raw fragment wins over HTML; malformed raw uses HTML; plain text remains unhandled', async () => {
  const { p, calls } = ports(), rich = encodeHtml('', JSON.stringify(wrap([image])));
  const input = { text: link('one', 'image'), fragment: JSON.stringify(wrap([])), html: rich };
  expect((await importContext(input, 0, p))?.context.records).toEqual([]);
  expect(calls).toEqual([]);
  expect((await importContext({ ...input, fragment: 'broken' }, 0, p))?.attachments).toHaveLength(1);
  expect(calls).toEqual(['source:asset']);
  expect(await importContext({ ...input, fragment: '', html: '' }, 0, p)).toBeNull();
});
test('serial partial attachment failures keep unavailable references but omit failed records', async () => {
  const { p, calls } = ports(), files = [image, { ...image, contextId: 'two', attachmentId: 'bad' }, { ...mention, contextId: 'three' }];
  p.importAttachment = async record => { calls.push(String(record.attachmentId)); if (record.attachmentId === 'bad') throw Error('download'); return { id: 'copy' }; };
  const result = await importContext({ text: link('one', 'image') + link('two', 'image') + link('three'), fragment: JSON.stringify(wrap(files)), html: '' }, 0, p);
  expect(result?.failures).toEqual(['a.png']); expect(result?.attachments).toEqual([{ id: 'copy' }]);
  expect(result?.context.records.map(r => r.contextId)).toEqual(['id1', 'id3']);
  expect(result?.text).toContain('/image/id2)'); expect(calls).toEqual(['asset', 'bad']);
});
test('attachment capacity is a per-file failure; context capacity refuses before any IO or minted IDs', async () => {
  const { p, calls } = ports(), input = { text: link('one', 'image'), fragment: JSON.stringify(wrap([image])), html: '' };
  expect((await importContext(input, 100, p))?.failures).toEqual(['a.png']); expect(calls).toEqual([]);
  await expect(importContext(input, 0, p, 200)).rejects.toThrow('Remove some context');
});
test('letGo from import propagates unchanged and performs no cleanup or later calls', async () => {
  const { p, discarded } = ports(), gone = { name: 'FetchError', kind: 'Aborted' };
  let count = 0; p.importAttachment = async () => { if (++count === 2) throw gone; return { id: 'first' }; };
  const files = [image, { ...image, contextId: 'two' }], input = { text: link('one', 'image') + link('two', 'image'), fragment: JSON.stringify(wrap(files)), html: '' };
  try { await importContext(input, 0, p); throw Error('resolved'); } catch (error) { expect(error).toBe(gone); }
  expect(discarded).toEqual([]); expect(count).toBe(2);
});
test('ordinary cancellation after an await discards the returned owned file; owner loss retains journal ownership', async () => {
  const input = { text: link('one', 'image'), fragment: JSON.stringify(wrap([image])), html: '' };
  for (const lost of [false, true]) {
    const { p, discarded } = ports(), error = lost ? { name: 'FetchError', kind: 'Aborted' } : Error('cancelled');
    let complete = false; p.importAttachment = async () => { complete = true; return { id: 'made' }; };
    p.assertCurrent = () => { if (complete) throw error; };
    try { await importContext(input, 0, p); throw Error('resolved'); } catch (actual) { expect(actual).toBe(error); }
    expect(discarded).toEqual(lost ? [] : ['made']);
  }
});
