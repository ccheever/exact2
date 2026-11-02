// What the thread is doing now and the turns queued behind it, as rows at the end of the timeline,
// adapted from T3 Code v0.1.5 (MIT); see LICENSE-T3: chat/ThreadNow.tsx, chat/ThreadActivityBanners.tsx
// (childInputNowItem, backgroundWorkNowItem, goalNowItem; runStopNowItem at 2359643cff), chat/QueuedRunsControl.tsx,
// threadSync.ts and ChatView.tsx (timelineFooter, scrollToEndSummary).
// threadEndRows is the timeline's footer and thread-end.contract draws its rows; the rows are
// `Message`s, so each field below names its role.
import { arr, obj, str, type Activity, type Message, type Obj } from '../client/domain';
import { activeRun } from '../client/protocol';
import type { T3Client } from '../client/client';
import { childThreadInputs, childThreadInputsTitle, childThreadInputsWaiting } from '../sidebar/subagent-requests';
import { pendingRequests } from './requests';
import { providerGoal, presentProviderGoal, providerGoalWorking } from './provider-goal';
import { presentBackgroundWork } from './composer-controls-view';
import { QUEUE_VISIBLE_ROWS, forgetQueueShownInFull, queueShownInFull, queueState, queuedEdit, queuedEditBlockedReason, queuedItems } from './composer-controls-queue';
import { chordGlyphs } from './composer-controls-model';
import { commandChords } from './composer-presentation';
import { probe, probeKey, type Probe } from './composer-menus';
import { anchorFrame } from '../shared/laid-out-widths';
import { clampInterfaceFontSize } from '../appearance/appearance-fonts';
import { userMessagePresentation } from '../timeline/user-message';
import { scheduledTaskTitle } from '../timeline/timeline-presentation';
import { canForceStop, forceStoppingThread, threadRunStop } from './run-stop';

/**
 * A line of thread state (`kind` "thread-now"): `icon` with `tone` "info" (or "warning", an unanswered Stop) on the one
 * line that needs you, `title`, then
 * `detail` (a plain description) or its `activities` with `status` "link" (an inline button opening `targetId`, named
 * `body` when set) or "text"; the `activities` with `status` "action" are its inline buttons (`detail` the op, `targetId`
 * its id, `tone` "muted" or "", `ok` false while disabled). `status` "alert" announces the line and "status" is the sync
 * line; `collapsible` says the text is cut, so the line offers its details.
 */
type Line = { id: string; icon: string; tone?: string; status?: string; title: string; detail?: string; segments?: Activity[]; actions: Activity[];
  /** A name after the title ("Running" then "npm run dev"), in the code font when `mono`, which sets the detail in it too. */
  label?: string; mono?: boolean;
  /** When the one background task started (epoch ms): the line ticks its elapsed time before the actions. */
  startedMs?: number };

const blank = { icon: '', output: '', result: '', failed: false, timestamp: '' };
const segment = (id: string, label: string, targetId = '', body = ''): Activity => ({ ...blank, id, label, body, status: targetId ? 'link' : 'text', detail: '', targetId, tone: 'muted' });
const action = (id: string, label: string, op: string, targetId: string, tone = '', ok = true): Activity => ({ ...blank, id, label, body: '', status: 'action', detail: op, targetId, tone, ok });

/** onOpenRelatedThread: a side chat of this thread opens in its tab (openSideChatTab), any other thread in the window. */
const openChild = (child: { sideChat: boolean }) => child.sideChat ? 'shelllocal:surface-side-chat-open' : 'select-thread';
/** childInputNowItem: subagents and side chats waiting on the user, unless this thread is itself waiting on them. */
function childInput(client: T3Client): Line | null {
  const children = childThreadInputs(client.shell.threads, client.threadId), first = children[0];
  const shell = client.shell.threads.find(thread => thread.id === client.threadId), requests = pendingRequests(client.projection);
  const parentAwaiting = requests.approvals.length > 0 || requests.inputs.length > 0 || obj(shell?.pendingRuntimeRequest).kind === 'user_input';
  if (!first || parentAwaiting) return null;
  return { id: `child-input:${first.id}`, icon: first.approval ? 'shield-question' : 'message-circle', tone: 'info', status: 'alert', title: childThreadInputsTitle(children),
    segments: children.map(child => ({ ...segment(child.id, child.title, child.id), detail: openChild(child) })),
    actions: [action('open', first.approval ? 'Open approval' : 'Open question', openChild(first), first.id)] };
}

const epoch = (value: unknown) => { const ms = typeof value === 'string' ? Date.parse(value) : NaN; return Number.isFinite(ms) ? ms : null; };
const oneLine = (value: unknown) => str(value).replace(/\s+/gu, ' ').trim();
/**
 * backgroundWorkNowItem: background commands and subagents once the turn is done. One task reads "Running <name>" (a
 * command's name in the code font), its subagent's newest progress after it, then its elapsed time and Open; several
 * read their count and the newest progress of any subagent, else their names. A terminal glyph when all are commands.
 */
function backgroundWork(client: T3Client): Line | null {
  if (activeRun(client.projection)) return null;
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  const presented = presentBackgroundWork(arr(shell?.pendingBackgroundTasks));
  if (!presented) return null;
  const turnItems = arr(client.projection.turnItems);
  const rows = presented.items.map(task => {
    // The key derivePendingBackgroundWork gives turn-item tasks; subagent items carry progress.
    const item = [...turnItems].reverse().find(candidate => (str(obj(candidate.nativeItemRef).nativeId) || str(candidate.id)) === task.taskId);
    const subagent = item?.type === 'subagent' ? item : undefined;
    return { task, activity: oneLine(subagent?.progress), startedMs: epoch(item?.startedAt), updatedMs: subagent ? epoch(subagent.updatedAt) ?? 0 : 0,
      childThreadId: task.childThreadId || str(subagent?.childThreadId) };
  });
  const only = rows.length === 1 ? rows[0] : undefined;
  const newest = rows.filter(row => row.activity).sort((left, right) => right.updatedMs - left.updatedMs)[0];
  const commands = rows.every(row => row.task.kind !== 'subagent');
  const stopping = client.busy && client.pending?.description === 'Stop background work';
  const canOperate = client.connection === 'connected' && client.scopes.includes('orchestration:operate');
  const names = rows.map(row => row.task.label).join(', ');
  return { id: `background-work:${client.threadId}`, icon: commands ? 'terminal' : 'circle-dashed',
    ...(only ? { title: presented.waiting ? 'Waiting on' : 'Running', label: only.task.label, mono: only.task.kind !== 'subagent', detail: only.activity }
      : { title: presented.title, detail: newest ? `${newest.task.label}: ${newest.activity}` : names, mono: !newest && commands }),
    ...(only?.startedMs != null ? { startedMs: only.startedMs } : {}),
    actions: [...(only?.childThreadId ? [action('open', 'Open', 'select-thread', only.childThreadId, 'muted')] : []),
      action('stop', stopping ? 'Stopping...' : 'Stop', 'cc:stop-background', '', '', canOperate && !stopping)] };
}

/**
 * runStopNowItem: a Stop the agent has not finished, with its elapsed time; once it stops answering, a warning with
 * Force stop.
 */
function runStop(client: T3Client): Line | null {
  const stop = threadRunStop(client);
  if (!stop) return null;
  const unresponsive = stop.state === 'unresponsive', forcing = forceStoppingThread(client) === client.threadId, requested = epoch(stop.requestedAt);
  return { id: `run-stop:${client.threadId}`, icon: unresponsive ? 'triangle-alert' : 'circle-stop', ...(unresponsive ? { tone: 'warning' } : {}),
    title: unresponsive ? 'Agent not responding to Stop' : 'Stopping', ...(unresponsive ? { detail: 'It may be short on memory. Force stop kills its processes.' } : {}),
    ...(requested !== null ? { startedMs: requested } : {}),
    actions: unresponsive && canForceStop(client) ? [action('force-stop', forcing ? 'Force stopping...' : 'Force stop', 'cc:force-stop', stop.runId, '', !forcing)] : [] };
}

/** goalNowItem: the thread's goal. Clear stops goal work; Resume would stop unrelated work, so it waits. */
function goal(client: T3Client): Line | null {
  const thread = client.shell.threads.find(thread => thread.id === client.threadId), current = providerGoal(thread?.goal);
  if (!current) return null;
  const working = providerGoalWorking(client), view = presentProviderGoal(current, working);
  const actions = [...(view.canResume && !working ? [action('resume', 'Resume', 'cc:goal-resume', '')] : []), action('clear', 'Clear', 'cc:goal-clear', '', 'muted')];
  return { id: `goal:${client.threadId}`, icon: 'target', title: view.usage ? `${view.title} · ${view.usage}` : view.title, detail: view.objective, actions };
}

const syncSeen = new Map<T3Client, string>();
/**
 * resolveThreadSyncPhase. useDelayedStatus holds a status back 400ms so a short
 * load never flashes; data sources have no clock here, so a phase shows once a
 * second snapshot still finds it (the next event batch after it began).
 */
export function syncStatus(client: T3Client): string {
  const phase = !client.threadId || client.connection !== 'connected' || !client.shell.threads.some(thread => thread.id === client.threadId) ? ''
    : !client.thread ? 'loading' : client.ready && !client.shellResuming && !client.threadResuming ? '' : 'syncing';
  const key = phase ? `${client.environmentId}:${client.threadId}:${phase}` : '';
  const shown = !!key && syncSeen.get(client) === key;
  syncSeen.set(client, key);
  return shown ? (phase === 'loading' ? 'Loading messages...' : 'Syncing messages...') : '';
}

/** ThreadNow's order: what needs you, background work, the sync status while messages load (`sync`), then the goal. */
function threadNowLines(client: T3Client, sync: string): Line[] {
  if (!client.threadId) return [];
  // A Stop that went unanswered needs the user; one in progress is the thread's work.
  const stop = runStop(client), unanswered = stop?.icon === 'triangle-alert';
  return [unanswered ? stop : childInput(client), stop && !unanswered ? stop : backgroundWork(client),
    sync ? { id: `thread-sync:${client.threadId}`, icon: 'refresh-cw', status: 'status', title: sync, actions: [] } : null, goal(client)].filter((line): line is Line => line !== null);
}

/** The texts a line lays out before its actions, each at its weight: links are medium, everything else regular. */
function lineTexts(line: Line): Array<[string, number]> {
  const described = !!line.detail || !!line.segments?.length;
  const parts: Array<[string, number]> = [[line.label ? `${line.title} ${line.label}` : line.title, 400], ...(described ? [[' ', 400] as [string, number]] : [])];
  if (line.detail) parts.push([line.detail, 400]);
  line.segments?.forEach((entry, index) => { parts.push([entry.label, entry.status === 'link' ? 500 : 400]); if (index < line.segments!.length - 1) parts.push([', ', 400]); });
  return parts;
}
/** The probes the lines measure by, laid out with the composer's (composer-presentation.ts); the short sync line needs none. */
export function threadNowProbes(client: T3Client): Probe[] {
  return threadNowLines(client, '').flatMap(line => lineTexts(line).map(([text, weight]) => probe(text, 12, weight)));
}

// NoticeText: the details button shows while the text is cut. It is left out of the room it takes, so it cannot keep
// itself shown once the text would fit.
const detailsShown = new WeakMap<T3Client, Set<string>>();
function cut(client: T3Client, line: Line, rootFontSize: number): boolean {
  const laidOut = anchorFrame(client.presentation, `tn:${line.id}`);
  const parts = lineTexts(line).map(([text, weight]) => anchorFrame(client.presentation, probeKey(text, 12, weight))?.[1] ?? 0);
  if (!laidOut || !parts.every(width => width > 0)) return false;
  const natural = parts.reduce((sum, width) => sum + width, 0) * rootFontSize / 16;
  return natural > laidOut[1] + (detailsShown.get(client)?.has(line.id) ? 28 * rootFontSize / 16 : 0) + 0.5;
}

/** The thread's present at the end of the timeline, as "thread-now" rows (see Line). Call it once a snapshot (syncStatus). */
export function threadNowRows(client: T3Client): Message[] {
  const shown = new Set<string>(), rootFontSize = clampInterfaceFontSize(client.local.clientSettings.fontSizeInterface);
  const rows = threadNowLines(client, syncStatus(client)).map((line): Message => {
    const details = cut(client, line, rootFontSize);
    if (details) shown.add(line.id);
    return { id: line.id, kind: 'thread-now', title: line.title, body: line.label ?? '', meta: line.mono === true, startedMs: line.startedMs ?? 0, icon: line.icon,
      tone: line.tone ?? '', status: line.status ?? '', detail: line.detail ?? '', activities: [...line.segments ?? [], ...line.actions], collapsible: details };
  });
  detailsShown.set(client, shown);
  return rows;
}

/**
 * QueuedMessage: who sent a queued message, as its sent form shows it. An automation's and another agent's fold to one
 * line under `sender` ("Automation" or the sending thread's title) with `summary` (the task's title or the first line);
 * T3 Code's says "Sent by T3 Code". `taskId` opens the automation, `senderThreadId` the sending thread.
 */
export function queuedSender(client: T3Client, message: Obj | undefined, text: string) {
  const presentation = message ? userMessagePresentation(message) : null, attribution = presentation?.attribution ?? '';
  const senderThreadId = attribution === 'agent' ? str(message?.senderThreadId) : '';
  const sender = attribution === 'automation' ? 'Automation'
    : attribution === 'agent' ? str(client.shell.threads.find(thread => thread.id === senderThreadId)?.title).trim() || 'Agent' : '';
  const summary = attribution === 'automation' ? scheduledTaskTitle(client, presentation!.scheduledTaskId) || (text.split(/\r?\n/, 1)[0] ?? '')
    : attribution === 'agent' ? text.split(/\r?\n/).find(line => line.trim()) ?? '' : '';
  return { attribution, sender, summary, taskId: presentation?.scheduledTaskId ?? '', senderThreadId };
}

/**
 * The queued turns as rows. A "queued" row is one dashed bubble: `body` its text in full, `images` its thumbnails,
 * `runId` ('' for a send still saving), `status` "editing", "pending" or "", `title` its accessible name; `actionLabel`
 * "Send now" while it can be sent, with `intentTip` that button's tooltip; `detail` the pencil's tooltip and `failed`
 * whether the pencil is disabled; `continues` another bubble follows and `collapsible` the "Show all" row follows;
 * `attribution` is the queue's announcement on the first row. Its sender (queuedSender): `intent` "automation", "agent"
 * or "t3code", `sender` and `summary` the fold's heading and line, `groupId` the automation's task, `sourceThreadId`
 * the sending thread, and `createdAt` the message's time. A "queued-more" row is that button: `title` its label,
 * `expanded` while the queue shows in full.
 */
export function queuedTurnRows(client: T3Client): Message[] {
  const items = queuedItems(client);
  if (!items.length) { forgetQueueShownInFull(client); return []; }
  const state = queueState(client.projection), edit = queuedEdit(client), blocked = queuedEditBlockedReason(client);
  // QueuedRunsControl's canSend: no active run (the message starts one), or one it can steer or a compaction it stops.
  const canSend = state.canSendNow || state.canSteer;
  const intent = state.canSendNow ? 'Start a turn with this message' : state.promoteSkipsCompaction ? 'Skip compaction and send this message' : 'Send into the current turn';
  const steerKey = chordGlyphs(commandChords(client.config, 'thread.steerQueuedMessage', 'Meta+Shift+Enter', false, { terminalFocus: false }).split(' ')[0] ?? '');
  const editKey = chordGlyphs(commandChords(client.config, 'thread.editQueuedMessage', 'Alt+ArrowUp', false, { composerFocus: true }).split(' ')[0] ?? '');
  const collapsible = items.length > QUEUE_VISIBLE_ROWS, full = queueShownInFull(client);
  const visible = collapsible && !full ? items.slice(0, QUEUE_VISIBLE_ROWS - 1) : items;
  const rows = visible.map((item, position): Message => {
    const editing = !!item.runId && item.runId === edit?.runId, from = queuedSender(client, item.message, item.text);
    return { intent: from.attribution, sender: from.sender, summary: from.summary, groupId: from.taskId, sourceThreadId: from.senderThreadId,
      ...(item.message ? { createdAt: str(item.message.createdAt) } : {}), id: `queued:${item.key}`, kind: 'queued', title: `Queued message ${position + 1} of ${items.length}, not sent`, body: item.text, runId: item.runId,
      images: item.thumbnails.map(thumb => ({ id: thumb.id, name: thumb.name, snapshot: false, appName: '', appInitial: '', windowTitle: '', accessible: true, video: false })),
      status: editing ? 'editing' : item.pending ? 'pending' : '',
      actionLabel: canSend && !editing ? 'Send now' : '',
      intentTip: `${intent}${item.serverIndex === 0 && steerKey ? ` (${steerKey})` : ''}`,
      detail: blocked || `Edit in the composer${item.serverIndex === state.queued.length - 1 && editKey ? ` (${editKey})` : ''}`,
      failed: !!blocked || !item.editable, continues: position < visible.length - 1, collapsible: collapsible && position === visible.length - 1,
      attribution: position === 0 ? `${items.length} queued` : '' };
  });
  if (collapsible) rows.push({ id: `queued-more:${client.threadId}`, kind: 'queued-more', title: full ? 'Show fewer' : `Show all ${items.length} queued`, body: '', expanded: full });
  return rows;
}

/**
 * The timeline's footer (ChatView timelineFooter): `before` (the pending handoff), what the thread is doing now, the
 * status line, then the queued turns. The timeline asks for it before the thread's detail arrives too: ThreadNow's sync
 * line says "Loading messages...", and the status line, which reads the shell, still shows ("Snoozed, 2h left"). Call
 * it once a snapshot (syncStatus).
 */
export function threadEndRows(client: T3Client, before: Message[], status: Message | null): Message[] {
  return [...before, ...threadNowRows(client), ...(status ? [status] : []), ...queuedTurnRows(client)];
}

/** scrollToEndSummary: what waits at the end of the timeline while it is out of view ("Subagent waiting · 2 queued"). */
export function endSummary(client: T3Client): string {
  if (!client.threadId) return '';
  const queued = queueState(client.projection).queued.length;
  const children = childInput(client) ? childThreadInputs(client.shell.threads, client.threadId) : [];
  return [children.length ? childThreadInputsWaiting(children) : '', queued ? `${queued} queued` : ''].filter(Boolean).join(' · ');
}
