import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftUnbind as unbind,
  mobileNewTaskDraftCurrent as current, mobileNewTaskDraftLookup as lookup, mobileNewTaskDraftList as list,
  mobileNewTaskDraftChanged as changed, mobileNewTaskDraftRetarget as retarget, mobileNewTaskDraftDiscard as discard,
  mobileNewTaskDraftStore as store, mobileNewTaskDraftPresentation as presentation, mobileNewTaskDraftChoicesCapture as choices } from './mobile-new-task-drafts';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import type { Pending } from './shared/client';
import type { Native, Files } from './shared/protocol';
const A = 'new-task:A', B = 'new-task:B', origin = 'https://draft.test';
const imageA = { id: '11111111-1111-4111-8111-111111111111', name: 'a.jpg', mimeType: 'image/jpeg', sizeBytes: 42, uploadId: 'upload-a' };
const imageB = { ...imageA, id: '22222222-2222-4222-8222-222222222222' };
function fixture(document: Obj = { version: 1 }) {
  const client = new MobileDraftClient(); let disk = JSON.stringify(document), fail = false;
  const writes: Obj[] = [], calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    if (fail) throw new Error('disk failure'); disk = new TextDecoder().decode(bytes); writes.push(obj(JSON.parse(disk)));
  } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const call = obj(input); calls.push(call);
    return { ok: true, generation: 1, value: call.op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const load = async () => {
    const handles = mobileDraftRecoveryHandles(client, native, storage);
    await client.refresh(handles.native, handles.storage);
    Object.assign(client, { environmentId: 'env', origin, projectId: 'project', threadId: '', generation: 1 });
  };
  return { client, native, storage, calls, writes, load, disk: () => obj(JSON.parse(disk)), fail(value = true) { fail = value; } };
}
async function populated() {
  const f = fixture(); await f.load();
  for (const id of ['A', 'B']) create(f.client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: `2026-10-08T00:00:0${id === 'A' ? 1 : 2}.000Z` });
  f.client.local.drafts[A] = 'same'; f.client.local.drafts[B] = 'same'; f.client.local.drafts['env:new:project'] = 'same';
  f.client.local.snapshotDrafts[A] = [{ ...imageA }]; f.client.local.snapshotDrafts[B] = [{ ...imageB }];
  f.client.local.snapshotDrafts['env:new:project'] = [{ ...imageA }]; bind(f.client, A, 'flow-A'); f.calls.length = 0; f.writes.length = 0;
  return f;
}
function pending(commandId = 'command-A'): Pending {
  return { method: 'orchestration.launchThread', payload: { commandId, projectId: 'project', threadId: 'thread-A', initialMessage: { text: 'same', attachments: [{ id: 'upload-a' }] } },
    description: 'Create thread', threadId: 'thread-A', text: 'same', uncertain: false };
}
function file(id: string, key = A): DraftFile {
  return { id, draftKey: key, environmentId: 'env', contextId: `file-${id}`, name: `${id}.txt`, mimeType: 'text/plain', sizeBytes: 1,
    source: 'attached', attachmentId: `upload-${id}`, status: 'ready' };
}
const slot = JSON.stringify(['env', 'command-A']);

test('new identities require hydration and unique supplied IDs, with content-only persistence and fresh bindings', async () => {
  const f = fixture(); expect(() => create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08' })).toThrow();
  await f.load(); create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08' });
  expect(() => create(f.client, { id: 'A', origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08' })).toThrow();
  expect(bind(f.client, A, 'one')).toBe(true); expect(f.client.draftKey).toBe(A);
  expect(list(f.client)).toEqual([]); await f.client.persist(f.storage);
  expect(obj(obj(f.disk().mobileNewTaskDrafts).records)[A]).toBeUndefined();
  f.client.local.drafts[A] = 'nonempty'; changed(f.client, A, true); await f.client.persist(f.storage);
  const restart = fixture(f.disk()); await restart.load(); expect(lookup(restart.client, A)?.revision).toBe(1);
  expect(current(restart.client)).toBeNull(); expect(bind(restart.client, A, 'two')).toBe(true);
  expect(restart.client.draft).toBe('nonempty'); expect(lookup(restart.client, A)?.choices).toEqual(choices(f.client));
});
test('actual load hydrates JPEG images, files, choices and metadata before first presentation', async () => {
  const f = await populated(); setDraftFiles(f.client.local, [file('f')]); changed(f.client, A, true); await f.client.persist(f.storage);
  const r = fixture(f.disk()), original = r.native.later; let seen = false;
  r.native.later = async input => { if (obj(input).op === 'devicePresentation') seen = !!lookup(r.client, A) && r.client.local.snapshotDrafts[A]?.[0]?.mimeType === 'image/jpeg'; return original(input); };
  await r.load(); expect(seen).toBe(true); expect(presentation(r.client, A)?.files[0]).toEqual(file('f'));
  expect(list(r.client).map(record => record.key)).toEqual([B, A]);
  r.client.local.drafts[A] = 'new'; await r.load(); expect(r.client.local.drafts[A]).toBe('new');
});
test('bindings are client/flow owned and actual threads never use independent keys', async () => {
  const f = await populated(), other = await populated(); unbind(f.client, 'wrong'); expect(current(f.client)?.key).toBe(A);
  f.client.projectId = 'other'; expect(current(f.client)).toBeNull(); expect(f.client.draftKey).toBe(A);
  f.client.threadId = 'thread'; expect(f.client.draftKey).toBe('env:thread');
  unbind(f.client, 'flow-A'); expect(current(other.client)?.key).toBe(A);
});
test('ACK uses captured A despite B selection and preserves equal-text/upload project slots', async () => {
  const f = await populated(), original = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'request') {
      expect(obj(obj(f.writes[0]?.mobileNewTaskDrafts).receipts)[slot]).toBeDefined(); bind(f.client, B, 'flow-B');
    }
    return original(input);
  };
  await f.client.write(f.native, f.storage, pending());
  expect(lookup(f.client, A)).toBeNull(); expect(f.client.local.drafts[B]).toBe('same'); expect(f.client.local.drafts['env:new:project']).toBe('same');
  expect(f.client.local.snapshotDrafts['env:new:project']).toEqual([imageA]); expect(f.client.pending).toBeUndefined();
  expect(f.calls.filter(call => call.op === 'snapshotDraftRemove')).toHaveLength(0); // Same local bytes remain referenced by another draft.
  expect(Object.keys(store(f.client).receipts)).toEqual([]); expect(f.client.draftKey).toBe(B);
});
test('ABA edits and replaced/new attachment records survive captured cleanup', async () => {
  const f = await populated(); const original = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'request') {
      f.client.local.drafts[A] = 'changed'; changed(f.client, A); f.client.local.drafts[A] = 'same'; changed(f.client, A);
      f.client.local.snapshotDrafts[A] = [{ ...imageA, name: 'replaced.jpg' }, imageB];
    }
    return original(input);
  };
  await f.client.write(f.native, f.storage, pending());
  expect(f.client.local.drafts[A]).toBe('same'); expect(f.client.local.snapshotDrafts[A]).toHaveLength(2); expect(lookup(f.client, A)).not.toBeNull();
});
test('captured sent files release only after durable cleanup and preserve other draft references', async () => {
  const f = await populated(); setDraftFiles(f.client.local, [file('a'), file('b'), file('a', B)]);
  const p = pending(); obj(p.payload.initialMessage).attachments = [{ id: 'upload-a' }];
  await f.client.write(f.native, f.storage, p);
  expect(draftFiles(f.client.local)).toEqual([file('b'), file('a', B)]);
  expect(f.calls.filter(call => call.op === 'composerAttachRemove')).toHaveLength(0);
  setDraftFiles(f.client.local, [file('b')]); await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls.filter(call => call.op === 'composerAttachRemove').map(call => call.id)).toEqual(['a']);
});
test('uncertain restart retries the immutable A payload while B is bound', async () => {
  const f = await populated(); f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow('lost reply');
  expect(f.client.pending?.uncertain).toBe(true); expect(() => discard(f.client, A)).toThrow(); expect(() => retarget(f.client, A, { origin, environmentId: 'env', projectId: 'other' })).toThrow();
  const r = fixture(f.disk()); await r.load(); bind(r.client, B, 'next'); const saved = r.client.pending!;
  await r.client.write(r.native, r.storage, { ...saved, uncertain: false });
  expect(r.calls.find(call => call.op === 'request')?.payload).toEqual(saved.payload);
  expect(lookup(r.client, A)).toBeNull(); expect(r.client.local.drafts[B]).toBe('same');
});
test('uncertain restart reconciles the captured draft exactly once through real shared shell reflection', async () => {
  const f = await populated(); f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow();
  const r = fixture(f.disk()); await r.load(); bind(r.client, B, 'next'); r.client.shell.threads = [{ id: 'thread-A' }];
  expect(r.client.reconcilePending()).toBe(true); expect(r.client.reconcilePending()).toBe(false);
  await r.client.persist(r.storage); const again = fixture(r.disk()); await again.load(); expect(lookup(again.client, A)).toBeNull(); expect(lookup(again.client, B)).not.toBeNull();
});
test('initial save failure sends nothing and known refusal preserves content without a receipt lock', async () => {
  const f = await populated(); f.fail(); await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow('Could not save');
  expect(f.calls).toHaveLength(0); expect(store(f.client).claims).toEqual({}); f.fail(false);
  f.native.later = async () => ({ ok: false, generation: 1, error: { message: 'refused', kind: 'remote', uncertain: false } });
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow('refused');
  expect(f.client.local.drafts[A]).toBe('same'); expect(store(f.client).claims).toEqual({});
  const r = fixture(f.disk()); await r.load(); expect(discard(r.client, A)).toBe(true);
});
test('post-ACK save failure retains old durable pending and receipt and releases no bytes', async () => {
  const f = await populated(); delete f.client.local.snapshotDrafts['env:new:project']; setDraftFiles(f.client.local, [file('a')]);
  const original = f.native.later; f.native.later = async input => { if (obj(input).op === 'request') f.fail(); return original(input); };
  await f.client.write(f.native, f.storage, pending());
  expect(obj(f.disk().pending).env).toBeDefined(); expect(obj(obj(f.disk().mobileNewTaskDrafts).receipts)[slot]).toBeDefined();
  expect(f.calls.filter(call => ['snapshotDraftRemove', 'composerAttachRemove'].includes(String(call.op)))).toHaveLength(0);
  const r = fixture(f.disk()); await r.load(); r.client.shell.threads = [{ id: 'thread-A' }]; expect(r.client.reconcilePending()).toBe(true);
});
test('invalid, foreign and payload-mutated receipts hold pending and never hit legacy cleanup or dispatch', async () => {
  const f = await populated(); f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow();
  for (const corrupt of ['shape', 'origin', 'payload', 'missing']) {
    const document = f.disk(), saved = obj(obj(document.mobileNewTaskDrafts).receipts);
    if (corrupt === 'shape') saved[slot] = { broken: true };
    else if (corrupt === 'origin') obj(saved[slot]).origin = 'https://other.test';
    else if (corrupt === 'payload') obj(obj(obj(document.pending).env).payload).threadId = 'other-thread';
    else delete saved[slot];
    const r = fixture(document); await r.load(); r.calls.length = 0; r.client.shell.threads = [{ id: 'thread-A' }, { id: 'other-thread' }];
    expect(r.client.reconcilePending()).toBe(false); await expect(r.client.write(r.native, r.storage, { ...r.client.pending!, uncertain: false })).rejects.toThrow('cannot be verified');
    expect(r.client.pending).toBeDefined(); expect(r.client.local.drafts['env:new:project']).toBe('same'); expect(r.calls).toEqual([]);
  }
});
test('endpoint changes before request retain pending ownership and dispatch nothing', async () => {
  const f = await populated(), original = f.storage.fs.atomicWriteFile;
  f.storage.fs.atomicWriteFile = async (path, bytes) => { await original(path, bytes); f.client.generation++; };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow('owner changed');
  expect(f.calls).toEqual([]); expect(f.client.pending).toBeDefined(); expect(store(f.client).claims[slot]).toBe(A);
});
test('duplicate writes and premature shell reconciliation cannot steal an active launch', async () => {
  const f = await populated(); let release!: () => void;
  f.native.later = async () => { await new Promise<void>(resolve => { release = resolve; }); return { ok: true, generation: 1, value: {} }; };
  const running = f.client.write(f.native, f.storage, pending());
  for (let i = 0; i < 8 && !release; i++) await Promise.resolve();
  f.client.shell.threads = [{ id: 'thread-A' }]; expect(f.client.reconcilePending()).toBe(false);
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow('already in progress'); release(); await running;
  expect(lookup(f.client, A)).toBeNull();
});
test('pending bytes stay protected even after editor chip removal', async () => {
  const f = await populated(); delete f.client.local.snapshotDrafts['env:new:project']; f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow();
  f.client.local.snapshotDrafts[A] = []; f.client.local.snapshotReleases.push(imageA.id);
  let removals = 0; f.native.later = async input => { if (obj(input).op === 'snapshotDraftRemove') removals++; return { ok: true, generation: 1, value: {} }; };
  await f.client.flushSnapshotReleases(f.native, f.storage); expect(removals).toBe(0);
});
test('retarget retains content and choices, clears project context, and drops only remote upload identities across environments', async () => {
  const f = await populated(); setDraftFiles(f.client.local, [file('f')]); changed(f.client, A, true);
  f.client.local.composerControls.contexts[A] = { envMode: 'worktree', branch: 'branch', worktreePath: '/path' };
  expect(retarget(f.client, A, { origin, environmentId: 'env', projectId: 'other' })).toBe(true);
  expect(f.client.local.snapshotDrafts[A]?.[0]?.uploadId).toBe('upload-a'); expect(presentation(f.client, A)?.workspace).toBeNull();
  expect(retarget(f.client, A, { origin: 'https://other.test', environmentId: 'other', projectId: 'p' })).toBe(true);
  expect(f.client.local.snapshotDrafts[A]?.[0]?.uploadId).toBeUndefined(); expect(draftFiles(f.client.local)[0]).toMatchObject({ id: 'f', attachmentId: '', status: 'staged', environmentId: 'other' });
  expect(lookup(f.client, A)?.choices).toEqual(choices(f.client)); expect(f.client.local.drafts[A]).toBe('same');
});
test('discard clears only the owned draft and queues local bytes after caller persistence', async () => {
  const f = await populated(); setDraftFiles(f.client.local, [file('a'), file('b', B)]);
  expect(discard(f.client, A)).toBe(true); expect(lookup(f.client, A)).toBeNull(); expect(lookup(f.client, B)).not.toBeNull();
  expect(f.client.local.drafts['env:new:project']).toBe('same'); expect(draftFiles(f.client.local)).toEqual([file('b', B)]);
  expect(store(f.client).fileReleases).toEqual(['a']); expect(f.calls).toEqual([]);
});

test('receipt environment or pending command corruption remains held through ordinary persist and restart', async () => {
  const f = await populated(); f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow();
  for (const corrupt of ['receipt-environment', 'command', 'extension', 'null-fields']) {
    const document = f.disk();
    if (corrupt === 'receipt-environment') obj(obj(obj(document.mobileNewTaskDrafts).receipts)[slot]).environmentId = 'other';
    else if (corrupt === 'command') obj(obj(obj(document.pending).env).payload).commandId = 'changed-command';
    else if (corrupt === 'null-fields') { obj(document.mobileNewTaskDrafts).receipts = null; obj(document.mobileNewTaskDrafts).claims = null; }
    else document.mobileNewTaskDrafts = { version: 99 };
    const r = fixture(document); await r.load(); await r.client.persist(r.storage);
    const again = fixture(r.disk()); await again.load(); again.calls.length = 0; again.client.shell.threads = [{ id: 'thread-A' }];
    expect(again.client.reconcilePending()).toBe(false);
    await expect(again.client.write(again.native, again.storage, { ...again.client.pending!, uncertain: false })).rejects.toThrow('cannot be verified');
    expect(again.client.pending).toBeDefined(); expect(again.client.local.drafts['env:new:project']).toBe('same'); expect(again.calls).toEqual([]);
  }
});
test('cross-environment retarget refuses files whose local bytes are not durably attached', async () => {
  const f = await populated(); setDraftFiles(f.client.local, [{ ...file('paste'), source: 'paste' }]);
  const before = presentation(f.client, A);
  expect(() => retarget(f.client, A, { origin: 'https://other.test', environmentId: 'other', projectId: 'p' })).toThrow('local file bytes');
  expect(presentation(f.client, A)).toEqual(before);
});


test('fresh and retried launch cleanup requires the exact text/body and thread identity', async () => {
  for (const field of ['text', 'thread']) {
    const f = await populated(), p = pending();
    if (field === 'text') obj(p.payload.initialMessage).text = 'other'; else p.threadId = 'other';
    await expect(f.client.write(f.native, f.storage, p)).rejects.toThrow('cannot be verified');
    expect(f.calls).toEqual([]); expect(f.client.local.drafts[A]).toBe('same');
  }
  const f = await populated(); f.native.later = async () => { throw new Error('lost reply'); };
  await expect(f.client.write(f.native, f.storage, pending())).rejects.toThrow();
  const document = f.disk(); obj(obj(document.pending).env).threadId = 'other';
  const r = fixture(document); await r.load(); r.client.shell.threads = [{ id: 'thread-A' }];
  expect(r.client.reconcilePending()).toBe(false); expect(r.client.local.drafts[A]).toBe('same');
});
test('a late ACK cannot remove a newer pending operation', async () => {
  const f = await populated(), original = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'request') f.client.local.pending.env = pending('newer-command');
    return original(input);
  };
  await f.client.write(f.native, f.storage, pending());
  expect(f.client.pending?.payload.commandId).toBe('newer-command'); expect(f.client.local.drafts[A]).toBe('same');
  expect(store(f.client).claims[slot]).toBe(A); expect(f.client.reconcilePending()).toBe(false);
});


import { mobileNewTaskDraftNoteBranch as noteBranch, mobileNewTaskDraftSelectedBranch as selectedBranch } from './mobile-new-task-drafts';
import { patchDraftContext } from './shared/composer-controls-branch';
test('explicit same-name checkout choice survives persistence and differs from automatic branch', async () => {
  const f = await populated();
  patchDraftContext(f.client, { envMode: 'local', branch: 'main', worktreePath: '' });
  noteBranch(f.client, 'automatic');
  expect(selectedBranch(presentation(f.client, A)!)).toBeNull();
  const before = lookup(f.client, A)!.revision;
  noteBranch(f.client, 'explicit');
  expect(selectedBranch(presentation(f.client, A)!)).toBe('main');
  expect(lookup(f.client, A)!.revision).toBe(before + 1);
  await f.client.persist(f.storage);
  const restart = fixture(f.disk()); await restart.load(); bind(restart.client, A, 'resumed');
  expect(selectedBranch(presentation(restart.client, A)!)).toBe('main');
  expect(selectedBranch(presentation(restart.client, B)!)).toBeNull();
});
test('missing, malformed and mismatched branch provenance cannot be mistaken for explicit picks', async () => {
  const f = await populated(); patchDraftContext(f.client, { envMode: 'local', branch: 'main', worktreePath: '' });
  expect(selectedBranch(presentation(f.client, A)!)).toBeUndefined();
  noteBranch(f.client, 'explicit'); patchDraftContext(f.client, { branch: 'other' });
  expect(selectedBranch(presentation(f.client, A)!)).toBeUndefined();
  patchDraftContext(f.client, { branch: 'main', envMode: 'worktree' });
  expect(selectedBranch(presentation(f.client, A)!)).toBeUndefined();
  patchDraftContext(f.client, { envMode: 'local', worktreePath: '/other' });
  expect(selectedBranch(presentation(f.client, A)!)).toBeUndefined();
  await f.client.persist(f.storage);
  const disk = f.disk(); obj(obj(obj(disk.mobileNewTaskDrafts).records)[A]).branchChoice = { kind: 'explicit', branch: 'main', envMode: 'local', worktreePath: 1 };
  const r = fixture(disk); await r.load();
  expect(lookup(r.client, A)?.branchChoice).toBeUndefined();
  expect(presentation(r.client, A)?.text).toBe('same');
});
test('retarget clears old branch provenance and stale binding cannot mark a new project', async () => {
  const f = await populated(); patchDraftContext(f.client, { branch: 'main' }); noteBranch(f.client, 'explicit');
  retarget(f.client, A, { environmentId: 'env', projectId: 'next', origin });
  expect(lookup(f.client, A)?.branchChoice).toBeUndefined();
  noteBranch(f.client, 'explicit'); expect(lookup(f.client, A)?.branchChoice).toBeUndefined();
  expect(lookup(f.client, B)?.branchChoice).toBeUndefined();
});
