import { afterEach, expect, test } from 'bun:test';
import { statusSince, topStatus } from '../../src/sidebar/sidebar-model';
import { parentThreadLink } from '../../src/sidebar/sidebar-lineage';
import { pullRequestSearchTerms, searchPullRequest } from '../../src/sidebar/search-pull-request';
import { isDetailDone, runningThreadKeepAlive } from '../../src/client/keep-alive';
import { ActivityScopes } from '../../src/transport/transport-activity';
import { pagesHome } from '../../src/pages/pages-home';
import { revealThreadTerminal } from '../../src/terminal/terminal-panel';
import { scopedThreadKey, selectThreadTerminalUiState, terminalUiStore } from '../../src/terminal/terminal-ui-state';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { opened } from '../support/composer-controls-fixture';
import { scratchMachine } from '../../src/sidebar/sidebar-view';
import { fleet } from '../../src/connections/settings-b-fleet';

afterEach(() => fleet.entries.clear());

test('Waiting uses the run completion and a dot-dashed icon; invalid or missing dates show no timer', () => {
  const completedAt = '2026-10-18T01:00:00.000Z';
  expect(statusSince({ latestRunId: 'r', status: 'completed', latestRunCompletedAt: completedAt }, 'waiting')).toBe(completedAt);
  expect(topStatus('waiting', false, false)).toMatchObject({ label: 'Waiting', icon: 'circle-dot-dashed' });
  expect(statusSince({ latestRunId: 'r', status: 'completed', latestRunCompletedAt: 'bad' }, 'waiting')).toBeNull();
  expect(statusSince({}, 'waiting')).toBeNull();
});

test('a queued run retains its detail stream through a shell transition to idle', () => {
  const kept = runningThreadKeepAlive({ environmentIds: ['e'], threads: () => [{ id: 't', status: 'queued' }], detail: () => undefined });
  expect([...kept.get('e')!]).toEqual(['t']);
  const detail = { status: 'live' as const, thread: { projection: { runs: [{ status: 'queued' }] }, sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null }, error: '' };
  expect(isDetailDone(detail)).toBe(false);
  expect([...runningThreadKeepAlive({ environmentIds: ['e'], threads: () => [{ id: 't', status: 'idle' }], detail: () => detail, previous: kept }).get('e')!]).toEqual(['t']);
});

test('parent breadcrumbs resolve the shell title and disappear on a normal thread', async () => {
  const { client } = await opened();
  client.shell.threads[0]!.lineage = { relationshipToParent: 'subagent', parentThreadId: 't2' };
  expect(parentThreadLink(client)).toEqual([{ id: 't2', label: 'Thread t2' }]);
  client.threadId = 't2';
  expect(parentThreadLink(client)).toEqual([]);
});

test('a branch PR number finds the thread even without a current link', async () => {
  const { client } = await opened();
  const thread = { branchPullRequest: { number: 42, repository: 'org/repo', url: 'https://github.com/org/repo/pull/42' }, pullRequests: [] };
  expect(pullRequestSearchTerms(thread)).toContain('org/repo#42');
  // A stored reference's number shows before the pull request's details load.
  expect(searchPullRequest(client, thread, '#42')).toEqual({ text: '42', icon: 'git-pull-request-arrow', state: '', label: 'PR #42, status pending' });
  expect(searchPullRequest(client, thread, '#99')).toBeNull();
});

test('a search result shows the sidebar row\'s pull request badge once its terms match', async () => {
  const { client } = await opened();
  const link = (number: number, state: string, extra: Record<string, unknown> = {}) => ({ number, repository: 'org/repo', host: 'github.com', url: `https://github.com/org/repo/pull/${number}`,
    linkedAt: `2026-10-0${number % 9 + 1}T00:00:00.000Z`, snapshot: { title: `Change ${number}`, state, isDraft: false, headBranch: `b${number}`, baseBranch: 'main' }, ...extra });
  const one = { id: 't-one', projectId: 'p1', pullRequests: [link(7, 'merged')] };
  // The query matches the link's title; the badge is the link's number in its state, as on the row.
  expect(searchPullRequest(client, one, 'change 7')).toEqual({ text: '7', icon: 'git-merge', state: 'merged', label: 'PR #7 - Merged: Change 7' });
  // Several unrelated links fold into "+N" whichever one matched, labelled by the current one.
  const many = { id: 't-many', projectId: 'p1', pullRequests: [link(7, 'merged'), link(8, 'open')] };
  expect(searchPullRequest(client, many, '#7')).toEqual({ text: '+2', icon: 'git-pull-request-arrow', state: 'open', label: 'PR #8 - Open: Change 8, and 1 more linked; overall open' });
  // One chain is a stack of N.
  const stack = { id: 't-stack', projectId: 'p1', pullRequests: [link(7, 'open'), link(8, 'open', { snapshot: { title: 'Change 8', state: 'open', isDraft: false, headBranch: 'b8', baseBranch: 'b7' } })] };
  expect(searchPullRequest(client, stack, '#8')).toEqual({ text: '2', icon: 'layers', state: 'open', label: 'Stack of 2 pull requests, open' });
  expect(searchPullRequest(client, stack, '#99')).toBeNull();
});

test('a GitLab result says MR and merge requests, as the host does', async () => {
  const { client } = await opened();
  const pending = { branchPullRequest: { number: 12, repository: 'group/project', url: 'https://gitlab.com/group/project/-/merge_requests/12' }, pullRequests: [] };
  expect(searchPullRequest(client, pending, '#12')).toEqual({ text: '12', icon: 'git-pull-request-arrow', state: '', label: 'MR #12, status pending' });
  const link = (number: number, base: string) => ({ number, repository: 'group/project', host: 'gitlab.com', url: `https://gitlab.com/group/project/-/merge_requests/${number}`,
    linkedAt: `2026-10-0${number % 9 + 1}T00:00:00.000Z`, snapshot: { title: `Change ${number}`, state: 'open', isDraft: false, headBranch: `b${number}`, baseBranch: base } });
  const stack = { id: 't-mr-stack', projectId: 'p1', pullRequests: [link(7, 'main'), link(8, 'b7')] };
  expect(searchPullRequest(client, stack, '#8')).toEqual({ text: '2', icon: 'layers', state: 'open', label: 'Stack of 2 merge requests, open' });
});

test('passive VCS reads retain no remote poller lease; active reads do', () => {
  const scopes = new ActivityScopes();
  expect(scopes.retain('e', 'subscribeVcsStatus', { cwd: '/repo', includeRemote: false })).toBeNull();
  expect(scopes.values('e')).toEqual([]);
  const release = scopes.retain('e', 'subscribeVcsStatus', { cwd: '/repo' });
  expect(scopes.values('e')).toEqual([{ type: 'vcs-status', cwd: '/repo' }]);
  release!();
  expect(scopes.values('e')).toEqual([]);
});

test('a thread route remains loading while its shell is not authoritative', async () => {
  const { client } = await opened();
  client.threadId = 'missing'; client.thread = null; client.threadLive = true; client.shellLive = false;
  expect((await pagesHome(client, null, false)).landing).toBe('offline');
  client.shellLive = true;
  expect((await pagesHome(client, null, false)).landing).toBe('no-thread');
  expect(client.threadId).toBe('missing');
});

test('a new-thread action for a deleted project keeps the open conversation', async () => {
  const { client, command } = await opened();
  await command('new-thread', 'removed-project');
  expect(client.threadId).toBe('t1');
  expect(client.projectId).toBe('p1');
});

test('revealing an existing terminal chooses its panel or drawer without allocating a shell', async () => {
  const { client } = await opened();
  const ref = { environmentId: client.environmentId, threadId: client.threadId }, key = scopedThreadKey(ref);
  revealThreadTerminal(client, ref, 'term-existing');
  expect(selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref)).toMatchObject({ terminalOpen: true, activeTerminalId: 'term-existing', terminalIds: ['term-existing'] });
  const state = { surfaces: [{ id: 'surface', kind: 'terminal' as const, terminal: { terminalIds: ['term-panel', 'term-other'], activeTerminalId: 'term-other', splitDirection: 'horizontal' as const } }], active: '', visible: false, userRevision: 0 };
  surfaceStore(client).panels.set(key, state);
  revealThreadTerminal(client, ref, 'term-panel');
  expect(state).toMatchObject({ active: 'surface', visible: true });
  expect(state.surfaces[0]!.terminal.activeTerminalId).toBe('term-panel');
});

test('the project combobox filters, moves its highlight, and commits only an existing row', async () => {
  const picker = contractComponent('pages-hero.contract', 'HeroPicker'), state = picker.state();
  const dbStep = await contractFn('diff.contract', 'dbStep', ['ids', 'current', 'down']);
  const calls: unknown[][] = [], scope = { current: 'a', rowIds: ['a', 'b'], highlight: 'a', dbStep,
    hidePopover: (...args: unknown[]) => calls.push(['hide', ...args]), focus: (...args: unknown[]) => calls.push(['focus', ...args]),
    command: (...args: unknown[]) => calls.push(['command', ...args]), addProject: () => calls.push(['add']), preventDefault() {} };
  picker.call('search', state, scope, ['Beta']);
  expect(state).toMatchObject({ query: 'Beta', cursor: '' });
  picker.call('searchKey', state, scope, ['ArrowDown']);
  expect(state.cursor).toBe('b');
  picker.call('pickRow', state, scope, ['b']);
  expect(calls).toEqual([['hide', 'hero-project-menu'], ['focus', 'empty-choose-project'], ['command', 'pages:hero-project', 'b', '', 0]]);
});


test('a scratch thread identifies its machine only when projects span environments', async () => {
  const { client } = await opened();
  client.config.scratchWorkspaceRoot = '/repo';
  client.config.environment = { ...client.config.environment, label: 'Work Mac' };
  expect(scratchMachine(client, client.shell.threads[0]!, false)).toBeNull();
  expect(scratchMachine(client, client.shell.threads[0]!, true)).toMatchObject({ label: 'Work Mac' });
  client.config.scratchWorkspaceRoot = '/other';
  expect(scratchMachine(client, client.shell.threads[0]!, true)).toBeNull();
});

test('a Working row lifts and can drop into the composer without changing shelves', () => {
  const window = contractComponent('app-window.contract', 'T3Window');
  const state = { dragId: '', dragSection: '', dragDx: 0, dragDy: 0, dragX0: 0, dragY0: 0, dragW: 0, dragH: 0, dragTopY: 0, dragDividerY: 0, dragWorkingY: 0, dragSnoozedY: 0, dragSettledY: 0 };
  const calls: unknown[][] = [], scope = { data: { sidebar: { dragEnabled: true, workingCount: 1, snoozedCount: 0 } }, commandPending: false,
    sidebarHoverId: '', sidebarHoverRoot() {}, frame: () => ({ x: 100, y: 200, width: 300, height: 100 }),
    sidebarRun: (...args: unknown[]) => calls.push(args), dragTarget: '', dragCX: 150, dragCY: 250 };
  window.call('sidebarDrag', state, scope, ['t', 'working', 20, 10]);
  expect(state).toMatchObject({ dragId: 't', dragSection: 'working', dragDx: 20, dragDy: 10 });
  window.call('sidebarDragEnd', state, scope, ['t', 0, 0]);
  expect(calls).toEqual([]);
  state.dragId = 't'; state.dragSection = 'working';
  window.call('sidebarDragEnd', state, { ...scope, dragTarget: 'context' }, ['t', 0, 0]);
  expect(calls).toEqual([['drop', 't', 'context|250|200|200|true']]);
  expect(state.dragId).toBe('');
});
