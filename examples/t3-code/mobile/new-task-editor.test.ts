import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mobileDraftRecoveryHydrated } from './mobile-draft-recovery';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorRefresh as refresh, mobileNewTaskEditorView as view } from './new-task-editor';
import { mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftLookup, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import type { ComposerEditorEvent, ComposerMountedIdentity } from './composer-editor-state';
import { obj, str, type Obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { mobileDraftAttachmentIds } from './draft-attachment-order';
import { contextLink } from './shared/composer-editor-menu';
import type { PickerIntakeFile } from './composer-picker-intake-io';
import { CountingClient, receiptId, deferred, mounted, event, receipt, fixture, operationId, frozenSource, picker, config,
  pickedId, oldId, pickedFile, pickedImage, intakeReply, heldReply, continuation, intakeCalls, savedRecord } from './new-task-editor-test-support';

test.skipIf(!process.env.T3_NATIVE_PREFERENCE_DOCUMENT)('first native preference document admits a writable new-task editor without restarting', async () => {
  const document = readFileSync(process.env.T3_NATIVE_PREFERENCE_DOCUMENT!, 'utf8');
  const f = await fixture(undefined, document);
  expect(mobileDraftRecoveryHydrated(f.client)).toBe(true);
  const text = 'Café 日本語';
  const next = view(f.client, f.route, event(f.identity, text, 2, 'text'));
  expect(next.kind).toBe('editing'); expect(next.editable).toBe(true);
  expect(f.client.local.drafts[f.key]).toBe(text);
});

test('Root watches before exactly one real refresh, then saves detached source across transport generations', async () => {
  const f = await fixture(), before = f.client.refreshes;
  f.receipts([receipt(f.identity)]); f.detach(); await f.reread();
  expect(f.client.refreshes).toBe(before + 1);
  expect(f.order[0]).toBe('watch:t3.composer-finalizations');
  expect(f.order.indexOf('devicePresentation:')).toBeLessThan(f.order.indexOf('composerEditorFinalizations:read'));
  expect(f.order.indexOf('save')).toBeLessThan(f.order.indexOf('composerEditorFinalizations:acknowledge'));
  expect(f.client.local.drafts[f.key]).toBe('🙂 final source');
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 final source');
  expect(f.client.local.drafts['other:other-thread']).toBe('unrelated');
  expect(f.calls.filter(call => call.op === 'composerEditorFinalizations').map(call => call.generation)).toEqual([4, 4]);
  expect(JSON.parse(f.identity.owner)[3]).toBe(3);
  const again = f.writes.length; await f.reread(); expect(f.writes).toHaveLength(again);
});

test('live observations authenticate ready and synchronously publish through the original context owner', async () => {
  const f = await fixture(), bad = { ...f.identity, mountId: 'foreign' };
  expect(await action(f.client, f.route, { kind: 'observe', payload: event(bad, 'wrong', 2, 'text') }, f.native, f.storage))
    .toMatchObject({ accepted: false });
  const gate = deferred<void>(), entered = deferred<void>();
  f.writeHook(async () => { entered.resolve(); await gate.promise; });
  const text = '🙂 exact text', editing = action(f.client, f.route,
    { kind: 'observe', payload: event(f.identity, text, 2, 'text') }, f.native, f.storage);
  expect(f.client.local.drafts[f.key]).toBe(text); await entered.promise;
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
  gate.resolve(); expect(await editing).toMatchObject({ accepted: true, message: '' });
  const saved = f.writes.length;
  expect(await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, text, 2, 'text') }, f.native, f.storage))
    .toMatchObject({ accepted: true });
  expect(f.writes).toHaveLength(saved);
  expect(await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, 'conflict', 2, 'text') }, f.native, f.storage))
    .toMatchObject({ accepted: false });
  f.receipts([receipt(f.identity, `${text} final`, 3)]); f.detach(); await f.reread();
  expect(f.client.local.drafts[f.key]).toBe(`${text} final`);
});

test('identical receipts publish once and a failed write retries the current authoritative source', async () => {
  const f = await fixture(), source = receipt(f.identity), failure = new Error('disk unavailable');
  f.receipts([source, structuredClone(source)]); f.writeHook(async () => { throw failure; });
  await f.reread();
  expect(f.client.local.drafts[f.key]).toBe(source.event.value);
  expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
  f.writeText('new authoritative text'); f.writeHook(null); await f.reread();
  expect(f.client.local.drafts[f.key]).toBe('new authoritative text');
  expect(obj(f.disk().drafts)[f.key]).toBe('new authoritative text');
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(2);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
});

test('durable receipts survive acknowledgment failure without replaying newer text', async () => {
  const f = await fixture(); f.receipts([receipt(f.identity)]);
  f.ackHook(async () => { throw new Error('ack reply lost'); }); await f.reread();
  const saves = f.writes.length; f.writeText('newer after save');
  f.ackHook(null); await f.reread();
  expect(f.writes).toHaveLength(saves); expect(f.client.local.drafts[f.key]).toBe('newer after save');
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(2);
  await f.reread(); expect(f.client.local.drafts[f.key]).toBe('newer after save');
});

test('false acknowledgment needs a newer complete absent read; a malformed read cannot release admission', async () => {
  const f = await fixture(); f.receipts([receipt(f.identity)]);
  f.ackHook(async () => ({ ok: true, generation: f.client.generation, value: { acknowledged: false } }));
  await f.reread(); expect(view(f.client, f.route).kind).toBe('held');
  f.readHook(async () => ({ ok: true, generation: f.client.generation, value: {} })); await f.reread();
  expect(view(f.client, f.route).kind).toBe('held');
  f.readHook(null); f.receipts([]); await f.reread();
  const route = await f.resume('replacement');
  expect(view(f.client, route).kind).toBe('mounting');
  expect(f.client.local.drafts[f.key]).toBe('🙂 final source');
});

test('let-go after dispatch is resolved by fresh IO and a newer complete read', async () => {
  const f = await fixture(); f.receipts([receipt(f.identity)]);
  f.ackHook(async () => { f.receipts([]); throw new ClientError('answer let go', 'superseded'); });
  await expect(f.reread()).rejects.toThrow('answer let go');
  expect(f.calls.some(call => call.op === 'r10Wake')).toBe(false);
  const writes = f.writes.length; f.ackHook(null); await f.reread();
  expect(f.writes).toHaveLength(writes); expect(f.client.local.drafts[f.key]).toBe('🙂 final source');
});

test('overlapping saves use independent attempts and stale completion cannot acknowledge', async () => {
  const f = await fixture(), source = receipt(f.identity), gate = deferred<void>(), entered = deferred<void>();
  f.receipts([source]); let first = true;
  f.writeHook(async () => { if (first) { first = false; entered.resolve(); await gate.promise; } });
  const older = f.reread(); await entered.promise;
  await f.reread();
  expect(f.writes).toHaveLength(2); expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
  gate.resolve(); await older;
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
});

test('an older pending read cannot resurrect a receipt already settled by a newer batch', async () => {
  const f = await fixture(), gate = deferred<unknown>(), entered = deferred<void>(), source = receipt(f.identity);
  f.readHook(async () => { entered.resolve(); return gate.promise; });
  const older = f.reread(); await entered.promise; f.readHook(null); f.receipts([source]); await f.reread();
  gate.resolve({ ok: true, generation: 3, value: { receipts: [source], reserved: [f.identity] } }); await older;
  expect(f.writes).toHaveLength(1); expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
});

for (const mutation of ['foreign', 'incarnation', 'destination', 'revision', 'terminal', 'range', 'count', 'zero-count',
  'oversize', 'envelope', 'duplicate', 'wrong-generation']) {
  test(`retains ${mutation} final source without acknowledging or changing content`, async () => {
    const f = await fixture(), source = receipt(f.identity);
    if (mutation === 'foreign') { source.identity = { ...source.identity, owner: 'arbitrary native owner' }; source.event.owner = source.identity.owner; }
    if (mutation === 'incarnation') mobileNewTaskDraftStore(f.client).records[f.key].createdAt = '2026-10-10T00:00:00Z';
    if (mutation === 'destination') mobileNewTaskDraftStore(f.client).records[f.key].projectId = 'elsewhere';
    if (mutation === 'revision') f.writeText('owned newer value');
    if (mutation === 'range') source.event.selection.end = source.event.value.length + 1;
    if (mutation === 'count') source.event.eventCount = Number.MAX_SAFE_INTEGER + 1;
    if (mutation === 'zero-count') source.event.eventCount = 0;
    if (mutation === 'oversize') source.event.value = 'x'.repeat(1_000_001);
    if (mutation === 'envelope') source.identity = { ...source.identity, mountId: 'another-envelope' };
    f.receipts(mutation === 'duplicate' ? [source, { ...source, event: { ...source.event, value: 'conflict' } }]
      : mutation === 'terminal' ? [{ ...source, event: { ...source.event, pendingCommand: { commandId: 'unresolved' } } }] : [source]);
    if (mutation === 'wrong-generation') f.readHook(async () => ({ ok: true, generation: 99, value: { receipts: [source], reserved: [f.identity] } }));
    const before = f.client.local.drafts[f.key]; await f.reread();
    expect(f.client.local.drafts[f.key]).toBe(before); expect(f.writes).toHaveLength(0);
    expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(0);
  });
}

test('unknown frozen custody stays retained when the same receipt later becomes retired', async () => {
  const f = await fixture(), source = receipt(f.identity);
  const frozen = { ...source, event: { ...source.event, kind: 'frozen', operationId: receiptId, operationRevision: 1 }, retired: false };
  f.receipts([frozen]); await f.reread();
  f.receipts([{ ...frozen, retired: true }]); await f.reread();
  expect(f.writes).toHaveLength(0); expect(f.client.local.drafts[f.key]).toBeUndefined();
  expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
});

test('hydration evidence gates reads and admission beyond shared preference loading', async () => {
  const client = new CountingClient(), calls: string[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch(topic) { calls.push(topic); }, async later(input) {
    const request = obj(input); calls.push(str(request.op)); return { ok: true, generation: 0, value: {} };
  } };
  await refresh(client, native, storage);
  expect(client.refreshes).toBe(1); expect(client.preferencesLoaded).toBe(true); expect(mobileDraftRecoveryHydrated(client)).toBe(false);
  expect(calls[0]).toBe('t3.composer-finalizations'); expect(calls).not.toContain('composerEditorFinalizations');
  expect(view(client, { owner: 'forged', visit: 'visit', location: '/new/draft', editorId: 'editor' }).kind).toBe('held');
});

test('persistent save failure requests one wake and explicit retry uses fresh IO', async () => {
  const f = await fixture(); f.receipts([receipt(f.identity)]); f.writeHook(async () => { throw Error('disk unavailable'); });
  await f.reread(); await f.reread();
  expect(f.calls.filter(call => call.op === 'r10Wake')).toHaveLength(1);
  f.writeHook(null);
  const freshCalls: Obj[] = [], fresh: Native = { available: true, watch(topic) { f.native.watch(topic); }, async later(input) {
    freshCalls.push(obj(input)); return f.native.later(input);
  } };
  await action(f.client, f.route, { kind: 'retry' }, fresh, f.storage);
  expect(freshCalls.map(call => call.action)).toEqual(['read', 'acknowledge']);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 final source');
});

test('same-incarnation admission stays blocked across transport generations until its exact receipt is acknowledged', async () => {
  const f = await fixture();
  const route = await f.resume('generation-four', 4);
  expect(view(f.client, route).kind).toBe('held');
  f.receipts([receipt(f.identity)]); await f.reread();
  const ready = await f.resume('after-recovery', 4), projection = view(f.client, ready);
  expect(projection.kind).toBe('mounting');
  expect(mounted(projection).owner).not.toBe(f.identity.owner);
  if (!projection.node) throw Error('missing replacement');
  expect(obj(obj(JSON.parse(projection.node.configuration)).document).value).toBe('🙂 final source');
});

test('eight acknowledged-ready mounts bound admission without evicting their sources', async () => {
  const f = await fixture(), identities = [f.identity];
  for (let index = 1; index < 8; index++) {
    const route = await f.open(`flow-${index}`), projection = view(f.client, route);
    expect(projection.kind).toBe('mounting');
    const identity = { ...mounted(projection), mountId: `mount-${index}` }; identities.push(identity);
    expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
  }
  const ninth = await f.open('ninth');
  expect(view(f.client, ninth)).toMatchObject({ kind: 'held', node: null });
  f.receipts(identities.map((identity, index) => ({ ...receipt(identity, `saved-${index}`),
    receiptId: `${String(index).padStart(8, '0')}-1111-4111-8111-111111111111` })));
  const saves = f.writes.length; await f.reread();
  expect(f.writes.length - saves).toBe(8);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(8);
});

test('an unacknowledged seed can be replaced without inventing final-source authority', async () => {
  const f = await fixture(), route = await f.open('unacknowledged'), key = f.client.draftKey;
  const seed = view(f.client, route); expect(seed.kind).toBe('mounting');
  const revisited = await f.resume('new-visit', 3, key), replacement = view(f.client, revisited);
  expect(replacement.kind).toBe('mounting');
  expect(mounted(replacement).renderEpoch).not.toBe(mounted(seed).renderEpoch);
  expect(await action(f.client, revisited, { kind: 'observe', payload: event(mounted(seed), 'stale ready') }, f.native, f.storage))
    .toMatchObject({ accepted: false });
});

test('an older live save cannot clear a newer synchronous observation before its fresh save', async () => {
  const f = await fixture(), gate = deferred<void>(), entered = deferred<void>();
  f.writeHook(async () => { entered.resolve(); await gate.promise; });
  const older = action(f.client, f.route, { kind: 'observe', payload: event(f.identity, 'first', 2, 'text') }, f.native, f.storage);
  await entered.promise;
  expect(view(f.client, f.route, event(f.identity, 'newest', 3, 'text')).kind).toBe('editing');
  gate.resolve(); await older; f.writeHook(null);
  expect(obj(f.disk().drafts)[f.key]).toBe('first');
  await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, 'newest', 3, 'text') }, f.native, f.storage);
  expect(obj(f.disk().drafts)[f.key]).toBe('newest'); expect(f.writes).toHaveLength(2);
});

test('lost hydration while a read is pending prevents publication and acknowledgment', async () => {
  const f = await fixture(), gate = deferred<unknown>(), entered = deferred<void>();
  f.readHook(async () => { entered.resolve(); return gate.promise; });
  const reading = f.reread(); await entered.promise;
  f.client.local = { ...f.client.local };
  gate.resolve({ ok: true, generation: 3, value: { receipts: [receipt(f.identity)], reserved: [f.identity] } }); await reading;
  expect(f.writes).toHaveLength(0); expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
});

test('retired composition preserves exact UTF16 source and acknowledgment uses the generation current after save', async () => {
  const f = await fixture(), source = receipt(f.identity, '🙂 e\u0301 unfinished'), gate = deferred<void>(), entered = deferred<void>();
  source.event.composing = true; f.receipts([source]);
  f.writeHook(async () => { entered.resolve(); await gate.promise; });
  const saving = f.reread(); await entered.promise; f.client.generation = 4; gate.resolve(); await saving;
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 e\u0301 unfinished');
  expect(f.calls.filter(call => call.op === 'composerEditorFinalizations').map(call => call.generation)).toEqual([3, 4]);
});

test('source identity changes during persistence retain the receipt without acknowledging the replacement incarnation', async () => {
  const f = await fixture(), gate = deferred<void>(), entered = deferred<void>(); f.receipts([receipt(f.identity)]);
  f.writeHook(async () => { entered.resolve(); await gate.promise; });
  const saving = f.reread(); await entered.promise;
  mobileNewTaskDraftStore(f.client).records[f.key].createdAt = '2026-10-11T00:00:00Z';
  gate.resolve(); await saving;
  expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
  expect(view(f.client, await f.resume('replacement-incarnation')).kind).toBe('mounting');
});

for (const source of ['photos', 'files'] as const) {
  test(`${source} saves the exact frozen source and keeps the same disabled node after ACK`, async () => {
    const f = await fixture(); picker(f); const before = view(f.client, f.route);
    const result = await action(f.client, f.route, { kind: 'pick', source }, f.native, f.storage);
    expect(result.accepted).toBe(true);
    expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toEqual([{
      op: 'composerEditorHandoff', generation: 3, identity: f.identity, action: 'freeze',
      operationId, operationRevision: 1, retireUndo: false,
    }]);
    expect(f.order.indexOf('save')).toBeLessThan(f.order.indexOf('composerEditorFinalizations:acknowledge'));
    expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
    expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
    const frozen = view(f.client, f.route);
    expect(frozen).toMatchObject({ kind: 'frozen', editable: false, node: { key: before.node?.key } });
    expect(config(frozen)).toMatchObject({ active: true, editable: false, readOnly: false,
      document: { value: '🙂 picked here', selection: { start: 3, end: 9 } } });
    expect(f.calls.filter(call => call.action === 'acknowledge').map(call => call.receiptId)).toEqual([receiptId]);
    await f.reread(); await f.reread();
    expect(view(f.client, f.route).kind).toBe('frozen'); expect(f.writes).toHaveLength(1);
    expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toHaveLength(1);
    expect(f.calls.some(call => call.op === 'composerPickerIntake' || call.action === 'resume')).toBe(false);
    expect(await action(f.client, f.route, { kind: 'pick', source }, f.native, f.storage)).toMatchObject({ accepted: false });
    expect(await action(f.client, f.route, { kind: 'observe', payload: event(f.identity, 'late text', 5, 'text') }, f.native, f.storage))
      .toMatchObject({ accepted: false });
    expect(f.client.local.drafts[f.key]).toBe('🙂 picked here');
  });
}

test('reserves one operation before UUID allocation while keeping freeze-eligible props', async () => {
  const f = await fixture(), pick = picker(f), gate = deferred<string[]>(), entered = deferred<void>();
  f.idsHook(async () => { entered.resolve(); return gate.promise; });
  const pending = pick(); await entered.promise;
  expect(config(view(f.client, f.route))).toMatchObject({ active: true, editable: true, readOnly: false });
  expect(await pick()).toMatchObject({ accepted: false });
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(1);
  expect(f.calls.some(call => call.op === 'composerEditorHandoff')).toBe(false);
  gate.resolve([operationId]); await pending;
  expect(view(f.client, f.route).kind).toBe('frozen');
});

test('a fresh Root allocation supersedes a late UUID reply without allocating a second operation', async () => {
  const f = await fixture(), pick = picker(f), gate = deferred<string[]>(), entered = deferred<void>();
  f.idsHook(async () => { entered.resolve(); return gate.promise; });
  const pending = pick(); await entered.promise;
  f.idsHook(async () => [operationId]); await f.reread();
  gate.resolve(['33333333-3333-4333-8333-333333333333']); await pending;
  expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toHaveLength(1);
  expect(view(f.client, f.route).kind).toBe('frozen'); expect(f.writes).toHaveLength(1);
});

for (const stage of ['allocation', 'freeze'] as const) {
  test(`superseded ${stage} failure leaves another acknowledged editor usable`, async () => {
    const f = await fixture(), pick = picker(f), gate = deferred<never>(), entered = deferred<void>();
    if (stage === 'allocation') f.idsHook(async () => { entered.resolve(); return gate.promise; });
    else f.handoffHook(async () => { entered.resolve(); return gate.promise; });
    const pending = pick(); await entered.promise;
    picker(f); await f.reread();
    const route = await f.open('unrelated-editor'), seed = view(f.client, route);
    const identity = { ...mounted(seed), mountId: 'unrelated-mount' };
    expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
    gate.reject(new Error('old request failed')); await pending;
    expect(view(f.client, route)).toMatchObject({ kind: 'editing', node: { key: seed.node?.key } });
    expect(await action(f.client, route, { kind: 'observe', payload: event(identity, 'still editable', 2, 'text') }, f.native, f.storage))
      .toMatchObject({ accepted: true, message: '' });
    expect(f.client.local.drafts[f.client.draftKey]).toBe('still editable');
  });
}

test('allocation failure after departure leaves the newly selected draft usable', async () => {
  const f = await fixture(), pick = picker(f), gate = deferred<string[]>(), entered = deferred<void>();
  f.idsHook(async () => { entered.resolve(); return gate.promise; });
  const pending = pick(); await entered.promise;
  f.idsHook(async () => [operationId]);
  const route = await f.open('departed-allocation'), seed = view(f.client, route);
  const identity = { ...mounted(seed), mountId: 'departed-allocation-mount' };
  expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
  gate.reject(new Error('departed request failed')); await pending;
  expect(view(f.client, route)).toMatchObject({ kind: 'editing', node: { key: seed.node?.key } });
  expect(f.calls.some(call => call.op === 'composerEditorHandoff')).toBe(false);
});

test('source save failure retries actual persistence with fresh IO and preserves latest draft settings', async () => {
  const f = await fixture(), pick = picker(f); f.writeHook(async () => { throw Error('disk unavailable'); });
  await pick();
  expect(f.client.local.drafts[f.key]).toBe('🙂 picked here');
  expect(view(f.client, f.route).kind).toBe('frozen');
  expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
  const choices = { providerId: 'codex', modelId: 'latest-choice', modelOptions: [] };
  expect(mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices)).toBe(true);
  f.writeHook(null);
  const calls: Obj[] = [], fresh: Native = { available: true, watch() {}, async later(input) {
    calls.push(obj(input)); return f.native.later(input);
  } };
  await action(f.client, f.route, { kind: 'retry' }, fresh, f.storage);
  expect(calls.map(call => call.action)).toEqual(['read', 'acknowledge']);
  const saved = obj(obj(f.disk().mobileNewTaskDrafts).records)[f.key];
  expect(obj(saved).choices).toEqual(choices);
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
  expect(f.writes).toHaveLength(2); expect(view(f.client, f.route).kind).toBe('frozen');
});

test('lost freeze reply recovers captured source after navigation and transport replacement', async () => {
  const f = await fixture(), pick = picker(f);
  f.handoffHook(async request => {
    const source = frozenSource(f, request); f.receipts([{ ...source, retired: true }]);
    f.detach(); throw new ClientError('answer let go', 'superseded');
  });
  await expect(pick()).rejects.toThrow('answer let go'); await f.reread();
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  expect(f.client.local.drafts['other:other-thread']).toBe('unrelated');
  expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toHaveLength(1);
  expect(f.calls.filter(call => call.action === 'acknowledge').map(call => call.generation)).toEqual([4]);
  const reopened = await f.resume('captured-operation');
  expect(view(f.client, reopened).kind).toBe('held');
  expect(f.calls.some(call => call.action === 'resume')).toBe(false);
});

test('an uncertain freeze retries only the exact request when a complete read has no receipt', async () => {
  const f = await fixture(), pick = picker(f); let first = true;
  f.handoffHook(async request => {
    if (first) { first = false; throw Error('reply lost before native dispatch'); }
    const source = frozenSource(f, request); f.receipts([source]);
    return { ok: true, generation: request.generation, value: { receipt: source } };
  });
  await pick(); await f.reread();
  const freezes = f.calls.filter(call => call.op === 'composerEditorHandoff');
  expect(freezes).toHaveLength(2); expect(freezes[0]).toEqual(freezes[1]);
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(1);
  expect(f.writes).toHaveLength(1); expect(view(f.client, f.route).kind).toBe('frozen');
});

test('an uncertain ACK is reconciled by absence without releasing or replaying known work', async () => {
  const f = await fixture(), pick = picker(f);
  f.ackHook(async () => { f.receipts([]); throw new ClientError('ack let go', 'superseded'); });
  await expect(pick()).rejects.toThrow('ack let go');
  f.writeText('newer owned content'); f.ackHook(null); await f.reread();
  expect(f.client.local.drafts[f.key]).toBe('newer owned content'); expect(f.writes).toHaveLength(1);
  expect(view(f.client, f.route).kind).toBe('frozen');
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
  expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toHaveLength(1);
});

test('retirement only advances a known frozen receipt and cannot resurrect it after ACK', async () => {
  const f = await fixture(), pick = picker(f), sources: ReturnType<typeof frozenSource>[] = [];
  f.handoffHook(async request => {
    const source = frozenSource(f, request); sources.push(source); f.receipts([source]);
    return { ok: true, generation: request.generation, value: { receipt: source } };
  });
  f.writeHook(async () => { throw Error('disk unavailable'); }); await pick();
  const captured = sources[0];
  if (!captured) throw Error('missing source');
  f.receipts([{ ...captured, retired: true }]); f.writeHook(null); await f.reread();
  expect(view(f.client, f.route).kind).toBe('held');
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1);
  const writes = f.writes.length;
  f.receipts([captured]); await f.reread();
  expect(view(f.client, f.route).kind).toBe('held'); expect(f.writes).toHaveLength(writes);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
});

for (const changed of ['route', 'generation', 'threadEpoch', 'revision']) {
  test(`${changed} change before UUID return revokes the undispatched producer`, async () => {
    const f = await fixture(), pick = picker(f), gate = deferred<string[]>(), entered = deferred<void>();
    f.idsHook(async () => { entered.resolve(); return gate.promise; });
    const pending = pick(); await entered.promise;
    if (changed === 'route') f.detach();
    if (changed === 'generation') f.client.generation++;
    if (changed === 'threadEpoch') f.client.threadEpoch++;
    if (changed === 'revision') f.writeText('newer owned content');
    gate.resolve([operationId]); await pending;
    expect(f.calls.some(call => call.op === 'composerEditorHandoff')).toBe(false);
    expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false); expect(f.writes).toHaveLength(0);
  });
}

for (const invalid of ['operationId', 'operationRevision', 'identity', 'eventCount', 'generation']) {
  test(`rejects a frozen reply with mismatched ${invalid}`, async () => {
    const f = await fixture(), pick = picker(f);
    f.handoffHook(async request => {
      const source = frozenSource(f, request);
      if (invalid === 'operationId') source.event.operationId = receiptId;
      if (invalid === 'operationRevision') source.event.operationRevision = 42;
      if (invalid === 'identity') source.identity = { ...source.identity, mountId: 'foreign-mount' };
      if (invalid === 'eventCount') source.event.eventCount = 1;
      return { ok: true, generation: invalid === 'generation' ? 99 : request.generation, value: { receipt: source } };
    });
    await pick();
    expect(f.client.local.drafts[f.key]).toBeUndefined(); expect(f.writes).toHaveLength(0);
    expect(f.calls.some(call => call.action === 'acknowledge')).toBe(false);
  });
}

test('confirmed predecessor release replaces one unacknowledged factory seed and rejects its late ready', async () => {
  const f = await fixture(), route = await f.open('blocked-factory'), seed = view(f.client, route);
  expect(seed.kind).toBe('mounting');
  const oldIdentity = mounted(seed);
  f.receipts([receipt(f.identity)]); await f.reread();
  const replacement = view(f.client, route);
  expect(replacement.kind).toBe('mounting'); expect(replacement.node?.key).not.toBe(seed.node?.key);
  await f.reread(); expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
  expect(await action(f.client, route, { kind: 'observe', payload: event(oldIdentity, '') }, f.native, f.storage))
    .toMatchObject({ accepted: false });
  const replacementIdentity = { ...mounted(replacement), mountId: 'replacement-mount' };
  expect(view(f.client, route, event(replacementIdentity, '')).kind).toBe('editing');
  f.receipts([receipt(f.identity)]); await f.reread();
  expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
});

test('uncertain predecessor ACK replaces a pending seed only after a newer complete absent read', async () => {
  const f = await fixture(), route = await f.open('blocked-factory'), seed = view(f.client, route);
  f.receipts([receipt(f.identity)]);
  f.ackHook(async () => { throw new ClientError('ack let go', 'superseded'); });
  await expect(f.reread()).rejects.toThrow('ack let go');
  expect(view(f.client, route).node?.key).toBe(seed.node?.key);
  f.receipts([]); f.ackHook(null); await f.reread();
  const replacement = view(f.client, route);
  expect(replacement.node?.key).not.toBe(seed.node?.key);
  await f.reread(); expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
});

test('ACK of a live frozen slot does not invalidate another pending factory seed', async () => {
  const f = await fixture(), pick = picker(f), gate = deferred<void>(), entered = deferred<void>();
  let first = true;
  f.writeHook(async () => { if (first) { first = false; entered.resolve(); await gate.promise; } });
  const pending = pick(); await entered.promise;
  const route = await f.open('other-factory'), seed = view(f.client, route);
  gate.resolve(); await pending; f.writeHook(null);
  expect(view(f.client, route).node?.key).toBe(seed.node?.key);
  await f.reread(); expect(view(f.client, route).node?.key).toBe(seed.node?.key);
});

test('a complete reservation read distinguishes live saved custody from released source without completing the picker', async () => {
  const f = await fixture(); await picker(f)();
  await f.reread(); expect(view(f.client, f.route).kind).toBe('frozen');
  const writes = f.writes.length, acknowledgments = f.calls.filter(call => call.action === 'acknowledge').length;
  f.reserved([]); await f.reread();
  expect(view(f.client, f.route)).toMatchObject({ kind: 'held', node: null });
  expect(f.writes).toHaveLength(writes);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(acknowledgments);
  expect(obj(f.disk().drafts)[f.key]).toBe('🙂 picked here');
  f.reserved([f.identity]); await f.reread();
  expect(view(f.client, f.route).node).toBeNull();
  expect(f.writes).toHaveLength(writes);
  expect(view(f.client, await f.resume('same-draft-after-release')).kind).toBe('held');
});

test('saved-slot release replaces an existing refused seed once while keeping picker work held', async () => {
  const f = await fixture(); await picker(f)();
  const route = await f.open('factory-after-saved-freeze'), seed = view(f.client, route);
  const writes = f.writes.length;
  expect(seed.kind).toBe('mounting');
  await f.reread(); expect(view(f.client, route).node?.key).toBe(seed.node?.key);
  f.reserved([]); await f.reread();
  const replacement = view(f.client, route);
  expect(replacement.node?.key).not.toBe(seed.node?.key);
  await f.reread(); expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
  const identity = { ...mounted(replacement), mountId: 'acknowledged-replacement' };
  expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
  f.reserved([f.identity]); await f.reread(); f.reserved([]); await f.reread();
  expect(view(f.client, route).node?.key).toBe(replacement.node?.key);
  expect(f.writes).toHaveLength(writes);
});

test('a read dispatched before freeze and ACK cannot release the newly saved operation', async () => {
  const f = await fixture(), gate = deferred<unknown>(), entered = deferred<void>();
  f.readHook(async () => { entered.resolve(); return gate.promise; });
  const reading = f.reread(); await entered.promise;
  await picker(f)();
  gate.resolve({ ok: true, generation: 3, value: { receipts: [], reserved: [] } }); await reading;
  expect(view(f.client, f.route).kind).toBe('frozen');
  f.readHook(null); await f.reread(); expect(view(f.client, f.route).kind).toBe('frozen');
  f.reserved([]); await f.reread(); expect(view(f.client, f.route).kind).toBe('held');
  expect(f.writes).toHaveLength(1);
});

test('a newer released snapshot wins over an older snapshot retaining the saved slot', async () => {
  const f = await fixture(); await picker(f)();
  const gate = deferred<unknown>(), entered = deferred<void>();
  f.readHook(async () => { entered.resolve(); return gate.promise; });
  const older = f.reread(); await entered.promise;
  f.readHook(null); f.reserved([]); await f.reread();
  expect(view(f.client, f.route).kind).toBe('held');
  gate.resolve({ ok: true, generation: 3, value: { receipts: [], reserved: [f.identity] } }); await older;
  expect(view(f.client, f.route).kind).toBe('held'); expect(f.writes).toHaveLength(1);
});

for (const malformed of ['missing', 'overflow', 'duplicate', 'duplicate-mount', 'identity', 'receipt-membership']) {
  test(`${malformed} reservation snapshot cannot release saved work or a refused seed`, async () => {
    const f = await fixture(); await picker(f)();
    const route = await f.open('snapshot-validation'), seed = view(f.client, route);
    const writes = f.writes.length;
    const value: Record<string, unknown> = { receipts: [], reserved: [] };
    if (malformed === 'missing') delete value.reserved;
    if (malformed === 'overflow') value.reserved = Array.from({ length: 9 }, (_, index) => ({ ...f.identity, mountId: `mount-${index}` }));
    if (malformed === 'duplicate') value.reserved = [f.identity, f.identity];
    if (malformed === 'duplicate-mount') value.reserved = [f.identity, { ...f.identity, renderEpoch: 'another-epoch' }];
    if (malformed === 'identity') value.reserved = [{ ...f.identity, mountId: '' }];
    if (malformed === 'receipt-membership') value.receipts = [receipt(f.identity)];
    f.readHook(async () => ({ ok: true, generation: f.client.generation, value })); await f.reread();
    f.readHook(null); await f.reread();
    expect(view(f.client, route).node?.key).toBe(seed.node?.key);
    expect(f.writes).toHaveLength(writes);
    f.reserved([]); await f.reread();
    expect(view(f.client, route).node?.key).not.toBe(seed.node?.key);
  });
}

test('one complete read reconciles an uncertain ACK and released reservation together', async () => {
  const f = await fixture(), pick = picker(f);
  f.ackHook(async () => {
    f.receipts([]); f.reserved([]);
    throw new ClientError('ACK reply lost', 'superseded');
  });
  await expect(pick()).rejects.toThrow('ACK reply lost');
  f.writeText('later owned text'); f.ackHook(null); await f.reread();
  expect(view(f.client, f.route)).toMatchObject({ kind: 'held', node: null });
  expect(f.client.local.drafts[f.key]).toBe('later owned text');
  expect(f.writes).toHaveLength(1);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
  expect(await action(f.client, f.route, { kind: 'pick', source: 'files' }, f.native, f.storage)).toMatchObject({ accepted: false });
});

test('released snapshot supersedes an in-flight ACK without reopening or replaying work', async () => {
  const f = await fixture(), pick = picker(f), entered = deferred<void>(), gate = deferred<unknown>();
  f.ackHook(async () => { entered.resolve(); return gate.promise; });
  const pending = pick(); await entered.promise;
  f.receipts([]); f.reserved([]); f.ackHook(null); await f.reread();
  expect(view(f.client, f.route)).toMatchObject({ kind: 'held', node: null });
  gate.resolve({ ok: true, generation: 3, value: { acknowledged: true } }); await pending;
  expect(view(f.client, f.route)).toMatchObject({ kind: 'held', node: null });
  expect(f.writes).toHaveLength(1);
  expect(f.calls.filter(call => call.action === 'acknowledge')).toHaveLength(1);
  expect(f.calls.filter(call => call.op === 'composerEditorHandoff')).toHaveLength(1);
});

for (const kind of ['files', 'photos'] as const) test(`${kind} publishes once from the final frozen UTF16 caret and finishes from actual saved bytes`, async () => {
  const f = await fixture(), pick = continuation(f, [kind === 'files' ? pickedFile : pickedImage]);
  await pick(kind);
  expect(intakeCalls(f, 'pick')).toHaveLength(1);
  const request = intakeCalls(f, 'pick')[0];
  expect(request).toMatchObject({ generation: 3, operationId, identity: f.identity,
    document: { incarnation: mobileNewTaskDraftLookup(f.client, f.key)?.createdAt, revision: 1 }, source: kind, remaining: 100 });
  const finish = intakeCalls(f, 'finish')[0], after = obj(obj(finish.publication).after);
  expect(after.record).toEqual(savedRecord(f)); expect(after.text).toBe(obj(f.disk().drafts)[f.key]);
  expect(String(after.text).startsWith('🙂 ')).toBe(true); expect(String(after.text).endsWith('here')).toBe(true);
  expect(after.images).toEqual(obj(f.disk().snapshotDrafts)[f.key]);
  expect(after.files).toEqual(f.disk().composerFiles);
  expect(after.attachmentOrder).toEqual([pickedId]);
  expect(obj(finish.publication)).toMatchObject({ acceptedIds: [pickedId], discardedIds: [] });
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(2);
  expect(f.order.indexOf('composerEditorFinalizations:acknowledge')).toBeLessThan(f.order.indexOf('composerPickerIntake:pick'));
  expect(f.order.lastIndexOf('save')).toBeLessThan(f.order.indexOf('composerPickerIntake:finish'));
  const holds = intakeCalls(f, 'hold'); expect(holds).toHaveLength(kind === 'files' ? 1 : 0);
  if (kind === 'files') expect(holds[0]).toMatchObject({ operationId, generation: 3, request: { target: { kind: 'new-task', draftKey: f.key } } });
  const writes = f.writes.length; await f.reread(); await f.reread();
  expect(f.writes).toHaveLength(writes); expect(intakeCalls(f, 'finish')).toHaveLength(1);
  expect(view(f.client, f.route).kind).toBe('frozen');
  expect(f.calls.some(call => call.action === 'resume')).toBe(true);
  expect(f.calls.some(call => call.action === 'release')).toBe(false);
});

test('a shelf-only image keeps empty source and saves the batch context presence exactly', async () => {
  const f = await fixture(), pick = continuation(f, [pickedImage], ''); await pick('photos');
  const finish = intakeCalls(f, 'finish')[0], after = obj(obj(finish.publication).after);
  expect(after.text).toBe(''); expect(obj(after.record).context).toEqual({ version: 1, records: [] });
  expect(after.images).toEqual([{ id: pickedId, name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 234 }]);
  expect(after.record).toEqual(savedRecord(f)); expect(f.calls.some(call => call.action === 'hold')).toBe(false);
});

test('selection replacement holds the old attached bytes before publication and retains them for native Undo', async () => {
  const f = await fixture(), oldText = contextLink('file', oldId, 'old');
  setDraftFiles(f.client.local, [{ id: oldId, contextId: oldId, draftKey: f.key, environmentId: 'env', name: 'old.txt',
    mimeType: 'text/plain', sizeBytes: 17, source: 'attached', status: 'staged', attachmentId: '' }]);
  const pick = continuation(f, [pickedFile], oldText, { start: 0, end: oldText.length }); await pick();
  expect(f.calls.filter(call => call.op === 'composerFileHold' && call.action === 'acquire')).toHaveLength(1);
  expect(f.order.indexOf('composerFileHold:acquire')).toBeLessThan(f.order.indexOf('composerPickerIntake:hold'));
  expect(draftFiles(f.client.local).map(file => file.id)).toEqual([pickedId]);
  expect(mobileNewTaskDraftStore(f.client).fileReleases).toContain(oldId);
  expect(f.calls.some(call => call.action === 'release')).toBe(false);
  expect(intakeCalls(f, 'finish')).toHaveLength(1);
});

test('the confirmed final serialization preserves latest choices, branch and raw extensions', async () => {
  const f = await fixture(), pick = continuation(f), choices = { providerId: 'codex', modelId: 'latest', modelOptions: [] };
  f.holdHook(async request => {
    expect(mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices)).toBe(true);
    const draft = mobileNewTaskDraftStore(f.client).records[f.key];
    Object.assign(draft, { branchChoice: { kind: 'explicit', envMode: 'local', branch: 'current-branch', worktreePath: '' }, futureRecord: { keep: ['exact', 9] } });
    draft.revision++;
    return heldReply(request);
  });
  await pick();
  const after = obj(obj(intakeCalls(f, 'finish')[0].publication).after);
  expect(after.record).toEqual(savedRecord(f));
  expect(after.record).toMatchObject({ choices, branchChoice: { kind: 'explicit', envMode: 'local', branch: 'current-branch', worktreePath: '' }, futureRecord: { keep: ['exact', 9] } });
});

test('failed publication save retries fresh IO without applying the batch again', async () => {
  const f = await fixture(), pick = continuation(f); let writes = 0;
  f.writeHook(async () => { if (++writes === 2) throw Error('publication disk failure'); });
  await pick(); expect(intakeCalls(f, 'finish')).toHaveLength(0);
  const text = f.client.local.drafts[f.key], revision = mobileNewTaskDraftLookup(f.client, f.key)?.revision;
  expect(text).not.toBe(obj(f.disk().drafts)[f.key]);
  const choices = { providerId: 'codex', modelId: 'after-failure', modelOptions: [] };
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices); f.writeHook(null);
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(f.client.local.drafts[f.key]).toBe(text); expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(revision);
  expect(intakeCalls(f, 'pick')).toHaveLength(1); expect(intakeCalls(f, 'hold')).toHaveLength(1);
  expect(intakeCalls(f, 'finish')).toHaveLength(1); expect(savedRecord(f).choices).toEqual(choices);
});

test('an uncertain pick reconciles status with the same original generation after departure', async () => {
  const f = await fixture(), pick = continuation(f);
  f.intakeHook(async request => { if (request.action === 'pick') throw Error('lost pick'); return intakeReply(request, request.action === 'cancel' ? 'cancelled' : 'staged', [pickedFile]); });
  await pick(); f.detach(); await f.reread();
  expect(intakeCalls(f, 'pick')).toHaveLength(1);
  expect(intakeCalls(f, 'status')[0]).toMatchObject({ generation: 3, operationId, identity: f.identity });
  expect(intakeCalls(f, 'cancel')[0]).toMatchObject({ generation: 3, operationId, identity: f.identity });
  expect(intakeCalls(f, 'hold')).toHaveLength(0); expect(intakeCalls(f, 'finish')).toHaveLength(0);
  expect(f.client.local.drafts[f.key]).toBe('🙂 picked here');
  expect(f.client.local.drafts['other:other-thread']).toBe('unrelated');
});

test('lost pick dispatch replays the immutable request only while its original editor remains live', async () => {
  const f = await fixture(), pick = continuation(f); let picks = 0;
  f.intakeHook(async request => {
    if (request.action === 'pick' && ++picks === 1) throw Error('not delivered');
    return intakeReply(request, request.action === 'status' ? 'absent' : request.action === 'finish' ? 'finished' : 'staged',
      request.action === 'status' ? [] : [pickedFile]);
  });
  await pick(); await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  const requests = intakeCalls(f, 'pick'); expect(requests).toHaveLength(2); expect(requests[1]).toEqual(requests[0]);
  expect(intakeCalls(f, 'finish')).toHaveLength(1); expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(2);
});

test('absent status requires independently released source custody before ending unpublished work', async () => {
  const f = await fixture(), pick = continuation(f);
  f.intakeHook(async request => { if (request.action === 'pick') throw Error('unknown'); return intakeReply(request, 'absent'); });
  await pick(); f.detach(); await f.reread();
  expect(intakeCalls(f, 'status')).toHaveLength(1); expect(intakeCalls(f, 'cancel')).toHaveLength(0);
  f.reserved([]); await f.reread(); const count = intakeCalls(f, 'status').length; await f.reread();
  expect(intakeCalls(f, 'status')).toHaveLength(count); expect(count).toBe(2);
  expect(view(f.client, await f.resume('absent-terminal')).kind).toBe('held');
});

for (const departure of ['route', 'generation', 'source', 'inventory', 'epoch'] as const)
  test(`${departure} departure during issued hold prevents publication and cancels the original operation`, async () => {
    const f = await fixture(), pick = continuation(f);
    f.holdHook(async request => {
      if (request.action === 'hold') {
        if (departure === 'route') f.detach();
        else if (departure === 'generation') f.client.generation++;
        else if (departure === 'epoch') f.client.threadEpoch++;
        else if (departure === 'source') f.writeText('newer source');
        else f.client.local.snapshotDrafts[f.key] = [{ id: oldId, name: 'new.jpg', mimeType: 'image/jpeg', sizeBytes: 1 }];
      }
      return heldReply(request);
    });
    await pick();
    expect(intakeCalls(f, 'finish')).toHaveLength(0); expect(intakeCalls(f, 'cancel')).toHaveLength(1);
    expect(intakeCalls(f, 'cancel')[0]).toMatchObject({ generation: 3, identity: f.identity, operationId });
    expect(draftFiles(f.client.local)).toHaveLength(0);
    expect(f.client.local.drafts[f.key]).toBe(departure === 'source' ? 'newer source' : '🙂 picked here');
    expect(f.calls.filter(call => call.action === 'release')).toHaveLength(1);
  });

test('published adoption saves and finishes its original stable destination after route and generation departure', async () => {
  const f = await fixture(), pick = continuation(f); let saves = 0;
  f.writeHook(async () => { if (++saves === 2) f.detach(); }); await pick();
  const finishes = intakeCalls(f, 'finish'); expect(finishes).toHaveLength(1);
  expect(finishes[0]).toMatchObject({ generation: 3, operationId, identity: f.identity });
  expect(obj(obj(finishes[0].publication).after).record).toEqual(savedRecord(f));
  expect(f.client.local.drafts['other:other-thread']).toBe('unrelated');
  expect(intakeCalls(f, 'cancel')).toHaveLength(0);
});

test('uncertain finish retains its exact request across current settings changes and reconnect', async () => {
  const f = await fixture(), pick = continuation(f); let finishes = 0;
  f.intakeHook(async request => {
    if (request.action === 'finish' && ++finishes === 1) throw Error('finish reply lost');
    return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
  });
  await pick(); const before = intakeCalls(f, 'finish')[0], writes = f.writes.length;
  const choices = { providerId: 'codex', modelId: 'newer-disk', modelOptions: [] };
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices); await f.client.persist(f.storage);
  f.detach(); await f.reread();
  expect(intakeCalls(f, 'finish')).toEqual([before, before]);
  expect(f.writes).toHaveLength(writes + 1); expect(savedRecord(f).choices).toEqual(choices);
  expect(intakeCalls(f, 'cancel')).toHaveLength(0); expect(intakeCalls(f, 'status')).toHaveLength(0);
});

test('a native saved-disk conflict retains immutable finish without rewriting newer settings', async () => {
  const f = await fixture(), pick = continuation(f);
  f.intakeHook(async request => { if (request.action === 'finish') throw Error('saved publication mismatch'); return intakeReply(request, 'staged', [pickedFile]); });
  await pick(); const before = intakeCalls(f, 'finish')[0], writes = f.writes.length;
  const choices = { providerId: 'codex', modelId: 'keep-latest', modelOptions: [] };
  mobileNewTaskDraftChoicesUpdate(f.client, f.key, choices); await f.client.persist(f.storage);
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(intakeCalls(f, 'finish')).toEqual([before, before]); expect(f.writes).toHaveLength(writes + 1);
  expect(savedRecord(f).choices).toEqual(choices); expect(view(f.client, f.route).kind).toBe('frozen');
});

for (const outcome of ['empty', 'all-rejected'] as const) test(`${outcome} intake legitimately cancels without inventing a saved record`, async () => {
  const f = await fixture(), text = outcome === 'empty' ? '' : 'x'.repeat(1_000_000);
  const pick = continuation(f, outcome === 'empty' ? [] : [pickedFile], text); await pick();
  expect(intakeCalls(f, 'finish')).toHaveLength(0); expect(intakeCalls(f, 'cancel')).toHaveLength(1);
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(1); expect(f.writes).toHaveLength(1);
  if (outcome === 'empty') expect(obj(obj(f.disk().mobileNewTaskDrafts).records)[f.key]).toBeUndefined();
  expect(f.client.local.drafts[f.key]).toBe(text); expect(draftFiles(f.client.local)).toHaveLength(0);
});

for (const malformed of ['duplicate', 'foreign-owner', 'changed-finish', 'wrong-lane'] as const)
  test(`${malformed} native inventory cannot invent an accepted terminal`, async () => {
    const f = await fixture(), pick = continuation(f);
    f.intakeHook(async request => {
      const reply = intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
      if (malformed === 'duplicate') reply.value.files = [pickedFile, pickedFile];
      if (malformed === 'foreign-owner') reply.value.identity = { ...f.identity, mountId: 'foreign' };
      if (request.action === 'finish' && malformed === 'changed-finish') reply.value.files = [{ ...pickedFile, name: 'different.md' }];
      if (request.action === 'finish' && malformed === 'wrong-lane') reply.value.files = [pickedImage];
      return reply;
    });
    await pick(); await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
    expect(view(f.client, f.route).kind).toBe('frozen'); expect(f.calls.some(call => call.action === 'resume')).toBe(false);
    if (malformed === 'duplicate' || malformed === 'foreign-owner') expect(intakeCalls(f, 'finish')).toHaveLength(0);
    else expect(intakeCalls(f, 'finish')).toHaveLength(2);
  });

for (const phase of ['source-save', 'source-ack', 'pick', 'finish'] as const)
  test(`superseded ${phase} rejection cannot disable an unrelated editor after newer recovery succeeds`, async () => {
    const f = await fixture(), pick = continuation(f), pending = deferred<never>(), entered = deferred<void>();
    if (phase === 'source-save') f.writeHook(async () => { entered.resolve(); await pending.promise; });
    if (phase === 'source-ack') f.ackHook(async () => { entered.resolve(); return pending.promise; });
    if (phase === 'pick' || phase === 'finish') f.intakeHook(async request => {
      if (request.action === phase) { entered.resolve(); return pending.promise; }
      return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
    });
    const older = pick(); await entered.promise;
    f.writeHook(null); f.ackHook(null);
    f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]));
    await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
    const route = await f.open(`new-${phase}`), seed = view(f.client, route), identity = { ...mounted(seed), mountId: `new-${phase}` };
    expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
    const writes = f.writes.length, finishes = intakeCalls(f, 'finish').length;
    pending.reject(Error('superseded failure')); await older;
    expect(view(f.client, route).kind).toBe('editing'); expect(f.writes).toHaveLength(writes);
    expect(intakeCalls(f, 'finish')).toHaveLength(finishes);
  });

for (const stage of ['pick', 'hold', 'save', 'finish'] as const) test(`let-go ${stage} retains plain work for fresh invocation recovery`, async () => {
  const f = await fixture(), pick = continuation(f), gone = new ClientError('answer ended', 'superseded');
  if (stage === 'pick' || stage === 'finish') f.intakeHook(async request => {
    if (request.action === stage) throw gone;
    return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
  });
  if (stage === 'hold') f.holdHook(async () => { throw gone; });
  if (stage === 'save') { let writes = 0; f.writeHook(async () => { if (++writes === 2) throw gone; }); }
  await expect(pick()).rejects.toBe(gone);
  f.writeHook(null); f.holdHook(async request => heldReply(request));
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]));
  const calls: Obj[] = [], native: Native = { available: true, watch() {}, async later(input) { calls.push(obj(input)); return f.native.later(input); } };
  await action(f.client, f.route, { kind: 'retry' }, native, f.storage);
  expect(calls.some(call => call.action === 'finish')).toBe(true);
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.revision).toBe(2);
  expect(mobileDraftAttachmentIds(f.client, f.key)).toEqual([pickedId]);
  expect(intakeCalls(f, 'pick')).toHaveLength(1); expect(f.calls.some(call => call.action === 'resume')).toBe(true);
});

test('file hold retry reuses the original issued request and cannot acquire through saved-file authority', async () => {
  const f = await fixture(), pick = continuation(f); let tries = 0;
  f.holdHook(async request => { if (++tries === 1) throw Error('hold reply lost'); return heldReply(request); });
  await pick(); expect(intakeCalls(f, 'finish')).toHaveLength(0);
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(intakeCalls(f, 'hold')).toHaveLength(2); expect(intakeCalls(f, 'hold')[1]).toEqual(intakeCalls(f, 'hold')[0]);
  expect(f.calls.some(call => call.op === 'composerFileHold' && call.action === 'acquire')).toBe(false);
  expect(intakeCalls(f, 'finish')).toHaveLength(1);
});

test('Photos uses image upload capability without requiring server file upload support', async () => {
  const f = await fixture(), pick = continuation(f, [pickedImage]);
  f.client.config = { environment: { capabilities: { attachmentUploads: true } } }; await pick('photos');
  expect(intakeCalls(f, 'pick')[0].fileLimit).toBe(0); expect(intakeCalls(f, 'finish')).toHaveLength(1);
});

test('partial acceptance preserves historical images and releases only rejected issued file holds after terminal', async () => {
  const f = await fixture();
  f.client.local.snapshotDrafts[f.key] = [{ id: oldId, name: 'old.jpg', mimeType: 'image/jpeg', sizeBytes: 30 }];
  const second: PickerIntakeFile = { ...pickedFile, id: '77777777-7777-4777-8777-777777777777', name: 'second.md' };
  const text = 'x'.repeat(999900), pick = continuation(f, [pickedFile, second], text);
  f.intakeHook(async request => {
    if (request.action === 'finish') expect(f.calls.filter(call => call.action === 'release')).toHaveLength(0);
    return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile, second]);
  });
  await pick();
  const finish = intakeCalls(f, 'finish')[0];
  expect(finish).toBeDefined(); expect(obj(finish.publication)).toMatchObject({ acceptedIds: [pickedId], discardedIds: [second.id] });
  expect(f.client.local.snapshotDrafts[f.key].map(row => row.id)).toEqual([oldId]);
  expect(f.client.local.snapshotReleases).toEqual([]);
  const releases = f.calls.filter(call => call.action === 'release'); expect(releases).toHaveLength(1);
  const rejectedHold = intakeCalls(f, 'hold').find(call => obj(obj(call.request).file).id === second.id);
  expect(releases[0].requestId).toBe(obj(rejectedHold?.request).requestId);
  expect(f.order.indexOf('composerPickerIntake:finish')).toBeLessThan(f.order.indexOf('composerFileHold:release'));
});

test('positive retirement delegates claims while published intake still requires exact native finish', async () => {
  const f = await fixture(), pick = continuation(f); let attempts = 0;
  f.intakeHook(async request => {
    if (request.action === 'finish' && ++attempts === 1) throw Error('uncertain finish');
    return intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]);
  });
  await pick(); const finish = intakeCalls(f, 'finish')[0]; f.reserved([]); f.detach(); await f.reread();
  expect(intakeCalls(f, 'finish')).toEqual([finish, finish]); expect(f.calls.some(call => call.action === 'release')).toBe(false);
  const reopened = view(f.client, await f.resume('terminal-retirement'));
  expect(reopened.kind).toBe('mounting');
  expect(config(reopened).document).toMatchObject({ value: f.client.local.drafts[f.key] });
  expect(intakeCalls(f, 'finish')).toEqual([finish, finish]);
});

test('post-publication absent is not cancellation authority', async () => {
  const f = await fixture(), pick = continuation(f);
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'absent' : 'staged', request.action === 'finish' ? [] : [pickedFile]));
  await pick(); f.reserved([]); f.detach(); await f.reread();
  expect(intakeCalls(f, 'finish')).toHaveLength(2); expect(intakeCalls(f, 'cancel')).toHaveLength(0);
  expect(intakeCalls(f, 'status')).toHaveLength(0); expect(mobileDraftAttachmentIds(f.client, f.key)).toEqual([pickedId]);
});

test('final serialized proof does not reread mutable settings after the successful write await', async () => {
  const f = await fixture(), pick = continuation(f); let saves = 0;
  const latest = { providerId: 'codex', modelId: 'after-bytes', modelOptions: [] };
  f.writeHook(async () => { if (++saves === 2) mobileNewTaskDraftChoicesUpdate(f.client, f.key, latest); });
  await pick();
  const after = obj(obj(intakeCalls(f, 'finish')[0].publication).after);
  expect(after.record).toEqual(savedRecord(f)); expect(obj(after.record).choices).toBeNull();
  expect(mobileNewTaskDraftLookup(f.client, f.key)?.choices).toEqual(latest);
});

for (const change of ['source', 'context-presence', 'inventory', 'incarnation', 'project', 'order'] as const)
  test(`a ${change} conflict after publication prevents save retry and blind cancellation`, async () => {
    const f = await fixture(), pick = continuation(f); let writes = 0;
    f.writeHook(async () => { if (++writes === 2) throw Error('not saved'); }); await pick();
    f.writeHook(null);
    const draft = mobileNewTaskDraftStore(f.client).records[f.key];
    if (change === 'source') f.client.local.drafts[f.key] = 'new content';
    else if (change === 'context-presence') delete draft.context;
    else if (change === 'inventory') setDraftFiles(f.client.local, []);
    else if (change === 'incarnation') draft.createdAt = '2026-10-15T00:00:00Z';
    else if (change === 'project') draft.projectId = 'replacement';
    else {
      f.client.local.snapshotDrafts[f.key] = [{ id: oldId, name: 'new.jpg', mimeType: 'image/jpeg', sizeBytes: 10 }];
      Object.assign(f.client.local, { mobileAttachmentOrder: { [f.key]: [oldId, pickedId] } });
    }
    const before = f.writes.length; await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
    expect(f.writes).toHaveLength(before); expect(intakeCalls(f, 'finish')).toHaveLength(0); expect(intakeCalls(f, 'cancel')).toHaveLength(0);
  });

for (const stage of ['pick', 'finish'] as const) test(`late ${stage} success cannot reapply a terminal batch or change another editor`, async () => {
  const f = await fixture(), pick = continuation(f), pending = deferred<unknown>(), entered = deferred<void>(); let blocked: Obj = {};
  f.intakeHook(async request => {
    if (request.action === stage) { blocked = request; entered.resolve(); return pending.promise; }
    return intakeReply(request, 'staged', [pickedFile]);
  });
  const old = pick(); await entered.promise;
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished' : 'staged', [pickedFile]));
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  const route = await f.open(`success-${stage}`), seed = view(f.client, route), identity = { ...mounted(seed), mountId: `success-${stage}` };
  expect(view(f.client, route, event(identity, '')).kind).toBe('editing');
  const writes = f.writes.length, finishes = intakeCalls(f, 'finish').length;
  pending.resolve(intakeReply(blocked, stage === 'pick' ? 'staged' : 'finished', [pickedFile])); await old;
  expect(view(f.client, route).kind).toBe('editing'); expect(f.writes).toHaveLength(writes); expect(intakeCalls(f, 'finish')).toHaveLength(finishes);
});

test('a selected legacy file frees picker capacity while every historical image remains attached', async () => {
  const f = await fixture(), source = contextLink('file', oldId, 'old');
  f.client.local.snapshotDrafts[f.key] = Array.from({ length: 99 }, (_, i) => ({ id: `old-image-${i}`, name: `old-${i}.jpg`, mimeType: 'image/jpeg', sizeBytes: 10 }));
  setDraftFiles(f.client.local, [{ id: oldId, contextId: oldId, draftKey: f.key, environmentId: 'env', name: 'old.txt',
    mimeType: 'text/plain', sizeBytes: 10, source: 'attached', status: 'staged', attachmentId: '' }]);
  const pick = continuation(f, [pickedFile], source, { start: 0, end: source.length }); await pick();
  expect(intakeCalls(f, 'pick')[0].remaining).toBe(1); expect(intakeCalls(f, 'finish')).toHaveLength(1);
  expect(f.client.local.snapshotDrafts[f.key]).toHaveLength(99); expect(draftFiles(f.client.local).map(row => row.id)).toEqual([pickedId]);
  expect(f.client.local.snapshotReleases).toEqual([]); expect(f.calls.some(call => call.action === 'release')).toBe(false);
});

test('lost cancellation retains rejected holds until exact replay confirms the terminal outcome', async () => {
  const f = await fixture(), pick = continuation(f, [pickedFile], 'x'.repeat(1_000_000)); let cancels = 0, releases = 0;
  f.intakeHook(async request => {
    if (request.action === 'cancel' && ++cancels === 1) throw Error('cancel reply lost');
    return intakeReply(request, request.action === 'cancel' ? 'cancelled' : 'staged', [pickedFile]);
  });
  f.holdHook(async request => { if (request.action === 'release' && ++releases === 1) throw Error('release reply lost'); return heldReply(request); });
  await pick(); expect(f.calls.some(call => call.action === 'release')).toBe(false);
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(intakeCalls(f, 'cancel')).toHaveLength(2); expect(intakeCalls(f, 'cancel')[1]).toEqual(intakeCalls(f, 'cancel')[0]);
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  const release = f.calls.filter(call => call.action === 'release'); expect(release).toHaveLength(2); expect(release[1]).toEqual(release[0]);
  expect(intakeCalls(f, 'cancel')).toHaveLength(2); expect(intakeCalls(f, 'finish')).toHaveLength(0);
});

test('native dictionary property order does not change the issued picker inventory', async () => {
  const f = await fixture(), pick = continuation(f);
  f.intakeHook(async request => intakeReply(request, request.action === 'finish' ? 'finished' : 'staged',
    request.action === 'finish' ? [{ sizeBytes: pickedFile.sizeBytes, mimeType: pickedFile.mimeType,
      name: pickedFile.name, id: pickedFile.id, kind: pickedFile.kind }] : [pickedFile]));
  await pick();
  await action(f.client, f.route, { kind: 'retry' }, f.native, f.storage);
  expect(intakeCalls(f, 'finish')).toHaveLength(1);
  expect(mobileDraftAttachmentIds(f.client, f.key)).toEqual([pickedId]);
});
