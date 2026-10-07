import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { mobileQueueSnapshot, mobileQueueCommand } from './queue';
import { mobileQueuePrepare } from './queue-read';
function fixture() {
  const client = new T3Client(), calls: Obj[] = [];
  Object.assign(client, { origin: 'https://example.test', environmentId: 'e', projectId: 'p', threadId: 't', generation: 9,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 't', activeProviderThreadId: 'pt' }, runs: [
      { id: 'active', status: 'running', ordinal: 0, activeAttemptId: 'attempt', providerThreadId: 'pt' },
      { id: 'q1', status: 'queued', ordinal: 1, queuePosition: 1, userMessageId: 'm1' },
      { id: 'q2', status: 'queued', ordinal: 2, queuePosition: 2, userMessageId: 'm2' }],
    messages: [{ id: 'm1', text: 'First\n  message', attachments: [] }, { id: 'm2', text: 'Second', attachments: [] }],
    providerThreads: [{ id: 'pt', providerSessionId: 'session' }], providerSessions: [{ id: 'session', capabilities: { turns: { supportsQueuedMessages: true, supportsActiveSteering: true } } }],
    providerTurns: [{ id: 'turn', runAttemptId: 'attempt', status: 'running' }] } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } };
  let grants = ['orchestration:operate'], counter = 0, hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions: grants }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++counter}`)
        : request.method === 'assets.createUrl' ? { relativeUrl: '/signed/image', expiresAt: 10000 } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const snapshot = () => mobileQueueSnapshot('visit', true, 0, client);
  const action = (op: string, id = 'q1', value = '') => {
    const view = snapshot(), ticket = id ? view.rows.find(row => row.id === id)!.actionId : view.actionId;
    return mobileQueueCommand([`queue:${op}`, ticket, value], native, files, client);
  };
  return { client, native, files, calls, snapshot, action, grant(value: string[]) { grants = value; }, hook(value: typeof hook) { hook = value; } };
}
const writes = (f: ReturnType<typeof fixture>) => f.calls.filter(call => call.method === 'orchestration.dispatchCommand').map(call => obj(call.payload));

test('queue rows preserve source text and exclude automatic notifications; edit uses a dedicated content owner', () => {
  const f = fixture(), p = f.client.projection;
  (p.messages as Obj[]).push({ id: 'automatic', text: 'Notification', notification: {} });
  (p.runs as Obj[]).push({ id: 'auto', userMessageId: 'automatic', status: 'queued', ordinal: 3 });
  const data = f.snapshot();
  expect(data.count).toBe(2); expect(data.rows[0]?.text).toBe('First\n  message');
  expect(data.rows[0]).toMatchObject({ canEdit: true, canSteer: true, canMoveUp: false, canMoveDown: true, canRemove: true });
  expect(data.rows[1]).toMatchObject({ canMoveUp: true, canMoveDown: false });
  expect(data.editingUnavailable).toBe(false);
});

test('real shared remove/reorder/steer payloads and durable completion preserve ordinary draft and images', async () => {
  for (const [op, id, expected] of [
    ['remove', 'q1', { type: 'queued-run.cancel', runId: 'q1' }],
    ['up', 'q2', { type: 'queued-run.reorder', runId: 'q2', beforeRunId: 'q1' }],
    ['down', 'q1', { type: 'queued-run.reorder', runId: 'q1', beforeRunId: null }],
    ['steer', 'q1', { type: 'queued-message.promote-to-steer', queuedRunId: 'q1', targetRunId: 'active' }],
  ] as const) {
    const f = fixture(); f.client.local.drafts['e:t'] = 'Ordinary'; f.client.local.snapshotDrafts['e:t'] = [{ id: 'image', uploadId: 'uploaded' }];
    expect((await f.action(op, id)).message).toBe('');
    expect(writes(f)).toHaveLength(1); expect(writes(f)[0]).toMatchObject({ ...expected, threadId: 't' });
    expect(f.client.draft).toBe('Ordinary'); expect(f.client.snapshotDrafts).toHaveLength(1);
    expect(f.client.pending).toBeUndefined(); expect(f.client.busy).toBe(false);
  }
});

test('Resume queue never dispatches the generic resume continuation prompt', async () => {
  const f = fixture(), runs = f.client.projection.runs as Obj[];
  runs[0]!.status = 'interrupted'; runs[0]!.startedAt = '2026-10-07T10:00:00Z'; runs[1]!.queueHeld = true;
  expect(f.snapshot().canResume).toBe(true);
  await f.action('resume', ''); expect(writes(f).map(payload => payload.type)).toEqual(['queue.resume']);
});

test('fresh explicit permission denial dispatches nothing', async () => {
  const f = fixture(); f.grant([]);
  expect((await f.action('remove')).message).toContain('cannot manage');
  expect(writes(f)).toHaveLength(0); expect(f.calls.some(call => call.op === 'ids')).toBe(false);
});

test('run/order/capability/active-target/selection changes during IDs prevent dispatch', async () => {
  for (const kind of ['run', 'order', 'capability', 'active', 'selection', 'permission', 'closed']) {
    const f = fixture(); let release!: () => void, entered!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
    f.hook(async request => {
      if (request.op !== 'ids') return request.path === '/api/auth/session' ? { ok: true, generation: 9, value: { authenticated: true, permissions: ['orchestration:operate'] } } : { ok: true, generation: 9, value: {} };
      entered(); await gate; return { ok: true, generation: 9, value: ['id'] };
    });
    const pending = f.action(kind === 'active' ? 'steer' : kind === 'order' || kind === 'capability' ? 'up' : 'remove', 'q2');
    await started;
    const p = f.client.projection;
    if (kind === 'run') (p.runs as Obj[])[2]!.status = 'running';
    if (kind === 'order') (p.runs as Obj[])[2]!.queuePosition = 0;
    if (kind === 'capability') obj(obj((p.providerSessions as Obj[])[0]!.capabilities).turns).supportsQueuedMessages = false;
    if (kind === 'active') (p.runs as Obj[])[0]!.id = 'new-active';
    if (kind === 'selection') f.client.threadId = 'other';
    if (kind === 'permission') f.client.scopes = [];
    if (kind === 'closed') mobileQueueSnapshot('visit', false, 0, f.client);
    release();
    try { await pending; } catch (error) { expect(obj(error).kind).toBe('superseded'); }
    expect(writes(f)).toHaveLength(0);
  }
});

test('sheet auto-closes only when a nonempty queue becomes empty during that visit', () => {
  const f = fixture(); f.snapshot(); f.client.projection.runs = [];
  expect(f.snapshot().dismiss).toBe(true);
  expect(mobileQueueSnapshot('another-visit', true, 0, f.client).dismiss).toBe(false);
});

test('queue thumbnail URLs come from actual owned attachment IDs and expire', async () => {
  const f = fixture(), images = Array.from({ length: 4 }, (_, i) => ({ id: `image-${i}`, name: `${i}.png`, mimeType: 'image/png' }));
  (f.client.projection.messages as Obj[])[0]!.attachments = [...images, { id: 'file', name: 'a.txt', mimeType: 'text/plain' }];
  const first = f.snapshot();
  await mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 0, f.native, f.client);
  const data = f.snapshot();
  expect(data.rows[0]?.attachmentOverflow).toBe('+2'); expect(data.rows[0]?.attachments).toHaveLength(3);
  expect(data.rows[0]?.attachments.every(item => item.url === 'https://example.test/signed/image')).toBe(true);
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(3);
  expect(mobileQueueSnapshot('visit', true, 100, f.client).thumbnailRequest).toBe('');
  expect(mobileQueueSnapshot('visit', true, 10000, f.client).rows[0]?.attachments[0]?.url).toBe('');
  expect(mobileQueueSnapshot('visit', true, 10000, f.client).thumbnailRequest).not.toBe('');
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(3);
});

test('closing the sheet while a thumbnail signs cannot reactivate it or adopt a late URL', async () => {
  const f = fixture(); (f.client.projection.messages as Obj[])[0]!.attachments = [{ id: 'image', name: 'a.png', mimeType: 'image/png' }];
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.hook(async request => { if (request.method === 'assets.createUrl') { entered(); await gate; } return { ok: true, generation: 9, value: { relativeUrl: '/late', expiresAt: 1000 } }; });
  const first = f.snapshot();
  const pending = mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 0, f.native, f.client); await started;
  mobileQueueSnapshot('visit', false, 0, f.client); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
});

test('a competing pending operation during IDs survives and prevents dispatch', async () => {
  const f = fixture(), other = { method: 'orchestration.dispatchCommand', payload: { type: 'other', commandId: 'other' }, description: 'Other', threadId: 't', text: '', uncertain: true };
  f.hook(request => { if (request.op === 'ids') { f.client.local.pending.e = other; return { ok: true, generation: 9, value: ['queue-id'] }; } });
  await expect(f.action('remove')).rejects.toMatchObject({ kind: 'superseded' });
  expect(writes(f)).toHaveLength(0); expect(f.client.pending).toBe(other);
});

test('known-unsent invalidation during persistence removes and persists only its queue pending', async () => {
  for (const kind of ['run', 'closed', 'permission']) {
    const f = fixture(), persisted: Obj[] = [];
    f.files.fs.atomicWriteFile = async (_path, bytes) => {
      persisted.push(obj(JSON.parse(new TextDecoder().decode(bytes))));
      if (persisted.length === 1) {
        if (kind === 'run') (f.client.projection.runs as Obj[])[1]!.status = 'running';
        if (kind === 'closed') mobileQueueSnapshot('visit', false, 0, f.client);
        if (kind === 'permission') f.client.scopes = [];
      }
    };
    await f.action('remove');
    expect(writes(f)).toHaveLength(0); expect(f.client.pending).toBeUndefined();
    expect(obj(persisted.at(-1)?.pending).e).toBeUndefined();
    expect(persisted.length).toBeGreaterThanOrEqual(2);
  }
});

test('pure rerenders and repeated preparation retain a pending metadata owner without duplicate signing', async () => {
  const f = fixture(); (f.client.projection.messages as Obj[])[0]!.attachments = [{ id: 'image', name: 'a.png', mimeType: 'image/png' }];
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.hook(async request => { if (request.method === 'assets.createUrl') { entered(); await gate; } return { ok: true, generation: 9, value: { relativeUrl: '/signed', expiresAt: 1000 } }; });
  const first = f.snapshot(), pending = mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 0, f.native, f.client);
  await started;
  const during = f.snapshot(); expect(during.rows[0]?.canRemove).toBe(true); expect(during.thumbnailRequest).toBe('');
  await mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 1, f.native, f.client);
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(1);
  release(); await pending; expect(f.snapshot().rows[0]?.attachments[0]?.url).toBe('https://example.test/signed');
});

test('a rejected thumbnail worker drains sibling answers before returning', async () => {
  const f = fixture(); (f.client.projection.messages as Obj[])[0]!.attachments = [0, 1].map(i => ({ id: `image-${i}`, name: `${i}.png`, mimeType: 'image/png' }));
  let release!: () => void, active = 0, settled = false;
  const gate = new Promise<void>(resolve => { release = resolve; });
  f.hook(async request => {
    if (obj(obj(request.payload).resource).attachmentId === 'image-0') throw { name: 'FetchError', kind: 'Aborted' };
    active++; await gate; active--; return { ok: true, generation: 9, value: { relativeUrl: '/signed', expiresAt: 1000 } };
  });
  const first = f.snapshot(), pending = mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 0, f.native, f.client).catch(() => { settled = true; });
  for (let i = 0; i < 30; i++) await Promise.resolve();
  expect(settled).toBe(false); expect(active).toBe(1);
  release(); await pending; expect(settled).toBe(true); expect(active).toBe(0);
});

test('same attachment ID with changed MIME or filename cannot reuse an old signed resource', async () => {
  const f = fixture(), attachment = { id: 'image', name: 'a.png', mimeType: 'image/png' };
  (f.client.projection.messages as Obj[])[0]!.attachments = [attachment];
  const first = f.snapshot(); await mobileQueuePrepare(first.owner, first.visit, first.thumbnailRequest, 0, f.native, f.client);
  attachment.name = 'renamed.png'; const changed = f.snapshot();
  expect(changed.rows[0]?.attachments[0]?.url).toBe(''); expect(changed.thumbnailRequest).not.toBe(first.thumbnailRequest);
});
