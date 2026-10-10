import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { mobileQueueSnapshot, mobileQueueCommand } from './queue';
import { mobileQueuedEditBegin, mobileQueuedEditCancel, mobileQueuedEditSave, mobileQueuedEditRetry, mobileQueuedEditRefresh, mobileQueuedEditPresentation } from './queued-edit';
import { mobileQueuedEditCurrent, mobileQueuedEditLookup, mobileQueuedEditPersist, mobileQueuedEditWriteText, queuedEditState, queuedEditReplaceAttachments } from './queued-edit-state';
import { queuedEditResolvePayload } from './queued-edit-upload';
import { queuedEditRefreshOrigin, mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileQueuedEditAttachmentAction } from './queued-edit-attachments';
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fixture() {
  const client = new T3Client(), records = new Map<string, Obj>(), operations = new Map<string, Obj>(), calls: Obj[] = [], sent: Obj[] = [];
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
        if (client.pending) return { ok: false, error: { kind: 'busy', message: 'Another write owns this environment.' } };
        const record = records.get(owner)!;
        const operation = { operationId, owner, editorRevision: request.editorRevision, revision: 1, origin: home, environmentId: record.environmentId,
          state: 'reserved', method: request.method, payload: clone(request.payload) };
        operations.set(operationId, operation); return ok(operation);
      }
      if (request.action === 'send') {
        const operation = operations.get(operationId)!;
        operation.state = 'issued'; operation.revision = Number(operation.revision) + 1; sent.push(clone(obj(operation.payload)));
        operation.state = uncertain ? 'uncertain' : rejected ? 'rejected' : 'acknowledged'; operation.revision = Number(operation.revision) + 1;
        if (uncertain || rejected) return { ok: false, error: { kind: 'server', message: uncertain ? 'lost reply' : 'run changed', uncertain } };
        return ok({ operation, result: {} });
      }
      if (request.action === 'retire') { const op = operations.get(operationId)!; if (['issued', 'uncertain'].includes(String(op.state))) throw new Error('unresolved'); operations.delete(operationId); return ok({ removed: true }); }
      if (request.action === 'cleanup') { if (!records.has(owner) || records.get(owner)?.revision !== request.editorRevision) return { ok: false, error: { kind: 'stale', message: 'The editor changed before cleanup.' } }; records.delete(owner); for (const [id, op] of operations) if (op.owner === owner) operations.delete(id); return ok({}); }
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
  queuedEditState(f.client).notice = 'The server has not confirmed the queued edit.';
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
