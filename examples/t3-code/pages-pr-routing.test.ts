// Ported tests for pages-pr-routing.ts (task pr-links-previews-and-routing). Sources: T3 Code (MIT,
// see LICENSE-T3) at 1e2ecbd975 — apps/web/src/components/pullRequest/
// {pullRequestProjectAssignment.logic,pullRequestList.logic}.test.ts and
// packages/client-runtime/src/state/pullRequests.test.ts, original describe/it names kept.
//
// The router cases run against a fake PullRequestRouterHost (each call recorded, answers from a
// table) instead of the reference's Effect registry, supervisor and RPC clients. A reference
// `Effect.never` answer is STALL here: the fake fails it when the router set a deadline
// (`timeoutSeconds`, the transport's own), which is what the reference's Effect.timeout does.
//
// Not ported (Effect-runtime only, no router decision in them; the clone covers the behaviour elsewhere):
// - "keeps concurrent diff file reads on different hosts separate", "keeps live detail reads separate
//   from reads that allow stale data", "keeps hover previews fresh after edits and turns", "shares close,
//   reopen, and merge with an untouched client's mounted PR readers", "refreshes pull request activity
//   after a comment is updated", "refreshes checks without refreshing full detail", "updates cached labels
//   …", "updates reviewer requests …", "refreshes stack state after reopening …": these test the
//   reference's atom families (keyed query atoms, AtomRegistry mounts, the subscribeRefreshes stream).
//   The clone has no atoms: its detail/activity/preview reads re-run per answer (pages-pr-detail.ts,
//   pages-pr-refresh.ts), tested in pages-pr-conversation.test.ts and pages-pr-writes.test.ts.
// - "the project an id names" (findScopedProject) is ported below although the task list did not name it.
import { describe, test, expect } from 'bun:test';
import { ClientError } from './protocol';
import { type Obj } from './domain';
import {
  assignProjectsToEnvironments, resolvePickableEnvironments, mergePullRequestLists, pullRequestEntryKey, pullRequestEnvironmentSetKey, pullRequestEntryViewer,
  isAuthoredByViewer, resolveProjectScope, findScopedProject, resolveQueryEnvironmentIds, resolveSelectedEnvironmentId, routingPermissionFor, routingAllowed,
  rejectedBeforeDispatch, isPullRequestRef, matchesReference, PullRequestRouter, PR_ROUTED_READS, PR_ROUTED_WRITES, PR_PAGE_SIZE, PR_MAX_PAGE_SIZE,
  type AssignableProject, type PullRequestRouterHost, type RoutedEnvironment, type RoutingPermission,
} from './pages-pr-routing';
import { gitHubRoutingConnectionKey, type RouteEntry } from './connection-routes';

// ── pullRequestProjectAssignment.logic.test.ts ──────────────────────────────

const project = (id: string, environmentId: string, canonicalKey?: string): AssignableProject & { workspaceRoot: string } => ({
  id, environmentId, repositoryIdentity: canonicalKey === undefined ? null : { canonicalKey }, workspaceRoot: `/srv/${environmentId}/${id}`,
});
const envs = (...ids: string[]) => ids;
const plain = (assignment: Map<string, string[]>) => Object.fromEntries([...assignment].map(([id, projectIds]) => [id, projectIds]));

describe('one server per repository', () => {
  test('lets the first server list a repository both hold', () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('b1', 'env-1', 'github.com/acme/tools'),
      project('a2', 'env-2', 'github.com/acme/app'), project('c2', 'env-2', 'github.com/acme/site')], envs('env-1', 'env-2'), 'env-1');
    expect(plain(assignment)).toEqual({ 'env-1': ['a1', 'b1'], 'env-2': ['c2'] });
  });
  test('prefers the named server over the first one', () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'github.com/acme/app')], envs('env-1', 'env-2'), 'env-2');
    expect(plain(assignment)).toEqual({ 'env-2': ['a2'] });
  });
  test('drops a server with nothing of its own', () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'github.com/acme/app')], envs('env-1', 'env-2'), 'env-1');
    expect(assignment.has('env-2')).toBe(false);
  });
  test('keeps every copy of a project that has no identity to compare', () => {
    const assignment = assignProjectsToEnvironments([project('p1', 'env-1'), project('p2', 'env-2')], envs('env-1', 'env-2'), 'env-1');
    expect(plain(assignment)).toEqual({ 'env-1': ['p1'], 'env-2': ['p2'] });
  });
  test('keeps a repository listed by the one server that holds it', () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('b2', 'env-2', 'gitlab.com/acme/app')], envs('env-1', 'env-2'), 'env-1');
    expect(plain(assignment)).toEqual({ 'env-1': ['a1'], 'env-2': ['b2'] });
  });
  test("keeps a server's own worktrees of the repository it lists", () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('a1-wt', 'env-1', 'GitHub.com/acme/app'),
      project('a2', 'env-2', 'github.com/acme/app')], envs('env-1', 'env-2'), 'env-1');
    expect(plain(assignment)).toEqual({ 'env-1': ['a1', 'a1-wt'] });
  });
  test("reads two servers' copies of one repository as one however the remote is cased", () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'git.example.com/Team/App'), project('a2', 'env-2', 'GIT.example.com/team/app')], envs('env-1', 'env-2'), 'env-1');
    expect(plain(assignment)).toEqual({ 'env-1': ['a1'] });
  });
  test('ignores a project on a server that is not being read', () => {
    const assignment = assignProjectsToEnvironments([project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'github.com/acme/app')], envs('env-2'), 'env-2');
    expect(plain(assignment)).toEqual({ 'env-2': ['a2'] });
  });
  test('answers the same whatever order the projects arrive in', () => {
    const projects = [project('a2', 'env-2', 'github.com/acme/app'), project('a1', 'env-1', 'github.com/acme/app')];
    const forward = assignProjectsToEnvironments(projects, envs('env-1', 'env-2'), null);
    const backward = assignProjectsToEnvironments([...projects].reverse(), envs('env-1', 'env-2'), null);
    expect(plain(forward)).toEqual({ 'env-1': ['a1'] });
    expect(plain(backward)).toEqual(plain(forward));
  });
});

const connected = (...ids: string[]) => ids.map(id => ({ environmentId: id, label: `Server ${id}` }));
const on = (environmentId: string, projectId: string) => ({ environmentId, projectId });

describe('where a pull request can be acted on', () => {
  test("offers every server holding the repository, the panel's own first", () => {
    const pickable = resolvePickableEnvironments(on('env-2', 'a2'), [project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'github.com/acme/app'),
      project('c3', 'env-3', 'github.com/acme/site')], connected('env-1', 'env-2', 'env-3'));
    expect(pickable).toEqual([
      { environmentId: 'env-2', projectId: 'a2', workspaceRoot: '/srv/env-2/a2', label: 'Server env-2' },
      { environmentId: 'env-1', projectId: 'a1', workspaceRoot: '/srv/env-1/a1', label: 'Server env-1' },
    ]);
  });
  test('matches copies however the remote is cased', () => {
    const pickable = resolvePickableEnvironments(on('env-1', 'a1'), [project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'GitHub.com/ACME/app')], connected('env-1', 'env-2'));
    expect(pickable.map(entry => entry.environmentId)).toEqual(['env-1', 'env-2']);
  });
  test('offers nothing where one server holds the repository', () => {
    expect(resolvePickableEnvironments(on('env-1', 'a1'), [project('a1', 'env-1', 'github.com/acme/app'), project('c2', 'env-2', 'github.com/acme/site')], connected('env-1', 'env-2'))).toEqual([]);
  });
  test('offers nothing for a project with no identity to compare', () => {
    expect(resolvePickableEnvironments(on('env-1', 'p1'), [project('p1', 'env-1'), project('p2', 'env-2')], connected('env-1', 'env-2'))).toEqual([]);
  });
  test('offers nothing while the projects are still arriving', () => {
    expect(resolvePickableEnvironments(on('env-1', 'a1'), [], connected('env-1', 'env-2'))).toEqual([]);
  });
  test('leaves out a server that is not connected', () => {
    expect(resolvePickableEnvironments(on('env-1', 'a1'), [project('a1', 'env-1', 'github.com/acme/app'), project('a2', 'env-2', 'github.com/acme/app')], connected('env-1'))).toEqual([]);
  });
  test('names one copy per server, so two worktrees are one choice', () => {
    const pickable = resolvePickableEnvironments(on('env-1', 'a1'), [project('a1', 'env-1', 'github.com/acme/app'), project('a1-wt', 'env-1', 'github.com/acme/app'),
      project('a2', 'env-2', 'github.com/acme/app'), project('a2-wt', 'env-2', 'github.com/acme/app')], connected('env-1', 'env-2'));
    expect(pickable.map(entry => entry.projectId)).toEqual(['a1', 'a2']);
  });
  test("keeps the panel's own worktree rather than the server's first copy", () => {
    const pickable = resolvePickableEnvironments(on('env-1', 'a1-wt'), [project('a1', 'env-1', 'github.com/acme/app'), project('a1-wt', 'env-1', 'github.com/acme/app'),
      project('a2', 'env-2', 'github.com/acme/app')], connected('env-1', 'env-2'));
    expect(pickable[0]).toEqual({ environmentId: 'env-1', projectId: 'a1-wt', workspaceRoot: '/srv/env-1/a1-wt', label: 'Server env-1' });
  });
});

// ── pullRequestList.logic.test.ts ───────────────────────────────────────────

function entry(overrides: Obj & { number: number }): Obj {
  return { environmentId: 'env-1', provider: 'github', host: 'github.com', projectId: 'project-1', projectTitle: 't3code', repository: 'pingdotgg/t3code',
    title: 'Add the pull requests page', url: `https://github.com/pingdotgg/t3code/pull/${overrides.number}`, author: { login: 'octocat', name: null, avatarUrl: null },
    headBranch: `feat/branch-${overrides.number}`, baseBranch: 'main', state: 'open', isDraft: false, mergeability: 'mergeable', additions: 1, deletions: 0,
    createdAt: '2026-07-01T00:00:00Z', updatedAt: '2026-07-02T00:00:00Z', viewerReviewRequested: false, labels: [], ...overrides };
}
const ENV_1 = 'env-1', ENV_2 = 'env-2';

describe('resolveProjectScope', () => {
  const projects = [{ id: 'p1' }, { id: 'p2' }];
  test('keeps an id the environment has', () => { expect(resolveProjectScope('p2', projects, true)).toBe('p2'); });
  test('drops an id from another environment', () => {
    expect(resolveProjectScope('p9', projects, false)).toBe('p9');
    expect(resolveProjectScope('p9', projects, true)).toBeUndefined();
  });
  test('drops an id once the environment is known to have no projects', () => { expect(resolveProjectScope('p9', [], true)).toBeUndefined(); });
  test('keeps an id while the projects are still unknown', () => { expect(resolveProjectScope('p9', [], false)).toBe('p9'); });
});

describe("merging the environments' own listings", () => {
  const answer = (overrides: Obj = {}): Obj => ({ viewers: { 'github.com': 'Bilal' },
    providers: [{ host: 'github.com', kind: 'github', searchesOnHost: true, projectCount: 1, configured: true, detail: null }],
    entries: [], errors: [], truncated: false, nextCursors: {}, ...overrides });
  test('answers nothing until an environment has', () => { expect(mergePullRequestLists([])).toBeNull(); });
  test('tags every row with the environment that read it, newest first', () => {
    const merged = mergePullRequestLists([
      [ENV_1, answer({ entries: [entry({ number: 1, updatedAt: '2026-07-01T00:00:00Z' })] })],
      [ENV_2, answer({ entries: [entry({ number: 2, updatedAt: '2026-08-01T00:00:00Z' })] })],
    ]);
    expect(merged?.entries.map(row => [row.environmentId, row.number])).toEqual([[ENV_2, 2], [ENV_1, 1]]);
  });
  test("tells two environments' copies of one pull request apart", () => {
    const row = entry({ number: 4 });
    expect(pullRequestEntryKey({ ...row, environmentId: ENV_1 })).not.toBe(pullRequestEntryKey({ ...row, environmentId: ENV_2 }));
  });
  test('keeps project errors scoped to the environment that reported them', () => {
    const error = { projectId: 'project-1', projectTitle: 'Web', message: 'Not signed in' };
    const merged = mergePullRequestLists([[ENV_1, answer({ errors: [error] })], [ENV_2, answer()]]);
    expect(merged?.errors).toEqual([{ ...error, environmentId: ENV_1 }]);
  });
  test('folds a host reached from two environments into one switcher row', () => {
    const merged = mergePullRequestLists([[ENV_1, answer()],
      [ENV_2, answer({ providers: [{ host: 'github.com', kind: 'github', searchesOnHost: false, projectCount: 2, configured: false, detail: 'Not signed in.' }] })]]);
    expect(merged?.providers).toEqual([{ host: 'github.com', kind: 'github', searchesOnHost: false, projectCount: 3, configured: true, detail: null }]);
  });
  test("keeps each environment's continuation to itself", () => {
    const merged = mergePullRequestLists([[ENV_1, answer({ nextCursors: { 'github.com pingdotgg/t3code': 'cursor-1' } })], [ENV_2, answer()]]);
    expect(merged?.nextCursors).toEqual({ [ENV_1]: { 'github.com pingdotgg/t3code': 'cursor-1' } });
  });
  test('reads the same key whichever order the environments connected in', () => {
    expect(pullRequestEnvironmentSetKey(['env-2', 'env-1'])).toBe(pullRequestEnvironmentSetKey(['env-1', 'env-2']));
  });
});

describe('who "I" am, per server', () => {
  const answer = (viewers: Record<string, string>, entries: Obj[]): Obj => ({ viewers, providers: [], entries, errors: [], truncated: false, nextCursors: {} });
  const byBilal = { login: 'Bilal', name: null, avatarUrl: null };
  test("does not let one server's account decide who authored another server's rows", () => {
    const merged = mergePullRequestLists([
      [ENV_1, answer({ 'github.com': 'Bilal' }, [entry({ number: 1, author: byBilal })])],
      [ENV_2, answer({ 'github.com': 'Octocat' }, [entry({ number: 2, author: byBilal })])],
    ])!;
    // groupPullRequestsByInvolvement / filterPullRequestsByInvolvement "authored" both rest on this test (pages-prs.ts groups by it).
    expect(merged.entries.filter(row => isAuthoredByViewer(row, merged.viewers)).map(row => row.number)).toEqual([1]);
    expect(pullRequestEntryViewer(merged.entries.find(row => row.number === 2)!, merged.viewers)).toBe('octocat');
  });
  test("still reads a single server's host-keyed viewers, which is what a snapshot carries", () => {
    expect([entry({ number: 1, author: byBilal })].filter(row => isAuthoredByViewer(row, { 'github.com': 'Bilal' })).map(row => row.number)).toEqual([1]);
  });
  test('names the servers with more rows and no cursor to reach them by', () => {
    const merged = mergePullRequestLists([
      [ENV_1, { ...answer({}, []), truncated: true, nextCursors: { 'github.com acme/web': 'cursor-1' } }],
      [ENV_2, { ...answer({}, []), truncated: true }],
    ])!;
    expect(merged.truncatedEnvironments).toEqual([ENV_1, ENV_2]);
    expect(Object.keys(merged.nextCursors)).toEqual([ENV_1]);
  });
});

describe('the project an id names', () => {
  const projects = [{ id: 'project-1', environmentId: ENV_1 }, { id: 'project-1', environmentId: ENV_2 }, { id: 'project-2', environmentId: ENV_2 }];
  test('takes the server it was given', () => { expect(findScopedProject(projects, ENV_2, 'project-1')?.environmentId).toBe(ENV_2); });
  test('answers a bare id only where one server has it', () => {
    expect(findScopedProject(projects, null, 'project-2')?.environmentId).toBe(ENV_2);
    expect(findScopedProject(projects, null, 'project-1')).toBeUndefined();
  });
  test('answers nothing for a server that does not have it', () => { expect(findScopedProject(projects, ENV_1, 'project-2')).toBeUndefined(); });
});

describe('which environments a listing should ask', () => {
  const ENV_3 = 'env-3';
  const environmentIds = [ENV_1, ENV_2, ENV_3];
  test('asks only the owning server once the project is unambiguous', () => {
    expect(resolveQueryEnvironmentIds(environmentIds, [{ id: 'project-1', environmentId: ENV_2 }], { environmentId: ENV_2 }, 'project-1', true)).toEqual([ENV_2]);
  });
  test('asks every environment when there is no project narrowing at all', () => {
    expect(resolveQueryEnvironmentIds(environmentIds, [], undefined, undefined, true)).toEqual(environmentIds);
  });
  test("asks only the environments that actually hold an ambiguous bare id, never a third that doesn't", () => {
    const projects = [{ id: 'project-1', environmentId: ENV_1 }, { id: 'project-1', environmentId: ENV_2 }];
    expect(resolveQueryEnvironmentIds(environmentIds, projects, undefined, 'project-1', true)).toEqual([ENV_1, ENV_2]);
  });
  test('asks nothing for a bare id no environment holds', () => {
    expect(resolveQueryEnvironmentIds(environmentIds, [{ id: 'project-1', environmentId: ENV_1 }], undefined, 'project-9', true)).toEqual([]);
  });
  test('asks every environment while the servers have not said what they hold yet', () => {
    expect(resolveQueryEnvironmentIds(environmentIds, [], undefined, 'project-1', false)).toEqual(environmentIds);
  });
  test('keeps the single-environment case unchanged', () => {
    expect(resolveQueryEnvironmentIds([ENV_1], [{ id: 'project-1', environmentId: ENV_1 }], undefined, 'project-1', true)).toEqual([ENV_1]);
  });
});

describe('the server a saved selection names', () => {
  const known = new Set([ENV_1, ENV_2]);
  test('resolves to nothing yet for a server that is known but not ready, rather than falling back', () => { expect(resolveSelectedEnvironmentId(ENV_2, known, ENV_1)).toBe(ENV_2); });
  test('falls back for a server the workspace genuinely no longer has', () => { expect(resolveSelectedEnvironmentId('env-gone', known, ENV_1)).toBe(ENV_1); });
  test('falls back to nothing when no server names a fallback either', () => { expect(resolveSelectedEnvironmentId('env-gone', known, null)).toBeNull(); });
  test('uses the fallback outright when nothing was named', () => {
    expect(resolveSelectedEnvironmentId(undefined, known, ENV_1)).toBe(ENV_1);
    expect(resolveSelectedEnvironmentId(undefined, known, null)).toBeNull();
  });
});

describe('paging constants (_chat.pull-requests.tsx)', () => {
  test('one host page of ninety-nine rows, five hundred at most', () => { expect([PR_PAGE_SIZE, PR_MAX_PAGE_SIZE]).toEqual([99, 500]); });
});

// ── pullRequests.test.ts (routing) ──────────────────────────────────────────

const STALL = Symbol('stall');
type Answer = Obj | null | typeof STALL;
type Handler = (payload: Obj) => Answer | Promise<Answer>;
type Call = { environment: string; method: string; payload: Obj; timeoutSeconds?: number };
const ORIGIN = 'environment-1', LOCAL = 'local-environment';

/** makeTestRuntime: an origin (remote unless `localOrigin`) and, unless `single`, the local alternate. */
class FakeHost implements PullRequestRouterHost {
  readonly originId = ORIGIN;
  calls: Call[] = [];
  entries: RoutedEnvironment[];
  constructor(readonly clients: Record<string, Record<string, Handler>>, options: { localOrigin?: boolean; single?: boolean; permission?: (id: string) => RoutingPermission } = {}) {
    const permission = options.permission ?? (() => 'read-write' as RoutingPermission);
    this.entries = [{ id: ORIGIN, enabled: true, connected: true, local: options.localOrigin === true, permission: permission(ORIGIN), pullRequestChecks: true },
      ...(options.single ? [] : [{ id: LOCAL, enabled: true, connected: true, local: true, permission: permission(LOCAL), pullRequestChecks: true }])];
  }
  environments(): ReadonlyArray<RoutedEnvironment> { return this.entries; }
  update(id: string, change: Partial<RoutedEnvironment>): void { this.entries = this.entries.map(entry => entry.id === id ? { ...entry, ...change } : entry); }
  async request(environment: string, method: string, payload: Obj, options: { write: boolean; timeoutSeconds?: number }): Promise<Obj> {
    this.calls.push({ environment, method, payload, ...(options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds }) });
    const handler = this.clients[environment]?.[method] ?? (method === 'pullRequests.invalidate' ? () => ({}) : undefined);
    if (!handler) throw new ClientError(`Unknown request tag: ${method}`, 'RpcClientError');
    const answer = await handler(payload);
    if (answer === STALL) {
      if (options.timeoutSeconds !== undefined) throw new ClientError('The environment did not respond to the PR request.', 'timeout');
      return new Promise<Obj>(() => {});
    }
    return answer as Obj;
  }
  names(suffix?: string): string[] { return this.calls.map(call => `${call.environment === ORIGIN ? 'origin' : 'local'}:${call.method.slice('pullRequests.'.length)}`).filter(name => suffix === undefined || name.endsWith(suffix)); }
}
const operationError = (operation: string, detail: string) => new ClientError(`Pull request operation ${operation} failed: ${detail}`, 'PullRequestOperationError');

const SCENARIOS = [
  'prefers the local environment with the same github account',
  'falls back before mutation when the local account differs',
  'never retries an ambiguous mutation failure',
  'returns a fast local source read without checking alternate identities',
  'keeps single-environment requests free of identity lookups',
  'keeps a local origin ahead of another local environment',
  'keeps mutations on an old origin server without retrying them',
  'skips an old alternate server before dispatching a mutation',
  'returns a successful mutation when source invalidation stalls',
  'does not dispatch after routing permission is revoked during the probe',
  'keeps mutations on the origin when the alternate is disabled',
  'does not dispatch after the alternate is disabled during the probe',
  'preserves a local source account rejection when alternate accounts differ',
] as const;

for (const scenario of SCENARIOS) {
  test(scenario, async () => {
    const inputs: Obj[] = [];
    let trusted = true;
    const switchedAccount = scenario === 'preserves a local source account rejection when alternate accounts differ';
    const mismatch = scenario === 'falls back before mutation when the local account differs' || switchedAccount;
    const ambiguous = scenario === 'never retries an ambiguous mutation failure';
    const reading = scenario === 'returns a fast local source read without checking alternate identities';
    const single = scenario === 'keeps single-environment requests free of identity lookups';
    const localOrigin = scenario === 'keeps a local origin ahead of another local environment' || switchedAccount || reading;
    const oldOrigin = scenario === 'keeps mutations on an old origin server without retrying them';
    const oldAlternate = scenario === 'skips an old alternate server before dispatching a mutation';
    const failure = operationError(switchedAccount ? 'routeIdentity' : 'runAction', 'connection lost after dispatch');
    let host: FakeHost;
    const clientFor = (local: boolean): Record<string, Handler> => ({
      [local ? 'pullRequests.routingIdentity' : 'pullRequests.routing']: input => {
        if (local) expect(input).toEqual({ host: 'github.com' });
        if (local && scenario === 'does not dispatch after the alternate is disabled during the probe') host.update(LOCAL, { enabled: false });
        if (local && scenario === 'does not dispatch after routing permission is revoked during the probe') { trusted = false; host.entries = host.entries.map(entry => ({ ...entry, permission: 'off' })); }
        if ((!local && oldOrigin) || (local && oldAlternate)) throw new ClientError('Unknown request tag: pullRequests.routing', 'RpcClientError');
        return { host: 'github.com', provider: 'github', viewer: 'maria-rcks', accountId: local && mismatch ? '456' : '123' };
      },
      'pullRequests.runAction': input => {
        inputs.push(input);
        if (!local && switchedAccount && input.expectedAccountId === '123') throw failure;
        if (local && ambiguous) throw failure;
        return {};
      },
      'pullRequests.summary': input => {
        expect(input.allowStale).toBe(false);
        if (local) throw failure;
        return null;
      },
      'pullRequests.invalidate': () => (!local && scenario === 'returns a successful mutation when source invalidation stalls' ? STALL : {}),
    });
    host = new FakeHost({ [ORIGIN]: clientFor(false), [LOCAL]: clientFor(true) }, { localOrigin, single });
    const disabled = scenario === 'keeps mutations on the origin when the alternate is disabled';
    const disabledDuringProbe = scenario === 'does not dispatch after the alternate is disabled during the probe';
    if (disabled) host.update(LOCAL, { enabled: false });
    const input = { projectId: 'project-1', host: 'github.com', repository: 'acme/web', number: 7, action: 'merge' };
    const route = new PullRequestRouter();
    let result: { ok: true } | { ok: false; error: unknown };
    try { await route.request(host, reading ? 'pullRequests.summary' : 'pullRequests.runAction', input, !reading); result = { ok: true }; }
    catch (error) { result = { ok: false, error }; }
    if (switchedAccount) {
      expect(result).toEqual({ ok: false, error: failure });
      expect(host.names(':runAction')).toEqual(['origin:runAction']);
      expect(inputs).toEqual([{ ...input, expectedAccountId: '123' }]);
    } else if (ambiguous) {
      expect(result).toEqual({ ok: false, error: failure });
      expect(host.names(':runAction')).toEqual(['local:runAction']);
    } else {
      expect(result.ok).toBe(true);
      if (single || disabled) expect(host.names().filter(name => !name.endsWith(':invalidate'))).toEqual(['origin:runAction']);
      else if (reading) expect(host.names()).toEqual(['origin:summary']);
      else if (mismatch || localOrigin || oldOrigin || oldAlternate || !trusted || disabledDuringProbe) expect(host.names(':runAction')).toEqual(['origin:runAction']);
      else {
        expect(host.names(':runAction')).toEqual(['local:runAction']);
        expect(host.names()).toContain('origin:invalidate');
        expect(inputs).toEqual([{ ...input, expectedAccountId: '123' }]);
      }
    }
  });
}

for (const provider of ['github', 'gitlab', 'bitbucket', 'azure-devops'] as const) {
  test(`routes ${provider} viewed marks to their storage environment`, async () => {
    const reference = { projectId: 'project-1', host: 'github.com', repository: 'acme/web', number: 7 };
    const calls: { environment: string; operation: string; input: Obj }[] = [];
    const clientFor = (environment: string): Record<string, Handler> => ({
      'pullRequests.routing': () => ({ host: reference.host, provider, accountId: '123', viewer: 'maria-rcks' }),
      'pullRequests.routingIdentity': () => ({ host: reference.host, provider, accountId: '123', viewer: 'maria-rcks' }),
      'pullRequests.filesViewed': input => { calls.push({ environment, operation: 'read', input }); return { files: [{ path: 'a.ts', state: 'viewed' }], truncated: false }; },
      'pullRequests.setFilesViewed': input => { calls.push({ environment, operation: 'write', input }); return {}; },
      'pullRequests.invalidate': input => { calls.push({ environment, operation: 'invalidate', input }); return {}; },
    });
    const host = new FakeHost({ [ORIGIN]: clientFor('origin'), [LOCAL]: clientFor('local') });
    const files = [{ path: 'a.ts', viewed: false }];
    const route = new PullRequestRouter();
    expect(await route.request(host, 'pullRequests.filesViewed', reference, false)).toEqual({ files: [{ path: 'a.ts', state: 'viewed' }], truncated: false });
    await route.request(host, 'pullRequests.setFilesViewed', { ...reference, files }, true);
    const environment = provider === 'github' ? 'local' : 'origin';
    const guard = provider === 'github' ? { expectedAccountId: '123' } : {};
    expect(calls.filter(call => call.operation !== 'invalidate')).toEqual([
      { environment, operation: 'read', input: { ...reference, allowStale: false, ...guard } },
      { environment, operation: 'write', input: { ...reference, files, ...guard } },
    ]);
    const invalidations = calls.filter(call => call.operation === 'invalidate');
    if (provider === 'github') expect(invalidations.length).toBeGreaterThan(0);
    else expect(invalidations).toEqual([]);
    for (const call of invalidations) expect(call.input).toEqual({ reference: expect.objectContaining(reference), filesViewedOnly: true });
  });
}

for (const permission of ['default', 'origin-off', 'destination-off', 'read-only'] as const) {
  test(`does not probe another environment with ${permission} routing permission`, async () => {
    const calls: string[] = [];
    const failure = operationError('summary', 'source failed');
    const client: Record<string, Handler> = {
      'pullRequests.summary': () => { calls.push('source-read'); throw failure; },
      'pullRequests.runAction': () => { calls.push('source-write'); return {}; },
    };
    const alternate: Record<string, Handler> = { 'pullRequests.routingIdentity': () => { throw new Error('untrusted environment was contacted'); } };
    // "default" is GitHubRoutingPermissions' default: every environment "off".
    const host = new FakeHost({ [ORIGIN]: client, [LOCAL]: alternate }, {
      permission: id => permission === 'default' ? 'off' : permission === 'read-only' ? 'read' : (id === ORIGIN) === (permission === 'origin-off') ? 'off' : 'read-write' });
    const ref = { projectId: 'project-1', repository: 'private/repo', number: 7 };
    const route = new PullRequestRouter();
    if (permission !== 'read-only') await expect(route.request(host, 'pullRequests.summary', ref, false)).rejects.toBe(failure);
    await route.request(host, 'pullRequests.runAction', { ...ref, action: 'merge' }, true);
    expect(calls).toEqual(permission === 'read-only' ? ['source-write'] : ['source-read', 'source-write']);
  });
}

// The reference re-reads the SSH profile from its store at routing time and compares connection keys.
// The clone's host computes the key from the saved catalog as it is now (connection-routes.ts
// gitHubRoutingConnectionKey) and looks the stored permission up by it (routingPermissionFor): a changed
// or missing profile, or one that cannot be read, gives a key nothing stored names, so "off".
for (const side of ['origin', 'destination'] as const) {
  for (const stored of ['matching', 'changed', 'missing', 'unavailable', 'failed'] as const) {
    test(`checks the current ${side} SSH profile before routing with ${stored} storage`, async () => {
      const calls: string[] = [];
      const identity = { host: 'github.com', provider: 'github', viewer: 'maria', accountId: '123' };
      const client: Record<string, Handler> = {
        'pullRequests.routing': () => { calls.push('source-probe'); return identity; },
        'pullRequests.runAction': input => { if (input.expectedAccountId !== undefined) throw operationError('routeIdentity', 'Source guard refused.'); calls.push('source-write'); return {}; },
      };
      const alternate: Record<string, Handler> = {
        'pullRequests.routingIdentity': () => { calls.push('alternate-probe'); return identity; },
        'pullRequests.runAction': () => { calls.push('alternate-write'); return {}; },
      };
      const environmentId = side === 'origin' ? ORIGIN : LOCAL;
      const ssh = { alias: 'work', hostname: 'work.example.test', username: 'maria', port: 22 };
      // The tunnel and a direct address: with two routes the key covers the SSH target itself.
      const entryWith = (target: typeof ssh | undefined): RouteEntry => ({ environmentId, label: 'SSH', routes: [
        { id: 'ssh:work', origin: 'http://127.0.0.1:41000', kind: 'ssh', ...(target ? { ssh: target } : {}) }, { id: 'http://work.example.test:3773', origin: 'http://work.example.test:3773' }] });
      const storedPermissions = { [gitHubRoutingConnectionKey(entryWith(ssh))!]: 'read-write' };
      const current = stored === 'matching' ? entryWith(ssh) : stored === 'changed' ? entryWith({ ...ssh, hostname: 'replacement.example.test' }) : stored === 'missing' ? entryWith(undefined) : null;
      const sshPermission = routingPermissionFor(current === null ? null : gitHubRoutingConnectionKey(current), storedPermissions);
      const host = new FakeHost({ [ORIGIN]: client, [LOCAL]: alternate }, { permission: id => id === environmentId ? sshPermission : 'read-write' });
      await new PullRequestRouter().request(host, 'pullRequests.runAction', { projectId: 'project-1', repository: 'private/repo', number: 7, action: 'merge' }, true);
      expect(calls).toEqual(stored === 'matching' ? ['source-probe', 'alternate-probe', 'alternate-write'] : ['source-write']);
    });
  }
}

for (const probe of ['origin', 'alternate'] as const) {
  test(`bounds a stalled ${probe} metadata probe without repeating a strict source read`, async () => {
    let sourceReads = 0;
    const identity = { host: 'github.com', provider: 'github', viewer: 'maria', accountId: '123' };
    const failure = operationError('summary', 'source failed');
    const host = new FakeHost({
      [ORIGIN]: { 'pullRequests.routing': () => (probe === 'origin' ? STALL : identity), 'pullRequests.summary': () => { sourceReads += 1; throw failure; } },
      [LOCAL]: { 'pullRequests.routingIdentity': () => STALL },
    });
    await expect(new PullRequestRouter().request(host, 'pullRequests.summary', { projectId: 'project-1', repository: 'acme/web', number: 7, allowStale: false }, false)).rejects.toBe(failure);
    expect(sourceReads).toBe(1);
    expect(host.calls.filter(call => call.method.startsWith('pullRequests.routing')).every(call => call.timeoutSeconds === 2)).toBe(true);
  });
}

for (const source of ['pending', 'pending-local', 'failed-local', 'failed', 'offline'] as const) {
  test(source === 'offline' ? 'returns held source data only after both fresh paths fail' : `uses one shared reader with a ${source} source`, async () => {
    const calls: string[] = [];
    const clientFor = (local: boolean): Record<string, Handler> => ({
      [local ? 'pullRequests.routingIdentity' : 'pullRequests.routing']: () => ({ host: 'github.com', provider: 'github', viewer: 'maria-rcks', accountId: '123' }),
      'pullRequests.summary': input => {
        calls.push(local ? 'local' : input.allowStale === false ? 'origin' : 'held');
        if (source === 'offline' && !local && input.allowStale === undefined) return { state: 'open' };
        expect(input.allowStale).toBe(false);
        if (local && source !== 'offline') return null;
        if (source !== 'pending' && source !== 'pending-local') throw operationError('summary', 'github unreachable');
        return STALL;
      },
    });
    const host = new FakeHost({ [ORIGIN]: clientFor(false), [LOCAL]: clientFor(true) }, { localOrigin: source === 'failed-local' || source === 'pending-local' });
    const result = await new PullRequestRouter().request(host, 'pullRequests.summary', { projectId: 'project-1', repository: 'acme/web', number: 7 }, false);
    if (source === 'offline') {
      expect(result).toEqual({ state: 'open' });
      expect(calls).toEqual(['local', 'origin', 'held']);
    } else {
      expect(result).toBeNull();
      expect(calls).toEqual(source === 'failed-local' || source === 'pending-local' ? ['origin', 'local'] : ['local']);
    }
  });
}

test('keeps source workspace metadata when an alternate answers a detail read', async () => {
  const clientFor = (local: boolean): Record<string, Handler> => ({
    [local ? 'pullRequests.routingIdentity' : 'pullRequests.routing']: () => ({ host: 'github.com', provider: 'github', viewer: 'maria-rcks', accountId: '123',
      projectTitle: local ? 'local project' : 'source project', workspaceRoot: local ? '/Users/local/repo' : '/srv/source/repo' }),
    'pullRequests.detail': () => (local ? { projectId: 'local-project', projectTitle: 'local project', workspaceRoot: '/Users/local/repo', title: 'github title' } : STALL),
  });
  const host = new FakeHost({ [ORIGIN]: clientFor(false), [LOCAL]: clientFor(true) });
  expect(await new PullRequestRouter().request(host, 'pullRequests.detail', { projectId: 'project-1', repository: 'acme/web', number: 7 }, false))
    .toEqual({ projectId: 'project-1', projectTitle: 'source project', workspaceRoot: '/srv/source/repo', title: 'github title' });
});

test('refreshes identity before writes and invalidates prior readers across router instances', async () => {
  let originAccountId = '123';
  const mutations: { environment: string; expectedAccountId: unknown }[] = [];
  const invalidations: { environment: string; input: Obj }[] = [];
  const identities: string[] = [];
  const clientFor = (local: boolean): Record<string, Handler> => {
    const environment = local ? 'local' : 'origin';
    return {
      [local ? 'pullRequests.routingIdentity' : 'pullRequests.routing']: () => { identities.push(environment); return { host: 'github.com', provider: 'github', viewer: 'maria-rcks', accountId: local ? '123' : originAccountId }; },
      'pullRequests.summary': () => (local ? null : STALL),
      'pullRequests.runAction': input => { mutations.push({ environment, expectedAccountId: input.expectedAccountId }); return {}; },
      'pullRequests.invalidate': input => { invalidations.push({ environment, input }); return {}; },
    };
  };
  const host = new FakeHost({ [ORIGIN]: clientFor(false), [LOCAL]: clientFor(true) });
  const reference = { projectId: 'project-1', repository: 'acme/web', number: 7 };
  const hostedReference = { ...reference, host: 'github.com', allowStale: false };
  // The reference keys the routed-reads memory by registry, so two routers over one registry share it.
  const memory = new Map();
  const route = new PullRequestRouter(memory);
  await route.request(host, 'pullRequests.summary', reference, false);
  originAccountId = '456';
  await route.request(host, 'pullRequests.runAction', { ...reference, action: 'merge' }, true);
  expect(identities.filter(environment => environment === 'origin')).toHaveLength(2);
  expect(mutations).toEqual([{ environment: 'origin', expectedAccountId: '456' }]);
  expect(invalidations).toEqual(expect.arrayContaining([
    { environment: 'origin', input: { reference: expect.objectContaining(reference) } },
    { environment: 'origin', input: { reference: hostedReference } },
    { environment: 'local', input: { reference: hostedReference } },
    { environment: 'origin', input: {} },
    { environment: 'local', input: {} },
  ]));
  invalidations.length = 0;
  const refresh = new PullRequestRouter(memory);
  await refresh.request(host, 'pullRequests.invalidate', { reference }, false);
  expect(invalidations).toContainEqual({ environment: 'local', input: { reference: hostedReference } });
  invalidations.length = 0;
  await refresh.request(host, 'pullRequests.invalidate', {}, false);
  expect(invalidations).toContainEqual({ environment: 'local', input: {} });
});

for (const oldAlternate of [false, true]) {
  test(`routes checks through one reader with old alternate: ${oldAlternate}`, async () => {
    const calls: string[] = [];
    const clientFor = (local: boolean): Record<string, Handler> => ({
      [local ? 'pullRequests.routingIdentity' : 'pullRequests.routing']: () => ({ host: 'github.com', provider: 'github', viewer: 'viewer', accountId: '123' }),
      'pullRequests.checks': () => { calls.push(local ? 'local' : 'origin'); if (local && oldAlternate) throw new ClientError('Unknown request tag: pullRequests.checks', 'RpcClientError'); return { state: 'open', checks: [] }; },
    });
    const host = new FakeHost({ [ORIGIN]: clientFor(false), [LOCAL]: clientFor(true) });
    // The alternate's server.getConfig capability, which the host folds into the entry.
    host.update(LOCAL, { pullRequestChecks: !oldAlternate });
    expect(await new PullRequestRouter().request(host, 'pullRequests.checks', { projectId: 'project-1', repository: 'acme/web', number: 1 }, false)).toEqual({ state: 'open', checks: [] });
    expect(calls).toEqual(oldAlternate ? ['origin'] : ['local']);
  });
}

// ── Clone-side decisions the reference states in prose (task record "Reference rules to keep") ──

describe('routing rules the task record lists', () => {
  const permissions: RoutingPermission[] = ['off', 'read', 'read-write'];
  test('writes route only when both servers are "read-write", reads when neither is "off"', () => {
    const at = (permission: RoutingPermission, id: string): RoutedEnvironment => ({ id, enabled: true, connected: true, local: false, permission });
    for (const origin of permissions) for (const destination of permissions) {
      expect(routingAllowed(at(origin, 'a'), at(destination, 'b'), true)).toBe(origin === 'read-write' && destination === 'read-write');
      expect(routingAllowed(at(origin, 'a'), at(destination, 'b'), false)).toBe(origin !== 'off' && destination !== 'off');
    }
    expect(routingAllowed({ ...at('read-write', 'a'), enabled: false }, at('read-write', 'b'), false)).toBe(false);
    expect(routingAllowed(at('read-write', 'a'), undefined, false)).toBe(false);
  });
  test('a write with one side "read" stays on the origin', async () => {
    const host = new FakeHost({
      [ORIGIN]: { 'pullRequests.routing': () => ({ host: 'github.com', provider: 'github', accountId: '1', viewer: 'v' }), 'pullRequests.comment': () => ({}) },
      [LOCAL]: { 'pullRequests.routingIdentity': () => ({ host: 'github.com', provider: 'github', accountId: '1', viewer: 'v' }), 'pullRequests.comment': () => ({}) },
    }, { permission: id => (id === LOCAL ? 'read' : 'read-write') });
    await new PullRequestRouter().request(host, 'pullRequests.comment', { projectId: 'p', repository: 'o/r', number: 1, body: 'hi' }, true);
    expect(host.names()).toEqual(['origin:comment']);
  });
  test('an identity mismatch is refused before dispatch: the alternate is skipped', async () => {
    const host = new FakeHost({
      [ORIGIN]: { 'pullRequests.routing': () => ({ host: 'github.com', provider: 'github', accountId: '1', viewer: 'v' }), 'pullRequests.comment': () => ({}) },
      [LOCAL]: { 'pullRequests.routingIdentity': () => ({ host: 'github.com', provider: 'github', accountId: '2', viewer: 'w' }), 'pullRequests.comment': () => ({}) },
    });
    await new PullRequestRouter().request(host, 'pullRequests.comment', { projectId: 'p', repository: 'o/r', number: 1, body: 'hi' }, true);
    expect(host.names().filter(name => !name.endsWith('invalidate'))).toEqual(['origin:routing', 'local:routingIdentity', 'origin:comment']);
    expect(host.calls.find(call => call.method === 'pullRequests.comment')?.payload).toEqual({ projectId: 'p', repository: 'o/r', number: 1, body: 'hi', host: 'github.com', expectedAccountId: '1' });
  });
  test('local servers come first for reads; a remote alternate after them; the origin last', async () => {
    const order: string[] = [];
    const clients: Record<string, Record<string, Handler>> = {};
    for (const id of [ORIGIN, LOCAL, 'remote-2']) {
      clients[id] = { [id === ORIGIN ? 'pullRequests.routing' : 'pullRequests.routingIdentity']: () => ({ host: 'github.com', provider: 'github', accountId: '1', viewer: 'v' }),
        'pullRequests.activity': () => { order.push(id); throw operationError('activity', 'down'); } };
    }
    const host = new FakeHost(clients);
    host.entries = [...host.entries, { id: 'remote-2', enabled: true, connected: true, local: false, permission: 'read-write' }];
    await expect(new PullRequestRouter().request(host, 'pullRequests.activity', { projectId: 'p', repository: 'o/r', number: 1 }, false)).rejects.toBeInstanceOf(ClientError);
    expect(order).toEqual([LOCAL, 'remote-2', ORIGIN]);
  });
  test('a disconnected alternate is not a candidate', async () => {
    const host = new FakeHost({ [ORIGIN]: { 'pullRequests.comment': () => ({}) }, [LOCAL]: {} });
    host.update(LOCAL, { connected: false });
    await new PullRequestRouter().request(host, 'pullRequests.comment', { projectId: 'p', repository: 'o/r', number: 1, body: 'x' }, true);
    expect(host.names()).toEqual(['origin:comment']);
  });
  test('routed method sets, references and guard rejections', () => {
    expect([...PR_ROUTED_READS].every(method => !PR_ROUTED_WRITES.has(method))).toBe(true);
    expect(PR_ROUTED_READS.has('pullRequests.list')).toBe(false);
    expect(isPullRequestRef({ projectId: 'p', repository: 'o/r', number: 3 })).toBe(true);
    expect(isPullRequestRef({ projectId: 'p', repository: 'o/r', number: 0 })).toBe(false);
    expect(isPullRequestRef({ projectId: 'p', repository: ' o/r', number: 3 })).toBe(false);
    expect(matchesReference({ projectId: 'p', host: 'GitHub.com', repository: 'O/R', number: 3 }, { projectId: 'p', repository: 'o/r', number: 3 })).toBe(true);
    expect(matchesReference({ projectId: 'p', host: 'github.com', repository: 'o/r', number: 3 }, { projectId: 'p', host: 'ghe.example', repository: 'o/r', number: 3 })).toBe(false);
    expect(rejectedBeforeDispatch(new ClientError('The server is not connected.', 'Disconnected'))).toBe(true);
    expect(rejectedBeforeDispatch(operationError('routeIdentity', 'refused'))).toBe(true);
    expect(rejectedBeforeDispatch(operationError('runAction', 'lost'))).toBe(false);
    expect(rejectedBeforeDispatch({ _tag: 'PullRequestOperationError', operation: 'routeIdentity' })).toBe(true);
    expect(routingPermissionFor(null, {})).toBe('off');
    expect(routingPermissionFor('k', { k: 'read' })).toBe('read');
    expect(routingPermissionFor('k', { k: 'bogus' })).toBe('off');
  });
});
