import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
afterEach(resetPrimary);
import { environmentIndicator, shellDetails } from '../../src/details/shell-details';
import { lineageView, statusLabel, formatElapsed, currentSubagent, latestMergeBackRun } from '../../src/details/shell-lineage';
import { adoptShellPrefs, inlineOpen, shellPrefs, toggleInline } from '../../src/shell/shell-prefs';
import { pushToast, toasts } from '../../src/shell/toast';
import { applyDismissals, toastViews } from '../../src/shell/shell';
import { applyStatusEvent, refreshVcsOnFocus, vcsStatusEvent, VCS_STATUS_KEY } from '../../src/details/shell-vcs';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';

const at = (minutes: number) => new Date(Date.UTC(2026, 9, 4, 10, minutes)).toISOString();
const shell = (id: string, extra: Obj = {}): Obj => ({ id, title: `T ${id}`, status: 'idle', createdAt: at(0), lineage: { parentThreadId: null, relationshipToParent: null }, ...extra });

describe('environment row (upstream bb7997709d)', () => {
  test('a lone primary machine has no row; a lone remote one is a static label; two machines can pick', () => {
    const base = { environmentId: 'env-1', runtimeLabel: '', savedLabel: '', machine: '' };
    expect(environmentIndicator({ ...base, isPrimary: true, available: 1 })).toMatchObject({ envShow: false, envLabel: 'This device' });
    expect(environmentIndicator({ ...base, isPrimary: false, available: 1, runtimeLabel: 'Studio', machine: 'mac-studio' })).toEqual({ envShow: true, envLabel: 'Studio', envKind: 'mac-studio' });
    expect(environmentIndicator({ ...base, isPrimary: false, available: 1 })).toMatchObject({ envShow: true, envLabel: 'env-1', envKind: 'server' });
    expect(environmentIndicator({ ...base, isPrimary: true, available: 2, runtimeLabel: 'Local' })).toMatchObject({ envShow: true, envLabel: 'This device' });
    expect(environmentIndicator({ ...base, isPrimary: true, available: 2, runtimeLabel: 'Laptop' }).envLabel).toBe('Laptop');
  });
});

describe('lineage (upstream d3071275d5)', () => {
  test('parent first, then the merge target, then newest first; statuses and badges', () => {
    const threads = [shell('root', { status: 'completed' }), shell('me', { lineage: { parentThreadId: 'root', relationshipToParent: 'fork' } }),
      shell('fork-old', { createdAt: at(1), lineage: { parentThreadId: 'me', relationshipToParent: 'fork' } }),
      shell('fork-new', { createdAt: at(5), status: 'running', activityRunStatus: 'running', lineage: { parentThreadId: 'me', relationshipToParent: 'fork' } })];
    const projection = { thread: { id: 'me', lineage: { parentThreadId: 'root', relationshipToParent: 'fork' } }, runs: [{ id: 'r1', ordinal: 1, status: 'completed' }], subagents: [], contextTransfers: [] };
    const view = lineageView(threads, projection, 'me', [], Date.parse(at(10)));
    expect(view.lineage.map(row => [row.id, row.icon, row.status, row.mergeTarget, row.group])).toEqual([
      ['root', 'corner-left-up', 'Done', true, 'related'], ['fork-new', 'git-fork', 'Running', false, 'related'], ['fork-old', 'git-fork', 'Idle', false, 'related']]);
    expect(view.lineage[0]!.dot).toBe('#00bc7d');
    expect(view).toMatchObject({ lineageTitle: 'Lineage', mergeTargetId: 'root', mergeSourceId: 'me', mergeRunId: 'r1', mergeLabel: 'Merge back to T root', mergeHint: 'Merge this conversation back into T root' });
    expect(view.lineage[1]!.hint).toBe('Open fork in this chat');
  });

  test('a settled subagent sent a follow-up shows as running, its timer follows the live run', () => {
    const threads = [shell('me'), shell('kid', { title: 'Subagent: /root/work/fix_tests', status: 'completed', activityRunStatus: 'running', activityRunStartedAt: at(8),
      lineage: { parentThreadId: 'me', relationshipToParent: 'subagent' } }), shell('done-kid', { status: 'completed', lineage: { parentThreadId: 'me', relationshipToParent: 'subagent' } })];
    const projection = { thread: { id: 'me', lineage: {} }, runs: [], contextTransfers: [], subagents: [
      { childThreadId: 'kid', status: 'completed', title: 'fix', driver: 'codex', startedAt: at(1), completedAt: at(2), result: 'ok' },
      { childThreadId: 'done-kid', status: 'completed', startedAt: at(1), completedAt: at(3) },
      { childThreadId: null, status: 'running' }] };
    const view = lineageView(threads, projection, 'me', [], Date.parse(at(10)));
    expect(view.lineageTitle).toBe('Lineage · 2 running');
    const kid = view.lineage.find(row => row.id === 'kid')!;
    expect(kid).toMatchObject({ group: 'active', title: 'Fix Tests', driver: 'codex', status: 'Running', agent: true, elapsed: '2m 00s' });
    expect(view.lineage.find(row => row.id === 'done-kid')).toMatchObject({ group: 'previous', elapsed: '2m 00s', status: 'Done' });
    expect(view.previousCount).toBe(1);
    expect(currentSubagent({ status: 'completed', result: 'x' }, { activityRunStatus: 'starting' })).toMatchObject({ status: 'pending', startedAt: null, result: null });
  });

  test('a queued follow-up and a finished one follow the child thread; a rolled-back child reads as stopped (#17696)', () => {
    const task = { status: 'completed', startedAt: at(1), completedAt: at(2), result: 'ok' };
    expect(currentSubagent(task, { status: 'queued', latestRunRequestedAt: at(3) })).toMatchObject({ status: 'pending', startedAt: null, completedAt: null, result: null });
    expect(currentSubagent(task, { status: 'failed', latestRunRequestedAt: at(3), latestRunStartedAt: at(4), latestRunCompletedAt: at(6) }))
      .toMatchObject({ status: 'failed', startedAt: at(4), completedAt: at(6), result: null });
    expect(currentSubagent(task, { status: 'rolled_back', latestRunRequestedAt: at(3) })).toMatchObject({ status: 'interrupted' });
    // A child run requested before the task settled is the task's own.
    expect(currentSubagent(task, { status: 'completed', latestRunRequestedAt: at(1) })).toBe(task);
    const threads = [shell('me'), shell('kid', { status: 'rolled_back', latestRunId: 'r2', lineage: { parentThreadId: 'me', relationshipToParent: 'subagent' } })];
    const projection = { thread: { id: 'me', lineage: {} }, runs: [], contextTransfers: [], subagents: [{ childThreadId: 'kid', status: 'completed' }] };
    expect(lineageView(threads, projection, 'me', [], Date.parse(at(10))).lineage.find(row => row.id === 'kid')).toMatchObject({ status: 'Stopped', group: 'previous' });
  });

  test('labels, elapsed format and the merge-back run', () => {
    expect(['preparing', 'queued', 'blocked', 'interrupted', 'rolled_back', 'resolved_native', 'nope'].map(statusLabel)).toEqual(['Starting', 'Queued', 'Waiting', 'Stopped', 'Reverted', 'Resolved (native)', 'Unknown']);
    expect([5, 65, 3725].map(formatElapsed)).toEqual(['5s', '1m 05s', '1h 02m']);
    expect(latestMergeBackRun({ runs: [{ id: 'a', ordinal: 1, status: 'completed' }, { id: 'b', ordinal: 2, status: 'running' }] })).toBeNull();
    expect(latestMergeBackRun({ runs: [{ id: 'a', ordinal: 1, status: 'completed' }, { id: 'b', ordinal: 2, status: 'waiting' }] })).toMatchObject({ id: 'b' });
  });
});

describe('Git status on window focus (GitActionsControl, exactPage().hasFocus: exact2 #219)', () => {
  test('the window regaining the focus asks vcs.refreshStatus for the card\'s workspace once', async () => {
    const requests: [string, Obj][] = [];
    const client = { restAccess: () => ({ request: async (method: string, payload: Obj) => { requests.push([method, payload]); return {}; } }) } as unknown as T3Client;
    const native = {} as Native;
    expect(await refreshVcsOnFocus(client, native, '/repo', true)).toBe(false); // the first answer only records
    expect(await refreshVcsOnFocus(client, native, '/repo', false)).toBe(false); // blur asks nothing
    expect(await refreshVcsOnFocus(client, native, '/repo', true)).toBe(true);
    expect(await refreshVcsOnFocus(client, native, '/repo', true)).toBe(false); // still focused: no second ask
    expect(await refreshVcsOnFocus(client, native, '', false)).toBe(false);
    expect(await refreshVcsOnFocus(client, native, '', true)).toBe(false); // no card: nothing to refresh
    expect(requests).toEqual([['vcs.refreshStatus', { cwd: '/repo' }]]);
  });
});

describe('shell preferences', () => {
  test('inline cards are open by default; a closed one is remembered and reloaded', () => {
    const owner = { local: {} };
    expect(inlineOpen(owner, 'env:t1')).toBe(true);
    expect(toggleInline(owner, 'env:t1')).toBe(false);
    expect(inlineOpen(owner, 'env:t1')).toBe(false);
    expect(inlineOpen(owner, 'env:t2')).toBe(true);
    shellPrefs(owner).nightlyNoticeDismissed = true;
    const next = {};
    adoptShellPrefs(next, JSON.parse(JSON.stringify({ shell: shellPrefs(owner) })));
    expect(next).toEqual({ shell: { nightlyNoticeDismissed: true, inlineClosed: ['env:t1'], providerUpdateDismissals: [], versionMismatchDismissals: [], lastEditor: '', chatgptSharingWelcome: [], permissionUpdateDismissals: [], databaseStorageDismissals: [] } });
    expect(toggleInline(owner, 'env:t1')).toBe(true);
  });
});

describe('toast action dismissal', () => {
  const client = () => ({ local: {}, environmentId: 'env' }) as unknown as T3Client;
  test('an ordinary action close leaves onClose alone', () => {
    const owner = client();
    let closed = 0;
    const id = pushToast(owner, { kind: 'info', title: 'x', action: { label: 'Go', op: 'ui:settings' }, onClose: () => { closed++; } });
    expect(applyDismissals(owner, `a${id}`)).toBe(false);
    expect(closed).toBe(0);
  });
});

describe('details source (upstream 429c625a85, d1034d62b2)', () => {
  const native: Native = { available: true, watch() {}, later: async () => ({}) };
  const make = (status: Obj, extra: Obj = {}) => {
    const calls: Obj[] = [];
    if (!extra.origin) primaryAt('http://127.0.0.1:3773', 'env'); // the focused environment is this Mac's embedded server, unless the case names another
    const client = { ready: true, projectId: 'p1', threadId: 't1', environmentId: 'env', origin: 'http://127.0.0.1:3773', local: {}, draftKey: 'env:t1', generation: 1,
      config: { availableEditors: ['cursor'], environment: { label: 'Local' }, providers: [] }, projection: { thread: { id: 't1', lineage: {} }, runs: [], subagents: [], contextTransfers: [] },
      shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [shell('t1', { projectId: 'p1' })] },
      // Only the git status stream is recorded; the live scheduled-task stream (live-streams.ts) has its own tests.
      restAccess: () => ({ call: async (request: Obj) => { if (request.key === 'scheduled-tasks' || request.key === 'project-clones') return { id: 'live-1' }; calls.push(request); return request.op === 'subscribe' ? { id: `sub-${calls.length}` } : {}; } }), ...extra } as unknown as T3Client;
    // The stream's first event, as the client's drain hands it over.
    const deliver = () => { const id = String(calls.filter(call => call.op === 'subscribe').length ? `sub-${calls.length}` : ''); vcsStatusEvent(client, { subscriptionId: id, value: { _tag: 'snapshot', local: status, remote: null } }); };
    return { client, calls, deliver };
  };
  test('inline where chat fits beside it and open by default; the popover only while open; one status stream', async () => {
    const { client, calls, deliver } = make({ isRepo: true, refName: 'main', workingTree: { insertions: 1, deletions: 2 } });
    expect(await shellDetails(client, native, false, 't1', false)).toMatchObject({ ready: false, inline: false });
    expect(await shellDetails(client, native, true, 't1', false)).toMatchObject({ ready: true, inline: false, isGit: false });
    expect(calls).toEqual([{ op: 'subscribe', key: VCS_STATUS_KEY, method: 'subscribeVcsStatus', payload: { cwd: '/work/project' } }]);
    deliver();
    const inline = await shellDetails(client, native, false, 't1', true);
    expect(inline).toMatchObject({ ready: true, inline: true, isGit: true, branch: 'main', folderName: 'project', folderLabel: '', insertions: 1, deletions: 2, envShow: false });
    expect(calls).toHaveLength(1);
    toggleInline(client, 'env:t1');
    expect(await shellDetails(client, native, false, 't1', true)).toMatchObject({ ready: false, inline: false });
    expect(calls.at(-1)).toEqual({ op: 'unsubscribe', key: VCS_STATUS_KEY });
    // Where the floating player leaves the docked card no placement, the header
    // opens the popover on a wide chat too, so an open popover reads the details whatever the docked card's state.
    expect(await shellDetails(client, native, true, 't1', true)).toMatchObject({ ready: true, inline: false });
  });
  test('the Changes row prefers the branch totals; a worktree is named', async () => {
    const { client, deliver } = make({ isRepo: true, refName: 'feature', workingTree: { insertions: 1, deletions: 0 }, branchChanges: { baseRef: null, insertions: 40, deletions: 7 } },
      { shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [shell('t1', { projectId: 'p1', worktreePath: '/work/wt/feature-x' })] } });
    await shellDetails(client, native, true, 't1', false);
    deliver();
    expect(await shellDetails(client, native, true, 't1', false)).toMatchObject({ folderName: 'feature-x', folderLabel: 'Worktree', insertions: 40, deletions: 7 });
  });
  test('a remote machine gets its row', async () => {
    const { client } = make({ isRepo: false }, { origin: 'https://box.example.com', config: { availableEditors: [], environment: { label: 'Build box', platform: { machine: 'cloud' } }, providers: [] } });
    expect(await shellDetails(client, native, true, 't1', false)).toMatchObject({ envShow: true, envLabel: 'Build box', envKind: 'cloud' });
  });
  test('stream events fold like applyGitStatusStreamEvent', () => {
    const local = { isRepo: true, refName: 'main', workingTree: { insertions: 0, deletions: 0 } };
    const first = applyStatusEvent(null, { _tag: 'snapshot', local, remote: { hasUpstream: true, aheadCount: 2, behindCount: 0, pr: null } });
    expect(first).toMatchObject({ refName: 'main', aheadCount: 2 });
    expect(applyStatusEvent(first, { _tag: 'localUpdated', local: { ...local, refName: 'next' } })).toMatchObject({ refName: 'next', aheadCount: 2, hasUpstream: true });
    expect(applyStatusEvent(first, { _tag: 'remoteUpdated', remote: null })).toMatchObject({ refName: 'main', aheadCount: 0, hasUpstream: false });
    expect(applyStatusEvent(null, { _tag: 'remoteUpdated', remote: { hasUpstream: true, aheadCount: 1, behindCount: 1, pr: null } })).toMatchObject({ isRepo: true, refName: null, behindCount: 1 });
  });
});

describe('copy error morph state (upstream f2cc80a7a4)', () => {
  test('a copied error reads Copied with a check for 2 s on the shell clock', async () => {
    const { markCopied, copiedToasts } = await import('../../src/shell/shell');
    const owner = { local: {}, environmentId: 'env' } as unknown as T3Client;
    const id = pushToast(owner, { kind: 'error', title: 'Failed', description: 'boom' });
    expect(copiedToasts(owner, 10_000).size).toBe(0);
    markCopied(owner, id);
    expect(toastViews(toasts(owner), copiedToasts(owner, 11_000))[0]).toMatchObject({ copyText: 'boom', copied: true });
    expect(toastViews(toasts(owner), copiedToasts(owner, 12_000))[0]!.copied).toBe(false);
  });
});

describe('workspace status stream ordering', () => {
  test('a snapshot drained before the subscribe reply is kept; an older stream never wins', async () => {
    const { watchVcsStatus, vcsStatusEvent: deliver } = await import('../../src/details/shell-vcs');
    let next = 10;
    const replies: ((value: Obj) => void)[] = [];
    const client = { generation: 1, revision: 1, restAccess: () => ({ call: (request: Obj) => request.op === 'subscribe' ? new Promise<Obj>(resolve => replies.push(resolve)) : Promise.resolve({}) }) } as unknown as T3Client;
    const native: Native = { available: true, watch() {}, later: async () => ({}) };
    const local = (refName: string) => ({ isRepo: true, refName, workingTree: { insertions: 0, deletions: 0 } });
    const first = watchVcsStatus(client, native, '/a', 1000);
    deliver(client, { subscriptionId: `1-${next}`, value: { _tag: 'snapshot', local: local('main'), remote: null } });
    replies.shift()!({ id: `1-${next}` });
    expect((await first).status).toMatchObject({ refName: 'main' });
    // The workspace changes; a late event from the old stream must not paint the new one.
    const second = watchVcsStatus(client, native, '/b', 1000);
    deliver(client, { subscriptionId: `1-${next}`, value: { _tag: 'localUpdated', local: local('stale') } });
    next = 12;
    deliver(client, { subscriptionId: `1-${next}`, value: { _tag: 'snapshot', local: local('feature'), remote: null } });
    replies.shift()!({ id: `1-${next}` });
    expect((await second).status).toMatchObject({ refName: 'feature' });
    expect((await watchVcsStatus(client, native, '/b', 1000)).status).toMatchObject({ refName: 'feature' });
  });
});
