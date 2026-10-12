import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, obj, type Obj } from './shared/domain';
import { ClientError, type Files, type Native } from './shared/protocol';
import { mobilePendingInput, mobilePendingInputAction, mobilePendingInputCommand, mobilePendingInputRetry, mobilePendingInputObserveRoute, type PendingInputRoute } from './pending-input';

function gate() { let resolve!: () => void; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; }
function fixture(client = new T3Client()) {
  Object.assign(client, { origin: 'https://one.test', environmentId: 'env', threadId: 'thread', projectId: 'project', generation: 1,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  client.local.drafts[client.draftKey] = 'Keep ordinary draft';
  const question = (id: string, extra: Obj = {}) => ({ id, header: id, question: `${id}?`, options: [{ label: 'One', value: 'one' }, { label: ' Two ' }], ...extra });
  client.thread = { projection: { thread: { id: 'thread' }, runs: [{ id: 'run', status: 'running' }],
    runtimeRequests: [{ id: 'request', kind: 'user_input', status: 'pending', createdAt: 'today', responseCapability: { type: 'live' } }],
    turnItems: [{ id: 'item', type: 'user_input_request', requestId: 'request', responseMode: 'message', questions: [question('first'), question('multi', { multiSelect: true }), question('custom', { options: [] })] }] } };
  const route: PendingInputRoute = { active: true, visit: 'visit', url: '/threads/env/thread', environmentId: 'env', threadId: 'thread' };
  const calls: Obj[] = [], writes: string[] = [];
  let onNative = async (_: Obj) => {}, onWrite = async () => {}, fail = false, disk = '{}';
  const native: Native = { available: true, watch() {}, async later(raw) {
    const r = obj(raw); calls.push(structuredClone(r)); await onNative(r);
    if (r.op === 'request' && fail) return { ok: false, generation: client.generation, error: { kind: 'transport', message: 'Response uncertain', uncertain: true } };
    return { ok: true, generation: client.generation, value: r.op === 'ids' ? ['command'] : {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) { const next = new TextDecoder().decode(bytes); writes.push(next); await onWrite(); disk = next; } } };
  const view = () => mobilePendingInput(client, route);
  const act = (op: string, id = '', text = '', owner = view().owner) => mobilePendingInputAction(client, native, storage, { ...route }, owner, op, id, text);
  const fill = async () => { await act('custom', 'custom', '  exact café 🧪  '); await act('option', 'multi', 'one'); await act('option', 'first', 'Two'); };
  return { client, native, storage, route, calls, writes, view, act, fill, disk: () => JSON.parse(disk), intercept: (fn: typeof onNative) => { onNative = fn; }, duringWrite: (fn: typeof onWrite) => { onWrite = fn; }, fail: () => { fail = true; } };
}

describe('mobile all-question request', () => {
  test('all questions support arbitrary order, explicit combined Submit and untouched ordinary draft', async () => {
    const f = fixture(); expect(f.view().questions.map(q => q.id)).toEqual(['first', 'multi', 'custom']);
    expect(f.view().canSubmit).toBe(false); await f.fill();
    expect(f.view().canSubmit).toBe(true); expect(f.calls).toEqual([]);
    await f.act('option', 'multi', 'Two'); await f.act('option', 'multi', 'one');
    expect(f.view().questions[1]!.options.map(o => o.selected)).toEqual([false, true]);
    await f.act('submit');
    expect(f.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual([{ type: 'runtime-request.respond', threadId: 'thread', requestId: 'request',
      answers: { first: 'Two', multi: ['Two'], custom: 'exact café 🧪' }, commandId: 'command' }]);
    expect(f.client.local.drafts[f.client.draftKey]).toBe('Keep ordinary draft'); expect(f.client.pending).toBeUndefined();
  });
  test('choosing clears only that custom answer and never carries text into ordinary draft', async () => {
    const f = fixture(); await f.act('custom', 'first', 'Custom'); await f.act('option', 'first', 'one');
    expect(f.view().questions[0]).toMatchObject({ answer: '', options: [{ selected: true }, { selected: false }] });
    expect(f.client.draft).toBe('Keep ordinary draft'); await f.act('custom', 'first', 'new'); await f.act('custom', 'first', '');
    expect(f.view().questions[0]!.options.every(o => !o.selected)).toBe(true);
  });
  test('read-only and reconnect retain local editing while refusing all wire writes', async () => {
    for (const kind of ['read-only', 'reconnect']) {
      const f = fixture(); if (kind === 'read-only') f.client.scopes = []; else { f.client.connection = 'reconnecting'; f.client.threadLive = false; }
      await f.fill(); expect(f.view()).toMatchObject({ active: true, editable: true, canSubmit: false, canDismiss: false, canStop: false });
      for (const op of ['submit', 'dismiss', 'stop']) await f.act(op);
      expect(f.calls).toEqual([]); expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  ');
    }
  });
  test('drafts survive leaving and returning; observed resolution rejects same-ID successor callbacks', async () => {
    const f = fixture(); await f.fill(); const first = f.view().owner;
    f.route.active = false; expect(f.view().active).toBe(false); f.route.active = true; expect(f.view().canSubmit).toBe(true);
    const requests = f.client.projection.runtimeRequests; f.client.projection.runtimeRequests = []; expect(f.view().active).toBe(false);
    f.client.projection.runtimeRequests = requests; expect(f.view().owner).not.toBe(first); expect(f.view().canSubmit).toBe(false);
    await f.act('custom', 'custom', 'Stale text', first); expect(f.view().questions[2]!.answer).toBe(''); expect(f.calls).toEqual([]);
  });
  test('all request fields and question IDs are checked without accepting stale route commands', async () => {
    const f = fixture(); const owner = f.view().owner;
    await f.act('option', 'missing', 'one'); await f.act('option', 'first', 'forged');
    expect(f.view().canSubmit).toBe(false);
    await mobilePendingInputCommand(f.client, f.native, f.storage, owner, 'custom:custom', 'stale', 'other\n/threads/env/thread');
    expect(f.view().questions[2]!.answer).toBe('');
    arr(f.client.projection.turnItems)[0]!.questions = [{ id: 'replacement', header: 'New', question: 'New?', options: [] }];
    await f.act('custom', 'custom', 'Old', owner); expect(f.view().questions[0]!.answer).toBe('');
  });
  test('unresumable refuses local edits, but message-mode resumes and dismissal stays explicit', async () => {
    const f = fixture(); obj(arr(f.client.projection.runtimeRequests)[0]!.responseCapability).type = 'not_resumable';
    expect(f.view().editable).toBe(false); await f.act('custom', 'custom', 'No'); expect(f.view().questions[2]!.answer).toBe('');
    obj(arr(f.client.projection.runtimeRequests)[0]!.responseCapability).type = 'message'; expect(f.view().editable).toBe(true);
    await f.act('dismiss'); expect(f.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual([{ type: 'thread.user-input.dismiss', threadId: 'thread', requestId: 'request', commandId: 'command' }]);
  });
  test('Stop captures the active run without answering questions', async () => {
    const f = fixture(); await f.act('stop');
    expect(f.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual([{ type: 'run.interrupt', threadId: 'thread', runId: 'run', holdQueue: true, commandId: 'command' }]);
  });
  test.each(['revoke', 'replace', 'route'] as const)('final pre-dispatch guard handles %s during persist and removes unsent pending state', async kind => {
    const f = fixture(); await f.fill(); let first = true;
    f.duringWrite(async () => {
      if (!first) return; first = false;
      if (kind === 'revoke') f.client.scopes = [];
      if (kind === 'replace') arr(f.client.projection.runtimeRequests)[0]!.id = 'new-request';
      if (kind === 'route') { f.route.visit = 'other'; f.view(); }
    });
    await f.act('submit'); expect(f.calls.filter(r => r.op === 'request')).toEqual([]); expect(f.client.pending).toBeUndefined();
    expect(f.client.draft).toBe('Keep ordinary draft');
  });
  test('duplicate submission and edits refuse while a request is in flight; late completion leaves successor draft intact', async () => {
    const f = fixture(); await f.fill(); const entered = gate(), release = gate();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
    const sent = f.act('submit'); await entered.promise;
    expect(f.view()).toMatchObject({ responding: true, editable: false, canSubmit: false });
    await f.act('submit'); await f.act('custom', 'custom', 'During submit'); expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1);
    f.client.projection.runtimeRequests = []; f.view(); f.client.projection.runtimeRequests = [{ id: 'request', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }];
    await f.act('custom', 'custom', 'Successor'); release.resolve(); await sent;
    expect(f.view().questions[2]!.answer).toBe('Successor'); expect(f.client.draft).toBe('Keep ordinary draft');
  });
  test('an uncertain response retains its receipt and local answers', async () => {
    const f = fixture(); await f.fill(); f.fail(); await f.act('submit');
    expect(f.client.pending?.uncertain).toBe(true); expect(f.view()).toMatchObject({ responding: true, canSubmit: false });
    expect(f.view().questions[2]!.answer).toBe('  exact café 🧪  '); expect(f.client.draft).toBe('Keep ordinary draft');
  });
});


import { questionJournalHydrate, questionRecordForPending } from './pending-input-journal';
import { MobileDraftClient } from './mobile-draft-recovery';

const successor = () => ({ method: 'orchestration.dispatchCommand', payload: { type: 'thread.rename', commandId: 'successor', threadId: 'other', title: 'Keep' }, description: 'successor', threadId: 'other', text: '', uncertain: true });
describe('questionnaire journal and actual app boundary', () => {
  test.each(['success', 'failure'])('initial persist %s preserves a successor and never dispatches', async outcome => {
    const f = fixture(); await f.fill(); const next = successor(); let once = true;
    f.duringWrite(async () => { if (once) { once = false; f.client.local.pending.env = next; if (outcome === 'failure') throw new Error('disk'); } });
    await f.act('submit'); expect(f.calls.filter(r => r.op === 'request')).toEqual([]); expect(f.client.pending).toBe(next);
  });
  test.each(['success', 'definite', 'uncertain', 'cancel'])('old RPC %s preserves replacement pending and journal', async outcome => {
    const f = fixture(); await f.fill(); const entered = gate(), release = gate(), next = successor();
    f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise;
      if (outcome === 'definite') throw new ClientError('refused');
      if (outcome === 'uncertain') throw new ClientError('lost', 'transport', true);
      if (outcome === 'cancel') throw new ClientError('gone', 'superseded');
    } });
    const task = f.act('submit'); await entered.promise;
    const old = questionRecordForPending(f.client)!;
    const newRecord = { ...old, pending: next }; obj(f.client.local).mobileQuestionCommands = { command: newRecord }; f.client.local.pending.env = next;
    release.resolve(); if (outcome === 'cancel') await expect(task).rejects.toThrow('gone'); else await task;
    expect(f.client.pending).toBe(next); expect(obj(obj(f.client.local).mobileQuestionCommands).command).toBe(newRecord);
  });
  test('Stop in flight excludes wire actions while preserving local question edits', async () => {
    const f = fixture(), entered = gate(), release = gate(); f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
    const task = f.act('stop'); await entered.promise; expect(f.view()).toMatchObject({ responding: false, editable: true, canStop: false, canSubmit: false });
    await f.act('custom', 'custom', 'During Stop'); expect(f.view().questions[2]!.answer).toBe('During Stop'); release.resolve(); await task;
  });
  test('known-unsent cleanup failure stays visible and Retry only saves cleanup', async () => {
    const f = fixture(); await f.fill(); let writes = 0;
    f.duringWrite(async () => { writes++; if (writes === 1) f.client.scopes = []; if (writes === 2) throw new Error('cleanup disk'); });
    const result = await f.act('submit'); expect(result.message).toContain('Could not save the answer outcome');
    expect(f.client.pending?.uncertain).toBe(true); expect(questionRecordForPending(f.client)?.phase).toBe('unsent');
    expect(obj(f.disk().pending).env).toBeUndefined(); expect(f.calls.filter(r => r.op === 'request')).toEqual([]);
    f.client.scopes = ['orchestration:operate']; await mobilePendingInputRetry(f.client, f.native, f.storage);
    expect(f.client.pending).toBeUndefined(); expect(f.calls.filter(r => r.op === 'request')).toEqual([]);
  });
  test('acknowledged cleanup failure Retry never dispatches a second command', async () => {
    const f = fixture(); await f.fill(); let writes = 0; f.duringWrite(async () => { if (++writes === 3) throw new Error('cleanup disk'); });
    await f.act('submit'); expect(questionRecordForPending(f.client)?.phase).toBe('settled'); expect(f.client.pending?.uncertain).toBe(true);
    await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.client.pending).toBeUndefined(); expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1);
  });
  test('recovered receipt restores answers and guarded Retry sends the immutable ID without allocating again', async () => {
    const f = fixture(); await f.fill(); f.fail(); await f.act('submit'); const disk = f.disk();
    expect(obj(disk.pending).env).toBeUndefined(); const next = fixture(); questionJournalHydrate(next.client, disk);
    expect(next.view().questions[2]!.answer).toBe('  exact café 🧪  ');
    await mobilePendingInputRetry(next.client, next.native, next.storage);
    expect(next.calls.filter(r => r.op === 'ids')).toEqual([]);
    expect(next.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual(f.calls.filter(r => r.op === 'request').map(r => r.payload));
    expect(next.client.pending).toBeUndefined();
  });
  test('Retry holds original payload and refuses invalidated route during its persist', async () => {
    const f = fixture(); await f.fill(); f.fail(); await f.act('submit'); const next = fixture(); questionJournalHydrate(next.client, f.disk()); next.view();
    let once = true; next.duringWrite(async () => { if (once) { once = false; mobilePendingInputObserveRoute(next.client, { ...next.route, active: false }); } });
    await mobilePendingInputRetry(next.client, next.native, next.storage);
    expect(next.calls.filter(r => r.op === 'request')).toEqual([]);
  });
  test('same request keeps card identity across generation/capability while action owner changes', () => {
    const f = fixture(), before = f.view(); f.client.generation++; obj(arr(f.client.projection.runtimeRequests)[0]!.responseCapability).type = 'message';
    expect(f.view().cardId).toBe(before.cardId); expect(f.view().owner).not.toBe(before.owner);
  });

});


test('ordinary preference persistence cannot publish a generic questionnaire replay', async () => {
  const f = fixture(new MobileDraftClient()); await f.fill(); const entered = gate(), release = gate();
  f.intercept(async r => { if (r.op === 'request') { entered.resolve(); await release.promise; } });
  const task = f.act('submit'); await entered.promise; await f.client.persist(f.storage);
  expect(obj(f.disk().pending).env).toBeUndefined(); expect(Object.keys(obj(f.disk().mobileQuestionCommands))).toEqual(['command']);
  release.resolve(); await task;
});
test('successor installed during cleanup save failure is never replaced', async () => {
  const f = fixture(); await f.fill(); const next = successor(); let count = 0;
  f.duringWrite(async () => { if (++count === 3) { f.client.local.pending.env = next; throw new Error('cleanup'); } });
  await f.act('submit'); expect(f.client.pending).toBe(next);
});

test('reloaded accepted response reconciles without a question card and preserves successor during save', async () => {
  const first = fixture(); await first.fill(); first.fail(); await first.act('submit');
  for (const replacement of [false, true]) {
    const f = fixture(new MobileDraftClient()); questionJournalHydrate(f.client, first.disk()); f.client.projection.runtimeRequests = [];
    expect(f.client.reconcilePending()).toBe(true); const next = successor();
    if (replacement) f.duringWrite(async () => { f.client.local.pending.env = next; });
    await f.client.persist(f.storage); expect(f.client.pending).toBe(replacement ? next : undefined);
    expect(f.view().active).toBe(false); expect(f.calls).toEqual([]); expect(Object.keys(obj(f.disk().mobileQuestionCommands))).toEqual([]);
  }
});
test('terminal cleanup Retry remains usable after the original request disappears', async () => {
  const f = fixture(); await f.fill(); let count = 0; f.duringWrite(async () => { if (++count === 3) throw new Error('cleanup'); });
  await f.act('submit'); f.client.projection.runtimeRequests = []; f.view();
  await mobilePendingInputRetry(f.client, f.native, f.storage); expect(f.client.pending).toBeUndefined(); expect(f.calls.filter(r => r.op === 'request')).toHaveLength(1);
});


test.each(['preparing', 'starting', 'running', 'waiting'])('uncertain Stop for active %s run retries its captured command', async status => {
  const first = fixture(); arr(first.client.projection.runs)[0]!.status = status; first.fail(); await first.act('stop');
  const next = fixture(); arr(next.client.projection.runs)[0]!.status = status; questionJournalHydrate(next.client, first.disk()); next.view();
  await mobilePendingInputRetry(next.client, next.native, next.storage);
  expect(next.calls.filter(r => r.op === 'ids')).toEqual([]);
  expect(next.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual(first.calls.filter(r => r.op === 'request').map(r => r.payload));
  expect(next.client.pending).toBeUndefined();
});
test.each(['preparing', 'starting', 'running', 'waiting'])('journal keeps older captured %s run active when a newer run is active', async status => {
  const first = fixture(); first.fail(); await first.act('stop');
  const next = fixture(new MobileDraftClient()); questionJournalHydrate(next.client, first.disk());
  next.client.projection.runs = [{ id: 'run', status }, { id: 'newer', status: 'running' }]; next.view();
  expect(next.client.reconcilePending()).toBe(false); expect(questionRecordForPending(next.client)?.phase).toBe('uncertain');
  await mobilePendingInputRetry(next.client, next.native, next.storage);
  expect(next.calls.filter(r => r.op === 'request').map(r => r.payload)).toEqual(first.calls.filter(r => r.op === 'request').map(r => r.payload));
});
test('completed captured Stop retires even while a different run is active', async () => {
  const first = fixture(); first.fail(); await first.act('stop');
  const next = fixture(new MobileDraftClient()); questionJournalHydrate(next.client, first.disk());
  next.client.projection.runs = [{ id: 'run', status: 'completed' }, { id: 'newer', status: 'waiting' }]; next.view();
  expect(next.client.reconcilePending()).toBe(true); await next.client.persist(next.storage);
  expect(next.client.pending).toBeUndefined(); expect(next.calls).toEqual([]);
});
