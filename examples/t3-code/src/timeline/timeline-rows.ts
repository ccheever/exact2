// Timeline rows adapted from T3 Code 9c161462c9 (MIT, see LICENSE-T3; the turn-ended live row at 74541bab17, pending
// approvals, handoffs before a superseded attempt, usage-limit pauses and pending queued messages at v0.1.5):
// apps/web/src/session-logic.ts deriveTimelineEntriesFromVisibleTurnItems and
// apps/web/src/components/chat/MessagesTimeline.logic.ts deriveMessagesTimelineRows
// (turn folds, superseded attempts, live work, working and thinking rows).
import { arr, obj, str, type Obj } from '../client/domain';
import {
  displayFailed, formatDuration, groupAction, isToolLike, liveEntryLabel, projectedWorkEntry,
  singleToolCallLabel, successKeepsLive, contextCompactionTokenCounts, last, findLast, findLastIndex, summarizeToolGroup, summaryIcon, summaryKind, toolPresentation, visibleInGroup, type WorkEntry,
} from './timeline-worklog';
import { inlineReference, type InlineReference } from './inline-documents';
import { compactDynamicToolOutput } from './inline-tool-output';
import { t3ToolDefinition } from './timeline-t3tools';
import { compactions, IDLE_COMPACTING_LABELS, isCompactionCommand, resumeCompactionAge } from './timeline-compaction';
import { groupToolPresentation } from './timeline-work-rows';
import { agentStarted } from './timeline-worktree';
import { turnItemIsWorkspacePreparation } from './workspace-retry';

export interface Attempt { id: string; runId: string; status: string; completedAt: string }
export type Entry =
  | { kind: 'message'; id: string; createdAt: string; row: Obj; item: Obj; role: 'user' | 'assistant'; attempt?: Attempt }
  | { kind: 'document'; id: string; createdAt: string; row: Obj; item: Obj; document: InlineReference; attempt?: Attempt }
  | { kind: 'work'; id: string; createdAt: string; row: Obj; entry: WorkEntry; attempt?: Attempt }
  | { kind: 'event'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }
  | { kind: 'plan'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }
  | { kind: 'checkpoint'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt };

export type Row =
  | { kind: 'entry'; id: string; createdAt: string; entries: WorkEntry[]; label?: string; continues?: boolean }
  | { kind: 'details'; id: string; createdAt: string; entries: WorkEntry[]; continues?: boolean }
  | { kind: 'live'; id: string; createdAt: string; entry: WorkEntry; entries: WorkEntry[]; groupId: string; expanded: boolean; active: boolean; label: string; continues?: boolean;
      /** The group's latest reasoning, shown above the status line. */
      thought?: WorkEntry }
  | { kind: 'working'; id: string; createdAt: string;
      /** A running compaction owns the Working row, so its divider appears only once it ends. */
      compacting?: true }
  | { kind: 'thinking'; id: string; createdAt: string; continues?: boolean; groupId?: string; expanded?: boolean }
  | { kind: 'group'; id: string; createdAt: string; runId: string; groupId: string; count: number; expanded: boolean; summary: string; icon: string; toolIcon: Obj | null; failed: boolean; continues?: boolean }
  | { kind: 'fold'; id: string; createdAt: string; runId: string; label: string; expanded: boolean }
  | { kind: 'attempt'; id: string; createdAt: string; runId: string; attemptId: string; expanded: boolean }
  | { kind: 'compaction'; id: string; createdAt: string; label: string; tokenCounts: { before: string; after: string } | null; active: boolean;
      /** Replaces "Context compacted" when T3 Code compacted without being asked. */
      title?: string; failed?: boolean;
      /** The provider's reason, when a failed compaction gave one. */
      failure?: string }
  | { kind: 'message'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'message' }>; meta: boolean; inProgress: boolean; revert: number }
  | { kind: 'meta'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'message' }> }
  | { kind: 'event'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'event' }>; subagents: Obj[]; summary: boolean }
  | { kind: 'plan'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'plan' }> }
  | { kind: 'checkpoint'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'checkpoint' }> }
  | { kind: 'document'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'document' }> }
  | { kind: 'setup'; id: string; createdAt: string; snapshot: Obj; embedded: boolean };

export interface RowInput {
  rows: Obj[]; runs: Obj[]; attempts: Obj[]; nodes: Obj[]; checkpoints: Obj[];
  isWorking: boolean; runningRunId: string; latestRun: Obj | null; activeStartedAt: string;
  /** The latest run is still open but its provider turn ended, so nothing is generating while the server settles it. */
  latestRunTurnEnded?: boolean;
  /** A provider-native subagent's runless work is live (ChatView runlessWorkStartedAt). */
  runlessWorkActive?: boolean;
  /** The visible worktree setup of a new worktree thread (timeline-worktree.ts). */
  worktreeSetup?: Obj | null;
  expandedRuns: ReadonlySet<string>; expandedAttempts: ReadonlySet<string>; expandedGroups: ReadonlySet<string>;
  root: string; rollback: boolean;
  /** The thread start's ready checkpoint is loaded (a paged window cannot tell its first run is the thread's first). */
  threadStartCheckpointReady?: boolean;
  /** The run ordinals the provider conversation can rewind to (ChatView availableRevertTurnCounts); undefined: any. */
  availableRevertTurnCounts?: ReadonlySet<number>;
  /** The approvals answerable on their own row (timeline-approvals.ts): each stays out of folds, groups and the live row. */
  pendingApprovalRequestIds?: ReadonlySet<string>;
}

const STANDALONE = new Set(['fork', 'handoff', 'run_interrupt_request', 'run_interrupt_result', 'subagent', 'secret_request']);
// A secret request stays answerable after a steer supersedes the attempt that asked.
const PERSISTENT = new Set(['fork', 'thread_created', 'secret_request']);
export function rowKey(row: Obj): string { return JSON.stringify([row.sourceThreadId, row.sourceItemId]); }

const attemptOf = (attempt: Obj): Attempt => ({ id: str(attempt.id), runId: str(attempt.runId), status: str(attempt.status), completedAt: str(attempt.completedAt) });

/** session-logic deriveTimelineEntriesFromVisibleTurnItems, keeping checkpoints for the changed-files card. */
export function timelineEntries(input: Pick<RowInput, 'rows' | 'attempts' | 'nodes'>): Entry[] {
  const byRoot = new Map(input.attempts.map(attempt => [str(attempt.rootNodeId), attempt]));
  const nodes = new Map(input.nodes.map(node => [str(node.id), node]));
  const resolveAttempt = (item: Obj): Attempt | undefined => {
    if (!item.nodeId || !item.runId) return undefined;
    let nodeId: string | null = str(item.nodeId);
    const seen = new Set<string>();
    while (nodeId && !seen.has(nodeId)) {
      seen.add(nodeId);
      const direct = byRoot.get(nodeId);
      if (direct && direct.runId === item.runId) return attemptOf(direct);
      const node = nodes.get(nodeId);
      if (!node) return undefined;
      const root = byRoot.get(str(node.rootNodeId));
      if (root && root.runId === item.runId) return attemptOf(root);
      nodeId = node.parentNodeId ? str(node.parentNodeId) : null;
    }
    return undefined;
  };
  const answered = new Set(input.rows.flatMap(row => {
    const item = obj(row.item);
    return item.type === 'user_input_request' && item.questionAnswer ? [`async-answer:${str(obj(item.questionAnswer).requestId)}`] : [];
  }));
  const entries: Entry[] = [];
  for (const row of input.rows) {
    const item = obj(row.item), type = str(item.type), id = rowKey(row);
    if (turnItemIsWorkspacePreparation(item)) continue; // a retried preparation's cancelled failure hides too
    // A queued message still waiting is the composer's queue row, not yet a message of the timeline.
    if (type === 'user_message' && item.inputIntent === 'queued_turn' && item.status === 'pending') continue;
    if (type === 'user_message' && answered.has(str(item.messageId))) continue;
    const createdAt = str(item.startedAt ?? item.updatedAt), attempt = resolveAttempt(item);
    const base = { id, createdAt, row, ...(attempt ? { attempt } : {}) };
    const document = inlineReference(item);
    if (document) { entries.push({ ...base, kind: 'document', item, document }); continue; }
    if (type === 'user_message' || type === 'assistant_message') entries.push({ ...base, kind: 'message', item, role: type === 'user_message' ? 'user' : 'assistant' });
    else if (type === 'proposed_plan') entries.push({ ...base, kind: 'plan', item });
    else if (type === 'checkpoint') entries.push({ ...base, kind: 'checkpoint', item });
    else if (STANDALONE.has(type)) entries.push({ ...base, kind: 'event', item });
    else entries.push({ ...base, kind: 'work', entry: { ...projectedWorkEntry(row), id } });
  }
  return entries;
}

const runOf = (entry: Entry): string => entry.kind === 'message' ? entry.role === 'assistant' ? str(entry.item.runId) : ''
  : entry.kind === 'plan' || entry.kind === 'document' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';
const isSteer = (entry: Entry) => entry.kind === 'message' && entry.role === 'user'
  && (entry.item.inputIntent === 'steer' || entry.item.inputIntent === 'promoted_queued_to_steer');
const startsResponse = (entry: Entry) => entry.kind === 'message' && entry.role === 'user' && !isSteer(entry)
  || entry.kind === 'work' && entry.entry.itemType === 'notification';
const isCompaction = (entry: Entry) => entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction';
const persistentResource = (entry: Entry) => entry.kind === 'event' && PERSISTENT.has(str(entry.item.type));
const failedItem = (entry: Entry): Obj | null => {
  const item = entry.kind === 'event' ? entry.item : entry.kind === 'work' ? entry.entry.item : null;
  return item?.type === 'error' && item.status === 'failed' && (item.parentItemId === null || item.parentItemId === undefined) ? item : null;
};
/** itemizedFailedRunIds: a usage-limit pause keeps its run's tool groups; any other failure itemizes the run's work. */
function itemizedFailedRuns(entries: Entry[], failedRuns: ReadonlySet<string>): Set<string> {
  const usageLimited = new Set<string>(), otherFailures = new Set<string>();
  for (const entry of entries) {
    const item = failedItem(entry);
    if (item?.runId) (obj(item.failure).class === 'usage_limit' ? usageLimited : otherFailures).add(str(item.runId));
  }
  return new Set([...failedRuns].filter(runId => !usageLimited.has(runId) || otherFailures.has(runId)));
}
const messageEnd = (entry: Entry) => entry.kind === 'message' ? str(entry.item.updatedAt, entry.createdAt) : entry.createdAt;
const latestIso = (a: string | null, b: string | null) => !a ? b : !b ? a : !Number.isFinite(Date.parse(a)) ? b : !Number.isFinite(Date.parse(b)) ? a
  : Date.parse(b) > Date.parse(a) ? b : a;

/** The turn folds over `input`'s rows (deriveTimelineTurnFolds) and what deriveRows reads beside them. */
function foldState(input: RowInput) {
  const entries = withoutDelegationRows(settleReasoning(timelineEntries(input)));
  const latestRun = input.latestRun;
  const unsettled = input.runningRunId || (latestRun && !(latestRun.completedAt && !['running', 'starting', 'waiting'].includes(str(latestRun.status))) ? str(latestRun.id) : '');
  const failedRuns = new Set<string>();
  if (latestRun?.status === 'failed') failedRuns.add(str(latestRun.id));
  for (const entry of entries) { const item = failedItem(entry); if (item?.runId) failedRuns.add(str(item.runId)); }
  // Terminal assistant answers per response.
  const terminal = new Map<string, string>(); let responseIndex = 0;
  for (const entry of entries) {
    if (entry.kind !== 'message') continue;
    if (entry.role === 'user') { responseIndex++; continue; }
    terminal.set(entry.item.runId ? `turn:${str(entry.item.runId)}` : `unkeyed:${responseIndex}`, entry.id);
  }
  const terminalIds = new Set(terminal.values());
  const liveSubagents = liveSubagentCardIds(entries), completeReplies = completeSupersededReplyIds(entries);
  // Runs still forming the visible response.
  const boundary = findLastIndex(entries, startsResponse);
  const activeRuns = new Set<string>();
  if (unsettled) {
    activeRuns.add(unsettled);
    if (input.isWorking) for (const entry of entries.slice(boundary + 1)) { const run = runOf(entry); if (run) activeRuns.add(run); }
  }
  const runlessWorkActive = input.isWorking && input.runlessWorkActive === true;
  const folds = turnFolds(entries, terminalIds, latestRun, new Set([...activeRuns, ...failedRuns]), runlessWorkActive, new Set([...liveSubagents, ...completeReplies]));
  return { entries, latestRun, unsettled, failedRuns, terminalIds, liveSubagents, completeReplies, boundary, activeRuns, runlessWorkActive, folds };
}

/**
 * timelineTurnFoldRunIdsByEntryId: the fold each hidden entry is under, by the fold's run key. An imported or
 * runless turn folds under a key of its own (`runless:<prompt>`), not its entries' run id.
 */
export function turnFoldRunIds(input: RowInput): Map<string, string> {
  const byEntry = new Map<string, string>();
  for (const fold of foldState(input).folds.values()) for (const id of fold.hidden) byEntry.set(id, fold.runId);
  return byEntry;
}

/** deriveMessagesTimelineRows, without T3's tool catalogue, worktree setup or created-thread summaries. */
export function deriveRows(input: RowInput): Row[] {
  const { entries, latestRun, unsettled, failedRuns, terminalIds, liveSubagents, completeReplies, boundary, activeRuns, runlessWorkActive, folds } = foldState(input);
  // Superseded attempts the steer cut off: one whose reply had already finished stays like any turn output.
  const completeAttempts = new Set(entries.flatMap(entry => entry.attempt && completeReplies.has(entry.id) ? [entry.attempt.id] : []));
  const attemptGroups = new Map<string, Entry[]>();
  for (const entry of entries) {
    if (entry.attempt?.status !== 'superseded' || completeAttempts.has(entry.attempt.id) || failedRuns.has(entry.attempt.runId) || entry.kind === 'message' && entry.role === 'user'
      || entry.kind === 'event' && entry.item.type === 'handoff' || entry.kind === 'document' || persistentResource(entry) || liveSubagents.has(entry.id)
      || entry.kind === 'work' && entry.entry.itemType === 'system_notice') continue;
    attemptGroups.set(entry.attempt.id, [...attemptGroups.get(entry.attempt.id) ?? [], entry]);
  }
  const attemptFolds = new Map<string, { runId: string; attemptId: string; createdAt: string; hidden: Set<string> }>();
  for (const group of attemptGroups.values()) {
    const first = group[0]!;
    attemptFolds.set(first.id, { runId: first.attempt!.runId, attemptId: first.attempt!.id, createdAt: first.createdAt, hidden: new Set(group.map(entry => entry.id)) });
  }
  const itemizedRuns = itemizedFailedRuns(entries, failedRuns);
  const pendingApprovals = input.pendingApprovalRequestIds ?? new Set<string>();
  const isPendingApproval = (entry: WorkEntry) => entry.item.type === 'approval_request' && pendingApprovals.has(str(entry.item.requestId));
  const collapsed = new Set<string>();
  for (const fold of folds.values()) if (!input.expandedRuns.has(fold.runId)) fold.hidden.forEach(id => collapsed.add(id));
  const collapsedAttempts = new Set<string>();
  for (const fold of attemptFolds.values()) if (!input.expandedAttempts.has(fold.attemptId)) fold.hidden.forEach(id => collapsedAttempts.add(id));
  // A pending approval stays answerable on its row, whatever fold it sits in.
  if (pendingApprovals.size) for (const entry of entries) if (entry.kind === 'work' && isPendingApproval(entry.entry)) { collapsed.delete(entry.id); collapsedAttempts.delete(entry.id); }
  const inActive = (runId: string) => runId ? activeRuns.has(runId) : runlessWorkActive;
  const entryActive = (entry: WorkEntry) => input.isWorking && entry.status === 'inProgress' && entry.runId === unsettled && !!unsettled;
  const headerIndex = input.isWorking ? boundary + 1 : entries.length;

  // Trailing work of the active run collapses into one live row.
  const activeTail: Extract<Entry, { kind: 'work' }>[] = [];
  if (input.isWorking && unsettled) {
    let tailAttempt: string | null | undefined;
    for (let index = entries.length - 1; index >= headerIndex; index--) {
      const entry = entries[index]!;
      if (entry.kind !== 'work' || entry.entry.tone === 'error' || entry.entry.sourceActivityKind === 'runtime.error'
        || ['system_notice', 'notification'].includes(entry.entry.itemType) || entry.entry.questionAnswer !== undefined || isPendingApproval(entry.entry) || !inActive(entry.entry.runId)
        || entry.entry.sourceActivityKind === 'context-compaction' || collapsed.has(entry.id) || collapsedAttempts.has(entry.id)
        || folds.has(entry.id) || attemptFolds.has(entry.id)) break;
      const attempt = entry.attempt?.id ?? null;
      if (tailAttempt === undefined) tailAttempt = attempt; else if (attempt !== tailAttempt) break;
      activeTail.push(entry);
    }
    activeTail.reverse();
  }
  const visibleTail = activeTail.filter(entry => visibleInGroup(entry.entry, true));
  const anchor = activeTail[0], latestVisible = last(visibleTail);
  const latestRunning = findLast(visibleTail, entry => entry.entry.status === 'inProgress');
  const keepsLive = !!latestRunning || !!latestVisible && successKeepsLive(latestVisible.entry) && !pendingApprovals.size;
  const latestFailed = !latestRunning && !!latestVisible && latestVisible.entry.status !== 'declined' && displayFailed(latestVisible.entry);
  const latestThought = findLast(visibleTail, item => item.entry.itemType === 'reasoning' && !!item.entry.detail?.trim());
  const liveRow: Extract<Row, { kind: 'live' }> | null = anchor && latestVisible && !latestFailed ? (() => {
    const groupId = `work-group:${anchor.id}`, entry = (latestRunning ?? latestVisible).entry;
    return { kind: 'live', id: keepsLive ? 'live-activity-row' : `work-live:${anchor.id}`, createdAt: anchor.createdAt, entry,
      entries: visibleTail.map(item => item.entry), groupId, expanded: input.expandedGroups.has(groupId), active: keepsLive,
      label: liveEntryLabel(entry, input.root, keepsLive), ...(latestThought ? { thought: latestThought.entry } : {}) };
  })() : null;
  const liveIds = new Set(liveRow || latestFailed ? activeTail.map(entry => entry.id) : []);
  const compaction = compactions(entries, input.isWorking, unsettled, entryActive, runOf);
  const rows: Row[] = [];
  const pushWorking = () => {
    if (compaction.onlyFinished || compaction.running !== null && compaction.waiting === null) return;
    rows.push({ kind: 'working', id: 'working-indicator-row', createdAt: compaction.startedAt ?? input.activeStartedAt,
      ...(compaction.waiting !== null ? { compacting: true as const } : {}) });
  };
  let activity = false;
  const pushLive = () => {
    if (!liveRow) return;
    rows.push(liveRow); activity ||= liveRow.active;
    if (liveRow.expanded) rows.push({ kind: 'details', id: `${liveRow.groupId}:details`, createdAt: liveRow.createdAt, entries: liveRow.entries });
  };
  const revertCounts = input.rollback ? revertTurnCounts(entries, input.checkpoints, { threadStartCheckpointReady: input.threadStartCheckpointReady === true,
    availableRevertTurnCounts: input.availableRevertTurnCounts, unsettledRunId: unsettled }) : new Map<string, number>();

  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index]!;
    if (input.isWorking && index === headerIndex) pushWorking();
    if (entry.id === latestVisible?.id) pushLive();
    // A stopped compaction's own divider already says it stopped.
    if (entry.kind === 'event' && (entry.item.type === 'run_interrupt_request' || entry.item.type === 'run_interrupt_result'
      && compaction.runs.has(str(entry.item.runId)) && compaction.compacted.has(str(entry.item.runId)))) continue;
    const fold = folds.get(entry.id);
    if (fold) rows.push({ kind: 'fold', id: `turn-fold:${fold.runId}`, createdAt: fold.createdAt, runId: fold.runId, label: fold.label, expanded: input.expandedRuns.has(fold.runId) });
    if (collapsed.has(entry.id)) continue;
    const attemptFold = attemptFolds.get(entry.id);
    if (attemptFold) rows.push({ kind: 'attempt', id: `attempt-fold:${attemptFold.attemptId}`, createdAt: attemptFold.createdAt, runId: attemptFold.runId,
      attemptId: attemptFold.attemptId, expanded: input.expandedAttempts.has(attemptFold.attemptId) });
    if (collapsedAttempts.has(entry.id) || liveIds.has(entry.id)) continue;
    if (entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction') {
      const active = entryActive(entry.entry), runId = entry.entry.runId;
      if (active && compaction.waiting !== null && runId === compaction.waiting) continue;
      const item = entry.entry.item, title = compaction.title(runId, entry.id), reason = runId ? compaction.runs.get(runId) : undefined;
      const failed = item.type === 'compaction' && item.status === 'failed', failure = failed ? str(item.summary).trim() : '';
      rows.push({ kind: 'compaction', id: entry.id, createdAt: entry.createdAt, ...(title ? { title } : {}), ...(failed ? { failed } : {}), ...(failure ? { failure } : {}),
        label: active && reason ? IDLE_COMPACTING_LABELS[reason] : entry.entry.label, tokenCounts: contextCompactionTokenCounts(item), active });
      continue;
    }
    if (entry.kind === 'work' && compaction.compacted.has(entry.entry.runId) && entry.entry.questionAnswer && resumeCompactionAge(entry.entry.questionAnswer) !== null) continue;
    if (entry.kind === 'work') {
      const work = entry.entry;
      if (isPendingApproval(work) || work.runId && itemizedRuns.has(work.runId) || work.itemType === 'error' && work.status === 'failed' || work.tone === 'error'
        || work.sourceActivityKind === 'runtime.error' || work.itemType === 'system_notice' || work.itemType === 'notification' || work.questionAnswer !== undefined) {
        // WorkGroupSection draws only what a collapsed group shows (a failed run's unfinished tool call has no row), and
        // a pending approval.
        if (isPendingApproval(work) || visibleInGroup(work)) rows.push({ kind: 'entry', id: entry.id, createdAt: entry.createdAt, entries: [work] });
        continue;
      }
      const grouped = [work];
      let cursor = index + 1;
      while (cursor < entries.length) {
        const next = entries[cursor]!;
        if (next.kind !== 'work' || next.entry.sourceActivityKind === 'context-compaction' || next.entry.itemType === 'error' && next.entry.status === 'failed' || next.entry.tone === 'error'
          || next.entry.sourceActivityKind === 'runtime.error' || ['system_notice', 'notification'].includes(next.entry.itemType)
          || next.entry.questionAnswer !== undefined || isPendingApproval(next.entry) || liveIds.has(next.id) || collapsed.has(next.id) || collapsedAttempts.has(next.id) || folds.has(next.id) || attemptFolds.has(next.id)
          || next.entry.runId !== work.runId || next.attempt?.id !== entry.attempt?.id) break;
        grouped.push(next.entry); cursor++;
      }
      const visible = grouped.filter(item => visibleInGroup(item, entryActive(item)));
      if (visible.length) {
        const running = visible.filter(entryActive);
        const groupId = `work-group:${entry.id}`, expanded = input.expandedGroups.has(groupId);
        if (running.length) {
          const latest = last(running)!;
          rows.push({ kind: 'live', id: `work-live:${entry.id}`, createdAt: entry.createdAt, entry: latest, entries: visible, groupId, expanded, active: true,
            label: liveEntryLabel(latest, input.root, true) });
          activity = true;
          if (expanded) rows.push({ kind: 'details', id: `${groupId}:details`, createdAt: entry.createdAt, entries: visible });
        } else if (visible.length === 1 && isToolLike(visible[0]!)) {
          const single = visible[0]!;
          rows.push({ kind: 'entry', id: entry.id, createdAt: entry.createdAt, entries: visible,
            label: groupAction(single) === 'edit' ? summarizeToolGroup(visible).summary : singleToolCallLabel(single, input.root) });
        } else {
          const single = visible.length === 1 ? visible[0]! : null;
          const singleLabel = single !== null && isToolLike(single) && groupAction(single) !== 'edit';
          const latestTool = findLast(visible, isToolLike);
          // WorkGroupToggleTimelineRow: a lone tool's T3 icon, else the tools' surface, else the summary glyph;
          // the image is the primary tool source's icon, else the last entry's.
          const presented = groupToolPresentation(visible.map(item => item.item));
          const summaryToolIcon = singleLabel ? toolPresentation(single, 'completed')?.icon : undefined;
          rows.push({ kind: 'group', id: `work-toggle:${entry.id}`, createdAt: entry.createdAt, runId: work.runId, groupId, count: visible.length, expanded,
            summary: singleLabel ? singleToolCallLabel(single, input.root) : single !== null && !isToolLike(single) ? single.label : summarizeToolGroup(visible).summary,
            icon: summaryToolIcon ?? presented.toolSurface ?? summaryIcon(summaryKind(visible)), toolIcon: presented.toolIcon ?? null, failed: !!latestTool && displayFailed(latestTool) });
          if (expanded) rows.push({ kind: 'details', id: `${groupId}:details`, createdAt: entry.createdAt, entries: visible });
        }
      }
      index = cursor - 1;
      continue;
    }
    if (entry.kind === 'document') { rows.push({ kind: 'document', id: entry.id, createdAt: entry.createdAt, entry }); continue; }
    if (entry.kind === 'plan') { rows.push({ kind: 'plan', id: entry.id, createdAt: entry.createdAt, entry }); continue; }
    if (entry.kind === 'checkpoint') { rows.push({ kind: 'checkpoint', id: entry.id, createdAt: entry.createdAt, entry }); continue; }
    if (entry.kind === 'event') {
      const previous = last(rows);
      if (entry.item.type === 'subagent' && previous?.kind === 'event' && previous.entry.item.type === 'subagent'
        && previous.entry.item.runId === entry.item.runId && previous.entry.item.providerTurnId === entry.item.providerTurnId) {
        previous.subagents = [...previous.subagents.length ? previous.subagents : [previous.entry.item], entry.item];
        continue;
      }
      rows.push({ kind: 'event', id: entry.id, createdAt: entry.createdAt, entry, subagents: [], summary: false });
      continue;
    }
    const runId = str(entry.item.runId);
    if (runId && isCompactionCommand(entry.item)) {
      if (!compaction.compacted.has(runId) && runId !== compaction.waiting) {
        const pending = runId === compaction.running, reason = compaction.runs.get(runId);
        rows.push({ kind: 'compaction', id: `compaction:${runId}`, createdAt: entry.createdAt, tokenCounts: null, active: pending,
          label: pending ? reason ? IDLE_COMPACTING_LABELS[reason] : 'Compacting context' : 'Context compaction' });
      }
      continue;
    }
    const inProgress = entry.role === 'assistant' && inActive(runId);
    rows.push({ kind: 'message', id: entry.id, createdAt: entry.createdAt, entry, inProgress,
      meta: entry.role === 'assistant' && terminalIds.has(entry.id) && !inProgress,
      revert: entry.role === 'user' ? revertCounts.get(str(entry.item.messageId)) ?? -1 : -1 });
  }
  // Until the agent's turn is live the setup card sits under the send, the
  // working header above it; a finished setup keeps that slot until the run
  // starts, and a failed or cancelled one stays under the send (3a7058d).
  const setup = input.worktreeSetup ?? null;
  const setupHandedOff = !!setup && agentStarted(setup) && !!latestRun?.startedAt;
  const setupOwnsWorkingSlot = !setupHandedOff && (setup?.phase === 'running' || setup?.phase === 'done');
  if (setup && (!setupHandedOff || setup.phase !== 'running')) {
    const setupRow: Row = { kind: 'setup', id: 'worktree-setup-row', createdAt: str(setup.startedAt), snapshot: setup, embedded: setupHandedOff };
    const firstUser = rows.findIndex(row => row.kind === 'message' && row.entry.role === 'user');
    const working = setupOwnsWorkingSlot ? rows.findIndex(row => row.kind === 'working') : -1;
    if (working >= 0) rows.splice(working + 1, 0, setupRow);
    else rows.splice(firstUser >= 0 ? firstUser + 1 : rows.length, 0,
      ...(setupOwnsWorkingSlot ? [{ kind: 'working', id: 'working-indicator-row', createdAt: str(setup.startedAt) } as Row, setupRow] : [setupRow]));
  }
  if (input.isWorking && !rows.some(row => row.kind === 'working') && headerIndex === entries.length) pushWorking();
  // A setup that owns the working slot shows no activity row of its own.
  if (input.isWorking && input.latestRunTurnEnded !== true && !setupOwnsWorkingSlot && !compaction.active && !compaction.onlyFinished
    && (latestFailed || !activity && !pendingApprovals.size)) {
    // A failed latest tool hands the row back to thinking; its group stays
    // reachable through the same disclosure the live row offers.
    if (latestFailed && anchor) {
      const groupId = `work-group:${anchor.id}`, expanded = input.expandedGroups.has(groupId);
      rows.push({ kind: 'thinking', id: 'live-activity-row', createdAt: input.activeStartedAt, groupId, expanded });
      if (expanded) rows.push({ kind: 'details', id: `${groupId}:details`, createdAt: anchor.createdAt, entries: visibleTail.map(item => item.entry) });
    } else rows.push({ kind: 'thinking', id: 'live-activity-row', createdAt: input.activeStartedAt });
  }
  const result = attachTrailingTools(rows);
  const workLog = (row: Row | undefined) => !!row && ['entry', 'details', 'live', 'group', 'thinking'].includes(row.kind);
  result.forEach((row, index) => { if (workLog(row) && workLog(result[index + 1])) (row as { continues?: boolean }).continues = true; });
  return result;
}

const boundaryRunOf = (entry: Entry): string => entry.kind === 'message' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';

/**
 * liveSubagentCardEntryIds: subagent cards whose child still works stay out of their turn's folds
 * (and a superseded attempt's); adjacent cards of one provider turn are one row, kept whole while any is live.
 */
function liveSubagentCardIds(entries: Entry[]): Set<string> {
  const visible = new Set<string>();
  let batch: Extract<Entry, { kind: 'event' }>[] = [];
  const flush = () => {
    if (batch.some(entry => ['pending', 'running', 'waiting'].includes(str(entry.item.status)))) for (const entry of batch) visible.add(entry.id);
    batch = [];
  };
  for (const entry of entries) {
    if (entry.kind !== 'event' || entry.item.type !== 'subagent') { flush(); continue; }
    const previous = last(batch)?.item;
    if (previous && (previous.runId !== entry.item.runId || previous.providerTurnId !== entry.item.providerTurnId)) flush();
    batch.push(entry);
  }
  flush();
  return visible;
}

/**
 * completeSupersededReplyEntryIds: a superseded attempt's last output, when it is an assistant reply
 * that completed before the steer stamped the attempt's completedAt, was whole (an interrupted
 * stream also closes as completed, so the item status alone cannot tell).
 */
function completeSupersededReplyIds(entries: Entry[]): Set<string> {
  const lastOutput = new Map<string, Entry>();
  for (const entry of entries) {
    if (entry.attempt?.status !== 'superseded' || entry.kind === 'message' && entry.role === 'user'
      || entry.kind === 'work' && ['system_notice', 'notification'].includes(entry.entry.itemType) || entry.kind !== 'message' && entry.kind !== 'work') continue;
    lastOutput.set(entry.attempt.id, entry);
  }
  const ids = new Set<string>();
  for (const entry of lastOutput.values()) {
    if (entry.kind !== 'message' || entry.role !== 'assistant' || entry.item.streaming === true || entry.item.status !== 'completed') continue;
    const completedAt = Date.parse(str(entry.item.completedAt)), supersededAt = Date.parse(entry.attempt?.completedAt ?? '');
    if (Number.isFinite(completedAt) && Number.isFinite(supersededAt) && completedAt < supersededAt) ids.add(entry.id);
  }
  return ids;
}

/**
 * withoutSubagentDelegationRows: a delegation already has its child's durable card, so its tool row
 * goes once the returned task id names that child (pending calls can share a prompt).
 */
function withoutDelegationRows(entries: Entry[]): Entry[] {
  const children = new Map<string, Set<string>>();
  for (const entry of entries) {
    if (entry.kind !== 'event' || entry.item.type !== 'subagent' || entry.item.origin !== 'app_owned' || !entry.item.runId) continue;
    const runId = str(entry.item.runId);
    children.set(runId, (children.get(runId) ?? new Set()).add(str(entry.item.subagentId)));
  }
  return entries.filter(entry => {
    if (entry.kind !== 'work' || displayFailed(entry.entry)) return true;
    const item = entry.entry.item;
    if (item.type !== 'dynamic_tool' || !item.runId || item.status !== 'running' && item.status !== 'completed'
      || t3ToolDefinition(str(item.toolName))?.summaryAction !== 'delegate') return true;
    const output = compactDynamicToolOutput(item.output);
    if (output?.isError || output?.taskId === undefined) return true;
    return !children.get(str(item.runId))?.has(output.taskId);
  });
}

function settleReasoning(entries: Entry[]): Entry[] {
  return entries.map((entry, index) => index === entries.length - 1 || entry.kind !== 'work' || entry.entry.itemType !== 'reasoning'
    || entry.entry.status !== 'inProgress' ? entry : { ...entry, entry: { ...entry.entry, status: 'completed' } });
}

interface Fold { runId: string; createdAt: string; hidden: Set<string>; label: string }
/**
 * deriveTurnFolds. A prompt without a run (a provider-native subagent, or a
 * turn imported from V1) lends its response a key of its own, decided per
 * prompt so a V1 thread's first V2 run does not unfold every imported turn.
 * A steer after the turn has produced output splits it: the work after each
 * steer folds under "Continued for …" below that steer, keyed `<run>:steer:<steer>`.
 */
function turnFolds(entries: Entry[], terminalIds: Set<string>, latestRun: Obj | null, unfolded: Set<string>, runlessWorkActive: boolean, kept: Set<string>): Map<string, Fold> {
  const interrupted = new Set(entries.flatMap(entry => entry.kind === 'event' && entry.item.runId
    && (entry.item.type === 'run_interrupt_request' || entry.item.type === 'run_interrupt_result') ? [str(entry.item.runId)] : []));
  const promptIndex = new Map<string, number>();
  entries.forEach((entry, index) => { const run = startsResponse(entry) ? boundaryRunOf(entry) : ''; if (run && !promptIndex.has(run)) promptIndex.set(run, index); });
  interface Group { entries: Entry[]; terminal: Entry | null; streaming: boolean; start: string | null; anchor: string; steers: { startIndex: number; steer: Entry }[] }
  const groups = new Map<string, Group>(), runlessFailed = new Set<string>();
  let runless: string | null = null, response: string | null = null, pending: { createdAt: string; anchor: string } | null = null;
  entries.forEach((entry, index) => {
    if (startsResponse(entry)) {
      const next = entries[index + 1];
      pending = next ? { createdAt: entry.createdAt, anchor: next.id } : null;
      const boundaryRun = boundaryRunOf(entry);
      runless = boundaryRun ? null : `runless:${entry.id}`;
      response = boundaryRun || runless;
      return;
    }
    if (entry.kind === 'message' && isSteer(entry)) {
      const steerRun = str(entry.item.runId) || response;
      const group = steerRun ? groups.get(steerRun) : undefined;
      if (group?.entries.length) {
        // Back-to-back steers split once, after the last of them.
        if (last(group.steers)?.startIndex === group.entries.length) group.steers.pop();
        group.steers.push({ startIndex: group.entries.length, steer: entry });
      }
      return;
    }
    let own: string | null = null;
    if (entry.kind === 'work' && entry.entry.itemType === 'system_notice') own = null;
    // An inline render or MCP app is the turn's output, never folded work.
    else if (entry.kind === 'message' && entry.role === 'assistant' || entry.kind === 'work') own = runOf(entry) || runless;
    else if (entry.kind === 'event' && (persistentResource(entry) || entry.item.type === 'subagent')) own = str(entry.item.runId) || runless;
    if (!own) return;
    // A resumed subagent's item moves to the resuming run but keeps its place in the turn that
    // started it: it folds with that turn, so the later run's fold still opens under its own prompt.
    const runId = response !== null && (promptIndex.get(own) ?? -1) > index ? response : own;
    if (runId === runless && failedItem(entry)) runlessFailed.add(runId);
    let group = groups.get(runId);
    if (!group) {
      const start: { createdAt: string; anchor: string } | null = pending;
      group = { entries: [], terminal: null, streaming: false, start: start?.createdAt ?? null, anchor: start?.anchor ?? entry.id, steers: [] };
      pending = null; groups.set(runId, group);
    }
    group.entries.push(entry);
    if (entry.kind === 'message') {
      if (terminalIds.has(entry.id)) group.terminal = entry;
      if (entry.item.streaming === true) group.streaming = true;
    }
  });
  const result = new Map<string, Fold>();
  for (const [runId, group] of groups) {
    if (unfolded.has(runId) || interrupted.has(runId) || runlessFailed.has(runId) || runlessWorkActive && runId === runless || group.streaming) continue;
    const terminalIndex = group.terminal ? group.entries.indexOf(group.terminal) : group.entries.length;
    const hidden = new Set<string>();
    group.entries.forEach((entry, index) => {
      if (entry === group.terminal) return;
      const compaction = isCompaction(entry);
      const trailing = entry.kind === 'work' && entry.entry.status !== 'inProgress' && !displayFailed(entry.entry);
      if (!compaction && index > terminalIndex && !trailing) return;
      // Compactions, linked resources, still-working children and finished replies a steer superseded outlive the
      // fold. An answered question stays visible after the surrounding work settles.
      if (compaction || persistentResource(entry) || kept.has(entry.id) || entry.kind === 'work' && (entry.entry.itemType === 'notification' || entry.entry.questionAnswer !== undefined)) return;
      hidden.add(entry.id);
    });
    const hidesWork = (segment: Entry[]) => segment.some(entry => hidden.has(entry.id) && !isCompaction(entry));
    if (!hidden.size || !hidesWork(group.entries)) continue;
    const first = group.entries[0]!, final = last(group.entries)!;
    const interruptedLatest = latestRun?.id === runId && latestRun.status === 'interrupted';
    const lastEnd = messageEnd(final);
    const end = latestIso(group.terminal ? messageEnd(group.terminal) : null, lastEnd) ?? lastEnd;
    const elapsed = latestRun?.id === runId && latestRun.startedAt && latestRun.completedAt
      ? Date.parse(str(latestRun.completedAt)) - Date.parse(str(latestRun.startedAt))
      : Date.parse(end) - Date.parse(group.start ?? first.createdAt);
    const duration = Number.isFinite(elapsed) ? formatDuration(Math.max(0, elapsed)) : null;
    const label = interruptedLatest ? duration ? `You stopped after ${duration}` : 'You stopped this response'
      : duration ? `Worked for ${duration}` : 'Worked';
    // The top row keeps the whole turn's time, steers included; each segment after a steer opens on its own.
    const segmentHidden = (segment: Entry[]) => new Set(segment.flatMap(entry => hidden.has(entry.id) ? [entry.id] : []));
    result.set(group.anchor, { runId, createdAt: group.start ?? first.createdAt, hidden: segmentHidden(group.entries.slice(0, group.steers[0]?.startIndex)), label });
    group.steers.forEach(({ startIndex, steer }, index) => {
      const segment = group.entries.slice(startIndex, group.steers[index + 1]?.startIndex), anchor = segment[0];
      if (!anchor || !hidesWork(segment)) return;
      const segmentEnd = segment.map(messageEnd).reduce<string | null>(latestIso, null) ?? steer.createdAt;
      const span = Date.parse(segmentEnd) - Date.parse(steer.createdAt);
      result.set(anchor.id, { runId: `${runId}:steer:${steer.id}`, createdAt: steer.createdAt, hidden: segmentHidden(segment),
        label: Number.isFinite(span) ? `Continued for ${formatDuration(Math.max(0, span))}` : 'Continued' });
    });
  }
  return result;
}

/**
 * A settled answer followed by its own tool groups takes its metadata after
 * them; its changed files stay with the answer, as T3 draws them inside it.
 */
function attachTrailingTools(rows: Row[]): Row[] {
  const withoutMeta = new Set<string>(), metaAfter = new Map<number, Row>(), filesAfter = new Map<string, Row>(), moved = new Set<Row>();
  rows.forEach((row, at) => {
    if (row.kind !== 'message' || row.entry.role !== 'assistant' || !row.meta) return;
    const runId = str(row.entry.item.runId);
    if (!runId) return;
    let last = -1, trailing = false;
    for (let index = at + 1; index < rows.length; index++) {
      const candidate = rows[index]!;
      if (candidate.kind === 'message') break;
      if (candidate.kind === 'group' && candidate.runId === runId) { trailing = true; last = index; continue; }
      if (candidate.kind === 'entry' && candidate.entries.some(entry => entry.runId === runId)) {
        if (candidate.entries.some(entry => isToolLike(entry) || entry.itemType === 'error' && entry.status === 'failed')) trailing = true;
        if (trailing) last = index;
      }
    }
    if (last < 0) return;
    withoutMeta.add(row.id);
    metaAfter.set(last, { kind: 'meta', id: `assistant-meta:${row.id}`, createdAt: rows[last]!.createdAt, entry: row.entry });
    const files = rows[last + 1];
    if (files?.kind === 'checkpoint' && (!files.entry.item.runId || files.entry.item.runId === runId)) { filesAfter.set(row.id, files); moved.add(files); }
  });
  const result: Row[] = [];
  rows.forEach((row, index) => {
    if (moved.has(row)) return;
    result.push(row.kind === 'message' && withoutMeta.has(row.id) ? { ...row, meta: false } : row);
    const files = filesAfter.get(row.id);
    if (files) result.push(files);
    const meta = metaAfter.get(index);
    if (meta) result.push(meta);
  });
  return result;
}

/**
 * session-logic deriveRevertTurnCountByUserMessageId (T3 Code 74541bab17): a started turn rewinds to the checkpoint of the
 * run before it, the thread start's for the first. Run ordinals keep increasing after a rewind, so only the visible local
 * runs (a message's, a notification's) define predecessors; inherited history belongs to the source thread.
 */
export function revertTurnCounts(entries: Entry[], checkpoints: Obj[], options: { threadStartCheckpointReady?: boolean;
  availableRevertTurnCounts?: ReadonlySet<number> | undefined; unsettledRunId?: string } = {}): Map<string, number> {
  const ready = new Map<string, number>();
  for (const checkpoint of checkpoints) if (checkpoint.status === 'ready' && checkpoint.runId && typeof checkpoint.appRunOrdinal === 'number') ready.set(str(checkpoint.runId), checkpoint.appRunOrdinal);
  const targets = new Map<string, number | undefined>(), counts = new Map<string, number>();
  let previous: string | null = null;
  for (const entry of entries) {
    if (entry.kind !== 'message' && entry.kind !== 'work') continue;
    if (entry.row.visibility === 'inherited') continue;
    const runId = entry.kind === 'message' ? str(entry.item.runId) : entry.entry.itemType === 'notification' ? entry.entry.runId : '';
    if (!runId) continue;
    if (!targets.has(runId)) {
      targets.set(runId, previous === null ? options.threadStartCheckpointReady ? 0 : undefined : ready.get(previous));
      previous = runId;
    }
    if (entry.kind !== 'message' || entry.role !== 'user' || (options.unsettledRunId && runId === options.unsettledRunId)) continue;
    if (!['turn_start', 'queued_turn'].includes(str(entry.item.inputIntent))) continue;
    const target = targets.get(runId);
    if (target !== undefined && (options.availableRevertTurnCounts === undefined || options.availableRevertTurnCounts.has(target))) counts.set(str(entry.item.messageId), target);
  }
  return counts;
}

/** findRevertCheckpoint: the ready checkpoint a rewind to `turnCount` restores (the thread start's for 0). */
export function findRevertCheckpoint(checkpoints: Obj[], turnCount: number): Obj | undefined {
  return [...checkpoints].reverse().find(checkpoint => checkpoint.status === 'ready' && (turnCount === 0
    ? checkpoint.ordinalWithinScope === 0 && checkpoint.appRunOrdinal == null : checkpoint.appRunOrdinal === turnCount));
}

/**
 * ChatView availableRevertTurnCounts: the provider conversation can rewind to the thread start and to each run whose
 * latest provider turn ran on the active provider thread, when that thread has a session on the thread's provider.
 * A ready file checkpoint alone cannot restore the conversation.
 */
export function availableRevertTurnCounts(projection: Obj): Set<number> {
  const available = new Set<number>(), thread = obj(projection.thread);
  const providerThread = arr(projection.providerThreads).find(candidate => candidate.id === thread.activeProviderThreadId);
  if (!providerThread?.providerSessionId || providerThread.providerInstanceId !== obj(thread.modelSelection).instanceId) return available;
  available.add(0);
  const attempts = new Map(arr(projection.attempts).map(attempt => [str(attempt.id), attempt]));
  const turns = new Map<string, Obj>(), latestByAttempt = new Map<string, Obj>();
  for (const turn of arr(projection.providerTurns)) {
    turns.set(str(turn.id), turn);
    if (turn.runAttemptId == null) continue;
    const latest = latestByAttempt.get(str(turn.runAttemptId));
    if (!latest || Number(turn.ordinal) > Number(latest.ordinal)) latestByAttempt.set(str(turn.runAttemptId), turn);
  }
  for (const run of arr(projection.runs)) {
    if (run.activeAttemptId == null) continue;
    const attempt = attempts.get(str(run.activeAttemptId));
    const turn = latestByAttempt.get(str(run.activeAttemptId)) ?? (attempt?.providerTurnId == null ? undefined : turns.get(str(attempt.providerTurnId)));
    if (turn?.providerThreadId === providerThread.id && typeof run.ordinal === 'number') available.add(run.ordinal);
  }
  return available;
}

/**
 * threadExecution.ts summarizeThreadRun's turnEnded: a run in an activity status whose active attempt's latest
 * provider turn (latestProviderTurnForAttempt) is neither pending nor running.
 */
export function runTurnEnded(projection: Obj, run: Obj | null | undefined): boolean {
  if (!run || !['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)) || run.activeAttemptId == null) return false;
  let latest: Obj | undefined;
  for (const turn of arr(projection.providerTurns)) {
    if (turn.runAttemptId !== run.activeAttemptId) continue;
    if (!latest || Number(turn.ordinal) > Number(latest.ordinal)) latest = turn;
  }
  return !!latest && latest.status !== 'pending' && latest.status !== 'running';
}

export function runList(projection: Obj): Obj[] { return arr(projection.runs); }
