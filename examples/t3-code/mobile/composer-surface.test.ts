import { expect, test } from 'bun:test';
import { mobileComposerSurfaceInput, mobileComposerSurfaceSnapshot as snapshot, mobileComposerSurfaceAction as action,
  type ComposerSurfaceInput } from './composer-surface';
import { mobileEditorOwner } from './composer-editor-owner';
import { fixture, picker, event, now, frozenSource } from './new-task-editor-test-support';
import { obj } from './shared/domain';
import { mobileVoiceStatus } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';
import { mobileComposerTarget } from './composer-target';

async function surface() {
  const f = await fixture();
  const input: ComposerSurfaceInput = { surface: 'new-task', flowOwner: f.route.owner, active: true,
    visit: f.route.visit, url: f.route.location, environmentId: '', threadId: '', editorId: f.route.editorId,
    preferencesReady: true, preferencesJSON: JSON.stringify({ planModeEnabled: false, followUpBehavior: 'queue' }),
    scheme: 'light', themeId: 't3-code', enterBehavior: 'send', focusSerial: '', focusAttempt: 0, focusOperation: 'none', now };
  const view = (latch = '') => snapshot(f.client, input, latch);
  const run = (op: string, key = '', payload = '', admission = view().admission) =>
    action(f.client, input, { op, key, payload, admission }, f.native, f.storage);
  return { ...f, input, view, run };
}

test('Root surface keeps independent native source outside the ordinary composer owner', async () => {
  const f = await surface(), ordinary = mobileEditorOwner(f.client);
  const observed = event(f.identity, '🙂 newest native text', 2, 'text');
  const shown = f.view(JSON.stringify(observed));
  expect(shown.enabled).toBe(true); expect(shown.eventKey).not.toBe('');
  expect(JSON.parse(shown.configuration)).toMatchObject({ editable: true, active: true,
    document: { value: observed.value }, presentation: { placeholder: 'Ask anything…', maxInputChars: 1_000_000 } });
  expect((await f.run('event', '', JSON.stringify(observed))).message).toBe('');
  expect(obj(f.disk().drafts)[f.key]).toBe(observed.value);
  expect(mobileEditorOwner(f.client)).toBe(ordinary);
  expect(f.client.threadId).toBe('');
});

test('closed pending and ordinary discriminators cannot admit a NewTask as a thread', async () => {
  const f = await surface();
  for (const surface of ['pending-task', 'inactive', 'thread'] as const) {
    expect(snapshot(f.client, { ...f.input, surface }, '').enabled).toBe(false);
  }
  expect(() => mobileComposerSurfaceInput({ ...f.input, surface: 'fake-thread' })).toThrow('Unknown composer surface');
  expect(f.view().enabled).toBe(true);
  expect(f.calls.some(call => call.action === 'freeze')).toBe(false);
});

test('Root navigation claims one effect only after saving the actual frozen source', async () => {
  const f = await surface(); picker(f);
  const requested = await f.run('new-task', 'model');
  expect(requested.navigation).toBe('');
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  const effect = f.view().effectId;
  expect(effect).not.toBe('');
  expect(await f.run('claim-effect', effect)).toMatchObject({ navigation: 'push', location: '/new/draft/settings' });
  expect(await f.run('claim-effect', effect)).toMatchObject({ navigation: '', location: '' });
});

test('Root refuses an old admission before freezing a producer', async () => {
  const f = await surface(); picker(f);
  expect((await f.run('new-task', 'send', '', 'old-admission')).message).toBe('The new task editor changed.');
  expect(f.calls.some(call => call.action === 'freeze')).toBe(false);
  expect(f.view().effectId).toBe('');
});

for (const kind of ['pasteImages', 'pasteText'] as const) test(`${kind} reports its native refusal without losing pending text`, async () => {
  const f = await surface(), source = event(f.identity, '🙂 pending text', 2, 'selection');
  f.view(JSON.stringify(source));
  const result = await f.run('rich', '', JSON.stringify({ ...source, kind, richEventId: 'native-paste', editorEvent: source, payload: {} }));
  expect(result.message).not.toBe(''); expect(obj(f.disk().drafts)[f.key]).toBe(source.value);
  expect(f.calls.some(call => call.action === 'freeze')).toBe(false);
});

test('completed producer navigation survives removal of the editor without borrowing another visit', async () => {
  const f = await surface(); picker(f); await f.run('new-task', 'model');
  const detached = { ...f.input, editorId: '' }, offered = snapshot(f.client, detached, '');
  expect(offered.effectId).not.toBe('');
  expect(snapshot(f.client, { ...detached, visit: 'new-visit' }, '').effectId).toBe('');
  expect(await action(f.client, detached, { op: 'claim-effect', admission: offered.admission, key: offered.effectId, payload: '' }, f.native, f.storage))
    .toMatchObject({ navigation: 'push', location: '/new/draft/settings' });
});

test('a held source can leave its own visit while stale Back has no navigation authority', async () => {
  const f = await surface(), before = f.client.local.drafts[f.key];
  f.client.threadEpoch++;
  expect(f.view().enabled).toBe(false);
  expect(await f.run('new-task', 'back')).toMatchObject({ navigation: 'back' });
  expect(await action(f.client, { ...f.input, visit: 'other' }, { op: 'new-task', key: 'back', payload: '', admission: '' }, f.native, f.storage))
    .toMatchObject({ navigation: '' });
  expect(f.client.local.drafts[f.key]).toBe(before);
});

for (const focused of [true, false]) test(`Root claims an unavailable voice reason once after native resume with focus ${focused}`, async () => {
  resetVoiceInputGlobalsForTests();
  const f = await surface(); picker(f);
  f.handoffHook(async request => {
    const receipt = frozenSource(f, request); receipt.event.focused = focused; f.receipts([receipt]);
    return { ok: true, generation: request.generation, value: { receipt } };
  });
  const original = f.native.later; let available = true;
  f.native.later = async request => obj(request).op === 'mobileVoice' && obj(request).action === 'status'
    ? { ok: true, generation: f.client.generation, value: { available, locale: 'en-US', session: '', event: 0, levels: [], elapsed: 0 } }
    : original(request);
  await mobileVoiceStatus(f.native, f.client);
  available = false;
  f.resumeHook(async request => {
    f.receipts([]);
    return { ok: true, generation: request.generation, value: { event: {
      ...event(f.identity, String(obj(request.document).value), 5, 'resumed'), selection: obj(request.document).selection,
      operationId: request.operationId, operationRevision: request.operationRevision, receiptId: request.receiptId,
    } } };
  });
  await f.run('new-task', 'voice');
  const shown = f.view();
  expect(JSON.parse(shown.configuration).editable).toBe(true);
  expect(shown.owner).toBe(mobileComposerTarget(f.client).owner);
  expect(shown.effectId).not.toBe('');
  await f.run('retry');
  expect(await f.run('claim-effect', shown.effectId)).toMatchObject({ message: 'Voice transcription is not available.', navigation: focused ? 'focus' : '' });
  expect(await f.run('claim-effect', shown.effectId)).toMatchObject({ message: '', navigation: '' });
});
