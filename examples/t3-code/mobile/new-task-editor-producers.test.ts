import { expect, test } from 'bun:test';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorView as view, mobileNewTaskEditorEffect as effect, mobileNewTaskEditorAllowsWriter } from './new-task-editor';
import { fixture, picker, config, event, receipt, mounted, intakeReply, continuation } from './new-task-editor-test-support';
import { obj } from './shared/domain';
import { mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftNoteBranch, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileComposerTarget } from './composer-target';
import { mobileVoiceAction, mobileVoiceSnapshot } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';
import { mobileNewTask } from './new-task';

for (const destination of ['model', 'branch', 'environment', 'project', 'back'] as const) test(`NewTask ${destination} saves and acknowledges the actual final source before navigation`, async () => {
  const f = await fixture(); picker(f);
  const result = await action(f.client, f.route, { kind: 'navigate', destination }, f.native, f.storage);
  expect(result.accepted).toBe(true);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  expect(f.order.indexOf('save')).toBeLessThan(f.order.indexOf('composerEditorFinalizations:acknowledge'));
  const e = effect(f.client, f.route); expect(e).toMatchObject({ kind: 'navigate', destination });
  expect(view(f.client, f.route).kind).toBe('frozen');
  expect(f.calls.some(call => call.action === 'resume')).toBe(false);
  expect((await action(f.client, f.route, { kind: 'claim', id: e!.id }, f.native, f.storage)).effect).toEqual(e!);
  expect((await action(f.client, f.route, { kind: 'claim', id: e!.id }, f.native, f.storage)).effect).toBeUndefined();
});
test('failed final save neither navigates nor acknowledges; fresh retry retains original source', async () => {
  const f = await fixture(); picker(f); f.writeHook(async () => { throw Error('disk full'); });
  await action(f.client, f.route, { kind: 'navigate', destination: 'back' }, f.native, f.storage);
  expect(effect(f.client, f.route)).toBeNull(); expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
  expect(mobileNewTaskEditorAllowsWriter(f.client, f.key)).toBe(false);
  f.writeHook(null); await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(effect(f.client, f.route)?.destination).toBe('back');
});
test('sheet retirement permits a fresh editor with newest saved text and UTF16 selection', async () => {
  const f = await fixture(); picker(f);
  await action(f.client, f.route, { kind: 'navigate', destination: 'model' }, f.native, f.storage);
  f.reserved([]); f.detach(); await f.reread();
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, { providerId: 'codex', modelId: 'new-choice', modelOptions: [] });
  const reopened = view(f.client, await f.resume());
  expect(reopened.kind).toBe('mounting');
  expect(obj(config(reopened).document)).toMatchObject({ value: '🙂 picked here', selection: { start: 3, end: 9 } });
});
test('preview with a stale native range has no navigation authority', async () => {
  const f = await fixture(); picker(f);
  const token = JSON.stringify({ ...event(f.identity, '', 0), kind: 'contextPress', richEventId: 'press', payload: { source: 'x', start: 0, end: 1 } });
  expect((await action(f.client, f.route, { kind: 'preview', id: '', token }, f.native, f.storage)).accepted).toBe(false);
  expect(effect(f.client, f.route)).toBeNull(); expect(f.calls.some(call => call.action === 'freeze')).toBe(false);
});

test('preview refused after freeze resumes the newest saved source for further editing', async () => {
  const source = '[note](t3-context://v1/file/note)';
  const f = await fixture((client, key) => { client.local.drafts[key] = source; }); picker(f);
  f.resumeHook(async request => {
    f.receipts([]);
    return { ok: true, generation: request.generation, value: { event: {
      ...event(f.identity, String(obj(request.document).value), 5, 'resumed'), selection: obj(request.document).selection,
      operationId: request.operationId, operationRevision: request.operationRevision, receiptId: request.receiptId,
    } } };
  });
  const token = JSON.stringify({ ...event(f.identity, source), kind: 'contextPress', richEventId: 'preview-press',
    payload: { source, start: 0, end: source.length } });
  expect((await action(f.client, f.route, { kind: 'preview', id: '', token }, f.native, f.storage)).accepted).toBe(true);
  expect(view(f.client, f.route).kind).toBe('editing');
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  expect(effect(f.client, f.route)?.kind).not.toBe('preview');
  expect(effect(f.client, f.route)?.message).toBe('The attachment source changed.');
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect((await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, '🙂 continuing', 6, 'text') }, f.native, f.storage)).accepted).toBe(true);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 continuing');
});

for (const policy of [
  { ready: true, enabled: true, blocked: false, message: '', mode: 'plan' },
  { ready: true, enabled: true, blocked: true, message: 'Wait for the current draft import to finish.', mode: undefined },
  { ready: true, enabled: false, blocked: false, message: 'Plan mode is not available for this draft.', mode: undefined },
  { ready: false, enabled: true, blocked: false, message: 'Plan mode is not available for this draft.', mode: undefined },
]) test(`native Plan mode preserves preference and import policy ${JSON.stringify(policy)}`, async () => {
  const f = await fixture(); picker(f);
  f.resumeHook(async request => ({ ok: true, generation: request.generation, value: { event: {
    ...event(f.identity, String(obj(request.document).value), 5, 'resumed'), selection: obj(request.document).selection,
    operationId: request.operationId, operationRevision: request.operationRevision, receiptId: request.receiptId,
  } } }));
  const preferences = { ready: policy.ready, serialized: JSON.stringify({ ready: true, planModeEnabled: policy.enabled }) };
  await action(f.client, f.route, { kind: 'setting', name: 'interaction', value: 'plan', preferences, interactionBlocked: policy.blocked }, f.native, f.storage);
  expect(mobileNewTaskDraftStore(f.client).records[f.key]!.choices?.interactionMode).toBe(policy.mode);
  expect(mobileNewTask('', f.client, undefined, preferences).interactionError).toBe(policy.message);
  expect(view(f.client, f.route).kind).toBe('editing');
});

test('actual automatic branch metadata keeps native source admitted while an unrelated revision still fences it', async () => {
  const f = await fixture();
  mobileNewTaskDraftNoteBranch(f.client, 'automatic');
  expect(view(f.client, f.route).kind).toBe('editing');
  expect((await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, '🙂 newest', 2, 'text') }, f.native, f.storage)).accepted).toBe(true);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 newest');
  mobileNewTaskDraftStore(f.client).records[f.key]!.revision++;
  expect(view(f.client, f.route).kind).toBe('held');
});

test('ordered ready establishes a fresh native mount without a coalesced text latch', async () => {
  const f = await fixture(); f.receipts([receipt(f.identity, '', 2)]); f.detach(); await f.reread();
  const route = await f.resume(), offer = view(f.client, route), identity = { ...mounted(offer), mountId: 'new-native-mount' };
  expect(offer.kind).toBe('mounting');
  expect((await action(f.client, route, { kind: 'observe', payload: event(identity, '') }, f.native, f.storage)).accepted).toBe(true);
  expect(view(f.client, route).kind).toBe('editing');
  expect((await action(f.client, route, { kind: 'observe', payload: event({ ...identity, mountId: 'foreign' }, '') }, f.native, f.storage)).accepted).toBe(false);
});

test('a concurrently observed open picker is pending without a modal error', async () => {
  const f = await fixture(); continuation(f);
  f.intakeHook(async request => intakeReply(request, 'picking'));
  const result = await action(f.client, f.route, { kind: 'pick', source: 'files' }, f.native, f.storage);
  expect(result.message).toBe(''); expect(view(f.client, f.route).kind).toBe('frozen');
  expect(f.calls.some(call => call.action === 'cancel' || call.action === 'resume')).toBe(false);
});

test('confirmed cancellation and native retirement settle the old route before reentry', async () => {
  const f = await fixture(); continuation(f);
  f.intakeHook(async request => { f.detach(); return intakeReply(request, 'cancelled'); });
  await action(f.client, f.route, { kind: 'pick', source: 'files' }, f.native, f.storage);
  const freeze = f.calls.find(call => call.action === 'freeze')!;
  const source = { receiptId: '11111111-1111-4111-8111-111111111111', identity: f.identity, retired: true,
    event: { ...event(f.identity, '🙂 picked here', 4, 'text'), kind: 'frozen', selection: { start: 3, end: 9 },
      operationId: freeze.operationId, operationRevision: freeze.operationRevision } };
  expect(f.calls.some(call => call.action === 'resume')).toBe(false);
  f.receipts([source]); f.detach(); await f.reread();
  expect(view(f.client, await f.resume())).toMatchObject({ kind: 'mounting' });
});

test('confirmed cancelled picker settles after the acknowledged receipt is consumed', async () => {
  const f = await fixture(); continuation(f);
  f.intakeHook(async request => intakeReply(request, 'picking'));
  await action(f.client, f.route, { kind: 'pick', source: 'files' }, f.native, f.storage);
  f.receipts([]); f.reserved([]); f.detach();
  f.intakeHook(async request => intakeReply(request, 'cancelled'));
  await f.reread();
  expect(f.calls.some(call => call.action === 'resume')).toBe(false);
  expect(view(f.client, await f.resume()).kind).toBe('mounting');
});

for (const alternate of [false, true]) test(`NewTask ${alternate ? 'alternate' : 'button'} submit cannot dispatch past a failed final source save`, async () => {
  const f = await fixture(); picker(f);
  f.writeHook(async () => { throw Error('disk full'); });
  await action(f.client, f.route, { kind: 'send', alternate, now: 1791420000000,
    preferences: { ready: true, serialized: '{}' } }, f.native, f.storage);
  expect(f.client.local.drafts[f.key]).toBe('🙂 picked here');
  expect(f.calls.filter(call => call.action === 'freeze')).toHaveLength(1);
  expect(f.calls.some(call => call.action === 'acknowledge' || call.op === 'mobileOutbox')).toBe(false);
  expect(effect(f.client, f.route)).toBeNull();
  expect(view(f.client, f.route).kind).toBe('frozen');
});

test('frozen NewTask voice uses final UTF16 offsets and hands its committed caret back through resume', async () => {
  resetVoiceInputGlobalsForTests();
  const f = await fixture(); picker(f);
  const target = mobileComposerTarget(f.client), prior = f.native.later;
  let session = '';
  f.native.later = async input => {
    const request = obj(input);
    if (request.op !== 'mobileVoice') return prior(input);
    f.calls.push(request);
    if (request.action === 'selection' || request.action === 'selection-commit') throw Error('Frozen rich port cannot own voice selection');
    if (request.action !== 'status') session = String(request.session);
    const value = request.action === 'status' ? { available: true, locale: 'en-US', session, event: 0, levels: [], elapsed: 0 }
      : request.action === 'permission' ? { granted: true, canAskAgain: true }
      : request.action === 'prepare' ? { locale: 'en-US' }
      : request.action === 'recorder-prepare' || request.action === 'stop' ? { uri: 'file:///voice-fixture.m4a' }
      : request.action === 'transcribe' ? { transcript: 'spoken' } : {};
    return { ok: true, generation: f.client.generation, value };
  };
  f.resumeHook(async request => ({ ok: true, generation: request.generation, value: { event: {
    ...event(f.identity, String(obj(request.document).value), 5, 'resumed'), selection: obj(request.document).selection,
    operationId: request.operationId, operationRevision: request.operationRevision, receiptId: request.receiptId,
  } } }));
  await action(f.client, f.route, { kind: 'voice', label: 'New task', owner: target.editorOwner }, f.native, f.storage);
  expect(mobileVoiceSnapshot(target.editorOwner, f.client).phase).toBe('recording');
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  expect(f.calls.some(call => call.action === 'resume')).toBe(false);
  expect((await mobileVoiceAction('stop', -1, -1, 'New task', target.editorOwner, f.native, f.storage, f.client)).message).toBe('');
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 spoken here');
  expect(f.calls.find(call => call.action === 'resume')?.document).toMatchObject({ value: '🙂 spoken here', selection: { start: 9, end: 9 } });
  expect(f.calls.some(call => call.action === 'selection' || call.action === 'selection-commit')).toBe(false);
  expect(view(f.client, f.route).kind).toBe('editing');
});
