// The right panel opens on its own (MIT reference 74541bab17, see LICENSE-T3:
// apps/web/src/hooks/useProactivePanels.ts, components/ChatView.logic.ts shouldOpenProactivePullRequest,
// observeProactivePanelUserChoice, shouldRetargetThreadPullRequestPanel, shouldOpenProactiveTurnDiff and
// resolveProactiveTurnDiffAction (at 2359643cff, with threadCheckpoints.ts checkpointFileTotals), rightPanelStore.ts
// openProactive and `proactiveDiffTurnId`, and the inputs ChatView.tsx hands the hook). With Settings › General › Proactive panels on, a thread's linked pull requests
// open first; otherwise a completed turn that touched at least 3 files or 50 lines opens the Changes diff. A panel
// the person opened or closed during the turn wins (PanelState.userRevision is rightPanelStore's
// userActionRevision). An open panel showing the thread's linked pull request follows it to a new link with the
// setting off too. panelView runs this on every shell answer, as the hook's effect runs on every render.
import type { T3Client } from '../client/client';
import { arr, checkpointFileTotals, num, obj, str, type CheckpointFileTotals, type Obj } from '../client/domain';
import { gitRepoState } from '../diff/diff';
import type { PanelState, Surface } from './panel-surfaces';
import { prSurfaceId, type PrTarget } from './pr-surface';
import { visiblePullRequests } from './shell-pr';

/** ThreadLinkedPullRequest: the thread's persisted link. */
export type LinkedPullRequest = { projectId: string; repository: string; number: number; url: string };
export type ProactiveObservation = {
  threadKey: string; runningTurnId: string | null | undefined; targetKey: string | null | undefined;
  userActionTurnId: string | null; userActionRevision: number;
};
/** A turn's checkpoint; `filesTotal` covers every file when `files` is only a prefix (checkpointFileTotals). */
export type TurnCheckpoint = { runId: string; status: string; files: { additions: number; deletions: number }[]; filesTotal?: CheckpointFileTotals };
export type ProactiveRequest = { kind: 'diff'; turnId: string } | { kind: 'pull-requests' } | { kind: 'pull-request'; target: PrTarget };
export type ProactiveInput = {
  /** The thread's panel key; null for drafts and while the thread is unavailable. */
  threadKey: string | null;
  enabled: boolean; settingsHydrated: boolean; threadDetailLoading: boolean;
  runningTurnId: string | null; latestRun: { runId: string; status: string } | null; latestRunSettled: boolean;
  turnDiffSummaries: TurnCheckpoint[]; isGitRepo: boolean | undefined;
  linkedPullRequest: LinkedPullRequest | null; hasLinkedPullRequestDetail: boolean;
  /** Identifies the change requests the thread shows, so a new one can open the panel. */
  pullRequestsKey: string | null;
  pullRequestsCapabilityKnown: boolean; supportsPullRequests: boolean; pullRequestsSurfaceAvailable: boolean; visiblePullRequestCount: number;
};

export function shouldOpenProactivePullRequest(previousTargetKey: string | null | undefined, targetKey: string | null): boolean {
  return targetKey !== null && targetKey !== previousTargetKey;
}

export function observeProactivePanelUserChoice(previous: ProactiveObservation | null,
  input: { threadKey: string; runningTurnId: string | null; userActionRevision: number }): ProactiveObservation {
  const sameThread = previous?.threadKey === input.threadKey;
  const newTurn = sameThread && input.runningTurnId !== null && input.runningTurnId !== previous.userActionTurnId;
  return {
    threadKey: input.threadKey,
    runningTurnId: sameThread ? previous.runningTurnId : undefined,
    targetKey: sameThread ? previous.targetKey : undefined,
    userActionTurnId: input.runningTurnId ?? (sameThread ? previous.userActionTurnId : null),
    userActionRevision: !sameThread || newTurn ? input.userActionRevision : previous.userActionRevision,
  };
}

/** The open surface showed the previous link, and the link changed: the panel follows it. */
export function shouldRetargetThreadPullRequestPanel(previous: LinkedPullRequest | null, current: LinkedPullRequest | null, surface: Surface | null): boolean {
  if (previous === null || current === null || surface?.kind !== 'pull-request' || !surface.pr) return false;
  const previousRepository = previous.repository.toLowerCase();
  return (previous.projectId !== current.projectId || previousRepository !== current.repository.toLowerCase() || previous.number !== current.number)
    && surface.pr.projectId === previous.projectId && surface.pr.repository.toLowerCase() === previousRepository && surface.pr.number === previous.number;
}

export function shouldOpenProactiveTurnDiff(input: { previousRunningTurnId: string | null | undefined; runningTurnId: string | null; settledTurnId: string | null; turnCompleted: boolean }): boolean {
  return input.runningTurnId === null && input.turnCompleted && input.settledTurnId !== null
    && (input.previousRunningTurnId === undefined || input.settledTurnId === input.previousRunningTurnId);
}

export function resolveProactiveTurnDiffAction(input: { checkpoint: Pick<TurnCheckpoint, 'status' | 'files' | 'filesTotal'> | undefined; isGitRepo: boolean | undefined }): 'defer' | 'ignore' | 'open' {
  if (input.checkpoint === undefined || input.checkpoint.status === 'missing') return 'defer';
  if (input.isGitRepo === undefined) return 'defer';
  if (!input.isGitRepo || input.checkpoint.status !== 'ready' || input.checkpoint.files.length === 0) return 'ignore';
  const totals = input.checkpoint.filesTotal ?? { count: input.checkpoint.files.length,
    additions: input.checkpoint.files.reduce((total, file) => total + file.additions, 0), deletions: input.checkpoint.files.reduce((total, file) => total + file.deletions, 0) };
  return totals.count >= 3 || totals.additions + totals.deletions >= 50 ? 'open' : 'ignore';
}

const surfaceOf = (request: Exclude<ProactiveRequest, { kind: 'diff' }>): Surface => request.kind === 'pull-requests'
  ? { id: 'pull-requests', kind: 'pull-requests', path: '', line: 0, reveal: 0 }
  : { id: prSurfaceId(request.target), kind: 'pull-request', path: '', line: 0, reveal: 0, pr: request.target };
function upsert(state: PanelState, surface: Surface): void {
  if (!state.surfaces.some(entry => entry.id === surface.id)) state.surfaces.push(surface);
  state.active = surface.id; state.visible = true;
}
const activeKind = (state: PanelState): string | null => state.visible ? state.surfaces.find(entry => entry.id === state.active)?.kind ?? null : null;

/**
 * rightPanelStore.openProactive: refused when the person made a panel choice after `expected` was read. A turn's diff
 * is offered once: the offer is recorded even when refused, so a revisit does not retry that turn, and it does not
 * take the panel from a Diff or pull request already showing. An existing pull request tab stays where the person
 * left it unless `activateExisting` (a retarget) asks for it.
 */
export function openProactive(state: PanelState, request: ProactiveRequest, expected: number, activateExisting = false): boolean {
  if (request.kind === 'diff') {
    if (state.proactiveDiffTurnId === request.turnId) return false;
    const active = activeKind(state);
    const opened = state.userRevision === expected && active !== 'diff' && active !== 'pull-request' && active !== 'pull-requests';
    if (opened) upsert(state, { id: 'diff', kind: 'diff', path: '', line: 0, reveal: 0 });
    state.proactiveDiffTurnId = request.turnId;
    return opened;
  }
  if (state.userRevision !== expected) return false;
  const surface = surfaceOf(request);
  if (!activateExisting && state.surfaces.some(entry => entry.id === surface.id)) return false;
  upsert(state, surface);
  return true;
}

const linkedTarget = (link: LinkedPullRequest): ProactiveRequest => ({ kind: 'pull-request', target: { projectId: link.projectId, host: '', repository: link.repository, number: link.number, url: link.url } });

/** useProactivePanels' memory: the observation (a ref in ChatView) and the last link seen per thread (until relaunch). */
export type ProactiveMemory = { observation: ProactiveObservation | null; pullRequests: Map<string, LinkedPullRequest | null> };
export const proactiveMemory = (): ProactiveMemory => ({ observation: null, pullRequests: new Map() });

/** One run of the hook's effect over the thread's panel. Returns the kind of surface it opened, '' for none. */
export function proactivePanels(memory: ProactiveMemory, state: PanelState, input: ProactiveInput): '' | 'diff' | 'pull-request' | 'pull-requests' {
  const { threadKey } = input;
  if (threadKey === null) { memory.observation = null; return ''; }
  let opened: '' | 'diff' | 'pull-request' | 'pull-requests' = '';
  const observation = observeProactivePanelUserChoice(memory.observation, { threadKey, runningTurnId: input.runningTurnId, userActionRevision: state.userRevision });
  memory.observation = observation;
  const { runningTurnId: previousRunningTurnId, targetKey: previousTargetKey, userActionRevision } = observation;
  const openSurface = state.visible ? state.surfaces.find(entry => entry.id === state.active) ?? null : null;
  const previousPullRequest = memory.pullRequests.get(threadKey) ?? null;
  // A missing link during loading must not erase the last observed one.
  if (input.linkedPullRequest !== null || !input.threadDetailLoading) memory.pullRequests.set(threadKey, input.linkedPullRequest);
  const followSelectedPullRequest = shouldRetargetThreadPullRequestPanel(previousPullRequest, input.linkedPullRequest, openSurface);
  if (followSelectedPullRequest && input.linkedPullRequest !== null && openProactive(state, linkedTarget(input.linkedPullRequest), userActionRevision, true)) opened = 'pull-request';
  if (!input.settingsHydrated) return opened;

  const eligibleLink = input.enabled && shouldOpenProactivePullRequest(previousTargetKey, input.pullRequestsKey);
  const shouldDeferLink = eligibleLink && !input.pullRequestsCapabilityKnown;
  memory.observation = { ...observation, targetKey: shouldDeferLink ? (previousTargetKey ?? null) : input.pullRequestsKey };
  if (eligibleLink && input.pullRequestsCapabilityKnown && !followSelectedPullRequest) {
    if (input.pullRequestsSurfaceAvailable && (input.visiblePullRequestCount > 1 || !input.hasLinkedPullRequestDetail || !input.supportsPullRequests)) {
      if (openProactive(state, { kind: 'pull-requests' }, userActionRevision)) opened = 'pull-requests';
    } else if (input.supportsPullRequests && input.linkedPullRequest !== null) {
      if (openProactive(state, linkedTarget(input.linkedPullRequest), userActionRevision)) opened = 'pull-request';
    }
  }
  if (input.threadDetailLoading) return opened;

  const settledTurnId = input.latestRunSettled ? (input.latestRun?.runId ?? null) : null;
  const newlyCompletedTurnId = shouldOpenProactiveTurnDiff({ previousRunningTurnId, runningTurnId: input.runningTurnId, settledTurnId, turnCompleted: input.latestRun?.status === 'completed' }) ? settledTurnId : null;
  const eligibleCompletion = input.enabled && newlyCompletedTurnId !== null
    && !(input.pullRequestsKey !== null && (!input.pullRequestsCapabilityKnown || input.supportsPullRequests || input.pullRequestsSurfaceAvailable));
  const diffAction = eligibleCompletion
    ? resolveProactiveTurnDiffAction({ checkpoint: input.turnDiffSummaries.find(checkpoint => checkpoint.runId === newlyCompletedTurnId), isGitRepo: input.isGitRepo })
    : 'ignore';
  // Preserve first-entry eligibility while capabilities, checkpoint or repository load.
  memory.observation = { ...memory.observation, runningTurnId: diffAction === 'defer' || shouldDeferLink ? previousRunningTurnId : input.runningTurnId };
  if (diffAction !== 'open' || newlyCompletedTurnId === null) return opened;
  return openProactive(state, { kind: 'diff', turnId: newlyCompletedTurnId }, userActionRevision) ? 'diff' : opened;
}

// ── ChatView's inputs, read from the client ─────────────────────────────────

const INTERRUPTIBLE = new Set(['preparing', 'starting', 'running']);
const UNSETTLED = new Set(['preparing', 'queued', 'starting', 'running', 'waiting']);
function latest(runs: Obj[], keep: (run: Obj) => boolean): Obj | null {
  let best: Obj | null = null, bestOrdinal = -Infinity;
  for (let index = 0; index < runs.length; index++) {
    const run = runs[index]!, ordinal = typeof run.ordinal === 'number' ? num(run.ordinal) : index;
    if (keep(run) && ordinal > bestOrdinal) { best = run; bestOrdinal = ordinal; }
  }
  return best;
}
function linkedOf(value: unknown): LinkedPullRequest | null {
  const link = value && typeof value === 'object' ? obj(value) : null;
  return link && str(link.projectId) && str(link.repository) && num(link.number) > 0
    ? { projectId: str(link.projectId), repository: str(link.repository), number: num(link.number), url: str(link.url) } : null;
}

/**
 * The hook's inputs as ChatView derives them: deriveThreadRuntime's activeRunId, deriveLatestThreadRun (the newest
 * run not held in the queue, without the usage-limit presentation, which only ever presents a failed run),
 * isLatestRunSettled, the checkpoint summaries, the repository state, the linked pull request and the pull request
 * capabilities. `sheet` is the narrow window, where the reference passes `enabled` false (shouldUsePlanSidebarSheet).
 */
export function proactiveInput(client: T3Client, threadKey: string, sheet: boolean): ProactiveInput {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId) ?? null;
  const capabilities = obj(obj(obj(client.config).environment).capabilities);
  const projection = client.thread ? client.projection : {};
  const runs = arr(projection.runs);
  const runningTurnId = str(latest(runs, run => INTERRUPTIBLE.has(str(run.status)))?.id) || null;
  const run = latest(runs, entry => !(entry.status === 'queued' && entry.queueHeld === true));
  const latestRun = run ? { runId: str(run.id), status: str(run.status) } : null;
  const visible = thread ? visiblePullRequests(thread.pullRequests) : [];
  const pullRequestsSurfaceAvailable = !!thread && capabilities.threadPullRequests === true && visible.length > 0;
  const linkedPullRequest = linkedOf(thread?.linkedPullRequest ?? obj(projection.thread).linkedPullRequest);
  const linkedKey = linkedPullRequest ? JSON.stringify([linkedPullRequest.projectId, linkedPullRequest.repository, linkedPullRequest.number]) : null;
  return {
    threadKey: thread ? threadKey : null,
    enabled: obj((client.local as { clientSettings?: Obj }).clientSettings).proactivePanelsEnabled === true && !sheet,
    settingsHydrated: client.preferencesLoaded,
    threadDetailLoading: !!thread && !client.thread,
    runningTurnId, latestRun,
    latestRunSettled: latestRun !== null && !UNSETTLED.has(latestRun.status) && runningTurnId !== latestRun.runId,
    turnDiffSummaries: arr(projection.checkpoints).flatMap(checkpoint => {
      if (typeof checkpoint.runId !== 'string' || typeof checkpoint.appRunOrdinal !== 'number') return [];
      const filesTotal = checkpointFileTotals(checkpoint.filesTotal);
      return [{ runId: checkpoint.runId, status: str(checkpoint.status), files: arr(checkpoint.files).map(file => ({ additions: num(file.additions), deletions: num(file.deletions) })),
        ...(filesTotal ? { filesTotal } : {}) }];
    }),
    isGitRepo: client.thread ? gitRepoState(client) : undefined,
    linkedPullRequest, hasLinkedPullRequestDetail: linkedPullRequest !== null,
    pullRequestsKey: pullRequestsSurfaceAvailable ? JSON.stringify(visible.map(link => [str(link.host), str(link.repository), num(link.number)]).sort()) : linkedKey,
    pullRequestsCapabilityKnown: Object.keys(obj(client.config)).length > 0,
    supportsPullRequests: capabilities.pullRequests === true,
    pullRequestsSurfaceAvailable, visiblePullRequestCount: visible.length,
  };
}

const memories = new WeakMap<T3Client, ProactiveMemory>();
/**
 * useProactivePanels for the client's open thread, over its panel. The hook lives in ChatView, which Settings, a page
 * and the welcome wizard unmount: while the chat view is not shown nothing runs, and the observation starts afresh on
 * return (a first entry), keeping only the last link seen per thread.
 */
export function applyProactivePanels(client: T3Client, threadKey: string, state: PanelState, sheet: boolean, chatShown = true): '' | 'diff' | 'pull-request' | 'pull-requests' {
  let memory = memories.get(client);
  if (!memory) { memory = proactiveMemory(); memories.set(client, memory); }
  if (!chatShown) { memory.observation = null; return ''; }
  return proactivePanels(memory, state, proactiveInput(client, threadKey, sheet));
}
