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
      if (request.action === 'status') value = { operation: terminal, durable: true };
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
    } else if (request.op === 'mobileOutbox') {
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
  return { state, native, storage, hooks, calls, load, input: { owner, current: () => true } };
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
