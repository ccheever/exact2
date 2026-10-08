import { expect, test } from 'bun:test';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxEnqueueTransfer, mobileOutboxTransferLookup,
  mobileOutboxTransferStatus, mobileOutboxCompleteTransfer, mobileOutboxEnqueue } from './mobile-outbox';
import { mobileOutboxTransferDecodeCapture, mobileOutboxTransferDecodeClaim,
  type MobileOutboxTransferCapture, type MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import type { Native } from './shared/protocol';
const capture = (): MobileOutboxTransferCapture => ({ version: 1, draft: { key: 'new-task:a', environmentId: 'env', projectId: 'project',
  origin: 'https://server.test', createdAt: '2026-10-08T00:00:00.000Z', revision: 1, choices: null, attachmentIds: [],
  text: '  original  ', images: [], files: [], workspace: null } });
const record = (messageId = 'message'): MobileOutboxRecord => ({ schemaVersion: 1, origin: 'https://server.test', environmentId: 'env',
  threadId: `thread-${messageId}`, messageId, commandId: `command-${messageId}`, text: 'original', attachments: [],
  creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null }, createdAt: '2026-10-08T01:00:00.000Z' });
const claim = (messageId = 'message', state: MobileOutboxTransferClaim['state'] = 'queued'): MobileOutboxTransferClaim => ({
  transferId: messageId, messageId, threadId: `thread-${messageId}`, commandId: `command-${messageId}`, mutationId: 'epoch:1',
  draftKey: 'new-task:a', fingerprint: 'a'.repeat(64), state,
  record: state === 'completed' || state === 'released' ? null : record(messageId),
  capture: state === 'completed' || state === 'released' ? null : capture() });
const current = (value: MobileOutboxRecord | null = record(), token = 'epoch:1', revision = 1) => ({ record: value, token, revision, pending: false });
const outcome = (owner = claim(), row = current(owner.record)) => ({ messageId: owner.messageId, mutationId: owner.mutationId,
  status: 'committed', revision: 1, message: '', record: record(owner.messageId), removed: null,
  ownerEpoch: 'epoch', sequenceFloor: 1, current: row });
const inventory = (transfers: unknown[] = []) => ({ ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [],
  records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers });
type Call = { request: any; resolve(value: unknown): void; reject(error: unknown): void };
async function fixture(initial = inventory()) {
  const client = { revision: 0 }, calls: Call[] = [];
  const native: Native = { available: true, watch() { throw new Error('no watch'); },
    later(request) { return new Promise((resolve, reject) => calls.push({ request, resolve, reject })); } };
  const answer = (n: number, value: unknown) => calls[n]!.resolve({ ok: true, generation: 0, value });
  const reading = mobileOutboxRead(client, native); answer(0, initial); await reading;
  return { client, native, calls, answer };
}
test('capture admits original raw draft and checks queue ownership/order without caller aliases', () => {
  const original = capture(), decoded = mobileOutboxTransferDecodeCapture(original, record());
  original.draft.text = 'changed'; expect(decoded.draft.text).toBe('  original  ');
  for (const change of [ { projectId: 'elsewhere' }, { text: 'other' }, { revision: -1 }, { key: 'thread:a' },
    { createdAt: 'invalid' }, { attachmentIds: ['missing'] }, { submittedMessageId: 'newly-allocated' } ])
    expect(() => mobileOutboxTransferDecodeCapture({ version: 1, draft: { ...capture().draft, ...change } }, record())).toThrow();
  expect(() => mobileOutboxTransferDecodeCapture({ ...capture(), commandId: 'metadata' }, record())).toThrow();
});
test('claim decoder refuses owner drift, absent capture and bytes on completed tombstones', () => {
  expect(mobileOutboxTransferDecodeClaim(claim()).capture?.draft.text).toBe('  original  ');
  for (const patch of [{ transferId: 'different' }, { threadId: 'different' }, { draftKey: 'new-task:b' }, { capture: null },
    { fingerprint: 'wrong' }, { state: 'completed' }, { record: { ...record(), origin: 'https://other.test' } }])
    expect(() => mobileOutboxTransferDecodeClaim({ ...claim(), ...patch })).toThrow();
  expect(mobileOutboxTransferDecodeClaim(claim('message', 'completed')).record).toBeNull();
});
test('capture enqueue issues one native call before awaiting and retains original outcome', async () => {
  const f = await fixture(), source = capture(), queued = record();
  const sending = mobileOutboxEnqueueTransfer(f.client, f.native, queued, source);
  expect(f.calls[1]!.request).toMatchObject({ action: 'enqueueTransfer', ownerEpoch: 'epoch', mutationId: 'epoch:1', capture: source });
  source.draft.text = 'later'; queued.text = 'later'; expect(f.calls[1]!.request.capture.draft.text).toBe('  original  ');
  f.answer(1, { disposition: 'created', claim: claim(), outcome: outcome() });
  expect((await sending).disposition).toBe('created'); expect(mobileOutboxSnapshot(f.client).rows[0]?.status).toBe('confirmed');
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.capture?.draft.text).toBe('  original  ');
});
test('native existing claim with original IDs removes only the new proposal', async () => {
  const f = await fixture();
  const sending = mobileOutboxEnqueueTransfer(f.client, f.native, record('new'), capture());
  f.answer(1, { disposition: 'existing', claim: claim(), outcome: outcome() });
  const result = await sending;
  expect(result.claim?.messageId).toBe('message'); expect(mobileOutboxSnapshot(f.client).rows.map(r => r.record.messageId)).toEqual(['message']);
  expect(mobileOutboxSnapshot(f.client).intents).toEqual([]);
});
test('conflicting capture cannot remove a later optimistic replacement with the same proposed ID', async () => {
  const f = await fixture(), draft = capture(); draft.draft.text = 'changed';
  const sending = mobileOutboxEnqueueTransfer(f.client, f.native, { ...record('new'), text: 'changed' }, draft);
  const replacement = mobileOutboxEnqueue(f.client, f.native, { ...record('new'), text: 'later' });
  f.answer(1, { disposition: 'conflict', claim: claim(), outcome: outcome() });
  expect((await sending).disposition).toBe('conflict');
  expect(mobileOutboxSnapshot(f.client).rows.find(r => r.record.messageId === 'new')?.record.text).toBe('later');
  f.calls[2]!.reject(new Error('interrupted')); await replacement;
});
test('lost enqueue reply leaves unknown intent and fresh status recovers original claim', async () => {
  const f = await fixture(), sending = mobileOutboxEnqueueTransfer(f.client, f.native, record(), capture());
  f.calls[1]!.reject({ name: 'FetchError', kind: 'Aborted' });
  expect((await sending).disposition).toBe('unknown'); expect(f.calls).toHaveLength(2);
  const recovery = mobileOutboxTransferStatus(f.client, f.native, 'message');
  f.answer(2, { claim: claim(), outcome: outcome() }); expect((await recovery).outcome?.status).toBe('committed');
  expect(mobileOutboxSnapshot(f.client).intents[0]?.status).toBe('committed');
});
test('completed replay does not fabricate an optimistic record or a new transfer', async () => {
  const f = await fixture(inventory([claim('message', 'completed')]));
  const sending = mobileOutboxEnqueueTransfer(f.client, f.native, record('new'), capture());
  f.answer(1, { disposition: 'existing', claim: claim('message', 'completed'), outcome: null });
  expect((await sending).disposition).toBe('existing'); expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
  expect(mobileOutboxSnapshot(f.client).transfers).toHaveLength(1);
});
test('completion and late read cannot resurrect original capture after queue deletion', async () => {
  const f = await fixture(inventory([claim()]));
  const reading = mobileOutboxRead(f.client, f.native), completing = mobileOutboxCompleteTransfer(f.client, f.native, claim());
  f.answer(2, { completed: true, claim: claim('message', 'completed') }); expect(await completing).toBe(true);
  f.answer(1, inventory([claim()])); expect(await reading).toBe(true);
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.state).toBe('completed');
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.capture).toBeNull();
});
test('incomplete ownership blocks admission while lookup preserves explicit incomplete status', async () => {
  const f = await fixture({ ...inventory(), complete: false });
  await expect(mobileOutboxEnqueueTransfer(f.client, f.native, record(), capture())).rejects.toThrow('incomplete');
  expect(f.calls).toHaveLength(1);
  const looking = mobileOutboxTransferLookup(f.client, f.native, 'new-task:a', capture());
  f.answer(1, { complete: false, fingerprint: 'a'.repeat(64), claims: [claim()] });
  expect((await looking).complete).toBe(false);
});
test('corrupt transfer inventory preserves readable claim and prevents duplicate admission', async () => {
  const f = await fixture(inventory([claim(), { ...claim('corrupt'), capture: null }]));
  expect(mobileOutboxSnapshot(f.client).complete).toBe(false); expect(mobileOutboxSnapshot(f.client).transfers).toHaveLength(1);
  await expect(mobileOutboxEnqueueTransfer(f.client, f.native, record('new'), capture())).rejects.toThrow('incomplete');
});
test('later current row can differ from original committed capture without authorizing wrong cleanup', async () => {
  const f = await fixture(), sending = mobileOutboxEnqueueTransfer(f.client, f.native, record(), capture());
  const changed = { ...record(), text: 'edited pending task' };
  f.answer(1, { disposition: 'created', claim: claim(), outcome: outcome(claim(), current(changed, 'epoch:2', 2)) });
  const result = await sending; expect(result.claim?.record?.text).toBe('original');
  expect(mobileOutboxSnapshot(f.client).rows[0]?.record.text).toBe('edited pending task');
});

test('same-ID duplicate or conflict without an outcome retains the previous queue row as uncertain', async () => {
  for (const disposition of ['existing', 'conflict'] as const) {
    const prior = { ...record(), text: 'edited pending task' };
    const f = await fixture(), reading = mobileOutboxRead(f.client, f.native);
    f.answer(1, { ...inventory(), sequenceFloor: 2, records: [{ ...current(prior, 'epoch:2', 2), held: false }],
      revisions: { message: 2 }, tokens: { message: 'epoch:2' } });
    expect(await reading).toBe(true);
    const draft = capture(), proposed = record();
    if (disposition === 'conflict') { draft.draft.text = 'changed draft'; proposed.text = 'changed draft'; }
    const sending = mobileOutboxEnqueueTransfer(f.client, f.native, proposed, draft);
    f.answer(2, { disposition, claim: claim('message', disposition === 'existing' ? 'completed' : 'prepared'), outcome: null });
    expect((await sending).disposition).toBe(disposition);
    const rows = mobileOutboxSnapshot(f.client).rows;
    expect(rows).toHaveLength(1); expect(rows[0]?.record).toEqual(prior);
    expect(rows[0]?.status).toBe('uncertain');
  }
});
test('same fingerprint cannot replace the original capture and record through status', async () => {
  const f = await fixture(inventory([claim()])), changed = claim();
  changed.record!.text = 'different'; changed.capture!.draft.text = 'different';
  const checking = mobileOutboxTransferStatus(f.client, f.native, 'message');
  f.answer(1, { claim: changed, outcome: { ...outcome(changed), record: changed.record } });
  await expect(checking).rejects.toThrow();
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.capture?.draft.text).toBe('  original  ');
});
test('duplicate transfer IDs make inventory incomplete and block new admission', async () => {
  const f = await fixture(inventory([claim(), claim()]));
  expect(mobileOutboxSnapshot(f.client).complete).toBe(false);
  await expect(mobileOutboxEnqueueTransfer(f.client, f.native, record(), capture())).rejects.toThrow('incomplete');
  expect(f.calls).toHaveLength(1);
});
test('capture rejects undefined nested array entries instead of cloning them to null', () => {
  const source = { ...capture(), draft: { ...capture().draft,
    choices: { providerId: 'provider', modelId: 'model', modelOptions: [{ id: 'option', value: [undefined] }] } } };
  expect(() => mobileOutboxTransferDecodeCapture(source)).toThrow();
});
test('late prepared lookup cannot revive a completed capture', async () => {
  const f = await fixture(inventory([claim()]));
  const looking = mobileOutboxTransferLookup(f.client, f.native, 'new-task:a');
  const finishing = mobileOutboxCompleteTransfer(f.client, f.native, claim());
  f.answer(2, { completed: true, claim: claim('message', 'completed') }); expect(await finishing).toBe(true);
  f.answer(1, { complete: true, fingerprint: null, claims: [claim('message', 'prepared')] }); await looking;
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.state).toBe('completed');
  expect(mobileOutboxSnapshot(f.client).transfers[0]?.capture).toBeNull();
});
