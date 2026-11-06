import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { mobileKeyboardTarget, mobileKeyboardSnapshot, mobileKeyboardCopy, mobileKeyboardDismiss } from './mobile-keyboard';
import type { Native } from './shared/protocol';

function fixture() {
  const client = new T3Client(); client.environmentId = 'env/one'; client.threadId = 'thread one';
  client.shell.threads = [{ id: 'thread one', projectId: 'p', title: 'Thread', pullRequests: [] }];
  const location = '/threads/env%2Fone/thread%20one';
  const snapshot = () => mobileKeyboardSnapshot(client, 'visit', location);
  const event = () => { const data = snapshot(); return JSON.stringify({ routeKey: data.routeKey, version: data.version, kind: 'copyThreadReference' }); };
  const requests: Record<string, unknown>[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = input as Record<string, unknown>; requests.push(request);
    return { ok: true, generation: -1, value: { copied: request.op === 'copyText' } };
  } };
  return { client, location, snapshot, event, native, requests };
}
function deferred() { let resolve!: (value: unknown) => void; const promise = new Promise<unknown>(answer => { resolve = answer; }); return { promise, resolve }; }

describe('mobile hardware copy reference', () => {
  test('copies active route current PR, ignores dismissed/terminal PR and never requests server authority', async () => {
    const f = fixture(); f.client.shell.threads[0]!.pullRequests = [
      { source: 'stack-dismissed', url: 'https://pr/dismissed', linkedAt: '2026-10-09' },
      { url: 'https://pr/closed', snapshot: { state: 'merged' }, linkedAt: '2026-10-08' },
      { url: 'https://pr/open', snapshot: { state: 'open' }, linkedAt: '2026-10-07' },
    ];
    f.client.shell.threads[0]!.linkedPullRequest = { url: 'https://pr/legacy' };
    await mobileKeyboardCopy(f.client, f.event(), f.native);
    expect(f.requests).toEqual([{ op: 'copyText', text: 'https://pr/open' }, { op: 'mobileHomeHaptic', kind: 'light' }]);
    expect(f.snapshot()).toMatchObject({ phase: 'success', label: 'PR link copied', description: 'https://pr/open' });
  });
  test('linked PR takes precedence over branch PR; missing linked value falls back exactly as source', () => {
    const f = fixture(), thread = f.client.shell.threads[0]!;
    thread.linkedPullRequest = { url: 'https://pr/linked' }; thread.branchPullRequest = { url: 'https://pr/branch' };
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('https://pr/linked');
    thread.linkedPullRequest = null;
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('https://pr/branch');
    thread.linkedPullRequest = {};
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('thread one');
  });
  test('route-ID fallback before focus adoption cannot borrow another selected environment/thread PR', () => {
    const f = fixture(); f.client.shell.threads[0]!.linkedPullRequest = { url: 'https://pr/wrong' };
    f.client.environmentId = 'other';
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('thread one');
    f.client.environmentId = 'env/one'; f.client.threadId = 'other';
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('thread one');
    f.client.threadId = ''; f.client.shell.threads = [];
    expect(mobileKeyboardTarget(f.client, f.location)?.value).toBe('thread one');
  });
  test('terminal, new task, non-thread and malformed routes install no command', async () => {
    const f = fixture();
    for (const location of ['/', '/settings', '/new/draft', `${f.location}/terminal`, `${f.location}/terminal/options`, '/threads/a/%ZZ', '/threads/a/new-task%3Adraft']) {
      expect(mobileKeyboardSnapshot(f.client, 'visit', location).enabled).toBe(false);
      await mobileKeyboardCopy(f.client, JSON.stringify({ routeKey: 'visit', version: 'old', kind: 'copyThreadReference' }), f.native);
    }
    expect(f.requests).toEqual([]);
    expect(mobileKeyboardTarget(f.client, `${f.location}/files/a?line=2`)?.value).toBe('thread one');
  });
  test('clipboard refusal, missing native and throw produce exact failure; haptic failure preserves clipboard success', async () => {
    for (const mode of ['false', 'throw', 'missing', 'haptic']) {
      const f = fixture(), native = { ...f.native, async later(input: unknown) {
        const op = (input as { op: string }).op;
        if (mode === 'throw' && op === 'copyText' || mode === 'haptic' && op === 'mobileHomeHaptic') throw new Error('unavailable');
        return { ok: true, generation: -1, value: { copied: mode === 'haptic' } };
      } };
      await mobileKeyboardCopy(f.client, f.event(), mode === 'missing' ? null : native);
      expect(f.snapshot()).toMatchObject(mode === 'haptic'
        ? { phase: 'success', label: 'Thread ID copied', description: 'thread one' }
        : { phase: 'error', label: 'Failed to copy thread ID', description: 'Try again.' });
    }
  });
  test('latest copy wins and a captured old dismissal cannot clear newer feedback', async () => {
    const f = fixture(), first = deferred(); let count = 0;
    const native: Native = { ...f.native, later(input) {
      if ((input as { op: string }).op === 'copyText' && ++count === 1) return first.promise;
      return f.native.later(input);
    } };
    const old = mobileKeyboardCopy(f.client, f.event(), native);
    await mobileKeyboardCopy(f.client, f.event(), native);
    const newest = f.snapshot();
    first.resolve({ ok: true, generation: -1, value: { copied: false } }); await old;
    expect(f.snapshot()).toMatchObject({ phase: 'success', feedbackId: newest.feedbackId });
    mobileKeyboardDismiss(f.client, newest.feedbackId - 1);
    expect(f.snapshot().phase).toBe('success');
    mobileKeyboardDismiss(f.client, newest.feedbackId);
    expect(f.snapshot().phase).toBe('');
  });
  test('navigation retires held reply and old native callback without another clipboard call', async () => {
    const f = fixture(), held = deferred(), event = f.event();
    const native = { ...f.native, later(input: unknown) { return (input as { op: string }).op === 'copyText' ? held.promise : f.native.later(input); } };
    const copy = mobileKeyboardCopy(f.client, event, native);
    mobileKeyboardSnapshot(f.client, 'next', '/');
    held.resolve({ ok: true, generation: -1, value: { copied: true } }); await copy;
    await mobileKeyboardCopy(f.client, event, f.native);
    expect(mobileKeyboardSnapshot(f.client, 'next', '/').phase).toBe('');
    expect(f.requests).toEqual([{ op: 'mobileHomeHaptic', kind: 'light' }]);
  });
  test('changed adopted PR between admission and native callback is rejected synchronously', async () => {
    const f = fixture(), event = f.event();
    f.client.shell.threads[0]!.branchPullRequest = { url: 'https://pr/new' };
    await mobileKeyboardCopy(f.client, event, f.native);
    expect(f.requests).toEqual([]);
  });
  test('let-go of either answer retires the entire copy invocation without success or failure feedback', async () => {
    for (const cancelled of ['copyText', 'mobileHomeHaptic']) {
      const f = fixture(), error = { name: 'FetchError', kind: 'Aborted' };
      const native: Native = { ...f.native, async later(input) {
        if ((input as { op: string }).op === cancelled) throw error;
        return f.native.later(input);
      } };
      let caught: unknown;
      try { await mobileKeyboardCopy(f.client, f.event(), native); } catch (value) { caught = value; }
      expect(caught).toBe(error);
      expect(f.snapshot()).toMatchObject({ phase: '', label: '', description: '', feedbackId: 0 });
    }
  });
});
