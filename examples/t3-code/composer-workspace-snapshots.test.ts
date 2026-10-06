import { describe, expect, test } from 'bun:test';
import { hasCompleteProviderWorkspaceSnapshot, workspaceValues, WorkspaceDiscovery } from './composer-workspace-snapshots';
import type { Obj } from './domain';

const partial = { instanceId: 'p', skills: [{ name: 'global' }], slashCommands: [], workspaceSnapshots: [
  { cwd: '/repo', slashCommandsPending: true, skills: [{ name: 'repo-skill' }], slashCommands: [{ name: 'pending-command' }] },
] };
const complete = { ...partial, workspaceSnapshots: [{ cwd: '/repo', skills: [], slashCommands: [] }] };
describe('workspace provider discovery', () => {
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
});
