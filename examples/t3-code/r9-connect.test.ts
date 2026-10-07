// Lane r9-connect: Settings › Connections lists every paired server as a saved
// environment, a failed pairing saves nothing, and a 1970 onboarding time from an
// older build reads as unset; the branch picker checks a pull request out (PullRequestThreadDialog).
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { noPrimary, resetPrimary } from './local-primary-fixture';
// These cases are the hosted rules (resolveHostedFirstRunDecision): no embedded server runs on this Mac.
beforeEach(noPrimary);
afterEach(resetPrimary);
import { setRuntimeClock } from './sidebar-state';
import { connectionsProjection, runConnectionOp, type ConnectionHost } from './connections';
import { fleet } from './settings-b-fleet';
import { adoptPagesPrefs, pagesPrefs } from './pages-prefs';
import { welcomeView } from './pages-welcome';
import { APP_EPOCH, storedCompletion } from './r9-connect-onboarding';
import { obj, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { checkoutConfirm, checkoutItems, checkoutLocal, checkoutView, checkoutState } from './r9-connect-checkout';

// The data runtime has no clock (sidebar-state.ts `clock`); these tests stand in for a host clock that reads Date.now.
beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => setRuntimeClock(() => Number.NaN));

/** A transport whose pairing exchange refuses one code, as the server's /oauth/token does. */
class Transport implements Native {
  available = true; calls: Obj[] = [];
  constructor(public saved: Obj[] = []) {}
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const ok = (value: unknown) => ({ ok: true, generation: 1, value });
    if (request.fleet) return ok({});
    if (request.op === 'environments') return ok({ saved: this.saved });
    if (request.op === 'connect') {
      if (request.credential === 'WRONGCODE') return { ok: false, generation: 1, error: { kind: 'Authentication', message: 'The environment credential is invalid.', uncertain: false } };
      return ok({ state: 'connected', origin: request.origin, environmentId: 'env-x', message: '' });
    }
    if (request.op === 'disconnect') return ok({ state: 'disconnected', origin: '', environmentId: '', message: 'Disconnected.' });
    if (request.op === 'pairEnvironment') return { ok: false, generation: 1, error: { kind: 'Authentication', message: 'The environment credential is invalid.', uncertain: false } };
    return { ok: false, generation: 1, error: { kind: 'Arguments', message: 'Unknown native operation.', uncertain: false } };
  }
}
const host = (connection: string, extra: Partial<ConnectionHost> = {}): ConnectionHost => ({
  connection, origin: 'http://127.0.0.1:14942', environmentId: 'env-x', statusMessage: 'The environment credential is invalid.', scopes: [], config: {}, ...extra,
});

describe('a failed pairing saves nothing', () => {
  test('a failed focus that was never saved is not an Environments row', () => {
    // The transport tried the link and stopped on the credential: the reference registers only a validated connection.
    expect(connectionsProjection(host('error'), []).environments).toEqual([]);
    expect(connectionsProjection(host('connecting'), []).environments).toEqual([]);
    // Once it connects it is listed before the catalog catches up; a saved one stays listed when it fails later.
    expect(connectionsProjection(host('connected'), []).environments.map(row => row.environmentId)).toEqual(['env-x']);
    const saved = [{ origin: 'http://127.0.0.1:14942', environmentId: 'env-x', label: 'Lane', enabled: true }];
    expect(connectionsProjection(host('error'), saved).environments.map(row => [row.label, row.subtitle])).toEqual([
      ['Lane', 'http://127.0.0.1:14942/ · Connection failed: The environment credential is invalid.']]);
  });
  test('with nothing connected, the wrong code leaves no connection behind', async () => {
    const native = new Transport();
    await expect(runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14942', 'WRONGCODE', false)).rejects.toThrow('The environment credential is invalid.');
    expect(native.calls.filter(call => !call.fleet).map(call => call.op)).toEqual(['connect', 'environments', 'disconnect']);
    expect(native.saved).toEqual([]);
    fleet.entries.clear();
  });
  test('a saved environment that was focused before the attempt is reconnected with its own credential', async () => {
    const native = new Transport([{ origin: 'http://127.0.0.1:14900', environmentId: 'env-home', label: 'Home', enabled: true }]);
    const client = { origin: 'http://127.0.0.1:14900/', environmentId: 'env-home', connection: 'reconnecting' } as unknown as T3Client;
    await expect(runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14942', 'WRONGCODE', false, client)).rejects.toThrow('invalid');
    expect(native.calls.filter(call => !call.fleet && call.op === 'connect').map(call => [call.origin, call.credential])).toEqual([
      ['http://127.0.0.1:14942', 'WRONGCODE'], ['http://127.0.0.1:14900', '']]);
    fleet.entries.clear();
  });
  test('beside a connected focus the pairing never touches the focus', async () => {
    const native = new Transport();
    await expect(runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14942', 'WRONGCODE', true)).rejects.toThrow('invalid');
    expect(native.calls.filter(call => !call.fleet).map(call => call.op)).toEqual(['pairEnvironment']);
  });
});

describe('a stored 1970 onboarding time is repaired', () => {
  test('values before the app existed read as unset; real ones are kept', () => {
    expect(APP_EPOCH).toBe(Date.parse('2026-02-07T00:00:00.000Z'));
    expect(storedCompletion('1970-01-01T00:01:58Z')).toBe('');
    expect(storedCompletion('1970-01-01T00:01:58.000Z')).toBe('');
    expect(storedCompletion('2025-12-31T23:59:59Z')).toBe('');
    expect(storedCompletion('yesterday')).toBe('');
    expect(storedCompletion(42)).toBe('');
    expect(storedCompletion('2026-10-04T18:00:36Z')).toBe('2026-10-04T18:00:36Z');
  });
  test('the welcome decision then stores the real time', async () => {
    const client = { local: {}, origin: '', environmentId: '', connection: 'disconnected', statusMessage: '', scopes: [], config: {}, shell: { projects: [], threads: [], sequence: 0 } } as unknown as T3Client;
    adoptPagesPrefs(client.local, { pages: { onboardingCompletedAt: '1970-01-01T00:01:58Z' } });
    expect(pagesPrefs(client).onboardingCompletedAt).toBe('');
    await welcomeView(client, new Transport([{ origin: 'http://127.0.0.1:14942', environmentId: 'env-x' }]), { step: 'connect', now: 118_000 });
    const stored = Date.parse(pagesPrefs(client).onboardingCompletedAt);
    expect(Math.abs(stored - Date.now())).toBeLessThan(60_000);
  });
});

// ── The branch picker's pull request checkout (PullRequestThreadDialog) ──────

type Call = { method: string; payload: Obj; write?: boolean };
function checkoutClient(threadId = '') {
  const calls: Call[] = [];
  const pull = (n: number) => ({ number: n, title: n === 101 ? 'Fix sidebar overflow on narrow windows' : 'Add retry to the sync client', url: `https://github.com/t3-fixture/pr-demo/pull/${n}`,
    baseBranch: 'main', headBranch: n === 101 ? 'feature/conflict' : 'feature/failing-checks', state: n === 104 ? 'merged' : 'open' });
  const respond = (method: string, payload: Obj) => {
    calls.push({ method, payload });
    if (method === 'git.resolvePullRequest') {
      const n = Number(String(payload.reference).replace(/.*\//, ''));
      if (n === 999) throw new ClientError('Pull request not found. Check the PR number or URL and try again.', 'SourceControlProviderError', false, { detail: 'Pull request not found. Check the PR number or URL and try again.' });
      if (n === 998) throw new ClientError('The connection failed.', 'Network');
      return { pullRequest: pull(n) };
    }
    if (method === 'git.preparePullRequestThread') {
      if (payload.mode === 'local') throw new Error('fake gh: pr checkout is not available in this fixture (writes are refused)');
      return { pullRequest: pull(101), branch: 'feature/conflict', worktreePath: '/home/worktrees/pr-demo/pr-101', isOnPullRequestHead: true };
    }
    throw new Error(`no reply for ${method}`);
  };
  const client = {
    environmentId: 'env', threadId, projectId: 'p1', ready: true, revision: 0,
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    local: { drafts: {} as Record<string, string>, composerControls: { contexts: {} as Record<string, Obj>, draftThreads: {} as Record<string, string> } },
    shell: { projects: [{ id: 'p1', title: 'pr-demo', workspaceRoot: '/repos/pr-demo' }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj, write = false) => { const reply = respond(method, payload); calls[calls.length - 1]!.write = write; return reply; },
    restAccess: () => ({ request: async (method: string, payload: Obj = {}) => respond(method, payload), ids: async (count: number) => Array.from({ length: count }, (_, i) => `draft-thread-${i + 1}`) }),
  } as unknown as T3Client;
  return { client, calls };
}
const nativeStub = {} as Native;
const github = { sourceControlProvider: { kind: 'github', name: 'GitHub', baseUrl: 'https://github.com' } };

describe('the branch picker offers "Checkout pull request" on a local draft', () => {
  test('a search that parses as a reference leads with the item; a thread or a plain ref name gets none', () => {
    const { client } = checkoutClient();
    expect(checkoutItems(client, '#101', github)).toEqual([{ reference: '101', title: 'Checkout pull request', mark: 'github' }]);
    expect(checkoutItems(client, ' gh pr checkout 102 ', github)).toEqual([{ reference: '102', title: 'Checkout pull request', mark: 'github' }]);
    expect(checkoutItems(client, 'https://github.com/t3-fixture/pr-demo/pull/103', null)[0]!.reference).toBe('https://github.com/t3-fixture/pr-demo/pull/103');
    expect(checkoutItems(client, 'feature/clean', github)).toEqual([]);
    expect(checkoutItems(client, '', github)).toEqual([]);
    expect(checkoutItems(client, '!7', { sourceControlProvider: { kind: 'gitlab', name: '' } })).toEqual([]);
    expect(checkoutItems(client, '7', { sourceControlProvider: { kind: 'gitlab', name: '' } })).toEqual([{ reference: '7', title: 'Checkout merge request', mark: 'gitlab' }]);
    expect(checkoutItems(checkoutClient('t1').client, '#101', github)).toEqual([]);
  });
});

describe('PullRequestThreadDialog', () => {
  test('opening resolves the reference and shows the pull request', async () => {
    const { client, calls } = checkoutClient();
    checkoutItems(client, '#101', github);
    await checkoutLocal(client, nativeStub, 'open', '101');
    expect(calls).toEqual([{ method: 'git.resolvePullRequest', payload: { cwd: '/repos/pr-demo', reference: '101' } }]);
    expect(checkoutView(client)).toMatchObject({ open: true, reference: '101', settled: '101', mark: 'github', title: 'Checkout pull request',
      description: 'Resolve a GitHub pull request, then create the draft thread in the main repo or in a dedicated worktree.', label: 'Pull Request',
      placeholder: 'PR URL, checkout command, or #42', prTitle: 'Fix sidebar overflow on narrow windows', prMeta: '#101 · feature/conflict to main', prState: 'Open',
      resolving: false, resolvingLabel: 'Resolving pull request...', error: '', preparing: '', canConfirm: true });
  });
  test('edits validate like the reference and failed lookups show the server message', async () => {
    const { client } = checkoutClient();
    await checkoutLocal(client, nativeStub, 'open', '101');
    await checkoutLocal(client, nativeStub, 'text', '');
    expect(checkoutView(client)).toMatchObject({ error: 'Paste a pull request URL, checkout command, or enter 123 / #123.', canConfirm: false, prTitle: '' });
    await checkoutLocal(client, nativeStub, 'text', 'feature/x');
    expect(checkoutView(client).error).toBe('Use a pull request URL, checkout command, 123, or #123.');
    await checkoutLocal(client, nativeStub, 'text', '#999');
    expect(checkoutView(client)).toMatchObject({ error: 'Source control provider github failed in getChangeRequest: Pull request not found. Check the PR number or URL and try again.', canConfirm: false, resolving: false });
    await checkoutLocal(client, nativeStub, 'text', '998');
    expect(checkoutView(client).error).toBe('The connection failed.');
    await checkoutLocal(client, nativeStub, 'text', '#104');
    expect(checkoutView(client)).toMatchObject({ prState: 'Merged', prMeta: '#104 · feature/failing-checks to main', error: '', canConfirm: true });
  });
  test('Worktree passes the draft’s thread id and moves the draft onto the checkout, then closes', async () => {
    const { client, calls } = checkoutClient();
    await checkoutLocal(client, nativeStub, 'open', '101');
    await checkoutConfirm(client, nativeStub, 'worktree');
    const prepare = calls.find(call => call.method === 'git.preparePullRequestThread')!;
    expect(prepare).toEqual({ method: 'git.preparePullRequestThread', payload: { cwd: '/repos/pr-demo', reference: '101', mode: 'worktree', threadId: 'draft-thread-1' }, write: true });
    expect(client.local.composerControls.draftThreads).toEqual({ 'env:new:p1': 'draft-thread-1' });
    expect(client.local.composerControls.contexts).toEqual({ 'env:new:p1': { envMode: 'worktree', branch: 'feature/conflict', worktreePath: '/home/worktrees/pr-demo/pr-101' } });
    expect(checkoutView(client).open).toBe(false);
  });
  test('Local passes no thread id; a failure stays in the dialog', async () => {
    const { client, calls } = checkoutClient();
    await checkoutLocal(client, nativeStub, 'open', '101');
    await checkoutConfirm(client, nativeStub, 'local');
    expect(calls.find(call => call.method === 'git.preparePullRequestThread')!.payload).toEqual({ cwd: '/repos/pr-demo', reference: '101', mode: 'local' });
    expect(checkoutView(client)).toMatchObject({ open: true, error: 'Failed to prepare pull request thread.', preparing: '', canConfirm: true });
    expect(client.local.composerControls.contexts).toEqual({});
    await checkoutLocal(client, nativeStub, 'close', '');
    expect(checkoutView(client).open).toBe(false);
  });
  test('an unresolved reference never prepares; Escape is ignored while preparing', async () => {
    const { client, calls } = checkoutClient();
    await checkoutLocal(client, nativeStub, 'open', '999');
    await checkoutConfirm(client, nativeStub, 'worktree');
    expect(calls.map(call => call.method)).toEqual(['git.resolvePullRequest']);
    checkoutState(client).preparing = 'worktree';
    await checkoutLocal(client, nativeStub, 'close', '');
    expect(checkoutView(client)).toMatchObject({ open: true, preparing: 'worktree', canConfirm: false });
    await expect(checkoutLocal(checkoutClient('t1').client, nativeStub, 'open', '101')).rejects.toThrow('Open a draft thread');
  });
});
