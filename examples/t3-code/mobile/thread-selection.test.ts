import { afterEach, expect, test } from 'bun:test';
import { mobileThreadSelection, mobileThreadSelectionFacts } from './thread-selection';
import { T3Client } from './shared/client';
import { fleet, type FleetEntry, type FleetPhase } from './shared/settings-b-fleet';
import { initialShell, obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

function fixture() {
  const client = new T3Client(), calls: Obj[] = [];
  client.adoptStatus({ state: 'connected', origin: 'https://focused.invalid', environmentId: 'focused', message: '' }, 1);
  client.shell = { ...initialShell(), projects: [{ id: 'project', title: 'Project' }],
    threads: [{ id: 'thread', projectId: 'project' }] };
  client.projectId = 'project';
  const projection: Obj = { thread: client.shell.threads[0] };
  for (const key of ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns',
    'runtimeRequests', 'messages', 'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[key] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'connect') return { ok: true, generation: 2,
      value: { state: 'connected', origin: 'https://remote.invalid', environmentId: 'remote', message: '' } };
    return { ok: true, generation: client.generation, value: request.op === 'http'
      ? { snapshotSequence: 1, projection } : request.op === 'subscribe' ? { id: 'thread-sub' } : {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  return { client, native, storage, calls, projection };
}

test('a stale fleet route opens the now-focused thread without reconnecting', async () => {
  const f = fixture();
  const result = await mobileThreadSelection(f.client, 'fleet:focused:thread', f.native, f.storage);
  expect(result.message).toBe('');
  expect(f.client.threadId).toBe('thread');
  expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
  expect(f.calls.some(call => call.path === '/api/orchestration/threads/thread/bounded')).toBe(true);
  expect(f.calls.filter(call => ['connect', 'fleetStop'].includes(str(call.op)))).toEqual([]);
});

test('a different environment retains its identity and uses the real fleet focus', async () => {
  const f = fixture(), key = 'https://remote.invalid\nremote';
  fleet.entries.set(key, { key, origin: 'https://remote.invalid', environmentId: 'remote', phase: 'connected', message: '',
    traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {}, config: {},
    shell: f.client.shell, scopes: ['orchestration:operate'], error: '', requested: true });
  const result = await mobileThreadSelection(f.client, 'fleet:remote:thread', f.native, f.storage);
  expect(result.message).toBe('');
  expect(f.client.environmentId).toBe('remote');
  expect(f.client.threadId).toBe('thread');
  expect(f.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', origin: 'https://remote.invalid', credential: '' }]);
  expect(f.calls.filter(call => call.op === 'fleetStop')).toEqual([{ op: 'fleetStop', fleet: key }]);
  expect(fleet.entries.has(key)).toBe(false);
  // The native focus has completed, but its shell has not loaded yet. A route
  // computed from the previous projection can repeat that exact fleet target.
  expect(f.client.shell.threads).toEqual([]);
  f.calls.length = 0;
  expect((await mobileThreadSelection(f.client, 'fleet:remote:thread', f.native, f.storage)).message).toBe('');
  expect(f.client.threadId).toBe('thread');
  expect(f.client.projectId).toBe('project');
  expect(f.calls).toEqual([]);
});

test('a missing different environment cannot open a same-named local thread', async () => {
  const f = fixture();
  const result = await mobileThreadSelection(f.client, 'fleet:missing:thread', f.native, f.storage);
  expect(result.message).toBe('That environment is no longer connected.');
  expect(f.client.threadId).toBe('');
  expect(f.client.environmentId).toBe('focused');
  expect(f.calls.some(call => call.op === 'http' || call.op === 'connect')).toBe(false);
});

test('normal local selection and real read failures keep their existing behavior', async () => {
  const f = fixture(), original = f.native.later;
  f.native.later = async input => obj(input).op === 'http'
    ? { ok: false, generation: 1, error: { kind: 'Permission', message: 'Thread read denied.', uncertain: false } }
    : original(input);
  expect((await mobileThreadSelection(f.client, 'fleet:focused:thread', f.native, f.storage)).message).toBe('Thread read denied.');
  expect(f.client.thread).toBeNull();
  f.native.later = original;
  expect((await mobileThreadSelection(f.client, 'thread', f.native, f.storage)).message).toBe('');
  expect(obj(f.client.thread?.projection.thread).id).toBe('thread');
});

/** The production client over three shell threads. A held native request waits
 * until the test releases it; nothing sleeps. */
function held() {
  const f = fixture(), holds: { match: (request: Obj) => boolean; taken: boolean; issued: () => void; released: Promise<void> }[] = [];
  f.client.shell = { ...f.client.shell, threads: ['a', 'b', 'c'].map(id => ({ id, projectId: 'project' })) };
  f.client.configLive = true; f.client.shellLive = true;
  const base = f.native.later;
  f.native.later = async input => {
    const request = obj(input), hold = holds.find(entry => !entry.taken && entry.match(request));
    if (hold) { hold.taken = true; hold.issued(); await hold.released; }
    const read = /^\/api\/orchestration\/threads\/([^/]+)\/bounded$/.exec(str(request.path));
    if (!read) return base(input);
    f.calls.push(request);
    return { ok: true, generation: f.client.generation,
      value: { snapshotSequence: 1, projection: { ...f.projection, thread: { id: read[1], projectId: 'project' } } } };
  };
  const hold = (match: (request: Obj) => boolean) => {
    let issued!: () => void, release!: () => void;
    const seen = new Promise<void>(resolve => { issued = resolve; });
    holds.push({ match, taken: false, issued: () => issued(), released: new Promise<void>(resolve => { release = resolve; }) });
    return { issued: seen, release: () => release() };
  };
  const facts = (environmentId: string, threadId: string) =>
    mobileThreadSelectionFacts(f.client, environmentId, threadId, `/threads/${environmentId}/${threadId}`);
  return { ...f, hold, facts };
}
const bounded = (id: string) => (request: Obj) => request.path === `/api/orchestration/threads/${id}/bounded`;
const remote = (phase: FleetPhase, shell = initialShell()): FleetEntry => ({ key: 'https://remote.invalid\nremote', origin: 'https://remote.invalid',
  environmentId: 'remote', phase, message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0, subscriptions: {}, config: {},
  shell, scopes: ['orchestration:operate'], error: '', requested: true });

test('a held previous read does not hold the route; the newer selection supersedes it', async () => {
  const f = held(), a = f.hold(bounded('a'));
  const first = mobileThreadSelection(f.client, 'a', f.native, f.storage);
  await a.issued;
  // The old gate read data.ready, which includes A's liveness.
  expect(f.client.threadId).toBe('a');
  expect(f.client.ready).toBe(false);
  const facts = f.facts('focused', 'b');
  expect(facts).toMatchObject({ selectionRoute: '/threads/focused/b', selectionReady: true, selectionReopen: false });
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
  a.release();
  expect((await first).message).toBe('');
  expect(f.client.threadId).toBe('b');
  expect(obj(f.client.thread?.projection.thread).id).toBe('b');
  expect(f.client.threadSubscription).toBe('b');
  // A same-environment selection changes nothing in the key: no second attempt.
  expect(f.facts('focused', 'b')).toEqual(facts);
});

test('B held, then C, then Back to B: the latest selection wins and the older one is not retried', async () => {
  const f = held(), b = f.hold(bounded('b'));
  const first = mobileThreadSelection(f.client, 'b', f.native, f.storage);
  await b.issued;
  const key = f.facts('focused', 'c').selectionKey;
  expect((await mobileThreadSelection(f.client, 'c', f.native, f.storage)).message).toBe('');
  b.release();
  expect((await first).message).toBe('');
  expect(f.client.threadId).toBe('c');
  expect(f.client.threadSubscription).toBe('c');
  expect(f.facts('focused', 'c').selectionKey).toBe(key);
  expect(f.facts('focused', 'b').selectionReopen).toBe(false);
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(obj(f.client.thread?.projection.thread).id).toBe('b');
  expect(f.client.threadSubscription).toBe('b');
});

test('a pending completion marker or failed stream on A is a wait, not an interrupted opening', async () => {
  const f = held();
  f.client.config = { ...f.client.config, threadResumeCompletionMarker: true };
  const waiting = f.facts('focused', 'a');
  expect((await mobileThreadSelection(f.client, 'a', f.native, f.storage)).message).toBe('');
  // Both leave the subscription on A and threadLive false (shared/client.ts readThread, drain).
  expect(f.client.threadSubscription).toBe('a');
  expect(f.client.threadLive).toBe(false);
  expect(f.client.ready).toBe(false);
  // No interruption is counted, so the gate's key stays and nothing repeats.
  expect(f.facts('focused', 'a')).toEqual(waiting);
  expect(f.facts('focused', 'b').selectionReady).toBe(true);
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: true, selectionReopen: false });
});

test('the focused target is ready when synchronized, cached while connecting, and offline otherwise', () => {
  const f = held();
  f.client.shellLive = false;
  const synchronizing = f.facts('focused', 'b');
  expect(synchronizing.selectionReady).toBe(false);
  f.client.shellLive = true;
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: true, selectionKey: synchronizing.selectionKey });
  f.client.connection = 'connecting';
  const uncached = f.facts('focused', 'b');
  expect(uncached.selectionReady).toBe(false);
  fleet.saved = [{ environmentId: 'focused', origin: 'https://focused.invalid', enabled: true }];
  const cached = f.facts('focused', 'b');
  expect(cached.selectionReady).toBe(true);
  expect(cached.selectionKey).not.toBe(uncached.selectionKey);
  // The existing offline selection owns a disconnected target and its message.
  f.client.connection = 'disconnected';
  expect(f.facts('focused', 'b').selectionReady).toBe(true);
  expect(f.facts('', 'b').selectionReady).toBe(false);
});

test('a fleet target uses its own phase, cache and switch refusal, not the focused thread', () => {
  const f = held();
  f.client.threadLive = false; f.client.threadId = 'a';
  // Absent and unsaved: the selection answers 'That environment is no longer connected.' once.
  const absent = f.facts('remote', 't');
  expect(absent.selectionReady).toBe(true);
  const entry = remote('connecting');
  fleet.entries.set(entry.key, entry);
  const connecting = f.facts('remote', 't');
  expect(connecting.selectionReady).toBe(false);
  fleet.saved = [{ environmentId: 'remote', origin: 'https://remote.invalid', enabled: true }];
  const cached = f.facts('remote', 't');
  expect(cached.selectionReady).toBe(true);
  entry.phase = 'connected';
  const connected = f.facts('remote', 't');
  expect(connected.selectionReady).toBe(true);
  expect(new Set([absent, connecting, cached, connected].map(facts => facts.selectionKey)).size).toBe(4);
  // Busy work alone is not a new attempt: only the end of a switch refusal is (below).
  entry.phase = 'available';
  const idle = f.facts('remote', 't');
  f.client.busy = true;
  expect(f.facts('remote', 't')).toEqual(idle);
  f.client.busy = false;
  expect(f.facts('remote', 't')).toEqual(idle);
});

const BUSY = 'Wait for the current send before switching threads.';

test('an uncatalogued saved target refuses before busy is read, so busy never re-arms it', async () => {
  const f = held();
  fleet.saved = [{ environmentId: 'remote', origin: 'https://remote.invalid', enabled: true }];
  expect(f.facts('remote', 't').selectionReady).toBe(true);
  // A replaced saved identity leaves the catalog pending until it is invalidated.
  fleet.saved = [{ environmentId: 'remote', origin: 'https://replaced.invalid', enabled: true }];
  const idle = f.facts('remote', 't');
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe('This thread is unavailable offline.');
  f.client.busy = true;
  // The same early refusal, not the busy one, so a re-armed attempt would only repeat this alert.
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe('This thread is unavailable offline.');
  expect(f.facts('remote', 't')).toEqual(idle);
  f.client.busy = false;
  expect(f.facts('remote', 't')).toEqual(idle);
});

test('only the end of a busy switch refusal re-arms a catalogued target, once', async () => {
  const f = held();
  fleet.saved = [{ environmentId: 'remote', origin: 'https://remote.invalid', enabled: true }];
  const idle = f.facts('remote', 't');
  const answer = (await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message;
  expect(answer).not.toBe(BUSY);
  // After a refusal busy cannot turn into success: toggling it is no new attempt.
  f.client.busy = true;
  expect(f.facts('remote', 't').selectionKey).toBe(idle.selectionKey);
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe(BUSY);
  // Recording the refusal is not itself a key change, so it alerts once.
  expect(f.facts('remote', 't').selectionKey).toBe(idle.selectionKey);
  f.client.busy = false;
  const released = f.facts('remote', 't');
  expect(released.selectionKey).not.toBe(idle.selectionKey);
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe(answer);
  expect(f.facts('remote', 't')).toEqual(released);
  f.client.busy = true;
  expect(f.facts('remote', 't')).toEqual(released);
});

test('a switch refused again under the same blocker keeps its key, so it alerts once per attempt', async () => {
  const f = held();
  fleet.saved = [{ environmentId: 'remote', origin: 'https://remote.invalid', enabled: true }];
  f.client.busy = true;
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe(BUSY);
  // A new visit (Back, then the same row) arms an attempt while the target is still refused and blocked.
  const blocked = f.facts('remote', 't');
  for (let attempt = 0; attempt < 2; attempt++) {
    expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).toBe(BUSY);
    expect(f.facts('remote', 't')).toEqual(blocked);
  }
  f.client.busy = false;
  const released = f.facts('remote', 't');
  expect(released.selectionKey).not.toBe(blocked.selectionKey);
  expect((await mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage)).message).not.toBe(BUSY);
  expect(f.facts('remote', 't')).toEqual(released);
});

test('releasing an environment switch hold publishes a revision for the routes it held', async () => {
  const f = held(), first = f.hold(() => true);
  fleet.saved = [{ environmentId: 'remote', origin: 'https://remote.invalid', enabled: true }];
  const switching = mobileThreadSelection(f.client, 'fleet:remote:t', f.native, f.storage).catch((error: unknown) => error);
  expect(await Promise.race([first.issued.then(() => 'held'), switching.then(() => 'answered')])).toBe('held');
  expect(f.facts('focused', 'b').selectionReady).toBe(false);
  const revision = f.client.revision;
  first.release();
  await switching;
  expect(f.facts('focused', 'b').selectionReady).toBe(true);
  // A held route re-reads its facts only when data.revision (or the minute tick) changes.
  expect(f.client.revision).toBeGreaterThan(revision);
});

test('a chosen, unopened thread whose retry another command interrupts during its first load opens again', async () => {
  const f = held(), read = f.storage.fs.readFile;
  let issued!: () => void, release!: () => void, first = true;
  const seen = new Promise<void>(resolve => { issued = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
  f.storage.fs.readFile = async path => { if (first) { first = false; issued(); await gate; } return read(path); };
  // Chosen while offline (thread-selection offlineSelection) and adopted connected without its opening.
  f.client.threadId = 'b';
  const selecting = mobileThreadSelection(f.client, 'b', f.native, f.storage);
  await seen;
  expect((await f.client.command('unsettle', '', '', 0, f.native, f.storage)).message)
    .toBe('Wait for synchronization and check your connection permissions.');
  expect(f.client.busy).toBe(false);
  release();
  expect((await selecting).message).toBe('');
  expect(f.client.threadSubscription).toBe('');
  expect(f.calls.some(bounded('b'))).toBe(false);
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: true, selectionReopen: true });
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
  expect(f.facts('focused', 'b').selectionReopen).toBe(false);
});

test('every route waits while an environment switch is in flight, then the switch owns the result', async () => {
  const f = held(), entry = remote('connected', f.client.shell), connect = f.hold(request => request.op === 'connect');
  fleet.entries.set(entry.key, entry);
  const switching = mobileThreadSelection(f.client, 'fleet:remote:a', f.native, f.storage);
  await connect.issued;
  // The focus has already forgotten the background entry, which changes the target's own key.
  expect(fleet.entries.has(entry.key)).toBe(false);
  expect(f.facts('remote', 'a').selectionReady).toBe(false);
  expect(f.facts('focused', 'b').selectionReady).toBe(false);
  connect.release();
  expect((await switching).message).toBe('');
  expect(f.client.environmentId).toBe('remote');
  expect(f.client.threadId).toBe('a');
  expect(f.facts('focused', 'b').selectionReady).toBe(true);
  expect(f.facts('remote', 'a').selectionReopen).toBe(false);
  expect(f.calls.filter(call => call.op === 'connect')).toHaveLength(1);
});

test('a selection a later command interrupts after it chose the thread opens it again once', async () => {
  const f = held(), b = f.hold(bounded('b'));
  const selecting = mobileThreadSelection(f.client, 'b', f.native, f.storage);
  await b.issued;
  const before = f.facts('focused', 'b');
  // A refused write still takes the shared command epoch before requireWrite.
  expect((await f.client.command('unsettle', '', '', 0, f.native, f.storage)).message)
    .toBe('Wait for synchronization and check your connection permissions.');
  b.release();
  expect((await selecting).message).toBe('');
  // Under the old gate the route now equals data, so nothing opened B.
  expect(f.client.threadId).toBe('b');
  expect(f.client.thread).toBeNull();
  expect(f.client.threadSubscription).toBe('');
  const after = f.facts('focused', 'b');
  expect(after).toMatchObject({ selectionReady: true, selectionReopen: true });
  expect(after.selectionKey).not.toBe(before.selectionKey);
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(obj(f.client.thread?.projection.thread).id).toBe('b');
  expect(f.client.threadSubscription).toBe('b');
  expect(f.facts('focused', 'b')).toEqual({ ...after, selectionReopen: false });
});

test('the retry waits for the interrupting command and never preempts it', async () => {
  const f = held(), present = f.hold(request => request.op === 'devicePresentation');
  // Interrupted before the shared client chose the thread.
  const selecting = mobileThreadSelection(f.client, 'b', f.native, f.storage);
  await present.issued;
  const read = f.hold(request => request.op === 'devicePresentation');
  const history = f.client.command('history', '', '', 0, f.native, f.storage);
  present.release();
  expect((await selecting).message).toBe('');
  expect(f.client.threadId).toBe('');
  await read.issued;
  expect(f.client.busy).toBe(true);
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: false, selectionReopen: false });
  read.release();
  expect((await history).message).toBe('');
  expect(f.client.busy).toBe(false);
  expect(f.facts('focused', 'b').selectionReady).toBe(true);
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
});

test('repeating the selection the client is already opening issues nothing and settles with it', async () => {
  const f = held(), b = f.hold(bounded('b'));
  const first = mobileThreadSelection(f.client, 'b', f.native, f.storage);
  await b.issued;
  const issued = f.calls.length;
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.calls).toHaveLength(issued);
  // Counted while the opening command is busy, so the route waits instead of repeating.
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: false, selectionReopen: true });
  b.release();
  expect((await first).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
  expect(f.facts('focused', 'b')).toMatchObject({ selectionReady: true, selectionReopen: false });
});

test('a selection does not take over a write that captured its target', async () => {
  const f = held(), request = f.hold(input => input.op === 'request');
  await mobileThreadSelection(f.client, 'a', f.native, f.storage);
  const writing = f.client.dispatch(f.native, f.storage, { type: 'thread.unsettle', commandId: 'command', threadId: 'a', reason: 'user' }, 'Un-settle thread');
  await request.issued;
  expect((await mobileThreadSelection(f.client, 'b', f.native, f.storage)).message).toBe('');
  expect(f.client.threadSubscription).toBe('b');
  expect(f.client.local.pending.focused).toMatchObject({ threadId: 'a', uncertain: false });
  request.release();
  await writing;
  expect(f.client.local.pending.focused).toBeUndefined();
  const sent = f.calls.filter(call => call.op === 'request');
  expect(sent).toHaveLength(1);
  expect(obj(sent[0]!.payload)).toMatchObject({ threadId: 'a' });
  expect(sent[0]!.generation).toBe(1);
  expect(f.client.threadId).toBe('b');
});
