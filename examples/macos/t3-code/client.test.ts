import { keyboardSettings, validShortcut, validWhen } from './keybinding-settings';
import { scheduledSettings, taskDraft, taskFromArguments, validateTaskInput } from './scheduled-settings';
import { scopedSettingPatch, scopedControls, scopedSearchControls } from './scoped-settings';
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot, transcriptPresentation, modelCatalog, providerBanner, projectIdentity, providerBadge } from './presentation';
import { parsePairing, nativeFiles, type Native, type Files } from './protocol';
import { obj, arr, str, type Obj } from './domain';
import { archivedSettings, decodeNotices, licenseSettings, diagnosticsSettings, storageSettings } from './settings-data';
import { answer } from './app';
import { chatLocal } from './timeline-presentation';

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
    if (op === 'devicePresentation') return this.good({});
    if (op === 'copyText') return this.good({ copied: true });
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
      if (request.method === 'orchestration.getArchivedShellSnapshot') return this.good({ snapshotSequence: this.sequence, projects: this.shell.projects, threads: this.shell.archivedThreads || [] });
      if (request.method === 'server.getConfig') return this.good(this.config);
      if (request.method === 'server.getSettings') return this.good(this.config.settings);
      if (request.method === 'server.updateSettings') {
        const payload = obj(request.payload), patch = obj(payload.patch), old = obj(this.config.settings);
        const overrides = { ...obj(old.projectSettingsOverrides) };
        for (const [id, entry] of Object.entries(obj(patch.projectSettingsOverrides))) { if (entry === null) delete overrides[id]; else overrides[id] = entry; }
        this.config.settings = { ...old, ...patch, projectSettingsOverrides: overrides };
        const mutation = obj(payload.providerInstanceMutation);
        if (mutation.operation) {
          const providers = { ...obj(old.providerInstances) };
          if (mutation.operation === 'remove') delete providers[str(mutation.instanceId)];
          else providers[str(mutation.instanceId)] = mutation.instance;
          obj(this.config.settings).providerInstances = providers;
        }
        return this.good(this.config.settings);
      }
      if (request.method === 'orchestration.getFullThreadDiff' || request.method === 'orchestration.getTurnDiff') return this.good({ diff: 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new' });
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
    } else if (payload.type === 'thread.unarchive' || payload.type === 'thread.delete') {
      const id = str(payload.threadId);
      const archived = arr(this.shell.archivedThreads).find(thread => thread.id === id);
      this.shell.archivedThreads = arr(this.shell.archivedThreads).filter(thread => thread.id !== id);
      if (payload.type === 'thread.unarchive' && archived) this.shell.threads = [...arr(this.shell.threads), { ...archived, archivedAt: null }];
    } else if (method === 'projects.mutate') {
      const existing = arr(this.shell.projects).find(project => project.id === payload.projectId);
      if (payload.type === 'project.delete') {
        this.shell.projects = arr(this.shell.projects).filter(project => project.id !== payload.projectId);
        this.shell.threads = arr(this.shell.threads).filter(thread => thread.projectId !== payload.projectId);
        this.emit('shell', { kind: 'project.removed', sequence: ++this.sequence, projectId: payload.projectId });
      } else {
        const project = { ...existing, id: payload.projectId, title: payload.title, workspaceRoot: existing?.workspaceRoot || payload.workspaceRoot };
        this.shell.projects = [...arr(this.shell.projects).filter(entry => entry.id !== project.id), project];
        this.emit('shell', { kind: 'project.updated', sequence: ++this.sequence, project });
      }
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
    obj(obj(client.config.environment).capabilities).threadSettlement = true;
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', updatedAt: timestamp });
    Object.assign(obj(client.projection.thread), { settledOverride: 'settled' });
    const data = snapshot(client, Date.parse(timestamp) + 120_000);
    expect(data.threads.find(row => row.id === 't1')).toMatchObject({ section: 'settled', age: '2m' });
    expect(data.settled).toBe(true);
    await command('unsettle');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.unsettle', threadId: 't1', reason: 'user' });
  });

  test('sidebar age follows user activity or settling, and activity never moves a row (threadSort.ts)', async () => {
    const { client } = await opened();
    obj(obj(client.config.environment).capabilities).threadSettlement = true;
    const now = Date.parse(timestamp) + 600_000;
    Object.assign(client.shell.threads[0]!, { latestUserMessageAt: timestamp, createdAt: timestamp,
      updatedAt: new Date(now).toISOString() });
    Object.assign(client.shell.threads[1]!, { latestUserMessageAt: new Date(now - 120_000).toISOString(),
      createdAt: new Date(Date.parse(timestamp) - 60_000).toISOString(), updatedAt: timestamp });
    expect(snapshot(client, now).threads.map(row => [row.id, row.age])).toEqual([['t1', '10m'], ['t2', '2m']]);
    Object.assign(client.shell.threads[0]!, { settledOverride: 'settled', settledAt: new Date(now - 10_000).toISOString() });
    expect(snapshot(client, now).threads.find(row => row.id === 't1')).toMatchObject({ section: 'settled', age: 'now' });
    Object.assign(client.shell.threads[0]!, { settledAt: 'invalid', latestRunId: 'r1', status: 'completed',
      latestRunRequestedAt: timestamp, latestRunCompletedAt: new Date(now - 60_000).toISOString() });
    expect(snapshot(client, now).threads.find(row => row.id === 't1')).toMatchObject({ age: '1m' });
  });

  test('UTC offsets do not change sidebar chronological ordering', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { createdAt: '2026-10-03T10:00:00+09:00' });
    Object.assign(client.shell.threads[1]!, { createdAt: '2026-10-03T02:00:00Z' });
    expect(snapshot(client).threads.map(row => row.id)).toEqual(['t2', 't1']);
  });

  test('a settled run folds alone while the next run stays live', async () => {
    const { client } = await opened();
    const at = (seconds: number) => new Date(Date.parse(timestamp) + seconds * 1000).toISOString();
    for (const status of ['running', 'completed']) {
      client.thread!.projection.runs = [{ id: 'a', status: 'completed', startedAt: at(0), completedAt: at(3) },
        { id: 'b', status, startedAt: at(4), completedAt: status === 'completed' ? at(9) : null }];
      const rows = [['ua', 'user_message', 'a'], ['a1', 'command_execution', 'a'], ['aa', 'assistant_message', 'a'], ['ub', 'user_message', 'b'],
        ['b1', 'command_execution', 'b'], ...status === 'completed' ? [['ba', 'assistant_message', 'b']] : []].map(([id, type, runId], index) => ({
        sourceThreadId: 't1', sourceItemId: id, visibility: 'local', item: { id, type, runId, status: 'completed', startedAt: at(index), updatedAt: at(index),
          text: id, messageId: id, inputIntent: 'turn_start', input: id, exitCode: 0, streaming: false } }));
      client.thread!.projection.visibleTurnItems = rows;
      const result = transcriptPresentation(client);
      expect(result.map(row => row.kind)).toEqual(status === 'running'
        ? ['user', 'work', 'assistant', 'user', 'working', 'live'] : ['user', 'work', 'assistant', 'user', 'work', 'assistant']);
      expect(result[1]).toMatchObject({ title: 'Worked for 2.0s', groupId: 'a' });
      if (status === 'running') expect(result[5]).toMatchObject({ title: 'Running b1', live: true });
    }
  });

  test('provider incompatibility remains visible when the server reports ready', () => {
    const provider = { instanceId: 'fixture', displayName: 'Fixture', status: 'ready', version: '0.145.0',
      auth: { status: 'authenticated' }, compatibilityAdvisory: { status: 'broken', message: 'Use >=0.159.0.' } };
    expect(providerBanner(provider)).toMatchObject({ providerBannerTitle: 'Fixture 0.145.0 is known to be broken',
      providerBannerMessage: 'Use >=0.159.0.', providerBannerWarning: false });
    expect(providerBanner({ ...provider, version: '0.146.0' }).providerBannerKey).not.toBe(providerBanner(provider).providerBannerKey);
    expect(providerBanner({ ...provider, compatibilityAdvisory: { status: 'unsupported', message: 'Update.' } }).providerBannerWarning).toBe(true);
    expect(providerBanner({ ...provider, compatibilityAdvisory: null }).providerBannerKey).toBe('');
    expect(providerBanner({ ...provider, status: 'disabled' }).providerBannerKey).toBe('');
    expect(providerBanner({ ...provider, status: 'error', auth: { status: 'unauthenticated' } }).providerBannerTitle).toBe('Fixture is unauthenticated');
  });

  test('compact work disclosures preserve tool details, failures and compaction boundaries', async () => {
    const { client, command } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', status: 'completed', startedAt: timestamp, completedAt: '2026-10-03T01:02:51.000Z' }];
    client.thread!.projection.visibleTurnItems = [
      { sourceThreadId: 't1', sourceItemId: 'tool', item: { id: 'tool', type: 'command_execution', input: 'cargo test', output: '585 tests passed', exitCode: 0, status: 'completed', runId: 'r1' } },
      { sourceThreadId: 't1', sourceItemId: 'compact', item: { id: 'compact', type: 'compaction', status: 'completed', summary: 'Preserved context' } },
      { sourceThreadId: 't1', sourceItemId: 'error', item: { id: 'error', type: 'error', status: 'failed', failure: { message: 'Provider failed' } } },
      { sourceThreadId: 't1', sourceItemId: 'answer', item: { id: 'answer', type: 'assistant_message', status: 'completed', text: 'No findings.' } },
    ];
    let rows = transcriptPresentation(client);
    expect(rows.map(row => row.kind)).toEqual(['work', 'compaction', 'entry', 'assistant']);
    expect(rows[0]).toMatchObject({ title: 'Worked for 2m 51s' });
    expect(rows[1]?.title).toBe('Context compacted');
    expect(rows[2]?.activities?.[0]).toMatchObject({ label: 'Provider error', detail: 'Provider failed', tone: 'provider-error' });
    expect(rows[3]?.body).toBe('No findings.');
    await command('chatlocal:fold', 'r1');
    rows = transcriptPresentation(client);
    expect(rows[1]?.activities?.[0]).toMatchObject({ label: 'cargo test', body: 'cargo test', result: 'Process exited with code 0' });
  });
  test('checkpoints remain changed-file footers while settled trailing work joins its run', async () => {
    const { client, command, native } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', status: 'completed', startedAt: timestamp, completedAt: '2026-10-03T01:00:00.847Z' }];
    client.thread!.projection.checkpoints = [{ id: 'cp1', runId: 'r1', appRunOrdinal: 1, status: 'ready' },
      { id: 'cp2', runId: 'r2', appRunOrdinal: 2, status: 'ready' }];
    client.thread!.projection.visibleTurnItems = [
      { sourceThreadId: 't1', sourceItemId: 'tool', item: { id: 'tool', type: 'command_execution', input: 'test', runId: 'r1' } },
      { sourceThreadId: 't1', sourceItemId: 'answer', item: { id: 'answer', type: 'assistant_message', text: 'Done', runId: 'r1' } },
      { sourceThreadId: 't1', sourceItemId: 'trailing', item: { id: 'trailing', type: 'command_execution', output: 'trailing output', runId: 'r1' } },
      { sourceThreadId: 't1', sourceItemId: 'cp', item: { id: 'cp', type: 'checkpoint', checkpointId: 'cp1', runId: 'r1',
        files: [{ path: 'a.ts', additions: 3, deletions: 1 }] } },
    ];
    const data = snapshot(client);
    expect(data.messages.map(row => row.kind)).toEqual(['work', 'assistant', 'checkpoint']);
    expect(data.messages[0]).toMatchObject({ title: 'Worked for 847ms' });
    await command('chatlocal:fold', 'r1');
    expect(transcriptPresentation(client).map(row => row.kind)).toEqual(['work', 'entry', 'assistant', 'checkpoint', 'entry', 'meta']);
    await command('chatlocal:fold', 'r1');
    expect(data.messages[2]).toMatchObject({ checkpointOrdinal: 1, additions: 3, deletions: 1 });
    const contract = await Bun.file(new URL('./shapes.contract', import.meta.url)).text();
    const declaration = contract.split('shape Message\n')[1]!.split('\nshape ')[0]!;
    const required = [...declaration.matchAll(/^  (\w+):/gm)].map(match => match[1]).sort();
    for (const row of data.messages) expect(Object.keys(row).sort()).toEqual(required);
    for (const row of data.messages.filter(row => row.kind !== 'checkpoint')) {
      expect(row).toMatchObject({ checkpointId: '', checkpointOrdinal: -1, additions: 0, deletions: 0, files: [] });
    }
    expect(data.messages[2]!.checkpointId).toBe('cp1');
    await command('checkpoint-diff', '', '', 1);
    expect(native.calls.at(-1)?.method).toBe('orchestration.getTurnDiff');
    expect(obj(native.calls.at(-1)?.payload).toTurnCount).toBe(1);
    await command('checkpoint-diff', '', '', 99);
    expect(client.diffError).toContain('No completed checkpoint');
  });

  test('model browsing and search are read-only; choosing another provider commits one selection', async () => {
    const { client, native, command, disk: store } = await opened();
    client.config.providers = [...arr(client.config.providers), { instanceId: 'second', driver: 'codex', displayName: 'Other account', enabled: true,
      installed: true, auth: { status: 'authenticated' }, status: 'ready', models: [{ slug: 'other-model', name: 'Other Model' }] }];
    const initial = client.providerId;
    expect(modelCatalog(client, 'second', '').models.map(model => model.id)).toEqual(['other-model']);
    expect(client.providerId).toBe(initial);
    expect(modelCatalog(client, initial, 'other account model').models.map(model => model.id)).toEqual(['other-model']);
    await command('favorite-model', 'second', 'other-model');
    expect(modelCatalog(client, 'favorites', '').models.map(model => model.id)).toEqual(['other-model']);
    expect(store.data().favoriteModels).toEqual([JSON.stringify(['second', 'other-model'])]);
    const before = native.committed.length;
    await command('model', 'other-model', 'second');
    expect(native.committed.slice(before)).toEqual([expect.objectContaining({ type: 'thread.model-selection.set',
      modelSelection: { instanceId: 'second', model: 'other-model' } })]);
    expect(client.providerId).toBe('second');
    await command('favorite-model', 'second', 'other-model');
    expect(modelCatalog(client, 'favorites', '').models).toEqual([]);
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
    expect(await answer('settings', ['env1', 'deleted-project'], null, files, native)).toMatchObject({ available: false, permission: '', overridden: false, environmentPermission: '' });
    expect(await answer('settings', ['deleted-environment', 'p1'], null, files, native)).toMatchObject({ available: false, permission: '', overridden: false, environmentPermission: '' });
    expect(await answer('settings', ['env1', 'p1'], null, files, native)).toMatchObject({ available: true, permission: 'full-access' });
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
    expect(client.runtimeMode).toBe('full-access');
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
    expect(payload).toMatchObject({ runtimeMode: 'full-access', interactionMode: 'default', workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, initialMessage: { text: 'Build the feature', attachments: [] } });
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
    expect(client.pending).toBeUndefined(); expect(client.draft).toBe(''); expect(client.error).toBe('');
  });
  test('reconciles an accepted send whose acknowledgment was lost without sending again', async () => {
    const { client, native, disk, command } = await opened(); native.failure = 'after';
    await command('send', '', 'Accepted once');
    expect(client.pending?.uncertain).toBe(true); expect(client.error).toContain('may have reached T3');
    await client.refresh(native, disk.files);
    expect(client.pending).toBeUndefined(); expect(client.draft).toBe(''); expect(client.error).toBe('');
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
    const { client, native, command } = await connected();
    await command('add-project', '/Users/me/projects/new-app/', '');
    expect(native.committed[0]).toMatchObject({ type: 'project.create', title: 'new-app', workspaceRoot: '/Users/me/projects/new-app/', createWorkspaceRootIfMissing: false });
    expect(client.shell.projects.some(project => project.id === client.projectId && project.workspaceRoot === '/Users/me/projects/new-app/')).toBe(true);
    await client.refresh(native, storage().files);
    expect(client.shell.projects.find(project => project.id === client.projectId)?.title).toBe('new-app');
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
    const reasons = () => { const view = snapshot(client), status = approval.status; approval.status = 'resolved'; const later = snapshot(client).questions[0]; approval.status = status; return [view.approvals[0], later]; };
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
    expect(reasons().every(item => item.responseReason.startsWith('Provider process is gone'))).toBe(true);
    approval.responseCapability.type = 'message';
    expect(reasons()[0].responseReason).toContain('interrupt or restart the run');
  });
  test('diff uses checkpoint ordinal and exposes read-only unified lines', async () => {
    const { client, native, command } = await opened();
    client.thread!.projection.checkpoints = [{ id: 'cp', status: 'ready', appRunOrdinal: 7, runId: 'r7' }];
    await command('checkpoint-diff', '', '', 7);
    expect(native.calls.find(call => call.method === 'orchestration.getTurnDiff')).toMatchObject({ payload: { threadId: 't1', fromTurnCount: 6, toTurnCount: 7, ignoreWhitespace: true } });
    expect(snapshot(client).diffFiles).toMatchObject([{ id: 'a.ts', name: 'a.ts', additions: 1, deletions: 1 }]);
    expect(snapshot(client).diffItems.map(item => item.kind)).toEqual(['file']);
  });
  test('project filtering and draft partitioning keep different workspace input separate', async () => {
    const { client, native, disk, command } = await connected();
    client.shell.projects.push({ id: 'p2', title: 'Other', workspaceRoot: '/other' });
    client.shell.threads.push({ ...thread('other'), projectId: 'p2' });
    await command('draft', '', 'Project one');
    await command('select-project', 'p2'); await command('draft', '', 'Project two');
    // The sidebar lists every project's threads until "Filter threads by project" scopes it.
    expect(snapshot(client).threads.map(thread => thread.id)).toEqual(['other', 't1', 't2']);
    await command('sidebarlocal:scope', '', client.projectGroups().find(group => group.members.some(member => member.id === 'p2'))!.key);
    expect(snapshot(client).threads.map(thread => thread.id)).toEqual(['other']);
    await command('sidebarlocal:scope', '', '');
    await command('select-project', 'p1'); expect(client.draft).toBe('Project one');
    expect(JSON.stringify(disk.data())).not.toContain('access_token');
    expect(native.committed).toHaveLength(0);
  });
});

 test('automatic project identities use the reference palette and Unicode monograms', () => {
  expect(projectIdentity('Parity fixture')).toEqual({projectMark: 'PF', projectInk: 'light-dark(#f54900, #ff8904)', projectSurface: 'light-dark(#f5490024, #ff890424)'});
  expect(projectIdentity('  Ｔ３ Code  ')).toEqual(projectIdentity('T3 Code'));
  expect(projectIdentity('Exact')).toMatchObject({projectMark: 'ET'});
  expect(projectIdentity('')).toMatchObject({projectMark: 'PR'});
});

test('inherited settled work folds across its answer and keeps the fork marker separate', async () => {
  const client = new T3Client();
  client.threadId = 'fork';
  const item = (id: string, type: string, changes: Obj = {}) => ({id, type, runId:'source-run', status:'completed', updatedAt:'2026-10-03T06:13:24.063Z', ...changes});
  const items = [item('user','user_message',{text:'Question', updatedAt:'2026-10-03T06:13:23.151Z'}),
    item('setup','command_execution',{input:'Preparing workspace'}),
    item('tool','command_execution',{input:'read file',output:'actual output',exitCode:0}), item('answer','assistant_message',{text:'Answer'}),
    item('tail','command_execution',{input:'trailing output'}),
    item('fork-marker','fork',{runId:null, source:{type:'run',threadId:'source',runId:'source-run'},targetThreadId:'fork'})];
  client.thread = {projection:{thread:{id:'fork'}, runs:[], visibleTurnItems:items.map((item,position)=>({position,visibility:'inherited',sourceThreadId:'source',sourceItemId:item.id,item}))}, sequence:1,historyCursor:null,hasMore:false,latestLocalTurnOrdinal:null};
  const rows = transcriptPresentation(client);
  expect(rows.map(row=>row.kind)).toEqual(['user','work','assistant','fork']);
  expect(rows[1]).toMatchObject({title:'Worked for 912ms'});
  expect(rows[3]).toMatchObject({body:'Forked from conversation',sourceThreadId:'source'});
  await chatLocal(client, {} as Native, 'fold', 'source-run', '');
  const open = transcriptPresentation(client);
  expect(open.map(row=>row.kind)).toEqual(['user','work','entry','assistant','entry','meta','fork']);
  expect(open[2]!.activities?.[0]).toMatchObject({label:'read file',body:'read file',icon:'terminal',result:'Process exited with code 0',failed:false});
  expect(open[4]!.activities?.[0]).toMatchObject({label:'trailing output',result:''});
});

test('provider account badges distinguish configured siblings and valid accents', () => {
  const named = {instanceId:'custom',driver:'codex',displayName:'Exact verification fixture'};
  expect(providerBadge(named,[named])).toMatchObject({providerBadge:''});
  expect(providerBadge(named,[named,{instanceId:'codex',driver:'codex',status:'disabled'}])).toMatchObject({providerBadge:'EV'});
  expect(providerBadge({...named,displayName:'🦊Fox',accentColor:'#aAbB00'},[named])).toEqual({providerBadge:'🦊F',providerBadgeColor:'#aAbB00'});
  expect(providerBadge({...named,accentColor:'red'},[named])).toMatchObject({providerBadge:''});
  const acp={driver:'acpRegistry',displayName:'Local Agent',acpRegistryAgentId:'one'};
  expect(providerBadge(acp,[acp,{driver:'acpRegistry',acpRegistryAgentId:'two'}])).toMatchObject({providerBadge:''});
});


test('timeline presentation belongs to one connection/project/thread and follows the configured collapse setting', () => {
  const client = new T3Client();
  client.origin = 'http://localhost:1'; client.projectId = 'p'; client.threadId = 't';
  client.presentation = {owner:'http://localhost:1:p:t',resting:true,atEnd:false};
  expect(snapshot(client)).toMatchObject({composerResting:true,transcriptAway:true,composerCollapseOnScroll:true});
  client.threadId = 'other';
  expect(snapshot(client)).toMatchObject({composerResting:false,transcriptAway:false});
  client.local.deviceSettings.composerCollapseOnScroll = false;
  expect(snapshot(client).composerCollapseOnScroll).toBe(false);
});


describe('project management and scoped defaults', () => {
  test('rename validates title and updates canonical project without another entry', async () => {
    const { client, native, command } = await connected();
    expect((await command('rename-project', 'p1', '  ')).message).toBe('Project title cannot be empty');
    expect(client.error).toBe('');
    expect(native.committed).toHaveLength(0);
    expect((await command('rename-project', 'p1', '  Renamed fixture  ')).message).toBe('');
    expect(client.shell.projects).toHaveLength(1);
    expect(client.shell.projects[0]).toMatchObject({ id: 'p1', title: 'Renamed fixture', workspaceRoot: '/repo' });
    expect(native.committed.at(-1)).toMatchObject({ type: 'project.update', projectId: 'p1', title: 'Renamed fixture' });
  });
  test('remove clears owner/drafts, keeps unrelated project and does not issue filesystem operations', async () => {
    const { client, native, command } = await opened();
    native.shell.projects = [...arr(native.shell.projects), { id: 'p2', title: 'Other', workspaceRoot: '/other' }];
    client.local.drafts['env1:t1'] = 'removed draft'; client.local.drafts['env1:new:p1'] = 'removed new draft';
    client.local.drafts['env1:new:p2'] = 'keep';
    expect((await command('remove-project', 'p1')).message).toBe('');
    expect(client.projectId).toBe('p2'); expect(client.threadId).toBe('');
    expect(client.local.drafts['env1:t1']).toBeUndefined(); expect(client.local.drafts['env1:new:p1']).toBeUndefined();
    expect(client.local.drafts['env1:new:p2']).toBe('keep');
    expect(native.committed.at(-1)).toMatchObject({ type: 'project.delete', projectId: 'p1', force: true });
    expect(native.calls.filter(call => String(call.op).includes('deleteFile'))).toHaveLength(0);
  });
  test('scoped permissions preserve unrelated overrides; inheritance changes only new-thread default', async () => {
    const { client, native, command } = await connected();
    obj(native.config.settings).projectSettingsOverrides = { p1: { defaultRuntimeMode: 'full-access', worktreeSubmodules: 'recursive' } };
    expect((await command('setting-permissions', 'env1:p1', 'approval-required')).message).toBe('');
    expect(obj(obj(obj(client.config.settings).projectSettingsOverrides).p1)).toEqual({ defaultRuntimeMode: 'approval-required', worktreeSubmodules: 'recursive' });
    expect(client.runtimeMode).toBe('approval-required');
    expect((await command('setting-permissions', 'env1:p1', 'inherit')).message).toBe('');
    expect(obj(obj(obj(client.config.settings).projectSettingsOverrides).p1)).toEqual({ worktreeSubmodules: 'recursive' });
    expect(client.runtimeMode).toBe('full-access');
    await command('select-thread', 't1'); await client.refresh(native, storage().files);
    await command('setting-permissions', 'env1:', 'full-access');
    expect(client.runtimeMode).toBe('approval-required');
  });
});

test('stale settings project refuses write rather than expanding to environment', async () => {
  const { client, native, command } = await connected();
  const before = JSON.stringify(native.config.settings);
  expect((await command('setting-permissions', 'env1:removed-id', 'approval-required')).message).toBe('That project is no longer available.');
  expect(JSON.stringify(native.config.settings)).toBe(before);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('stale environment refuses scoped setting write', async () => {
  const { native, command } = await connected();
  expect((await command('setting-permissions', 'old-environment:p1', 'full-access')).message).toBe('That environment is no longer selected.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('repository group management resolves current members and leaves unrelated checkout untouched', async () => {
  const { client, native, command } = await connected();
  native.shell.projects = [
    { id: 'p1', title: 'One', workspaceRoot: '/repo/one', repositoryIdentity: { canonicalKey: 'repo-fixture', displayName: 'Fixture' } },
    { id: 'p2', title: 'Two', workspaceRoot: '/repo/two', repositoryIdentity: { canonicalKey: 'repo-fixture', displayName: 'Fixture' } },
    { id: 'p3', title: 'Other', workspaceRoot: '/other' } ];
  client.shell = { ...client.shell, projects: arr(native.shell.projects) };
  expect((await command('rename-group', 'repo-fixture', 'Together')).message).toBe('');
  expect(client.shell.projects.map(project => project.title)).toEqual(['Other', 'Together', 'Together']);
  expect((await command('remove-group', 'repo-fixture')).message).toBe('');
  expect(client.shell.projects.map(project => project.id)).toEqual(['p3']);
  expect((await command('rename-group', 'missing', 'No')).message).toBe('That project group is no longer available.');
});

test('provider management preserves configuration and targets one current instance', async () => {
  const { client, native, command } = await connected();
  const original = { driver: 'codex', displayName: 'Fixture', enabled: true, accentColor: '#123456', environment: { FIXTURE: 'isolated' }, config: { cliPath: '/fixture/codex', codexHome: '/fixture/home' } };
  obj(native.config.settings).providerInstances = { fixture: original, sibling: { driver: 'claude', enabled: false } };
  expect((await command('provider-name', 'fixture', '  ')).message).toBe('Provider name cannot be empty.');
  expect(client.error).toBe('');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await command('provider-name', 'fixture', '  Renamed  ')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).fixture).toEqual({ ...original, displayName: 'Renamed' });
  expect((await command('provider-enabled', 'fixture', 'false')).message).toBe('');
  expect(client.configuredProviders().find(provider => provider.id === 'fixture')?.enabled).toBe(false);
  expect((await command('provider-enabled', 'fixture', 'true')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).fixture).toEqual({ ...original, displayName: 'Renamed' });
  expect((await command('provider-remove', 'fixture')).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances)).toEqual({ sibling: { driver: 'claude', enabled: false } });
  const writes = native.calls.filter(call => call.method === 'server.updateSettings').length;
  expect((await command('provider-name', 'fixture', 'Stale')).message).toBe('That provider instance is no longer available.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(writes);
});

test('provider creation validates advertised driver/identity and uses atomic create without replacing siblings', async () => {
  const { client, native, command } = await connected();
  obj(native.config.settings).providerInstances = { keep: { driver: 'codex', displayName: 'Keep', config: { untouched: true } } };
  const draft = JSON.stringify({ driver: 'codex', name: 'Disposable "quoted" fixture', binaryPath: '/fixture/codex', homePath: '/fixture/home' });
  expect((await command('provider-create', '123-invalid', draft)).message).toContain('starting with a letter');
  expect((await command('provider-create', 'keep', draft)).message).toBe('That provider instance ID already exists.');
  expect((await command('provider-create', 'new', JSON.stringify({ driver: 'unadvertised', name: 'No' }))).message).toContain('advertised');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await command('provider-create', 'disposable', draft)).message).toBe('');
  expect(obj(obj(client.config.settings).providerInstances).keep).toEqual({ driver: 'codex', displayName: 'Keep', config: { untouched: true } });
  expect(obj(obj(client.config.settings).providerInstances).disposable).toEqual({ driver: 'codex', displayName: 'Disposable "quoted" fixture', enabled: true, config: { setupMode: 'existing', binaryPath: '/fixture/codex', homePath: '/fixture/home' } });
  expect(obj(obj(native.calls.find(call => obj(obj(call.payload).providerInstanceMutation).instanceId === 'disposable')).payload).providerInstanceMutation).toMatchObject({ operation: 'create' });
  expect((await command('provider-remove', 'disposable')).message).toBe('');
  expect(client.configuredProviders().map(provider => provider.id)).toEqual(['keep']);
});

test('repository_path and physical overrides persist; guarded member removal retains sibling', async () => {
  const { client, native, command, disk } = await connected();
  native.shell.projects = [{ id: 'p1', title: 'One', workspaceRoot: '/repo/one/', repositoryIdentity: { canonicalKey: 'repo', rootPath: '/repo' } }, { id: 'p2', title: 'Two', workspaceRoot: '/repo/two', repositoryIdentity: { canonicalKey: 'repo', rootPath: '/repo' } }];
  client.shell = { ...client.shell, projects: arr(native.shell.projects) };
  await command('grouping-mode', '', 'repository_path');
  expect(client.projectGroups().map(group => group.key)).toEqual(['repo::one', 'repo::two']);
  await command('grouping-override', 'p1', 'repository');
  expect(client.projectGroups().map(group => group.key)).toEqual(['repo', 'repo::two']);
  await command('grouping-override', 'p1', 'inherit');
  expect(client.local.groupingOverrides).toEqual({});
  await command('grouping-mode', '', 'repository');
  expect((await command('remove-group-member', 'p1', 'stale-group')).message).toContain('membership changed');
  expect(native.committed).toHaveLength(0);
  expect((await command('remove-group-member', 'p1', 'repo')).message).toBe('');
  expect(client.shell.projects.map(project => project.id)).toEqual(['p2']);
  expect((await command('grouping-override', 'p1', 'separate')).message).toBe('That project is no longer available.');
  await command('grouping-mode', '', 'repository_path');
  await command('grouping-override', 'p2', 'separate');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.groupingMode).toBe('repository_path');
  expect(restored.local.groupingOverrides).toEqual({ 'env1:/repo/two': 'separate' });
});

test('device settings persist locally, affect projection, and do not write an unavailable server scope', async () => {
  const { client, native, command, disk } = await connected();
  const before = JSON.stringify(native.config.settings);
  await command('device-setting', 'composerCollapseOnScroll', 'false');
  await command('device-setting', 'planModeEnabled', 'true');
  await command('device-setting', 'timestampFormat', '24-hour');
  expect(snapshot(client)).toMatchObject({ composerCollapseOnScroll: false, planModeEnabled: true });
  expect(JSON.stringify(native.config.settings)).toBe(before);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings).toEqual({ composerCollapseOnScroll: false, planModeEnabled: true, timestampFormat: '24-hour', appearanceMode: 'system', sendShortcut: 'enter', snapShotShortcut: 'shift+shift', snapShotEnabled: false, snapShotIncludeAccessibility: true, snapShotPlaySound: true, snapShotSound: 'soft-pop', snapShotFlash: true, snapShotAnimations: true });
  expect((await command('device-setting', 'timestampFormat', 'invalid')).message).toBe('Unsupported device setting.');
  expect(client.local.deviceSettings.timestampFormat).toBe('24-hour');
});

 test('appearance and send shortcut validate and persist without server changes', async () => {
  const { client, native, command, disk } = await connected();
  for (const value of ['light', 'dark', 'system']) expect((await command('device-setting', 'appearanceMode', value)).message).toBe('');
  await command('device-setting', 'appearanceMode', 'dark');
  await command('device-setting', 'sendShortcut', 'mod-enter-multiline');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings.appearanceMode).toBe('dark');
  expect(restored.local.deviceSettings.sendShortcut).toBe('mod-enter-multiline');
  expect(native.calls.filter(call => call.op === 'devicePresentation').slice(-1)[0]).toMatchObject({ appearanceMode: 'dark', sendShortcut: 'mod-enter-multiline' });
  expect((await command('device-setting', 'appearanceMode', 'bad')).message).toBe('Unsupported device setting.');
  expect((await command('device-setting', 'sendShortcut', 'bad')).message).toBe('Unsupported device setting.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
});

test('local appearance and send shortcut reset to reference defaults and survive reload', async () => {
  const { client, native, command, disk } = await connected();
  await command('device-setting', 'appearanceMode', 'light');
  await command('device-setting', 'sendShortcut', 'mod-enter');
  await command('device-setting', 'timestampFormat', '12-hour');
  await command('device-setting', 'appearanceMode', 'system');
  await command('device-setting', 'sendShortcut', 'enter');
  await command('device-setting', 'timestampFormat', 'locale');
  const restored = new T3Client(); await restored.refresh(native, disk.files);
  expect(restored.local.deviceSettings).toEqual(client.local.deviceSettings);
  expect(restored.local.deviceSettings).toMatchObject({ appearanceMode: 'system', sendShortcut: 'enter', timestampFormat: 'locale' });
});

test('scoped default model preserves options and other fields; unavailable scopes do not broaden writes', async () => {
  const { client, native, command } = await connected();
  client.providerId = 'codex-personal'; client.modelId = 'model-b'; client.modelOptions = [{ id: 'reasoningEffort', value: 'high' }];
  native.config.settings = { ...obj(native.config.settings), projectSettingsOverrides: { p1: { defaultRuntimeMode: 'approval-required' } } };
  expect((await command('setting-model', 'env1:p1', 'current')).message).toBe('');
  expect(obj(obj(native.config.settings).projectSettingsOverrides).p1).toEqual({ defaultRuntimeMode: 'approval-required', defaultModelSelection: { instanceId: 'codex-personal', model: 'model-b', options: [{ id: 'reasoningEffort', value: 'high' }] } });
  await command('new-thread', 'p1');
  expect(client.modelId).toBe('model-b');
  expect(client.runtimeMode).toBe('approval-required');
  expect((await command('setting-model', 'env1:p1', 'inherit')).message).toBe('');
  await command('new-thread', 'p1'); expect(client.modelId).toBe('model-a');
  const count = native.calls.filter(call => call.method === 'server.updateSettings').length;
  native.shell.projects = [];
  expect((await command('setting-model', 'env1:p1', 'current')).message).toBe('That project is no longer available.');
  expect((await command('setting-model', 'other:', 'current')).message).toBe('That environment is no longer selected.');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(count);
});

test('Automatic model chooses ready instance default and scoped reset uses null', async () => {
  const { client, native, command } = await connected();
  native.config.settings = { defaultModelSelection: null };
  native.config.providers = [{ instanceId: 'disabled', enabled: false, installed: true, models: [{ slug: 'bad' }] }, ...arr(native.config.providers)];
  client.config = native.config;
  await command('new-thread', 'p1');
  expect(client.providerId).toBe('codex-personal'); expect(client.modelId).toBe('model-a');
  client.modelId = 'model-b';
  await command('setting-model', 'env1:', 'current');
  await command('setting-model', 'env1:', 'automatic');
  expect(obj(native.config.settings).defaultModelSelection).toBeNull();
  await command('new-thread', 'p1'); expect(client.modelId).toBe('model-a');
});

test('archived restore/delete target exact current archived member; reject stale scope without global writes', async () => {
  const { client, native, command } = await connected();
  native.shell.archivedThreads = [{ ...thread('a1'), createdAt: timestamp, archivedAt: timestamp }, { ...thread('a2'), createdAt: timestamp, archivedAt: timestamp }];
  const listed = await archivedSettings(client, native, 'env1', 'p1', true);
  expect(listed.groups).toHaveLength(1);
  expect((await command('unarchive-thread', 'env1:p1:a1')).message).toBe('');
  expect(client.shell.threads.map(thread => thread.id)).toContain('a1');
  expect(arr(native.shell.archivedThreads).map(thread => thread.id)).toEqual(['a2']);
  const count = native.committed.length;
  // Settings failures toast (shell-commands.ts settingsFailure); `toasted:` keeps the page's inline error empty.
  expect((await command('delete-archived-thread', 'env1:p1:a1')).message).toBe('toasted:That archived thread is no longer available.');
  expect((await command('unarchive-thread', 'other:p1:a2')).message).toContain('no longer selected');
  expect(native.committed).toHaveLength(count);
  expect((await command('delete-archived-thread', 'env1:p1:a2')).message).toBe('');
  expect(arr(native.shell.archivedThreads)).toEqual([]);
  const stale = await archivedSettings(client, native, 'env1', 'deleted', true);
  expect(stale.available).toBe(false); expect(stale.groups).toEqual([]);
});

test('license manifest validates provenance shape, all-term search, disclosure and retry', async () => {
  const { client, native } = await connected();
  const entry = { name: 'React', kind: 'package', version: '19.2', license: 'MIT', bundles: ['web'], sourceUrl: 'https://react.dev/', noticeText: 'Actual notice' };
  const manifest = { schemaVersion: 1, entries: [entry, { ...entry, name: 'Other', license: 'ISC' }] };
  const later = native.later.bind(native); let reads = 0;
  native.later = async request => { if (obj(request).path === '/third-party-licenses.json') { reads++; return native.good(manifest); } return later(request); };
  const filtered = await licenseSettings(client, native, 'react mit', '', 0, true);
  expect(filtered.total).toBe(2); expect(filtered.matched).toBe(1);
  expect(filtered.entries[0].noticeText).toBe('');
  const expanded = await licenseSettings(client, native, '', str(filtered.entries[0].id), 0, true);
  expect(expanded.entries[0].noticeText).toBe('Actual notice'); expect(reads).toBe(1);
  expect((await licenseSettings(client, native, 'missing', '', 0, true)).matched).toBe(0);
  await licenseSettings(client, native, '', '', 1, true); expect(reads).toBe(2);
  expect(() => decodeNotices({ schemaVersion: 2, entries: [] })).toThrow('unsupported format');
  expect(() => decodeNotices({ schemaVersion: 1, entries: [entry, entry] })).toThrow('duplicate');
  expect(() => decodeNotices({ schemaVersion: 1, entries: [{ ...entry, sourceUrl: 'file:///personal' }] })).toThrow('source URL');
});

test('diagnostics reads actual sections and range; partial/offline errors remain local', async () => {
  const { client, native } = await connected();
  const later = native.later.bind(native);
  native.later = async request => {
    const method = str(obj(request).method);
    if (method === 'server.getProcessDiagnostics') return native.good({ readAt: timestamp, processCount: 1, totalCpuPercent: 2, totalRssBytes: 1024, processes: [] });
    if (method === 'server.getProcessResourceHistory') { expect(obj(obj(request).payload)).toEqual({ windowMs: 300000, bucketMs: 15000 }); return native.good({ retainedSampleCount: 3, buckets: [], topProcesses: [] }); }
    if (method === 'server.getTraceDiagnostics') return native.bad('Trace unavailable');
    return later(request);
  };
  const result = await diagnosticsSettings(client, native, 'env1', '5m', true);
  expect(result.available).toBe(true); expect(result.sections).toHaveLength(3);
  expect(result.sections[0].rows[0].value).toBe('1'); expect(result.sections[2].error).toBe('Trace unavailable');
  expect(client.error).toBe('');
  expect((await diagnosticsSettings(client, native, 'env1', 'invalid', true)).error).toContain('Unsupported');
  expect((await diagnosticsSettings(client, native, 'deleted', '15m', true)).available).toBe(false);
  client.connection = 'disconnected';
  expect((await diagnosticsSettings(client, native, 'env1', '15m', true)).available).toBe(false);
});


test('storage cleanup validates capability, retention and stale checkout before scoped writes', async () => {
  const { client, native, command } = await connected();
  obj(native.config.environment).capabilities = { serverResolvedCommandContext: true, storageCleanup: true, projectWorktreeCleanup: true };
  native.config.settings = { storageCleanup: { logsAfterDays: null, worktreeOnMerge: false }, projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' }, sibling: { defaultRuntimeMode: 'approval-required' } } };
  const change = (scope: string, key: string, value: unknown) => command('setting-storage', scope, JSON.stringify({ key, value }));
  expect((await change('env1:', 'logsAfterDays', 3651)).message).toContain('3650');
  expect((await change('env1:deleted', 'mode', 'custom')).message).toContain('no longer available');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toHaveLength(0);
  expect((await change('env1:p1', 'mode', 'custom')).message).toBe('');
  expect((await change('env1:p1', 'worktreeAfterDays', 12)).message).toBe('');
  const overrides = obj(obj(native.config.settings).projectSettingsOverrides);
  expect(obj(overrides.p1).defaultRuntimeMode).toBe('full-access');
  expect(obj(overrides.sibling).defaultRuntimeMode).toBe('approval-required');
  expect(obj(obj(overrides.p1).worktreeCleanup)).toEqual({ mode: 'custom', rules: { worktreeAfterDays: 12, worktreeOnDelete: false, worktreeOnMerge: false, worktreeUnchanged: false } });
  expect((await storageSettings(client, native, 'env1', 'p1', true)).worktrees.find(rule => rule.key === 'worktreeAfterDays')?.days).toBe('12');
  expect((await change('env1:p1', 'mode', 'inherit')).message).toBe('');
  expect(obj(obj(obj(native.config.settings).projectSettingsOverrides).p1).worktreeCleanup).toBeUndefined();
  expect((await change('env1:p1', 'logsAfterDays', 8)).message).toContain('environment');
  expect((await change('env1:', 'logsAfterDays', 8)).message).toBe('');
  expect(obj(obj(native.config.settings).storageCleanup).worktreeOnMerge).toBe(false);
  expect((await change('env1:', 'logsAfterDays', null)).message).toBe('');
  obj(obj(native.config.environment).capabilities).storageCleanup = false;
  expect((await change('env1:', 'worktreeOnDelete', true)).message).toContain('Update');
});


test('scoped source control writes preserve unrelated settings and reset the exact project tier', () => {
  const settings = { sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: 'keep', followChangeRequestTemplates: true }, projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' }, p2: { defaultAutoPull: true } } };
  expect(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.mode', 'custom', false)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', sourceControlWritingStyle: { mode: 'custom', customInstructions: 'keep', followChangeRequestTemplates: true } } } });
  expect(scopedSettingPatch(settings, 'p1', 'defaultAutoPull', true, false)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access', defaultAutoPull: true } } });
  expect(scopedSettingPatch(settings, 'p1', 'defaultAutoPull', null, true)).toEqual({ projectSettingsOverrides: { p1: { defaultRuntimeMode: 'full-access' } } });
  expect(() => scopedSettingPatch(settings, '', 'defaultAutoPull', null, true)).toThrow('project');
  expect(() => scopedSettingPatch(settings, 'p1', 'pullRequestMergeMethod', 'invalid', false)).toThrow('supported');
  expect(() => scopedSettingPatch(settings, '', 'observability', {}, false)).toThrow('Unsupported');
});
test('scoped controls reject unavailable identities and gate device access by the actual hub', async () => {
  const { client, native } = await connected();
  expect((await scopedControls(client, native, 'deleted-environment', '', 'integrations', true)).available).toBe(false);
  expect((await scopedControls(client, native, client.environmentId, 'deleted-project', 'source-control', true)).rows).toEqual([]);
  const state = await scopedControls(client, native, client.environmentId, '', 'integrations', true);
  expect(state.rows.find(row => row.key === 'enableAgentDeviceAccess')?.disabled).toBe(true);
  const response = await client.command('setting-scoped', client.environmentId + ':deleted-project', JSON.stringify({ key: 'defaultAutoPull', value: true }), 0, native, storage().files);
  expect(response.message).toContain('no longer available');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('writing group reset is explicit; text matches reference trim and preserves sibling fields', () => {
  const settings = { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'before', followChangeRequestTemplates: false }, projectSettingsOverrides: { p1: { sourceControlWritingStyle: { mode: 'custom', customInstructions: 'own', followChangeRequestTemplates: true }, defaultAutoPull: true } } };
  expect(() => scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.customInstructions', null, true)).toThrow('group reset');
  expect(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle', null, true)).toEqual({ projectSettingsOverrides: { p1: { defaultAutoPull: true } } });
  expect(obj(obj(obj(scopedSettingPatch(settings, 'p1', 'sourceControlWritingStyle.customInstructions', '  first\n second  ', false).projectSettingsOverrides).p1).sourceControlWritingStyle)).toEqual({ mode: 'custom', customInstructions: 'first\n second', followChangeRequestTemplates: true });
});
test('device access action cannot bypass disabled hub or stale project capabilities', async () => {
  const { client, native } = await connected();
  native.config.settings = { enableDeviceSupport: false, enableAgentDeviceAccess: false, defaultAutoPull: false };
  const command = (scope: string, key: string) => client.command('setting-scoped', scope, JSON.stringify({ key, value: true }), 0, native, storage().files);
  expect((await command('env1:', 'enableAgentDeviceAccess')).message).toContain('device hub');
  expect((await command('env1:p1', 'defaultAutoPull')).message).toContain('project overrides');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('scheduled defaults and edit conversion match reference daily and origin semantics', () => {
  const fresh = taskDraft(); expect(fresh.runtimeMode).toBe('full-access'); expect([fresh.monday, fresh.friday, fresh.sunday]).toEqual([true, true, false]);
  const existing = taskDraft({ id: 's1', schedule: { type: 'fixed_time', timeOfDay: '10:00' }, workspaceStrategy: { type: 'worktree', baseRef: 'release' } });
  expect([existing.sunday, existing.saturday, existing.startFromOrigin, existing.baseRef]).toEqual([true, true, false, 'release']);
  const args = ['env:', '', 'Fixture', 'Safe fixture prompt', false, 'fixed_time', '15', '', false, false, false, false, false, false, false, 'p1', '', 'worktree', '', true, '', 'fixture', 'luna', 'approval-required', 'default'];
  const payload = taskFromArguments(args);
  expect(payload.schedule).toEqual({ type: 'fixed_time', timeOfDay: '09:00' });
  expect(payload.workspaceStrategy).toEqual({ type: 'worktree', baseRef: 'main', startFromOrigin: true });
  expect(validateTaskInput(payload).enabled).toBe(false);
  expect(() => validateTaskInput({ ...payload, schedule: { type: 'fixed_time', timeOfDay: '25:61' } })).toThrow('valid time');
  expect(() => validateTaskInput({ ...payload, schedule: { type: 'interval', everyMs: 59999 } })).toThrow('one minute');
});

test('scheduled edit rechecks membership, forces requireExisting and preserves provider options', async () => {
  const { client, native } = await connected();
  let entries: Obj[] = [{ id: 's1', projectId: 'p1', modelSelection: { instanceId: 'codex-personal', model: 'model-a', options: [{ id: 'effort', value: 'high' }] } }];
  const writes: Obj[] = [], later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.method === 'scheduledTasks.list') return native.good({ tasks: entries });
    if (call.method === 'scheduledTasks.upsert') { writes.push(obj(call.payload)); return native.good({ task: obj(call.payload) }); }
    return later(request);
  };
  const input = { id: 's1', title: 'Fixture', prompt: 'Safe disabled fixture', enabled: false, schedule: { type: 'interval', everyMs: 60000 }, projectId: 'p1', threadId: null, workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex-personal', model: 'model-a' }, runtimeMode: 'approval-required', interactionMode: 'default' };
  expect((await client.command('task-save', 'env1:p1', JSON.stringify(input), 0, native, storage().files)).message).toBe('');
  expect(writes[0].requireExisting).toBe(true); expect(obj(writes[0].modelSelection).options).toEqual([{ id: 'effort', value: 'high' }]); expect(writes[0].enabled).toBe(false);
  entries = [];
  expect((await client.command('task-save', 'env1:p1', JSON.stringify(input), 0, native, storage().files)).message).toContain('no longer exists');
  expect(writes.length).toBe(1);
  expect((await client.command('task-delete', 'env1:deleted-project', JSON.stringify({ id: 's1' }), 0, native, storage().files)).message).toContain('no longer available');
});

test('scheduled list query reaches the allowlisted backend and preserves readonly/offline scope state', async () => {
  const { client, native } = await connected();
  const later = native.later.bind(native);
  native.later = async request => obj(request).method === 'scheduledTasks.list' ? native.good({ tasks: [] }) : later(request);
  expect((await scheduledSettings(client, native, 'env1', '', '', true)).available).toBe(true);
  expect((await scheduledSettings(client, native, 'env1', '', 'missing-task', true)).error).toContain('no longer exists');
  expect((await scheduledSettings(client, native, 'env1', 'deleted-project', '', true)).available).toBe(false);
});


test('keybinding precedence is global across commands and physical mod/meta aliases', () => {
  const rules = [
    { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } },
    { command: 'chat.new', shortcut: { key: 'b', metaKey: true } },
    { command: 'chat.new', shortcut: { key: 'n', modKey: true } },
  ];
  const keys = keyboardSettings({ keybindings: rules }, false);
  expect(keys.keySidebar).toBe(''); expect(keys.keyNewThread).toBe('Meta+n Meta+b');
  expect(keyboardSettings({ keybindings: rules }, false, { modalOpen: true }).keyNewThread).toBe('');
});

test('unknown negated shortcut context cannot activate or revive an older binding', () => {
  const earlier = { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } };
  const later = { command: 'chat.new', shortcut: { key: 'b', modKey: true }, whenAst: { type: 'not', node: { type: 'identifier', name: 'unsupportedContext' } } };
  const keys = keyboardSettings({ keybindings: [earlier, later] }, false);
  expect(keys.keySidebar).toBe(''); expect(keys.keyNewThread).toBe('');
  const editable = { ...later, whenAst: { type: 'not', node: { type: 'identifier', name: 'editableFocus' } } };
  expect(keyboardSettings({ keybindings: [earlier, editable] }, true).keySidebar).toBe('Meta+b');
  expect(keyboardSettings({ keybindings: [earlier, editable] }, false).keyNewThread).toBe('Meta+b');
});


test('scoped search follows the mounted conditional fields without changing settings', () => {
  const settings = { branchNamingMode: 'static', sourceControlWritingStyle: { mode: 'repo_conventions' }, projectSettingsOverrides: { p1: { branchNamingMode: 'custom' } } };
  const scope = scopedSearchControls(settings, 'p1', { projectSettingsOverrides: true });
  expect(scope.some(row => row[1] === 'scoped-input-branchNameInstructions')).toBe(true);
  expect(scope.some(row => row[1] === 'scoped-input-branchNamePrefix')).toBe(false);
  expect(scope.some(row => row[1] === 'scoped-input-sourceControlWritingStyle.customInstructions')).toBe(false);
  expect(scopedSearchControls(settings, 'p1', {}).length).toBe(0);
  expect(settings.branchNamingMode).toBe('static');
});


test('provider catalog includes first instances and disabled or setup-needed drivers', async () => {
  const { client } = await connected();
  client.config.providers = [{ driver: 'codex', availability: 'unavailable', enabled: false, setup: { canAuthenticate: true } }];
  const drivers = client.providerDrivers();
  expect(drivers.map(driver => driver.id)).toContain('acpRegistry');
  expect(drivers.map(driver => driver.id)).toContain('claudeAgent');
  expect(drivers.find(driver => driver.id === 'codex')?.canAuthenticate).toBe(true);
});


test('keybinding grammar rejects invalid expressions before server writes', async () => {
  expect(validShortcut('mod+alt+shift+y')).toBe(true); expect(validShortcut('mod++')).toBe(true);
  expect(validShortcut('mod+x+y')).toBe(false); expect(validShortcut('mod')).toBe(false);
  expect(validWhen('!terminalFocus && (composerFocus || draftThreadRoute)')).toBe(true);
  expect(validWhen(')')).toBe(false); expect(validWhen('!unsupportedContext')).toBe(true);
  const { client, native, disk } = await connected();
  const result = await client.command('keybinding-save', 'env1', JSON.stringify({ command: 'sidebar.toggle', key: 'mod+y', when: ')' }), 0, native, disk.files);
  expect(result.message).toContain('valid shortcut condition');
  expect(native.calls.filter(call => call.method === 'server.upsertKeybinding')).toEqual([]);
});


test('denied snapshot setup leaves device preferences and backend unchanged', async () => {
  const { client, native, disk } = await connected();
  const later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return call.enabled ? native.bad('Screen Recording access is required.') : native.good({ enabled: false });
    if (call.op === 'snapshotState') return native.good({ mode: 'direct', enabled: false, screenRecording: false, accessibility: false, pending: [] });
    return later(request);
  };
  expect((await client.snapshotState(native)).screenRecording).toBe(false);
  const result = await client.command('setting-snapshot', 'snapShotEnabled', 'true', 0, native, disk.files);
  expect(result.message).toContain('Screen Recording'); expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect(client.error).toBe('');
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

test('snapshot state cannot be adopted into a different draft owner', async () => {
  const { client, native } = await connected(); const later = native.later.bind(native);
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return native.good({ enabled: false });
    if (call.op === 'snapshotState') { client.projectId = 'changed-owner'; return native.good({ pending: ['capture-one'] }); }
    return later(request);
  };
  await expect(client.snapshotState(native)).rejects.toThrow('capture draft changed');
});


test('revoked snapshot grants preserve enabled choice and reachable disable without backend writes', async () => {
  const { client, native, disk } = await connected(); const later = native.later.bind(native);
  client.local.deviceSettings.snapShotEnabled = true;
  native.later = async request => {
    const call = obj(request);
    if (call.op === 'snapshotConfigure') return call.enabled ? native.bad('Screen Recording access was revoked.') : native.good({ enabled: false });
    if (call.op === 'snapshotState') return native.good({ enabled: false, screenRecording: false, accessibility: false, pending: [], capturing: [] });
    return later(request);
  };
  const { snapshotSettings } = await import('./snapshot-settings');
  const state = await snapshotSettings(client, native, true);
  expect(state).toMatchObject({ available: true, enabled: true, screenRecording: false, accessibility: false });
  expect(state.error).toContain('revoked');
  expect(client.local.deviceSettings.snapShotEnabled).toBe(true);
  const disabled = await client.command('setting-snapshot', 'snapShotEnabled', 'false', 0, native, disk.files);
  expect(disabled.message).not.toContain('revoked');
  expect(client.local.deviceSettings.snapShotEnabled).toBe(false);
  expect(native.calls.filter(call => call.method === 'server.updateSettings')).toEqual([]);
});

describe('r13-store: Remove forgets the focused environment', () => {
  test('forget clears the address, selection and cached threads of the focused environment', async () => {
    const context = await opened();
    expect(context.client.threadId).toBe('t1');
    expect(context.client.shell.threads.length).toBe(2);
    const native = context.native, original = native.later.bind(native);
    native.later = async (input: unknown) => {
      const request = obj(input);
      if (request.op === 'disconnect' && request.forget) { native.calls.push(request); native.generation++; return native.good({ state: 'disconnected', origin: '', environmentId: '', message: '' }); }
      return original(input);
    };
    await context.command('forget');
    expect(context.client.origin).toBe('http://127.0.0.1:3773');
    expect(context.client.environmentId).toBe('');
    expect(context.client.threadId).toBe('');
    expect(context.client.shell.threads.length).toBe(0);
  });
  test('a plain disconnect keeps the focused environment', async () => {
    const context = await opened();
    await context.command('disconnect');
    expect(context.client.threadId).toBe('t1');
    expect(context.client.shell.threads.length).toBe(2);
  });
});
