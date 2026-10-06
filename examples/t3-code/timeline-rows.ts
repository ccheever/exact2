// Timeline rows adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/session-logic.ts deriveTimelineEntriesFromVisibleTurnItems and
// apps/web/src/components/chat/MessagesTimeline.logic.ts deriveMessagesTimelineRows
// (turn folds, superseded attempts, live work, working and thinking rows).
import { arr, obj, str, type Obj } from './domain';
import {
  displayFailed, entryIcon, formatDuration, groupAction, isToolLike, liveEntryLabel, projectedWorkEntry,
  singleToolCallLabel, successKeepsLive, last, findLast, findLastIndex, summarizeToolGroup, summaryIcon, summaryKind, visibleInGroup, type WorkEntry,
} from './timeline-worklog';
import { agentStarted } from './timeline-worktree';
import { turnItemIsWorkspacePreparation } from './r11-upstream-retry';

export interface Attempt { id: string; runId: string; status: string }
export type Entry =
  | { kind: 'message'; id: string; createdAt: string; row: Obj; item: Obj; role: 'user' | 'assistant'; attempt?: Attempt }
  | { kind: 'work'; id: string; createdAt: string; row: Obj; entry: WorkEntry; attempt?: Attempt }
  | { kind: 'event'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }
  | { kind: 'plan'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt }
  | { kind: 'checkpoint'; id: string; createdAt: string; row: Obj; item: Obj; attempt?: Attempt };

export type Row =
  | { kind: 'entry'; id: string; createdAt: string; entries: WorkEntry[]; label?: string; continues?: boolean }
  | { kind: 'details'; id: string; createdAt: string; entries: WorkEntry[]; continues?: boolean }
  | { kind: 'live'; id: string; createdAt: string; entry: WorkEntry; entries: WorkEntry[]; groupId: string; expanded: boolean; active: boolean; label: string; continues?: boolean }
  | { kind: 'working'; id: string; createdAt: string }
  | { kind: 'thinking'; id: string; createdAt: string; continues?: boolean; groupId?: string; expanded?: boolean }
  | { kind: 'group'; id: string; createdAt: string; runId: string; groupId: string; count: number; expanded: boolean; summary: string; icon: string; failed: boolean; continues?: boolean }
  | { kind: 'fold'; id: string; createdAt: string; runId: string; label: string; expanded: boolean }
  | { kind: 'attempt'; id: string; createdAt: string; runId: string; attemptId: string; expanded: boolean }
  | { kind: 'compaction'; id: string; createdAt: string; label: string; active: boolean }
  | { kind: 'message'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'message' }>; meta: boolean; inProgress: boolean; revert: number }
  | { kind: 'meta'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'message' }> }
  | { kind: 'event'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'event' }>; subagents: Obj[]; summary: boolean }
  | { kind: 'plan'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'plan' }> }
  | { kind: 'checkpoint'; id: string; createdAt: string; entry: Extract<Entry, { kind: 'checkpoint' }> }
  | { kind: 'setup'; id: string; createdAt: string; snapshot: Obj; embedded: boolean };

export interface RowInput {
  rows: Obj[]; runs: Obj[]; attempts: Obj[]; nodes: Obj[]; checkpoints: Obj[];
  isWorking: boolean; runningRunId: string; latestRun: Obj | null; activeStartedAt: string;
  /** A provider-native subagent's runless work is live (ChatView runlessWorkStartedAt). */
  runlessWorkActive?: boolean;
  /** The visible worktree setup of a new worktree thread (timeline-worktree.ts). */
  worktreeSetup?: Obj | null;
  expandedRuns: ReadonlySet<string>; expandedAttempts: ReadonlySet<string>; expandedGroups: ReadonlySet<string>;
  root: string; rollback: boolean;
}

const STANDALONE = new Set(['fork', 'handoff', 'run_interrupt_request', 'run_interrupt_result', 'subagent']);
const PERSISTENT = new Set(['fork', 'thread_created']);
export function rowKey(row: Obj): string { return JSON.stringify([row.sourceThreadId, row.sourceItemId]); }

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
      if (direct && direct.runId === item.runId) return { id: str(direct.id), runId: str(direct.runId), status: str(direct.status) };
      const node = nodes.get(nodeId);
      if (!node) return undefined;
      const root = byRoot.get(str(node.rootNodeId));
      if (root && root.runId === item.runId) return { id: str(root.id), runId: str(root.runId), status: str(root.status) };
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
    if (turnItemIsWorkspacePreparation(item) || type === 'todo_list') continue; // lane r11-upstream: a retried preparation's cancelled failure hides too
    if (type === 'user_message' && answered.has(str(item.messageId))) continue;
    const createdAt = str(item.startedAt ?? item.updatedAt), attempt = resolveAttempt(item);
    const base = { id, createdAt, row, ...(attempt ? { attempt } : {}) };
    if (type === 'user_message' || type === 'assistant_message') entries.push({ ...base, kind: 'message', item, role: type === 'user_message' ? 'user' : 'assistant' });
    else if (type === 'proposed_plan') entries.push({ ...base, kind: 'plan', item });
    else if (type === 'checkpoint') entries.push({ ...base, kind: 'checkpoint', item });
    else if (STANDALONE.has(type)) entries.push({ ...base, kind: 'event', item });
    else entries.push({ ...base, kind: 'work', entry: { ...projectedWorkEntry(row), id } });
  }
  return entries;
}

const runOf = (entry: Entry): string => entry.kind === 'message' ? entry.role === 'assistant' ? str(entry.item.runId) : ''
  : entry.kind === 'plan' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';
const startsResponse = (entry: Entry) => entry.kind === 'message' && entry.role === 'user' && entry.item.inputIntent !== 'steer'
  && entry.item.inputIntent !== 'promoted_queued_to_steer' || entry.kind === 'work' && entry.entry.itemType === 'notification';
const persistentResource = (entry: Entry) => entry.kind === 'event' && PERSISTENT.has(str(entry.item.type));
const failedItem = (entry: Entry): Obj | null => {
  const item = entry.kind === 'event' ? entry.item : entry.kind === 'work' ? entry.entry.item : null;
  return item?.type === 'error' && item.status === 'failed' && (item.parentItemId === null || item.parentItemId === undefined) ? item : null;
};
const messageEnd = (entry: Entry) => entry.kind === 'message' ? str(entry.item.updatedAt, entry.createdAt) : entry.createdAt;
const latestIso = (a: string | null, b: string | null) => !a ? b : !b ? a : Date.parse(b) > Date.parse(a) ? b : a;

/** deriveMessagesTimelineRows, without T3's tool catalogue, worktree setup or created-thread summaries. */
export function deriveRows(input: RowInput): Row[] {
  const entries = settleReasoning(timelineEntries(input));
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
  // Superseded attempts.
  const attemptGroups = new Map<string, Entry[]>();
  for (const entry of entries) {
    if (entry.attempt?.status !== 'superseded' || failedRuns.has(entry.attempt.runId) || entry.kind === 'message' && entry.role === 'user'
      || persistentResource(entry) || entry.kind === 'work' && entry.entry.itemType === 'system_notice') continue;
    attemptGroups.set(entry.attempt.id, [...attemptGroups.get(entry.attempt.id) ?? [], entry]);
  }
  const attemptFolds = new Map<string, { runId: string; attemptId: string; createdAt: string; hidden: Set<string> }>();
  for (const group of attemptGroups.values()) {
    const first = group[0]!;
    attemptFolds.set(first.id, { runId: first.attempt!.runId, attemptId: first.attempt!.id, createdAt: first.createdAt, hidden: new Set(group.map(entry => entry.id)) });
  }
  // Runs still forming the visible response.
  const boundary = findLastIndex(entries, startsResponse);
  const activeRuns = new Set<string>();
  if (unsettled) {
    activeRuns.add(unsettled);
    if (input.isWorking) for (const entry of entries.slice(boundary + 1)) { const run = runOf(entry); if (run) activeRuns.add(run); }
  }
  const runlessWorkActive = input.isWorking && input.runlessWorkActive === true;
  const folds = turnFolds(entries, terminalIds, latestRun, new Set([...activeRuns, ...failedRuns]), runlessWorkActive);
  const collapsed = new Set<string>();
  for (const fold of folds.values()) if (!input.expandedRuns.has(fold.runId)) fold.hidden.forEach(id => collapsed.add(id));
  const collapsedAttempts = new Set<string>();
  for (const fold of attemptFolds.values()) if (!input.expandedAttempts.has(fold.attemptId)) fold.hidden.forEach(id => collapsedAttempts.add(id));
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
        || ['system_notice', 'notification'].includes(entry.entry.itemType) || !inActive(entry.entry.runId)
        || entry.entry.sourceActivityKind === 'context-compaction' || collapsed.has(entry.id) || collapsedAttempts.has(entry.id)
        || folds.has(entry.id) || attemptFolds.has(entry.id)) break;
      const attempt = entry.attempt?.id ?? null;
      if (tailAttempt === undefined) tailAttempt = attempt; else if (attempt !== tailAttempt) break;
      activeTail.unshift(entry);
    }
  }
  const visibleTail = activeTail.filter(entry => visibleInGroup(entry.entry, true));
  const anchor = activeTail[0], latestVisible = last(visibleTail);
  const latestRunning = findLast(visibleTail, entry => entry.entry.status === 'inProgress');
  const keepsLive = !!latestRunning || !!latestVisible && successKeepsLive(latestVisible.entry);
  const latestFailed = !latestRunning && !!latestVisible && latestVisible.entry.status !== 'declined' && displayFailed(latestVisible.entry);
  const liveRow: Extract<Row, { kind: 'live' }> | null = anchor && latestVisible && !latestFailed ? (() => {
    const groupId = `work-group:${anchor.id}`, entry = (latestRunning ?? latestVisible).entry;
    return { kind: 'live', id: keepsLive ? 'live-activity-row' : `work-live:${anchor.id}`, createdAt: anchor.createdAt, entry,
      entries: visibleTail.map(item => item.entry), groupId, expanded: input.expandedGroups.has(groupId), active: keepsLive,
      label: liveEntryLabel(entry, input.root, keepsLive) };
  })() : null;
  const liveIds = new Set(liveRow || latestFailed ? activeTail.map(entry => entry.id) : []);
  const rows: Row[] = [];
  let activity = false, compacting = false;
  const pushLive = () => {
    if (!liveRow) return;
    rows.push(liveRow); activity ||= liveRow.active;
    if (liveRow.expanded) rows.push({ kind: 'details', id: `${liveRow.groupId}:details`, createdAt: liveRow.createdAt, entries: liveRow.entries });
  };
  const revertCounts = input.rollback ? revertTurnCounts(entries, input.checkpoints) : new Map<string, number>();

  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index]!;
    if (input.isWorking && index === headerIndex) rows.push({ kind: 'working', id: 'working-indicator-row', createdAt: input.activeStartedAt });
    if (entry.id === latestVisible?.id) pushLive();
    if (entry.kind === 'event' && entry.item.type === 'run_interrupt_request') continue;
    const fold = folds.get(entry.id);
    if (fold) rows.push({ kind: 'fold', id: `turn-fold:${fold.runId}`, createdAt: fold.createdAt, runId: fold.runId, label: fold.label, expanded: input.expandedRuns.has(fold.runId) });
    if (collapsed.has(entry.id)) continue;
    const attemptFold = attemptFolds.get(entry.id);
    if (attemptFold) rows.push({ kind: 'attempt', id: `attempt-fold:${attemptFold.attemptId}`, createdAt: attemptFold.createdAt, runId: attemptFold.runId,
      attemptId: attemptFold.attemptId, expanded: input.expandedAttempts.has(attemptFold.attemptId) });
    if (collapsedAttempts.has(entry.id) || liveIds.has(entry.id)) continue;
    if (entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction') {
      const active = entryActive(entry.entry); compacting ||= active;
      rows.push({ kind: 'compaction', id: entry.id, createdAt: entry.createdAt, label: entry.entry.label, active });
      continue;
    }
    if (entry.kind === 'work') {
      const work = entry.entry;
      if (work.runId && failedRuns.has(work.runId) || work.itemType === 'error' && work.status === 'failed' || work.tone === 'error'
        || work.sourceActivityKind === 'runtime.error' || work.itemType === 'system_notice' || work.itemType === 'notification') {
        rows.push({ kind: 'entry', id: entry.id, createdAt: entry.createdAt, entries: [work] });
        continue;
      }
      const grouped = [work];
      let cursor = index + 1;
      while (cursor < entries.length) {
        const next = entries[cursor]!;
        if (next.kind !== 'work' || next.entry.sourceActivityKind === 'context-compaction' || next.entry.tone === 'error'
          || next.entry.sourceActivityKind === 'runtime.error' || ['system_notice', 'notification'].includes(next.entry.itemType)
          || liveIds.has(next.id) || collapsed.has(next.id) || collapsedAttempts.has(next.id) || folds.has(next.id) || attemptFolds.has(next.id)
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
          rows.push({ kind: 'group', id: `work-toggle:${entry.id}`, createdAt: entry.createdAt, runId: work.runId, groupId, count: visible.length, expanded,
            summary: singleLabel ? singleToolCallLabel(single, input.root) : single !== null && !isToolLike(single) ? single.label : summarizeToolGroup(visible).summary,
            icon: singleLabel ? entryIcon(single) : summaryIcon(summaryKind(visible)), failed: !!latestTool && displayFailed(latestTool) });
          if (expanded) rows.push({ kind: 'details', id: `${groupId}:details`, createdAt: entry.createdAt, entries: visible });
        }
      }
      index = cursor - 1;
      continue;
    }
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
    const inProgress = entry.role === 'assistant' && inActive(str(entry.item.runId));
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
  if (input.isWorking && !rows.some(row => row.kind === 'working') && headerIndex === entries.length) {
    rows.push({ kind: 'working', id: 'working-indicator-row', createdAt: input.activeStartedAt });
  }
  // A setup that owns the working slot shows no activity row of its own.
  if (input.isWorking && !setupOwnsWorkingSlot && !compacting && (!activity || latestFailed)) {
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

function settleReasoning(entries: Entry[]): Entry[] {
  return entries.map((entry, index) => index === entries.length - 1 || entry.kind !== 'work' || entry.entry.itemType !== 'reasoning'
    || entry.entry.status !== 'inProgress' ? entry : { ...entry, entry: { ...entry.entry, status: 'completed' } });
}

interface Fold { runId: string; createdAt: string; hidden: Set<string>; label: string }
/**
 * deriveTurnFolds. A prompt without a run (a provider-native subagent, or a
 * turn imported from V1) lends its response a key of its own, decided per
 * prompt so a V1 thread's first V2 run does not unfold every imported turn.
 */
function turnFolds(entries: Entry[], terminalIds: Set<string>, latestRun: Obj | null, unfolded: Set<string>, runlessWorkActive: boolean): Map<string, Fold> {
  const interrupted = new Set(entries.flatMap(entry => entry.kind === 'event' && entry.item.runId
    && (entry.item.type === 'run_interrupt_request' || entry.item.type === 'run_interrupt_result') ? [str(entry.item.runId)] : []));
  interface Group { entries: Entry[]; terminal: Entry | null; streaming: boolean; start: string | null; anchor: string }
  const groups = new Map<string, Group>(), runlessFailed = new Set<string>();
  let runless: string | null = null, pending: { createdAt: string; anchor: string } | null = null;
  entries.forEach((entry, index) => {
    if (startsResponse(entry)) {
      const next = entries[index + 1];
      pending = next ? { createdAt: entry.createdAt, anchor: next.id } : null;
      const boundaryRun = entry.kind === 'message' ? str(entry.item.runId) : entry.kind === 'work' ? entry.entry.runId : '';
      runless = boundaryRun ? null : `runless:${entry.id}`;
      return;
    }
    let runId: string | null = null;
    if (entry.kind === 'work' && entry.entry.itemType === 'system_notice') runId = null;
    else if (entry.kind === 'message' && entry.role === 'assistant' || entry.kind === 'work') runId = runOf(entry) || runless;
    else if (entry.kind === 'event' && (persistentResource(entry) || entry.item.type === 'subagent')) runId = str(entry.item.runId) || runless;
    if (!runId) return;
    if (runId === runless && failedItem(entry)) runlessFailed.add(runId);
    let group = groups.get(runId);
    if (!group) {
      const start: { createdAt: string; anchor: string } | null = pending;
      group = { entries: [], terminal: null, streaming: false, start: start?.createdAt ?? null, anchor: start?.anchor ?? entry.id };
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
      const compaction = entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction';
      const trailing = entry.kind === 'work' && entry.entry.status !== 'inProgress' && !displayFailed(entry.entry);
      if (!compaction && index > terminalIndex && !trailing) return;
      if (persistentResource(entry) || entry.kind === 'work' && entry.entry.itemType === 'notification') return;
      hidden.add(entry.id);
    });
    if (!hidden.size || !group.entries.some(entry => hidden.has(entry.id) && !(entry.kind === 'work' && entry.entry.sourceActivityKind === 'context-compaction'))) continue;
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
    result.set(group.anchor, { runId, createdAt: group.start ?? first.createdAt, hidden, label });
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

/** session-logic deriveRevertTurnCountByUserMessageId: a started turn with a ready checkpoint can be edited. */
export function revertTurnCounts(entries: Entry[], checkpoints: Obj[]): Map<string, number> {
  const ready = new Map<string, Obj>();
  for (const checkpoint of checkpoints) if (checkpoint.status === 'ready' && checkpoint.runId) ready.set(str(checkpoint.runId), checkpoint);
  const counts = new Map<string, number>();
  for (const entry of entries) {
    if (entry.kind !== 'message' || entry.role !== 'user' || !['turn_start', 'queued_turn'].includes(str(entry.item.inputIntent)) || !entry.item.runId) continue;
    const checkpoint = ready.get(str(entry.item.runId));
    if (checkpoint && typeof checkpoint.appRunOrdinal === 'number') counts.set(str(entry.item.messageId), Math.max(0, checkpoint.appRunOrdinal - 1));
  }
  return counts;
}

export function runList(projection: Obj): Obj[] { return arr(projection.runs); }
