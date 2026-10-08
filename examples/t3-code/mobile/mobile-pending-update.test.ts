import { expect, test } from 'bun:test';
import { mobileOutboxRead, mobileOutboxPrepareUpdate, mobileOutboxResumeUpdate, mobileOutboxSnapshot } from './mobile-outbox';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import type { Native } from './shared/protocol';
const record = (text = 'original'): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://server.test',
  environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command', text, attachments: [],
  createdAt: '2026-10-08T00:00:00.000Z', creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } });
const expected = { expectedToken: 'old:1', expectedRevision: 1 };
const inventory = (epoch = 'old', floor = 1) => ({ ownerEpoch: epoch, sequenceFloor: floor, complete: true, errors: [],
  records: [{ record: record(), token: 'old:1', revision: 1, pending: false, held: true }],
  revisions: { message: 1 }, tokens: { message: 'old:1' }, outcomes: [], mutations: [], transfers: [] });
type Call = { request: any; answer(value: unknown): void };
async function fixture(epoch = 'old', floor = 1) {
  const client = { revision: 0 }, calls: Call[] = [];
  const native: Native = { available: true, watch() { throw new Error('no watches'); }, later(request) {
    return new Promise(resolve => calls.push({ request, answer: value => resolve({ ok: true, generation: 0, value }) }));
  } };
  const read = mobileOutboxRead(client, native); calls[0]!.answer(inventory(epoch, floor)); await read;
  return { client, native, calls };
}
function result(request: any, epoch = 'old', status = 'committed') {
  return { mutationId: request.mutationId, messageId: 'message', status, message: '', revision: 2,
    record: status === 'committed' ? request.record : null, removed: null, ownerEpoch: epoch, sequenceFloor: 2,
    current: { record: status === 'committed' ? request.record : record(), token: status === 'committed' ? request.mutationId : 'old:1', revision: 2, pending: false } };
}
test('prepare reserves immutable captured revisions without admitting native work', async () => {
  const f = await fixture(), input = record('first'), saved = mobileOutboxPrepareUpdate(f.client, input, expected);
  input.text = 'later'; expect(saved.record.text).toBe('first'); expect(Object.isFrozen(saved.record.creation)).toBe(true);
  expect(f.calls).toHaveLength(1); expect(saved.mutationId).toBe('old:2');
  expect(mobileOutboxPrepareUpdate(f.client, record('second'), expected).mutationId).toBe('old:3');
  expect(() => mobileOutboxPrepareUpdate(f.client, record(), {})).toThrow('exact pending task');
});
test('restart submits original ID and frozen payload under new native epoch', async () => {
  const first = await fixture(), saved = mobileOutboxPrepareUpdate(first.client, record('captured'), expected);
  const cold = await fixture('new', 40), pending = mobileOutboxResumeUpdate(cold.client, cold.native, saved, 'editor-session');
  expect(cold.calls[1]!.request).toEqual({ op: 'mobileOutbox', action: 'resumeUpdate', ownerEpoch: 'new', holdOwner: 'editor-session', request: saved });
  cold.calls[1]!.answer(result(saved, 'new')); expect((await pending).mutationId).toBe('old:2');
  expect(mobileOutboxSnapshot(cold.client).rows[0]!.record.text).toBe('captured');
  expect(mobileOutboxPrepareUpdate(cold.client, record(), expected).mutationId).toBe('new:41');
});
test('same-epoch saved request reserves floor immediately and uncertain retry keeps exact request', async () => {
  const first = await fixture('old', 8), saved = mobileOutboxPrepareUpdate(first.client, record('frozen'), expected);
  const resumed = await fixture('old', 1), pending = mobileOutboxResumeUpdate(resumed.client, resumed.native, saved, 'editor');
  expect(mobileOutboxPrepareUpdate(resumed.client, record(), expected).mutationId).toBe('old:10');
  resumed.calls[1]!.answer({ mutationId: saved.mutationId, messageId: saved.messageId, status: 'unknown', message: 'interrupted' });
  expect((await pending).status).toBe('unknown');
  const retry = mobileOutboxResumeUpdate(resumed.client, resumed.native, saved, 'editor');
  expect(resumed.calls[2]!.request.request).toEqual(saved); resumed.calls[2]!.answer(result(saved)); await retry;
  expect(mobileOutboxSnapshot(resumed.client).outcomes[0]!.status).toBe('committed');
});
test('invalid saved ownership is refused before native; mismatched receipt cannot adopt a row', async () => {
  const f = await fixture(), saved = mobileOutboxPrepareUpdate(f.client, record(), expected);
  await expect(mobileOutboxResumeUpdate(f.client, f.native, { ...saved, mutationId: 'different:9' }, 'editor')).rejects.toThrow('invalid');
  await expect(mobileOutboxResumeUpdate(f.client, f.native, saved, '')).rejects.toThrow('invalid');
  expect(f.calls).toHaveLength(1);
  const pending = mobileOutboxResumeUpdate(f.client, f.native, saved, 'editor');
  f.calls[1]!.answer({ ...result(saved), mutationId: 'elsewhere:1' });
  expect((await pending).status).toBe('unknown'); expect(mobileOutboxSnapshot(f.client).rows[0]!.record.text).toBe('original');
});
