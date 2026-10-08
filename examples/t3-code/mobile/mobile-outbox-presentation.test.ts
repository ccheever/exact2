// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import type { Native } from './shared/protocol';
import { mobileOutboxRead } from './mobile-outbox';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxCreationDetailReady, mobileOutboxThread, mobileOutboxPendingTasks, mobileOutboxOwner, projectHomePending, mobileOutboxStatus } from './mobile-outbox-presentation';
import { projectMobileHome, type HomeSource } from './home';
import { initialShell, type Obj } from './shared/domain';

const now = Date.parse('2026-10-08T12:00:00Z');
const record = (messageId = 'm', minutes = 0): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://home.test',
  environmentId: 'env', threadId: `thread-${messageId}`, messageId, commandId: `command-${messageId}`, text: 'Inspect the app', attachments: [],
  creation: { projectId: 'project', projectTitle: 'Saved project', workspaceMode: 'local', branch: 'main', worktreePath: null },
  createdAt: new Date(now + minutes * 60000).toISOString() });
async function loaded(records: MobileOutboxRecord[]) {
  const client = new T3Client(); client.environmentId = 'env'; client.threadId = 'other-thread'; client.projectId = 'other-project';
  const native: Native = { available: true, watch() { throw Error('No watch'); }, async later() { return { ok: true, generation: 0,
    value: { ownerEpoch: 'epoch', sequenceFloor: records.length, complete: true, errors: [],
      records: records.map(record => ({ record, revision: 1, token: 'epoch:1', held: false, pending: false })),
      revisions: Object.fromEntries(records.map(record => [record.messageId, 1])), tokens: Object.fromEntries(records.map(record => [record.messageId, 'epoch:1'])),
      outcomes: [], mutations: [], transfers: [] } }; } };
  expect(await mobileOutboxRead(client, native)).toBe(true); return client;
}
test('queued route retains original owner and prompt without requesting local attachment IDs', async () => {
  const original = record(); original.attachments.push({ kind: 'image', id: '11111111-1111-4111-a111-111111111111', name: 'Screenshot.png',
    mimeType: 'image/png', sizeBytes: 4, uploadId: '', status: 'staged' });
  const client = await loaded([original]), before = [client.threadId, client.projectId, client.pending];
  const view = mobileOutboxThread('env', original.threadId, now, false, client)!;
  expect(view).toMatchObject({ queued: true, queuedOwner: mobileOutboxOwner(original), title: 'Inspect the app', loaded: true, loading: false,
    rows: [{ id: 'm', kind: 'pending', body: original.text, media: [] }],
    composer: { contentOwner: '', canOperate: false, canSend: false, canStop: false, draft: '' } });
  expect([client.threadId, client.projectId, client.pending]).toEqual(before);
  expect(mobileOutboxThread('different', original.threadId, now, false, client)).toBeNull();
  expect(mobileOutboxThread('env', 'missing', now, false, client)).toBeNull();
});
test('matching shell permits detail selection while the local creation stays visible', async () => {
  const original = record(), client = await loaded([original]);
  client.environmentId = 'different'; client.shell.threads = [{ id: original.threadId }];
  expect(mobileOutboxThread('env', original.threadId, now, false, client)?.queued).toBe(true);
  client.environmentId = 'env';
  expect(mobileOutboxThread('env', original.threadId, now, false, client)).toMatchObject({ queued: true, queuedCanSelect: true });
});
test('ordinary queued follow-ups never manufacture a pending creation route', async () => {
  const original = record(); delete original.creation;
  const client = await loaded([original]);
  expect(mobileOutboxPendingTasks(client, now)).toEqual([]);
  expect(mobileOutboxThread('env', original.threadId, now, false, client)).toBeNull();
});
test('pending Home tasks are scoped, filtered and newest-first with captured project fallback', async () => {
  const client = await loaded([record('old', -5), record('new', -1)]), tasks = mobileOutboxPendingTasks(client, now);
  expect(projectHomePending(tasks).map(item => item.record.messageId)).toEqual(['new', 'old']);
  expect(projectHomePending(tasks, { environmentId: 'other' })).toEqual([]);
  expect(projectHomePending(tasks, { projectRefs: [] })).toEqual([]);
  expect(projectHomePending(tasks, { query: 'no match' })).toEqual([]);
  const result = projectMobileHome([], now, { pendingTasks: tasks });
  expect(result.items.map(item => [item.kind, item.title, item.projectTitle, item.showPendingDivider])).toEqual([
    ['pending', 'Inspect the app', 'Saved project', true], ['pending', 'Inspect the app', 'Saved project', false] ]);
  expect(result.items[0]?.queuedOwner).toBe(mobileOutboxOwner(record('new', -1)));
});
test('drafts precede pending creations, synchronized rows do not duplicate and divider survives filtering', async () => {
  const current = record('present'), client = await loaded([current, record('pending', -1)]);
  const source: HomeSource = { environmentId: 'env', label: 'Local', machine: 'laptop', focused: true, config: {},
    shell: { ...initialShell(), projects: [{ id: 'project', title: 'Project' }], threads: [{ id: current.threadId, projectId: 'project',
      title: 'Synchronized', createdAt: current.createdAt, updatedAt: current.createdAt }] } };
  const tasks = mobileOutboxPendingTasks(client, now);
  const queued = projectMobileHome([source], now, { pendingTasks: tasks });
  expect(queued.items.map(item => item.kind)).toEqual(['thread', 'pending']);
  expect(queued.items[1]?.showPendingDivider).toBe(true);
  const mixed = projectMobileHome([source], now, { pendingTasks: tasks, drafts: [{ key: 'new-task:draft', environmentId: 'env', projectId: 'project',
    origin: 'https://home.test', createdAt: new Date(now - 600000).toISOString(), text: 'Older draft', images: [], files: [], workspace: null }] });
  expect(mixed.items.map(item => item.kind)).toEqual(['thread', 'draft', 'pending']);
  expect(mixed.items.filter(item => item.showPendingDivider).length).toBe(1);
  expect(mixed.items[1]?.trailingDivider).toBe(true);
});
test('durable ACK and ambiguous outcomes use distinct status labels', () => {
  expect(mobileOutboxStatus('delivered')).toBe('Starting task…');
  expect(mobileOutboxStatus('recovery-required')).toBe('Needs attention');
  expect(mobileOutboxStatus('cleanup-pending')).toBe('Finishing send…');
  expect(mobileOutboxStatus('queued')).toBe('Sends on reconnect');
});


test('a preparing route uses the real echoed prompt while waiting for its turn', async () => {
  const original = record(), client = await loaded([original]); client.threadId = original.threadId;
  const item: Obj = { id: 'user-item', type: 'user_message', threadId: original.threadId, runId: null, ordinal: 1,
    text: 'Canonical server prompt', messageId: original.messageId, status: 'completed', createdBy: 'user', attachments: [] };
  client.shell.threads = [{ id: original.threadId, latestRun: null }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
    projection: { thread: { id: original.threadId }, messages: [{ id: original.messageId }], runs: [], attempts: [], nodes: [], checkpoints: [],
      visibleTurnItems: [{ position: 0, visibility: 'local', sourceThreadId: original.threadId, sourceItemId: item.id, item }] } };
  const preparing = mobileOutboxThread('env', original.threadId, now, false, client)!;
  expect(preparing.queuedCanSelect).toBe(true); expect(preparing.rows.map(row => row.body)).toEqual(['Canonical server prompt']);
  expect(preparing.composer.canSend).toBe(false);
  client.thread.projection.messages = [];
  const awaitingPrompt = mobileOutboxThread('env', original.threadId, now, false, client)!;
  expect(awaitingPrompt.rows.map(row => [row.first, row.last])).toEqual([[true, false], [false, true]]);
  client.thread.projection.messages = [{ id: original.messageId }];
  client.thread.projection.runs = [{ id: 'first', status: 'running' }];
  expect(mobileOutboxThread('env', original.threadId, now, false, client)).toBeNull();
});
test('only matching prompt plus turn or a terminal detail releases creation presentation', () => {
  const original = record();
  for (const detail of [null, {}, { messages: [{ id: original.messageId }] }, { messages: [], runs: [{ status: 'running' }] },
    { messages: [{ id: 'other' }], latestTurn: { turnId: 'run' } }]) expect(mobileOutboxCreationDetailReady(original, detail)).toBe(false);
  for (const timing of [{ runs: [{ status: 'running' }] }, { latestTurn: { turnId: 'run' } }])
    expect(mobileOutboxCreationDetailReady(original, { messages: [{ id: original.messageId }], ...timing })).toBe(true);
  for (const status of ['failed', 'error', 'cancelled', 'stopped', 'interrupted']) {
    expect(mobileOutboxCreationDetailReady(original, { runs: [{ status }], messages: [] })).toBe(true);
    expect(mobileOutboxCreationDetailReady(original, { session: { status }, messages: [] })).toBe(true);
  }
});
