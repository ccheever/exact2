import { beforeEach, describe, expect, test } from 'bun:test';
import { mobileDraftChanged } from './draft';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { mobileVoiceAction, mobileVoiceObserveDraft, mobileVoiceSnapshot, mobileVoiceStatus } from './voice-data';
import { resetVoiceInputGlobalsForTests } from './voice-controller';

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
    const f = fixture(); expect((await f.action('start')).data.phase).toBe('recording');
    expect((await f.action('stop')).data.phase).toBe('idle');
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
