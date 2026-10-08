import { mobileOutboxThread, mobileOutboxOwner, mobileOutboxPendingTasks } from './mobile-outbox-presentation';
import { mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileOutboxRootSnapshot as rootView, mobileOutboxRootAction as rootAction, mobileOutboxRootEdit as editRecovered } from './mobile-outbox-root';
import { mobilePendingTaskEditorsCreate } from './mobile-pending-task-state';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileNewTaskDraftBind, mobileNewTaskDraftDiscard, mobileNewTaskDraftRetarget } from './mobile-new-task-drafts';
import { mobileNewTaskSubmit } from './new-task-submit';
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileOutboxDraftRecover as recover, mobileOutboxDraftRecoveryBlocked as blocked } from './mobile-outbox-draft-recovery';
import { mobileOutboxDraftHandoffProjection } from './mobile-outbox-draft-handoff';
import { obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import type { MobileOutboxRecord } from './mobile-outbox-model';
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const owner = { origin: 'https://recover.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
const key = 'new-task:restored-message';
const record: MobileOutboxRecord = { schemaVersion: 1, ...owner, text: 'recover me', attachments: [], createdAt: '2026-10-08T00:00:00.000Z',
  creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } };
function backend() {
  const state: { row: MobileOutboxRecord | null; token: string; revision: number; floor: number; epoch: string; hold: string;
    proof: Obj | null; removal: boolean; outcomes: Map<string, Obj>; requests: Map<string, Obj>; disk: string } = {
    row: copy(record), token: 'epoch:1', revision: 1, floor: 1, epoch: 'epoch', hold: '', proof: null, removal: false,
    outcomes: new Map(), requests: new Map(), disk: '{"version":1}' };
  const terminal = { kind: 'outbox', operationId: 'command', revision: 3, origin: owner.origin, environmentId: owner.environmentId, threadId: owner.threadId, messageId: owner.messageId, rowToken: 'epoch:1', rowRevision: 1,
    record: copy(record), stage: 'start-turn', method: 'orchestration.launchThread', payload: { commandId: 'command', threadId: 'thread', initialMessage: { messageId: 'message' } },
    attachmentIDs: [], state: 'rejected', attemptRevision: 2, attemptPreviousState: 'reserved', error: { kind: 'EnvironmentAuthorizationError' } };
  const calls: Obj[] = [], hooks: { before?: (request: Obj) => Promise<void>; after?: (request: Obj) => Promise<void>; write?: (document: Obj) => Promise<void> } = {};
  const current = () => ({ record: copy(state.row), token: state.token, revision: state.revision, pending: false });
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(state.disk).buffer; },
    async atomicWriteFile(_path, bytes) { const document = JSON.parse(new TextDecoder().decode(bytes)); await hooks.write?.(document); state.disk = JSON.stringify(document); } } };
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(copy(request)); await hooks.before?.(request); let value: unknown;
    if (request.op === 'status') value = { phase: 'disconnected' };
    else if (request.op === 'devicePresentation') value = {};
    else if (request.op === 'mobileOutboxDelivery') {
      if (request.action === 'status') value = { operation: request.operationId === 'command' ? terminal : null, durable: request.operationId === 'command' };
      else if (request.action === 'draftHandoffStatus') value = { handoff: state.proof, durable: !!state.proof };
      else if (request.action === 'draftHandoffComplete') {
        if (!state.proof) {
          const handoff = obj(request.handoff), disk = obj(JSON.parse(state.disk));
          expect(mobileOutboxDraftHandoffProjection(disk, str(handoff.draftKey))).toEqual(handoff.draft);
          expect(obj(disk.mobileOutboxDraftHandoffs).command).toEqual(handoff);
          expect(state.outcomes.get(str(obj(handoff.request).mutationId))?.status).toBe('committed');
          state.proof = copy(handoff);
        } else expect(request.handoff).toEqual(state.proof);
        value = { handoff: state.proof, durable: true };
      } else throw Error('Unexpected delivery action');
    } else if (request.op === 'mobileOutboxInline' && request.action === 'lookup') value = { operations: [] };
    else if (request.op === 'mobileOutbox') {
      if (request.action === 'read') value = { complete: true, errors: [], ownerEpoch: state.epoch, sequenceFloor: state.floor,
        records: state.row ? [{ ...current(), held: !!state.hold }] : [], mutations: [], transfers: [], outcomes: [...state.outcomes.values()],
        revisions: { message: state.revision }, tokens: { message: state.token } };
      else if (request.action === 'hold') { const held = !!state.row && request.expectedToken === state.token; if (held) state.hold = str(request.owner); value = { held }; }
      else if (request.action === 'releaseHold') { state.hold = ''; value = { released: true }; }
      else if (request.action === 'resumeRemoval') {
        const saved = obj(request.request), mutation = str(saved.mutationId), known = state.requests.get(mutation);
        if (known) expect(saved).toEqual(known); else state.requests.set(mutation, copy(saved));
        if (state.outcomes.has(mutation)) value = state.outcomes.get(mutation);
        else {
          const committed = !!state.row && state.hold === request.holdOwner && saved.expectedToken === state.token && saved.expectedRevision === state.revision;
          const removed = committed ? copy(state.row) : null; state.floor++;
          if (committed) { state.row = null; state.token = mutation; state.revision++; state.removal = true; }
          value = { messageId: 'message', mutationId: mutation, status: committed ? 'committed' : 'stale', message: '', revision: state.revision,
            record: null, removed, current: current(), ownerEpoch: state.epoch, sequenceFloor: state.floor };
          state.outcomes.set(mutation, obj(value));
        }
      } else if (request.action === 'acknowledge') { state.outcomes.delete(str(request.mutationId)); value = { acknowledged: true }; }
      else if (request.action === 'completeRemoval') { value = state.removal ? { completed: true } : { completed: false, absent: true }; state.removal = false; }
      else throw Error('Unexpected outbox action');
    } else throw Error('No network fallback');
    await hooks.after?.(request); return { ok: true, generation: 0, value: copy(value) };
  } };
  async function load() { const client = new MobileDraftClient(), h = mobileDraftRecoveryHandles(client, native, storage);
    await client.refresh(h.native, h.storage); Object.assign(client, { origin: owner.origin, environmentId: 'env', projectId: 'project', threadId: '' }); return client; }
  return { state, terminal, native, storage, hooks, calls, load, input: { owner, current: () => true } };
}
test('real coordinator publishes before removal, releases owners, and never restores a consumed target on retry', async () => {
  const f = backend(); let client = await f.load();
  expect(await recover(client, f.native, f.storage, f.input)).toMatchObject({status:'recovered'}); expect(client.local.drafts[key]).toBe('recover me');
  expect(f.state.row).toBeNull(); expect(f.state.removal).toBe(false); expect(blocked(client, key)).toBe(false);
  client.local.drafts[key] = ''; await client.persist(f.storage); client = await f.load();
  expect(await recover(client, f.native, f.storage, f.input)).toMatchObject({status:'recovered'}); expect(client.local.drafts[key] ?? '').toBe('');
  expect(f.state.requests.size).toBe(1);
});
for (const step of ['preference', 'removal-reply', 'completion-reply', 'clear-proof']) test(`interrupted ${step} resumes one saved request after restart`, async () => {
  const f = backend(); let client = await f.load(), fired = false;
  f.hooks.write = async document => {
    if (!fired && (step === 'preference' || step === 'clear-proof' && f.state.proof && Object.keys(obj(document.mobileOutboxDraftHandoffs)).length === 0)) { fired = true; throw Error('disk failed'); }
  };
  f.hooks.after = async request => {
    if (!fired && (step === 'removal-reply' && request.action === 'resumeRemoval' || step === 'completion-reply' && request.action === 'draftHandoffComplete')) { fired = true; throw Error('reply lost'); }
  };
  expect((await recover(client, f.native, f.storage, f.input)).status).toBe('retained'); expect(fired).toBe(true);
  f.hooks.write = undefined; f.hooks.after = undefined;
  // A failed pre-admission write has only live ownership; retry it before simulating restart.
  if (step === 'preference') await client.persist(f.storage);
  f.state.epoch = 'cold'; f.state.hold = ''; client = await f.load();
  expect(await recover(client, f.native, f.storage, f.input)).toMatchObject({status:'recovered'}); expect(client.local.drafts[key]).toBe('recover me');
  expect(f.state.requests.size).toBe(1); expect(f.state.removal).toBe(false);
});
test('typing during the first saved write survives all subsequent publication and completion writes', async () => {
  const f = backend(), client = await f.load(); let fired = false;
  f.hooks.write = async () => { if (!fired) { fired = true; client.local.drafts[key] += ' plus newer typing'; } };
  expect(await recover(client, f.native, f.storage, f.input)).toMatchObject({status:'recovered'});
  expect(client.local.drafts[key]).toBe('recover me plus newer typing'); expect(obj(obj(JSON.parse(f.state.disk)).drafts)[key]).toBe(client.local.drafts[key]);
});
test('stale removal rolls back an untouched contribution but preserves newer text and the replacement queue row', async () => {
  for (const newer of [false, true]) {
    const f = backend(), client = await f.load();
    f.hooks.before = async request => { if (request.action === 'resumeRemoval') { f.state.row = { ...record, text: 'winner' }; f.state.revision++; f.state.token = 'winner:1';
      if (newer) client.local.drafts[key] += ' typed'; } };
    expect((await recover(client, f.native, f.storage, f.input)).status).toBe('stale');
    expect(f.state.row?.text).toBe('winner'); expect(client.local.drafts[key] ?? '').toBe(newer ? 'recover me typed' : ''); expect(f.state.proof).toBeNull();
  }
});
test('preparation refusal releases an unpublished hold and admits no removal', async () => {
  const f = backend(), client = await f.load(); client.local.drafts[key] = 'unowned destination';
  expect((await recover(client, f.native, f.storage, f.input)).status).toBe('retained');
  expect(f.state.hold).toBe(''); expect(f.state.requests.size).toBe(0); expect(client.local.drafts[key]).toBe('unowned destination');
});

test('unfinished recovery blocks actual draft Send, bind, discard and retarget admission', async () => {
  const f = backend(), client = await f.load(); let observed = false;
  f.hooks.write = async document => {
    if (observed || !Object.keys(obj(document.mobileOutboxDraftHandoffs)).length) return;
    observed = true; expect(blocked(client, key)).toBe(true);
    expect(mobileNewTaskDraftBind(client, key, 'editor')).toBe(false);
    expect(() => mobileNewTaskDraftDiscard(client, key)).toThrow('Finish restoring');
    expect(() => mobileNewTaskDraftRetarget(client, key, { origin: owner.origin, environmentId: 'env', projectId: 'other' })).toThrow('Finish restoring');
    expect((await mobileNewTaskSubmit(client, f.native, f.storage, { draftKey: key, now: 1, current: () => true })).status).toBe('blocked');
    Object.assign(client.local, { mobileOutboxDraftHandoffs: { ...obj(obj(client.local).mobileOutboxDraftHandoffs), other: { draftKey: 'env:thread' } } });
    await expect(client.write(f.native, f.storage, { method: 'orchestration.dispatchCommand', payload: { type: 'message.dispatch', threadId: 'thread', commandId: 'later', text: 'later' },
      description: 'Send', threadId: 'thread', text: 'later', uncertain: false })).rejects.toThrow('Finish restoring');
    delete obj(obj(client.local).mobileOutboxDraftHandoffs).other;
  };
  expect(await recover(client, f.native, f.storage, f.input)).toMatchObject({status:'recovered'}); expect(observed).toBe(true);
});

async function rootDiscover(f: ReturnType<typeof backend>, client: MobileDraftClient, now = 1000) {
  await rootAction(client, f.native, f.storage, 'read', '', now);
  const first = rootView(client, now); expect(first.next).not.toBe('');
  await rootAction(client, f.native, f.storage, 'deliver', first.next, now);
  const next = rootView(client, now); expect(JSON.parse(next.next).recovery).toEqual(owner);
  return next;
}
test('root timer discovers a rejected task offline and recovers it without provider traffic or navigation', async () => {
  const f = backend(), client = await f.load(), selected = [client.origin, client.environmentId, client.projectId, client.threadId, client.draftKey];
  const next = await rootDiscover(f, client);
  await rootAction(client, f.native, f.storage, 'deliver', next.next, 1000);
  expect(client.local.drafts[key]).toBe('recover me'); expect(f.state.row).toBeNull(); expect(f.state.proof).not.toBeNull();
  expect(rootView(client, 1000).next).toBe(''); expect(f.state.requests.size).toBe(1);
  expect([client.origin, client.environmentId, client.projectId, client.threadId, client.draftKey]).toEqual(selected);
  expect(f.calls.some(call => ['request', 'http', 'mobileSelectSavedEnvironment'].includes(str(call.op)))).toBe(false);
});
for (const lost of ['resumeRemoval', 'draftHandoffComplete']) test(`root restart discovers handoff after row deletion and lost ${lost} reply`, async () => {
  const f = backend(); let client = await f.load(); const first = await rootDiscover(f, client); let fired = false;
  f.hooks.after = async request => { if (!fired && request.action === lost) { fired = true; throw Error('reply lost'); } };
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 1000);
  expect(fired).toBe(true); expect(f.state.row).toBeNull(); expect(obj(client.local).mobileOutboxDraftHandoffs).toBeTruthy();
  if (lost === 'draftHandoffComplete') { client.local.drafts[key] = 'consumed destination'; await client.persist(f.storage); }
  f.hooks.after = undefined; f.state.hold = ''; f.state.epoch = 'cold'; client = await f.load();
  await rootAction(client, f.native, f.storage, 'read', '', 2000); expect(rootView(client, 2000).count).toBe(0);
  const next = rootView(client, 2000); expect(JSON.parse(next.next).recovery).toEqual(owner);
  await rootAction(client, f.native, f.storage, 'deliver', next.next, 2000);
  expect(rootView(client, 2000).next).toBe(''); expect(f.state.requests.size).toBe(1);
  expect(client.local.drafts[key]).toBe(lost === 'draftHandoffComplete' ? 'consumed destination' : 'recover me');
});
test('root recovery uses deadline backoff and rejects stale timer keys', async () => {
  const f = backend(), client = await f.load(), first = await rootDiscover(f, client);
  f.hooks.write = async () => { throw Error('disk unavailable'); };
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 1000);
  const second = rootView(client, 1000); expect(second.delay).toBe(1000); expect(second.next).not.toBe(first.next);
  const before = f.calls.length;
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 2000);
  await rootAction(client, f.native, f.storage, 'deliver', second.next, 1999); expect(f.calls.length).toBe(before);
  delete f.hooks.write; await rootAction(client, f.native, f.storage, 'deliver', second.next, 2000);
  expect(rootView(client, 2000).next).toBe(''); expect(client.local.drafts[key]).toBe('recover me');
});
test('root recovery rechecks editor ownership and refuses foreign or malformed saved ownership', async () => {
  const f = backend(), client = await f.load(), first = await rootDiscover(f, client);
  client.environmentId = 'other'; expect(rootView(client, 1000).next).toBe('');
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 1000); expect(f.state.row).not.toBeNull();
  client.environmentId = 'env';
  const row = mobileOutboxSnapshot(client).rows[0]!;
  mobilePendingTaskEditorsCreate(client, { version: 1, owner, session: 'editor', revision: 1, contentRevision: 1, draftKey: 'new-task:pending-message',
    baseline: { record, token: row.token, revision: row.nativeRevision! }, pending: null });
  expect(rootView(client, 1000).next).toBe('');
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 1000); expect(f.state.requests.size).toBe(0);
  Object.assign(client.local, { mobileOutboxDraftHandoffs: { corrupt: { record: { text: 'do not discard' } } } });
  expect(rootView(client, 1000).next).toBe('');
  expect((await rootAction(client, f.native, f.storage, 'retry', JSON.stringify(owner), 1000)).message).toContain('ownership');
});

test('backed-off recovery permits another owner and explicit retry resumes its exact handoff', async () => {
  const f = backend(), client = await f.load(), first = await rootDiscover(f, client);
  f.hooks.write = async () => { throw Error('disk unavailable'); };
  await rootAction(client, f.native, f.storage, 'deliver', first.next, 1000);
  const other = { ...record, threadId: 'other-thread', messageId: 'other-message', commandId: 'other-command' };
  const native: Native = { ...f.native, async later(request) {
    const answer = obj(await f.native.later(request));
    if (obj(request).op === 'mobileOutbox' && obj(request).action === 'read') {
      const value = obj(answer.value); value.records = [...value.records as Obj[], { record: other, token: 'epoch:9', revision: 1, pending: false, held: false }];
      value.revisions = { ...obj(value.revisions), 'other-message': 1 }; value.tokens = { ...obj(value.tokens), 'other-message': 'epoch:9' }; value.sequenceFloor = 9;
    }
    return answer;
  } };
  await rootAction(client, native, f.storage, 'read', '', 1001);
  expect(JSON.parse(rootView(client, 1001).next).owner.messageId).toBe('other-message');
  delete f.hooks.write;
  expect((await rootAction(client, f.native, f.storage, 'retry', JSON.stringify(owner), 1001)).message).toBe('');
  expect(client.local.drafts[key]).toBe('recover me'); expect(f.state.requests.size).toBe(1);
});


test('successful rejection recovery keeps its failed thread visible with an exact Edit task destination', async () => {
  const f = backend(), client = await f.load(); f.terminal.error = { kind: 'EnvironmentAuthorizationError', message: 'This environment cannot start tasks.' };
  const next = await rootDiscover(f, client);
  expect(mobileOutboxThread('env', 'thread', 1000, false, client)?.queuedFailed).toBe(false);
  await rootAction(client, f.native, f.storage, 'deliver', next.next, 1000);
  const view = mobileOutboxThread('env', 'thread', 1000, false, client)!;
  expect(view).toMatchObject({ queued: true, queuedFailed: true, queuedCanEdit: true, queuedCanRetry: false,
    queuedStatus: 'Could not start task', queuedReason: 'This environment cannot start tasks.', rows: [{ body: 'recover me' }] });
  expect(view.composer.canSend).toBe(false); expect(mobileOutboxPendingTasks(client, 1000)).toEqual([]);
  const before = f.calls.length, selected = [client.environmentId, client.projectId, client.threadId];
  expect(editRecovered(client, mobileOutboxOwner(record))).toMatchObject({ message: '',
    nextLocation: '/new/draft?environmentId=env&projectId=project&draftId=new-task%3Arestored-message' });
  expect(f.calls.length).toBe(before); expect([client.environmentId, client.projectId, client.threadId]).toEqual(selected);
  expect(mobileOutboxThread('other', 'thread', 1000, false, client)).toBeNull();
  client.shell.threads = [{ id: 'thread' }]; expect(mobileOutboxThread('env', 'thread', 1000, false, client)).toBeNull();
});
test('failed or incomplete recovery never claims that the prompt is in a project draft', async () => {
  const f = backend(), client = await f.load(), next = await rootDiscover(f, client);
  f.hooks.write = async () => { throw Error('disk unavailable'); };
  await rootAction(client, f.native, f.storage, 'deliver', next.next, 1000);
  expect(mobileOutboxThread('env', 'thread', 1000, false, client)?.queuedFailed).toBe(false);
  expect(editRecovered(client, mobileOutboxOwner(record)).nextLocation).toBe('');
});
for (const mode of ['consumed', 'retargeted', 'forged-owner'] as const) test(`Edit task cannot recreate or replace a ${mode} recovered destination`, async () => {
  const f = backend(), client = await f.load(), next = await rootDiscover(f, client);
  await rootAction(client, f.native, f.storage, 'deliver', next.next, 1000);
  const store = mobileNewTaskDraftStore(client), original = copy(store.records[key]);
  if (mode === 'consumed') { delete store.records[key]; delete client.local.drafts[key]; }
  if (mode === 'retargeted') store.records[key].origin = 'https://replacement.test';
  const requested = mode === 'forged-owner' ? mobileOutboxOwner({ ...record, commandId: 'other' }) : mobileOutboxOwner(record);
  const before = f.calls.length, draftBefore = client.local.drafts[key];
  expect(editRecovered(client, requested).nextLocation).toBe('');
  expect(f.calls.length).toBe(before); expect(client.local.drafts[key]).toBe(draftBefore);
  if (mode === 'consumed') expect(store.records[key]).toBeUndefined();
  if (mode === 'retargeted') expect(store.records[key]).toEqual({ ...original, origin: 'https://replacement.test' });
  expect(mobileOutboxThread('env', 'thread', 1000, false, client)?.queuedCanEdit).toBe(mode === 'forged-owner');
});
