import { expect, test } from 'bun:test';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorView as view, mobileNewTaskEditorEffect as effect, mobileNewTaskEditorAllowsWriter } from './new-task-editor';
import { fixture, picker, config, event, receipt, mounted, intakeReply, continuation } from './new-task-editor-test-support';
import { obj } from './shared/domain';
import { mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftNoteBranch, mobileNewTaskDraftStore } from './mobile-new-task-drafts';

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
