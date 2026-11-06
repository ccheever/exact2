import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { answer } from './app';
import { mobileClient, mobileConnectionSources } from './client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileHomeActionsObserve } from './home-actions';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { arr, obj, type Obj } from './shared/domain';
import { nativeFiles, type Native, type Files } from './shared/protocol';
import { fleet } from './shared/settings-b-fleet';
import { reconnectOnLaunch } from './shared/r8-pointer-reconnect';

test('an unidentified saved retry presents its real failure through the complete Home source chain', async () => {
  if (process.env.T3_RETRY_HOME_TEST_PROCESS !== '1') {
    const child = spawnSync(process.execPath, ['test', import.meta.path, '--test-name-pattern',
      '^an unidentified saved retry presents its real failure through the complete Home source chain$'],
    { env: { ...process.env, T3_RETRY_HOME_TEST_PROCESS: '1' }, encoding: 'utf8', timeout: 15_000 });
    expect({ error: child.error?.message ?? '', status: child.status, signal: child.signal,
      output: child.status === 0 ? '' : child.stdout + child.stderr }).toEqual({ error: '', status: 0, signal: null, output: '' });
    expect(child.stderr).toContain('1 pass');
    return;
  }
  const priorClient = { ...mobileClient }, priorFleet = { ...fleet }, calls: Obj[] = [];
  const saved = [{ environmentId: 'retry-environment', origin: 'https://saved.test', mobileLabel: 'Retry Mac', enabled: true }];
  const status = { state: 'reconnecting', origin: 'https://route.test', homeOrigin: 'https://saved.test', environmentId: '',
    message: 'Could not connect to the server. Reconnecting in 6s…', failureKind: 'Network', traceId: '' };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return ''; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: 7, value: request.op === 'status' ? status : request.op === 'environments' ? { saved }
      : request.op === 'readPreferences' || request.op === 'connectionPreferences' ? { text: '{"version":1}' } : {} };
  } };
  try {
    Object.assign(mobileClient, new MobileDraftClient(), { generation: 7, origin: saved[0]!.origin });
    Object.assign(fleet, { saved, entries: new Map() });
    await reconnectOnLaunch(mobileClient, native, { state: 'disconnected', origin: saved[0]!.origin, environmentId: '' });
    mobileClient.adoptStatus(status, 7);
    await answer('snapshotRead', [], {} as never, storage, native);
    const snapshot = obj(answer('snapshot', ['light', 't3-code', 0, 1791590400000], {} as never, storage, native));
    expect(arr(snapshot.environments)[0]).toMatchObject({ environmentId: 'retry-environment', state: 'reconnecting', hasError: true, retrying: true });
    expect(arr(snapshot.environments)[0]!.status).toContain('Could not connect to the server.');
    const chrome = obj(answer('homeChrome', ['home-route', '', '', '', 'light', 34, true, snapshot.revision,
      snapshot.environments, 'repository', 'compact'], {} as never, storage, native));
    expect(obj(JSON.parse(String(chrome.configuration))).status).toEqual({ label: 'Reconnecting to Retry Mac', progress: false });
    const home = obj(answer('homeView', [snapshot.revision, 1791590400000, '', 10, true, false, false, false, false,
      '', '', '', 'repository', 'home-route', true, false, snapshot.environments, 'compact', true], {} as never, storage, native));
    expect(home.loading).toBe(false); expect(home.emptyTitle).not.toBe('Connecting to environment');
    expect(mobileClient.environmentId).toBe(''); expect(mobileClient.ready).toBe(false);
    expect(calls.some(call => call.op === 'connect' && typeof call.fleet === 'string')).toBe(false);
    status.homeOrigin = 'https://foreign.test';
    await answer('snapshotRead', [], {} as never, storage, native);
    const foreign = obj(answer('snapshot', ['light', 't3-code', 0, 1791590400000], {} as never, storage, native));
    expect(arr(foreign.environments)[0]).toMatchObject({ state: 'available', hasError: false });
    status.homeOrigin = saved[0]!.origin;
    saved.push({ ...saved[0]!, environmentId: 'ambiguous' });
    await answer('snapshotRead', [], {} as never, storage, native);
    const ambiguous = obj(answer('snapshot', ['light', 't3-code', 0, 1791590400000], {} as never, storage, native));
    expect(arr(ambiguous.environments)).toHaveLength(2);
    expect(arr(ambiguous.environments).find(row => row.environmentId === 'retry-environment'))
      .toMatchObject({ state: 'available', hasError: false });
    expect(mobileConnectionSources().every(row => !row.focused)).toBe(true);
    saved.pop();
    await answer('snapshotRead', [], {} as never, storage, native);
    Object.assign(mobileClient, { connection: 'connected', environmentId: 'retry-environment', generation: 8,
      configLive: true, shellLive: true, shellLoaded: true, error: '' });
    const recovered = obj(answer('snapshot', ['light', 't3-code', 0, 1791590400000], {} as never, storage, native));
    expect(arr(recovered.environments)[0]).toMatchObject({ state: 'connected', hasError: false, retrying: false });
    const recoveredChrome = obj(answer('homeChrome', ['home-route', '', '', '', 'light', 34, true, recovered.revision,
      recovered.environments, 'repository', 'compact'], {} as never, storage, native));
    expect(obj(JSON.parse(String(recoveredChrome.configuration))).status).toEqual({ label: '', progress: false });
  } finally { Object.assign(mobileClient, priorClient); Object.assign(fleet, priorFleet); }
});

test('actual bare draft source waits for grouping preferences and uses the supplied mode', async () => {
  const priorClient = { ...mobileClient }, priorFleet = { ...fleet }, calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: mobileClient.generation, value: request.op === 'environments'
      ? { saved: [{ environmentId: 'bare-root', enabled: true }] }
      : request.op === 'readPreferences' ? { text: '{"version":1}' } : {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('No portable storage'); }, async atomicWriteFile() {} } };
  try {
    Object.assign(mobileClient, new MobileDraftClient()); Object.assign(fleet, { saved: [], entries: new Map(), revision: 0 });
    await mobileClient.command('dismiss-error', '', '', 0, native, nativeFiles(native));
    Object.assign(mobileClient, { environmentId: 'bare-root', origin: 'https://bare.test', projectId: 'a', connection: 'connected', shellLoaded: true });
    mobileClient.shell.projects = ['a', 'b'].map(id => ({ id, title: id, workspaceRoot: `/${id}`, repositoryIdentity: { canonicalKey: 'same' } }));
    calls.length = 0;
    for (const [mode, expected] of [['', 'loading'], ['separate', 'pick'], ['repository', 'prepare']]) {
      const result = obj(await answer('newTaskFlow', ['bare-root-flow', 'bare-visit', '/new/draft', true, 0, 1791420000000, 0, mode], {} as never, storage, native));
      expect(result.status).toBe(expected); expect(result.ready).toBe(false);
      expect(result.needsPrepare).toBe(mode === 'repository');
    }
    expect(calls.map(call => call.op)).toEqual(['environments', 'environments', 'environments']);
    expect(mobileClient.projectId).toBe('a');
  } finally { Object.assign(mobileClient, priorClient); Object.assign(fleet, priorFleet); }
});

test('actual Home source persists Discard through native preferences, not the portable file handle', async () => {
  const key = 'new-task:root-storage-regression';
  const writes: Obj[] = [], calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileOutbox') return { ok: true, generation: mobileClient.generation, value: request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] } };
    if (request.op === 'writePreferences') writes.push(obj(JSON.parse(String(request.text))));
    return { ok: true, generation: mobileClient.generation, value: request.op === 'mobileAlert' ? { choice: 'discard' } : request.op === 'readPreferences' ? { text: '{"version":1}' } : {} };
  } };
  const storage: Files = { fs: { async mkdir() { throw new Error('portable storage unavailable'); },
    async readFile() { throw new Error('portable storage unavailable'); }, async atomicWriteFile() { throw new Error('portable storage unavailable'); } } };
  await mobileClient.command('dismiss-error', '', '', 0, native, nativeFiles(native));
  mobileNewTaskDraftCreate(mobileClient, { id: 'root-storage-regression', environmentId: 'offline-root', projectId: 'project',
    origin: 'https://root.test', createdAt: '2026-10-08T00:00:00Z' });
  mobileClient.local.drafts[key] = 'Root persistence proof';
  mobileHomeActionsObserve('root-home-test', true, false, mobileClient);
  writes.length = 0; calls.length = 0;
  {
    const result = await answer('homeAction', ['root-home-test', 'offline-root', '', 'draft-discard', key, 0], {} as never, storage, native);
    expect(obj(result).message).toBe(''); expect(mobileNewTaskDraftLookup(mobileClient, key)).toBeNull();
    expect(writes.length).toBeGreaterThan(0); expect(obj(writes[0].drafts)[key]).toBeUndefined();
    expect(obj(obj(writes[0].mobileNewTaskDrafts).records)[key]).toBeUndefined();
    expect(calls.some(call => call.op === 'writePreferences')).toBe(true);
    expect(calls.filter(call => call.op === 'mobileOutbox').map(call => call.action)).toEqual(['read', 'transferLookup']);
  }
});

test('actual snapshot shares a later disabled catalog with compact Home and sidebar', async () => {
  const priorClient = { ...mobileClient }, priorFleet = { ...fleet };
  const saved = { environmentId: 'saved-off', origin: 'https://saved-off.test', mobileLabel: 'Saved Mac', enabled: false };
  const calls: Obj[] = [], catalogReads: boolean[] = [];
  let laterCatalog = false;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'localBackendStatus') laterCatalog = true;
    if (request.op === 'environments') catalogReads.push(laterCatalog);
    return { ok: true, generation: 1, value: request.op === 'status'
      ? { state: 'disconnected', origin: '', environmentId: '', message: '' }
      : request.op === 'environments' ? { saved: laterCatalog ? [saved] : [] }
        : request.op === 'readPreferences' ? { text: '{"version":1}' } : {} };
  } };
  const storage: Files = { fs: { async mkdir() { throw new Error('portable storage unavailable'); },
    async readFile() { throw new Error('portable storage unavailable'); }, async atomicWriteFile() { throw new Error('portable storage unavailable'); } } };
  try {
    Object.assign(mobileClient, new MobileDraftClient());
    Object.assign(fleet, { saved: [], entries: new Map(), revision: 0 });
    const now = Date.parse('2026-10-10T00:00:00Z');
    const read = obj(await answer('snapshotRead', [], {} as never, storage, native));
    const snapshot = obj(answer('snapshot', ['light', 't3-code', read.serial, now], {} as never, storage, native));
    expect(catalogReads).toContain(false); expect(catalogReads.at(-1)).toBe(true);
    expect(arr(snapshot.environments)).toHaveLength(1);
    expect(arr(snapshot.environments)[0]).toMatchObject({ environmentId: 'saved-off', label: 'Saved Mac', enabled: false, status: 'Off' });
    expect(snapshot.ready).toBe(false);
    for (const [homeVisible, sidebarVisible, surface] of [[true, false, 'compact'], [false, true, 'sidebar']]) {
      const home = obj(await answer('homeView', [snapshot.revision, now, '', 10, true, false, false, false, false,
        '', '', '', 'repository', 'catalog-root', homeVisible, sidebarVisible, snapshot.environments, surface, true], {} as never, storage, native));
      expect(home.emptyTitle).toBe(surface === 'sidebar' ? 'No threads yet' : 'Environment unavailable'); expect(home.loading).toBe(false);
      expect(home.items).toEqual([]); expect(home.addEnvironment).toBe(surface !== 'sidebar');
    }
    expect(calls.some(call => call.op === 'connect' || call.op === 'fleetStop' || call.fleet !== undefined)).toBe(false);
  } finally {
    Object.assign(mobileClient, priorClient); Object.assign(fleet, priorFleet);
  }
});

test('actual inactive and mismatched thread queries preserve scalar metadata without formatting retained transcript bodies', async () => {
  const prior = { ...mobileClient }, now = Date.parse('2026-10-10T00:00:00Z');
  const text = 'Complete retained 漢字 \u{1f6e0}\n'.repeat(800), body = `Before\n\n\`\`\`unregistered-language\n${text}\`\`\`\nAfter`;
  let bodyReads = 0;
  const item: Obj = { id: 'retained-answer', threadId: 'retained-thread', runId: null, nodeId: null, ordinal: 1,
    type: 'assistant_message', status: 'completed', messageId: 'answer-message', streaming: false };
  Object.defineProperty(item, 'text', { enumerable: true, get() { bodyReads++; return body; } });
  const storage: Files = { fs: { async mkdir() { throw new Error('No storage demand'); },
    async readFile() { throw new Error('No storage demand'); }, async atomicWriteFile() { throw new Error('No storage demand'); } } };
  try {
    Object.assign(mobileClient, new MobileDraftClient());
    mobileClient.environmentId = 'retained-env'; mobileClient.threadId = 'retained-thread'; mobileClient.projectId = 'retained-project';
    mobileClient.thread = { sequence: 22, historyCursor: 'older-opaque', hasMore: true, latestLocalTurnOrdinal: 1, projection: {
      thread: { id: 'retained-thread', projectId: 'retained-project', title: 'Retained thread' }, runs: [], attempts: [], nodes: [],
      checkpoints: [], runtimeRequests: [], turnItems: [item], visibleTurnItems: [{ position: 0, visibility: 'local',
        sourceThreadId: 'retained-thread', sourceItemId: 'retained-answer', item }],
    } };
    mobileClient.local.drafts[mobileClient.draftKey] = '  ';
    const canonical = mobileClient.thread;
    for (const [environment, thread, active, route] of [
      ['retained-env', 'retained-thread', false, 'home'], ['other-env', 'retained-thread', true, 'thread'],
      ['retained-env', 'other-thread', true, 'thread'],
    ]) {
      const view = obj(await answer('threadView', [22, now, 'light', environment, thread, active, 'inactive-visit', route], {} as never, storage));
      expect(view).toMatchObject({ title: '', loaded: false, rows: [], approvals: [], readsNeeded: false,
        environmentId: 'retained-env', threadId: 'retained-thread', hasMore: true, answerFilesRequest: '',
        composer: { draft: '', canSend: false, canStop: false, canOperate: false } });
      expect(bodyReads).toBe(0); expect(mobileClient.thread).toBe(canonical);
    }
    const visible = obj(await answer('threadView', [22, now, 'light', 'retained-env', 'retained-thread', true,
      'visible-visit', 'thread'], {} as never, storage));
    expect(visible.title).toBe('Retained thread'); expect(visible.loaded).toBe(true);
    expect(bodyReads).toBeGreaterThan(0);
    const row = arr(visible.rows)[0]!;
    expect(row.body).toBe(body); expect(arr(row.blocks).find(block => block.kind === 'code')!.text).toBe(text.replace(/\n$/, ''));
    expect(mobileClient.thread).toBe(canonical); expect(obj(visible.composer).draft).toBe('  ');
  } finally { Object.assign(mobileClient, prior); }
});

test('actual preparing and ordinary thread sources bind Copy to the current feed visit', () => {
  // These singleton and WeakMap owners have no exact restore API.
  const result = spawnSync(process.execPath, ['--eval', String.raw`
    import assert from 'node:assert/strict';
    import { answer } from './app.ts';
    import { mobileClient } from './client.ts';
    import { MobileDraftClient } from './mobile-draft-recovery.ts';
    import { mobileOutboxRead, mobileOutboxSnapshot } from './mobile-outbox.ts';
    import { mobileOutboxThread } from './mobile-outbox-presentation.ts';

    let checks = 0, scenarios = 0, nativeReads = 0, bodyReads = 0;
    const failures = [];
    const check = (name, actual, expected) => {
      checks++;
      try { assert.deepStrictEqual(actual, expected); }
      catch (error) { failures.push(name + ': ' + error.message); }
    };
    const scenario = async (name, run) => {
      scenarios++;
      try { await run(); }
      catch (error) { failures.push(name + ': ' + error.stack); }
    };
    const now = Date.parse('2026-10-17T12:00:00Z');
    const environment = 'copy-root-env', thread = 'copy-root-thread', project = 'copy-root-project';
    const raw = 'Canonical server prompt\n漢字 🛠';
    const record = { schemaVersion: 1, origin: 'https://copy-root.invalid', environmentId: environment,
      threadId: thread, messageId: 'copy-root-message', commandId: 'copy-root-command', text: 'Queued prompt', attachments: [],
      creation: { projectId: project, workspaceMode: 'local', branch: 'main', worktreePath: null },
      createdAt: new Date(now - 1000).toISOString() };
    Object.assign(mobileClient, new MobileDraftClient(), { origin: record.origin, environmentId: environment,
      threadId: thread, projectId: project, generation: 7, threadEpoch: 11 });
    const native = { available: true, watch() { throw new Error('No watch'); }, async later(request) {
      assert.equal(request.op, 'mobileOutbox'); assert.equal(request.action, 'read'); nativeReads++;
      return { ok: true, generation: 7, value: { ownerEpoch: 'copy-root-epoch', sequenceFloor: 1, complete: true,
        errors: [], records: [{ record, revision: 1, token: 'copy-root-epoch:1', held: false, pending: false }],
        revisions: { [record.messageId]: 1 }, tokens: { [record.messageId]: 'copy-root-epoch:1' },
        outcomes: [], mutations: [], transfers: [] } };
    } };
    assert.equal(await mobileOutboxRead(mobileClient, native), true);
    const outboxBefore = mobileOutboxSnapshot(mobileClient);
    const item = { id: 'copy-root-item', type: 'user_message', threadId: thread, runId: null, ordinal: 1,
      messageId: record.messageId, status: 'completed', createdBy: 'user', attachments: [],
      get text() { bodyReads++; return raw; } };
    mobileClient.shell.threads = [{ id: thread }];
    mobileClient.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { thread: { id: thread, projectId: project, title: 'Canonical title' },
        messages: [{ id: record.messageId }], runs: [], attempts: [], nodes: [], checkpoints: [],
        visibleTurnItems: [{ position: 0, visibility: 'local', sourceThreadId: thread, sourceItemId: item.id, item }] } };
    const view = (route, visit, active = true, env = environment, id = thread) => answer('threadView',
      [0, now, 'light', env, id, active, visit, route, '', 360, 't3-code',
        '/threads/' + env + '/' + id + (route === 'threadFiles' ? '/files' : '')]);
    const configuration = visit => ({ id: JSON.stringify([thread, item.id]), text: raw, owner: {
      origin: record.origin, environment, generation: mobileClient.generation, project,
      thread, epoch: mobileClient.threadEpoch, route: visit, sourceThread: thread, sourceItem: item.id } });
    try {
      for (const route of ['thread', 'threadFiles']) {
        await scenario('preparing ' + route, async () => {
          mobileClient.thread.projection.messages = [{ id: record.messageId }];
          const visit = 'preparing|' + route + ':one/漢字', result = await view(route, visit);
          check(route + ' preparing', result.queued, true);
          check(route + ' acknowledged rows', result.rows.map(row => row.kind), ['user']);
          check(route + ' acknowledged binding', JSON.parse(result.rows[0].copyConfiguration), configuration(visit));
        });
        await scenario('mixed ' + route, async () => {
          mobileClient.thread.projection.messages = [];
          const visit = 'preparing|' + route + ':two/漢字', result = await view(route, visit);
          check(route + ' mixed preparing', result.queued, true);
          check(route + ' mixed binding', JSON.parse(result.rows[0].copyConfiguration), configuration(visit));
          check(route + ' pending Copy absent', result.rows.map(row => [row.kind, row.copyConfiguration === '', row.first, row.last]),
            [['user', false, true, false], ['pending', true, false, true]]);
        });
      }
      mobileClient.generation = 8; mobileClient.threadEpoch = 12;
      mobileClient.thread.projection.messages = [{ id: record.messageId }];
      mobileClient.thread.projection.runs = [{ id: 'copy-root-run', status: 'completed' }];
      for (const route of ['thread', 'threadFiles']) await scenario('ordinary ' + route, async () => {
        const visit = 'ordinary|' + route + ':new/漢字', result = await view(route, visit);
        check(route + ' handoff completed', result.queued, false);
        check(route + ' ordinary rows', result.rows.map(row => row.kind), ['user']);
        check(route + ' ordinary binding', JSON.parse(result.rows[0].copyConfiguration), configuration(visit));
        check(route + ' owner counters unchanged', [mobileClient.generation, mobileClient.threadEpoch], [8, 12]);
      });
      for (const [name, active, env, id] of [['inactive', false, environment, thread],
        ['foreign environment', true, 'other-env', thread], ['foreign thread', true, environment, 'other-thread']]) {
        await scenario(name, async () => {
          const before = bodyReads, result = await view('thread', name, active, env, id);
          check(name + ' rows absent', result.rows, []);
          check(name + ' retained text untouched', bodyReads, before);
        });
      }
      await scenario('unselected pending creation', async () => {
        mobileClient.threadId = 'other-selected-thread';
        const before = bodyReads, result = await view('thread', 'pending|foreign:visit');
        check('unselected creation retained', result.queued, true);
        check('unselected pending Copy absent', result.rows.map(row => [row.kind, row.copyConfiguration]), [['pending', '']]);
        check('unselected retained text untouched', bodyReads, before);
      });
      check('outbox projection unchanged', mobileOutboxSnapshot(mobileClient), outboxBefore);
      check('only fixture inventory read', nativeReads, 1);
    } finally { mobileOutboxThread('', '', now, false, mobileClient); }
    console.log(JSON.stringify({ checks, scenarios, nativeReads, failures }));
    process.exitCode = failures.length ? 1 : 0;
  `], { cwd: import.meta.dir, encoding: 'utf8', timeout: 15_000 });
  expect({ error: result.error?.message ?? '', status: result.status, signal: result.signal, stderr: result.stderr })
    .toEqual({ error: '', status: 0, signal: null, stderr: '' });
  expect(JSON.parse(result.stdout.trim())).toEqual({ checks: 31, scenarios: 10, nativeReads: 1, failures: [] });
});
