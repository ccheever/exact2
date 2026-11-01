import { describe, expect, test } from 'bun:test';
import { hasCompleteProviderWorkspaceSnapshot, workspaceValues, WorkspaceDiscovery } from '../../src/composer/composer-workspace-snapshots';
import type { Obj } from '../../src/client/domain';

const partial = { instanceId: 'p', skills: [{ name: 'global' }], slashCommands: [], workspaceSnapshots: [
  { cwd: '/repo', slashCommandsPending: true, skills: [{ name: 'repo-skill' }], slashCommands: [{ name: 'pending-command' }] },
] };
const complete = { ...partial, workspaceSnapshots: [{ cwd: '/repo', skills: [], slashCommands: [] }] };
describe('workspace provider discovery', () => {
  test('expires completed workspace scans after five minutes and bounds requests when the server clock lags', async () => {
    const discovery = new WorkspaceDiscovery(), now = Date.parse('2026-10-18T00:00:00Z');
    const provider = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: new Date(now).toISOString() }] };
    const wake = { prompt: '', config: provider };
    expect(discovery.state('env', 1, provider, '/repo', wake, true, now).needed).toBe(false);
    const stale = discovery.state('env', 1, provider, '/repo', wake, true, now + 300_000);
    expect(stale.needed).toBe(true);
    // The lagging server's new scan is still older than the TTL by this clock; its new timestamp is the progress.
    const rescanned = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: new Date(now + 1).toISOString() }] };
    expect((await discovery.refresh(stale.key, async () => ({ providers: [rescanned] }))).retry).toBe(false);
    expect(discovery.state('env', 1, rescanned, '/repo', { prompt: '', config: rescanned }, true, now + 300_001).needed).toBe(false);
    expect(discovery.state('env', 1, rescanned, '/repo', { prompt: '', config: rescanned }, true, now + 600_000).needed).toBe(true);
  });
  test('a rescan that returns the same expired catalog is retried, three times at most (providerSkills.ts)', async () => {
    const discovery = new WorkspaceDiscovery(), now = Date.parse('2026-10-18T00:05:00Z');
    const stale = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: '2026-10-18T00:00:00.000Z' }] };
    const wake = { prompt: '', config: stale };
    let calls = 0;
    for (const attempt of [1, 2, 3]) {
      const state = discovery.state('env', 1, stale, '/repo', wake, true, now + attempt * 10_000);
      expect(state.needed).toBe(true);
      expect((await discovery.refresh(state.key, async () => { calls++; return { providers: [stale] }; })).retry).toBe(attempt < 3);
    }
    expect(calls).toBe(3);
    expect(discovery.state('env', 1, stale, '/repo', wake, true, now + 60_000).needed).toBe(false);
    const fresh = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: '2026-10-18T00:05:10.000Z' }] };
    const again = new WorkspaceDiscovery(), first = again.state('env', 1, stale, '/repo', wake, true, now);
    expect((await again.refresh(first.key, async () => ({ providers: [stale] }))).retry).toBe(true);
    const second = again.state('env', 1, stale, '/repo', wake, true, now + 10_000);
    expect((await again.refresh(second.key, async () => ({ providers: [fresh] }))).retry).toBe(false);
  });
  test('uses partial workspace skills and commands while keeping discovery retryable', () => {
    expect(hasCompleteProviderWorkspaceSnapshot(partial, '/repo')).toBe(false);
    expect(workspaceValues(partial, '/repo', 'skills')).toEqual([{ name: 'repo-skill' }]);
    expect(workspaceValues(partial, '/repo', 'slashCommands')).toEqual([{ name: 'pending-command' }]);
    expect(workspaceValues(partial, '/other', 'skills')).toEqual([{ name: 'global' }]);
  });
  test('asks the root to retry incomplete responses and stops after completion', async () => {
    const discovery = new WorkspaceDiscovery(); const calls: Obj[] = [];
    const initial = discovery.state('env', 1, partial, '/repo');
    const request = async (method: string, payload: Obj) => { calls.push({ method, ...payload }); return { providers: [partial] }; };
    expect(await discovery.refresh(initial.key, request)).toMatchObject({ key: initial.key, retry: true });
    expect(discovery.state('env', 1, partial, '/repo').needed).toBe(true);
    expect(await discovery.refresh(initial.key, async () => ({ providers: [complete] }))).toMatchObject({ key: initial.key, retry: false });
    expect(discovery.state('env', 1, partial, '/repo').needed).toBe(false);
    await discovery.refresh(initial.key, request); expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({ method: 'server.refreshProviders', instanceId: 'p', cwd: '/repo' });
  });
  test('a cwd that never scans stops after three failures until the TTL from its last request (useProviderWorkspaceSnapshotRefresh)', async () => {
    const discovery = new WorkspaceDiscovery(), now = Date.parse('2026-10-25T00:00:00Z'), wake = { prompt: '', config: partial };
    let calls = 0;
    const failing = async () => { calls++; throw new Error('no such directory'); };
    for (const attempt of [1, 2, 3]) {
      const state = discovery.state('env', 1, partial, '/gone', wake, true, now + attempt);
      expect(state.needed).toBe(true);
      expect((await discovery.refresh(state.key, failing)).retry).toBe(attempt < 3);
    }
    expect(calls).toBe(3);
    expect(discovery.state('env', 1, partial, '/gone', wake, true, now + 60_000).needed).toBe(false);
    expect(discovery.state('env', 1, partial, '/gone', wake, true, now + 3 + 300_000).needed).toBe(true);
  });
  test('the failure count starts over when the composer mounts again or a complete snapshot arrives (providerSkills.ts)', async () => {
    const now = Date.parse('2026-10-25T00:00:00Z'), failing = async () => { throw new Error('no such directory'); };
    const state = (discovery: WorkspaceDiscovery, provider: Obj, cwd: string, at: number, shown = true) => discovery.state('env', 1, provider, cwd, { prompt: '', config: provider }, shown, at);
    const exhaust = async (discovery: WorkspaceDiscovery, cwd: string) => {
      for (const attempt of [1, 2, 3]) await discovery.refresh(state(discovery, partial, cwd, now + attempt).key, failing);
    };
    // Mounted again after its last failure: the next failure is the first of three.
    const remounted = new WorkspaceDiscovery();
    await exhaust(remounted, '/gone');
    state(remounted, partial, '/gone', now + 10, false);
    expect((await remounted.refresh(state(remounted, partial, '/gone', now + 11).key, failing)).retry).toBe(true);
    // A complete snapshot by subscription: once it goes stale, a failure is again the first of three.
    const subscribed = new WorkspaceDiscovery(), current = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: new Date(now + 10).toISOString() }] };
    await exhaust(subscribed, '/repo');
    state(subscribed, current, '/repo', now + 10);
    const stale = state(subscribed, partial, '/repo', now + 3 + 300_001);
    expect(stale.needed).toBe(true);
    expect((await subscribed.refresh(stale.key, failing)).retry).toBe(true);
  });
  test('a folder the composer moved from loses its count; a stale complete snapshot does not end one', async () => {
    const now = Date.parse('2026-10-25T00:00:00Z'), failing = async () => { throw new Error('no such directory'); };
    const state = (discovery: WorkspaceDiscovery, provider: Obj, cwd: string, at: number) => discovery.state('env', 1, provider, cwd, { prompt: '', config: provider }, true, at);
    // Three failures on /gone, then another folder, then /gone again: a fresh count.
    const moved = new WorkspaceDiscovery();
    for (const attempt of [1, 2, 3]) await moved.refresh(state(moved, partial, '/gone', now + attempt).key, failing);
    expect(state(moved, partial, '/gone', now + 10).needed).toBe(false);
    state(moved, partial, '/other', now + 11);
    const back = state(moved, partial, '/gone', now + 12);
    expect(back.needed).toBe(true);
    expect((await moved.refresh(back.key, failing)).retry).toBe(true);
    // A complete snapshot checked long ago may be all a failing scan returns: the count holds until the TTL.
    const old = { ...complete, workspaceSnapshots: [{ ...complete.workspaceSnapshots[0], checkedAt: new Date(now - 86_400_000).toISOString() }] };
    const stale = new WorkspaceDiscovery();
    for (const attempt of [1, 2, 3]) await stale.refresh(state(stale, old, '/repo', now + attempt).key, failing);
    expect(state(stale, old, '/repo', now + 10).needed).toBe(false);
    const next = state(stale, old, '/repo', now + 3 + 300_001);
    expect(next.needed).toBe(true);
    expect((await stale.refresh(next.key, failing)).retry).toBe(true);
  });
  test('retries failures and keeps delimiter-containing keys distinct', async () => {
    const discovery = new WorkspaceDiscovery();
    const first = discovery.state('a:b', 1, { instanceId: 'c' }, '/repo');
    expect(await discovery.refresh(first.key, async () => { throw new Error('offline'); })).toMatchObject({ key: first.key, retry: true });
    const second = discovery.state('a', 1, { instanceId: 'b:c' }, '/repo');
    expect(second.key).not.toBe(first.key);
    expect(discovery.state('a', 2, { instanceId: 'b:c' }, '/repo').key).not.toBe(second.key);
  });
  test('coalesces overlapping mutations and never dispatches an obsolete selection', async () => {
    const discovery = new WorkspaceDiscovery(); let resolve!: (value: Obj) => void; let calls = 0;
    const request = () => { calls++; return new Promise<Obj>(done => { resolve = done; }); };
    const { key } = discovery.state('env', 1, partial, '/repo');
    const first = discovery.refresh(key, request), second = discovery.refresh(key, request);
    expect(calls).toBe(1); resolve({ providers: [partial] }); await Promise.all([first, second]);
    discovery.state('other', 1, partial, '/repo');
    await discovery.refresh(key, request); expect(calls).toBe(1);
  });
  test('late completion cannot suppress discovery after a snapshot reset', async () => {
    const discovery = new WorkspaceDiscovery(); let resolve!: (value: Obj) => void;
    const initial = discovery.state('env', 1, partial, '/repo');
    const old = discovery.refresh(initial.key, () => new Promise<Obj>(done => { resolve = done; }));
    discovery.state('env', 1, complete, '/repo');
    const reset = discovery.state('env', 1, partial, '/repo');
    expect(reset.key).not.toBe(initial.key);
    resolve({ providers: [complete] }); await old;
    expect(discovery.state('env', 1, partial, '/repo')).toMatchObject({ key: reset.key, needed: true });
    let calls = 0;
    await discovery.refresh(reset.key, async () => { calls++; return { providers: [partial] }; });
    expect(calls).toBe(1);
  });
  test('a config snapshot arriving during a request wins over a partial response', async () => {
    const discovery = new WorkspaceDiscovery(); let resolve!: (value: Obj) => void;
    const { key } = discovery.state('env', 1, partial, '/repo');
    const pending = discovery.refresh(key, () => new Promise<Obj>(done => { resolve = done; }));
    discovery.state('env', 1, complete, '/repo');
    resolve({ providers: [partial] }); expect((await pending).retry).toBe(false);
  });
  // ChatComposer.tsx:2195-2268: only a pending snapshot arms the retry timer; otherwise
  // the root retries after the cooldown when `wake` moved past the completion's.
  test('counts prompt and provider-list changes as wakes for a failed or snapshot-less answer', async () => {
    const discovery = new WorkspaceDiscovery(), provider = { instanceId: 'codex' }, config = { providers: [provider] };
    const first = discovery.state('env', 1, provider, '/repo', { prompt: '', config });
    expect(first).toMatchObject({ needed: true, timer: false });
    const failed = await discovery.refresh(first.key, async () => { throw new Error('probe failed'); });
    expect(failed).toEqual({ key: first.key, retry: true, wake: first.wake });
    expect(discovery.state('env', 1, provider, '/repo', { prompt: '', config }).wake).toBe(failed.wake);
    expect(discovery.state('env', 1, provider, '/repo', { prompt: 'h', config }).wake).toBe(failed.wake + 1);
    expect(discovery.state('env', 1, provider, '/repo', { prompt: 'h', config: { providers: [provider] } }).wake).toBe(failed.wake + 2);
    const empty = await discovery.refresh(first.key, async () => ({ providers: [provider] }));
    expect(empty).toMatchObject({ retry: true, wake: failed.wake + 2 });
  });
  test('a pending workspace snapshot keeps the retry timer', async () => {
    const discovery = new WorkspaceDiscovery(), config = { providers: [partial] };
    const { key, timer } = discovery.state('env', 1, partial, '/repo', { prompt: '', config });
    expect(timer).toBe(true);
    expect(await discovery.refresh(key, async () => ({ providers: [partial] }))).toMatchObject({ key, retry: true });
    expect(discovery.state('env', 1, partial, '/repo', { prompt: '', config })).toMatchObject({ needed: true, timer: true });
  });
  test('changes seen while the request is out are part of the completion', async () => {
    const discovery = new WorkspaceDiscovery(), provider = { instanceId: 'codex' }, config = { providers: [provider] };
    let resolve!: (value: Obj) => void;
    const { key, wake } = discovery.state('env', 1, provider, '/repo', { prompt: '', config });
    const pending = discovery.refresh(key, () => new Promise<Obj>(done => { resolve = done; }));
    discovery.state('env', 1, provider, '/repo', { prompt: 'typed while out', config });
    resolve({ providers: [provider] }); expect(await pending).toEqual({ key, retry: true, wake: wake + 1 });
  });
  // #298: with Settings › Providers open, a pending snapshot kept the root's
  // retry clock sending server.refreshProviders every ~11 s. The reference's ChatComposer is unmounted on the
  // settings, usage, pull request and welcome routes, and mounting it again starts its refs over.
  test('an unmounted composer asks for nothing; mounting it again refreshes at once with a fresh attempt', async () => {
    const discovery = new WorkspaceDiscovery(), config = { providers: [partial] }; let calls = 0;
    const request = async () => { calls++; return { providers: [partial] }; };
    const mounted = discovery.state('env', 1, partial, '/repo', { prompt: '', config }, true);
    expect(await discovery.refresh(mounted.key, request)).toMatchObject({ key: mounted.key, retry: true });
    const hidden = discovery.state('env', 1, partial, '/repo', { prompt: '', config }, false);
    expect(hidden).toMatchObject({ key: '', needed: false, timer: false });
    // A mutation the root sent just before the route changed sends nothing either.
    expect(await discovery.refresh(mounted.key, request)).toMatchObject({ retry: false });
    // The mutation's own recheck keeps the last answer's visibility.
    expect(discovery.state('env', 1, partial, '/repo', { prompt: '', config })).toMatchObject({ key: '', needed: false });
    expect(calls).toBe(1);
    const again = discovery.state('env', 1, partial, '/repo', { prompt: '', config }, true);
    expect(again).toMatchObject({ needed: true, timer: true });
    expect(again.key).not.toBe(mounted.key);
    await discovery.refresh(again.key, request); expect(calls).toBe(2);
    // A complete snapshot stays complete across the remount: nothing to refresh.
    discovery.state('env', 1, complete, '/repo', { prompt: '', config }, false);
    expect(discovery.state('env', 1, complete, '/repo', { prompt: '', config }, true)).toMatchObject({ needed: false });
  });
});
