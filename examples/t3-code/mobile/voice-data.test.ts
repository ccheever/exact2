import { beforeEach, describe, expect, test } from 'bun:test';
import { mobileDraftChanged } from './draft';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { mobileVoiceAction, mobileVoiceBlocksSubmission, mobileVoiceObserveDraft, mobileVoiceSnapshot, mobileVoiceStatus } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';
import { mobileComposerTarget } from './composer-target';
import { queuedEditThreadKey, queuedEditEndMemory, mobileQueuedEditLookup, mobileQueuedEditWriteText } from './queued-edit-state';
import { queuedEditState, type MobileQueuedEditSession } from './queued-edit-memory';

beforeEach(resetVoiceInputGlobalsForTests);
function fixture() {
  const client = new T3Client(); client.origin = 'https://voice.test'; client.environmentId = 'env'; client.projectId = 'p';
  client.threadId = 'one'; client.local.drafts[client.draftKey] = 'Hello world';
  const calls: Obj[] = []; let saves = 0; let currentSession = '';
  const status: Obj = { available: true, locale: 'en-US', event: 0, levels: [-160, -20], elapsed: 0 };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() { saves++; } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.action !== 'status') currentSession = String(request.session);
    const value = request.action === 'status' ? { ...status, session: currentSession }
      : request.action === 'permission' ? { granted: true, canAskAgain: true }
      : request.action === 'prepare' ? { locale: 'en-US' }
      : request.action === 'recorder-prepare' || request.action === 'stop' ? { uri: 'file:///fixture.m4a' }
      : request.action === 'transcribe' ? { transcript: 'spoken words' } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const action = (op: string, start = 6, end = 11, bridge = native) => mobileVoiceAction(op, start, end, 'Original task', client.draftKey, bridge, storage, client);
  return { client, calls, storage, native, action, status, saves: () => saves };
}
describe('mobile voice ownership and answer lifetime, mocked native only', () => {
  test('completed recording replaces captured selection and persists shared draft', async () => {
    const f = fixture(); expect(mobileVoiceBlocksSubmission(f.client)).toBe(false);
    expect((await f.action('start')).data.phase).toBe('recording'); expect(mobileVoiceBlocksSubmission(f.client)).toBe(true);
    expect((await f.action('stop')).data.phase).toBe('idle'); expect(mobileVoiceBlocksSubmission(f.client)).toBe(false);
    expect(f.client.draft).toBe('Hello spoken words'); expect(f.saves()).toBeGreaterThan(0);
    expect(mobileVoiceSnapshot('env:one', f.client)).toMatchObject({ selectionOwner: 'env:one', selectionStart: 18, selectionEnd: 18 });
    expect(f.calls.filter(call => call.action === 'delete')).toHaveLength(1);
  });
  test('offscreen target persists without selecting it or reusing Start answer handles', async () => {
    const f = fixture(); let valid = true;
    const starting: Native = { ...f.native, later: input => { if (!valid) throw new Error('Start handle expired'); return f.native.later(input); } };
    await f.action('start', 6, 11, starting); valid = false;
    f.client.threadId = 'two'; f.client.local.drafts['env:two'] = 'Other draft';
    expect(mobileVoiceSnapshot('env:two', f.client)).toMatchObject({ focused: false, globalVisible: true });
    expect((await f.action('stop')).message).toBe('');
    expect(f.client.threadId).toBe('two'); expect(f.client.draft).toBe('Other draft');
    expect(f.client.local.drafts['env:one']).toBe('Hello spoken words');
  });
  test('offscreen caret commit is invalidated by a later edit even if text is restored', async () => {
    const f = fixture(); await f.action('start'); f.client.threadId = 'two'; await f.action('stop');
    expect(mobileVoiceSnapshot('env:two', f.client).selectionOwner).toBe('env:one');
    f.client.local.drafts['env:one'] = 'Next edit'; mobileVoiceObserveDraft(f.client);
    f.client.local.drafts['env:one'] = 'Hello spoken words'; mobileVoiceObserveDraft(f.client);
    expect(mobileVoiceSnapshot('env:one', f.client).selectionOwner).toBe('');
  });
  test('starting another target invalidates an older offscreen pending caret', async () => {
    const f = fixture(); await f.action('start'); f.client.threadId = 'two'; await f.action('stop');
    expect(mobileVoiceSnapshot('env:two', f.client).selectionOwner).toBe('env:one');
    f.client.local.drafts['env:two'] = 'Second draft'; await f.action('start');
    expect(mobileVoiceSnapshot('env:two', f.client).selectionOwner).toBe(''); await f.action('cancel');
  });
  test('edit away and back changes revision and refuses transcript', async () => {
    const f = fixture(); await f.action('start');
    f.client.local.drafts['env:one'] = 'Changed'; mobileVoiceObserveDraft(f.client);
    f.client.local.drafts['env:one'] = 'Hello world'; mobileVoiceObserveDraft(f.client);
    const result = await f.action('stop'); expect(result.data.error).toContain('draft changed'); expect(f.client.draft).toBe('Hello world');
    expect(f.saves()).toBe(0);
  });
  test('connection identity change cannot commit into reused draft key', async () => {
    const f = fixture(); await f.action('start'); f.client.generation++;
    expect((await f.action('stop')).data.error).toContain('draft changed'); expect(f.client.draft).toBe('Hello world');
  });
  test('a stale editor owner cannot start dictation in the newly selected draft', async () => {
    const f = fixture();
    const result = await mobileVoiceAction('start', 0, 0, 'Old task', 'env:old', f.native, f.storage, f.client);
    expect(result.message).toContain('no longer available'); expect(f.calls).toHaveLength(0);
  });
  test('selection changes during status readiness refuse Start before recording', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => {
      const reply = await original(input);
      if (obj(input).action === 'status') f.client.threadId = 'other';
      return reply;
    };
    expect((await f.action('start')).message).toContain('no longer available');
    expect(f.calls.some(call => call.action === 'permission')).toBe(false);
  });
  test('unavailable status never requests permission or recorder', async () => {
    const f = fixture(); f.status.available = false; f.status.reason = 'Unsupported device';
    expect((await f.action('start')).message).toBe('Unsupported device'); expect(f.calls.map(c => c.action)).toEqual(['status']);
  });
  test('cancel settles cleanup before its answer is discarded', async () => {
    const f = fixture(); await f.action('start');
    expect((await f.action('cancel')).data.phase).toBe('idle'); expect(f.client.draft).toBe('Hello world');
    expect(f.calls.some(c => c.action === 'stop')).toBe(true); expect(f.calls.some(c => c.action === 'release')).toBe(true); expect(f.calls.some(c => c.action === 'delete')).toBe(true);
    expect(f.calls.some(c => c.action === 'transcribe')).toBe(false);
  });
  test('native bounded finish is reconciled once and releases temporary recording', async () => {
    const f = fixture(); await f.action('start'); Object.assign(f.status, { event: 1, eventKind: 'finished', uri: 'file:///fixture.m4a', elapsed: 300 });
    await mobileVoiceStatus(f.native, f.client); await f.action('reconcile'); await f.action('reconcile');
    expect(f.client.draft).toBe('Hello spoken words'); expect(f.calls.filter(c => c.action === 'transcribe')).toHaveLength(1);
  });
  test('shared draft input hook notices synchronous edit then revert', async () => {
    const f = fixture(); await f.action('start');
    const first = mobileDraftChanged(f.client, 'Changed', f.native, f.storage);
    const second = mobileDraftChanged(f.client, 'Hello world', f.native, f.storage);
    await Promise.all([first, second]);
    expect((await f.action('stop')).data.error).toContain('draft changed'); expect(f.client.draft).toBe('Hello world');
  });
  test('runtime approval disables dictation instead of overwriting its answer', async () => {
    const f = fixture(); f.client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { runtimeRequests: [{ id: 'approval', status: 'pending', kind: 'approval' }], turnItems: [] } };
    expect((await f.action('start')).message).toContain('request answer'); expect(f.calls).toHaveLength(0);
  });
  test('native selection is requested for actual editor starts and stale text is refused', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => {
      if (obj(input).action === 'selection') { f.client.local.drafts['env:one'] = 'User edit'; return { ok: true, generation: f.client.generation, value: { start: 0, end: 0 } }; }
      return original(input);
    };
    expect((await f.action('start', -1, -1)).message).toContain('draft changed'); expect(f.calls.some(c => c.action === 'permission')).toBe(false);
  });
  test('cancel during asynchronous preparation settles before another session starts', async () => {
    const f = fixture(), original = f.native.later; let entered!: () => void, release!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
    f.native.later = async input => {
      if (obj(input).action === 'prepare') { entered(); await gate; }
      if (obj(input).action === 'cancel') release();
      return original(input);
    };
    const pending = f.action('start'); await started; await f.action('cancel'); await pending;
    expect(f.calls.some(c => c.action === 'record')).toBe(false); expect(mobileVoiceSnapshot('env:one', f.client).phase).toBe('idle');
    expect((await f.action('start')).data.phase).toBe('recording'); await f.action('cancel');
  });
  test('background observed while record reply is pending is drained before Start returns', async () => {
    const f = fixture(), original = f.native.later; let entered!: () => void, release!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
    f.native.later = async input => { const reply = await original(input); if (obj(input).action === 'record') { entered(); await gate; } return reply; };
    const pending = f.action('start'); await started;
    Object.assign(f.status, { event: 1, eventKind: 'background' }); await mobileVoiceStatus(f.native, f.client);
    expect((await f.action('reconcile')).message).toContain('finishing'); release();
    expect((await pending).data.phase).toBe('error'); expect(f.calls.some(c => c.action === 'transcribe')).toBe(false);
    expect(f.calls.some(c => c.action === 'delete')).toBe(true);
  });
  test('abandoned answer cancels controller without showing an ordinary voice error', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => {
      if (obj(input).action === 'prepare') throw Object.assign(new Error('answer ended'), { name: 'FetchError', kind: 'Aborted' });
      return original(input);
    };
    await expect(f.action('start')).rejects.toMatchObject({ kind: 'superseded' });
    expect(mobileVoiceSnapshot('env:one', f.client).phase).toBe('idle');
  });
  test('background interruption discards audio instead of transcribing', async () => {
    const f = fixture(); await f.action('start'); Object.assign(f.status, { event: 1, eventKind: 'background' });
    await mobileVoiceStatus(f.native, f.client); const result = await f.action('reconcile');
    expect(result.data.phase).toBe('error'); expect(f.client.draft).toBe('Hello world'); expect(f.calls.some(c => c.action === 'transcribe')).toBe(false);
  });
});

// Real target/state reducers; sessions are seeded directly. These tests do not
// exercise Begin/Cancel's durable journal or actual speech recognition.
function queuedVoiceFixture() {
  const f = fixture(), state = queuedEditState(f.client);
  const session: MobileQueuedEditSession = { owner: 'queued-session-one', draftKey: 'env:queued-edit:run-one',
    origin: f.client.origin, environmentId: 'env', threadId: 'one', projectId: 'p', generation: f.client.generation,
    session: 'session-one', revision: 1, runId: 'run-one', messageId: 'message-one', text: 'Queued world',
    attachments: [], existingAttachments: [], saving: false };
  const install = (value: MobileQueuedEditSession) => {
    state.sessions.set(value.owner, value); state.active.set(queuedEditThreadKey(value.environmentId, value.threadId), value.owner);
  };
  install(session);
  const action = (op: string, bridge = f.native, start = 7, end = 12) => mobileVoiceAction(op, start, end, 'Queued task',
    mobileComposerTarget(f.client).editorOwner, bridge, f.storage, f.client);
  return { ...f, session, state, install, action, text: () => mobileQueuedEditLookup(session.owner, f.client)?.text };
}
describe('queued voice captured content target, seeded real state and mocked native', () => {
  test('completion writes only dedicated content and persists with the finishing answer', async () => {
    const f = queuedVoiceFixture(); let valid = true;
    const starting: Native = { ...f.native, later: input => { if (!valid) throw new Error('expired Start'); return f.native.later(input); } };
    await f.action('start', starting); valid = false;
    expect((await f.action('stop')).data.phase).toBe('idle');
    expect(f.text()).toBe('Queued spoken words'); expect(f.client.draft).toBe('Hello world'); expect(f.saves()).toBe(0);
    expect(f.calls.find(call => call.action === 'selection-commit')?.owner).toBe(f.session.owner);
    expect(f.calls.find(call => call.op === 'mobileQueuedEdit' && call.action === 'cas')).toMatchObject({ owner: f.session.owner,
      record: { text: 'Queued spoken words', draftKey: f.session.draftKey } });
  });
  test('navigation allows completion into the still-existing captured edit', async () => {
    const f = queuedVoiceFixture(); await f.action('start');
    f.client.threadId = 'two'; f.client.local.drafts['env:two'] = 'Other';
    expect(mobileVoiceSnapshot('env:two', f.client)).toMatchObject({ focused: false, globalVisible: true });
    await f.action('stop'); expect(f.text()).toBe('Queued spoken words');
    expect(f.client.threadId).toBe('two'); expect(f.client.draft).toBe('Other'); expect(f.client.local.drafts['env:one']).toBe('Hello world');
  });
  test('ending an edit while transcription awaits cannot fall back to ordinary content', async () => {
    const f = queuedVoiceFixture(); await f.action('start'); const original = f.native.later;
    f.native.later = async input => { if (obj(input).action === 'transcribe') queuedEditEndMemory(f.session.owner, f.client); return original(input); };
    const result = await f.action('stop'); expect(result.data.error).toContain('draft changed');
    expect(f.text()).toBeUndefined(); expect(f.client.draft).toBe('Hello world');
    expect(f.calls.some(call => call.action === 'selection-commit' || call.op === 'mobileQueuedEdit')).toBe(false);
  });
  test('same-key replacement session rejects old transcription even with identical text', async () => {
    const f = queuedVoiceFixture(); await f.action('start'); queuedEditEndMemory(f.session.owner, f.client);
    const replacement = { ...f.session, owner: 'queued-session-two', session: 'session-two' }; f.install(replacement);
    expect(mobileVoiceSnapshot(replacement.owner, f.client).focused).toBe(false);
    expect((await f.action('stop')).data.error).toContain('draft changed');
    expect(mobileQueuedEditLookup(replacement.owner, f.client)?.text).toBe('Queued world'); expect(f.client.draft).toBe('Hello world');
  });
  test('edit then revert between all voice observations still invalidates transcription', async () => {
    const f = queuedVoiceFixture(); await f.action('start');
    mobileQueuedEditWriteText(f.session.owner, 'Changed', f.client); mobileQueuedEditWriteText(f.session.owner, 'Queued world', f.client);
    expect((await f.action('stop')).data.error).toContain('draft changed'); expect(f.text()).toBe('Queued world');
    expect(f.calls.some(call => call.action === 'selection-commit' || call.op === 'mobileQueuedEdit')).toBe(false);
  });
  test('a saving edit refusing WriteText never receives a caret commit or persistence', async () => {
    const f = queuedVoiceFixture(); await f.action('start'); const original = f.native.later;
    f.native.later = async input => {
      if (obj(input).action === 'transcribe') f.state.sessions.set(f.session.owner, { ...mobileQueuedEditLookup(f.session.owner, f.client)!, saving: true });
      return original(input);
    };
    expect((await f.action('stop')).data.phase).toBe('error'); expect(f.text()).toBe('Queued world');
    expect(f.calls.some(call => call.action === 'selection-commit' || call.op === 'mobileQueuedEdit')).toBe(false);
  });
  test('a replaced edit during selection readiness cannot start recording', async () => {
    const f = queuedVoiceFixture(), original = f.native.later;
    f.native.later = async input => {
      if (obj(input).action === 'selection') {
        queuedEditEndMemory(f.session.owner, f.client); f.install({ ...f.session, owner: 'replacement' });
        return { ok: true, generation: f.client.generation, value: { start: 7, end: 12 } };
      }
      return original(input);
    };
    expect((await f.action('start', f.native, -1, -1)).message).toContain('draft changed');
    expect(f.calls.some(call => call.action === 'permission')).toBe(false);
  });
  test('generation changes invalidate a retained queued session', async () => {
    const f = queuedVoiceFixture(); await f.action('start'); f.client.generation++;
    expect((await f.action('stop')).data.error).toContain('draft changed'); expect(f.text()).toBe('Queued world');
  });
  test('queued caret is invalidated by a later unobserved ABA or session end', async () => {
    const f = queuedVoiceFixture(); await f.action('start'); await f.action('stop');
    expect(mobileVoiceSnapshot(f.session.owner, f.client).selectionOwner).toBe(f.session.owner);
    mobileQueuedEditWriteText(f.session.owner, 'Changed', f.client); mobileQueuedEditWriteText(f.session.owner, 'Queued spoken words', f.client);
    expect(mobileVoiceSnapshot(f.session.owner, f.client).selectionOwner).toBe('');
    await f.action('start'); await f.action('stop'); queuedEditEndMemory(f.session.owner, f.client);
    expect(mobileVoiceSnapshot(f.session.owner, f.client).selectionOwner).toBe('');
  });
});


test('queued native editor owner changes for same-draft-key replacement', async () => {
  const f = queuedVoiceFixture(); await f.action('start'); await f.action('stop');
  const old = mobileComposerTarget(f.client); expect(old.editorOwner).toBe(f.session.owner);
  queuedEditEndMemory(f.session.owner, f.client);
  f.install({ ...f.session, owner: 'new-unique-editor', session: 'new-unique-session' });
  const next = mobileComposerTarget(f.client);
  expect(next.key).toBe(old.key); expect(next.editorOwner).not.toBe(old.editorOwner);
  const snapshot = mobileVoiceSnapshot(next.editorOwner, f.client);
  expect(snapshot.selectionOwner).toBe(''); expect(snapshot.focused).toBe(false);
});


test('external picker caret uses root token and invalidates queued ABA without a voice target', async () => {
  const { mobileComposerStageSelection } = await import('./voice-data');
  const f = queuedVoiceFixture(), target = mobileComposerTarget(f.client);
  await mobileComposerStageSelection(f.client, target, 'Queued world', 3, 3, f.native);
  const snapshot = mobileVoiceSnapshot(target.editorOwner, f.client);
  const commit = f.calls.find(call => call.action === 'selection-commit')!;
  expect(snapshot.selectionOwner).toBe(target.editorOwner); expect(commit.revision).toBe(snapshot.selectionRevision);
  expect(commit.start).toBe(3); expect(f.calls.some(call => call.action === 'record')).toBe(false);
  mobileQueuedEditWriteText(f.session.owner, 'Changed', f.client); mobileQueuedEditWriteText(f.session.owner, 'Queued world', f.client);
  expect(mobileVoiceSnapshot(target.editorOwner, f.client).selectionOwner).toBe('');
});

test('late external caret reply cannot clear a newer selection', async () => {
  const { mobileComposerStageSelection } = await import('./voice-data');
  const f = queuedVoiceFixture(), target = mobileComposerTarget(f.client), original = f.native.later;
  let release!: () => void;
  f.native.later = async input => { if (obj(input).action === 'selection-commit' && obj(input).start === 1) await new Promise<void>(resolve => { release = resolve; }); return original(input); };
  const old = mobileComposerStageSelection(f.client, target, 'Queued world', 1, 1, f.native);
  await Promise.resolve();
  await mobileComposerStageSelection(f.client, target, 'Queued world', 2, 2, f.native);
  release(); await expect(old).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileVoiceSnapshot(target.editorOwner, f.client)).toMatchObject({ selectionOwner: target.editorOwner, selectionStart: 2 });
});
