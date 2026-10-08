import { expect, test } from 'bun:test';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxEnqueue, mobileOutboxUpdate, mobileOutboxRemove,
  mobileOutboxCapture, mobileOutboxConfirmQueued, mobileOutboxStatus, mobileOutboxAcknowledge,
  mobileOutboxHold, mobileOutboxReleaseHold, mobileOutboxRecover } from './mobile-outbox';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import type { Native } from './shared/protocol';
const record = (text = 'original', messageId = 'message'): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://server.test',
  environmentId: 'env', threadId: 'thread', messageId, commandId: 'command', text, attachments: [], createdAt: '2026-10-08T00:00:00.000Z' });
const row = (record: MobileOutboxRecord | null, revision: number, token: string, pending = false) => ({ record, revision, token, pending, held: false });
const inventory = (rows: ReturnType<typeof row>[] = [], floor = 0, revisions: Record<string, number> = Object.fromEntries(rows.map(value => [value.record!.messageId, value.revision])), epoch = 'epoch') =>
  ({ ownerEpoch: epoch, sequenceFloor: floor, complete: true, errors: [], records: rows, revisions, tokens: Object.fromEntries(rows.map(value => [value.record!.messageId, value.token])), outcomes: [], mutations: [] });
type Call = { request: any; resolve: (value: unknown) => void; reject: (error: unknown) => void };
function fixture() {
  const calls: Call[] = [], client = { revision: 0 };
  const native: Native = { available: true, watch() { throw new Error('no subscriptions'); }, later(request) { return new Promise((resolve, reject) => calls.push({ request, resolve, reject })); } };
  const answer = (index: number, value: unknown) => calls[index]!.resolve({ ok: true, generation: 0, value });
  const settle = (index: number, status: string, current: ReturnType<typeof row>, floor: number, extra: object = {}) => {
    const request = calls[index]!.request;
    answer(index, { mutationId: request.mutationId, messageId: request.messageId, status, revision: current.revision,
      record: status === 'committed' && request.operation !== 'remove' ? request.record : null,
      removed: status === 'committed' && request.operation === 'remove' ? record() : null, message: '',
      ownerEpoch: 'epoch', sequenceFloor: floor, current, ...extra });
  };
  return { calls, client, native, answer, settle };
}
async function opened(rows: ReturnType<typeof row>[] = [], floor = 0) {
  const f = fixture(), read = mobileOutboxRead(f.client, f.native); f.answer(0, inventory(rows, floor)); expect(await read).toBe(true); return f;
}
const text = (f: ReturnType<typeof fixture>) => mobileOutboxSnapshot(f.client).rows[0]?.record.text;

test('enqueue enters native before await; failed A cannot erase later B', async () => {
  const f = await opened(), a = record('A'), b = record('B');
  const pa = mobileOutboxEnqueue(f.client, f.native, a), pb = mobileOutboxEnqueue(f.client, f.native, b);
  expect(f.calls.map(call => call.request.action)).toEqual(['read', 'mutate', 'mutate']);
  expect(text(f)).toBe('B'); expect(f.calls[1]!.request.mutationId).toBe('epoch:1'); expect(f.calls[2]!.request.mutationId).toBe('epoch:2');
  a.text = 'caller changed'; b.text = 'caller changed'; expect(f.calls[1]!.request.record.text).toBe('A'); expect(f.calls[2]!.request.record.text).toBe('B');
  f.settle(1, 'failed', row(record('B'), 2, 'epoch:2', true), 2); expect((await pa).status).toBe('failed'); expect(text(f)).toBe('B');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('optimistic');
  f.settle(2, 'committed', row(record('B'), 2, 'epoch:2'), 2); await pb;
  expect(text(f)).toBe('B'); expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('confirmed');
});
test('failed replacement does not resurrect prior O, and snapshots do not alias state', async () => {
  const f = await opened([row(record('O'), 1, 'epoch:1')], 1), pending = mobileOutboxEnqueue(f.client, f.native, record('A'));
  const snapshot = mobileOutboxSnapshot(f.client); snapshot.rows[0]!.record.text = 'mutated'; expect(text(f)).toBe('A');
  f.settle(1, 'failed', row(null, 2, 'epoch:2'), 2); await pending; expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
  const reread = mobileOutboxRead(f.client, f.native); f.answer(2, inventory([row(record('O'), 1, 'epoch:1')], 2)); await reread;
  expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
});
test('update and remove publish only after persistence; removal returns actual native payload', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1);
  const update = mobileOutboxUpdate(f.client, f.native, record('new'), { expectedToken: 'epoch:1' }); expect(text(f)).toBe('original');
  f.settle(1, 'committed', row(record('new'), 2, 'epoch:2'), 2); await update; expect(text(f)).toBe('new');
  const remove = mobileOutboxRemove(f.client, f.native, 'message', { expectedRevision: 2 }); expect(text(f)).toBe('new');
  f.settle(2, 'committed', row(null, 3, 'epoch:3'), 3, { removed: record('new') });
  expect((await remove).removed?.text).toBe('new'); expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
  expect(f.calls[2]!.request.requireUnheld).toBe(true);
});
test('guarded update or removal losing to enqueue preserves native winner', async () => {
  for (const operation of ['update', 'remove'] as const) {
    const f = await opened([row(record(), 1, 'epoch:1')], 1);
    const a = operation === 'update' ? mobileOutboxUpdate(f.client, f.native, record('A'), { expectedToken: 'epoch:1' }) :
      mobileOutboxRemove(f.client, f.native, 'message', { expectedToken: 'epoch:1' });
    const b = mobileOutboxEnqueue(f.client, f.native, record('B'));
    f.settle(1, 'stale', row(record('B'), 2, 'epoch:3', true), 3); expect((await a).status).toBe('stale'); expect(text(f)).toBe('B');
    f.settle(2, 'committed', row(record('B'), 2, 'epoch:3'), 3); await b; expect(text(f)).toBe('B');
  }
});
test('unguarded update can replace optimistic B exactly as pinned manager does', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1);
  const a = mobileOutboxUpdate(f.client, f.native, record('A')), b = mobileOutboxEnqueue(f.client, f.native, record('B'));
  expect(f.calls[1]!.request.expectedToken).toBeUndefined(); expect(text(f)).toBe('B');
  f.settle(1, 'committed', row(record('A'), 3, 'epoch:2'), 3); await a; expect(text(f)).toBe('A');
  f.settle(2, 'committed', row(record('A'), 3, 'epoch:2'), 3, { revision: 2, record: record('B') }); await b;
  expect(text(f)).toBe('A');
});
test('completion watermark cannot overwrite enqueue admitted after its snapshot', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1);
  const a = mobileOutboxUpdate(f.client, f.native, record('A')), b = mobileOutboxEnqueue(f.client, f.native, record('B'));
  f.settle(1, 'committed', row(record('A'), 2, 'epoch:2'), 2); await a; expect(text(f)).toBe('B');
  f.settle(2, 'committed', row(record('B'), 3, 'epoch:3'), 3); await b; expect(text(f)).toBe('B');
});
test('no-CAS serialized updates and out-of-order replies retain highest native revision', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1);
  const a = mobileOutboxUpdate(f.client, f.native, record('A')), b = mobileOutboxUpdate(f.client, f.native, record('B'));
  f.settle(2, 'committed', row(record('B'), 3, 'epoch:3'), 3); await b; expect(text(f)).toBe('B');
  f.settle(1, 'committed', row(record('A'), 2, 'epoch:2'), 2); await a; expect(text(f)).toBe('B');
});
test('slow load preserves accepted enqueue and overlapping reads never reset sequence floor', async () => {
  const f = await opened(), reading = mobileOutboxRead(f.client, f.native), writing = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.answer(1, inventory([], 0, { message: 0 })); await reading; expect(text(f)).toBe('A');
  f.settle(2, 'committed', row(record('A'), 1, 'epoch:1'), 1); await writing;
  const a = mobileOutboxRead(f.client, f.native), b = mobileOutboxRead(f.client, f.native);
  f.answer(4, inventory([row(record('A'), 1, 'epoch:1')], 8)); await b;
  f.answer(3, inventory([row(record('A'), 1, 'epoch:1')], 2)); await a;
  const next = mobileOutboxEnqueue(f.client, f.native, record('B')); expect(f.calls[5]!.request.mutationId).toBe('epoch:9');
  f.settle(5, 'committed', row(record('B'), 2, 'epoch:9'), 9); await next;
});
test('partial and malformed inventories preserve readable siblings and retry without destructive absence', async () => {
  const f = await opened([row(record('kept'), 1, 'epoch:1')], 1), read = mobileOutboxRead(f.client, f.native);
  f.answer(1, { ...inventory([row(record('sibling', 'other'), 1, 'epoch:2')], 2, { message: 2, other: 1 }), complete: false,
    errors: [{ path: 'bad', message: 'corrupt' }], records: [row(record('sibling', 'other'), 1, 'epoch:2'), { record: { messageId: 'message' }, revision: 2 }] });
  expect(await read).toBe(false); expect(mobileOutboxSnapshot(f.client).rows.map(row => row.record.text).sort()).toEqual(['kept', 'sibling']);
  const retry = mobileOutboxRead(f.client, f.native); f.answer(2, inventory([row(record('kept'), 1, 'epoch:1'), row(record('sibling', 'other'), 1, 'epoch:2')], 2));
  expect(await retry).toBe(true); expect(mobileOutboxSnapshot(f.client).errors).toEqual([]);
});
test('LetGo keeps unknown state, permits independent B, and uses a fresh answer for recovery', async () => {
  const f = await opened(), a = mobileOutboxEnqueue(f.client, f.native, record('A')), b = mobileOutboxEnqueue(f.client, f.native, record('B', 'second'));
  f.calls[1]!.reject({ name: 'FetchError', kind: 'Aborted' }); expect((await a).status).toBe('unknown'); expect(f.calls).toHaveLength(3);
  f.settle(2, 'committed', row(record('B', 'second'), 1, 'epoch:2'), 2); await b;
  expect(await mobileOutboxConfirmQueued(f.client, f.native, mobileOutboxCapture(f.client, 'message')!)).toBe(false); expect(f.calls).toHaveLength(3);
  const recovery = mobileOutboxStatus(f.client, f.native, 'message', 'epoch:1');
  f.settle(3, 'committed', row(record('A'), 1, 'epoch:1'), 2, { record: record('A') }); await recovery;
  expect(mobileOutboxSnapshot(f.client).rows.find(row => row.record.messageId === 'message')?.status).toBe('confirmed');
  expect(f.calls.map(call => call.request.action)).toEqual(['read', 'mutate', 'mutate', 'status']);
});
test('malformed mutation reply stays unknown instead of rolling back or repeating work', async () => {
  const f = await opened(), a = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.answer(1, { status: 'committed', messageId: 'different' }); expect((await a).status).toBe('unknown'); expect(text(f)).toBe('A');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('uncertain'); expect(f.calls).toHaveLength(2);
});
test('confirm barrier rejects a later replacement even after native confirmed the original', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1), capture = mobileOutboxCapture(f.client, 'message')!;
  const confirm = mobileOutboxConfirmQueued(f.client, f.native, capture), writing = mobileOutboxEnqueue(f.client, f.native, record('B'));
  f.answer(1, { current: true, revision: 1 }); expect(await confirm).toBe(false);
  f.settle(2, 'committed', row(record('B'), 2, 'epoch:2'), 2); await writing;
  expect(await mobileOutboxConfirmQueued(f.client, f.native, capture)).toBe(false);
});
test('editor holds are scoped; acknowledgment never performs removal cleanup', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1), capture = mobileOutboxCapture(f.client, 'message')!;
  const hold = mobileOutboxHold(f.client, f.native, capture, 'editor-a'); expect(f.calls[1]!.request.expectedToken).toBe('epoch:1');
  f.answer(1, { held: true }); expect(await hold).toBe(true); expect(await mobileOutboxConfirmQueued(f.client, f.native, capture)).toBe(false);
  const release = mobileOutboxReleaseHold(f.client, f.native, 'message', 'editor-a'); f.answer(2, { released: true }); expect(await release).toBe(true);
  expect(mobileOutboxSnapshot(f.client).rows[0]?.held).toBe(true);
  const write = mobileOutboxUpdate(f.client, f.native, record('A')); f.settle(3, 'committed', row(record('A'), 2, 'epoch:2'), 2); await write;
  const ack = mobileOutboxAcknowledge(f.client, f.native, 'message', 'epoch:2'); f.answer(4, { acknowledged: true }); expect(await ack).toBe(true);
  expect(f.calls.map(call => call.request.action)).not.toContain('completeRemoval'); expect(mobileOutboxSnapshot(f.client).outcomes).toEqual([]);
});
test('new JS owner resumes native sequence floor and native epoch transition retains unknown receipts', async () => {
  const f = await opened([], 12), writing = mobileOutboxEnqueue(f.client, f.native, record('A')); expect(f.calls[1]!.request.mutationId).toBe('epoch:13');
  f.calls[1]!.reject(new Error('lost reply')); await writing;
  const read = mobileOutboxRead(f.client, f.native); f.answer(2, inventory([row(record('A'), 1, 'epoch:13', true)], 0, { message: 1 }, 'replacement')); await read;
  expect(mobileOutboxSnapshot(f.client).intents[0]?.status).toBe('unknown');
  const next = mobileOutboxEnqueue(f.client, f.native, record('B', 'second')); expect(f.calls[3]!.request.mutationId).toBe('replacement:1');
  f.settle(3, 'committed', row(record('B', 'second'), 1, 'replacement:1'), 1, { ownerEpoch: 'replacement' }); await next;
});
test('recovery is explicit and only an exact result changes uncertain state', async () => {
  const f = await opened(), writing = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.settle(1, 'uncertain', row(record('A'), 1, 'epoch:1', true), 1); await writing;
  expect(await mobileOutboxAcknowledge(f.client, f.native, 'message', 'epoch:1')).toBe(false); expect(f.calls).toHaveLength(2);
  const recover = mobileOutboxRecover(f.client, f.native, 'message', 'epoch:1', 'retry');
  f.settle(2, 'committed', row(record('A'), 1, 'epoch:1'), 1, { record: record('A') }); expect((await recover).status).toBe('committed');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('confirmed');
});

test('a malformed known row and invalid tombstone evidence cannot erase live content', async () => {
  const f = await opened([row(record('safe'), 4, 'epoch:4')], 4), read = mobileOutboxRead(f.client, f.native);
  f.answer(1, { ...inventory([], 5, { message: 5 }), tokens: { message: 'epoch:5' }, records: [{ record: { messageId: 'message' }, revision: 5, token: 'epoch:5', pending: false, held: false }] });
  expect(await read).toBe(false); expect(text(f)).toBe('safe');
  const invalid = mobileOutboxRead(f.client, f.native); f.answer(2, { ...inventory([], 5, { message: 5 }), tokens: { message: '' } });
  expect(await invalid).toBe(false); expect(text(f)).toBe('safe');
  const deleted = mobileOutboxRead(f.client, f.native); f.answer(3, { ...inventory([], 5, { message: 5 }), tokens: { message: 'epoch:5' } });
  expect(await deleted).toBe(true); expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
});
test('malformed unresolved inventory is exposed and cannot become a complete load', async () => {
  const f = fixture(), read = mobileOutboxRead(f.client, f.native);
  f.answer(0, { ...inventory(), mutations: [{ messageId: 'message', mutation: 42 }] });
  expect(await read).toBe(false); expect(mobileOutboxSnapshot(f.client).recovery).toEqual([{ messageId: 'message', mutation: 42 }]);
  expect(mobileOutboxSnapshot(f.client).errors).toHaveLength(1);
});

test('lost update or removal outcome blocks confirmation even when the old row is still visible', async () => {
  for (const operation of ['update', 'remove'] as const) {
    const f = await opened([row(record(), 1, 'epoch:1')], 1), capture = mobileOutboxCapture(f.client, 'message')!;
    const mutation = operation === 'update' ? mobileOutboxUpdate(f.client, f.native, record('A')) : mobileOutboxRemove(f.client, f.native, 'message');
    f.calls[1]!.reject(new Error('reply lost')); expect((await mutation).status).toBe('unknown'); expect(text(f)).toBe('original');
    expect(await mobileOutboxConfirmQueued(f.client, f.native, capture)).toBe(false); expect(f.calls).toHaveLength(2);
  }
});

test('identical reads and status outcomes do not cause revision-driven refresh loops', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1), revision = f.client.revision;
  const read = mobileOutboxRead(f.client, f.native); f.answer(1, inventory([row(record(), 1, 'epoch:1')], 1)); await read;
  expect(f.client.revision).toBe(revision);
  const writing = mobileOutboxEnqueue(f.client, f.native, record('A')); f.settle(2, 'committed', row(record('A'), 2, 'epoch:2'), 2);
  const result = await writing, settledRevision = f.client.revision;
  const status = mobileOutboxStatus(f.client, f.native, 'message', 'epoch:2'); f.answer(3, result); await status;
  expect(f.client.revision).toBe(settledRevision);
  const again = mobileOutboxRead(f.client, f.native); f.answer(4, { ...inventory([row(record('A'), 2, 'epoch:2')], 2), outcomes: [result] }); await again;
  expect(f.client.revision).toBe(settledRevision);
});
test('an older read cannot erase a newer enqueue after its reply is lost', async () => {
  const f = await opened(), reading = mobileOutboxRead(f.client, f.native), writing = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.calls[2]!.reject(new Error('reply lost')); await writing;
  f.answer(1, { ...inventory([], 0, { message: 0 }), tokens: { message: '' } }); await reading;
  expect(text(f)).toBe('A'); expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('uncertain');
});

test('Swift object-key order changes do not invalidate the logical row or public snapshot', async () => {
  const original = { ...record(), modelSelection: { instanceId: 'p', model: 'm', options: [{ id: 'fast', value: true }] } };
  const f = await opened([row(original, 1, 'epoch:1')], 1), revision = f.client.revision, capture = mobileOutboxCapture(f.client, 'message');
  const reordered = Object.fromEntries(Object.entries(original).reverse()) as unknown as MobileOutboxRecord;
  reordered.modelSelection = { options: [{ value: true, id: 'fast' }], model: 'm', instanceId: 'p' };
  const read = mobileOutboxRead(f.client, f.native); f.answer(1, inventory([row(reordered, 1, 'epoch:1')], 1)); await read;
  expect(f.client.revision).toBe(revision); expect(mobileOutboxCapture(f.client, 'message')).toEqual(capture);
});

test('new native epoch reconciles an old unknown enqueue to a different accepted token', async () => {
  const f = await opened(), writing = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.calls[1]!.reject(new Error('lost reply')); await writing;
  const accepted = row(record('B'), 2, 'epoch:2'), read = mobileOutboxRead(f.client, f.native);
  f.answer(2, { ...inventory([accepted], 0, { message: 2 }, 'replacement'), outcomes: [{ mutationId: 'epoch:1', messageId: 'message',
    status: 'committed', revision: 1, record: record('A'), removed: null, message: '', ownerEpoch: 'replacement', sequenceFloor: 0, current: accepted }] });
  expect(await read).toBe(true); expect(text(f)).toBe('B');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('confirmed');
  expect(mobileOutboxSnapshot(f.client).outcomes[0]?.status).toBe('committed');
});
test('new native epoch can reconcile an old unknown row to an authoritative deletion', async () => {
  const f = await opened(), writing = mobileOutboxEnqueue(f.client, f.native, record('A'));
  f.calls[1]!.reject(new Error('lost reply')); await writing;
  const deleted = row(null, 2, 'epoch:2'), read = mobileOutboxRead(f.client, f.native);
  f.answer(2, { ...inventory([], 0, { message: 2 }, 'replacement'), tokens: { message: 'epoch:2' }, outcomes: [{ mutationId: 'epoch:1', messageId: 'message',
    status: 'failed', revision: 1, record: null, removed: null, message: '', ownerEpoch: 'replacement', sequenceFloor: 0, current: deleted }] });
  expect(await read).toBe(true); expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
  expect(mobileOutboxSnapshot(f.client).outcomes[0]?.status).toBe('failed');
});
test('new-epoch reply cannot overwrite a newer same-epoch local admission', async () => {
  const f = await opened([row(record(), 1, 'epoch:1')], 1), firstRead = mobileOutboxRead(f.client, f.native);
  f.answer(1, inventory([row(record(), 1, 'epoch:1')], 0, { message: 1 }, 'replacement')); await firstRead;
  const staleRead = mobileOutboxRead(f.client, f.native), writing = mobileOutboxEnqueue(f.client, f.native, record('B'));
  f.answer(2, inventory([row(record('old snapshot'), 1, 'epoch:1')], 0, { message: 1 }, 'replacement')); await staleRead;
  expect(text(f)).toBe('B'); expect(f.calls[3]!.request.mutationId).toBe('replacement:1');
  f.settle(3, 'committed', row(record('B'), 2, 'replacement:1'), 1, { ownerEpoch: 'replacement' }); await writing;
  expect(text(f)).toBe('B');
});
test('pending editor may retarget a whole valid queue record while keeping metadata IDs and time', async () => {
  // Pinned new-task-flow-provider.ts buildPendingTaskMessage selects environment/project,
  // while metadata retains threadId/messageId/commandId/createdAt for pending edits.
  const original = { ...record(), creation: { projectId: 'old-project', workspaceMode: 'local' as const, branch: null, worktreePath: null } };
  const f = await opened([row(original, 1, 'epoch:1')], 1);
  const changed = { ...original, origin: 'https://second.test', environmentId: 'second-env', creation: { ...original.creation, projectId: 'new-project' } };
  const updating = mobileOutboxUpdate(f.client, f.native, changed, { expectedToken: 'epoch:1' }).then(value => value, error => ({ error: String(error) }));
  const call = f.calls[1]; expect(call).toBeDefined();
  if (call) f.settle(1, 'committed', row(changed, 2, 'epoch:2'), 2);
  expect('error' in await updating).toBe(false);
  const stored = mobileOutboxSnapshot(f.client).rows[0]!.record;
  expect(stored.origin).toBe('https://second.test'); expect(stored.environmentId).toBe('second-env'); expect(stored.creation?.projectId).toBe('new-project');
  expect([stored.threadId, stored.messageId, stored.commandId, stored.createdAt]).toEqual([original.threadId, original.messageId, original.commandId, original.createdAt]);
});
