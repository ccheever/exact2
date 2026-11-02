// Screen reader announcements (MIT reference, see LICENSE-T3; T3 Code 7c3b3b3605ef: apps/web/src/lib/announce.ts,
// hooks/useLiveRegion.ts, components/chat/threadActivityAnnouncement.ts and ThreadActivityAnnouncer.tsx,
// components/backgroundThreadAnnouncement.ts and BackgroundThreadAnnouncer.tsx).
//
// The polite live regions the window draws (live-regions.contract): the open thread's (it starts, needs you,
// finishes), the background threads' ("Done: <title>"), and `announce`'s pair for what an action did when focus alone
// does not show it. The host speaks a region whose text changes (Accessibility.swift liveSpeaks), so a region exists
// before its first message, and a repeat gets a trailing no-break space to count as new. The thread regions clear 7 s
// after they speak; `announce`'s messages wait for the focus to settle and are then spoken together, alternating
// between two regions so the same message can be spoken twice in a row, and the pair keeps its text until the next.
import type { T3Client } from '../client/client';
import { arr, isListedThreadLineage, obj, str, type Obj } from '../client/domain';
import { activeRun } from '../client/protocol';
import { pendingRequests } from '../composer/requests';
import { latestExecutedRun } from '../composer/composer-controls';
import { activityRun, latestUnheldRun } from '../timeline/worktree-card-run';
import { effectiveSnoozed, latestRun, pendingApproval, pendingInput, shellRuntime } from '../sidebar/sidebar-model';
import { pendingSubagentRequest } from '../sidebar/subagent-requests';
import { isProviderSubagent } from '../composer/composer-controls-subagent';
import { runlessWorkStartedAt } from '../timeline/timeline-inspect';
import { notifyEnvironments } from './shell-notify';
import type { EnvironmentFleet } from '../connections/settings-b-fleet';

// ── threadActivityAnnouncement.ts ─────────────────────────────────────────────────────────────────────────────────

export type ThreadActivityStatus = 'working' | 'approval' | 'input' | 'idle';
/** Background threads say these too, so a status sounds the same wherever it's heard. */
export const THREAD_STATUS_TEXT = { approval: 'Needs approval', input: 'Needs your input', done: 'Done', failed: 'Failed', stopped: 'Stopped' } as const;
export type ThreadActivitySnapshot = { status: ThreadActivityStatus; requestId: string | null; runId: string | null; runStatus: string | null };
export type ThreadAnnouncementMemory = {
  snapshot: ThreadActivitySnapshot;
  /** The turn under way: the run it belongs to, "unknown" until that run shows up, or null between turns. */
  turnRunId: string | null;
  /** The run whose ending is already known, so a stale run is never announced as this turn's. */
  settledRunId: string | null;
  /** The run shown while idle belongs to the last turn, even if it is still settling. */
  idleRunId: string | null;
};
const ACTIVE_RUN_STATUSES = new Set<string | null>(['preparing', 'starting', 'running', 'waiting']);
const STARTING_RUN_STATUSES = new Set<string | null>(['queued', 'preparing', 'starting']);
const UNKNOWN = '\u0000unknown';

export function runOutcomeText(status: string | null): string | null {
  return status === 'completed' ? THREAD_STATUS_TEXT.done : status === 'failed' ? THREAD_STATUS_TEXT.failed
    : status === 'interrupted' || status === 'cancelled' ? THREAD_STATUS_TEXT.stopped : null;
}

type RunState = { runId: string; status: string };
/** The thread's detail has not caught up with its live shell: a frame read as the baseline then would replay history. */
export function isDetailBehindShell(detailRun: RunState | null, shellRun: RunState | null): boolean {
  return shellRun !== null && (detailRun?.runId !== shellRun.runId || detailRun.status !== shellRun.status);
}

/** A queued message cancelled before it started is no run: the turn ended however the last run that did run. */
export function announcedRun(activity: (RunState & { startedAt: string | null }) | null, executed: RunState | null): RunState | null {
  if (activity?.status !== 'cancelled' || activity.startedAt !== null) return activity;
  return executed;
}

/**
 * Decides what a screen reader hears when the open thread's turn changes state. The first snapshot of a thread is a
 * silent baseline, so opening or switching threads never replays what already happened.
 */
export function nextThreadAnnouncement(previous: ThreadAnnouncementMemory | null, next: ThreadActivitySnapshot): { memory: ThreadAnnouncementMemory; announcement: string | null } {
  const idle = next.status === 'idle';
  // A run still starting up while the thread looks idle (a queued message, a resume) is the next turn's.
  const idleRunId = idle && !STARTING_RUN_STATUSES.has(next.runStatus) ? next.runId : (previous?.idleRunId ?? null);
  // A run doing new work; the run left over from the last turn doesn't count.
  const newRunId = !idle && ACTIVE_RUN_STATUSES.has(next.runStatus) && next.runId !== idleRunId ? next.runId : null;
  if (previous === null) {
    return { memory: { snapshot: next, turnRunId: idle ? null : (newRunId ?? UNKNOWN), settledRunId: runOutcomeText(next.runStatus) === null ? null : next.runId, idleRunId }, announcement: null };
  }
  const memory = { ...previous, snapshot: next, idleRunId };
  const { turnRunId } = previous;
  if (!idle) {
    // Sending can look idle for a moment before the server reports the run, so a turn is tied to its run as soon as
    // one appears, and only another run taking over (a queued message starting) is a new turn.
    const turnStarted = turnRunId === null || (newRunId !== null && turnRunId !== UNKNOWN && newRunId !== turnRunId);
    const nextTurnRunId = turnStarted || turnRunId === UNKNOWN ? (newRunId ?? UNKNOWN) : turnRunId;
    const announcement = next.status === 'approval'
      ? previous.snapshot.status === 'approval' && previous.snapshot.requestId === next.requestId ? null : THREAD_STATUS_TEXT.approval
      : next.status === 'input'
        ? previous.snapshot.status === 'input' && previous.snapshot.requestId === next.requestId ? null : THREAD_STATUS_TEXT.input
        : turnStarted ? 'Working' : null;
    return { memory: { ...memory, turnRunId: nextTurnRunId }, announcement };
  }
  // Work can stop a moment before the run settles; the ending waits for the run.
  const outcome = runOutcomeText(next.runStatus);
  if (turnRunId === null) {
    // A run that settled unseen is history too, or the next failed send would claim it.
    return { memory: { ...memory, settledRunId: outcome === null ? previous.settledRunId : next.runId }, announcement: null };
  }
  if (next.runStatus === 'rolled_back') return { memory: { ...memory, turnRunId: null, settledRunId: next.runId }, announcement: null };
  if (outcome === null) return { memory, announcement: null };
  // Work stopped without a new run, e.g. a send that failed; the retry is a new turn.
  if (next.runId === previous.settledRunId) return { memory: { ...memory, turnRunId: null }, announcement: null };
  // The last turn's run settling while this turn waits for its own run is not this turn's ending.
  if (turnRunId === UNKNOWN && next.runId === previous.idleRunId) return { memory: { ...memory, settledRunId: next.runId }, announcement: null };
  return { memory: { ...memory, turnRunId: null, settledRunId: next.runId }, announcement: outcome };
}

// ── backgroundThreadAnnouncement.ts ───────────────────────────────────────────────────────────────────────────────

export type BackgroundThreadSnapshot = {
  key: string; title: string; request: 'approval' | 'input' | null;
  /** Null for a subagent's request, which the parent's shell only knows by kind. */
  requestId: string | null;
  runId: string | null; runStatus: string | null;
  /** The run doing work now, which can be older than a queued run cancelled while it ran. */
  activeRunId: string | null;
  /** A snoozed thread stays hidden until its timer, so it stays quiet too. */
  snoozed: boolean;
};
type BackgroundThreadMemoryEntry = { request: BackgroundThreadSnapshot['request']; requestId: string | null; settledRunId: string | null };
export type BackgroundThreadMemory = ReadonlyMap<string, BackgroundThreadMemoryEntry>;

/** The request and its id come from the same source, so a subagent's request never takes the parent's id. */
export function resolveBackgroundThreadRequest(thread: Obj): Pick<BackgroundThreadSnapshot, 'request' | 'requestId'> {
  const own = thread.pendingRuntimeRequest == null ? null : str(obj(thread.pendingRuntimeRequest).id) || null, subagent = pendingSubagentRequest(thread);
  if (pendingApproval(thread)) return { request: 'approval', requestId: own };
  if (subagent === 'approval') return { request: 'approval', requestId: null };
  if (pendingInput(thread)) return { request: 'input', requestId: own };
  if (subagent === 'user_input') return { request: 'input', requestId: null };
  return { request: null, requestId: null };
}

/** First sightings are a silent baseline, so opening the app or reconnecting does not read out every thread. */
export function nextBackgroundThreadAnnouncement(previous: BackgroundThreadMemory, threads: ReadonlyArray<BackgroundThreadSnapshot>, openThreadKey: string | null): { memory: BackgroundThreadMemory; announcement: string | null } {
  const memory = new Map<string, BackgroundThreadMemoryEntry>(), announcements: string[] = [];
  for (const thread of threads) {
    const before = previous.get(thread.key), outcome = runOutcomeText(thread.runStatus);
    const settled = outcome !== null || thread.runStatus === 'rolled_back';
    memory.set(thread.key, { request: thread.request, requestId: thread.requestId, settledRunId: settled ? thread.runId : (before?.settledRunId ?? null) });
    if (before === undefined || thread.key === openThreadKey || thread.snoozed) continue;
    if (thread.request !== null && (thread.request !== before.request || thread.requestId !== before.requestId)) announcements.push(`${THREAD_STATUS_TEXT[thread.request]}: ${thread.title}`);
    // While another run still works, the latest run's ending (a queued message cancelled before it started) isn't the
    // thread finishing. The shell then only reports the latest run, so the working run's own ending stays quiet.
    const anotherRunWorking = thread.activeRunId !== null && thread.activeRunId !== thread.runId;
    if (outcome !== null && thread.runId !== before.settledRunId && !anotherRunWorking) announcements.push(`${outcome}: ${thread.title}`);
  }
  return { memory, announcement: announcements.length === 0 ? null : announcements.join('. ') };
}

// ── The window's regions ──────────────────────────────────────────────────────────────────────────────────────────

/** What the window's four regions show (live-regions.contract). */
export type AnnounceView = { thread: string; background: string; first: string; second: string };
/**
 * A region's text: `seen` is the shell's clock at the answer that wrote it, `at` the first later reading of the clock
 * (0 until then). The shell's clock reads the time only when it ticks, so an answer a command brought can carry an old
 * reading; the first tick after a write is when it was spoken.
 */
type Region = { text: string; seen: number; at: number };
/** A waiting message: `seen` as a region's (-1 until an answer sees it). */
type Pending = { message: string; source: string | undefined; seen: number };
type Announcer = {
  thread: Region; background: Region; first: Region; second: Region; pending: Pending[];
  open: { threadKey: string; memory: ThreadAnnouncementMemory } | null;
  backgroundMemory: BackgroundThreadMemory;
  /** The open thread's send that the server has not acknowledged yet (useLocalDispatchState). */
  dispatch: LocalDispatch | null;
};
/** useLiveRegion's CLEAR_AFTER_MS: a thread region's text is cleared once it has had time to be spoken. */
export const CLEAR_AFTER_MS = 7_000;
const announcers = new WeakMap<T3Client, Announcer>();
function announcerOf(client: T3Client): Announcer {
  let value = announcers.get(client);
  if (!value) {
    const region = (): Region => ({ text: '', seen: 0, at: 0 });
    value = { thread: region(), background: region(), first: region(), second: region(), pending: [], open: null, backgroundMemory: new Map(), dispatch: null };
    announcers.set(client, value);
  }
  return value;
}
const NBSP = '\u00a0';
/** useLiveRegion's announce: a repeat of the region's text gets a no-break space, so it changes and is spoken. */
function speak(region: Region, text: string, now: number): void {
  region.text = region.text === text ? `${text}${NBSP}` : text;
  region.seen = now;
  region.at = 0;
}

/**
 * Tells screen reader users what an action did when focus alone doesn't show it. A message replaces one still waiting
 * from the same `source`, so a superseded status isn't read out.
 */
export function announce(client: T3Client, message: string, source?: string): void {
  const state = announcerOf(client);
  if (source !== undefined) state.pending = state.pending.filter(entry => entry.source !== source);
  if (state.pending.at(-1)?.message !== message) state.pending.push({ message, source, seen: -1 });
  // A new message restarts the settle for every waiting one, as scheduleFlush's timer does.
  for (const entry of state.pending) entry.seen = -1;
}
/** Drops a message from `source` that hasn't been spoken yet, once it no longer applies. */
export function cancelAnnouncement(client: T3Client, source: string): void {
  const state = announcerOf(client);
  state.pending = state.pending.filter(entry => entry.source !== source);
}

// ── useLocalDispatchState (ChatView.logic.ts createLocalDispatchSnapshot, hasServerAcknowledgedLocalDispatch) ──────

/** The open thread as its send found it: a send stays in flight until the server shows it took it. */
type DispatchFacts = { userMessageId: string | null; runId: string | null; requestedAt: string | null; startedAt: string | null;
  completedAt: string | null; runtimeStatus: string | null; runtimeUpdatedAt: string | null; activeRunId: string | null };
type LocalDispatch = DispatchFacts & { threadKey: string; error: string };
function dispatchFacts(client: T3Client, shellThread: Obj | undefined): DispatchFacts {
  const run = shellThread ? latestRun(shellThread) : null, runtime = shellThread ? shellRuntime(shellThread) : null;
  const userMessage = client.threadId ? arr(client.projection.messages).filter(message => message.role === 'user').at(-1) : undefined;
  return {
    userMessageId: userMessage ? str(userMessage.id) : null, runId: shellThread?.latestRunId == null ? null : str(shellThread.latestRunId),
    requestedAt: run?.requestedAt ?? null, startedAt: run?.startedAt ?? null, completedAt: run?.completedAt ?? null,
    runtimeStatus: runtime?.status ?? null, runtimeUpdatedAt: runtime?.updatedAt ?? null, activeRunId: runtime?.activeRunId || null,
  };
}
/** derivePhase over a runtime status. */
const phaseOf = (status: string | null): string => status === null ? 'disconnected'
  : ['preparing', 'starting', 'queued'].includes(status) ? 'connecting' : ['running', 'waiting'].includes(status) ? 'running' : 'ready';
/** hasServerAcknowledgedLocalDispatch: a request or an error ends it; otherwise the thread's run or runtime must move. */
function acknowledged(sent: LocalDispatch, now: DispatchFacts, waitingOnPerson: boolean, error: string): boolean {
  if (waitingOnPerson || (error !== '' && error !== sent.error)) return true;
  const phase = phaseOf(now.runtimeStatus);
  if (phase === 'connecting') return false;
  const userMessageChanged = sent.userMessageId !== now.userMessageId;
  const runChanged = sent.runId !== now.runId || sent.requestedAt !== now.requestedAt || sent.startedAt !== now.startedAt || sent.completedAt !== now.completedAt;
  if (phase === 'running') {
    if (userMessageChanged) return true;
    if (!runChanged || now.runId === null || now.startedAt === null) return false;
    return now.activeRunId === null || now.runId === now.activeRunId;
  }
  return runChanged || sent.runtimeStatus !== now.runtimeStatus || sent.runtimeUpdatedAt !== now.runtimeUpdatedAt;
}
/**
 * The open thread's own send whose request is out (beginLocalDispatch's callers): a message, a draft's launch, an
 * unstarted server thread's first message into a new worktree (a launch that reuses the thread), or Implement in a new
 * thread (a launch from the open thread). Another write in flight (a model, a mode, a stop) is not a send.
 */
function sendRequested(client: T3Client, launching: string): boolean {
  const pending = client.pending;
  if (!client.busy || !pending) return false;
  const payload = obj(pending.payload);
  if (pending.method === 'orchestration.launchThread') return client.threadId ? payload.reuseExistingThread !== true || str(payload.threadId) === client.threadId : launching !== '';
  return !!client.threadId && pending.threadId === client.threadId && payload.type === 'message.dispatch';
}

/** The open thread's activity as ChatView passes it to the timeline's announcer; null while there is no thread to follow. */
function openThreadActivity(client: T3Client, state: Announcer): { threadKey: string; snapshot: ThreadActivitySnapshot; detailBehind: boolean } | null {
  if (!client.environmentId || !client.ready) return null;
  const projection = client.projection, shellThread = client.shell.threads.find(thread => thread.id === client.threadId);
  // A provider's own subagent thread has no turn of its own to announce (announceTurnStatus).
  if (isProviderSubagent(obj(projection.thread))) return null;
  // The draft's thread id up front, as the reference's drafts have one: a launch keeps its region's memory.
  const pending = client.pending, launching = !client.threadId && pending?.method === 'orchestration.launchThread' && pending.payload.reuseExistingThread !== true ? str(pending.payload.threadId) : '';
  const threadKey = `${client.environmentId}:${client.threadId || launching || `draft:${client.draftKey}`}`;
  const { approvals, inputs } = client.threadId ? pendingRequests(projection) : { approvals: [], inputs: [] };
  const waitingOnPerson = approvals.length > 0 || inputs.length > 0 || (shellThread ? pendingInput(shellThread) : false);
  // isSendBusy: from the send until the server acknowledges it, past the request's reply.
  const facts = dispatchFacts(client, shellThread);
  if (state.dispatch && state.dispatch.threadKey !== threadKey && !state.dispatch.threadKey.endsWith(`:draft:${client.draftKey}`)) state.dispatch = null;
  if (sendRequested(client, launching)) { if (!state.dispatch) state.dispatch = { ...facts, threadKey, error: client.error }; else state.dispatch.threadKey = threadKey; }
  else if (state.dispatch && acknowledged(state.dispatch, facts, waitingOnPerson, client.error)) state.dispatch = null;
  // ChatView's isWorking: a run, runless work, or this thread's send in flight.
  const working = !!activeRun(projection) || runlessWorkStartedAt(projection) !== '' || state.dispatch !== null;
  const status: ThreadActivityStatus = approvals.length ? 'approval' : inputs.length || (shellThread ? pendingInput(shellThread) : false) ? 'input' : working ? 'working' : 'idle';
  const requestId = approvals[0]?.requestId ?? inputs[0]?.requestId ?? null;
  const activity = client.threadId ? activityRun(projection) : null, executed = client.threadId ? latestExecutedRun(projection) : undefined;
  const run = announcedRun(activity ? { runId: str(activity.id), status: str(activity.status), startedAt: activity.startedAt == null ? null : str(activity.startedAt) } : null,
    executed ? { runId: str(executed.id), status: str(executed.status) } : null);
  const detail = client.threadId ? latestUnheldRun(projection) : null, shellRun = shellThread?.latestRunId != null ? latestRun(shellThread) : null;
  return {
    threadKey, snapshot: { status, requestId, runId: run?.runId ?? null, runStatus: run?.status ?? null },
    detailBehind: isDetailBehindShell(detail ? { runId: str(detail.id), status: str(detail.status) } : null, shellRun ? { runId: str(shellThread!.latestRunId), status: shellRun.status } : null),
  };
}

/** ThreadActivityAnnouncer: the open thread's region follows its activity; a switch clears it and starts a new baseline. */
function followOpenThread(client: T3Client, state: Announcer, now: number): void {
  const activity = openThreadActivity(client, state);
  if (!activity) {
    if (state.open) { state.open = null; state.thread.text = ''; }
    return;
  }
  if (activity.detailBehind) return;
  let previous = state.open?.threadKey === activity.threadKey ? state.open.memory : null;
  // A draft that launched keeps the memory of its draft: the reference's draft already had this thread's id.
  if (!previous && state.open && state.open.threadKey === `${client.environmentId}:draft:${client.draftKey}`) previous = state.open.memory;
  const { memory, announcement } = nextThreadAnnouncement(previous, activity.snapshot);
  if (previous === null) state.thread.text = '';
  else if (announcement !== null) speak(state.thread, announcement, now);
  state.open = { threadKey: activity.threadKey, memory };
}

/** startBackgroundThreadAnnouncements: every live environment's root threads but the open one. */
function followBackgroundThreads(client: T3Client, state: Announcer, now: number, source?: EnvironmentFleet): void {
  const snapshots: BackgroundThreadSnapshot[] = [];
  for (const environment of notifyEnvironments(client, source)) {
    if (!environment.live) continue;
    for (const thread of environment.shell.threads) {
      // Subagents and side chats have no row of their own; their requests surface on the parent.
      if (thread.archivedAt != null || !isListedThreadLineage(thread.lineage)) continue;
      const runtime = shellRuntime(thread);
      snapshots.push({
        key: `${environment.environmentId}:${str(thread.id)}`, title: str(thread.title), ...resolveBackgroundThreadRequest(thread),
        runId: thread.latestRunId == null ? null : str(thread.latestRunId), runStatus: latestRun(thread)?.status ?? null,
        activeRunId: runtime?.activeRunId || null, snoozed: effectiveSnoozed(thread, now),
      });
    }
  }
  const openThreadKey = client.environmentId && client.threadId ? `${client.environmentId}:${client.threadId}` : null;
  const next = nextBackgroundThreadAnnouncement(state.backgroundMemory, snapshots, openThreadKey);
  state.backgroundMemory = next.memory;
  if (next.announcement !== null) speak(state.background, next.announcement, now);
}

/**
 * One shell answer's turn of the announcers, then what the window's regions show. The shell's clock (`now`) does the
 * reference's timers: messages are spoken together at the first tick after the last one came in (the clock ticks
 * every 500 ms while `announcing`, so a focus that a command moved has settled by then), and a thread region is
 * cleared CLEAR_AFTER_MS after the tick that followed its write.
 */
export function announcements(client: T3Client, now: number, source?: EnvironmentFleet): AnnounceView {
  const state = announcerOf(client);
  followOpenThread(client, state, now);
  followBackgroundThreads(client, state, now, source);
  for (const entry of state.pending) if (entry.seen < 0) entry.seen = now;
  if (state.pending.length && state.pending.every(entry => now > entry.seen)) {
    // Alternating between two regions lets the same message be spoken twice in a row.
    const [target, other] = state.first.text ? [state.second, state.first] : [state.first, state.second];
    other.text = '';
    target.text = state.pending.map(entry => entry.message).join('. ');
    state.pending = [];
  }
  for (const region of [state.thread, state.background]) {
    if (!region.text) continue;
    if (!region.at && now > region.seen) region.at = now;
    if (region.at && now - region.at >= CLEAR_AFTER_MS) region.text = '';
  }
  return { thread: state.thread.text, background: state.background.text, first: state.first.text, second: state.second.text };
}
/** Whether a message waits or a thread region still shows text: the shell's clock must keep running to settle or clear it. */
export function announcing(client: T3Client): boolean {
  const state = announcerOf(client);
  return state.pending.length > 0 || state.thread.text !== '' || state.background.text !== '';
}
