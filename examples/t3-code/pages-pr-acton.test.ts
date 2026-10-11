// "Act on" (pr-links-previews-and-routing): on the Pull Requests page, when two connected servers hold the pull
// request's repository, its hand-offs act on the chosen server (PullRequestDetailPanel.tsx pickableEnvironments,
// ActOnEnvironmentPicker, startAsk/startHandoff with the acting environment), driven through pages-pr-detail.ts with a
// background server in the fleet and injected replies.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { prCommand, pullRequestDetail } from './pages-pr-detail';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { toasts } from './toast';

const URL7 = 'https://github.com/lane/sandbox/pull/7';
const KEY_B = 'http://127.0.0.1:16361\nenv-b';
const identity = { provider: 'github', canonicalKey: 'github.com/lane/sandbox', displayName: 'lane/sandbox' };
const detail: Obj = { provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add a changelog', url: URL7, state: 'open', isDraft: false, mergeability: 'mergeable',
  baseComparison: 'up-to-date', changedFiles: 1, additions: 3, deletions: 0, headBranch: 'feature/changelog', baseBranch: 'main', author: { login: 'lane-primary' }, checks: [], labels: [],
  capabilities: { actions: [] }, viewerPermissions: { actions: [] }, workspaceRoot: '/a/sandbox', updatedAt: '2026-10-08T10:00:00Z', body: '' };

function setup() {
  const entry: FleetEntry = { key: KEY_B, origin: 'http://127.0.0.1:16361', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4, lastEvent: 0,
    subscriptions: {}, config: { environment: { label: 'Lane B', platform: { machine: 'laptop' }, capabilities: { pullRequests: true, threadPullRequests: true } } }, scopes: [], error: '', requested: true,
    shell: { ...initialShell(), projects: [{ id: 'b1', title: 'sandbox', workspaceRoot: '/b/sandbox', repositoryIdentity: identity }] } };
  fleet.entries.set(KEY_B, entry);
  const requests: Obj[] = [], adopted: Obj[] = [];
  const client = {
    environmentId: 'env-a', origin: 'http://127.0.0.1:16360', connection: 'connected', statusMessage: '', scopes: [], threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, error: '',
    get draftKey() { return `env-a:${this.threadId || `new:${this.projectId}`}`; },
    local: { drafts: {} as Record<string, string>, composerControls: { contexts: {} as Record<string, Obj> }, selections: {} as Record<string, Obj> },
    config: { environment: { label: 'Lane A', platform: { machine: 'desktop' }, capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/a/sandbox', repositoryIdentity: identity }], threads: [] as Obj[] },
    rpc: async (_native: unknown, method: string) => {
      if (method === 'pullRequests.detail') return detail;
      if (method === 'pullRequests.activity') return { reviewers: [], commits: [], comments: [], reviewThreads: [] };
      if (method === 'pullRequests.stack') return null;
      throw new Error(`no reply for ${method}`);
    },
    restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `draft-thread-${index + 1}`) }),
    adoptStatus(value: Obj, generation: number) { adopted.push({ ...value, generation }); },
    async openProjectDraft() { throw new Error('the focused server should not open a draft'); },
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => {
    requests.push(request);
    if (request.op === 'request' && request.fleet === KEY_B) return { ok: true, generation: 4, value: { branch: 'feature/changelog', worktreePath: '/b/worktrees/feature-changelog', isOnPullRequestHead: true } };
    if (request.op === 'connect') return { ok: true, generation: 2, value: { state: 'connecting' } };
    return { ok: true, generation: 1, value: {} };
  } } as unknown as Native;
  return { client, native, requests, adopted };
}
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
async function open(client: T3Client, native: Native, page = true) {
  let view = await pullRequestDetail(client, native, { selected, refresh: 1, now: 0, page });
  for (let asked = 0; asked < 6; asked++) view = await pullRequestDetail(client, native, { selected, refresh: 1, now: 0, page });
  return view;
}
afterEach(() => { fleet.entries.delete(KEY_B); });

describe('Act on (ActOnEnvironmentPicker)', () => {
  test('two servers holding the repository are offered, the panel\'s own first and acting; beside a thread nothing is', async () => {
    const { client, native } = setup();
    expect((await open(client, native)).actOn.items).toEqual([
      { key: 'env-a', label: 'Lane A', machine: 'desktop', selected: true },
      { key: 'env-b', label: 'Lane B', machine: 'laptop', selected: false },
    ]);
    expect((await open(client, native, false)).actOn.items).toEqual([]);
  });
  test('choosing the other server makes it act, for this pull request only', async () => {
    const { client, native } = setup();
    await open(client, native);
    await prCommand(client, native, 'act-on', selected, 'env-b');
    expect((await open(client, native)).actOn.items.map(item => item.selected)).toEqual([false, true]);
  });
  test('Check out with Act on the other server checks out there, points that server\'s draft at it and moves the window to it', async () => {
    const { client, native, requests, adopted } = setup();
    await open(client, native);
    await prCommand(client, native, 'act-on', selected, 'env-b');
    expect(await prCommand(client, native, 'handoff', selected, 'page|checkout:worktree')).toBe('sidebar:new-thread');
    expect(requests.find(request => request.op === 'request' && request.fleet === KEY_B)).toMatchObject({ method: 'git.preparePullRequestThread', generation: 4,
      payload: { cwd: '/b/sandbox', reference: URL7, mode: 'worktree', threadId: 'draft-thread-1' } });
    expect(client.local.composerControls.contexts!['env-b:new:b1']).toEqual({ envMode: 'worktree', branch: 'feature/changelog', worktreePath: '/b/worktrees/feature-changelog' });
    expect(client.local.selections['env-b']).toEqual({ projectId: 'b1', threadId: '' });
    expect(requests.find(request => request.op === 'connect')).toMatchObject({ origin: 'http://127.0.0.1:16361', credential: '' });
    expect(adopted).toHaveLength(1);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Checked out' });
  });
  test('Ask with Act on the other server writes the pull request into that server\'s draft and moves there', async () => {
    const { client, native, requests } = setup();
    await open(client, native);
    await prCommand(client, native, 'act-on', selected, 'env-b');
    expect(await prCommand(client, native, 'handoff', selected, 'page|ask')).toBe('sidebar:new-thread');
    expect(client.local.drafts['env-b:new:b1']).toMatch(/\[#7\]\(t3-context:/);
    expect(requests.some(request => request.method === 'git.preparePullRequestThread')).toBe(false);
  });
});
