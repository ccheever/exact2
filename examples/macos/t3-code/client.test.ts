import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot, diffPresentation, transcriptPresentation } from './presentation';
import { parsePairing, nativeFiles, type Native, type Files } from './protocol';
import { obj, arr, str, type Obj } from './domain';
import { answer } from './app';

const timestamp = '2026-10-03T01:00:00.000Z';
const thread = (id = 't1'): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, runtimeMode: 'approval-required', interactionMode: 'default' });
function detail(id = 't1'): Obj {
  return { snapshotSequence: 10, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0,
    projection: { thread: thread(id), runs: [], attempts: [], nodes: [], subagents: [], providerSessions: [], providerThreads: [], providerTurns: [],
      runtimeRequests: [], messages: [], plans: [], turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [], contextTransfers: [], visibleTurnItems: [], updatedAt: timestamp } };
}
function storage(initial?: Obj) {
  let saved = initial ? JSON.stringify(initial) : '', writes = 0;
  const files: Files = { fs: {
    async mkdir() {},
    async readFile() { if (!saved) throw new Error('not found'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { writes++; saved = new TextDecoder().decode(bytes); },
  } };
  return { files, data: () => obj(JSON.parse(saved || '{}')), writes: () => writes };
}
class Backend implements Native {
  available = true;
  calls: Obj[] = [];
  watchers: string[] = [];
  generation = 1;
  sequence = 20;
  eventSequence = 0;
  serial = 0;
  autoMarkers = true;
  pageSize = 100;
  subscriptions: Record<string, string> = {};
  events: Obj[] = [];
  scopes = ['orchestration:read', 'orchestration:operate'];
  failure: '' | 'before' | 'after' = '';
  preferencesText = '';
  seen = new Set<string>();
  committed: Obj[] = [];
  shell: Obj = { snapshotSequence: 10, projects: [{ id: 'p1', title: 'Example', workspaceRoot: '/repo' }], threads: [thread(), thread('t2')], archivedThreads: [] };
  details: Record<string, Obj> = { t1: detail(), t2: detail('t2') };
  config: Obj = { environment: { environmentId: 'env1', orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'codex-personal', driver: 'codex', displayName: 'Codex', enabled: true, installed: true,
      auth: { status: 'authenticated' }, status: 'ready', supportedRuntimeModes: ['approval-required', 'full-access'],
      models: [{ slug: 'model-a', name: 'Model A', isDefault: true }, { slug: 'model-b', name: 'Model B' }] }],
    settings: { defaultModelSelection: { instanceId: 'codex-personal', model: 'model-a' }, defaultRuntimeMode: 'full-access' },
    shellResumeCompletionMarker: true, threadResumeCompletionMarker: true };
  watch(topic: string) { this.watchers.push(topic); }
  good(value: unknown) { return { ok: true, generation: this.generation, value }; }
  bad(message: string, uncertain = false) { return { ok: false, generation: this.generation, error: { kind: 'transport', message, uncertain } }; }
  status() { return { state: 'connected', origin: 'http://127.0.0.1:3773', environmentId: 'env1', message: '', descriptor: this.config.environment }; }
  emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) {
    this.events.push({ seq: ++this.eventSequence, generation: this.generation, key, subscriptionId, value });
  }
  emitDomain(type: string, payload: Obj, threadId = 't1') {
    this.emit('thread', { kind: 'event', sequence: ++this.sequence,
      event: { id: `e${this.sequence}`, type, threadId, occurredAt: timestamp, payload } });
  }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (typeof request.generation === 'number' && request.generation !== this.generation) return this.bad('stale');
    const op = request.op;
    if (op === 'readPreferences') return this.good({ text: this.preferencesText });
    if (op === 'writePreferences') { this.preferencesText = str(request.text); return this.good({}); }
    if (op === 'status') return this.good(this.status());
    if (op === 'connect') { this.generation++; this.subscriptions = {}; return this.good(this.status()); }
    if (op === 'disconnect') { this.generation++; return this.good({ ...this.status(), state: 'disconnected' }); }
    if (op === 'ids') return this.good(Array.from({ length: Number(request.count) }, () => `uuid-${++this.serial}`));
    if (op === 'http') {
      if (request.path === '/api/auth/session') return this.good({ authenticated: true, scopes: this.scopes });
      if (request.path === '/api/orchestration/shell') return this.good(this.shell);
      const found = str(request.path).match(/\/threads\/([^/]+)\/bounded/);
      if (found) return this.good(this.details[found[1]]);
      if (str(request.path).includes('/history?')) return this.good({ snapshotSequence: this.sequence, items: [], nextCursor: null, hasMoreHistory: false });
      return this.bad('not found');
    }
    if (op === 'subscribe') {
      const key = str(request.key), id = `sub-${++this.serial}`;
      this.subscriptions[key] = id;
      if (key === 'config') this.emit(key, { type: 'snapshot', config: this.config });
      if (key !== 'config' && this.autoMarkers) this.emit(key, { kind: 'synchronized' });
      return this.good({ id });
    }
    if (op === 'unsubscribe') { delete this.subscriptions[str(request.key)]; return this.good({}); }
    if (op === 'events') return this.good({ events: this.events.filter(event => Number(event.seq) > Number(request.after)).slice(0, this.pageSize), latest: this.eventSequence, reset: false });
    if (op === 'ack') { this.events = this.events.filter(event => Number(event.seq) > Number(request.through)); return this.good({ latest: this.eventSequence }); }
    if (op === 'request') {
      if (request.method === 'server.getConfig') return this.good(this.config);
      if (request.method === 'orchestration.getFullThreadDiff') return this.good({ diff: 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new' });
      const payload = obj(request.payload);
      if (this.failure === 'before') { this.failure = ''; return this.bad('lost connection', true); }
      if (!this.seen.has(str(payload.commandId))) {
        this.seen.add(str(payload.commandId)); this.committed.push(payload);
        this.commit(str(request.method), payload);
      }
      if (this.failure === 'after') { this.failure = ''; return this.bad('lost acknowledgment', true); }
      if (request.method === 'orchestration.launchThread') return this.good({ threadId: payload.threadId, projection: obj(this.details[str(payload.threadId)]).projection, resumed: false });
      return this.good({ sequence: this.sequence });
    }
    return this.bad(`Unknown operation ${String(op)}`);
  }
  commit(method: string, payload: Obj) {
    if (method === 'orchestration.launchThread') {
      const id = str(payload.threadId), created = { ...thread(id), projectId: payload.projectId, title: payload.title, modelSelection: payload.modelSelection };
      this.details[id] = detail(id); obj(this.details[id].projection).thread = created;
      arr(this.shell.threads).push(created);
      this.emit('shell', { kind: 'thread.updated', sequence: ++this.sequence, location: 'active', thread: created });
    } else if (method === 'projects.mutate') {
      const project = { id: payload.projectId, title: payload.title, workspaceRoot: payload.workspaceRoot };
      arr(this.shell.projects).push(project);
      this.emit('shell', { kind: 'project.updated', sequence: ++this.sequence, project });
    } else if (payload.type === 'message.dispatch') {
      const message = { id: payload.messageId, threadId: payload.threadId, role: 'user', text: payload.text };
      const projection = obj(this.details[str(payload.threadId)].projection);
      (projection.messages as Obj[]).push(message);
      this.emitDomain('message.updated', message, str(payload.threadId));
    }
  }
}
async function connected() {
  const client = new T3Client(), native = new Backend(), disk = storage();
  await client.refresh(native, disk.files);
  return { client, native, disk, command: (op: string, id = '', value = '', n = 0) => client.command(op, id, value, n, native, disk.files) };
}
async function opened() {
  const context = await connected();
  await context.command('select-thread', 't1');
  await context.client.refresh(context.native, context.disk.files);
  return context;
}

describe('desktop presentation and model options', () => {
  const capabilities = { optionDescriptors: [{ id: 'reasoningEffort', type: 'select', label: 'Reasoning', currentValue: 'medium',
    options: [{ id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] }] };

  test('reasoning control follows the catalog and sends canonical option selections', async () => {
    const { client, native, command } = await opened();
    arr(arr(client.config.providers)[0]?.models)[0]!.capabilities = capabilities;
    client.modelOptions = [{ id: 'serviceTier', value: 'priority' }];
    expect(snapshot(client).optionLabel).toBe('Medium');
    await command('model-option', 'reasoningEffort', 'high');
    expect(native.committed.at(-1)?.modelSelection).toEqual({ instanceId: 'codex-personal', model: 'model-a',
      options: [{ id: 'serviceTier', value: 'priority' }, { id: 'reasoningEffort', value: 'high' }] });
    expect(snapshot(client).optionLabel).toBe('High');
    const count = native.committed.length;
    await command('model-option', 'reasoningEffort', 'unknown');
    expect(native.committed.length).toBe(count);
    expect(client.error).toContain('no longer offered');
  });

  test('a new thread retains chosen reasoning options in its launch', async () => {
    const { client, native, command } = await connected();
    arr(arr(client.config.providers)[0]?.models)[0]!.capabilities = capabilities;
    await command('model-option', 'reasoningEffort', 'low');
    await command('send', '', 'Build a small app');
    expect(obj(native.committed.at(-1)?.modelSelection).options).toEqual([{ id: 'reasoningEffort', value: 'low' }]);
  });

  test('settled shelves and ages use server state and the supplied clock', async () => {
    const { client, native, command } = await opened();
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', updatedAt: timestamp });
    Object.assign(obj(client.projection.thread), { settledOverride: 'settled' });
    const data = snapshot(client, Date.parse(timestamp) + 120_000);
    expect(data.threads[0]).toMatchObject({ section: 'settled', age: '2m' });
    expect(data.settled).toBe(true);
    await command('unsettle');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.unsettle', threadId: 't1', reason: 'user' });
  });

  test('compact work disclosures preserve tool output, failures and compaction boundaries', async () => {
    const { client } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', status: 'completed', startedAt: timestamp, completedAt: '2026-10-03T01:02:51.000Z' }];
    client.thread!.projection.visibleTurnItems = [
      { sourceThreadId: 't1', sourceItemId: 'tool', item: { id: 'tool', type: 'command_execution', input: 'cargo test', output: '585 tests passed', runId: 'r1' } },
      { sourceThreadId: 't1', sourceItemId: 'compact', item: { id: 'compact', type: 'compaction', summary: 'Preserved context' } },
      { sourceThreadId: 't1', sourceItemId: 'error', item: { id: 'error', type: 'error', failure: { message: 'Provider failed' } } },
      { sourceThreadId: 't1', sourceItemId: 'answer', item: { id: 'answer', type: 'assistant_message', text: 'No findings.' } },
    ];
    const rows = transcriptPresentation(client);
    expect(rows.map(row => row.kind)).toEqual(['work', 'system', 'error', 'assistant']);
    expect(rows[0]).toMatchObject({ title: 'Worked for 2m 51s', body: 'Command\ncargo test\n\n585 tests passed' });
    expect(rows[1]?.title).toBe('Context compacted');
    expect(rows[2]?.body).toBe('Provider failed');
    expect(rows[3]?.body).toBe('No findings.');
  });
});

function abandonNext(native: Backend, matches: (request: Obj) => boolean): Promise<void> {
  const original = native.later.bind(native);
  let reached!: () => void;
  const waiting = new Promise<void>(resolve => { reached = resolve; });
  let abandoned = false;
  native.later = async input => {
    const result = await original(input);
    if (!abandoned && matches(obj(input))) {
      abandoned = true; reached();
      // Exact forgets the callback without rejecting its Promise or running finally.
      return new Promise<never>(() => {});
    }
    return result;
  };
  return waiting;
}

describe('answer cancellation', () => {
  const stages: [string, (request: Obj) => boolean][] = [
    ['status', request => request.op === 'status'],
    ['authentication', request => request.path === '/api/auth/session'],
    ['configuration', request => request.method === 'server.getConfig'],
    ['configuration subscription', request => request.op === 'subscribe' && request.key === 'config'],
    ['shell snapshot', request => request.path === '/api/orchestration/shell'],
    ['shell subscription', request => request.op === 'subscribe' && request.key === 'shell'],
    ['thread snapshot', request => str(request.path).includes('/bounded')],
    ['thread subscription', request => request.op === 'subscribe' && request.key === 'thread'],
    ['event read', request => request.op === 'events'],
    ['event acknowledgment', request => request.op === 'ack'],
  ];
  test.each(stages)('a snapshot abandoned during %s does not block the next answer', async (_stage, matches) => {
    const client = new T3Client(), native = new Backend();
    const disk = storage({ version: 1, selections: { env1: { projectId: 'p1', threadId: 't1' } } });
    const abandoned = abandonNext(native, matches);
    void client.refresh(native, disk.files);
    await abandoned;
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(true);
    expect(snapshot(client).providers[0].id).toBe('codex-personal');
    expect(client.threadId).toBe('t1');
  });
  test('a discarded speculative status probe cannot supersede an active event read', async () => {
    const { client, native, disk } = await opened();
    const original = native.later.bind(native);
    let held = false, discardStatus = false;
    let reached!: () => void, probed!: () => void, release!: () => void;
    const waiting = new Promise<void>(resolve => { reached = resolve; });
    const probing = new Promise<void>(resolve => { probed = resolve; });
    native.later = async input => {
      const request = obj(input);
      if (request.op === 'status' && discardStatus) {
        probed();
        // Exact discards an optimistic asynchronous reread before dispatching
        // its request; the preserved answer still owns its original reply.
        return new Promise<never>(() => {});
      }
      const response = await original(input);
      if (request.op === 'events' && !held) {
        held = true; reached();
        await new Promise<void>(resolve => { release = resolve; });
      }
      return response;
    };
    native.emit('shell', { kind: 'thread.updated', sequence: 41, location: 'active', thread: thread('retained') });
    const active = client.refresh(native, disk.files);
    await waiting;
    discardStatus = true;
    void client.refresh(native, disk.files);
    await probing;
    release();
    await active;
    expect(client.shell.threads.some(row => row.id === 'retained')).toBe(true);
    expect(native.events).toEqual([]);
    expect(client.ready).toBe(true);
    expect(client.error).toBe('');
  });
  test('a late older status reply cannot replace a newer completed refresh', async () => {
    const { client, native, disk } = await opened();
    const original = native.later.bind(native);
    let held = false, reached!: () => void, release!: () => void;
    const waiting = new Promise<void>(resolve => { reached = resolve; });
    native.later = async input => {
      if (obj(input).op === 'status' && !held) {
        held = true; reached();
        await new Promise<void>(resolve => { release = resolve; });
        return native.good({ ...native.status(), state: 'disconnected' });
      }
      return original(input);
    };
    const older = client.refresh(native, disk.files);
    await waiting;
    await client.refresh(native, disk.files);
    release();
    await older;
    expect(client.connection).toBe('connected');
    expect(client.ready).toBe(true);
    expect(client.error).toBe('');
  });
  test('an abandoned preferences read is retried by the next answer', async () => {
    const client = new T3Client(), native = new Backend();
    const disk = storage({ version: 1, sidebarWidth: 310, drafts: { 'env1:new:p1': 'Saved draft' } });
    const read = disk.files.fs.readFile;
    let first = true, reached!: () => void;
    const waiting = new Promise<void>(resolve => { reached = resolve; });
    disk.files.fs.readFile = async path => {
      if (first) { first = false; reached(); return new Promise<never>(() => {}); }
      return read(path);
    };
    void client.refresh(native, disk.files);
    await waiting;
    await client.refresh(native, disk.files);
    expect(client.local.sidebarWidth).toBe(310);
    expect(client.draft).toBe('Saved draft');
  });
  test('an abandoned storage write cannot trap later local edits behind its Promise', async () => {
    const { client, native, disk } = await connected();
    const write = disk.files.fs.atomicWriteFile;
    let first = true, reached!: () => void;
    const waiting = new Promise<void>(resolve => { reached = resolve; });
    disk.files.fs.atomicWriteFile = async (path, bytes) => {
      await write(path, bytes);
      if (first) { first = false; reached(); return new Promise<never>(() => {}); }
    };
    void client.command('draft', '', 'Old draft', 0, native, disk.files);
    await waiting;
    await client.command('draft', '', 'New draft', 0, native, disk.files);
    expect(obj(disk.data().drafts)['env1:new:p1']).toBe('New draft');
  });
  test('a superseding thread command recovers an abandoned selection', async () => {
    const { client, native, disk, command } = await connected();
    const abandoned = abandonNext(native, request => str(request.path).includes('/bounded'));
    void command('select-thread', 't1');
    await abandoned;
    await command('select-thread', 't2');
    await client.refresh(native, disk.files);
    expect(client.busy).toBe(false);
    expect(client.threadId).toBe('t2');
    expect(client.ready).toBe(true);
  });
  test('an inbox event arriving between read and ACK is drained using the ACK watermark', async () => {
    const { client, native, disk } = await connected();
    const original = native.later.bind(native);
    let injected = false;
    native.later = async input => {
      if (obj(input).op === 'ack' && !injected) {
        injected = true;
        native.emit('shell', { kind: 'thread.updated', sequence: 42, location: 'active', thread: thread('late') });
      }
      return original(input);
    };
    native.emit('shell', { kind: 'thread.updated', sequence: 41, location: 'active', thread: thread('early') });
    await client.refresh(native, disk.files);
    expect(client.shell.threads.map(row => row.id)).toContain('late');
    expect(native.events).toEqual([]);
  });
  test('an empty overflow reset is acknowledged before resnapshot and completion markers are drained', async () => {
    const { client, native, disk } = await connected();
    const original = native.later.bind(native);
    let reset = true;
    native.later = async input => {
      if (obj(input).op === 'events' && reset) {
        reset = false; native.eventSequence = 100; native.events = [];
        return native.good({ events: [], latest: 100, reset: true });
      }
      return original(input);
    };
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(true);
    expect(native.calls.some(call => call.op === 'ack' && call.through === 100)).toBe(true);
    expect(native.events).toEqual([]);
  });
});

describe('native preference persistence', () => {
  test('the app data source never starts Exact filesystem work', async () => {
    const native = new Backend();
    const forbidden = async () => { throw new Error('Exact filesystem must not be used by this app'); };
    const files: Files = { fs: { mkdir: forbidden, readFile: forbidden, atomicWriteFile: forbidden } };
    const state = await answer('snapshot', [], null, files, native);
    expect(obj(state).syncComplete).toBe(true);
    await answer('command', ['draft', '', 'Bridge draft', 0], null, files, native);
    expect(obj(obj(JSON.parse(native.preferencesText)).drafts)['env1:new:p1']).toBe('Bridge draft');
  });
  test('loads and saves the versioned preference file solely through native tickets', async () => {
    const native = new Backend(), files = nativeFiles(native), client = new T3Client();
    await client.refresh(native, files);
    await client.command('draft', '', 'Native saved draft', 0, native, files);
    await client.command('sidebar', '', 'closed', 315, native, files);
    const relaunched = new T3Client();
    await relaunched.refresh(native, nativeFiles(native));
    expect(relaunched.draft).toBe('Native saved draft');
    expect(relaunched.local.sidebarWidth).toBe(315);
    expect(relaunched.local.sidebarOpen).toBe(false);
    expect(obj(JSON.parse(native.preferencesText)).version).toBe(1);
    expect(native.calls.filter(call => call.op === 'writePreferences')).toHaveLength(2);
  });
  test('a preference read failure is visible and retryable without overwriting pending operations', async () => {
    const native = new Backend(), files = nativeFiles(native), client = new T3Client();
    const original = native.later.bind(native);
    let fail = true;
    native.preferencesText = JSON.stringify({ version: 1, drafts: { 'env1:new:p1': 'Saved before failure' } });
    native.later = async input => {
      if (obj(input).op === 'readPreferences' && fail) return { ok: false, generation: 1, error: { kind: 'Persistence', message: 'Read denied', uncertain: false } };
      return original(input);
    };
    await client.refresh(native, files);
    expect(client.error).toBe('Read denied');
    expect(native.calls.some(call => call.op === 'writePreferences')).toBe(false);
    fail = false;
    await client.refresh(native, files);
    expect(client.ready).toBe(true);
    expect(client.draft).toBe('Saved before failure');
  });
  test('a failed native operation save prevents the server mutation', async () => {
    const native = new Backend(), files = nativeFiles(native), client = new T3Client();
    await client.refresh(native, files);
    const original = native.later.bind(native);
    native.later = async input => obj(input).op === 'writePreferences'
      ? { ok: false, generation: 1, error: { kind: 'Persistence', message: 'Write denied', uncertain: false } }
      : original(input);
    await client.command('send', '', 'Keep this draft', 0, native, files);
    expect(native.committed).toEqual([]);
    expect(client.draft).toBe('Keep this draft');
    expect(client.error).toContain('Could not save this operation');
  });
});

describe('connection and bootstrap', () => {
  test('bake stays disconnected without storage or native work', async () => {
    const client = new T3Client();
    await client.refresh(null, undefined as unknown as Files);
    const state = snapshot(client);
    expect(state.available).toBe(false);
    expect(state.canSend).toBe(false);
    expect(state.serverUrl).toBe('http://127.0.0.1:3773');
    expect(state.sidebarWidth).toBe(256);
    expect(state.messages).toEqual([]);
  });
  test('parses token fragments, hosted links and credential-field URLs without retaining secrets in origins', () => {
    expect(parsePairing('https://app.t3.codes/pair?host=http%3A%2F%2F127.0.0.1%3A3773#token=secret', '')).toEqual({ origin: 'http://127.0.0.1:3773', credential: 'secret' });
    expect(parsePairing('http://old-host:3773', 'http://new-host:3773/pair?token=query#token=fragment')).toEqual({ origin: 'http://new-host:3773', credential: 'fragment' });
    expect(() => parsePairing('file:///etc/passwd', 'x')).toThrow('HTTP');
  });
  test('subscribes from snapshot cursors and gates writes on synchronization markers', async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    native.autoMarkers = false;
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(false);
    const subscription = native.calls.find(call => call.method === 'orchestration.subscribeShell')!;
    expect(subscription).toMatchObject({ generation: 1, payload: { afterSequence: 10, requestCompletionMarker: true } });
    native.emit('shell', { kind: 'synchronized' });
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(true);
    expect(client.runtimeMode).toBe('approval-required');
    expect(client.interactionMode).toBe('default');
    expect(native.watchers).toContain('t3.events');
  });
  test('a misrouted events reply cannot replace the last valid connection status', async () => {
    const { client, native, disk } = await opened();
    const previous = { generation: client.generation, connection: client.connection, origin: client.origin,
      environmentId: client.environmentId, statusMessage: client.statusMessage, threadId: client.threadId,
      shell: client.shell, thread: client.thread, config: client.config };
    const original = native.later.bind(native);
    native.later = async input => obj(input).op === 'status'
      ? native.good({ events: [], reset: false, latest: 184 }) : original(input);
    await client.refresh(native, disk.files);
    expect(client).toMatchObject(previous);
    expect(client.ready).toBe(true);
    expect(client.error).toBe('The native bridge returned an invalid connection status. Reconnect and try again.');
  });
  test.each(['state', 'origin', 'environmentId', 'message'])('rejects an invalid status %s before adopting other fields', async field => {
    const { client, native, disk } = await opened();
    const original = native.later.bind(native);
    native.later = async input => obj(input).op === 'status'
      ? native.good({ ...native.status(), origin: 'http://other-server:3773', [field]: field === 'state' ? 'unknown' : null })
      : original(input);
    await client.refresh(native, disk.files);
    expect(client.origin).toBe('http://127.0.0.1:3773');
    expect(client.connection).toBe('connected');
    expect(client.threadId).toBe('t1');
    expect(client.ready).toBe(true);
    expect(client.error).toContain('invalid connection status');
  });
  test('a genuine non-connected status requires synchronization even if the generation is unchanged', async () => {
    const { client, native, disk } = await opened();
    const original = native.later.bind(native);
    let state = 'reconnecting';
    native.later = async input => obj(input).op === 'status'
      ? native.good({ ...native.status(), state }) : original(input);
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(false);
    expect(client.connection).toBe('reconnecting');
    const previous = { ...native.subscriptions };
    state = 'connected';
    await client.refresh(native, disk.files);
    expect(client.generation).toBe(1);
    expect(client.ready).toBe(true);
    expect(client.error).toBe('');
    for (const key of ['config', 'shell', 'thread']) expect(native.subscriptions[key]).not.toBe(previous[key]);
  });
  test('requires explicit model selection when the configured default is unavailable', async () => {
    const client = new T3Client(), native = new Backend(), disk = storage();
    native.config.settings = { defaultModelSelection: { instanceId: 'missing', model: 'model-a' } };
    await client.refresh(native, disk.files);
    expect(client.providerId).toBe(''); expect(client.modelId).toBe('');
    expect(snapshot(client).canSend).toBe(false);
    await client.command('provider', 'codex-personal', '', 0, native, disk.files);
    expect(client.modelId).toBe('model-a');
  });
  test('read-only credentials display history and reject mutation dispatch', async () => {
    const client = new T3Client(), native = new Backend(), disk = storage(); native.scopes = ['orchestration:read'];
    await client.refresh(native, disk.files);
    expect(client.ready).toBe(true); expect(snapshot(client).canSend).toBe(false);
    await client.command('send', '', 'must not send', 0, native, disk.files);
    expect(native.committed).toEqual([]);
  });
});

describe('commands and uncertain outcomes', () => {
  test('creates a root thread with explicit safe modes and stable instance model routing', async () => {
    const { client, native, disk, command } = await connected();
    await command('send', '', 'Build the feature');
    const payload = native.committed[0];
    expect(payload).toMatchObject({ runtimeMode: 'approval-required', interactionMode: 'default', workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, initialMessage: { text: 'Build the feature', attachments: [] } });
    expect(str(payload.commandId)).not.toBe(str(payload.threadId));
    expect(client.threadId).toBe(payload.threadId as string);
    expect(obj(disk.data().drafts)[`env1:new:p1`]).toBeUndefined();
  });
  test('sends auto delivery intent, retains uncertain payload and retries only on explicit action with the same IDs', async () => {
    const { client, native, disk, command } = await opened(); native.failure = 'before';
    await command('send', '', 'Important draft');
    const pending = client.pending!;
    expect(pending.uncertain).toBe(true);
    expect(pending.payload).toMatchObject({ type: 'message.dispatch', deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' }, text: 'Important draft', attachments: [] });
    expect(client.draft).toBe('Important draft');
    const writesBefore = native.calls.filter(call => call.method === 'orchestration.dispatchCommand').length;
    await client.refresh(native, disk.files);
    expect(native.calls.filter(call => call.method === 'orchestration.dispatchCommand')).toHaveLength(writesBefore);
    await command('retry');
    const writes = native.calls.filter(call => call.method === 'orchestration.dispatchCommand');
    expect(writes.map(write => obj(write.payload).commandId)).toEqual([pending.payload.commandId, pending.payload.commandId]);
    expect(client.pending).toBeUndefined(); expect(client.draft).toBe('');
  });
  test('reconciles an accepted send whose acknowledgment was lost without sending again', async () => {
    const { client, native, disk, command } = await opened(); native.failure = 'after';
    await command('send', '', 'Accepted once');
    expect(client.pending?.uncertain).toBe(true);
    await client.refresh(native, disk.files);
    expect(client.pending).toBeUndefined(); expect(client.draft).toBe('');
    expect(native.committed).toHaveLength(1);
  });
  test('restores uncertain write identity and drafts from app storage after restart', async () => {
    const { client, native, disk, command } = await opened(); native.failure = 'before';
    await command('send', '', 'Keep me');
    const restarted = new T3Client(); await restarted.refresh(native, disk.files);
    expect(restarted.pending?.payload.commandId).toBe(client.pending?.payload.commandId);
    expect(restarted.pending?.uncertain).toBe(true); expect(restarted.draft).toBe('Keep me');
  });
  test('stop targets the active run and holds the queue', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'running' }];
    await command('stop');
    expect(native.committed[0]).toMatchObject({ type: 'run.interrupt', runId: 'r2', holdQueue: true, threadId: 't1' });
  });
  test('project add derives optional title from the server path and never creates the folder', async () => {
    const { native, command } = await connected();
    await command('add-project', '/Users/me/projects/new-app/', '');
    expect(native.committed[0]).toMatchObject({ type: 'project.create', title: 'new-app', workspaceRoot: '/Users/me/projects/new-app/', createWorkspaceRootIfMissing: false });
  });
});

describe('streams and requests', () => {
  test('hydrates chunked native JSON and releases it before applying the snapshot', async () => {
    class ChunkedBackend extends Backend {
      parts: string[] = [];
      released = false;
      override async later(input: unknown): Promise<unknown> {
        const request = obj(input);
        if (request.op === 'readChunk') return this.good({ text: this.parts[Number(request.index)] });
        if (request.op === 'releaseChunk') { this.released = true; return this.good({}); }
        if (request.op === 'http' && request.path === '/api/orchestration/shell') {
          const json = JSON.stringify(this.shell);
          const middle = Math.floor(json.length / 2);
          this.parts = [json.slice(0, middle), json.slice(middle)];
          return this.good({ _nativeTransfer: { id: 'large-shell', parts: 2 } });
        }
        return super.later(input);
      }
    }
    const client = new T3Client(), native = new ChunkedBackend(), disk = storage();
    await client.refresh(native, disk.files);
    expect(client.shell.projects[0].title).toBe('Example');
    expect(client.ready).toBe(true); expect(native.released).toBe(true);
  });
  test('an older history response cannot replace a newer snapshot cursor', async () => {
    let deliver: ((response: unknown) => void) | undefined;
    let observed: (() => void) | undefined;
    const waiting = new Promise<void>(resolve => { observed = resolve; });
    class HistoryBackend extends Backend {
      override async later(input: unknown): Promise<unknown> {
        if (str(obj(input).path).includes('/history?')) {
          observed!();
          return new Promise(resolve => { deliver = resolve; });
        }
        return super.later(input);
      }
    }
    const client = new T3Client(), native = new HistoryBackend(), disk = storage();
    await client.refresh(native, disk.files);
    await client.command('select-thread', 't1', '', 0, native, disk.files);
    await client.refresh(native, disk.files);
    client.thread!.historyCursor = 'old'; client.thread!.hasMore = true;
    const command = client.command('history', '', '', 0, native, disk.files);
    await waiting;
    client.thread!.historyCursor = 'new-snapshot';
    deliver!(native.good({ snapshotSequence: 10, items: [], nextCursor: 'old-page', hasMoreHistory: true }));
    await command;
    expect(client.thread!.historyCursor).toBe('new-snapshot');
  });
  test('old completion and error markers cannot affect a replacement thread subscription', async () => {
    const { client, native, disk, command } = await opened();
    const oldId = native.subscriptions.thread;
    native.autoMarkers = false;
    await command('select-thread', 't2');
    native.emit('thread', { kind: 'synchronized' }, oldId);
    native.emit('thread', { _transportError: { message: 'old failure' } }, oldId);
    await client.refresh(native, disk.files);
    expect(client.threadId).toBe('t2'); expect(client.threadLive).toBe(false); expect(client.error).not.toBe('old failure');
    native.emit('thread', { kind: 'synchronized' });
    await client.refresh(native, disk.files); expect(client.threadLive).toBe(true);
  });
  test('an inbox read cannot acknowledge a new thread marker before its subscription reply arrives', async () => {
    const { client, native, disk, command } = await opened();
    const original = native.later.bind(native);
    let readHeld = false, subscriptionHeld = false;
    let readReached!: () => void, subscriptionReached!: () => void;
    let releaseRead!: () => void, releaseSubscription!: () => void;
    const reading = new Promise<void>(resolve => { readReached = resolve; });
    const subscribing = new Promise<void>(resolve => { subscriptionReached = resolve; });
    native.later = async input => {
      const request = obj(input);
      if (request.op === 'events' && !readHeld) {
        readHeld = true; readReached();
        // Capture the inbox only after the new subscription has emitted its marker.
        await new Promise<void>(resolve => { releaseRead = resolve; });
      }
      const response = await original(input);
      if (request.op === 'subscribe' && request.key === 'thread' && !subscriptionHeld) {
        subscriptionHeld = true; subscriptionReached();
        await new Promise<void>(resolve => { releaseSubscription = resolve; });
      }
      return response;
    };
    const oldRead = client.refresh(native, disk.files);
    await reading;
    const selection = command('select-thread', 't2');
    await subscribing;
    const marker = native.events.find(entry => entry.subscriptionId === native.subscriptions.thread
      && obj(entry.value).kind === 'synchronized')!;
    releaseRead();
    await oldRead;
    const markerRetained = native.events.some(entry => entry.seq === marker.seq);
    releaseSubscription();
    await selection;
    await client.refresh(native, disk.files);
    expect(markerRetained).toBe(true);
    expect(client.threadId).toBe('t2');
    expect(client.threadLive).toBe(true);
    expect(client.ready).toBe(true);
    expect(native.events).toEqual([]);
  });
  test('a late command refresh cannot replace subscriptions installed by a newer snapshot synchronization', async () => {
    const { client, native, disk, command } = await connected();
    const original = native.later.bind(native);
    let held = false, reached!: () => void, release!: () => void;
    const waiting = new Promise<void>(resolve => { reached = resolve; });
    native.later = async input => {
      const request = obj(input), response = await original(input);
      if (request.op === 'subscribe' && request.key === 'shell' && !held) {
        held = true; reached();
        await new Promise<void>(resolve => { release = resolve; });
      }
      return response;
    };
    const olderCommand = command('refresh');
    await waiting;
    await client.refresh(native, disk.files);
    const readyBeforeOldReply = client.ready;
    release();
    await olderCommand;
    await client.refresh(native, disk.files);
    expect(readyBeforeOldReply).toBe(true);
    expect(client.shellLive).toBe(true);
    expect(client.ready).toBe(true);
    native.emit('shell', { kind: 'thread.updated', sequence: 40, location: 'active', thread: thread('after-overlap') });
    await client.refresh(native, disk.files);
    expect(client.shell.threads.map(row => row.id)).toContain('after-overlap');
    expect(client.error).toBe('');
    expect(native.events).toEqual([]);
  });
  test('drains multiple inbox pages and ACKs only the processed cursor', async () => {
    const { client, native, disk } = await connected(); native.pageSize = 1;
    for (let i = 0; i < 4; i++) native.emit('shell', { kind: 'thread.updated', sequence: 30 + i, location: 'active', thread: thread(`extra-${i}`) });
    await client.refresh(native, disk.files);
    expect(client.shell.threads.filter(thread => str(thread.id).startsWith('extra-'))).toHaveLength(4);
    expect(native.events).toHaveLength(0);
    expect(native.calls.filter(call => call.op === 'ack').slice(-1)[0].through).toBe(native.eventSequence);
  });
  test('approval forwards only provider-advertised choices and waits for pending live request', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.runtimeRequests = [{ id: 'approval', status: 'pending', kind: 'permission', responseCapability: { type: 'live' } }];
    client.thread!.projection.turnItems = [{ id: 'approval-item', type: 'approval_request', requestId: 'approval', options: [{ decision: 'acceptAlways', label: 'Always for this tool' }, { decision: 'decline', label: 'No' }] }];
    await command('approval', 'approval', 'accept'); expect(native.committed).toHaveLength(0);
    await command('approval', 'approval', 'acceptAlways');
    expect(native.committed[0]).toMatchObject({ type: 'runtime-request.respond', requestId: 'approval', decision: 'acceptAlways' });
  });
  test('question answers preserve exact option values and send arrays for multiple selection', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.runtimeRequests = [{ id: 'questions', status: 'pending', kind: 'user_input', responseCapability: { type: 'message' } }];
    client.thread!.projection.turnItems = [{ id: 'questions-item', type: 'user_input_request', requestId: 'questions', questions: [{ id: 'scope', header: 'Scope', question: 'Choose', multiSelect: true, allowCustomAnswer: false, options: [{ label: 'One', value: ' exact ', description: 'One' }, { label: 'Two', value: 'two', description: 'Two' }] }] }];
    await command('choice', 'questions::scope', ' exact ');
    await command('choice', 'questions::scope', 'two');
    expect(snapshot(client).questions[0].customAllowed).toBe(false);
    await command('submit-answers', 'questions');
    expect(native.committed[0]).toMatchObject({ type: 'runtime-request.respond', answers: { scope: [' exact ', 'two'] } });
  });
  test('request response reasons distinguish permanent, connection, permission and submission states', async () => {
    const { client } = await opened();
    const approval = { id: 'approval', status: 'pending', responseCapability: { type: 'live' } };
    const question = { id: 'question', status: 'pending', responseCapability: { type: 'message' } };
    client.thread!.projection.runtimeRequests = [approval, question];
    client.thread!.projection.turnItems = [
      { id: 'a', type: 'approval_request', requestId: 'approval' },
      { id: 'q', type: 'user_input_request', requestId: 'question', questions: [{ id: 'q1', question: 'Choose' }] },
    ];
    const reasons = () => { const view = snapshot(client); return [view.approvals[0], view.questions[0]]; };
    expect(reasons().map(item => [item.canRespond, item.responseReason])).toEqual([[true, ''], [true, '']]);
    client.busy = true;
    expect(reasons().every(item => !item.canRespond && item.responseReason.includes('submission'))).toBe(true);
    client.busy = false; client.scopes = ['orchestration:read'];
    expect(reasons().every(item => item.responseReason.includes('permission'))).toBe(true);
    client.scopes.push('orchestration:operate'); client.threadLive = false;
    expect(reasons().every(item => item.responseReason.includes('synchronization'))).toBe(true);
    client.connection = 'reconnecting';
    expect(reasons().every(item => item.responseReason.includes('Reconnect'))).toBe(true);
    approval.responseCapability.type = 'not_resumable'; question.responseCapability.type = 'not_resumable';
    expect(reasons().every(item => item.responseReason.includes('provider process is gone'))).toBe(true);
    approval.responseCapability.type = 'message';
    expect(reasons()[0].responseReason).toContain('no longer live');
  });
  test('diff uses checkpoint ordinal and exposes read-only unified lines', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.checkpoints = [{ id: 'cp', status: 'ready', appRunOrdinal: 7, runId: 'r7' }];
    await command('diff');
    expect(native.calls.find(call => call.method === 'orchestration.getFullThreadDiff')).toMatchObject({ payload: { threadId: 't1', toTurnCount: 7, ignoreWhitespace: false } });
    expect(snapshot(client).diffFiles).toEqual([{ id: '0', name: 'a.ts', additions: 1, deletions: 1 }]);
    expect(diffPresentation('+line\n-line').diffLines.map(line => line.kind)).toEqual(['addition', 'deletion']);
  });
  test('project filtering and draft partitioning keep different workspace input separate', async () => {
    const { client, native, disk, command } = await connected();
    client.shell.projects.push({ id: 'p2', title: 'Other', workspaceRoot: '/other' });
    client.shell.threads.push({ ...thread('other'), projectId: 'p2' });
    await command('draft', '', 'Project one');
    await command('select-project', 'p2'); await command('draft', '', 'Project two');
    expect(snapshot(client).threads.map(thread => thread.id)).toEqual(['other']);
    await command('select-project', 'p1'); expect(client.draft).toBe('Project one');
    expect(JSON.stringify(disk.data())).not.toContain('access_token');
    expect(native.committed).toHaveLength(0);
  });
});
