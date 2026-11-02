import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Files, type Native } from './shared/protocol';
import { mobilePendingInput, mobilePendingInputAction, mobilePendingInputCommand, mobilePendingInputRetry, mobilePendingInputObserveRoute, type PendingInputRoute } from './pending-input';
import { questionJournalHome, questionJournalHydrate, questionRecordForPending, questionJournalStatus, questionJournalRefresh } from './pending-input-journal';
import { MobileDraftClient } from './mobile-draft-recovery';

const gate = () => { let resolve!: () => void; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; };
/** Native protocol fixture for actual core exports. Filesystem/durability proof is the Swift owner batch. */
function fixture(client = new T3Client(), saved: Obj = {}) {
  Object.assign(client, { origin: 'https://one.test', environmentId: 'env', threadId: 'thread', projectId: 'project', generation: 1,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  client.local.drafts[client.draftKey] = 'Keep ordinary draft';
  const question = (id: string, extra: Obj = {}) => ({ id, header: id, question: `${id}?`, options: [{ label: 'One', value: 'one' }, { label: ' Two ' }], ...extra });
  client.thread = { projection: { thread: { id: 'thread' }, runs: [{ id: 'run', status: 'running' }],
    runtimeRequests: [{ id: 'request', kind: 'user_input', status: 'pending', createdAt: 'today', responseCapability: { type: 'live' } }],
    turnItems: [{ id: 'item', type: 'user_input_request', requestId: 'request', responseMode: 'message', questions: [question('first'), question('multi', { multiSelect: true }), question('custom', { options: [] })] }] } };
  const route: PendingInputRoute = { active: true, visit: 'visit', url: '/threads/env/thread', environmentId: 'env', threadId: 'thread' };
  const calls: Obj[] = [], payloads: Obj[] = [];
  let serial = 0, consumeSuccess = true, persistentSaveFailure = false;
  let disk: Obj = structuredClone(saved), fail = false, outcomeSaveFailure = false, retainedSuccess = false, retainedCommand = 'command';
  let onNative = async (_: Obj) => {}, onSave = async (_: Obj) => {}, beforeSave = async (_: Obj) => {};
  const initialize = () => {
    disk.mobileQuestionCommands ??= {}; disk.mobileQuestionMetadata ??= { epoch: 'epoch', process: 'fixture-process', revision: 0, commands: {}, prepared: {}, tombstones: {}, mutations: {} };
  };
  const state = () => ({ ...structuredClone(disk), verifiedHome: { home: 'https://one.test', endpoint: client.origin, environmentId: 'env', generation: client.generation } });
  const native: Native = { available: true, watch() {}, async later(raw) {
    const r = obj(raw); calls.push(structuredClone(r)); await onNative(r); initialize();
    const records = obj(disk.mobileQuestionCommands), journal = obj(disk.mobileQuestionMetadata), commands = obj(journal.commands);
    if (r.op === 'ids') return { ok: true, generation: client.generation, value: [serial++ ? `command-${serial}` : 'command'] };
    if (r.op === 'mobileQuestionJournal') {
      const id = str(r.commandId), action = str(r.action), record = obj(records[id]), metadata = obj(commands[id]);
      if (action === 'read' && retainedSuccess) {
        if (persistentSaveFailure) return { ok: false, generation: 0, error: { kind: 'Persistence', message: 'Outcome save still failed' } };
        obj(records[retainedCommand]).phase = 'settled'; retainedSuccess = false;
      }
      if (action !== 'read') await beforeSave(r);
      if (action === 'prepare') obj(journal.prepared)[str(obj(obj(r.record).pending).payload && obj(obj(obj(r.record).pending).payload).commandId)] = structuredClone(obj(r.record));
      if (action === 'create') {
        records[id] = structuredClone(obj(journal.prepared)[id]); delete obj(journal.prepared)[id];
        commands[id] = { recordRevision: 0, recordDigest: `record-${id}`, payloadDigest: `payload-${id}`, homeOrigin: 'https://one.test' };
      }
      if (action === 'phase' || action === 'terminal') record.phase = r.phase;
      if (action === 'terminal') metadata.releasePending = true;
      if (action === 'release') metadata.releaseToken = `release-${id}`;
      if (action === 'ack') metadata.releaseAck = true;
      if (action === 'remove') { obj(journal.tombstones)[id] = { recordDigest: metadata.recordDigest, payloadDigest: metadata.payloadDigest,
        releaseToken: metadata.releaseToken, epoch: journal.epoch, process: journal.process }; delete records[id]; delete commands[id]; }
      if (action !== 'read') { journal.revision = Number(journal.revision) + 1; metadata.recordRevision = Number(metadata.recordRevision ?? 0) + 1;
        obj(journal.mutations)[str(r.mutationId)] = { digest: 'a'.repeat(64), epoch: journal.epoch, revision: journal.revision, commandId: id, action };
        await onSave(r); }
      return { ok: true, generation: client.generation, value: state() };
    }
    if (r.op === 'request' && r.question) {
      const id = str(obj(r.question).commandId), record = obj(records[id]);
      if (!fail && consumeSuccess && obj(obj(record.pending).payload).type !== 'run.interrupt') client.projection.runtimeRequests = [];
      payloads.push(structuredClone(obj(obj(record.pending).payload)));
      if (outcomeSaveFailure) {
        outcomeSaveFailure = false; retainedSuccess = true; retainedCommand = id;
        return { ok: false, generation: client.generation, error: { kind: 'Persistence', message: 'Outcome save failed', uncertain: true } };
      }
      record.phase = fail ? 'uncertain' : 'settled';
      if (fail) return { ok: false, generation: client.generation, error: { kind: 'RPC', message: 'Response uncertain', uncertain: true } };
      return { ok: true, generation: client.generation, value: state() };
    }
    return { ok: true, generation: client.generation, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(JSON.stringify(disk)).buffer; }, async atomicWriteFile(_path, bytes) {
    const incoming = obj(JSON.parse(new TextDecoder().decode(bytes))), pending = { ...obj(incoming.pending) };
    const owned = new Set([...Object.keys(obj(disk.mobileQuestionCommands)), ...Object.keys(obj(obj(disk.mobileQuestionMetadata).tombstones))]);
    for (const [env, entry] of Object.entries(pending)) if (owned.has(str(obj(obj(entry).payload).commandId))) delete pending[env];
    disk = { ...incoming, pending, mobileQuestionCommands: disk.mobileQuestionCommands!, mobileQuestionMetadata: disk.mobileQuestionMetadata! };
  } } };
  const view = () => mobilePendingInput(client, route);
  const act = (op: string, id = '', text = '', owner = view().owner) => mobilePendingInputAction(client, native, storage, { ...route }, owner, op, id, text);
  const fill = async () => { await act('custom', 'custom', '  exact café 🧪  '); await act('option', 'multi', 'one'); await act('option', 'first', 'Two'); };
  questionJournalStatus(client, { origin: client.origin, homeOrigin: 'https://one.test', environmentId: 'env' }, client.generation);
  if (Object.keys(saved).length) questionJournalHydrate(client, state());
  return { client, native, storage, route, calls, payloads, view, act, fill, disk: () => structuredClone(disk),
    intercept: (fn: typeof onNative) => { onNative = fn; }, duringSave: (fn: typeof onSave) => { onSave = fn; }, beforeSave: (fn: typeof beforeSave) => { beforeSave = fn; },
    changeDisk: (fn: (value: Obj) => void) => { fn(disk); },
    fail: () => { fail = true; }, failOutcome: (persistent = false) => { outcomeSaveFailure = true; persistentSaveFailure = persistent; },
    recover: () => { persistentSaveFailure = false; fail = false; }, keepPending: () => { consumeSuccess = false; } };
}

describe('actual question core and native operation boundary', () => {
  test('all answers produce one immutable marked request and preserve ordinary draft', async () => {
    const f = fixture(); await f.fill(); await f.act('option', 'multi', 'Two'); await f.act('option', 'multi', 'one'); await f.act('submit');
    expect(f.payloads).toEqual([{ type: 'runtime-request.respond', threadId: 'thread', requestId: 'request', answers: { first: 'Two', multi: ['Two'], custom: 'exact café 🧪' }, commandId: 'command' }]);
    const rpc = f.calls.find(r => r.op === 'request')!; expect(rpc.payload).toBeUndefined(); expect(rpc.question).toMatchObject({ commandId: 'command', payloadDigest: 'payload-command' });
    expect(f.calls.filter(r => r.op === 'mobileQuestionJournal').map(r => r.action)).toEqual(['read', 'prepare', 'create', 'bind', 'phase', 'terminal', 'release', 'ack', 'remove']);
    expect(f.client.draft).toBe('Keep ordinary draft'); expect(f.client.pending).toBeUndefined();
  });
  test('choices clear only their custom value; local custom remains untrimmed', async () => {
    const f = fixture(); await f.act('custom', 'first', 'Custom'); await f.act('option', 'first', 'one');
    expect(f.view().questions[0]!.answer).toBe(''); await f.act('custom', 'custom', '  keep  '); expect(f.view().questions[2]!.answer).toBe('  keep  '); expect(f.calls).toEqual([]);
  });
  test.each(['read-only', 'reconnect'])('%s retains edits and refuses native wire actions', async kind => {
    const f = fixture(); if (kind === 'read-only') f.client.scopes = []; else { f.client.connection = 'reconnecting'; f.client.threadLive = false; }
    await f.fill(); for (const op of ['submit', 'dismiss', 'stop']) await f.act(op);
    expect(f.calls).toEqual([]); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  ');
  });
  test('route token and request signature reject old local callbacks', async () => {
    const f = fixture(), owner = f.view().owner;
    await mobilePendingInputCommand(f.client, f.native, f.storage, owner, 'custom:custom', 'stale', 'other\n/threads/env/thread');
    expect(f.view().questions[2]!.answer).toBe(''); arr(f.client.projection.turnItems)[0]!.questions = [{ id: 'replacement', question: 'New?', options: [] }];
    await f.act('custom', 'custom', 'old', owner); expect(f.view().questions[0]!.answer).toBe('');
  });
  test.each(['dismiss', 'stop'])('%s sends no attachment map and keeps question-local draft', async action => {
    const f = fixture(); f.keepPending(); await f.fill(); await f.act(action);
    expect(f.payloads[0]!.type).toBe(action === 'stop' ? 'run.interrupt' : 'thread.user-input.dismiss');
    expect(f.payloads[0]!.attachmentsByQuestionId).toBeUndefined(); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  ');
    if (action === 'stop') expect(f.payloads[0]).toMatchObject({ runId: 'run', holdQueue: true });
  });
  test.each(['capability', 'route', 'signature'])('every awaited native preparation is guarded against %s changes', async kind => {
    const f = fixture(); await f.fill(); f.intercept(async r => { if (r.op === 'mobileQuestionJournal' && r.action === 'prepare') {
      if (kind === 'capability') f.client.scopes = [];
      if (kind === 'route') { f.route.visit = 'new-visit'; f.view(); }
      if (kind === 'signature') arr(f.client.projection.turnItems)[0]!.id = 'new-item';
    } });
    await f.act('submit'); expect(f.payloads).toEqual([]); expect(f.client.draft).toBe('Keep ordinary draft');
  });
  test('duplicate submit refuses while native attempt runs; late success preserves ordinary successor', async () => {
    const f = fixture(), entered = gate(), release = gate(); await f.fill();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
    const task = f.act('submit'); await entered.promise; await f.act('submit'); await f.act('custom', 'custom', 'During attempt');
    const successor = { method: 'orchestration.dispatchCommand', payload: { commandId: 'ordinary', type: 'thread.rename' }, description: 'Keep', threadId: 'other', text: '', uncertain: true };
    f.client.local.pending.env = successor; release.resolve(); await task;
    expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBe(successor); expect(f.client.draft).toBe('Keep ordinary draft');
  });
  test('uncertain receipt Retry uses captured payload without allocating a new command ID', async () => {
    const f = fixture(); await f.fill(); f.fail(); await f.act('submit');
    const next = fixture(new T3Client(), f.disk()); expect(next.view().questions[2]!.answer).toBe('  exact café 🧪  ');
    await mobilePendingInputRetry(next.client, next.native, next.storage);
    expect(next.calls.filter(r => r.op === 'ids')).toEqual([]); expect(next.payloads).toEqual(f.payloads); expect(next.client.pending).toBeUndefined();
  });
  test('native success after outcome-save failure is save-only, with no second request', async () => {
    const f = fixture(); await f.fill(); f.failOutcome(); await f.act('submit');
    expect(f.payloads).toHaveLength(1); expect(Object.keys(obj(f.disk().mobileQuestionCommands))).toEqual([]);
  });
  test('let-go before request creates no dispatch and makes no cleanup call through abandoned wrapper', async () => {
    const f = fixture(); await f.fill(); f.intercept(async r => { if (r.op === 'mobileQuestionJournal' && r.action === 'bind') throw new ClientError('gone', 'superseded'); });
    await expect(f.act('submit')).rejects.toThrow('gone'); expect(f.payloads).toEqual([]);
    expect(f.calls.at(-1)).toMatchObject({ op: 'mobileQuestionJournal', action: 'bind' });
  });
  test('authoritative consumption is required when the card disappears', async () => {
    const f = fixture(new MobileDraftClient()); f.fail(); await f.act('stop'); f.client.projection.runtimeRequests = [];
    f.client.threadLive = false; expect(f.client.reconcilePending()).toBe(false); expect(questionRecordForPending(f.client)?.phase).toBe('uncertain');
    f.client.threadLive = true; f.client.projection.runs = [{ id: 'run', status: 'completed' }];
    await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  });
  test.each(['preparing', 'starting', 'running', 'waiting'])('Stop retries captured %s run beside newer active run', async status => {
    const first = fixture(); arr(first.client.projection.runs)[0]!.status = status; first.fail(); await first.act('stop');
    const next = fixture(new MobileDraftClient(), first.disk()); next.client.projection.runs = [{ id: 'run', status }, { id: 'newer', status: 'running' }]; next.view();
    expect(next.client.reconcilePending()).toBe(false); await mobilePendingInputRetry(next.client, next.native, next.storage); expect(next.payloads).toEqual(first.payloads);
  });
  test('home stays stable across active route changes without changing command origin bytes', async () => {
    const f = fixture(); questionJournalStatus(f.client, { origin: f.client.origin, homeOrigin: 'https://home.test', environmentId: 'env' }, f.client.generation);
    await f.fill(); const card = f.view().cardId; f.client.origin = 'https://other-route.test'; f.client.generation++;
    questionJournalStatus(f.client, { origin: f.client.origin, homeOrigin: 'https://home.test', environmentId: 'env' }, f.client.generation);
    expect(f.view().cardId).toBe(card); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  ');
  });
});


const successor = () => ({ method: 'orchestration.dispatchCommand', payload: { type: 'thread.rename', commandId: 'successor', threadId: 'other', title: 'Keep' }, description: 'successor', threadId: 'other', text: '', uncertain: true });
describe('restored lifecycle behavior through actual core exports', () => {
  test('leave and return retains drafts, observed consumption resets a reused request incarnation', async () => {
    const f = fixture(); await f.fill(); const old = f.view().owner;
    f.route.active = false; expect(f.view().active).toBe(false); f.route.active = true; expect(f.view().canSubmit).toBe(true);
    const requests = f.client.projection.runtimeRequests; f.client.projection.runtimeRequests = []; expect(f.view().active).toBe(false);
    f.client.projection.runtimeRequests = requests; arr(f.client.projection.turnItems)[0]!.id = 'later-item';
    expect(f.view().owner).not.toBe(old); expect(f.view().canSubmit).toBe(false);
    await f.act('custom', 'custom', 'stale', old); expect(f.view().questions[2]!.answer).toBe('');
  });
  test('unresumable blocks edits and message capability allows explicit Dismiss', async () => {
    const f = fixture(); obj(arr(f.client.projection.runtimeRequests)[0]!.responseCapability).type = 'not_resumable';
    expect(f.view().editable).toBe(false); await f.act('custom', 'custom', 'no'); expect(f.view().questions[2]!.answer).toBe('');
    obj(arr(f.client.projection.runtimeRequests)[0]!.responseCapability).type = 'message'; expect(f.view().editable).toBe(true);
    await f.act('dismiss'); expect(f.payloads[0]!.type).toBe('thread.user-input.dismiss');
  });
  test.each(['success', 'definite', 'uncertain', 'cancel'])('old RPC %s never changes a replacement receipt or ordinary pending', async outcome => {
    const f = fixture(), entered = gate(), release = gate(), next = successor(); await f.fill();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise;
      if (outcome === 'definite') throw new ClientError('refused');
      if (outcome === 'uncertain') throw new ClientError('lost', 'transport', true);
      if (outcome === 'cancel') throw new ClientError('gone', 'superseded');
    } });
    const task = f.act('submit'); await entered.promise; const old = questionRecordForPending(f.client)!;
    const replacement = { ...old, pending: next }; obj(f.client.local).mobileQuestionCommands = { command: replacement }; f.client.local.pending.env = next;
    release.resolve(); if (outcome === 'cancel') await expect(task).rejects.toThrow('gone'); else await task;
    expect(f.client.pending).toBe(next); expect(obj(obj(f.client.local).mobileQuestionCommands).command).toBe(replacement);
  });
  test('Stop in flight permits local editing and retains the still pending request', async () => {
    const f = fixture(), entered = gate(), release = gate();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
    const task = f.act('stop'); await entered.promise; expect(f.view()).toMatchObject({ editable: true, responding: false, canStop: false, canSubmit: false });
    await f.act('custom', 'custom', 'during Stop'); release.resolve(); await task;
    expect(f.view().questions[2]!.answer).toBe('during Stop'); expect(arr(f.client.projection.runtimeRequests)[0]!.status).toBe('pending');
  });
  test.each(['release', 'ack', 'remove'])('failed %s leaves save-only Retry usable after card disappearance', async action => {
    const f = fixture(); await f.fill(); let once = true;
    f.duringSave(async r => { if (r.action === action && once) { once = false; throw new Error('cleanup disk'); } });
    await f.act('submit'); expect(questionRecordForPending(f.client)?.phase).toBe('settled'); expect(f.view().active).toBe(false);
    await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  });
  test('successor during failed cleanup keeps its pending slot', async () => {
    const f = fixture(), next = successor(); await f.fill();
    f.duringSave(async r => { if (r.action === 'release') { f.client.local.pending.env = next; throw new Error('cleanup disk'); } });
    await f.act('submit'); expect(f.client.pending).toBe(next);
  });
  test('persistent outcome save failure retains receipt until explicit save-only Retry', async () => {
    const f = fixture(); f.keepPending(); await f.fill(); f.failOutcome(true); const result = await f.act('submit');
    expect(result.message).toContain('Outcome save'); expect(f.payloads).toHaveLength(1); expect(questionRecordForPending(f.client)).toBeDefined();
    const failed = await mobilePendingInputRetry(f.client, f.native, f.storage); expect(failed?.message).toContain('Outcome save'); expect(f.payloads).toHaveLength(1);
    f.recover(); f.client.projection.runtimeRequests = []; await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  });
  test('generic successful RPC keeps a pending request until complete consumption', async () => {
    const f = fixture(); f.keepPending(); await f.fill(); await f.act('submit');
    expect(questionRecordForPending(f.client)?.phase).toBe('settled'); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  ');
    expect(f.calls.some(r => r.action === 'remove')).toBe(false);
    f.client.projection.runtimeRequests = []; await mobilePendingInputRetry(f.client, f.native, f.storage);
    expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  });
  test('complete successor item consumes only the old receipt and shows a new empty catalog', async () => {
    const f = fixture(new MobileDraftClient()); await f.fill(); f.fail(); await f.act('submit');
    arr(f.client.projection.turnItems)[0]!.id = 'later-item'; arr(f.client.projection.turnItems)[0]!.questions = [{ id: 'new', question: 'Later?', options: [] }];
    f.view(); await mobilePendingInputRetry(f.client, f.native, f.storage);
    expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined(); expect(f.view().questions).toHaveLength(1); expect(f.view().questions[0]!.answer).toBe('');
  });
  test('Retry refuses a route invalidated during its awaited native phase', async () => {
    const first = fixture(); await first.fill(); first.fail(); await first.act('submit');
    const f = fixture(new T3Client(), first.disk()); f.view();
    f.intercept(async r => { if (r.action === 'phase') mobilePendingInputObserveRoute(f.client, { ...f.route, active: false }); });
    await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.payloads).toEqual([]);
  });
  test('completed captured Stop settles beside a newer active run without another RPC', async () => {
    const first = fixture(); first.fail(); await first.act('stop'); const f = fixture(new MobileDraftClient(), first.disk());
    f.client.projection.runs = [{ id: 'run', status: 'completed' }, { id: 'newer', status: 'waiting' }]; f.view();
    expect(f.client.reconcilePending()).toBe(false); await mobilePendingInputRetry(f.client, f.native, f.storage);
    expect(f.payloads).toEqual([]); expect(f.client.pending).toBeUndefined(); expect(f.view().active).toBe(true);
  });
  test('ordinary persistence excludes the native pending command while preserving its receipt', async () => {
    const f = fixture(new MobileDraftClient()), entered = gate(), release = gate(); await f.fill();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
    const task = f.act('submit'); await entered.promise; await f.client.persist(f.storage);
    expect(obj(f.disk().pending).env).toBeUndefined(); expect(Object.keys(obj(f.disk().mobileQuestionCommands))).toEqual(['command']); release.resolve(); await task;
  });
  test('preference home fields never establish transport identity, and missing generation proof stays unresolved', () => {
    const f = fixture(); f.client.generation++;
    questionJournalHydrate(f.client, { homeOrigin: 'https://forged.test', origin: f.client.origin, environmentId: 'env', generation: f.client.generation });
    expect(questionJournalHome(f.client)).toBe('');
    questionJournalStatus(f.client, { origin: f.client.origin, homeOrigin: 'https://native.test', environmentId: 'env' }, f.client.generation - 1);
    expect(questionJournalHome(f.client)).toBe('');
  });
});


describe('final native cleanup confirmation and restored guards', () => {
  test('unknown question and option IDs preserve the entire local form', async () => {
    const f = fixture(); await f.fill(); const prior = f.view().questions;
    expect((await f.act('option', 'unknown', 'one')).message).toContain('changed');
    expect((await f.act('option', 'first', 'unknown')).message).toContain('changed');
    expect((await f.act('custom', 'unknown', 'text')).message).toContain('changed');
    expect(f.view().questions).toEqual(prior); expect(f.calls).toEqual([]);
  });
  test('clearing custom text does not revive a discarded option selection', async () => {
    const f = fixture(); await f.act('option', 'first', 'one'); await f.act('custom', 'first', 'custom'); await f.act('custom', 'first', '');
    expect(f.view().questions[0]!.options.every(option => !option.selected)).toBe(true); expect(f.view().complete).toBe(false);
  });
  test('card identity survives capability and generation changes while old action owners refuse', async () => {
    const f = fixture(); await f.fill(); const prior = f.view(); f.client.scopes = []; f.client.generation++;
    questionJournalStatus(f.client, { origin: f.client.origin, homeOrigin: 'https://one.test', environmentId: 'env' }, f.client.generation);
    expect(f.view().cardId).toBe(prior.cardId); expect(f.view().owner).not.toBe(prior.owner); expect(f.view().canSubmit).toBe(false);
    await f.act('custom', 'custom', 'stale', prior.owner); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  '); expect(f.calls).toEqual([]);
  });
  test.each(['success', 'failure'])('ordinary successor during initial persistence %s remains owned', async outcome => {
    const f = fixture(), next = successor(); await f.fill();
    f.duringSave(async r => { if (r.action === 'create') { f.client.local.pending.env = next; if (outcome === 'failure') throw new Error('initial save'); } });
    await f.act('submit'); expect(f.client.pending).toBe(next); expect(f.payloads).toEqual([]); expect(f.client.draft).toBe('Keep ordinary draft');
    expect(obj(f.disk().mobileQuestionCommands).command).toBeDefined();
  });
  const rows = ['release', 'ack', 'remove'].flatMap(action => ['before-visible', 'visible-lost-ack'].flatMap(boundary =>
    ['transient', 'persistent'].flatMap(failure => ['submit', 'stop'].map(operation => ({ action, boundary, failure, operation })))));
  test.each(rows)('$operation $action $boundary $failure requires manual save-only Retry', async row => {
    const f = fixture(); await f.fill(); let armed = true, errors = 0;
    const fault = async (r: Obj) => { if (r.action === row.action && armed) { errors++; if (row.failure === 'transient') armed = false; throw new Error('cleanup disk'); } };
    if (row.boundary === 'before-visible') f.beforeSave(fault); else f.duringSave(fault);
    const result = await f.act(row.operation);
    expect(errors).toBe(1); expect(questionRecordForPending(f.client)?.phase).toBe('settled'); expect(f.payloads).toHaveLength(1);
    expect(f.view().active).toBe(row.operation === 'stop'); if (row.operation === 'stop') expect(result.message).toContain('Use Retry');
    const calls = f.calls.length; await questionJournalRefresh(f.client, f.native, f.storage);
    expect(f.calls).toHaveLength(calls + 1); expect(f.calls.at(-1)?.action).toBe('read'); expect(questionRecordForPending(f.client)?.phase).toBe('settled');
    const first = await mobilePendingInputRetry(f.client, f.native, f.storage);
    const committedRemove = row.action === 'remove' && row.boundary === 'visible-lost-ack';
    if (row.failure === 'persistent' && !committedRemove) {
      expect(first?.message).toContain('Use Retry'); expect(questionRecordForPending(f.client)?.phase).toBe('settled'); expect(f.payloads).toHaveLength(1);
      armed = false; const later = await mobilePendingInputRetry(f.client, f.native, f.storage); expect(later?.message).toBe('');
    } else expect(first?.message).toBe('');
    expect(f.client.pending).toBeUndefined(); expect(f.payloads).toHaveLength(1); expect(Object.keys(obj(f.disk().mobileQuestionCommands))).toEqual([]);
  });
  test.each(['absence', 'record', 'payload', 'token', 'epoch', 'process', 'pin', 'mutation', 'cas', 'replacement'])('Retry refuses %s instead of accepting unrelated removal proof', async kind => {
    const f = fixture(); await f.fill(); let once = true;
    f.duringSave(async r => { if (r.action === 'remove' && once) { once = false; throw new Error('lost remove'); } });
    await f.act('stop'); const saved = questionRecordForPending(f.client)!;
    f.changeDisk(disk => {
      const journal = obj(disk.mobileQuestionMetadata), tombstone = obj(obj(journal.tombstones).command);
      if (kind === 'absence') journal.tombstones = {};
      if (kind === 'record') tombstone.recordDigest = 'different';
      if (kind === 'payload') tombstone.payloadDigest = 'different';
      if (kind === 'token') tombstone.releaseToken = 'different';
      if (kind === 'epoch') tombstone.epoch = 'new-epoch';
      if (kind === 'process') tombstone.process = 'new-process';
      if (kind === 'pin') tombstone.pinId = 'different-pin';
      if (kind === 'mutation') journal.mutations = {};
      if (kind === 'cas') for (const value of Object.values(obj(journal.mutations))) if (obj(value).action === 'remove') obj(value).revision = 999;
      if (kind === 'replacement') { obj(disk.mobileQuestionCommands).command = { ...saved, pending: { ...saved.pending, payload: { ...saved.pending.payload, runId: 'replacement' } } }; obj(journal.commands).command = { recordRevision: 1, recordDigest: 'replacement', payloadDigest: 'replacement', homeOrigin: 'https://one.test' }; }
    });
    const result = await mobilePendingInputRetry(f.client, f.native, f.storage); expect(result?.message).toContain('changed'); expect(f.payloads).toHaveLength(1);
  });
  test('abandoned committed remove confirms exactly on a fresh Retry', async () => {
    const f = fixture(); await f.fill(); let once = true;
    f.duringSave(async r => { if (r.action === 'remove' && once) { once = false; throw new ClientError('gone', 'superseded'); } });
    await expect(f.act('stop')).rejects.toThrow('gone'); expect(questionRecordForPending(f.client)?.phase).toBe('settled');
    const result = await mobilePendingInputRetry(f.client, f.native, f.storage); expect(result?.message).toBe(''); expect(f.payloads).toHaveLength(1); expect(f.client.pending).toBeUndefined();
  });
  test.each(['release', 'ack', 'remove'])('ordinary successor during %s failure is never retired by Retry confirmation', async action => {
    const f = fixture(), next = successor(); await f.fill();
    f.duringSave(async r => { if (r.action === action) { f.client.local.pending.env = next; throw new Error('lost cleanup'); } });
    await f.act('stop'); expect(f.client.pending).toBe(next); expect(await mobilePendingInputRetry(f.client, f.native, f.storage)).toBeNull(); expect(f.client.pending).toBe(next); expect(f.payloads).toHaveLength(1);
  });
});


describe('cold cleanup receipts and ordinary successor ownership', () => {
  const rows = ['release', 'ack', 'remove'].flatMap(action => ['before-visible', 'visible-lost-ack'].flatMap(boundary =>
    ['submit', 'stop'].map(operation => ({ action, boundary, operation }))));
  test.each(rows)('cold $operation $action $boundary reconstructs native facts without warm failure state', async row => {
    const first = fixture(); await first.fill(); let armed = true;
    const fault = async (r: Obj) => { if (r.action === row.action && armed) { armed = false; throw new Error('cleanup disk'); } };
    if (row.boundary === 'before-visible') first.beforeSave(fault); else first.duringSave(fault);
    await first.act(row.operation); expect(questionRecordForPending(first.client)?.phase).toBe('settled');
    const next = fixture(new T3Client(), first.disk()); next.client.thread = { projection: structuredClone(first.client.projection) }; next.view();
    const committed = row.action === 'remove' && row.boundary === 'visible-lost-ack';
    if (committed) { expect(next.client.pending).toBeUndefined(); expect(await mobilePendingInputRetry(next.client, next.native, next.storage)).toBeNull(); }
    else { expect(questionRecordForPending(next.client)?.phase).toBe('settled'); expect((await mobilePendingInputRetry(next.client, next.native, next.storage))?.message).toBe(''); expect(next.client.pending).toBeUndefined(); }
    expect(next.payloads).toEqual([]); expect(next.calls.filter(r => r.op === 'ids')).toEqual([]); expect(next.view().error).toBe('');
    expect(Object.keys(obj(next.disk().mobileQuestionCommands))).toEqual([]);
  });
  test.each(['release', 'ack', 'remove'])('cold ordinary successor survives retained %s cleanup failure', async action => {
    const first = fixture(), ordinary = successor(); await first.fill();
    first.beforeSave(async r => { if (r.action === action) throw new Error('cleanup disk'); });
    await first.act('stop'); first.client.local.pending.env = ordinary; await first.client.persist(first.storage);
    const client = new T3Client(); client.local.pending.env = ordinary; const next = fixture(client, first.disk()); next.view();
    expect(next.client.pending).toBe(ordinary); expect(questionRecordForPending(next.client)).toBeUndefined();
    expect(await mobilePendingInputRetry(next.client, next.native, next.storage)).toBeNull(); expect(next.client.pending).toBe(ordinary); expect(next.payloads).toEqual([]);
    expect(obj(obj(next.client.local).mobileQuestionCommands).command).toBeDefined();
  });
});


describe('automatic read preserves only unconfirmed cleanup ownership', () => {
  test('ordinary successor survives a warm read while its failed terminal receipt remains Retry-owned', async () => {
    const f = fixture(), ordinary = successor(); await f.fill();
    f.beforeSave(async r => { if (r.action === 'release') throw new Error('release disk'); });
    await f.act('stop'); const retained = questionRecordForPending(f.client)!; f.client.local.pending.env = ordinary;
    await questionJournalRefresh(f.client, f.native, f.storage);
    expect(f.calls.at(-1)?.action).toBe('read'); expect(f.client.pending).toBe(ordinary);
    expect(obj(obj(f.client.local).mobileQuestionCommands).command).toBe(retained); expect(f.payloads).toHaveLength(1);
    expect(await mobilePendingInputRetry(f.client, f.native, f.storage)).toBeNull(); expect(f.client.pending).toBe(ordinary);
    await f.client.write(f.native, f.storage, ordinary); expect(f.client.pending).toBeUndefined();
    expect(f.calls.some(r => r.op === 'request' && obj(r.payload).commandId === 'successor')).toBe(true);
    expect(obj(obj(f.client.local).mobileQuestionCommands).command).toBe(retained);
  });
  test('another environment reads and adopts its receipt beside a failed cleanup owner', async () => {
    const f = fixture(); await f.fill(); f.beforeSave(async r => { if (r.action === 'release') throw new Error('release disk'); });
    await f.act('stop'); const retained = questionRecordForPending(f.client)!;
    f.changeDisk(disk => {
      const record = structuredClone(retained); record.environmentId = 'other'; record.threadId = 'other-thread'; record.phase = 'uncertain';
      record.pending = { ...record.pending, threadId: 'other-thread', payload: { ...record.pending.payload, commandId: 'other-command', threadId: 'other-thread', runId: 'other-run' } };
      obj(disk.mobileQuestionCommands)['other-command'] = record as unknown as Obj;
      obj(obj(disk.mobileQuestionMetadata).commands)['other-command'] = { recordRevision: 0, recordDigest: 'other-record', payloadDigest: 'other-payload', homeOrigin: 'https://one.test' };
    });
    f.client.environmentId = 'other'; f.client.threadId = 'other-thread';
    questionJournalStatus(f.client, { origin: f.client.origin, homeOrigin: 'https://one.test', environmentId: 'other' }, f.client.generation);
    await questionJournalRefresh(f.client, f.native, f.storage);
    expect(f.calls.at(-1)?.action).toBe('read'); expect(questionRecordForPending(f.client)?.pending.payload.commandId).toBe('other-command');
    expect(f.client.local.pending.env).toBe(retained.pending); expect(obj(obj(f.client.local).mobileQuestionCommands).command).toBe(retained);
    expect(f.payloads).toHaveLength(1);
  });
  test('cold local clone cannot claim completion without captured removal/CAS evidence', async () => {
    const first = fixture(); await first.fill(); first.duringSave(async r => { if (r.action === 'remove') throw new Error('lost reply'); });
    await first.act('stop'); const localSnapshot = { mobileQuestionCommands: structuredClone(obj(first.client.local).mobileQuestionCommands), mobileQuestionMetadata: structuredClone(obj(first.client.local).mobileQuestionMetadata) };
    const next = fixture(new T3Client(), first.disk()); questionJournalHydrate(next.client, localSnapshot); next.view();
    expect(questionRecordForPending(next.client)).toBeDefined(); expect((await mobilePendingInputRetry(next.client, next.native, next.storage))?.message).toContain('changed'); expect(next.payloads).toEqual([]);
  });
});
