import { expect, test } from 'bun:test';
import { answer } from './app';
import { mobileClient } from './client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileHomeActionsObserve } from './home-actions';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { arr, obj, type Obj } from './shared/domain';
import { nativeFiles, type Native, type Files } from './shared/protocol';
import { fleet } from './shared/settings-b-fleet';

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
    const snapshot = obj(await answer('snapshot', ['light', 't3-code', now], {} as never, storage, native));
    expect(catalogReads).toContain(false); expect(catalogReads.at(-1)).toBe(true);
    expect(arr(snapshot.environments)).toHaveLength(1);
    expect(arr(snapshot.environments)[0]).toMatchObject({ environmentId: 'saved-off', label: 'Saved Mac', enabled: false, status: 'Off' });
    expect(snapshot.ready).toBe(false);
    for (const [homeVisible, sidebarVisible] of [[true, false], [false, true]]) {
      const home = obj(await answer('homeView', [snapshot.revision, now, '', 10, true, false, false, false, false,
        '', '', '', 'repository', 'catalog-root', homeVisible, sidebarVisible, snapshot.environments], {} as never, storage, native));
      expect(home.emptyTitle).toBe('Environment unavailable'); expect(home.loading).toBe(false);
      expect(home.items).toEqual([]); expect(home.addEnvironment).toBe(true);
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
