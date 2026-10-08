// @ref llp/1109.004-home-projection.decision.md#decision
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileHomeAction, mobileHomeActionsObserve } from './home-actions';
import { mobileOutboxRead } from './mobile-outbox';
import { mobileOutboxOwner, mobileOutboxThread } from './mobile-outbox-presentation';
import { mobileOutboxDriveSnapshot } from './mobile-outbox-drive';
import type { MobileOutboxRecord } from './mobile-outbox-model';

const now = Date.parse('2026-10-08T12:00:00Z');
const original = (): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://home.test', environmentId: 'queued/env ?#',
  threadId: 'queued/thread ?#', messageId: 'original-message', commandId: 'original-command', text: 'Inspect the app', attachments: [],
  creation: { projectId: 'project', projectTitle: 'Saved project', workspaceMode: 'local', branch: 'main', worktreePath: null },
  createdAt: new Date(now).toISOString() });
async function fixture(record = original()) {
  const client = new T3Client(), background = new EnvironmentFleet(), calls: unknown[] = [];
  Object.assign(client, { environmentId: 'focused-env', origin: 'https://focused.test', threadId: 'focused-thread', projectId: 'focused-project' });
  client.local.drafts[client.draftKey] = 'Unrelated draft';
  const native: Native = { available: true, watch() { throw Error('No watch'); }, async later(request) {
    calls.push(request); return { ok: true, generation: 0, value: { ownerEpoch: 'epoch', sequenceFloor: 1, complete: true, errors: [],
      records: [{ record, revision: 1, token: 'epoch:1', held: false, pending: false }], revisions: { [record.messageId]: 1 },
      tokens: { [record.messageId]: 'epoch:1' }, outcomes: [], mutations: [], transfers: [] } }; } };
  expect(await mobileOutboxRead(client, native)).toBe(true); calls.length = 0;
  const owner = mobileOutboxOwner(record);
  const act = (value = owner, route = 'home-route', env = record.environmentId, thread = record.threadId) =>
    mobileHomeAction(route, env, thread, 'pending-open', value, now, native, client, background);
  return { client, background, record, native, calls, owner, act };
}
test('pending-open matches exact owner on visible Home and encodes original route without selecting a shared thread', async () => {
  const f = await fixture(), before = [f.client.environmentId, f.client.threadId, f.client.projectId, f.client.draft, f.client.pending];
  mobileHomeActionsObserve('home-route', true, false, f.client);
  expect(await f.act()).toMatchObject({ message: '', nextLocation: '/threads/queued%2Fenv%20%3F%23/queued%2Fthread%20%3F%23' });
  expect([f.client.environmentId, f.client.threadId, f.client.projectId, f.client.draft, f.client.pending]).toEqual(before);
  expect(f.calls).toEqual([]);
});
test('visible sidebar admits original pending row while Home is hidden', async () => {
  const f = await fixture(); mobileHomeActionsObserve('home-route', false, true, f.client);
  expect((await f.act()).nextLocation).toContain('/threads/'); expect(f.calls).toEqual([]);
});
test('unobserved, hidden and superseded Home routes do not navigate or call native', async () => {
  const f = await fixture();
  expect((await f.act()).nextLocation).toBe('');
  mobileHomeActionsObserve('home-route', false, false, f.client);
  expect((await f.act()).nextLocation).toBe('');
  mobileHomeActionsObserve('new-route', true, true, f.client);
  expect((await f.act()).nextLocation).toBe(''); expect(f.calls).toEqual([]);
});
test('changing any captured owner field or supplied env/thread refuses pending navigation', async () => {
  const f = await fixture(); mobileHomeActionsObserve('home-route', true, false, f.client);
  for (const field of ['origin', 'environmentId', 'threadId', 'messageId', 'commandId']) {
    const owner = JSON.parse(f.owner); owner[field] = 'forged';
    expect(await f.act(JSON.stringify(owner))).toMatchObject({ nextLocation: '', message: 'This pending task changed. Refresh the list before opening it.' });
  }
  expect((await f.act(f.owner, 'home-route', 'different')).nextLocation).toBe('');
  expect((await f.act(f.owner, 'home-route', f.record.environmentId, 'different')).nextLocation).toBe('');
  expect(f.calls).toEqual([]);
});
test('invalid serialized owners cannot navigate or trigger endpoint operations', async () => {
  const f = await fixture(); mobileHomeActionsObserve('home-route', true, false, f.client);
  for (const invalid of ['', '{}', 'null', '[]', 'not-json', JSON.stringify({ ...JSON.parse(f.owner), extra: true })]) {
    expect((await f.act(invalid)).nextLocation).toBe('');
  }
  expect(f.calls).toEqual([]);
});
test('queue content edits preserve the original navigation identity', async () => {
  const record = original(); record.text = 'Edited while still queued';
  const f = await fixture(record); mobileHomeActionsObserve('home-route', true, false, f.client);
  expect((await f.act()).nextLocation).toContain('/threads/');
  expect(mobileOutboxThread(record.environmentId, record.threadId, now, false, f.client)?.rows[0]?.body).toBe(record.text);
});
test('ordinary follow-up is not a pending creation and cannot be opened through the pending row action', async () => {
  const record = original(); delete record.creation;
  const f = await fixture(record); mobileHomeActionsObserve('home-route', true, false, f.client);
  expect(await f.act()).toMatchObject({ nextLocation: '', message: 'This pending task changed. Refresh the list before opening it.' });
  expect(f.calls).toEqual([]);
});
test('foreign queued creation is readable but cannot become an automatic delivery on the wrong focused environment', async () => {
  const f = await fixture(); mobileHomeActionsObserve('home-route', true, false, f.client);
  expect((await f.act()).nextLocation).not.toBe('');
  expect(mobileOutboxThread(f.record.environmentId, f.record.threadId, now, false, f.client)?.queued).toBe(true);
  expect(mobileOutboxDriveSnapshot(f.client, now).next).toBe('');
  expect(f.client.environmentId).toBe('focused-env'); expect(f.calls).toEqual([]);
});
test('removed pending owner cannot be reopened from a stale visible row', async () => {
  const f = await fixture(); mobileHomeActionsObserve('home-route', true, false, f.client);
  const removed: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {
    ownerEpoch: 'epoch', sequenceFloor: 2, complete: true, errors: [], records: [], revisions: { [f.record.messageId]: 2 },
    tokens: { [f.record.messageId]: 'epoch:2' }, outcomes: [], mutations: [], transfers: [] } }; } };
  expect(await mobileOutboxRead(f.client, removed)).toBe(true);
  expect(await f.act()).toMatchObject({ nextLocation: '', message: 'This pending task changed. Refresh the list before opening it.' });
  expect(f.calls).toEqual([]);
});
