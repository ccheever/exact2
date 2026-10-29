import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles, mobileDraftRecoveryHydrated } from './mobile-draft-recovery';
import { mobileNewTaskEditorAction as action, mobileNewTaskEditorRefresh as refresh, mobileNewTaskEditorView as view,
  type NewTaskEditorRoute, type NewTaskEditorView } from './new-task-editor';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction } from './new-task-flow';
import { mobileNewTaskDraftLookup, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskContextGuard, mobileNewTaskContextWrite } from './mobile-new-task-context';
import type { ComposerEditorEvent, ComposerMountedIdentity } from './composer-editor-state';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { noteNow } from './shared/composer-controls';
import { obj, str, type Obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';

const origin = 'https://new-task-editor.test', now = 1791420000000;
const receiptId = '11111111-1111-4111-8111-111111111111';
function deferred<T>() {
  let resolve: (value: T) => void = () => { throw Error('uninitialized'); };
  let reject: (error: unknown) => void = () => { throw Error('uninitialized'); };
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function mounted(projection: NewTaskEditorView): ComposerMountedIdentity {
  if (!projection.node) throw Error(projection.message);
  const configuration = obj(JSON.parse(projection.node.configuration));
  return { owner: str(configuration.owner), editorId: str(configuration.editorId), routeVisit: str(configuration.routeVisit),
    renderEpoch: str(configuration.renderEpoch), mountId: str(configuration.mountId) || 'native-mount' };
}
function event(identity: ComposerMountedIdentity, value: string, eventCount = 1, kind: ComposerEditorEvent['kind'] = 'ready'): ComposerEditorEvent {
  return { ...identity, kind, eventCount, value, selection: { start: value.length, end: value.length }, composing: false, focused: true };
}
function receipt(identity: ComposerMountedIdentity, value = '🙂 final source', eventCount = 3) {
  return { receiptId, identity, event: { ...event(identity, value, eventCount, 'text'), kind: 'retired' }, retired: true };
}
class CountingClient extends MobileDraftClient {
  refreshes = 0;
  override async refresh(native: Native | null | undefined, storage: Files): Promise<void> {
    this.refreshes++; await super.refresh(native, storage);
  }
}
async function fixture() {
  const client = new CountingClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [], order: string[] = [], writes: Obj[] = [];
  let disk = JSON.stringify({ version: 1 }), receipts: unknown[] = [], allocated = 0;
  let writeHook: (() => Promise<void>) | null = null;
  let readHook: (() => Promise<unknown>) | null = null;
  let ackHook: (() => Promise<unknown>) | null = null;
  const storage: Files = { fs: {
    async mkdir() {}, async readFile() { order.push('load'); return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) {
      const output = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(output))); order.push('save');
      if (writeHook) await writeHook();
      disk = output;
    },
  } };
  const native: Native = { available: true, watch(topic) { order.push(`watch:${topic}`); }, async later(input) {
    const request = obj(input); calls.push(request); order.push(`${request.op}:${request.action ?? ''}`);
    const generation = client.generation;
    if (request.op === 'composerEditorFinalizations') {
      if (request.action === 'read') return readHook ? readHook() : { ok: true, generation, value: { receipts } };
      if (ackHook) return ackHook();
      receipts = receipts.filter(value => obj(value).receiptId !== request.receiptId);
      return { ok: true, generation, value: { acknowledged: true } };
    }
    if (request.op === 'mobileOutbox') return { ok: true, generation, value: request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] } };
    return { ok: true, generation, value: request.op === 'status' ? { phase: 'disconnected' }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `draft-${++allocated}`)
      : request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] } : {} };
  } };
  const handles = () => mobileDraftRecoveryHandles(client, native, storage);
  const initial = handles(); await client.refresh(initial.native, initial.storage);
  const readyClient = () => {
    Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 3, connection: 'connected',
      configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
    client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
    client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/project' }]; noteNow(client, now);
  };
  readyClient();
  const checked = handles(); await refresh(client, checked.native, checked.storage); readyClient();
  const open = async (session = 'flow', visit = 'draft') => {
    readyClient();
    const chooser = mobileNewTaskFlowView(session, 'choose', '/new', true, true, client, fleet);
    const selected = await mobileNewTaskFlowAction(chooser.owner, 'choose', 'project', '["env","project"]', '', native, storage, client, fleet, now);
    expect(selected.message).toBe('');
    const draft = mobileNewTaskFlowView(session, visit, '/new/draft', true, true, client, fleet);
    expect(draft.ready).toBe(true);
    return { owner: draft.owner, visit, location: '/new/draft', editorId: 'new-task-editor' };
  };
  const route = await open(), key = client.draftKey;
  const resume = async (session = 'resume', generation = 3, draftKey = key) => {
    readyClient(); client.generation = generation;
    const location = `/new/draft?draftId=${encodeURIComponent(draftKey)}`;
    const initial = mobileNewTaskFlowView(session, 'resume', location, true, true, client, fleet);
    const prepared = await mobileNewTaskFlowAction(initial.owner, 'resume', 'prepare', '', '', native, storage, client, fleet, now);
    expect(prepared.message).toBe('');
    const current = mobileNewTaskFlowView(session, 'resume', location, true, true, client, fleet);
    expect(current.ready).toBe(true);
    return { owner: current.owner, visit: 'resume', location, editorId: 'new-task-editor' };
  };
  const offer = view(client, route), identity = mounted(offer);
  expect(offer.kind).toBe('mounting');
  expect(view(client, route, event(identity, '')).kind).toBe('editing');
  writes.length = 0; calls.length = 0; order.length = 0;
  const reread = async () => { const current = handles(); await refresh(client, current.native, current.storage); };
  const detach = () => {
    mobileNewTaskFlowView('flow', 'away', '/', false, true, client, fleet);
    Object.assign(client, { environmentId: 'other', projectId: 'other-project', threadId: 'other-thread', generation: 4 });
    client.local.drafts['other:other-thread'] = 'unrelated';
  };
  const writeText = (text: string) => {
    const guard = mobileNewTaskContextGuard(client, key);
    if (!guard || !mobileNewTaskContextWrite(client, guard, text)) throw Error('write refused');
  };
  return { client, native, storage, route, key, identity, order, writes, calls, open, resume, detach, reread, writeText,
    disk: () => obj(JSON.parse(disk)), receipts: (next: unknown[]) => { receipts = next; },
    writeHook: (next: typeof writeHook) => { writeHook = next; }, readHook: (next: typeof readHook) => { readHook = next; },
    ackHook: (next: typeof ackHook) => { ackHook = next; } };
}

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
  gate.resolve({ ok: true, generation: 3, value: { receipts: [source] } }); await older;
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
    if (mutation === 'wrong-generation') f.readHook(async () => ({ ok: true, generation: 99, value: { receipts: [source] } }));
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
  gate.resolve({ ok: true, generation: 3, value: { receipts: [receipt(f.identity)] } }); await reading;
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
