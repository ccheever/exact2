// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobilePendingTaskEditorOpen as open, mobilePendingTaskEditorSave as save, mobilePendingTaskEditorFinish as finish } from './mobile-pending-task-editor';
import { mobilePendingTaskEditorKey, mobilePendingTaskEditorsSnapshot } from './mobile-pending-task-state';
import { mobileNewTaskDraftBind as bind, mobileNewTaskDraftUnbind as unbind, mobileNewTaskDraftStore as drafts } from './mobile-new-task-drafts';
import { mobilePendingTaskDraftFingerprint } from './mobile-pending-task-draft';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';
import { obj, str, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
const owner = { origin: 'https://editor.test', environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command' };
const key = 'new-task:pending-message', markerKey = mobilePendingTaskEditorKey(owner);
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function original(): MobileOutboxRecord {
  return { schemaVersion: 1, ...owner, text: 'original', attachments: [], createdAt: '2026-10-08T00:00:00.000Z',
    modelSelection: { instanceId: 'provider', model: 'model' }, runtimeMode: 'auto', interactionMode: 'default',
    creation: { projectId: 'project', projectTitle: 'Captured project', projectCwd: '/captured', workspaceMode: 'local', branch: null, worktreePath: null, startFromOrigin: false } };
}
function backend() {
  const state: { record: MobileOutboxRecord | null; epoch: string; token: string; revision: number; floor: number;
    held: Set<string>; outcomes: Map<string, Obj>; requests: Map<string, Obj>; unknown: boolean; releaseFalse: boolean } = { record: original(), epoch: 'epoch', token: 'epoch:1', revision: 1, floor: 1,
    held: new Set<string>(), outcomes: new Map<string, Obj>(), requests: new Map<string, Obj>(), unknown: false, releaseFalse: false };
  const calls: Obj[] = [], hooks = new Map<string, (request: Obj) => void | Promise<void>>();
  const current = () => ({ record: copy(state.record), revision: state.revision, token: state.token, pending: false });
  const native: Native = { available: true, watch() {}, async later(raw) {
    const request = obj(raw); calls.push(copy(request)); await hooks.get(`${request.op}:${request.action ?? ''}`)?.(request);
    let value: unknown;
    if (request.op === 'status') value = { phase: 'disconnected' };
    else if (request.op === 'devicePresentation') value = {};
    else if (request.op === 'ids') value = ['aaaaaaaa-1111-4111-8111-111111111111'];
    else if (request.op === 'mobileOutboxDelivery' && request.action === 'status') value = { operation: null, durable: false };
    else if (request.op === 'mobileOutboxInline' && request.action === 'lookup') value = { operations: [] };
    else if (request.op === 'mobileOutbox') {
      if (request.action === 'read') value = { ownerEpoch: state.epoch, sequenceFloor: state.floor, complete: true, errors: [],
        records: state.record ? [{ ...current(), held: state.held.size > 0 }] : [], revisions: { message: state.revision },
        tokens: { message: state.token }, outcomes: [...state.outcomes.values()].map(copy), mutations: [], transfers: [] };
      else if (request.action === 'hold') {
        const held = !!state.record && request.expectedToken === state.token;
        if (held) state.held.add(str(request.owner)); value = { held };
      } else if (request.action === 'releaseHold') value = { released: state.releaseFalse ? false : state.held.delete(str(request.owner)) };
      else if (request.action === 'acknowledge') { state.outcomes.delete(str(request.mutationId)); value = { acknowledged: true }; }
      else if (request.action === 'resumeUpdate') {
        const saved = obj(request.request), mutation = str(saved.mutationId), previous = state.requests.get(mutation);
        if (previous) expect(saved).toEqual(previous); else state.requests.set(mutation, copy(saved));
        const decoded = mobileOutboxDecode(saved.record); if (!decoded.ok) throw Error(decoded.error);
        const known = state.outcomes.get(mutation);
        if (known?.status === 'committed') value = known;
        else if (state.unknown) { value = { status: 'unknown', mutationId: mutation, messageId: 'message', message: 'transport interrupted' }; }
        else {
          const committed = !!state.record && saved.expectedToken === state.token && saved.expectedRevision === state.revision && state.held.has(str(request.holdOwner));
          state.floor++;
          if (committed) { state.record = decoded.record; state.token = mutation; state.revision++; }
          value = { status: committed ? 'committed' : 'stale', mutationId: mutation, messageId: 'message', message: committed ? '' : 'CAS changed',
            revision: state.revision, record: committed ? decoded.record : null, removed: null, current: current(), ownerEpoch: state.epoch, sequenceFloor: state.floor };
        }
        state.outcomes.set(mutation, obj(copy(value)));
      } else throw Error('Unexpected outbox action ' + request.action);
    } else throw Error('No network fallback allowed: ' + JSON.stringify(request));
    return { ok: true, generation: 1, value: copy(value) };
  } };
  return { state, native, calls, hooks, writes: () => calls.filter(call => call.action === 'resumeUpdate'),
    remove() { state.record = null; state.revision++; state.floor++; state.token = `${state.epoch}:${state.floor}`; },
    replace(text = 'external winner') { state.record = { ...original(), text }; state.revision++; state.floor++; state.token = `${state.epoch}:${state.floor}`; } };
}
async function fixture(document: Obj = { version: 1 }, host = backend()) {
  const client = new MobileDraftClient(), writes: Obj[] = []; let disk = JSON.stringify(document), active = true;
  const hooks: { write?: (document: Obj) => void | Promise<void> } = {};
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { const document = obj(JSON.parse(new TextDecoder().decode(bytes))); await hooks.write?.(document); disk = JSON.stringify(document); writes.push(copy(document)); } } };
  const handles = mobileDraftRecoveryHandles(client, host.native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin: owner.origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 1 });
  const input = { owner: copy(owner), current: () => active };
  return { client, storage, host, hooks, writes, input, disk: () => obj(JSON.parse(disk)), route(value: boolean) { active = value; } };
}
function live(f: Awaited<ReturnType<typeof fixture>>) {
  const marker = mobilePendingTaskEditorsSnapshot(f.client).markers[0]; if (!marker) throw Error('Missing live marker'); return marker;
}
function savedMarker(document: Obj): Obj { return obj(obj(obj(document.mobilePendingTaskEditors).markers)[markerKey]); }
function edit(f: Awaited<ReturnType<typeof fixture>>, text: string) { f.client.local.drafts[key] = text; drafts(f.client).records[key].revision++; }
async function opened() {
  const f = await fixture(); const value = await open(f.client, f.host.native, f.storage, f.input);
  expect(value.status).toBe('ready'); expect(value.marker).not.toBeNull(); return f;
}
function latch() { let resolve: () => void = () => {}; const promise = new Promise<void>(done => { resolve = done; }); return { promise, resolve }; }

test('Open persists adopted content and exact session hold; Save persists original-identity request before native and committed baseline before ACK', async () => {
  const f = await opened(); expect(f.client.local.drafts[key]).toBe('original'); expect(savedMarker(f.disk()).session).toBe(live(f).session);
  expect(f.host.state.held.size).toBe(1); const before = live(f); edit(f, 'edited');
  f.host.hooks.set('mobileOutbox:resumeUpdate', request => {
    expect(obj(savedMarker(f.disk()).pending).request).toEqual(request.request);
    expect(obj(f.disk().drafts)[key]).toBe('edited');
    expect(obj(obj(request.request).record)).toMatchObject({ ...owner, createdAt: original().createdAt, text: 'edited', creation: original().creation });
  });
  f.host.hooks.set('mobileOutbox:acknowledge', () => {
    expect(savedMarker(f.disk()).pending).toBeNull(); expect(obj(obj(savedMarker(f.disk()).baseline).record).text).toBe('edited');
  });
  const result = await save(f.client, f.host.native, f.storage, { ...f.input, expected: before });
  expect(result.status).toBe('saved'); expect(f.host.writes()).toHaveLength(1); expect(f.host.state.record?.text).toBe('edited');
  expect(f.client.local.drafts[key]).toBe('edited'); expect(f.host.state.held.size).toBe(1); expect(result.fingerprint).not.toBeNull();
});

test('failed pending persistence dispatches nothing; retry persists the same pending request before native', async () => {
  const f = await opened(); edit(f, 'edited');
  f.hooks.write = document => { if (savedMarker(document).pending) throw Error('disk blocked'); };
  const first = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  expect(first.status).toBe('retained'); expect(f.host.writes()).toHaveLength(0); expect(savedMarker(f.disk()).pending).toBeNull();
  const pending = copy(live(f).pending); expect(pending).not.toBeNull(); delete f.hooks.write;
  f.host.hooks.set('mobileOutbox:resumeUpdate', request => { expect(savedMarker(f.disk()).pending).toEqual(pending); expect(obj(savedMarker(f.disk()).pending).request).toEqual(request.request); });
  const retry = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  expect(retry.status).toBe('saved'); expect(f.host.writes()).toHaveLength(1);
});

test('cold interrupted replay uses the exact old request and preserves newer saved editor content', async () => {
  const f = await opened(); edit(f, 'captured edit'); f.host.state.unknown = true;
  expect((await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) })).status).toBe('retained');
  const request = copy(live(f).pending?.request); expect(request).toBeDefined(); edit(f, 'newer typing'); await f.client.persist(f.storage);
  f.host.state.held.clear(); f.host.state.epoch = 'cold'; f.host.state.unknown = false;
  const restart = await fixture(f.disk(), f.host);
  const result = await open(restart.client, restart.host.native, restart.storage, restart.input);
  expect(result.status).toBe('ready'); expect(restart.host.writes().map(call => call.request)).toEqual([request, request]);
  expect(restart.host.state.record?.text).toBe('captured edit'); expect(restart.client.local.drafts[key]).toBe('newer typing');
  expect(obj(restart.disk().drafts)[key]).toBe('newer typing'); expect(live(restart).pending).toBeNull(); expect(restart.host.state.held.size).toBe(1);
});

test('post-commit persistence failure retains original durable request for exact cold recovery', async () => {
  const f = await opened(); edit(f, 'saved by native');
  f.hooks.write = document => { if (f.host.writes().length && savedMarker(document).pending === null) throw Error('baseline write failed'); };
  const value = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  expect(value.status).toBe('retained'); expect(f.host.state.record?.text).toBe('saved by native');
  const request = obj(savedMarker(f.disk()).pending).request; expect(request).toBeDefined();
  expect(f.host.calls.filter(call => call.action === 'acknowledge')).toHaveLength(0);
  f.host.state.held.clear(); const restart = await fixture(f.disk(), f.host);
  expect((await open(restart.client, restart.host.native, restart.storage, restart.input)).status).toBe('ready');
  expect(restart.host.writes().map(call => call.request)).toEqual([request, request]); expect(savedMarker(restart.disk()).pending).toBeNull();
});

test('missing or stale native row retains editor and never enqueues or overwrites a winner', async () => {
  for (const mode of ['removed', 'changed']) {
    const f = await opened(); edit(f, 'keep local'); if (mode === 'removed') f.host.remove(); else f.host.replace();
    const value = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
    expect(value.status).toBe('retained'); expect(f.client.local.drafts[key]).toBe('keep local'); expect(f.host.writes()).toHaveLength(0);
    expect(f.host.state.record?.text ?? null).toBe(mode === 'removed' ? null : 'external winner');
  }
  const host = backend(); host.remove(); const f = await fixture({ version: 1 }, host);
  expect((await open(f.client, host.native, f.storage, f.input)).status).toBe('retained');
  expect(mobilePendingTaskEditorsSnapshot(f.client).markers).toEqual([]); expect(f.client.local.drafts[key]).toBeUndefined();
});

test('stale expected owner/session/revision and stale route never submit native changes', async () => {
  const f = await opened(), marker = live(f); edit(f, 'keep');
  for (const expected of [{ ...marker, session: 'old' }, { ...marker, revision: marker.revision - 1 }, { ...marker, owner: { ...owner, commandId: 'other' } }]) {
    await expect(save(f.client, f.host.native, f.storage, { ...f.input, expected })).rejects.toMatchObject({ kind: 'superseded' });
  }
  f.route(false); await expect(save(f.client, f.host.native, f.storage, { ...f.input, expected: marker })).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.host.writes()).toHaveLength(0); expect(f.client.local.drafts[key]).toBe('keep');
});

test('newer typing during native Save is retained while its captured predecessor commits', async () => {
  const f = await opened(); edit(f, 'first'); const entered = latch(), release = latch();
  f.host.hooks.set('mobileOutbox:resumeUpdate', async () => { entered.resolve(); await release.promise; });
  const saving = save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) }); await entered.promise;
  edit(f, 'newer'); release.resolve(); const result = await saving;
  expect(result.status).toBe('retained'); expect(result.reason).toContain('Newer'); expect(f.host.state.record?.text).toBe('first');
  expect(f.client.local.drafts[key]).toBe('newer'); expect(obj(f.disk().drafts)[key]).toBe('newer'); expect(f.host.state.held.size).toBe(1);
});

test('Finish refuses bound editor and unwritten changes, then persists cleanup before hold release and live deletion', async () => {
  const f = await opened(); const initial = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  expect(initial.status).toBe('saved'); expect(bind(f.client, key, 'editor')).toBe(true);
  const input = { ...f.input, expected: live(f), fingerprint: initial.fingerprint! };
  expect((await finish(f.client, f.host.native, f.storage, input)).status).toBe('retained');
  expect(f.host.state.held.size).toBe(1); unbind(f.client, 'editor'); edit(f, 'unsaved');
  expect((await finish(f.client, f.host.native, f.storage, { ...input, fingerprint: mobilePendingTaskDraftFingerprint(f.client, key)! })).status).toBe('retained');
  edit(f, 'original'); const saved = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  f.host.hooks.set('mobileOutbox:releaseHold', () => { expect(obj(f.disk().drafts)[key]).toBeUndefined(); expect(savedMarker(f.disk())).toEqual({}); });
  expect((await finish(f.client, f.host.native, f.storage, { ...f.input, expected: live(f), fingerprint: saved.fingerprint! })).status).toBe('finished');
  expect(f.client.local.drafts[key]).toBeUndefined(); expect(mobilePendingTaskEditorsSnapshot(f.client).markers).toEqual([]); expect(f.host.state.held.size).toBe(0);
});

test('failed Finish persistence keeps live and saved content plus the native editor hold', async () => {
  const f = await opened(), saved = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  const before = f.disk(); f.hooks.write = document => { if (!obj(document.drafts)[key]) throw Error('cleanup disk failed'); };
  expect((await finish(f.client, f.host.native, f.storage, { ...f.input, expected: live(f), fingerprint: saved.fingerprint! })).status).toBe('retained');
  expect(f.disk()).toEqual(before); expect(f.client.local.drafts[key]).toBe('original'); expect(f.host.state.held.size).toBe(1);
});

test('newer typing or route change during fulfilled Finish cleanup repairs the saved document and retains ownership', async () => {
  for (const mode of ['typing', 'route']) {
    const f = await opened(), saved = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
    const entered = latch(), release = latch(); let intercepted = false;
    f.hooks.write = async document => { if (!obj(document.drafts)[key] && !intercepted) { intercepted = true; entered.resolve(); await release.promise; } };
    const finishing = finish(f.client, f.host.native, f.storage, { ...f.input, expected: live(f), fingerprint: saved.fingerprint! }); await entered.promise;
    if (mode === 'typing') edit(f, 'newer finish typing'); else f.route(false);
    release.resolve();
    if (mode === 'route') await expect(finishing).rejects.toMatchObject({ kind: 'superseded' });
    else expect((await finishing).status).toBe('retained'); expect(f.client.local.drafts[key]).toBe(mode === 'typing' ? 'newer finish typing' : 'original');
    expect(obj(f.disk().drafts)[key]).toBe(f.client.local.drafts[key]); expect(savedMarker(f.disk()).session).toBe(live(f).session);
    expect(f.host.state.held.size).toBe(1); expect(f.host.calls.filter(call => call.action === 'releaseHold')).toHaveLength(0);
  }
});


test('ordinary preference writer admitted during Finish receives the same cleanup projection and cannot resurrect editor', async () => {
  const f = await opened(), saved = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
  const first = latch(), releaseFirst = latch(), second = latch(), releaseSecond = latch(); let writes = 0;
  f.hooks.write = async document => {
    expect(obj(document.drafts)[key]).toBeUndefined(); expect(savedMarker(document)).toEqual({});
    if (++writes === 1) { first.resolve(); await releaseFirst.promise; }
    else if (writes === 2) { second.resolve(); await releaseSecond.promise; }
  };
  const finishing = finish(f.client, f.host.native, f.storage, { ...f.input, expected: live(f), fingerprint: saved.fingerprint! });
  await first.promise; expect(f.client.local.drafts[key]).toBe('original');
  const ordinary = f.client.persist(f.storage); await second.promise; releaseFirst.resolve();
  expect((await finishing).status).toBe('finished'); expect(f.client.local.drafts[key]).toBeUndefined();
  releaseSecond.resolve(); await ordinary; expect(obj(f.disk().drafts)[key]).toBeUndefined(); expect(savedMarker(f.disk())).toEqual({});
  expect(f.host.state.held.size).toBe(0); expect(f.client.pendingTaskCleanup).toBeNull();
});

test('release refusal, failure or admitted release with lost reply preserves preferences and retries the same session', async () => {
  for (const mode of ['false', 'failure', 'lost-reply']) {
    const f = await opened(), saved = await save(f.client, f.host.native, f.storage, { ...f.input, expected: live(f) });
    const expected = live(f), input = { ...f.input, expected, fingerprint: saved.fingerprint! };
    const before = f.disk(), holdOwner = [...f.host.state.held][0];
    if (mode === 'false') f.host.state.releaseFalse = true;
    else f.host.hooks.set('mobileOutbox:releaseHold', request => {
      if (mode === 'lost-reply') expect(f.host.state.held.delete(str(request.owner))).toBe(true);
      throw Error(mode === 'lost-reply' ? 'release committed but reply lost' : 'release transport failed');
    });
    expect((await finish(f.client, f.host.native, f.storage, input)).status).toBe('retained');
    expect(savedMarker(f.disk()).session).toBe(expected.session); expect(obj(f.disk().drafts)[key]).toBe('original');
    expect(live(f)).toEqual(expected); expect(f.host.state.held.size).toBe(mode === 'lost-reply' ? 0 : 1); expect(f.client.pendingTaskCleanup).toBeNull();
    expect(f.disk()).toEqual(before); expect(f.client.local.drafts[key]).toBe('original');
    const holdsBeforeRetry = f.host.calls.filter(call => call.action === 'hold').length;
    f.host.state.releaseFalse = false; f.host.hooks.delete('mobileOutbox:releaseHold');
    expect((await finish(f.client, f.host.native, f.storage, input)).status).toBe('finished');
    expect(f.host.state.held.size).toBe(0); expect(savedMarker(f.disk())).toEqual({});
    if (mode === 'lost-reply') {
      const holds = f.host.calls.filter(call => call.action === 'hold');
      expect(holds).toHaveLength(holdsBeforeRetry + 1); expect(holds.at(-1)?.owner).toBe(holdOwner);
      expect(f.host.calls.filter(call => call.op === 'ids')).toHaveLength(1);
      expect(f.client.local.drafts[key]).toBeUndefined();
    }
  }
});
