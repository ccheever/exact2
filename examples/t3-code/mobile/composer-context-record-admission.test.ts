// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect, test } from 'bun:test';
import { mobileContextRecordValid as valid } from './mobile-context-record';
import { decodeMobileComposerClipboardValue as decode } from './composer-context-schema';
import { mobileNewTaskContextProject as project } from './mobile-new-task-context';
import type { Obj } from './shared/domain';
const base = { version: 1, contextId: 'one', label: 'One' };
const details = { pageUrl: '', pageTitle: null, tagName: 'div', selector: null, htmlPreview: '', componentName: null,
  source: { functionName: null, fileName: 'a.ts', lineNumber: 1, columnNumber: null }, styles: '' };
const element = { ...base, kind: 'element', ...details };
const annotation = { ...base, kind: 'preview-annotation', annotationId: 'a', pageUrl: '', pageTitle: null,
  comment: '', targetSummary: '', styleChanges: [], elements: [details], screenshotContextId: 'photo' };
test('new stored element admission accepts canonical nested values and harmless excess struct fields', () => {
  expect(valid(element)).toBe(true);
  expect(valid({ ...element, extra: 'keep', source: { ...details.source, extra: 'keep' } })).toBe(true);
  for (const record of [{ ...element, tagName: ' div ' }, { ...element, contextId: ' one ' },
    { ...element, source: { ...details.source, lineNumber: -1 } }, { ...element, source: {} }]) {
    const before = JSON.stringify(record); expect(valid(record)).toBe(false); expect(JSON.stringify(record)).toBe(before);
  }
});
test('annotation optional fields and nested elements cannot normalize invalid saved values', () => {
  expect(valid(annotation)).toBe(true);
  for (const patch of [{ elements: [{ ...details, tagName: ' div ' }] }, { screenshotContextId: ' photo ' },
    { elements: undefined }, { regionCount: undefined }, { regionCount: -1 }, { styleChanges: [undefined] }])
    expect(valid({ ...annotation, ...patch } as Obj)).toBe(false);
});
test('future payloads retain all JSON values but malformed known records still fail', () => {
  for (const payload of [null, 1, 'x', [1, null], { arbitrary: { extra: [true, 'stay'] } }]) {
    const record = { ...base, kind: 'future-v2', payload }, before = JSON.stringify(record);
    expect(valid(record)).toBe(true); expect(JSON.stringify(record)).toBe(before);
  }
  expect(valid({ ...base, kind: ' future-v2 ', payload: {} })).toBe(false);
  expect(valid({ ...base, kind: 'future-v2', payload: { invalid: undefined } })).toBe(false);
  expect(valid({ ...base, kind: 'image', payload: {} })).toBe(false);
});
test('clipboard decoding then stored projection keeps annotations and their screenshot dependencies', () => {
  const image = { ...base, kind: 'image', contextId: 'photo', attachmentId: 'asset', name: 'a.png', mimeType: 'image/png', sizeBytes: 1 };
  const decoded = decode({ version: 1, source: { environmentId: 'e' }, records: [{ ...annotation, elements: [{ ...details, tagName: ' div ' }] }, image] })!;
  expect(decoded.records.every(valid)).toBe(true);
  const selected = project('[annotation](t3-context://v1/preview-annotation/one)', { version: 1, records: decoded.records });
  expect(selected).toEqual({ ok: true, context: { version: 1, records: [{ ...annotation, elements: [details] }, image] } });
  const bad = { version: 1, records: [{ ...annotation, screenshotContextId: ' photo ' }, image] };
  expect(project('', bad).ok).toBe(false); // Validate first; pruning all links cannot launder the bad record.
});
