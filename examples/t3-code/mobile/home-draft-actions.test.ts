import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftChanged, mobileNewTaskDraftLookup, mobileNewTaskDraftStore, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileHomeAction, mobileHomeActionsObserve } from './home-actions';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import type { Files, Native } from './shared/protocol';
const A = 'new-task:A', B = 'new-task:B';
const image = { id: '11111111-1111-4111-8111-111111111111', name: 'photo.jpg', mimeType: 'image/jpeg', sizeBytes: 12 };
async function fixture(saved: Obj = { version: 1 }) {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet();
  let disk = JSON.stringify(saved), failed = false, choice = 'discard';
  const calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    if (failed) throw new Error('disk full'); disk = new TextDecoder().decode(bytes);
  } } };
  let onPrompt = async () => {};
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileAlert') { await onPrompt(); return { ok: true, generation: 1, value: { choice } }; }
    return { ok: true, generation: 1, value: request.op === 'status' ? { phase: 'disconnected' } : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  mobileHomeActionsObserve('home-visit', true, false, client); calls.length = 0;
  const action = (operation = 'draft-discard', key = A, environment = 'offline') => mobileHomeAction('home-visit', environment, '', operation, key, 0, native, client, fleet, storage);
  return { client, native, storage, calls, action, disk: () => obj(JSON.parse(disk)), fail: () => { failed = true; }, succeed: () => { failed = false; },
    choice: (value: string) => { choice = value; }, prompt: (fn: () => Promise<void>) => { onPrompt = fn; } };
}
async function populated() {
  const f = await fixture();
  for (const id of ['A', 'B']) {
    mobileNewTaskDraftCreate(f.client, { id, environmentId: 'offline', projectId: 'p:?/#', origin: 'https://old.test', createdAt: '2026-10-08T00:00:00Z' });
    f.client.local.drafts[`new-task:${id}`] = `text ${id}`;
  }
  f.client.local.snapshotDrafts[A] = [image];
  setDraftFiles(f.client.local, [{ id: 'file-A', draftKey: A, environmentId: 'offline', contextId: 'context-A', name: 'a.txt', mimeType: 'text/plain', sizeBytes: 1, source: 'attached', status: 'staged' }]);
  f.client.local.composerControls.contexts[A] = { envMode: 'local', branch: 'branch-a', worktreePath: '' };
  await f.client.persist(f.storage); return f;
}
test('offline open routes the full identity without choosing a thread or changing the draft', async () => {
  const f = await populated(), before = JSON.stringify(f.client.local), revision = f.client.revision;
  const reply = await f.action('draft-open');
  expect(reply).toEqual({ revision, requestRoute: 'home-visit', message: '', alertTitle: '', nextLocation: '/new/draft?environmentId=offline&projectId=p%3A%3F%2F%23&draftId=new-task%3AA', archiveChanged: false, uncertain: false });
  expect(JSON.stringify(f.client.local)).toBe(before); expect(f.calls).toEqual([]); expect(f.client.threadId).toBe('');
  expect((await f.action('draft-open', A, 'wrong')).nextLocation).toBe('');
});
test('confirmed discard persists only A removal and releases its images and attached files', async () => {
  const f = await populated(), beforeB = mobileNewTaskDraftLookup(f.client, B);
  expect((await f.action()).message).toBe('');
  expect(mobileNewTaskDraftLookup(f.client, A)).toBeNull(); expect(f.client.local.drafts[B]).toBe('text B');
  expect(mobileNewTaskDraftLookup(f.client, B)).toEqual(beforeB); expect(f.client.local.composerControls.contexts[A]).toBeUndefined();
  expect(draftFiles(f.client.local)).toEqual([]); expect(obj(f.disk().drafts)[A]).toBeUndefined();
  expect(f.calls[0]).toEqual({ op: 'mobileAlert', kind: 'discard', title: 'Discard draft?', message: '“text A” will be removed.' });
  expect(f.calls.filter(call => call.op === 'snapshotDraftRemove').map(call => call.id)).toEqual([image.id]);
  expect(f.calls.filter(call => call.op === 'composerAttachRemove').map(call => call.id)).toEqual(['file-A']);
  expect(f.calls.some(call => ['request', 'session', 'ids'].includes(String(call.op)))).toBe(false);
});
test('Cancel, stale surface, and changed content or revision preserve the captured draft', async () => {
  for (const mode of ['cancel', 'surface', 'revision', 'bytes', 'stamp']) {
    const f = await populated();
    if (mode === 'cancel') f.choice('cancel');
    f.prompt(async () => {
      if (mode === 'surface') mobileHomeActionsObserve('other', false, false, f.client);
      if (mode === 'revision') mobileNewTaskDraftChanged(f.client, A);
      if (mode === 'bytes') f.client.local.snapshotDrafts[A]!.push({ ...image, id: 'new-image' });
      if (mode === 'stamp') mobileNewTaskDraftStore(f.client).records[A]!.origin = 'https://replacement.test';
    });
    await f.action(); expect(mobileNewTaskDraftLookup(f.client, A)).not.toBeNull();
    expect(f.calls.some(call => String(call.op).endsWith('Remove'))).toBe(false);
  }
});
test('duplicate prompt and captured launch claims refuse a second discard', async () => {
  const f = await populated(); let finish!: () => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; });
  f.prompt(() => { entered(); return new Promise<void>(resolve => { finish = resolve; }); });
  const first = f.action(); await started; expect((await f.action()).message).toBe('');
  mobileNewTaskDraftStore(f.client).claims.pending = A; finish();
  expect((await first).message).toContain('Resolve the captured launch');
  expect(f.calls.filter(call => call.op === 'mobileAlert')).toHaveLength(1); expect(mobileNewTaskDraftLookup(f.client, A)).not.toBeNull();
});
test('failed persistence preserves durable old draft and queued bytes, then existing retry commits deletion', async () => {
  const f = await populated(), oldDisk = f.disk(); f.fail();
  expect((await f.action()).message).toContain('may return after restarting');
  expect(f.disk()).toEqual(oldDisk); expect(f.client.local.drafts[B]).toBe('text B');
  expect(mobileNewTaskDraftLookup(f.client, A)).toBeNull();
  expect(f.client.local.snapshotReleases).toContain(image.id); expect(mobileNewTaskDraftStore(f.client).fileReleases).toContain('file-A');
  await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(f.calls.some(call => String(call.op).endsWith('Remove'))).toBe(false);
  const restarted = await fixture(f.disk()); expect(restarted.client.local.drafts[A]).toBe('text A'); expect(mobileNewTaskDraftLookup(restarted.client, A)).not.toBeNull();
  f.succeed(); await f.client.flushSnapshotReleases(f.native, f.storage);
  expect(obj(f.disk().drafts)[A]).toBeUndefined(); expect(obj(f.disk().drafts)[B]).toBe('text B');
  expect(f.calls.filter(call => String(call.op).endsWith('Remove'))).toHaveLength(2);
});
test('shared attachment bytes survive a successful discard and current same-key edits block confirmation', async () => {
  const f = await populated(); f.client.local.snapshotDrafts[B] = [image];
  await f.action(); expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(false);
  expect(f.client.local.snapshotDrafts[B]).toEqual([image]);
  const g = await populated(); Object.assign(g.client, { environmentId: 'offline', origin: 'https://old.test', projectId: 'p:?/#', threadId: '' });
  expect(mobileNewTaskDraftBind(g.client, A, 'editor')).toBe(true);
  g.prompt(async () => { g.client.local.drafts[A] = 'newer'; mobileNewTaskDraftChanged(g.client, A); });
  expect((await g.action()).message).toContain('This draft changed'); expect(g.client.local.drafts[A]).toBe('newer');
});
