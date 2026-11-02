// T3 Code 74541bab17: useProactivePanels, its ChatView.logic.ts rules and rightPanelStore.openProactive over the port.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Files, Native } from '../../src/client/protocol';
import { diffOps } from '../../src/client/client-ops-diff';
import type { PanelState, Surface } from '../../src/panel/panel-surfaces';
import { panelState, surfaceLocal } from '../../src/panel/panel-surfaces';
import { adoptRightPanels, restoreRightPanel, savedPanel, syncRightPanels } from '../../src/panel/saved-panel';
import {
  applyProactivePanels, openProactive, proactiveInput, proactiveMemory, proactivePanels, resolveProactiveTurnDiffAction, shouldOpenProactiveTurnDiff,
  type LinkedPullRequest, type ProactiveInput, type TurnCheckpoint,
} from '../../src/panel/proactive-panels';

const empty = (): PanelState => ({ surfaces: [], active: '', visible: false, userRevision: 0 });
const file = (path: string): Surface => ({ id: `file:${path}`, kind: 'file', path, line: 0, reveal: 0 });
const files = (count: number, lines = 2) => Array.from({ length: count }, () => ({ additions: lines, deletions: 0 }));
const ready = (runId: string, count = 3, lines = 2): TurnCheckpoint => ({ runId, status: 'ready', files: files(count, lines) });
const linked: LinkedPullRequest = { projectId: 'p1', repository: 'acme/notes', number: 7, url: 'https://github.com/acme/notes/pull/7' };
const prTarget = (link: LinkedPullRequest) => ({ projectId: link.projectId, host: '', repository: link.repository, number: link.number, url: link.url });

function input(change: Partial<ProactiveInput> = {}): ProactiveInput {
  return {
    threadKey: 'env:t1', enabled: true, settingsHydrated: true, threadDetailLoading: false,
    runningTurnId: null, latestRun: null, latestRunSettled: false, turnDiffSummaries: [], isGitRepo: true,
    linkedPullRequest: null, hasLinkedPullRequestDetail: false, pullRequestsKey: null,
    pullRequestsCapabilityKnown: true, supportsPullRequests: true, pullRequestsSurfaceAvailable: false, visiblePullRequestCount: 0,
    ...change,
  };
}
const running = (runId: string) => input({ runningTurnId: runId, latestRun: { runId, status: 'running' } });
const completed = (runId: string, checkpoints: TurnCheckpoint[] = [ready(runId)], change: Partial<ProactiveInput> = {}) =>
  input({ latestRun: { runId, status: 'completed' }, latestRunSettled: true, turnDiffSummaries: checkpoints, ...change });

describe('which completed turn opens Changes', () => {
  test('at least 3 files or 50 changed lines, in a repository, once the checkpoint is ready', () => {
    const action = (checkpoint: TurnCheckpoint | undefined, isGitRepo: boolean | undefined = true) => resolveProactiveTurnDiffAction({ checkpoint, isGitRepo });
    expect(action(ready('r', 3))).toBe('open');
    expect(action(ready('r', 1, 50))).toBe('open');
    expect(action(ready('r', 2, 24))).toBe('ignore');
    expect(action(ready('r', 0))).toBe('ignore');
    expect(action({ runId: 'r', status: 'error', files: files(5) })).toBe('ignore');
    expect(action(ready('r', 3), false)).toBe('ignore');
    // Still loading: the checkpoint, a missing one, or the repository status.
    expect(action(undefined)).toBe('defer');
    expect(action({ runId: 'r', status: 'missing', files: [] })).toBe('defer');
    expect(resolveProactiveTurnDiffAction({ checkpoint: ready('r', 3), isGitRepo: undefined })).toBe('defer');
  });
  test('a huge checkpoint\'s prefix of files counts by its totals (checkpointFileTotals)', () => {
    const action = (filesTotal: { count: number; additions: number; deletions: number }) => resolveProactiveTurnDiffAction({ checkpoint: { ...ready('r', 1), filesTotal }, isGitRepo: true });
    expect(action({ count: 3, additions: 1, deletions: 0 })).toBe('open');
    expect(action({ count: 1, additions: 30, deletions: 20 })).toBe('open');
    expect(action({ count: 2, additions: 10, deletions: 10 })).toBe('ignore');
    const totals = { count: 400, additions: 9000, deletions: 120 };
    const input = proactiveInput({ threadId: 't1', environmentId: 'env', generation: 1, preferencesLoaded: true, local: { clientSettings: {} }, config: {}, shell: { projects: [], threads: [{ id: 't1' }] },
      thread: {}, projection: { runs: [], checkpoints: [{ runId: 'r1', appRunOrdinal: 1, status: 'ready', files: [{ path: 'a', additions: 1, deletions: 0 }], filesTotal: totals },
        { runId: 'r2', appRunOrdinal: 2, status: 'ready', files: [], filesTotal: { count: -1 } }] } } as unknown as T3Client, 'env:t1', false);
    expect(input.turnDiffSummaries).toEqual([{ runId: 'r1', status: 'ready', files: [{ additions: 1, deletions: 0 }], filesTotal: totals }, { runId: 'r2', status: 'ready', files: [] }]);
  });
  test('only the turn seen running, or the latest one on first entry', () => {
    const open = (previousRunningTurnId: string | null | undefined, settledTurnId = 'r1') =>
      shouldOpenProactiveTurnDiff({ previousRunningTurnId, runningTurnId: null, settledTurnId, turnCompleted: true });
    expect([open('r1'), open(undefined), open(null), open('r0')]).toEqual([true, true, false, false]);
    expect(shouldOpenProactiveTurnDiff({ previousRunningTurnId: 'r1', runningTurnId: 'r2', settledTurnId: 'r1', turnCompleted: true })).toBe(false);
    expect(shouldOpenProactiveTurnDiff({ previousRunningTurnId: 'r1', runningTurnId: null, settledTurnId: 'r1', turnCompleted: false })).toBe(false);
  });
});

describe('openProactive', () => {
  test('offers each turn diff once, recorded even when refused, and never over a Diff or pull request', () => {
    const state = empty();
    expect(openProactive(state, { kind: 'diff', turnId: 'r1' }, 0)).toBe(true);
    expect([state.active, state.visible, state.proactiveDiffTurnId]).toEqual(['diff', true, 'r1']);
    state.visible = false; state.userRevision++;
    expect(openProactive(state, { kind: 'diff', turnId: 'r1' }, state.userRevision)).toBe(false);
    expect(state.visible).toBe(false);
    // A choice after the turn began refuses the next turn too, but the offer is spent.
    expect(openProactive(state, { kind: 'diff', turnId: 'r2' }, state.userRevision - 1)).toBe(false);
    expect([state.visible, state.proactiveDiffTurnId]).toEqual([false, 'r2']);
    const pr = empty();
    expect(openProactive(pr, { kind: 'pull-requests' }, 0)).toBe(true);
    expect(openProactive(pr, { kind: 'diff', turnId: 'r1' }, 0)).toBe(false);
    expect([pr.active, pr.proactiveDiffTurnId]).toEqual(['pull-requests', 'r1']);
  });
  test('a pull request tab already there stays where the person left it, unless a retarget asks for it', () => {
    const state = empty(), request = { kind: 'pull-request' as const, target: prTarget(linked) };
    expect(openProactive(state, request, 0)).toBe(true);
    const id = state.active;
    expect(id).toBe('pull-request:p1:acme%2Fnotes:7');
    state.surfaces.push(file('a.ts')); state.active = 'file:a.ts'; state.userRevision++;
    expect(openProactive(state, request, state.userRevision)).toBe(false);
    expect(state.active).toBe('file:a.ts');
    expect(openProactive(state, request, state.userRevision, true)).toBe(true);
    expect([state.active, state.surfaces.length]).toEqual([id, 2]);
    expect(openProactive(state, request, state.userRevision - 1, true)).toBe(false);
  });
});

describe('useProactivePanels', () => {
  test('a turn that edits 3 files opens Changes when it completes, with the setting on only', () => {
    for (const enabled of [true, false]) {
      const memory = proactiveMemory(), state = empty();
      expect(proactivePanels(memory, state, { ...input(), enabled })).toBe('');
      expect(proactivePanels(memory, state, { ...running('r1'), enabled })).toBe('');
      // The checkpoint lands after the run settles: the opening waits for it.
      expect(proactivePanels(memory, state, completed('r1', [], { enabled }))).toBe('');
      expect(proactivePanels(memory, state, completed('r1', [ready('r1')], { enabled }))).toBe(enabled ? 'diff' : '');
      expect(state.visible).toBe(enabled);
      expect(proactivePanels(memory, state, completed('r1', [ready('r1')], { enabled }))).toBe('');
    }
  });
  test('a turn under the thresholds, or one already settled while watched, opens nothing', () => {
    const memory = proactiveMemory(), state = empty();
    proactivePanels(memory, state, running('r1'));
    expect(proactivePanels(memory, state, completed('r1', [ready('r1', 2, 10)]))).toBe('');
    // Watched idle, then a settled turn appears (another client's): not the turn seen running.
    expect(proactivePanels(memory, state, completed('r2'))).toBe('');
    expect(state.proactiveDiffTurnId).toBeUndefined();
  });
  test('a panel the person opened or closed during the turn wins', () => {
    const memory = proactiveMemory(), state = empty();
    proactivePanels(memory, state, running('r1'));
    state.surfaces.push(file('a.ts')); state.active = 'file:a.ts'; state.visible = true; state.userRevision++;
    expect(proactivePanels(memory, state, completed('r1'))).toBe('');
    expect([state.active, state.proactiveDiffTurnId]).toEqual(['file:a.ts', 'r1']);
    // The next turn reads the revision afresh when it starts.
    proactivePanels(memory, state, running('r2'));
    expect(proactivePanels(memory, state, completed('r2', [ready('r2')]))).toBe('diff');
    expect(state.active).toBe('diff');
  });
  test('entering a thread offers its latest completed turn once; a closed proactive diff stays closed on return', () => {
    const memory = proactiveMemory(), state = empty();
    expect(proactivePanels(memory, state, completed('r1'))).toBe('diff');
    state.visible = false; state.userRevision++;
    proactivePanels(memory, empty(), input({ threadKey: 'env:t2' }));
    expect(proactivePanels(memory, state, completed('r1'))).toBe('');
    expect(state.visible).toBe(false);
  });
  test('linked pull requests open first: the linked one, or the list when the thread shows several', () => {
    const memory = proactiveMemory(), state = empty();
    const linkedKey = JSON.stringify([linked.projectId, linked.repository, linked.number]);
    const withLink = completed('r1', [ready('r1')], { linkedPullRequest: linked, hasLinkedPullRequestDetail: true, pullRequestsKey: linkedKey });
    expect(proactivePanels(memory, state, withLink)).toBe('pull-request');
    expect(state.surfaces.map(surface => surface.kind)).toEqual(['pull-request']);
    // A thread that has a pull request does not also offer its turn's diff.
    expect(state.proactiveDiffTurnId).toBeUndefined();
    const listKey = JSON.stringify([['github.com', 'acme/notes', 7], ['github.com', 'acme/notes', 8]]);
    const list = empty();
    expect(proactivePanels(proactiveMemory(), list, input({ linkedPullRequest: linked, hasLinkedPullRequestDetail: true, pullRequestsKey: listKey, pullRequestsSurfaceAvailable: true, visiblePullRequestCount: 2 }))).toBe('pull-requests');
    // Off, or before the server's capabilities are known (then it waits for them).
    expect(proactivePanels(proactiveMemory(), empty(), { ...withLink, enabled: false })).toBe('');
    const deferred = proactiveMemory(), later = empty();
    expect(proactivePanels(deferred, later, { ...withLink, pullRequestsCapabilityKnown: false })).toBe('');
    expect(proactivePanels(deferred, later, withLink)).toBe('pull-request');
  });
  test('an open panel showing the linked pull request follows a new link, with the setting off too', () => {
    const memory = proactiveMemory(), state = empty(), relinked = { ...linked, number: 9, url: 'https://github.com/acme/notes/pull/9' };
    proactivePanels(memory, state, input({ enabled: false, linkedPullRequest: linked }));
    openProactive(state, { kind: 'pull-request', target: prTarget(linked) }, state.userRevision);
    expect(proactivePanels(memory, state, input({ enabled: false, linkedPullRequest: relinked }))).toBe('pull-request');
    expect(state.active).toBe('pull-request:p1:acme%2Fnotes:9');
    // Not when the panel shows something else.
    state.surfaces.push(file('a.ts')); state.active = 'file:a.ts';
    expect(proactivePanels(memory, state, input({ enabled: false, linkedPullRequest: linked }))).toBe('');
  });
  test('drafts reset the observation', () => {
    const memory = proactiveMemory(), state = empty();
    proactivePanels(memory, state, running('r1'));
    expect(proactivePanels(memory, state, input({ threadKey: null }))).toBe('');
    expect(memory.observation).toBeNull();
  });
});

describe('the chat view unmounts the hook', () => {
  const client = (projection: object, change: object = {}) => ({
    threadId: 't1', environmentId: 'env', generation: 1, preferencesLoaded: true, local: { clientSettings: { proactivePanelsEnabled: false } },
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [], threads: [{ id: 't1', projectId: 'p1', ...change }] },
    thread: {}, projection,
  }) as unknown as T3Client;
  const enable = (app: T3Client) => { (app.local as { clientSettings: { proactivePanelsEnabled: boolean } }).clientSettings.proactivePanelsEnabled = true; };
  test('enabled in Settings, returning to a thread with a linked pull request opens it', () => {
    const app = client({}, { linkedPullRequest: linked }), state = empty();
    expect(applyProactivePanels(app, 'env:t1', state, false)).toBe('');
    enable(app);
    expect(applyProactivePanels(app, 'env:t1', state, false, false)).toBe(''); // Settings shows
    expect(applyProactivePanels(app, 'env:t1', state, false)).toBe('pull-request');
    expect(state.active).toBe(`pull-request:p1:acme%2Fnotes:7`);
  });
  test('enabled in Settings, returning offers the latest qualifying turn once', () => {
    const projection = { runs: [{ id: 'r1', ordinal: 1, status: 'completed' }], checkpoints: [{ runId: 'r1', appRunOrdinal: 1, status: 'ready', files: [{ additions: 1, deletions: 0 }, { additions: 1, deletions: 0 }, { additions: 1, deletions: 0 }] }] };
    const app = client(projection), state = empty();
    const memory = proactiveMemory();
    // The repository state is the one input read from the VCS stream: feed it through the pure hook.
    const read = (shown: boolean) => shown ? proactivePanels(memory, state, { ...proactiveInput(app, 'env:t1', false), isGitRepo: true }) : (memory.observation = null, '');
    expect(read(true)).toBe('');
    enable(app);
    expect(read(false)).toBe('');
    expect(read(true)).toBe('diff');
    state.visible = false; state.userRevision++;
    expect(read(false)).toBe('');
    expect(read(true)).toBe('');
  });
});

describe('the inputs ChatView hands the hook', () => {
  const client = (change: object = {}) => ({
    threadId: 't1', environmentId: 'env', generation: 1, preferencesLoaded: true, local: { clientSettings: { proactivePanelsEnabled: true } },
    config: { environment: { capabilities: { pullRequests: true, threadPullRequests: true } } },
    shell: { projects: [], threads: [{ id: 't1', projectId: 'p1', linkedPullRequest: linked, pullRequests: [{ host: 'github.com', repository: 'acme/notes', number: 7 }, { host: 'github.com', repository: 'acme/notes', number: 8, source: 'stack-dismissed' }] }] },
    thread: {}, projection: {
      runs: [{ id: 'r1', ordinal: 1, status: 'completed' }, { id: 'r2', ordinal: 2, status: 'running' }, { id: 'r3', ordinal: 3, status: 'queued', queueHeld: true }],
      checkpoints: [{ runId: 'r1', appRunOrdinal: 1, status: 'ready', files: [{ path: 'a', additions: 3, deletions: 1 }] }, { runId: null, appRunOrdinal: null, status: 'ready', files: [] }],
    }, ...change,
  }) as unknown as T3Client;
  test('the running and latest runs, the checkpoints and the pull request key', () => {
    const value = proactiveInput(client(), 'env:t1', false);
    expect(value).toMatchObject({ threadKey: 'env:t1', enabled: true, runningTurnId: 'r2', latestRun: { runId: 'r2', status: 'running' }, latestRunSettled: false,
      turnDiffSummaries: [{ runId: 'r1', status: 'ready', files: [{ additions: 3, deletions: 1 }] }], linkedPullRequest: linked, hasLinkedPullRequestDetail: true,
      pullRequestsKey: JSON.stringify([['github.com', 'acme/notes', 7]]), pullRequestsSurfaceAvailable: true, visiblePullRequestCount: 1, supportsPullRequests: true });
    expect(proactiveInput(client(), 'env:t1', true).enabled).toBe(false); // the narrow window's sheet
    const settled = proactiveInput(client({ projection: { runs: [{ id: 'r1', ordinal: 1, status: 'completed' }] } }), 'env:t1', false);
    expect([settled.runningTurnId, settled.latestRunSettled]).toEqual([null, true]);
    expect(proactiveInput(client({ threadId: '' }), 'env:new:p1', false).threadKey).toBeNull();
    expect(proactiveInput(client({ thread: null }), 'env:t1', false)).toMatchObject({ threadDetailLoading: true, runningTurnId: null, turnDiffSummaries: [] });
  });
});

describe('the panel choices around the Diff', () => {
  const native = { available: true, watch() {}, async later() { return {}; } } as unknown as Native;
  test('opening or closing the Diff counts as a choice; its scope and view options do not', async () => {
    const app = { local: {}, environmentId: 'env', threadId: 't1', projectId: 'p1', diffOpen: true, diffState: { ignoreWhitespace: false }, get draftKey() { return 'env:t1'; } } as unknown as T3Client;
    const out = { message: '', id: '', value: '' };
    await diffOps.call(app, 'close-diff', '', '', 0, native, {} as Files, out);
    expect(panelState(app).userRevision).toBe(1);
    await diffOps.call(app, 'diff-whitespace', '', '', 0, native, {} as Files, out);
    expect(panelState(app).userRevision).toBe(1);
  });
  test('an op inside a surface counts only when it changes the panel; a Files search during a turn still lets the Diff open', async () => {
    const app = { local: {}, environmentId: 'env', threadId: 't1', projectId: 'p1', diffOpen: false, ready: true, shell: { projects: [], threads: [] },
      get draftKey() { return 'env:t1'; }, restAccess: () => ({ call: async () => ({}), request: async () => ({}) }) } as unknown as T3Client;
    const state = panelState(app); state.surfaces = [{ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }]; state.active = 'files'; state.visible = true;
    const memory = proactiveMemory();
    proactivePanels(memory, state, running('r1'));
    await surfaceLocal(app, native, 'files-search', '', 'notes').catch(() => '');
    await surfaceLocal(app, native, 'pr-tab', 'files', 'code');
    expect(state.userRevision).toBe(0);
    expect(proactivePanels(memory, state, completed('r1'))).toBe('diff');
    // A panel op counts even when it changes nothing.
    await surfaceLocal(app, native, 'hide', '', '');
    await surfaceLocal(app, native, 'hide', '', '');
    expect(state.userRevision).toBe(2);
  });
  test('the offered turn is saved with the panel, also with no surface, and read back after a relaunch', () => {
    const state = { ...empty(), proactiveDiffTurnId: 'r1' };
    expect(savedPanel(state)).toEqual({ visible: false, active: '', surfaces: [], proactiveDiffTurnId: 'r1' });
    expect(savedPanel(empty())).toBeNull();
    const owner = { local: {} as object };
    expect(syncRightPanels(owner, new Map([['env:t1', state]]))).toBe(true);
    const local = {};
    adoptRightPanels(local, owner.local as Record<string, never>);
    const restored = empty();
    expect(restoreRightPanel({ local, preferencesLoaded: true, ready: true }, 'env:t1', restored)).toBe(false);
    expect([restored.proactiveDiffTurnId, restored.visible, restored.surfaces]).toEqual(['r1', false, []]);
    // With surfaces too.
    const withDiff = { ...empty(), surfaces: [file('a.ts')], active: 'file:a.ts', visible: true, proactiveDiffTurnId: 'r2' };
    expect(savedPanel(withDiff)?.proactiveDiffTurnId).toBe('r2');
  });
});
