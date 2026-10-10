import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { mobileCanForkResponse, mobileThreadForkObserveRoute as observe, mobileThreadForkPresentation as view,
  mobileThreadForkAction as action } from './thread-fork';
import { mobileThreadRows } from './thread';

const now = Date.parse('2026-10-10T12:00:00Z');
const rowId = JSON.stringify(['thread', 'answer']);
const capable = { threads: { canForkThread: true, canForkFromTurn: true }, identity: { nativeThreadIds: 'strong' }, context: {} };
const projected = (extra: Obj = {}, sourceThreadId = 'thread'): Obj => ({ sourceThreadId, sourceItemId: 'answer', visibility: 'local',
  item: { id: 'answer', type: 'assistant_message', runId: 'run', status: 'completed', providerThreadId: 'provider-thread',
    text: 'Completed answer', streaming: false, updatedAt: new Date(now).toISOString(), ...extra } });
function deferred() { let resolve!: () => void; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; }
function fixture() {
  const client = new T3Client(), calls: Obj[] = [], saved: Obj[] = [], hooks = new Map<string, () => unknown | Promise<unknown>>();
  let time = 1000;
  Object.assign(client, { origin: 'https://test.invalid', environmentId: 'env', generation: 3, threadId: 'thread', projectId: 'project',
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, shellLoaded: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  client.shell.threads = [{ id: 'thread', projectId: 'project', title: 'Original' }];
  client.thread = { sequence: 1, hasMore: false, historyCursor: null, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 'thread', projectId: 'project', title: 'Original' }, runs: [{ id: 'run', status: 'completed' }],
    turnItems: [projected().item], visibleTurnItems: [projected()], providerThreads: [{ id: 'provider-thread', providerSessionId: 'session' }],
    providerSessions: [{ id: 'session', capabilities: capable }] } };
  client.local.drafts['env:thread'] = 'Original unsent draft'; client.local.drafts['env:other'] = 'Another draft';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const hook = hooks.get(String(request.op)); if (hook) { const reply = await hook(); if (reply !== undefined) return reply; }
    return { ok: true, generation: 3, value: request.op === 'ids' ? ['command', 'target']
      : request.op === 'http' ? { authenticated: true, permissions: ['orchestration:operate'] }
      : request.op === 'timelineSleep' ? { monotonicMs: time += Number(request.ms) } : {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile(_path, bytes) {
    saved.push(obj(JSON.parse(new TextDecoder().decode(bytes)))); const hook = hooks.get('persist'); if (hook) await hook();
  } } };
  observe(client, 'visit', true);
  const run = (token = view(client, rowId).forkKey) => action(client, rowId, token, 'visit', native, storage);
  const writes = () => calls.filter(call => call.method === 'orchestration.dispatchCommand');
  const ready = () => client.shell.threads.push({ id: 'target', projectId: 'project', title: 'Original fork' });
  return { client, calls, saved, hooks, native, storage, run, writes, ready };
}

test('source eligibility includes absent evidence, native turn fork and portable handoff', () => {
  expect(mobileCanForkResponse(projected())).toBe(true);
  expect(mobileCanForkResponse(projected(), capable)).toBe(true);
  expect(mobileCanForkResponse(projected(), { context: { supportsFullThreadHandoff: true } })).toBe(true);
  for (const capabilities of [{}, { ...capable, identity: { nativeThreadIds: 'weak' } }, { ...capable, threads: { canForkThread: true } }])
    expect(mobileCanForkResponse(projected(), capabilities)).toBe(false);
  for (const change of [{ status: 'running' }, { status: 'failed' }, { runId: null }, { type: 'user_message' }])
    expect(mobileCanForkResponse(projected(change), capable)).toBe(false);
});

test('item support follows the source provider thread, retaining observed inherited evidence', () => {
  const f = fixture();
  f.client.projection.providerSessions = [{ id: 'session', capabilities: {} }, { id: 'active', capabilities: capable }];
  observe(f.client, 'visit', true); expect(view(f.client, rowId).canFork).toBe(false);
  // The selected fork's capable active session cannot override its ancestor.
  f.client.threadId = 'child'; f.client.threadEpoch++;
  f.client.thread!.projection = { thread: { id: 'child' }, turnItems: [], visibleTurnItems: [projected()],
    providerThreads: [{ id: 'provider-thread', providerSessionId: 'active' }], providerSessions: [{ id: 'active', capabilities: capable }] };
  observe(f.client, 'child-visit', true); expect(view(f.client, rowId).canFork).toBe(false);
  f.client.generation++; observe(f.client, 'child-visit', true);
  expect(view(f.client, rowId).canFork).toBe(true); // missing source record, never the selected session
});

test('one durable mobile fork waits for the shell and returns navigation without changing drafts or selection', async () => {
  const f = fixture(), drafts = structuredClone(f.client.local.drafts);
  f.hooks.set('timelineSleep', () => { if (f.calls.filter(call => call.op === 'timelineSleep').length === 3) f.ready(); });
  const token = view(f.client, rowId).forkKey, pending = f.run(token);
  expect(mobileThreadRows(f.client, now).find(row => row.id === rowId)?.forkBusy).toBe(true);
  expect(await f.run(token)).toMatchObject({ message: '', threadId: '' });
  expect(await pending).toMatchObject({ message: '', environmentId: 'env', threadId: 'target', requestRoute: 'visit' });
  expect(f.writes()).toHaveLength(1);
  expect(f.writes()[0]!.payload).toEqual({ type: 'thread.fork', commandId: 'command', targetThreadId: 'target', sourceThreadId: 'thread',
    sourcePoint: { type: 'run', runId: 'run' }, title: 'Original fork', createdBy: 'user', creationSource: 'mobile' });
  expect(f.saved[0]!.pending).toMatchObject({ env: { payload: { commandId: 'command', targetThreadId: 'target' } } });
  expect(f.client.pending).toBeUndefined(); expect(f.client.threadId).toBe('thread');
  expect(f.client.local.drafts).toEqual(drafts); expect(view(f.client, rowId).forkBusy).toBe(false);
  expect(f.calls.filter(call => call.op === 'timelineSleep').map(call => call.ms)).toEqual([0, 40, 40]);
});

test('a missing shell gives the source Fork created guidance after a bounded two second wait', async () => {
  const f = fixture(); expect(await f.run()).toMatchObject({ message: '', threadId: '' });
  const sleeps = f.calls.filter(call => call.op === 'timelineSleep');
  expect(sleeps).toHaveLength(51); expect(sleeps.reduce((total, call) => total + Number(call.ms), 0)).toBe(2000);
  expect(f.calls.find(call => call.op === 'mobileAlert')).toMatchObject({ title: 'Fork created',
    message: 'Its thread data did not reach this client. Reconnect and try opening it from the thread list.' });
  expect(f.writes()).toHaveLength(1); expect(f.client.threadId).toBe('thread'); expect(f.client.draft).toBe('Original unsent draft');
});

test('an already received target does not sleep or use an alert', async () => {
  const f = fixture(); f.hooks.set('request', () => { f.ready(); }); expect((await f.run()).threadId).toBe('target');
  expect(f.calls.filter(call => ['timelineSleep', 'mobileAlert'].includes(String(call.op)))).toHaveLength(0);
});

for (const boundary of ['http', 'ids', 'persist', 'request', 'timelineSleep']) {
  test(`departing and returning to the same visit during ${boundary} cannot complete stale work`, async () => {
    const f = fixture(), entered = deferred(), release = deferred();
    f.hooks.set(boundary, async () => { entered.resolve(); await release.promise; });
    const pending = f.run(); await entered.promise;
    observe(f.client, 'review', false); observe(f.client, 'visit', true); release.resolve();
    await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.writes()).toHaveLength(['request', 'timelineSleep'].includes(boundary) ? 1 : 0);
    expect(f.calls.filter(call => call.op === 'mobileAlert')).toHaveLength(0);
    expect(f.client.threadId).toBe('thread'); expect(f.client.draft).toBe('Original unsent draft');
  });
}

for (const change of ['origin', 'environmentId', 'generation', 'threadEpoch', 'response', 'capabilities']) {
  test(`a changed ${change} during permission read prevents identifier allocation and writes`, async () => {
    const f = fixture(); f.hooks.set('http', () => {
      if (change === 'response') obj((f.client.projection.visibleTurnItems as Obj[])[0]!.item).runId = 'changed-run';
      else if (change === 'capabilities') (f.client.projection.providerSessions as Obj[])[0]!.capabilities = {};
      else if (change === 'generation' || change === 'threadEpoch') f.client[change]++;
      else f.client[change] = 'changed';
    });
    await expect(f.run()).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(0); expect(f.writes()).toHaveLength(0);
  });
}

test('explicit denied permissions and existing pending writes refuse another fork', async () => {
  const f = fixture(); f.hooks.set('http', () => ({ ok: true, generation: 3,
    value: { authenticated: true, permissions: [], scopes: ['orchestration:operate'] } }));
  expect((await f.run()).message).toContain('cannot fork'); expect(f.writes()).toHaveLength(0);
  const g = fixture(); g.client.local.pending.env = { method: 'old', payload: { commandId: 'old' }, description: 'Old', threadId: 'thread', text: '', uncertain: true };
  expect((await g.run()).message).toContain('current operation'); expect(g.calls).toHaveLength(0);
});

test('an uncertain reply retains the original IDs for shared reconciliation and prevents duplicate creation', async () => {
  const f = fixture(); f.hooks.set('request', () => ({ ok: false, generation: 3,
    error: { kind: 'transport', message: 'The reply was lost.', uncertain: true } }));
  expect((await f.run()).message).toBe('The reply was lost.');
  expect(f.client.pending).toMatchObject({ uncertain: true, payload: { commandId: 'command', targetThreadId: 'target' } });
  expect((await f.run()).message).toContain('current operation'); expect(f.writes()).toHaveLength(1);
  f.ready(); expect(f.client.reconcilePending()).toBe(true); expect(f.client.pending).toBeUndefined();
  expect(f.client.draft).toBe('Original unsent draft');
});

test('a let-go wait propagates cancellation without a timeout alert or another write', async () => {
  const f = fixture(); f.hooks.set('timelineSleep', () => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await expect(f.run()).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.writes()).toHaveLength(1); expect(f.calls.filter(call => call.op === 'mobileAlert')).toHaveLength(0);
  expect(view(f.client, rowId).forkBusy).toBe(false);
});
