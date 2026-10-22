import { expect, test } from 'bun:test';
import { answer } from './app';
import { mobileClient } from './client';

function clientFixture() {
  const old = { environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, shell: mobileClient.shell };
  mobileClient.environmentId = 'keyboard-env'; mobileClient.threadId = 'keyboard-thread';
  mobileClient.shell = { ...old.shell, threads: [{ id: 'keyboard-thread', pullRequests: [] }] };
  return () => { Object.assign(mobileClient, old); answer('keyboardSnapshot', ['', '/', 0]); };
}
const event = (snapshot: { routeKey: string; version: string }) => JSON.stringify({ routeKey: snapshot.routeKey, version: snapshot.version, kind: 'copyThreadReference' });
const location = '/threads/keyboard-env/keyboard-thread';

test('keyboard root exporters admit current route, copy and dismiss only the displayed feedback', async () => {
  const restore = clientFixture(), requests: unknown[] = [];
  try {
    const initial = answer('keyboardSnapshot', ['keyboard-visit', location, 0]);
    const native = { available: true, watch() {}, async later(input: unknown) { requests.push(input); return { ok: true, generation: -1, value: { copied: true } }; } };
    await answer('keyboardCopy', [event(initial), 'keyboard-visit', location], undefined, undefined, native);
    expect(requests).toContainEqual({ op: 'copyText', text: 'keyboard-thread' });
    const shown = answer('keyboardSnapshot', ['keyboard-visit', location, 0]);
    expect(shown.phase).toBe('success'); expect(shown.description).toBe('keyboard-thread');
    const firstId = shown.feedbackId;
    await answer('keyboardCopy', [event(shown), 'keyboard-visit', location], undefined, undefined, native);
    answer('keyboardDismiss', [firstId]);
    const newer = answer('keyboardSnapshot', ['keyboard-visit', location, 0]);
    expect(newer.phase).toBe('success'); expect(newer.feedbackId).not.toBe(firstId);
    answer('keyboardDismiss', [newer.feedbackId]);
    expect(answer('keyboardSnapshot', ['keyboard-visit', location, 0]).phase).toBe('');
  } finally { restore(); }
});

test('root copy admission retires stale native command before delayed snapshot observation', async () => {
  const restore = clientFixture(); let calls = 0;
  try {
    const old = answer('keyboardSnapshot', ['old-visit', location, 0]);
    const native = { available: true, watch() {}, async later() { calls++; throw new Error('stale key command reached clipboard'); } };
    // The current navigation arguments differ, even though no snapshot source ran yet.
    await answer('keyboardCopy', [event(old), 'settings-visit', '/settings'], undefined, undefined, native);
    expect(calls).toBe(0);
    const current = answer('keyboardSnapshot', ['settings-visit', '/settings', 0]);
    expect(current.enabled).toBe(false); expect(current.phase).toBe('');
  } finally { restore(); }
});
