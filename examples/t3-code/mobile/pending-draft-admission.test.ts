import { mobileSend } from './composer-behavior';
// @ref llp/1109.005-composer-and-transcript.decision.md#pending-task-editor-save-and-restart-recovery
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind, mobileNewTaskDraftBoundKey,
  mobileNewTaskDraftPresentation, mobileNewTaskDraftRetarget, mobileNewTaskDraftDiscard,
  mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftList } from './mobile-new-task-drafts';
import { mobileHomeAction, mobileHomeActionsObserve } from './home-actions';
import { mobileHomeDraftAction } from './home-draft-actions';
import { projectHomeDrafts } from './home-drafts';
import { mobileBindNewTaskDraft } from './new-task-draft-binding';
import { mobileNewTaskSubmit } from './new-task-submit';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { noteNow } from './shared/composer-controls';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
const pending = 'new-task:pending-message', ordinary = 'new-task:ordinary';
async function fixture() {
  const client = new MobileDraftClient(), calls: Obj[] = [], writes: string[] = [];
  let allocated = 'fresh-id';
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}'); },
    async atomicWriteFile(_path, bytes) { writes.push(new TextDecoder().decode(bytes)); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.op === 'status' ? { phase: 'disconnected' }
      : request.op === 'ids' ? [allocated] : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin: 'https://draft.test', environmentId: 'env', projectId: 'project', threadId: '' });
  for (const id of ['ordinary', 'pending-message']) {
    mobileNewTaskDraftCreate(client, { id, origin: client.origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts[`new-task:${id}`] = `Saved ${id}`;
  }
  client.local.snapshotDrafts[pending] = [{ id: '11111111-1111-4111-8111-111111111111', name: 'kept.png', mimeType: 'image/png', sizeBytes: 12 }];
  mobileNewTaskDraftBind(client, ordinary, 'existing-flow'); noteNow(client, 1791420000000);
  mobileHomeActionsObserve('home', true, false, client); calls.length = 0; writes.length = 0;
  return { client, native, storage, calls, writes, allocated: (value: string) => { allocated = value; } };
}
test('Home generic open/discard preserves a pending editor orphan, including visible content and attachments', async () => {
  const f = await fixture(), before = JSON.stringify(f.client.local), revision = f.client.revision;
  for (const operation of ['draft-open', 'draft-discard']) {
    const reply = await mobileHomeAction('home', 'env', '', operation, pending, 0, f.native, f.client, new EnvironmentFleet(), f.storage);
    expect(reply.nextLocation).toBe(''); expect(reply.message).toContain('pending task');
    const direct = await mobileHomeDraftAction('home', 'env', '', operation, pending, () => true, f.client, f.native, f.storage);
    expect(direct.nextLocation).toBe(''); expect(direct.message).toContain('pending task');
  }
  expect(JSON.stringify(f.client.local)).toBe(before); expect(f.client.revision).toBe(revision);
  expect(f.calls).toEqual([]); expect(f.writes).toEqual([]);
  const saved = mobileNewTaskDraftList(f.client).map(draft => mobileNewTaskDraftPresentation(f.client, draft.key)!);
  expect(projectHomeDrafts(saved).some(row => row.draftKey === pending)).toBe(true);
  expect((await mobileHomeAction('home', 'env', '', 'draft-open', ordinary, 0, f.native, f.client, new EnvironmentFleet(), f.storage)).nextLocation).toContain('new-task%3Aordinary');
});
test('generic binding cannot resume, retarget, or replace an internal pending editor', async () => {
  for (const [previous, resume] of [[pending, ''], ['', pending], [pending, ordinary], [ordinary, pending]]) {
    const f = await fixture(), before = JSON.stringify(f.client.local);
    await expect(mobileBindNewTaskDraft(f.client, 'new-flow', previous, resume, f.native, f.storage, () => true)).rejects.toThrow('pending task');
    expect(JSON.stringify(f.client.local)).toBe(before); expect(mobileNewTaskDraftBoundKey(f.client)).toBe(ordinary);
    expect(f.calls).toEqual([]); expect(f.writes).toEqual([]);
  }
  const f = await fixture();
  expect(await mobileBindNewTaskDraft(f.client, 'new-flow', '', ordinary, f.native, f.storage, () => true)).toBe(ordinary);
  expect(f.writes).toHaveLength(1);
});
test('reserved malformed keys and reserved allocated IDs cannot create generic draft ownership', async () => {
  const f = await fixture(), before = JSON.stringify(f.client.local);
  for (const key of ['new-task:pending-', 'new-task:pending-?', 'new-task:pending-' + 'x'.repeat(140)]) {
    expect(mobileNewTaskDraftIsPendingKey(key)).toBe(true);
    await expect(mobileBindNewTaskDraft(f.client, 'flow', '', key, f.native, f.storage, () => true)).rejects.toThrow('pending task');
    const reply = await mobileNewTaskSubmit(f.client, f.native, f.storage, { draftKey: key, now: 1791420000000, current: () => true });
    expect(reply.status).toBe('blocked'); expect(reply.disposition).toBe('stay');
  }
  expect(f.calls).toEqual([]); f.allocated('pending-collision');
  await expect(mobileBindNewTaskDraft(f.client, 'flow', '', '', f.native, f.storage, () => true)).rejects.toThrow('reserved');
  expect(f.calls.map(call => call.op)).toEqual(['ids']); expect(f.writes).toEqual([]); expect(JSON.stringify(f.client.local)).toBe(before);
});
test('generic retarget and discard primitives preserve pending content even without a current marker', async () => {
  const f = await fixture(), before = JSON.stringify(f.client.local);
  expect(mobileNewTaskDraftRetarget(f.client, pending, { origin: 'https://other.test', environmentId: 'other', projectId: 'other' })).toBe(false);
  expect(mobileNewTaskDraftDiscard(f.client, pending)).toBe(false);
  expect(JSON.stringify(f.client.local)).toBe(before);
});
test('fresh Submit refuses a bound pending editor before allocating IDs or touching transfer storage', async () => {
  const f = await fixture(); expect(mobileNewTaskDraftBind(f.client, pending, 'pending-flow')).toBe(true);
  const before = JSON.stringify(f.client.local), revision = f.client.revision;
  for (const retryFailed of [false, true]) {
    const reply = await mobileNewTaskSubmit(f.client, f.native, f.storage,
      { draftKey: pending, now: 1791420000000, current: () => true, retryFailed });
    expect(reply).toMatchObject({ status: 'blocked', claim: null, owner: null, threadId: '', messageId: '', commandId: '', disposition: 'stay', draftRetained: true });
    expect(reply.message).toContain('pending task editor');
  }
  expect(f.calls).toEqual([]); expect(f.writes).toEqual([]); expect(JSON.stringify(f.client.local)).toBe(before); expect(f.client.revision).toBe(revision);
});


test('ordinary composer Send cannot bypass pending-editor Save with a bound pending key', async () => {
  const f = await fixture(); mobileNewTaskDraftBind(f.client, pending, 'pending-flow');
  for (const alternate of [false, true]) expect((await mobileSend(f.client, alternate, f.native, f.storage)).message).toContain('pending task editor');
  expect(f.calls).toEqual([]); expect(f.writes).toEqual([]); expect(f.client.local.drafts[pending]).toBe('Saved pending-message');
});
