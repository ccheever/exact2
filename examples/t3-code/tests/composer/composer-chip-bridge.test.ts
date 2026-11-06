import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerChipClose, composerChipCurrent, composerChipOpen, composerChipPreview, composerChipTake } from '../../src/composer/composer-chip-bridge';
import { composerFieldObserved } from '../../src/composer/composer-field-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';

function setup(value = '$skill @file.ts ') {
  const client = new T3Client(), owner = composerFieldOwner(client);
  const snapshot = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner, value, start: 0, end: 0, composing: false, focused: true, live: true, inputSerial: 0 };
  composerFieldObserved(client, [snapshot, false], owner);
  const session = composerSourceSession(client);
  session.observe(owner, value);
  const request = (index = 0) => ({ ...snapshot, ...session.chips[index]!, surface: 'composer', comment: false, frame: { unavailable: false, x: 10, y: 20, width: 80, height: 18 } });
  return { client, owner, snapshot, session, request };
}

test('queued presses cannot use an older source even when its projection still exists', () => {
  const { client, owner, snapshot, request } = setup(), old = request();
  expect(composerChipOpen(client, old)?.kind).toBe('skill');
  composerFieldObserved(client, [{ ...snapshot, value: `${snapshot.value}new`, inputSerial: 1 }, true], owner);
  expect(composerChipOpen(client, old)).toBeNull();
  expect(composerChipCurrent(client)).toBeNull();
  expect(composerChipOpen(client, { ...old, value: `${snapshot.value}new` })).toBeNull();
});

test('shared chip actions are consumed once and a stale close cannot dismiss the newest press', () => {
  const { client, request } = setup();
  const skill = composerChipOpen(client, request())!;
  const action = composerChipOpen(client, request(1))!;
  expect(composerChipClose(client, skill.seq)).toBe(false);
  expect(composerChipTake(client, action.seq)).toMatchObject({ act: 'mention', path: 'file.ts' });
  expect(composerChipTake(client, action.seq)).toBeNull();
  expect(composerChipCurrent(client)).toBeNull();
});

test('owner, composition and geometry guards apply to the actual observed field', () => {
  const { client, owner, snapshot, request } = setup(), press = request();
  expect(composerChipOpen(client, { ...press, owner: 'stale' })).toBeNull();
  expect(composerChipOpen(client, { ...press, generation: client.generation + 1 })).toBeNull();
  expect(composerChipOpen(client, { ...press, frame: { ...press.frame, unavailable: true } })).toBeNull();
  composerFieldObserved(client, [{ ...snapshot, composing: true }, false], owner);
  expect(composerChipOpen(client, press)).toBeNull();
  composerFieldObserved(client, [snapshot, false], owner);
  expect(composerChipOpen(client, press)).not.toBeNull();
  client.generation++;
  expect(composerChipCurrent(client)).toBeNull();
});

test('preview uses its own session, globally replaces a composer press and closes on unmount', () => {
  const { client, snapshot, request } = setup(), composer = composerChipOpen(client, request())!;
  const owner = `${client.snapshotOwner}:prompt-preview`, value = '$preview ', session = composerSourceSession(client, 'preview');
  session.observe(owner, value, '', 'plain');
  const preview = { ...snapshot, owner, value, inputSerial: 1 };
  composerChipPreview(client, preview);
  const next = composerChipOpen(client, { ...request(), ...session.chips[0]!, surface: 'preview', owner, value, inputSerial: preview.inputSerial })!;
  expect(next.seq).toBeGreaterThan(composer.seq);
  expect(next).toMatchObject({ surface: 'preview', name: 'preview', act: '' });
  expect(composerChipClose(client, composer.seq)).toBe(false);
  expect(composerChipCurrent(client)?.seq).toBe(next.seq);
  expect(composerSourceSession(client).matches(snapshot.owner, snapshot.value)).toBe(true);
  composerChipPreview(client, { ...preview, inputSerial: 0, value: 'stale' });
  expect(composerChipCurrent(client)?.seq).toBe(next.seq);
  composerChipPreview(client, { ...preview, live: false });
  expect(composerChipCurrent(client)).toBeNull();
  const reopen = { ...preview, inputSerial: 2 };
  composerChipPreview(client, reopen);
  const oldCallback = { ...request(), ...session.chips[0]!, surface: 'preview', owner, value, inputSerial: 1 };
  expect(composerChipOpen(client, oldCallback)).toBeNull();
  expect(composerChipOpen(client, { ...oldCallback, inputSerial: 2 })?.surface).toBe('preview');
});

test('same-source undo cannot revive a callback from an older accepted input serial', () => {
  const { client, owner, snapshot, request, session } = setup(), old = request();
  composerFieldObserved(client, [{ ...snapshot, value: 'new', inputSerial: 1 }, true], owner);
  composerFieldObserved(client, [{ ...snapshot, inputSerial: 2 }, true], owner);
  session.observe(owner, snapshot.value, 'historyUndo');
  expect(composerChipOpen(client, old)).toBeNull();
  expect(composerChipOpen(client, { ...old, inputSerial: 2 })?.kind).toBe('skill');
});

test('product press admission resolves context state and rejects unavailable context callbacks', () => {
  const state = setup('[PR #7](t3-context://v1/review-comment/review-comment_p) [gone](t3-context://v1/terminal/terminal_missing)');
  Object.assign(state.client.local, { reviewComments: { 'review-comment_p': { kind: 'review-comment', contextId: 'review-comment_p', pullRequest: { state: 'merged', number: 7 } } } });
  expect(composerChipOpen(state.client, state.request())).toMatchObject({ kind: 'pr-merged', act: 'pr-merged', context: 'review-comment_p' });
  expect(composerChipOpen(state.client, state.request(1))).toBeNull();
  expect(composerChipOpen(state.client, { ...state.request(), comment: true })).toBeNull();
});
