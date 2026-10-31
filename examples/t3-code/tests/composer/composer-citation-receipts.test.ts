import { expect, test } from 'bun:test';
import { ComposerCitationReceipts } from '../../src/composer/composer-citation-receipts';
import { insertCitation } from '../../src/composer/composer-source-chips';

const snapshot = (value = 'before') => ({ generation: 1, snapshotOwner: 'thread', owner: 'prompt', value, start: value.length, end: value.length, live: true, composing: false });
const frame = [0, 10, 100, 20] as const;

test('open waits for accepted input, follows surrounding typing and consumes once', () => {
  const state = new ComposerCitationReceipts(), current = snapshot(), inserted = insertCitation(current.value, current.start, '[citation]');
  const receipt = state.prepare(current, inserted.citation.value, { kind: 'open', fresh: inserted.citation, sourceFrame: frame })!;
  const typed = snapshot(`${inserted.citation.value}later`);
  const effect = state.complete(receipt, true, typed);
  expect(effect).toMatchObject({ kind: 'open', fresh: { value: typed.value, start: inserted.citation.start, source: '[citation]' }, sourceFrame: frame });
  expect(state.complete(receipt, true, typed)).toBeNull();
  const refused = state.prepare(current, inserted.citation.value, { kind: 'open', fresh: inserted.citation, sourceFrame: frame })!;
  expect(state.complete(refused, false, typed)).toBeNull();
  const modified = state.prepare(current, inserted.citation.value, { kind: 'open', fresh: inserted.citation, sourceFrame: frame })!;
  expect(state.complete(modified, true, snapshot(inserted.citation.value.replace('[citation]', 'edited')))).toBeNull();
});

test('deferred send needs the exact edited source and current owner, without replaying a newer press', () => {
  const state = new ComposerCitationReceipts(), current = snapshot();
  expect(state.prepare(current, current.value, { kind: 'finish', seq: 7, send: true })).toBeNull();
  const receipt = state.prepare(current, 'saved', { kind: 'finish', seq: 7, send: true })!;
  expect(state.complete(receipt, true, snapshot('saved'))).toEqual({ kind: 'finish', seq: 7, send: true, interaction: 0 });
  const old = state.prepare(current, 'saved', { kind: 'finish', seq: 8, send: true })!;
  expect(state.complete(old, true, snapshot('saved later typing'))).toEqual({ kind: 'finish', seq: 8, send: false, interaction: 0 });
  const owner = state.prepare(current, 'saved', { kind: 'finish', seq: 9, send: true })!;
  expect(state.complete(owner, true, { ...snapshot('saved'), owner: 'question' })).toBeNull();
  expect(state.complete(owner, true, snapshot('saved'))).toBeNull();
  const composition = state.prepare(current, 'saved', { kind: 'finish', seq: 10, send: true })!;
  expect(state.complete(composition, true, { ...snapshot('saved'), composing: true })).toBeNull();
});

test('pending effects are bounded and reset keeps receipt ids monotonic', () => {
  const state = new ComposerCitationReceipts(), current = snapshot();
  const pending = Array.from({ length: 16 }, () => state.prepare(current, 'saved', { kind: 'finish', seq: 1, send: true })!);
  expect(state.prepare(current, 'saved', { kind: 'finish', seq: 1, send: true })).toBeNull();
  state.reset();
  const next = state.prepare(current, 'saved', { kind: 'finish', seq: 1, send: true })!;
  expect(next.citation).toBeGreaterThan(pending.at(-1)!.citation);
  state.refuse(pending[0]!);
  expect(state.complete(next, true, snapshot('saved'))?.kind).toBe('finish');
  const retained = state.prepare(current, 'saved', { kind: 'finish', seq: 1, send: true })!;
  state.refuse({ ...retained, owner: 'stale' });
  expect(state.complete(retained, true, snapshot('saved'))?.kind).toBe('finish');
});
