import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { mobileQueueSnapshot, mobileQueueCommand } from './queue';
import { mobileQueuedEditBegin, mobileQueuedEditCancel, mobileQueuedEditSave, mobileQueuedEditRetry, mobileQueuedEditRefresh, mobileQueuedEditPresentation } from './queued-edit';
import { mobileQueuedEditCurrent, mobileQueuedEditLookup, mobileQueuedEditPersist, mobileQueuedEditWriteText, queuedEditReplaceAttachments } from './queued-edit-state';
import { queuedEditState, queuedEditSetNotice, queuedEditNoticeOwner } from './queued-edit-memory';
import { queuedEditResolvePayload } from './queued-edit-upload';
import { queuedEditRefreshOrigin, mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileQueuedEditAttachmentAction } from './queued-edit-attachments';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileThreadComposer } from './thread';
import { mobileDraftChanged } from './draft';
import { nativeFiles } from './shared/protocol';
import { mobileComposerTarget } from './composer-target';
import { fleet } from './shared/settings-b-fleet';
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fixture(client: T3Client = new T3Client()) {
  const records = new Map<string, Obj>(), operations = new Map<string, Obj>(), calls: Obj[] = [], sent: Obj[] = [];
  let counter = 0, hook: ((input: Obj) => Promise<void> | void) | undefined, uncertain = false, rejected = false, grants = true, failPersist = false, picked: Obj[] = [], home = 'https://example.test', saved = '';
  Object.assign(client, { origin: 'https://example.test', environmentId: 'e', projectId: 'p', threadId: 't', generation: 9, providerId: 'provider', modelId: 'model',
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'], loaded: true });
  client.config = { providers: [{ instanceId: 'provider', enabled: true, installed: true, models: [{ slug: 'model' }] }], environment: { capabilities: { serverResolvedCommandContext: true, attachmentUploads: true, fileAttachments: { maxUploadBytes: 50000000 } } } };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 't' }, runs: [{ id: 'q', status: 'queued', ordinal: 1, queuePosition: 1, userMessageId: 'm' }],
    messages: [{ id: 'm', text: 'Original', attachments: [{ type: 'image', id: 'server-image', name: 'kept.png', mimeType: 'image/png', sizeBytes: 3 }] }] } };
  const ok = (value: unknown) => ({ ok: true, generation: client.generation, value });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(clone(request)); await hook?.(request);
    if (request.op === 'status') return ok({ origin: client.origin, environmentId: client.environmentId, homeOrigin: home });
    if (request.op === 'forgetEnvironment') return ok({ state: client.connection, origin: client.origin, environmentId: client.environmentId, message: client.statusMessage });
    if (request.op === 'ids') return ok(Array.from({ length: Number(request.count) }, () => `00000000-0000-4000-8000-${String(++counter).padStart(12, '0')}`));
    if (request.path === '/api/auth/session') return ok({ authenticated: true, permissions: grants ? ['orchestration:operate'] : [] });
    if (request.op === 'writePreferences') { if (failPersist) throw new Error('disk failed'); saved = String(request.text); return ok({}); }
    if (request.op === 'mobileQueuedEdit') {
      const owner = String(request.owner), operationId = String(request.operationId);
      if (request.action === 'read') return ok({ records: [...records.entries()].map(([owner, record]) => ({ owner, revision: record.revision, record })), operations: [...operations.values()] });
      if (request.action === 'cas') {
        const previous = records.get(owner), incoming = obj(request.record);
        if (!previous || Number(previous.revision) <= Number(request.revision)) records.set(owner, clone(incoming));
        return ok({ applied: true, record: records.get(owner), revision: records.get(owner)?.revision });
      }
      if (request.action === 'reserve') {
        if (client.pending) return { ok: false, generation: client.generation, error: { kind: 'busy', message: 'Another write owns this environment.' } };
        const record = records.get(owner)!;
        const operation = { operationId, owner, editorRevision: request.editorRevision, revision: 1, origin: home, environmentId: record.environmentId,
          state: 'reserved', method: request.method, payload: clone(request.payload) };
        operations.set(operationId, operation); return ok(operation);
      }
      if (request.action === 'send') {
        const operation = operations.get(operationId)!;
        operation.state = 'issued'; operation.revision = Number(operation.revision) + 1; sent.push(clone(obj(operation.payload)));
        operation.state = uncertain ? 'uncertain' : rejected ? 'rejected' : 'acknowledged'; operation.revision = Number(operation.revision) + 1;
        if (uncertain || rejected) return { ok: false, generation: client.generation, error: { kind: 'server', message: uncertain ? 'lost reply' : 'run changed', uncertain } };
        return ok({ operation, result: {} });
      }
      if (request.action === 'retire') { const op = operations.get(operationId)!; if (['issued', 'uncertain'].includes(String(op.state))) throw new Error('unresolved'); operations.delete(operationId); return ok({ removed: true }); }
      if (request.action === 'cleanup') { if (!records.has(owner) || records.get(owner)?.revision !== request.editorRevision) return { ok: false, generation: client.generation, error: { kind: 'stale', message: 'The editor changed before cleanup.' } }; records.delete(owner); for (const [id, op] of operations) if (op.owner === owner) operations.delete(id); return ok({}); }
      if (request.action === 'release') return ok({});
    }
    if (request.op === 'mobileVoice' && request.action === 'selection') return ok({ start: String(request.text).length, end: String(request.text).length });
    if (request.op === 'composerAttachPick') return ok({ files: picked });
    if (request.op === 'snapshotDraftRead' || request.op === 'composerAttachRead') return ok({ base64: 'YWJj', sizeBytes: 3 });
    if (request.method === 'attachments.createUploadUrl') return ok({ attachmentId: `uploaded-${++counter}`, relativeUrl: '/api/attachments/upload/token' });
    if (request.method === 'assets.createUrl') return ok({ relativeUrl: '/signed', expiresAt: 999999 });
    return ok({});
  } };
  const begin = async () => {
    const row = mobileQueueSnapshot('visit', true, 0, client).rows[0]!;
    const result = await mobileQueuedEditBegin(row.actionId, native, client); expect(result.message).toBe('');
    return mobileQueuedEditCurrent(client)!;
  };
  return { client, native, records, operations, calls, sent, begin, hook(value: typeof hook) { hook = value; }, uncertain(value: boolean) { uncertain = value; },
    home(value: string) { home = value; }, picked(value: Obj[]) { picked = value; }, rejected(value: boolean) { rejected = value; }, grants(value: boolean) { grants = value; }, failPersist(value: boolean) { failPersist = value; }, saved: () => saved };
}

function mobileFixture() {
  const f = fixture(new MobileDraftClient()), projection = clone(f.client.projection);
  for (const key of ['attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests',
    'plans', 'turnItems', 'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems']) projection[key] = [];
  projection.messages = (projection.messages as Obj[]).map(message => ({ ...message, role: 'user' }));
  f.client.shell = { ...f.client.shell, projects: [{ id: 'p', title: 'Project' }], threads: ['t', 'other'].map(id => ({ id, projectId: 'p' })) };
  const original = f.native.later;
  f.native.later = async input => {
    const request = obj(input), id = String(request.path ?? '').match(/^\/api\/orchestration\/threads\/([^/]+)\/bounded$/)?.[1];
    if (id) return { ok: true, generation: f.client.generation, value: { snapshotSequence: 1, projection: {
      ...clone(projection), thread: { id, projectId: 'p', modelSelection: { instanceId: 'provider', model: 'model' } },
    } } };
    return original(input);
  };
  return { ...f, async select(id: string) {
    expect((await f.client.command('select-thread', id, '', 0, f.native, nativeFiles(f.native))).message).toBe('');
    expect(f.client.threadId).toBe(id);
  } };
}

test('actual mobile Dismiss clears only its presented notice without native or persistence work', async () => {
  const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
  queuedEditSetNotice(f.client, 'This message failed.', owner);
  const shown = mobileThreadComposer(f.client), before = f.calls.length;
  expect(shown.editNoticeDismissKey).not.toBe('');
  expect((await f.client.command('queued-edit-notice-dismiss', shown.editNoticeDismissKey, '', 0, f.native, nativeFiles(f.native))).message).toBe('');
  expect(mobileThreadComposer(f.client)).toMatchObject({ editNotice: '', editNoticeDismissKey: '' });
  expect(f.calls).toHaveLength(before);
});

test('captured Dismiss cannot erase another thread, environment, canonical server or same-text replacement', async () => {
  for (const changed of ['thread', 'environment', 'origin', 'replacement'] as const) {
    const f = mobileFixture(), original = queuedEditNoticeOwner(f.client)!;
    queuedEditSetNotice(f.client, 'Same message', original);
    const key = mobileThreadComposer(f.client).editNoticeDismissKey;
    if (changed === 'thread') await f.select('other');
    if (changed === 'environment') f.client.environmentId = 'another';
    if (changed === 'origin') f.client.origin = 'https://replacement.test';
    queuedEditSetNotice(f.client, 'Same message', queuedEditNoticeOwner(f.client));
    await f.client.command('queued-edit-notice-dismiss', key, '', 0, f.native, nativeFiles(f.native));
    expect(mobileThreadComposer(f.client).editNotice).toBe('Same message');
    expect(mobileThreadComposer(f.client).editNoticeDismissKey).not.toBe(key);
    if (changed !== 'replacement') {
      Object.assign(f.client, { origin: original.origin, environmentId: original.environmentId, threadId: original.threadId });
      expect(mobileThreadComposer(f.client).editNoticeDismissKey).toBe(key);
    }
  }
});

test('journal error hides scoped Dismiss and rejects an old scoped action until healthy Refresh', async () => {
  const f = mobileFixture(); queuedEditSetNotice(f.client, 'Thread error', queuedEditNoticeOwner(f.client));
  const key = mobileThreadComposer(f.client).editNoticeDismissKey;
  queuedEditSetNotice(f.client, 'Journal unavailable', null);
  await f.client.command('queued-edit-notice-dismiss', key, '', 0, f.native, nativeFiles(f.native));
  expect(mobileThreadComposer(f.client)).toMatchObject({ editNotice: 'Journal unavailable', editNoticeDismissKey: '' });
  await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileThreadComposer(f.client)).toMatchObject({ editNotice: 'Thread error', editNoticeDismissKey: key });
});

test('successful actual environment removal retires every old route notice, late failures included, despite later save failure', async () => {
  const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
  queuedEditSetNotice(f.client, 'Primary', owner);
  queuedEditSetNotice(f.client, 'Old route', { ...owner, origin: 'https://old-route.test', threadId: 'older' });
  queuedEditSetNotice(f.client, 'Other environment', { ...owner, environmentId: 'e10' });
  queuedEditSetNotice(f.client, 'Journal unavailable', null);
  f.hook(request => { if (request.op === 'forgetEnvironment') {
    queuedEditSetNotice(f.client, 'Late failure on removed environment', { ...owner, threadId: 'late' });
    f.client.environmentId = 'other'; f.client.threadId = 'other';
    f.failPersist(true);
  } });
  const result = await f.client.command('environment-forget', owner.origin, owner.environmentId, 0, f.native, nativeFiles(f.native));
  expect(result.message).toBe('');
  expect(f.calls.filter(request => request.op === 'writePreferences')).toHaveLength(1);
  expect(f.client.error).toContain('Could not save local drafts and preferences. Keep a copy before closing.');
  expect(f.calls.filter(request => request.op === 'forgetEnvironment')).toEqual([{ op: 'forgetEnvironment', origin: owner.origin, environmentId: owner.environmentId }]);
  const keys = [...queuedEditState(f.client).notices.keys()].map(key => JSON.parse(key));
  expect(keys).toEqual([[owner.origin, 'e10', 't']]);
  expect(queuedEditState(f.client).globalNotice?.message).toBe('Journal unavailable');
  Object.assign(f.client, { origin: owner.origin, environmentId: owner.environmentId, threadId: 'late' });
  expect(queuedEditState(f.client).notices.size).toBe(1);
});

test('failed, invalid, interrupted and superseded removal replies cannot retire composer notices', async () => {
  for (const failure of ['failed', 'invalid', 'let-go', 'superseded', 'superseded-invalid-status'] as const) {
    const f = mobileFixture(); queuedEditSetNotice(f.client, 'Keep error', queuedEditNoticeOwner(f.client));
    const key = mobileThreadComposer(f.client).editNoticeDismissKey, native = f.native.later;
    f.native.later = async input => {
      if (obj(input).op !== 'forgetEnvironment') return native(input);
      if (failure === 'failed') return { ok: false, generation: f.client.generation, error: { kind: 'storage', message: 'Removal failed' } };
      if (failure === 'invalid') return { ok: true, value: {} };
      if (failure === 'let-go') throw { name: 'FetchError', kind: 'Aborted' };
      // Actual shared nonlocal command supersession after native work but before
      // the first removal's ownedNative can accept the successful reply.
      await f.client.command('close-diff', '', '', 0, f.native, nativeFiles(f.native));
      await f.client.command('unknown-nonlocal-command', '', '', 0, f.native, nativeFiles(f.native));
      return { ok: true, generation: f.client.generation, value: failure === 'superseded-invalid-status' ? {} : {
        state: f.client.connection, origin: f.client.origin, environmentId: f.client.environmentId, message: f.client.statusMessage } };
    };
    await f.client.command('environment-forget', f.client.origin, f.client.environmentId, 0, f.native, nativeFiles(f.native));
    expect(mobileThreadComposer(f.client).editNoticeDismissKey).toBe(key);
    expect(f.calls.some(request => request.op === 'sshForget')).toBe(false);
  }
});

test('generated successful replies with malformed decoded status preserve notices and the shared protocol error', async () => {
  const good = { state: 'connected', origin: 'https://example.test', environmentId: 'e', message: '' };
  for (const status of [{}, null, [], { ...good, state: 'ready' }, { ...good, origin: null },
    { ...good, environmentId: 9 }, { ...good, message: false }]) {
    const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
    queuedEditSetNotice(f.client, 'Keep malformed-status error', owner);
    queuedEditSetNotice(f.client, 'Keep another route', { ...owner, origin: 'https://old-route.test', threadId: 'older' });
    const key = mobileThreadComposer(f.client).editNoticeDismissKey, native = f.native.later;
    f.native.later = async input => {
      const reply = await native(input);
      return obj(input).op === 'forgetEnvironment' ? { ...obj(reply), value: status } : reply;
    };
    const result = await f.client.command('environment-forget', owner.origin, owner.environmentId, 0, f.native, nativeFiles(f.native));
    expect(result.message).toBe('The native bridge returned an invalid connection status. Reconnect and try again.');
    expect(mobileThreadComposer(f.client).editNoticeDismissKey).toBe(key);
    expect([...queuedEditState(f.client).notices.values()].map(notice => notice.message)).toEqual(['Keep malformed-status error', 'Keep another route']);
    expect(f.client.connection).toBe('connected'); expect(f.client.generation).toBe(9);
    // Shared connection cleanup still runs before its status adoption. It must
    // neither normalize the invalid body nor retire the app's notice owner.
    expect(f.calls.filter(request => request.op === 'sshForget')).toHaveLength(1);
    expect(f.calls.some(request => request.op === 'writePreferences')).toBe(false);
  }
});

test('a valid removal status older than the live generation cannot retire its notices', async () => {
  const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
  queuedEditSetNotice(f.client, 'Keep stale removal error', owner);
  const key = mobileThreadComposer(f.client).editNoticeDismissKey, native = f.native.later;
  f.native.later = async input => {
    if (obj(input).op !== 'forgetEnvironment') return native(input);
    const generation = f.client.generation; f.client.generation++;
    return { ...obj(await native(input)), generation };
  };
  const result = await f.client.command('environment-forget', owner.origin, owner.environmentId, 0, f.native, nativeFiles(f.native));
  expect(result.message).toBe(''); expect(f.client.generation).toBe(10);
  expect(mobileThreadComposer(f.client).editNoticeDismissKey).toBe(key);
  expect([...queuedEditState(f.client).notices.values()].map(notice => notice.message)).toEqual(['Keep stale removal error']);
  expect(f.calls.filter(request => request.op === 'sshForget')).toHaveLength(1);
});

test('actual status adoption after removal reply admission preserves notices at cleanup', async () => {
  for (let depth = 1; depth <= 8; depth++) {
    const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
    queuedEditSetNotice(f.client, 'Keep stale generation error', owner);
    const key = mobileThreadComposer(f.client).editNoticeDismissKey, phases: string[] = [];
    const status = { state: 'connected', origin: owner.origin, environmentId: owner.environmentId, message: '' };
    const adoptStatus = f.client.adoptStatus.bind(f.client);
    f.client.adoptStatus = (value, generation) => {
      phases.push(`adopt:${generation}:live:${f.client.generation}`); adoptStatus(value, generation);
    };
    f.native.later = async input => {
      const request = obj(input); phases.push(`${request.op}:live:${f.client.generation}`);
      if (request.op === 'forgetEnvironment') {
        const advance = (left: number) => queueMicrotask(() => {
          if (left > 1) advance(left - 1);
          else f.client.adoptStatus(status, 10);
        });
        advance(depth);
        return { ok: true, generation: 9, value: status };
      }
      return { ok: true, generation: f.client.generation, value: {} };
    };
    const result = await f.client.command('environment-forget', owner.origin, owner.environmentId, 0, f.native, nativeFiles(f.native));
    expect(result.message).toBe(''); expect(f.client.generation).toBe(10);
    expect(phases).toContain('sshForget:live:10');
    expect(phases).toContain('adopt:9:live:10');
    expect(phases.indexOf('adopt:10:live:9')).toBeLessThan(phases.indexOf('sshForget:live:10'));
    expect(mobileThreadComposer(f.client).editNoticeDismissKey).toBe(key);
    expect([...queuedEditState(f.client).notices.values()].map(notice => notice.message)).toEqual(['Keep stale generation error']);
  }
});

test('disable and route-only removal preserve notices, while successful removal protects a proven replacement catalog', async () => {
  const f = mobileFixture(), owner = queuedEditNoticeOwner(f.client)!;
  queuedEditSetNotice(f.client, 'Old error', owner);
  const key = mobileThreadComposer(f.client).editNoticeDismissKey;
  await f.client.command('environment-enabled', `${owner.origin}\n${owner.environmentId}`, 'off', 0, f.native, nativeFiles(f.native));
  await f.client.command('environment-route-remove', `${owner.origin}\n${owner.environmentId}`, 'old-route', 0, f.native, nativeFiles(f.native));
  expect(queuedEditState(f.client).notices.get(JSON.stringify([owner.origin, owner.environmentId, owner.threadId]))?.serial).toBe(JSON.parse(key)[3]);
  const before = fleet.saved; fleet.saved = [{ origin: owner.origin, environmentId: owner.environmentId }];
  try {
    f.hook(request => { if (request.op === 'forgetEnvironment') {
      const replacement = 'https://new-canonical.test'; fleet.saved = [{ origin: replacement, environmentId: owner.environmentId }];
      Object.assign(f.client, { origin: replacement, environmentId: owner.environmentId, threadId: owner.threadId });
      queuedEditSetNotice(f.client, 'Replacement error', queuedEditNoticeOwner(f.client));
    } });
    await f.client.command('environment-forget', owner.origin, owner.environmentId, 0, f.native, nativeFiles(f.native));
    expect([...queuedEditState(f.client).notices.values()].map(value => value.message)).toEqual(['Replacement error']);
  } finally { fleet.saved = before; }
});

test('actual mobile client keeps a failed Begin notice on its thread through selection and fresh typing', async () => {
  const f = mobileFixture();
  f.client.local.drafts['e:t'] = 'Original ordinary draft';
  f.hook(request => { if (request.op === 'mobileQueuedEdit' && request.action === 'cas') throw new Error('Queued editor could not be saved.'); });
  const row = mobileQueueSnapshot('visit', true, 0, f.client).rows[0]!;
  expect((await mobileQueuedEditBegin(row.actionId, f.native, f.client)).message).toBe('Queued editor could not be saved.');
  expect(mobileThreadComposer(f.client).editNotice).toBe('Queued editor could not be saved.');
  await f.select('other');
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  expect((await mobileDraftChanged(f.client, 'Fresh other draft', f.native, nativeFiles(f.native), mobileComposerTarget(f.client).owner)).message).toBe('');
  expect(f.client.draft).toBe('Fresh other draft');
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('Queued editor could not be saved.');
  expect(f.client.draft).toBe('Original ordinary draft');
  expect(f.sent).toHaveLength(0);
});

test('thread notices survive reconnect but do not follow a project draft or an equal thread ID in another environment', async () => {
  const f = mobileFixture();
  queuedEditSetNotice(f.client, 'The queued edit remains unconfirmed.', queuedEditNoticeOwner(f.client, 'm'));
  await f.client.openProjectDraft(f.native, 'p');
  expect(f.client.draftKey).toBe('e:new:p');
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t');
  const config = f.client.config, shell = f.client.shell;
  f.client.adoptStatus({ state: 'connected', origin: f.client.origin, environmentId: 'second', message: '' }, 10);
  f.client.config = config; f.client.shell = shell;
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  f.client.adoptStatus({ state: 'connected', origin: f.client.origin, environmentId: 'e', message: '' }, 11);
  f.client.config = config; f.client.shell = shell;
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('The queued edit remains unconfirmed.');
});

test('a failed awaited Begin stays on its captured thread without clobbering another composer', async () => {
  const f = mobileFixture(); let release!: () => void, entered!: () => void;
  const wait = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.hook(async request => { if (request.op === 'mobileQueuedEdit' && request.action === 'cas') {
    entered(); await wait; throw new Error('The original queued editor failed.');
  } });
  const row = mobileQueueSnapshot('visit', true, 0, f.client).rows[0]!;
  const pending = mobileQueuedEditBegin(row.actionId, f.native, f.client); await started;
  await f.select('other');
  await mobileDraftChanged(f.client, 'New text while the original save waits', f.native, nativeFiles(f.native), mobileComposerTarget(f.client).owner);
  release(); expect((await pending).message).toBe('The original queued editor failed.');
  expect(mobileThreadComposer(f.client)).toMatchObject({ editNotice: '', draft: 'New text while the original save waits' });
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('The original queued editor failed.');
  expect(mobileQueuedEditCurrent(f.client)).toBeNull(); expect(f.sent).toHaveLength(0);
});

test('awaited Begin supersession preserves prior notices without creating a notice on the destination', async () => {
  const f = mobileFixture(); let release!: () => void, entered!: () => void;
  const wait = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  queuedEditSetNotice(f.client, 'Earlier message error', queuedEditNoticeOwner(f.client, 'earlier'));
  f.hook(async request => { if (request.op === 'ids') { entered(); await wait; } });
  const row = mobileQueueSnapshot('visit', true, 0, f.client).rows[0]!;
  const pending = mobileQueuedEditBegin(row.actionId, f.native, f.client); await started;
  await f.select('other'); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('Earlier message error');
  expect(f.records.size).toBe(0); expect(f.sent).toHaveLength(0);
});

for (const action of ['save', 'cancel', 'attachment'] as const) test(`${action} failures belong to the captured queued editor`, async () => {
  const f = mobileFixture(), edit = await f.begin();
  f.client.local.drafts['e:t'] = 'Ordinary text';
  f.hook(request => {
    if (action === 'save' && request.op === 'mobileQueuedEdit' && request.action === 'cas'
      || action === 'cancel' && request.op === 'mobileQueuedEdit' && request.action === 'cleanup'
      || action === 'attachment' && request.op === 'composerAttachPick') throw new Error(`${action} could not finish.`);
  });
  const reply = action === 'save' ? await mobileQueuedEditSave(edit.owner, f.native, f.client)
    : action === 'cancel' ? await mobileQueuedEditCancel(edit.owner, f.native, f.client)
      : await mobileQueuedEditAttachmentAction('photos', '', edit.owner, f.native, f.client);
  expect(reply.message).toBe(`${action} could not finish.`);
  expect(mobileThreadComposer(f.client).editNotice).toBe(reply.message);
  await f.select('other'); expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t'); expect(mobileThreadComposer(f.client).editNotice).toBe(reply.message);
  expect(mobileQueuedEditCurrent(f.client)?.owner).toBe(edit.owner);
  expect(f.client.draft).toBe('Ordinary text'); expect(f.sent).toHaveLength(0);
});

test('opening and cancelling a different queued message cannot clear the original message notice', async () => {
  const f = mobileFixture();
  f.hook(request => { if (request.op === 'mobileQueuedEdit' && request.action === 'cas') throw new Error('First queued message failed.'); });
  const row = mobileQueueSnapshot('visit', true, 0, f.client).rows[0]!;
  await mobileQueuedEditBegin(row.actionId, f.native, f.client); f.hook(undefined);
  (f.client.projection.runs as Obj[]).push({ id: 'q2', status: 'queued', ordinal: 2, queuePosition: 2, userMessageId: 'm2' });
  (f.client.projection.messages as Obj[]).push({ id: 'm2', role: 'user', text: 'Other message' });
  const other = mobileQueueSnapshot('visit', true, 0, f.client).rows.find(row => JSON.parse(row.actionId).runId === 'q2')!;
  expect((await mobileQueuedEditBegin(other.actionId, f.native, f.client)).message).toBe('');
  expect(mobileThreadComposer(f.client).editNotice).toBe('First queued message failed.');
  expect((await mobileQueuedEditCancel(mobileQueuedEditCurrent(f.client)!.owner, f.native, f.client)).cancelled).toBe(true);
  expect(mobileThreadComposer(f.client).editNotice).toBe('First queued message failed.');
  expect((await mobileQueuedEditBegin(row.actionId, f.native, f.client)).message).toBe('');
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
});

for (const newer of [false, true]) for (const memory of [false, true]) test(`background acknowledgment clears only its own message notice, newer=${newer}, memory=${memory}`, async () => {
  const f = mobileFixture(), edit = await f.begin(); f.uncertain(true);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).message).toBe('lost reply');
  const operation = [...f.operations.values()][0]!; operation.state = 'acknowledged';
  if (newer) queuedEditSetNotice(f.client, 'A newer queued message failed.', queuedEditNoticeOwner(f.client, 'm2'));
  if (!memory) { queuedEditState(f.client).sessions.clear(); queuedEditState(f.client).active.clear(); }
  await f.select('other'); await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe(newer ? 'A newer queued message failed.' : '');
  expect(f.operations.size).toBe(0); expect(f.records.size).toBe(0); expect(f.sent).toHaveLength(1);
});

test('an explicit Retry failure stays with its queued operation and preserves its uncertain ownership', async () => {
  const f = mobileFixture(), edit = await f.begin(); f.uncertain(true);
  await mobileQueuedEditSave(edit.owner, f.native, f.client);
  const operation = mobileQueuedEditPresentation(f.client).pendingId; f.grants(false);
  expect((await mobileQueuedEditRetry(operation, f.native, f.client)).message).toBe('This connection cannot save queued messages.');
  await f.select('other'); expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t'); expect(mobileThreadComposer(f.client).editNotice).toBe('This connection cannot save queued messages.');
  expect(mobileQueuedEditPresentation(f.client)).toMatchObject({ pendingId: operation, uncertain: true });
  expect(f.operations.size).toBe(1); expect(f.sent).toHaveLength(1);
});

test('global journal failure remains visible on another thread and after a successful scoped Begin', async () => {
  const f = mobileFixture();
  f.hook(request => { if (request.op === 'mobileQueuedEdit' && request.action === 'read') throw new Error('The queued journal could not be read.'); });
  expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe('The queued journal could not be read.');
  await f.select('other');
  expect(mobileThreadComposer(f.client).editNotice).toBe('The queued journal could not be read.');
  f.hook(undefined); await f.begin();
  expect(mobileThreadComposer(f.client).editNotice).toBe('The queued journal could not be read.');
  await f.select('t'); expect(mobileThreadComposer(f.client).editNotice).toBe('The queued journal could not be read.');
});

for (const phase of ['read', 'release'] as const) test(`a healthy full Refresh retires its transient global ${phase} error and preserves scoped notices`, async () => {
  const f = mobileFixture(), original = f.native.later;
  f.native.later = async input => {
    const reply = await original(input), request = obj(input);
    return phase === 'release' && request.op === 'mobileQueuedEdit' && request.action === 'read'
      ? { ...obj(reply), value: { ...obj(obj(reply).value), releases: [{ kind: 'image', id: 'pending-release' }] } } : reply;
  };
  f.hook(request => { if (request.op === 'mobileQueuedEdit' && request.action === phase) throw new Error(`Transient journal ${phase} failure`); });
  expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe(`Transient journal ${phase} failure`);
  queuedEditSetNotice(f.client, 'An unrelated thread message remains unconfirmed.', queuedEditNoticeOwner(f.client, 'm'));
  expect(mobileThreadComposer(f.client).editNotice).toBe(`Transient journal ${phase} failure`);
  await f.select('other'); expect(mobileThreadComposer(f.client).editNotice).toBe(`Transient journal ${phase} failure`);
  f.hook(undefined); await f.begin();
  expect(mobileThreadComposer(f.client).editNotice).toBe(`Transient journal ${phase} failure`);
  expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe('');
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('t');
  expect(mobileThreadComposer(f.client).editNotice).toBe('An unrelated thread message remains unconfirmed.');
  expect(f.sent).toHaveLength(0);
});

test('an awaited healthy Refresh cannot clear a newer global report with identical text', async () => {
  const f = mobileFixture(); let release!: () => void, entered!: () => void;
  const wait = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  queuedEditSetNotice(f.client, 'Journal is unavailable', null);
  f.hook(async request => { if (request.op === 'mobileQueuedEdit' && request.action === 'read') { entered(); await wait; } });
  const pending = mobileQueuedEditRefresh(f.native, f.client); await started;
  queuedEditSetNotice(f.client, 'Journal is unavailable', null);
  release(); expect((await pending).message).toBe('');
  expect(mobileThreadComposer(f.client).editNotice).toBe('Journal is unavailable');
  f.hook(undefined); await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
});

test('a known orphan cleanup failure follows its saved thread rather than the current thread', async () => {
  const f = mobileFixture();
  f.records.set('orphan', { owner: 'orphan', revision: 1, origin: f.client.origin, environmentId: 'e', threadId: 'other', messageId: 'm-other' });
  f.hook(request => { if (request.op === 'mobileQueuedEdit' && request.action === 'cleanup') throw new Error('The other thread could not be cleaned.'); });
  await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  await f.select('other'); expect(mobileThreadComposer(f.client).editNotice).toBe('The other thread could not be cleaned.');
  await f.select('t'); expect(mobileThreadComposer(f.client).editNotice).toBe('');
  expect(f.records.has('orphan')).toBe(true);
});

test('a confirmed same-home route failover retains notices and an unrelated canonical server does not', async () => {
  const f = mobileFixture(); f.home('https://canonical.test'); await queuedEditRefreshOrigin(f.native, f.client);
  queuedEditSetNotice(f.client, 'Keep this queued message error.', queuedEditNoticeOwner(f.client, 'm'));
  f.client.origin = 'https://alternate-route.test'; f.client.generation++;
  await queuedEditRefreshOrigin(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('Keep this queued message error.');
  f.home('https://unrelated.test'); await queuedEditRefreshOrigin(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('');
  f.home('https://canonical.test'); await queuedEditRefreshOrigin(f.native, f.client);
  expect(mobileThreadComposer(f.client).editNotice).toBe('Keep this queued message error.');
});

test('Begin/Edit/Cancel use unique dedicated sessions and leave ordinary content unchanged', async () => {
  const f = fixture(); f.client.local.drafts['e:t'] = 'Ordinary'; f.client.local.snapshotDrafts['e:t'] = [{ id: 'ordinary-image' }];
  const first = await f.begin(); expect(first.draftKey).toBe('e:t~queued-edit~q');
  expect(mobileQueuedEditWriteText(first.owner, 'Changed', f.client)).toBe(true); await mobileQueuedEditPersist(first.owner, f.native, f.client);
  expect((await mobileQueuedEditCancel(first.owner, f.native, f.client)).cancelled).toBe(true);
  expect(mobileQueuedEditWriteText(first.owner, 'Late voice', f.client)).toBe(false);
  const second = await f.begin(); expect(second.owner).not.toBe(first.owner); expect(second.text).toBe('Original');
  expect(f.client.draft).toBe('Ordinary'); expect(f.client.snapshotDrafts).toHaveLength(1);
});
test('successful queued Save never invokes shared ordinary finishPending even when text and image IDs match', async () => {
  const f = fixture(), edit = await f.begin(); f.client.local.drafts['e:t'] = 'Original'; f.client.local.snapshotDrafts['e:t'] = [{ id: 'local', uploadId: 'server-image' }];
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).saved).toBe(true);
  expect(f.sent[0]).toMatchObject({ type: 'queued-run.edit', threadId: 't', runId: 'q', text: 'Original', attachments: [{ id: 'server-image' }] });
  expect(f.client.draft).toBe('Original'); expect(f.client.snapshotDrafts).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  expect(mobileQueuedEditCurrent(f.client)).toBeNull(); expect(f.records.size).toBe(0);
});
test('empty text rejects attachment-only edit and fresh permission denial sends nothing', async () => {
  const f = fixture(), edit = await f.begin(); mobileQueuedEditWriteText(edit.owner, ' ', f.client);
  expect(mobileQueuedEditPresentation(f.client).canSave).toBe(false);
  expect(await mobileQueuedEditSave(edit.owner, f.native, f.client)).toMatchObject({ saved: false, errorTitle: 'Add a message' });
  mobileQueuedEditWriteText(edit.owner, 'Changed', f.client); f.grants(false);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).message).toContain('cannot save'); expect(f.sent).toHaveLength(0);
});
test('uncertain Save remains durable; Retry sends exact immutable payload and releases only after acknowledgment', async () => {
  const f = fixture(), edit = await f.begin(); f.uncertain(true);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).saved).toBe(false);
  const pending = mobileQueuedEditPresentation(f.client); expect(pending.uncertain).toBe(true); expect(pending.canCancel).toBe(false);
  f.uncertain(false); expect((await mobileQueuedEditRetry(pending.pendingId, f.native, f.client)).saved).toBe(true);
  expect(f.sent).toHaveLength(2); expect(f.sent[1]).toEqual(f.sent[0]); expect(f.operations.size).toBe(0);
});
test('definitive rejection retires old intent without ending editor; another Save has a new command ID', async () => {
  const f = fixture(), edit = await f.begin(); f.rejected(true); await mobileQueuedEditSave(edit.owner, f.native, f.client);
  expect(f.operations.size).toBe(0); expect(mobileQueuedEditCurrent(f.client)?.owner).toBe(edit.owner);
  f.rejected(false); mobileQueuedEditWriteText(edit.owner, 'Second edit', f.client);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).saved).toBe(true);
  expect(f.sent[0]?.commandId).not.toBe(f.sent[1]?.commandId);
});
test('Cancel locks synchronous writes while native cleanup is pending', async () => {
  const f = fixture(), edit = await f.begin(); let release!: () => void, entered!: () => void;
  const wait = new Promise<void>(r => { release = r; }), started = new Promise<void>(r => { entered = r; });
  f.hook(async request => { if (request.action === 'cleanup') { entered(); await wait; } });
  const pending = mobileQueuedEditCancel(edit.owner, f.native, f.client); await started;
  expect(mobileQueuedEditWriteText(edit.owner, 'Must not vanish', f.client)).toBe(false); release(); expect((await pending).cancelled).toBe(true);
});
test('owner change after reservation retires known-unsent intent without writing or losing edit', async () => {
  const f = fixture(), edit = await f.begin(); f.hook(request => { if (request.action === 'reserve') f.client.threadId = 'other'; });
  await expect(mobileQueuedEditSave(edit.owner, f.native, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.sent).toHaveLength(0); expect(f.operations.size).toBe(0); expect(mobileQueuedEditLookup(edit.owner, f.client)?.text).toBe('Original');
});
test('run-loss recovery marks native intent before ordinary persistence, keeps context, and discards kept server attachment refs', async () => {
  const f = fixture(), edit = await f.begin(); mobileQueuedEditWriteText(edit.owner, 'Recovered text', f.client);
  f.client.projection.runs = []; await mobileQueuedEditRefresh(f.native, f.client);
  expect(f.client.draft).toBe('Recovered text'); expect(f.client.snapshotDrafts).toHaveLength(0); expect(mobileQueuedEditCurrent(f.client)).toBeNull();
  const marked = f.calls.findIndex(call => call.action === 'cas' && obj(call.record).recoveryKey === 'e:t');
  const written = f.calls.findIndex(call => call.op === 'writePreferences'); const cleaned = f.calls.findIndex(call => call.action === 'cleanup');
  expect(marked).toBeGreaterThan(-1); expect(written).toBeGreaterThan(marked); expect(cleaned).toBeGreaterThan(written);
});
test('failed ordinary persistence retains native recovery and refresh never overwrites another ordinary draft', async () => {
  const f = fixture(), edit = await f.begin(); f.client.projection.runs = []; f.failPersist(true); await mobileQueuedEditRefresh(f.native, f.client);
  expect(f.records.get(edit.owner)?.recoveryKey).toBe('e:t'); expect(f.operations.size).toBe(0);
  f.failPersist(false); await mobileQueuedEditRefresh(f.native, f.client); expect(f.records.size).toBe(0);
  const other = fixture(), old = await other.begin(); other.client.local.drafts['e:t'] = 'Ordinary'; other.client.projection.runs = [];
  await mobileQueuedEditRefresh(other.native, other.client); expect(other.client.draft).toBe('Ordinary'); expect(mobileQueuedEditLookup(old.owner, other.client)).toBeNull();
});
test('payload retains order, rebinds new attachment context IDs, and prunes removed retained records', async () => {
  const f = fixture(), edit = await f.begin();
  edit.attachments = [{ id: 'local', kind: 'image', name: 'a.jpg', mimeType: 'image/jpeg', sizeBytes: 3, uploadId: '', status: 'staged' }];
  edit.context = { version: 1, records: [{ contextId: 'one', kind: 'image', attachmentId: 'local' }, { contextId: 'two', kind: 'image', attachmentId: 'removed' }] };
  const resolved = queuedEditResolvePayload(edit, [{ id: 'new-server', type: 'image' }]);
  expect(resolved.attachments.map(file => file.id)).toEqual(['server-image', 'new-server']);
  expect(resolved.context?.records).toEqual([{ contextId: 'one', kind: 'image', attachmentId: 'new-server' }]);
});
test('real upload uses JPEG content type and dedicated CAS ownership before edit wire payload', async () => {
  const f = fixture(), edit = await f.begin(); queuedEditReplaceAttachments(edit.owner, [{ id: '00000000-0000-4000-8000-999999999999', kind: 'image', name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 3, uploadId: '', status: 'staged' }], edit.existingAttachments, f.client);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).saved).toBe(true);
  expect(f.calls.find(call => call.op === 'uploadAttachment')?.contentType).toBe('image/jpeg');
  expect(obj((f.sent[0]?.attachments as Obj[])[1]).mimeType).toBe('image/jpeg');
});
test('successful queue edit marks same-visit dismissal and fresh visits do not inherit it', async () => {
  const f = fixture(), view = mobileQueueSnapshot('visit', true, 0, f.client);
  await mobileQueueCommand(['queue:edit', view.rows[0]!.actionId], f.native, { fs: { async mkdir() {}, async atomicWriteFile() {}, async readFile() { return new Uint8Array(); } } }, f.client);
  expect(mobileQueueSnapshot('visit', true, 0, f.client).dismiss).toBe(true);
  expect(mobileQueueSnapshot('next', true, 0, f.client).dismiss).toBe(false);
});

test('uncertain journal freezes content writes while keeping explicit Retry enabled', async () => {
  const f = fixture(), edit = await f.begin(); f.uncertain(true); await mobileQueuedEditSave(edit.owner, f.native, f.client);
  expect(mobileQueuedEditWriteText(edit.owner, 'Unpersistable change', f.client)).toBe(false);
  expect(mobileQueuedEditPresentation(f.client)).toMatchObject({ saving: true, canRetry: true, canSave: false, canCancel: false });
});
test('recovery hydrates the durable marker after process loss without reopening the editing session', async () => {
  const f = fixture(), edit = await f.begin(); f.client.projection.runs = []; f.failPersist(true); await mobileQueuedEditRefresh(f.native, f.client);
  const state = queuedEditState(f.client); state.sessions.clear(); state.active.clear(); state.recoveries.clear();
  f.failPersist(false); await mobileQueuedEditRefresh(f.native, f.client);
  expect(f.client.draft).toBe('Original'); expect(mobileQueuedEditCurrent(f.client)).toBeNull(); expect(f.records.has(edit.owner)).toBe(false);
});
test('foreign shared pending installed during IDs prevents queue reservation and preserves the foreign owner', async () => {
  const f = fixture(), edit = await f.begin(), foreign = { method: 'orchestration.dispatchCommand', payload: { commandId: 'foreign' } };
  f.hook(request => { if (request.op === 'ids') f.client.local.pending.e = foreign; });
  await expect(mobileQueuedEditSave(edit.owner, f.native, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.pending).toBe(foreign); expect(f.sent).toHaveLength(0); expect(f.operations.size).toBe(0);
});
test('picker uses unique editor owner, inserts context at captured caret and stages mixed attachment order', async () => {
  const f = fixture(), edit = await f.begin(); const image = '00000000-0000-4000-8000-000000000101', file = '00000000-0000-4000-8000-000000000102';
  f.picked([{ id: image, kind: 'image', name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 3 }, { id: file, kind: 'file', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 3 }]);
  expect((await mobileQueuedEditAttachmentAction('files', '', edit.owner, f.native, f.client)).message).toBe('');
  const updated = mobileQueuedEditCurrent(f.client)!; expect(updated.attachments.map(file => file.id)).toEqual([image, file]);
  expect(updated.text).toContain('Original ![photo.jpg]');
  expect(f.calls.find(call => call.action === 'selection')).toMatchObject({ owner: edit.owner, optional: true });
  expect(f.calls.find(call => call.action === 'selection-commit')?.owner).toBe(edit.owner);
  mobileQueuedEditWriteText(edit.owner, 'Original', f.client);
  expect(mobileQueuedEditCurrent(f.client)?.attachments.map(file => file.id)).toEqual([image]);
  expect(mobileQueuedEditCurrent(f.client)?.context).toBeUndefined();
});
test('picker completion after cancelled session cannot attach to a reopened queue editor and releases unadopted bytes', async () => {
  const f = fixture(), edit = await f.begin(), image = '00000000-0000-4000-8000-000000000103';
  f.picked([{ id: image, kind: 'image', name: 'a.png', mimeType: 'image/png', sizeBytes: 3 }]);
  f.hook(async request => { if (request.op === 'composerAttachPick') await mobileQueuedEditCancel(edit.owner, f.native, f.client); });
  await expect(mobileQueuedEditAttachmentAction('photos', '', edit.owner, f.native, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.op === 'snapshotDraftRemove' && call.id === image)).toBe(true); expect(mobileQueuedEditCurrent(f.client)).toBeNull();
});

test('verified same-home route failover keeps dedicated edit selected; a different canonical server does not', async () => {
  const f = fixture(), edit = await f.begin(); f.client.origin = 'https://fallback.test'; f.client.generation++;
  await queuedEditRefreshOrigin(f.native, f.client); expect(mobileQueuedEditOrigin(f.client)).toBe('https://example.test');
  expect(mobileQueuedEditCurrent(f.client)?.owner).toBe(edit.owner);
  f.home('https://unrelated.test'); await queuedEditRefreshOrigin(f.native, f.client); expect(mobileQueuedEditCurrent(f.client)).toBeNull();
});
test('read snapshot removes a locally cached operation which native has already retired', async () => {
  const f = fixture(), edit = await f.begin(); f.uncertain(true); await mobileQueuedEditSave(edit.owner, f.native, f.client);
  expect(mobileQueuedEditPresentation(f.client).uncertain).toBe(true);
  f.operations.clear(); await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileQueuedEditPresentation(f.client).uncertain).toBe(false); expect(queuedEditState(f.client).operations.size).toBe(0);
});
test('rejected operation does not block run-loss recovery on refresh', async () => {
  const f = fixture(), edit = await f.begin(); f.uncertain(true); await mobileQueuedEditSave(edit.owner, f.native, f.client);
  const operation = [...f.operations.values()][0]!; operation.state = 'rejected'; operation.revision = Number(operation.revision) + 1;
  f.client.projection.runs = []; await mobileQueuedEditRefresh(f.native, f.client);
  expect(f.client.draft).toBe('Original'); expect(mobileQueuedEditCurrent(f.client)).toBeNull(); expect(f.operations.size).toBe(0);
});

test('acknowledgment recovered from native read cleans its captured record once without stale cleanup notice', async () => {
  const f = fixture(), edit = await f.begin(); f.uncertain(true); await mobileQueuedEditSave(edit.owner, f.native, f.client);
  const operation = [...f.operations.values()][0]!; operation.state = 'acknowledged'; operation.revision = Number(operation.revision) + 1;
  queuedEditState(f.client).sessions.clear(); queuedEditState(f.client).active.clear();
  const response = await mobileQueuedEditRefresh(f.native, f.client);
  expect(response.message).toBe(''); expect(f.records.size).toBe(0); expect(f.operations.size).toBe(0);
  expect(f.calls.filter(call => call.action === 'cleanup')).toHaveLength(1);
});


test('queued edit preserves a non-catalog Codex selection through durable save', async () => {
  const f = fixture(), edit = await f.begin();
  Object.assign((f.client.config.providers as Obj[])[0]!, { driver: 'codex', auth: { status: 'authenticated' }, status: 'ready', models: [{ slug: 'gpt-6.1-sol' }] });
  f.client.modelId = 'gpt-5.4';
  expect(mobileQueuedEditPresentation(f.client).canSave).toBe(true);
  expect((await mobileQueuedEditSave(edit.owner, f.native, f.client)).saved).toBe(true);
  expect(f.sent[0]).toMatchObject({ type: 'queued-run.edit', threadId: 't', runId: 'q', text: 'Original' });
  expect(f.client.modelId).toBe('gpt-5.4'); expect(f.operations.size).toBe(0);
});
test('queued edit refuses Antigravity or lost provider auth before attachment or journal work', async () => {
  for (const patch of [{ driver: 'antigravity' }, { driver: 'codex', auth: { status: 'unauthenticated' } }]) {
    const f = fixture(), edit = await f.begin();
    Object.assign((f.client.config.providers as Obj[])[0]!, { status: 'ready', models: [], ...patch });
    f.client.modelId = 'gpt-5.4'; f.calls.length = 0;
    expect(mobileQueuedEditPresentation(f.client).canSave).toBe(false);
    await expect(mobileQueuedEditSave(edit.owner, f.native, f.client)).rejects.toThrow('Model unavailable. Open model settings.');
    expect(f.calls).toHaveLength(0); expect(f.sent).toHaveLength(0);
  }
});


test('a newer native status during startup hydration is superseded without a composer notice', async () => {
  const f = fixture(), original = f.native.later;
  f.client.generation = 0; f.client.environmentId = ''; f.client.connection = 'disconnected';
  f.native.later = async input => obj(input).op === 'status'
    ? { ok: true, generation: 1, value: { origin: f.client.origin, environmentId: 'e', homeOrigin: f.client.origin } }
    : original(input);
  await expect(mobileQueuedEditRefresh(f.native, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileQueuedEditPresentation(f.client).error).toBe('');
  expect(f.calls.some(call => call.op === 'mobileQueuedEdit')).toBe(false);
  f.client.generation = 1; f.client.environmentId = 'e'; f.client.connection = 'connected';
  for (let i = 0; i < 3; i++) expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe('');
  expect(mobileQueuedEditPresentation(f.client).error).toBe('');
  expect(mobileQueuedEditPresentation(f.client).editing).toBe(false);
});

test('superseded origin reads and later healthy hydration preserve a prior queued-write notice', async () => {
  const f = fixture(), original = f.native.later;
  queuedEditSetNotice(f.client, 'The server has not confirmed the queued edit.', queuedEditNoticeOwner(f.client, 'm'));
  f.native.later = async input => obj(input).op === 'status'
    ? { ok: true, generation: f.client.generation + 1, value: { origin: f.client.origin, environmentId: f.client.environmentId } }
    : original(input);
  await expect(mobileQueuedEditRefresh(f.native, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileQueuedEditPresentation(f.client).error).toBe('The server has not confirmed the queued edit.');
  f.native.later = original;
  await mobileQueuedEditRefresh(f.native, f.client);
  expect(mobileQueuedEditPresentation(f.client).error).toBe('The server has not confirmed the queued edit.');
});

test('current origin-read failure remains visible instead of being treated as a generation race', async () => {
  const f = fixture(), original = f.native.later;
  f.native.later = async input => obj(input).op === 'status'
    ? { ok: false, generation: f.client.generation, error: { kind: 'Persistence', message: 'Saved origin could not be read.', uncertain: false } }
    : original(input);
  expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe('Saved origin could not be read.');
  expect(mobileQueuedEditPresentation(f.client).error).toBe('Saved origin could not be read.');
});

test('stale durable cleanup after a valid origin read remains an actionable queued notice', async () => {
  const f = fixture();
  f.records.set('orphan', { owner: 'orphan', revision: 1 });
  f.hook(input => { if (input.op === 'mobileQueuedEdit' && input.action === 'cleanup') throw new ClientError('The editor changed before cleanup.', 'stale'); });
  expect((await mobileQueuedEditRefresh(f.native, f.client)).message).toBe('The editor changed before cleanup.');
  expect(mobileQueuedEditPresentation(f.client).error).toBe('The editor changed before cleanup.');
  expect(f.records.has('orphan')).toBe(true);
});
