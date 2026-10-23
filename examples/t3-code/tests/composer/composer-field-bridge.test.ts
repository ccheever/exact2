import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
import { T3Client } from '../../src/client/client';
import { editorLocal, replaceComposerPrompt } from '../../src/composer/composer-editor';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldResource, composerFieldObserved, composerFieldAcknowledged, composerFieldNative, type ComposerFieldObservation } from '../../src/composer/composer-field-bridge';
import { composerHistoryAvailability } from '../../src/composer/composer-prompt-history';
import { composerChipClose, composerChipCurrent, composerChipOpen, composerCitationPrepare } from '../../src/composer/composer-chip-bridge';
import { composerSourceSession } from '../../src/composer/composer-rich-state';
import { serializeAssistantCitation } from '../../src/timeline/diff-citations';
import { insertCitation } from '../../src/composer/composer-source-chips';

function setup() {
  const host = { generation: 1, snapshotOwner: 'snapshot', draftKey: 'draft', renderedOwner: 'rendered' };
  const observation: ComposerFieldObservation = { ...host, owner: 'rendered', value: 'one', start: 3, end: 3, composing: false, live: true, focused: false, inputSerial: 0 };
  const passthrough: unknown[] = [];
  const native = { available: true, watch() {}, later: async (request: unknown) => { passthrough.push(request); return { ok: true, generation: 1, value: {} }; } };
  const resource = () => composerFieldResource(host, [observation, 0], host.renderedOwner);
  resource();
  return { host, observation, passthrough, native, get resource() { return resource(); } };
}
const fieldCommandKeys = readFileSync(appPath('src/composer/composer-rich.contract'), 'utf8')
  .match(/^shape ComposerFieldCommand\n((?:  [^\n]+\n)+)/m)![1]!.trim().split('\n').map(line => line.trim().split(':')[0]!).sort();

describe('composer field app bridge', () => {
  for (const order of ['resource-echo-before-input', 'ACK-before-input']) test(`early ACK defers through finite wake for authoritative input: ${order}`, async () => {
    const state = setup(), owner = state.host.renderedOwner;
    const native = composerFieldNative(state.host, state.native, () => owner);
    await native.later({ op: 'editorEdit', text: '!', then: 'models' });
    const command = state.resource, entered = { ...state.observation, value: 'one!', start: 4, end: 4, inputSerial: 1 };
    if (order === 'resource-echo-before-input') composerFieldResource(state.host, [entered, command.seq], owner);
    const calls: unknown[] = [];
    const current = { available: true, watch() {}, later: async (request: unknown) => {
      calls.push(request); composerFieldObserved(state.host, [entered, true], owner);
      return { ok: true, generation: 1, value: {} };
    } };
    const ack = await composerFieldAcknowledged(state.host, [order === 'ACK-before-input' ? state.observation : entered, command.seq, true], owner, current);
    expect(calls).toEqual([{ op: 'wake', topic: 't3.notify' }]);
    expect(ack).toMatchObject({ applied: true, value: 'one!', start: 4, end: 4, inputSerial: 1, then: 'models' });
    expect((state.host as { error?: string }).error).toBeUndefined();
    expect(await native.later({ op: 'editorState' })).toMatchObject({ value: { text: 'one!', caret: 4 } });
  });
  for (const live of [true, false]) test(`idle resource emits exactly the declared command keys when live=${live}`, () => {
    const state = setup(), observation = { ...state.observation, live, composing: !live, focused: true, inputSerial: 7 };
    const command = composerFieldResource(state.host, [observation, 12], state.host.renderedOwner);
    expect(command).not.toBeInstanceOf(Promise);
    expect(Object.keys(command).sort()).toEqual(fieldCommandKeys);
    expect(command).toMatchObject({ generation: observation.generation, snapshotOwner: observation.snapshotOwner, owner: observation.owner,
      ready: false, seq: 12, value: '', start: 0, end: 0 });
  });
  test('menu replacements use current fence policy and reject another owner or source policy', async () => {
    for (const situation of ['current', 'owner', 'source']) {
      const state = setup(), owner = state.host.renderedOwner, value = '```\n@path';
      const session = composerSourceSession(state.host), sessionOwner = situation === 'owner' ? 'other' : owner;
      const extra = situation === 'source' ? 'x' : '';
      session.observe(sessionOwner, '\n@path' + extra);
      session.observe(sessionOwner, value + extra, 'insertText');
      const observed = { ...state.observation, value, start: value.length, end: value.length, inputSerial: 1 };
      composerFieldObserved(state.host, [observed, true], owner);
      const native = composerFieldNative(state.host, state.native, () => owner);
      const pending = native.later({ op: 'editorEdit', kind: 'path', start: 4, end: 9, text: '@next ' });
      if (situation !== 'current') {
        expect(await pending).toMatchObject({ value: { applied: false, reason: 'changed' } });
        continue;
      }
      const command = state.resource;
      expect(command).toMatchObject({ start: 4, end: 9, text: '@next ' });
      const accepted = { ...observed, value: '```\n@next ', start: 10, end: 10, inputSerial: 2 };
      composerFieldObserved(state.host, [accepted, true], owner);
      (await composerFieldAcknowledged(state.host, [accepted, command.seq, true], owner));
      expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
    }
  });
  test('native callers receive queued admission and independent ACK carries finite effects once', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = native.later({ op: 'editorEdit', text: '!', then: 'models', focus: false });
    const command = state.resource;
    expect(command).toMatchObject({ seq: 1, text: '!', focus: false, then: 'models' });
    expect(Object.keys(command).sort()).toEqual(fieldCommandKeys);
    const observed = { ...state.observation, value: 'one!', start: 4, end: 4, inputSerial: 1 };
    composerFieldObserved(state.host, [observed, true], state.host.renderedOwner);
    const ack = (await composerFieldAcknowledged(state.host, [observed, command.seq, true], state.host.renderedOwner));
    expect(ack).toMatchObject({ applied: true, seq: 1, then: 'models' });
    expect(await pending).toEqual({ ok: true, generation: 1, value: { accepted: true, queued: true, applied: false, reason: '', seq: 1 } });
    expect((await composerFieldAcknowledged(state.host, [observed, command.seq, true], state.host.renderedOwner)).then).toBe('');
    expect(state.passthrough.filter(item => (item as {op:string}).op !== 'wake')).toEqual([]);
  });
  test('resource/moved read ahead cannot erase pending actual input; repeated old input cannot rewind source', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = native.later({ op: 'editorEdit', text: '!' });
    const command = state.resource;
    const later = { ...state.observation, value: 'one!x', start: 5, end: 5, inputSerial: 2 };
    void composerFieldResource(state.host, [later, command.seq], state.host.renderedOwner);
    composerFieldObserved(state.host, [{ ...later, value: 'one!', start: 4, end: 4, inputSerial: 1 }, true], state.host.renderedOwner);
    composerFieldObserved(state.host, [later, true], state.host.renderedOwner);
    composerFieldObserved(state.host, [{ ...later, value: 'one!', start: 4, end: 4, inputSerial: 1 }, true], state.host.renderedOwner);
    expect(await native.later({ op: 'editorState' })).toMatchObject({ value: { text: 'one!x', caret: 5 } });
    expect((await composerFieldAcknowledged(state.host, [later, command.seq, true], state.host.renderedOwner)).applied).toBe(true);
    expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
  });
  test('stale resource metadata and ACK captures cannot rewind newer accepted source/caret', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = native.later({ op: 'editorEdit', text: '!' });
    const command = state.resource;
    const first = { ...state.observation, value: 'one!', start: 4, end: 4, inputSerial: 1 };
    const latest = { ...first, value: 'one!x', start: 5, end: 5, inputSerial: 2 };
    composerFieldObserved(state.host, [first, true], state.host.renderedOwner);
    composerFieldObserved(state.host, [latest, true], state.host.renderedOwner);
    void composerFieldResource(state.host, [first, command.seq], state.host.renderedOwner);
    expect(await native.later({ op: 'editorState' })).toMatchObject({ value: { text: 'one!x', caret: 5 } });
    const ack = (await composerFieldAcknowledged(state.host, [first, command.seq, true], state.host.renderedOwner));
    expect(ack).toMatchObject({ applied: true, value: 'one!x', start: 5, end: 5 });
    expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
  });
  test('a late ACK alone reads the newest accepted source and post-selection caret', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = native.later({ op: 'editorEdit', text: '!' });
    const command = state.resource;
    const first = { ...state.observation, value: 'one!', start: 4, end: 4, inputSerial: 1 };
    const latest = { ...first, value: 'one!x', start: 2, end: 2, inputSerial: 2 };
    composerFieldObserved(state.host, [first, true], state.host.renderedOwner);
    composerFieldObserved(state.host, [latest, true], state.host.renderedOwner);
    const ack = (await composerFieldAcknowledged(state.host, [first, command.seq, true], state.host.renderedOwner));
    expect(ack).toMatchObject({ applied: true, value: 'one!x', start: 2, end: 2 });
    expect(await native.later({ op: 'editorState' })).toMatchObject({ value: { text: 'one!x', caret: 2 } });
    expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
  });
  test('no-op resource echo cannot acknowledge and stale generations/owners cannot edit', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = native.later({ op: 'editorEdit', text: '' });
    const command = state.resource;
    const metadata = { ...state.observation, focused: true, inputSerial: 1 };
    composerFieldObserved(state.host, [metadata, false], state.host.renderedOwner);
    expect((await composerFieldAcknowledged(state.host, [metadata, command.seq, true], state.host.renderedOwner)).applied).toBe(false);
    expect(await pending).toMatchObject({ value: { applied: false } });
    state.host.generation++;
    expect(await native.later({ op: 'editorInsert', text: 'stale' })).toMatchObject({ value: { applied: false } });
    expect(composerFieldObserved(state.host, [state.observation, true], state.host.renderedOwner).reason).toBe('field observation stale');
    expect(state.passthrough.filter(item => (item as {op:string}).op !== 'wake')).toEqual([]);
  });
  test('question/queued-edit owner changes reject old callers without poisoning the new field', async () => {
    const state = setup(), oldNative = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const oldEdit = oldNative.later({ op: 'editorEdit', text: 'old' });
    await state.resource;
    state.host.renderedOwner = 'question-two';
    const next = { ...state.observation, owner: state.host.renderedOwner, value: 'answer', start: 6, end: 6 };
    const resource = () => composerFieldResource(state.host, [next, 0], state.host.renderedOwner);
    expect(await oldEdit).toMatchObject({ value: { applied: false } });
    expect(await oldNative.later({ op: 'editorInsert', text: 'stale' })).toMatchObject({ value: { applied: false } });
    resource();
    const current = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    const pending = current.later({ op: 'editorEdit', text: '!' });
    const command = resource();
    expect(composerFieldObserved(state.host, [{ ...state.observation, inputSerial: 1 }, true], state.host.renderedOwner).reason).toBe('field observation stale');
    const observed = { ...next, value: 'answer!', start: 7, end: 7, inputSerial: 1 };
    composerFieldObserved(state.host, [observed, true], state.host.renderedOwner);
    (await composerFieldAcknowledged(state.host, [observed, command.seq, true], state.host.renderedOwner));
    expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
  });
  test('OS calls pass through; a source edit goes to the field', async () => {
    const state = setup(), native = composerFieldNative(state.host, state.native, () => state.host.renderedOwner);
    await native.later({ op: 'pasteFiles' });
    const pending = native.later({ op: 'editorEdit', text: '!' });
    await pending;
    const command = state.resource;
    expect(state.passthrough).toEqual([{ op: 'pasteFiles' }, {op:'wake',topic:'t3.notify'}]);
    const observed = { ...state.observation, value: 'one!', start: 4, end: 4, inputSerial: 1 };
    composerFieldObserved(state.host, [observed, true], state.host.renderedOwner);
    (await composerFieldAcknowledged(state.host, [observed, command.seq, true], state.host.renderedOwner));
    expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
  });
});

describe('shared field caller persistence', () => {
  test('history recall uses one guarded field command and typing ends the recall position', async () => {
    const client = new T3Client();
    Object.assign(client, { environmentId: 'env', threadId: 'thread' });
    client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'sent', item: { type: 'user_message', text: 'sent prompt' } }] } };
    const owner = composerFieldOwner(client);
    const observed: ComposerFieldObservation = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner,
      value: '', start: 0, end: 0, composing: false, live: true, focused: true, inputSerial: 0 };
    const resource = () => composerFieldResource(client, [observed, 0], owner);
    resource();
    const native = composerFieldNative(client, { available: true, watch() {}, later: async () => ({ok:true,generation:client.generation,value:{}}) }, () => composerFieldOwner(client));
    expect(composerHistoryAvailability(client, owner, '')).toEqual({ historyPrevious: true, historyNext: false });
    const pending = editorLocal(client, native, 'history', 'previous', '');
    const command = resource();
    expect(command).toMatchObject({ value: '', start: 0, end: 0, text: 'sent prompt' });
    expect(client.draft).toBe('');
    const input = { ...observed, value: command.text, start: 11, end: 11, inputSerial: 1 };
    composerFieldObserved(client, [input, true], owner);
    client.local.drafts[client.draftKey] = input.value;
    const edited = { ...input, value: input.value + '!', start: 12, end: 12, inputSerial: 2 };
    composerFieldObserved(client, [edited, true], owner);
    client.local.drafts[client.draftKey] = edited.value;
    expect((await composerFieldAcknowledged(client, [input, command.seq, true], owner))).toMatchObject({ applied: true, value: edited.value });
    await pending;
    expect(composerHistoryAvailability(client, owner, input.value)).toEqual({ historyPrevious: false, historyNext: true });
    expect(composerHistoryAvailability(client, owner, edited.value)).toEqual({ historyPrevious: false, historyNext: false });
    expect(composerHistoryAvailability(client, owner + ':question', input.value)).toEqual({ historyPrevious: false, historyNext: false });
  });
  test('replace prompt persists only its actual accepted input', async () => {
    const client = new T3Client();
    Object.assign(client, { environmentId: 'env', threadId: 'thread' });
    client.local.drafts[client.draftKey] = 'old';
    const owner = composerFieldOwner(client);
    const observed: ComposerFieldObservation = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner,
      value: 'old', start: 3, end: 3, composing: false, live: true, focused: true, inputSerial: 0 };
    const resource = () => composerFieldResource(client, [observed, 0], owner);
    resource();
    const native = composerFieldNative(client, { available: true, watch() {}, later: async () => ({ok:true,generation:client.generation,value:{}}) }, () => composerFieldOwner(client));
    const pending = replaceComposerPrompt(client, native, 'new');
    const command = resource();
    expect(client.draft).toBe('old');
    const input = { ...observed, value: 'new', inputSerial: 1 };
    composerFieldObserved(client, [input, true], owner);
    client.local.drafts[client.draftKey] = input.value;
    (await composerFieldAcknowledged(client, [input, command.seq, true], owner));
    await pending;
    expect(client.draft).toBe('new');
  });
  test('a refused replacement leaves the persisted prompt intact', async () => {
    const client = new T3Client();
    Object.assign(client, { environmentId: 'env', threadId: 'thread' });
    client.local.drafts[client.draftKey] = 'old';
    const owner = composerFieldOwner(client);
    const observed: ComposerFieldObservation = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner,
      value: 'old', start: 3, end: 3, composing: false, live: true, focused: true, inputSerial: 0 };
    const resource = () => composerFieldResource(client, [observed, 0], owner);
    resource();
    const native = composerFieldNative(client, { available: true, watch() {}, later: async () => ({ok:true,generation:client.generation,value:{}}) }, () => composerFieldOwner(client));
    const pending = replaceComposerPrompt(client, native, 'new');
    const command = resource();
    (await composerFieldAcknowledged(client, [observed, command.seq, false], owner));
    await pending; expect(client.error).toBe('The prompt changed before the edit could be applied. Please try again.');
    expect(client.draft).toBe('old');
  });
});

describe('citation effects through the actual field bridge', () => {
  const link = serializeAssistantCitation({ version: 1, environmentId: 'env', threadId: 'thread', messageId: 'message', start: 0, end: 5, text: 'hello', prefix: '', suffix: '' });
  async function citationField(value = link) {
    const client = new T3Client(), owner = composerFieldOwner(client);
    const observation: ComposerFieldObservation = { generation: client.generation, snapshotOwner: client.snapshotOwner, owner, value, start: value.length, end: value.length, composing: false, live: true, focused: true, inputSerial: 0 };
    const resource = () => composerFieldResource(client, [observation, 0], owner);
    resource();
    const native = composerFieldNative(client, { available: true, watch() {}, async later() { return { ok: true, generation: client.generation, value: {} }; } }, () => composerFieldOwner(client));
    const session = composerSourceSession(client);
    session.observe(owner, value);
    const chip = session.chips[0];
    const press = chip ? composerChipOpen(client, { ...observation, ...chip, surface: 'composer', comment: chip.kind === 'citation', frame: { unavailable: false, x: 10, y: 20, width: 80, height: 18 } }) : null;
    return { client, owner, observation, get resource() { return resource(); }, native, session, press };
  }
  test('citation ACK uses live paragraph atoms only for the matching session owner and value', async () => {
    for (const situation of ['current', 'owner', 'source']) {
      const state = await citationField('\n$skill '), value = '```\n$skill ';
      state.session.observe(state.owner, value, 'insertText');
      const observed = { ...state.observation, value, start: value.length, end: value.length, inputSerial: 1 };
      composerFieldObserved(state.client, [observed, true], state.owner);
      const inserted = insertCitation(value, value.length, link);
      const receipt = composerCitationPrepare(state.client, value, inserted.citation.value,
        { kind: 'open', fresh: inserted.citation, sourceFrame: [10, 20, 80, 18] });
      if (!receipt) throw new Error('expected guarded citation receipt');
      const pending = state.native.later({ op: 'editorEdit', all: true, expect: value, text: inserted.citation.value, citation: receipt.citation });
      const command = state.resource;
      const accepted = { ...observed, value: inserted.citation.value, start: inserted.citation.value.length, end: inserted.citation.value.length, inputSerial: 2 };
      composerFieldObserved(state.client, [accepted, true], state.owner);
      state.session.observe(state.owner, accepted.value, 'insertReplacementText');
      if (situation !== 'current') {
        const other = situation === 'owner' ? 'other' : state.owner, extra = situation === 'source' ? 'x' : '';
        state.session.observe('reset', '');
        state.session.observe(other, accepted.value.slice(3) + extra);
        state.session.observe(other, accepted.value + extra, 'insertText');
      }
      expect((await composerFieldAcknowledged(state.client, [accepted, command.seq, true], state.owner)).applied).toBe(true);
      expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
      expect(composerChipCurrent(state.client)?.kind).toBe(situation === 'current' ? 'citation' : undefined);
    }
  });
  test('save-and-send completes once through ACK after original input, while later typing prevents send', async () => {
    for (const situation of ['ordinary', 'typing', 'dismissed']) {
      const extra = situation === 'typing' ? ' later' : '';
      const state = await citationField(), text = serializeAssistantCitation({ version: 1, environmentId: 'env', threadId: 'thread', messageId: 'message', start: 0, end: 5, text: 'hello', prefix: '', suffix: '', comment: 'saved' });
      const receipt = composerCitationPrepare(state.client, state.observation.value, text, { kind: 'finish', seq: state.press!.seq, send: true })!;
      const pending = state.native.later({ op: 'editorEdit', all: true, expect: state.observation.value, text, citation: receipt.citation });
      const command = state.resource;
      const accepted = { ...state.observation, value: text, start: text.length, end: text.length, inputSerial: 1 };
      composerFieldObserved(state.client, [accepted, true], state.owner);
      const latest = { ...accepted, value: text + extra, start: text.length + extra.length, end: text.length + extra.length, inputSerial: extra ? 2 : 1 };
      if (extra) composerFieldObserved(state.client, [latest, true], state.owner);
      if (situation === 'dismissed') expect(composerChipClose(state.client, state.press!.seq)).toBe(false);
      const ack = (await composerFieldAcknowledged(state.client, [latest, command.seq, true], state.owner));
      expect(ack).toMatchObject({ applied: true, value: latest.value, send: situation === 'ordinary' });
      expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
      expect((await composerFieldAcknowledged(state.client, [latest, command.seq, true], state.owner)).send).toBe(false);
    }
  });
  test('fresh insertion opens from ACK and a newer product press cannot be displaced by late completion', async () => {
    for (const situation of ['ordinary', 'newer', 'dismissed']) {
      const state = await citationField('$skill '), inserted = insertCitation(state.observation.value, state.observation.start, link);
      const receipt = composerCitationPrepare(state.client, state.observation.value, inserted.citation.value, { kind: 'open', fresh: inserted.citation, sourceFrame: [10, 20, 80, 18] })!;
      const pending = state.native.later({ op: 'editorEdit', all: true, expect: state.observation.value, text: inserted.citation.value, citation: receipt.citation });
      const command = state.resource;
      const accepted = { ...state.observation, value: inserted.citation.value, start: inserted.citation.value.length, end: inserted.citation.value.length, inputSerial: 1 };
      composerFieldObserved(state.client, [accepted, true], state.owner);
      if (situation === 'newer') {
        state.session.observe(state.owner, accepted.value);
        composerChipOpen(state.client, { ...accepted, ...state.session.chips[0]!, surface: 'composer', comment: false, frame: { unavailable: false, x: 10, y: 20, width: 80, height: 18 } });
      }
      if (situation === 'dismissed') composerChipClose(state.client, state.press!.seq);
      expect((await composerFieldAcknowledged(state.client, [accepted, command.seq, true], state.owner)).applied).toBe(true);
      expect(await pending).toMatchObject({ value: { accepted: true, queued: true, applied: false } });
      expect(composerChipCurrent(state.client)?.kind).toBe(situation === 'newer' ? 'skill' : situation === 'ordinary' ? 'citation' : undefined);
    }
  });
});
