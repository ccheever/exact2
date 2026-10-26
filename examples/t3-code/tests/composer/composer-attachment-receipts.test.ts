import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Files, Native } from '../../src/client/protocol';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldAcknowledged, composerFieldNative, composerFieldObserved, composerFieldResource } from '../../src/composer/composer-field-bridge';
import { prepareAttachmentEffect } from '../../src/composer/composer-attachment-receipts';
import { draftFiles, foldPaste } from '../../src/composer/composer-editor-files';
import { observeComposer } from '../support/composer-field-fixture';
import { captureHost, commitAnnotationSend, takeAnnotationDispatch } from '../../src/browser/browser-capture';
import { composerWrites } from '../../src/client/client-ops-composer';
import { toasts } from '../../src/shell/toast';
import { composerChipClose, composerChipFinishView, composerCitationNoEdit, takeComposerChipFinish } from '../../src/composer/composer-chip-bridge';
import { formatAssistantCitationHref } from '../../src/timeline/diff-citations';
import { citationField } from '../timeline/citation-field.fixture';

function setup(value = 'seed') {
  const client = new T3Client(), snapshot = observeComposer(client, value), owner = composerFieldOwner(client);
  client.local.drafts[client.draftKey] = value;
  const os: Native = { available: true, watch() {}, later: async () => ({ ok: true, generation: client.generation, value: {} }) };
  const native = composerFieldNative(client, os, () => composerFieldOwner(client));
  const input = (value: string, serial: number) => {
    client.local.drafts[client.draftKey] = value;
    const next = { ...snapshot, value, start: value.length, end: value.length, inputSerial: serial };
    composerFieldObserved(client, [next, true], owner);
    return next;
  };
  return { client, snapshot, owner, native, input };
}

test('fold staging settles once in the independent ACK, before its caller continuation', async () => {
  const state = setup(), { client, snapshot, owner, native } = state;
  const waiting = () => composerFieldResource(client, [snapshot, 0], owner);
  const taking = foldPaste(client, native, 'fold', { id: 1, snapshotOwner: snapshot.snapshotOwner, fieldOwner: owner, value: snapshot.value,
    start: 4, end: 4, text: 'x'.repeat(40_000) }, 'fold-1');
  await taking;
  const command = waiting();
  expect(command.completion).toBeGreaterThan(0);
  expect(draftFiles(client.local)).toHaveLength(1);
  expect(toasts(client)).toEqual([]);
  const current = state.input(command.value.slice(0, command.start) + command.text + command.value.slice(command.end), 1);
  expect((await composerFieldAcknowledged(client, [current, command.seq, true], owner)).applied).toBe(true);
  expect(toasts(client).map(toast => toast.title)).toEqual(['Large paste attached as pasted-text.txt']);
  await taking;
  expect(toasts(client)).toHaveLength(1);
});

test('a refused fold clears its reserved bytes and metadata without a success toast', async () => {
  const state = setup(), { client, snapshot, owner, native } = state;
  const waiting = () => composerFieldResource(client, [snapshot, 0], owner);
  const taking = foldPaste(client, native, 'fold', { id: 1, snapshotOwner: snapshot.snapshotOwner, fieldOwner: owner, value: snapshot.value,
    start: 4, end: 4, text: 'large paste' }, 'fold-2');
  await taking;
  waiting();
  state.input('manual edit', 1);
  await taking;
  expect(draftFiles(client.local)).toEqual([]);
  expect(toasts(client).map(toast => toast.title)).toEqual(['Paste was not added']);
});

test('annotation send receipt requires the accepted expected source at ACK and at actual deferred dispatch', async () => {
  for (const change of ['input-before-ack', 'input-after-ack', 'owner-after-ack', 'undo-to-same-source']) {
    const state = setup(), { client, snapshot, owner, native } = state;
    const completion = prepareAttachmentEffect(client, snapshot, 'seed!', { kind: 'annotation-send', draftKey: client.draftKey });
    const waiting = () => composerFieldResource(client, [snapshot, 0], owner);
    waiting();
  const pending = native.later({ op: 'editorEdit', value: 'seed', start: 4, end: 4, text: '!', completion });
    const command = waiting();
    let current = state.input('seed!', 1);
    if (change === 'input-before-ack') current = state.input('seed!x', 2);
    (await composerFieldAcknowledged(client, [current, command.seq, true], owner));
    await pending;
    if (change === 'input-before-ack') { expect(captureHost(client).sendSerial).toBe(0); continue; }
    expect(captureHost(client).sendSerial).toBe(1);
    if (change === 'owner-after-ack') client.generation++;
    else { state.input('seed!x', 2); if (change === 'undo-to-same-source') state.input('seed!', 3); }
    const out = { message: '', id: '', value: '' };
    expect(await composerWrites.call(client, 'send-annotation', '1', '', 0, native, {} as Files, out)).toBe(true);
    expect(captureHost(client).sendTarget).toBeNull();
    expect(client.pending).toBeUndefined();
  }
});

test('an already present chip uses a once-only source-qualified send target without inventing input', () => {
  const { client } = setup('[kept](t3-context://v1/preview-annotation/preview-annotation_a) ');
  commitAnnotationSend(client);
  expect(takeAnnotationDispatch(client, 0)).toBeNull();
  expect(takeAnnotationDispatch(client, 1)?.value).toBe(client.draft);
  expect(takeAnnotationDispatch(client, 1)).toBeNull();
});

test('unchanged citation send cannot dispatch after later input, history, dismissal or owner replacement', async () => {
  const href = formatAssistantCitationHref({ version: 1, environmentId: 'env', threadId: 'verify-timeline', messageId: 'message:fixture',
    text: 'quoted', start: 0, end: 6, prefix: '', suffix: '' });
  const source = `[Assistant quote](${href}) `;
  for (const change of ['input', 'undo-to-same-source', 'dismiss', 'owner', 'composition']) {
    const state = citationField(source), press = state.press();
    expect(composerCitationNoEdit(state.client, press.seq, { send: true, focus: 'after' })).toBe(true);
    expect(composerChipFinishView(state.client)).toMatchObject({ seq: press.seq, value: source, send: true, inputSerial: 0 });
    if (change === 'input' || change === 'undo-to-same-source') {
      state.type(source + 'edited');
      if (change === 'undo-to-same-source') state.type(source);
    } else if (change === 'dismiss') composerChipClose(state.client, press.seq);
    else if (change === 'composition') state.observe({ composing: true });
    else state.client.generation++;
    const before = state.current().value;
    const out = { message: '', id: '', value: '' };
    expect(await composerWrites.call(state.client, 'send-citation', `${press.seq}`, '', 0, state.native, {} as Files, out)).toBe(true);
    expect(state.calls).toEqual([]);
    expect(state.current().value).toBe(before);
    expect(state.client.pending).toBeUndefined();
    expect(composerChipFinishView(state.client)).toBeNull();
  }
});

test('a no-edit citation finish survives moved selection without inventing input and is consumed once', () => {
  const href = formatAssistantCitationHref({ version: 1, environmentId: 'env', threadId: 'verify-timeline', messageId: 'message:fixture',
    text: 'quoted', start: 0, end: 6, prefix: '', suffix: '' });
  const state = citationField(`[Assistant quote](${href}) `), press = state.press();
  expect(composerCitationNoEdit(state.client, press.seq, { send: true, focus: 'after' })).toBe(true);
  state.observe({ start: press.end, end: press.end, focused: true });
  expect(takeComposerChipFinish(state.client, press.seq - 1)).toBeNull();
  expect(takeComposerChipFinish(state.client, press.seq)).toMatchObject({ value: state.current().value, inputSerial: 0, start: press.start, end: press.end });
  expect(takeComposerChipFinish(state.client, press.seq)).toBeNull();
  expect(state.calls).toEqual([]);
});
