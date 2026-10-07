// Ported from T3 Code (MIT, see LICENSE-T3), reference 1e2ecbd975: apps/web/src/state/threads.test.ts
// (createRunningThreadKeepAliveAtom). Change from the reference: the keep-alive is a pure pass
// (`runningThreadKeepAlive`) over each environment's shell and its kept details, not an atom over a
// registry; "open streams" are the kept map's entries. The clone's wiring follows: each environment
// keeps its streams on its own transport, and opening a kept thread starts from its kept detail.
import { describe, expect, it, test } from 'bun:test';
import { runningThreadKeepAlive, isDetailDone, keepAliveFleetEvent, keepAliveFleetPass, keepAliveEvent, keepAlivePrepare, keptThread,
  handoffKeptThread, adoptHandoff, KEEP_PREFIX, type KeptDetail, type KeptThreads } from './keep-alive';
import { initialShell, type Obj, type ThreadState } from './domain';
import type { FleetEntry } from './settings-b-fleet';
import type { Native } from './protocol';

const LOCAL = 'local', REMOTE = 'remote';
type Status = 'running' | 'starting' | 'idle';
const shell = (id: string, status: Status | null) => ({ id, status: status ?? 'idle' });
const ARRAYS = ['attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests', 'messages', 'plans', 'turnItems',
  'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems'];
function projection(id: string, status: Status): Obj {
  return { ...Object.fromEntries(ARRAYS.map(name => [name, []])), thread: { id, title: id },
    runs: status === 'idle' ? [] : [{ id: `run-${id}`, threadId: id, ordinal: 1, status }], updatedAt: '2026-10-07T00:00:00.000Z' };
}
function detail(id: string, status: Status, overrides: Partial<KeptDetail> = {}): KeptDetail {
  const thread: ThreadState = { projection: projection(id, status), sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  return { status: 'live', thread, error: '', ...overrides };
}

/** The reference's harness: listed environments, each one's shell threads and details, and the open streams after a pass. */
function makeHarness() {
  let environmentIds: string[] = [LOCAL, REMOTE];
  const threads = new Map<string, { id: string; status: string }[]>();
  const details = new Map<string, KeptDetail>();
  let kept: KeptThreads | undefined;
  const run = () => (kept = runningThreadKeepAlive({ environmentIds, threads: id => threads.get(id) ?? [], detail: (environmentId, threadId) => details.get(`${environmentId}:${threadId}`), previous: kept }));
  return {
    setEnvironments: (ids: string[]) => { environmentIds = ids; },
    setThreads: (environmentId: string, list: { id: string; status: string }[]) => threads.set(environmentId, list),
    setDetail: (environmentId: string, threadId: string, value: KeptDetail) => details.set(`${environmentId}:${threadId}`, value),
    run,
    openStreams: () => { run(); return [...kept!].flatMap(([environmentId, ids]) => [...ids].map(id => `${environmentId}:${id}`)).sort(); },
  };
}

describe('createRunningThreadKeepAliveAtom', () => {
  it('keeps running threads open across shell updates and thread view visits', () => {
    const h = makeHarness();
    h.setThreads(LOCAL, [shell('a', 'running'), shell('b', 'idle'), shell('c', null)]);
    h.setThreads(REMOTE, [shell('d', 'starting')]);
    expect(h.openStreams()).toEqual(['local:a', 'remote:d']);

    // A thread view that comes and goes shares the kept stream: its detail is the kept one.
    h.setDetail(LOCAL, 'a', detail('a', 'running'));

    // A shell update that starts or stops nothing does not rebuild the set.
    const kept = h.run();
    h.setThreads(LOCAL, [shell('a', 'running'), shell('b', 'idle')]);
    expect(h.run()).toBe(kept);
    expect(h.openStreams()).toEqual(['local:a', 'remote:d']);
  });

  it('holds a stopped thread until its own stream is live and shows the stop', () => {
    const h = makeHarness();
    h.setThreads(LOCAL, [shell('a', 'running'), shell('b', 'running'), shell('c', 'running')]);
    h.run();
    // "b" has not loaded yet. "c" hit a stream error.
    h.setDetail(LOCAL, 'a', detail('a', 'running'));
    h.setDetail(LOCAL, 'c', detail('c', 'running', { status: 'loading', error: 'Could not sync.' }));

    // The shell reports the stops first. A failed stream cannot deliver its stop, so only it is released now.
    h.setThreads(LOCAL, [shell('a', 'idle'), shell('b', 'idle'), shell('c', 'idle')]);
    expect(h.openStreams()).toEqual(['local:a', 'local:b']);

    h.setDetail(LOCAL, 'a', detail('a', 'idle'));
    h.setDetail(LOCAL, 'b', detail('b', 'idle', { status: 'loading' }));
    expect(h.openStreams()).toEqual(['local:b']);
    h.setDetail(LOCAL, 'b', detail('b', 'idle'));
    expect(h.openStreams()).toEqual([]);
  });

  it('follows environments that connect and go away', () => {
    const h = makeHarness();
    h.setEnvironments([LOCAL]);
    h.setThreads(REMOTE, [shell('d', 'running')]);
    expect(h.openStreams()).toEqual([]);

    h.setEnvironments([LOCAL, REMOTE]);
    expect(h.openStreams()).toEqual(['remote:d']);

    // Removal drops every mount, including one still waiting for its stop.
    h.setDetail(REMOTE, 'd', detail('d', 'running'));
    h.setThreads(REMOTE, [shell('d', 'idle')]);
    expect(h.openStreams()).toEqual(['remote:d']);
    h.setEnvironments([LOCAL]);
    expect(h.openStreams()).toEqual([]);
  });
});

// ── The clone's wiring ─────────────────────────────────────────────────────
const shellOf = (list: { id: string; status: string }[]) => ({ ...initialShell(), threads: list.map(thread => ({ ...thread, projectId: 'p' })) });
const snapshotItem = (id: string, status: Status, sequence = 5) => ({ kind: 'snapshot', snapshotSequence: sequence, projection: projection(id, status), hasMoreHistory: false });
function fleetEntry(list: { id: string; status: string }[]): FleetEntry {
  return { key: 'https://box.example.com\nremote', origin: 'https://box.example.com', environmentId: REMOTE, phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3,
    lastEvent: 0, subscriptions: {}, config: {}, shell: shellOf(list), scopes: [], error: '', requested: true };
}

test('a background environment keeps each running thread\'s detail stream and releases it once its detail shows the stop', async () => {
  const calls: Obj[] = [];
  const call = async (request: Obj) => { calls.push(request); return request.op === 'subscribe' ? { id: `3-${calls.length}` } : {}; };
  const entry = fleetEntry([shell('t1', 'running'), shell('t2', 'idle')]);
  await keepAliveFleetPass(call, entry);
  const subscribe = calls.find(request => request.op === 'subscribe' && request.key === `${KEEP_PREFIX}t1`)!;
  expect(subscribe).toMatchObject({ method: 'orchestration.subscribeThread', payload: { threadId: 't1', afterSequence: 0, acceptBoundedSnapshot: true } });
  expect(calls.filter(request => String(request.key).startsWith(KEEP_PREFIX))).toHaveLength(1);
  // Its first item is the bounded snapshot: the detail is live.
  expect(keepAliveFleetEvent(entry, { key: `${KEEP_PREFIX}t1`, generation: 3, subscriptionId: '3-1', value: snapshotItem('t1', 'running') })).toBe(true);
  // The shell reports the stop before the detail: the stream stays.
  entry.shell = shellOf([shell('t1', 'idle'), shell('t2', 'idle')]);
  await keepAliveFleetPass(call, entry);
  expect(calls.filter(request => request.op === 'unsubscribe')).toEqual([]);
  // The detail shows it too: released.
  keepAliveFleetEvent(entry, { key: `${KEEP_PREFIX}t1`, generation: 3, subscriptionId: '3-1', value: snapshotItem('t1', 'idle', 6) });
  await keepAliveFleetPass(call, entry);
  expect(calls.filter(request => request.op === 'unsubscribe')).toEqual([{ op: 'unsubscribe', key: `${KEEP_PREFIX}t1` }]);
  // An old generation's or another subscription's entry is taken but not applied.
  expect(keepAliveFleetEvent(entry, { key: `${KEEP_PREFIX}t1`, generation: 2, subscriptionId: '2-1', value: snapshotItem('t1', 'running') })).toBe(true);
  expect(keepAliveFleetEvent(entry, { key: 'shell', generation: 3, subscriptionId: '3-9', value: {} })).toBe(false);
});

test('opening a kept thread of a background environment starts from its kept detail, without a refetch', async () => {
  const entry = fleetEntry([shell('t1', 'running')]);
  await keepAliveFleetPass(async request => request.op === 'subscribe' ? { id: '3-1' } : {}, entry);
  keepAliveFleetEvent(entry, { key: `${KEEP_PREFIX}t1`, generation: 3, subscriptionId: '3-1', value: snapshotItem('t1', 'running', 9) });
  const client = { environmentId: 'previous', threadId: '', thread: null as ThreadState | null };
  handoffKeptThread(client, entry, 't1');
  // focusFleetThread moves the client; adoptStatus resets the thread, then the handoff shows the kept detail.
  Object.assign(client, { environmentId: REMOTE, threadId: 't1', thread: null });
  expect(adoptHandoff(client)).toBe(true);
  expect(client.thread?.sequence).toBe(9);
  expect(String((client.thread?.projection.thread as Obj).id)).toBe('t1');
  expect(adoptHandoff(client)).toBe(false); // once
});

test('the focused environment keeps the running threads other than the open one, and a kept one opens from its detail', async () => {
  const calls: Obj[] = [];
  const client = { environmentId: LOCAL, origin: 'https://box.example.com', generation: 4, ready: true, threadId: 'open', thread: null,
    shell: shellOf([shell('open', 'running'), shell('t2', 'running'), shell('t3', 'idle')]),
    restAccess: () => ({ call: async (request: Obj) => { calls.push(request); return request.op === 'subscribe' ? { id: `4-${calls.length}` } : {}; } }) };
  const native: Native = { available: true, watch() {}, later: async () => ({}) };
  await keepAlivePrepare(client, native);
  expect(calls.filter(request => request.op === 'subscribe').map(request => request.key)).toEqual([`${KEEP_PREFIX}t2`]);
  expect(keepAliveEvent(client, { key: `${KEEP_PREFIX}t2`, generation: 4, subscriptionId: '4-1', value: snapshotItem('t2', 'running', 12) })).toBe(true);
  expect(keptThread(client, 't2')?.sequence).toBe(12);
  expect(keptThread(client, 't3')).toBeNull();
  expect(isDetailDone(undefined)).toBe(false);
});
