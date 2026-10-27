// The thread details panel's Lineage section (MIT reference, see LICENSE-T3:
// packages/client-runtime/src/state/threadRelationships.ts, threadWorkflows.ts
// resolveLatestMergeBackRun, apps/web/src/components/chat/
// ThreadRelationshipsControl.tsx, ThreadRelationshipIcon.tsx, AgentElapsed.tsx;
// upstream 74541bab17): the open thread's immediate relationships (fork parent,
// forks, subagents, context transfers) from the thread shells and its own
// projection. A settled subagent whose child thread has a live run shows that
// run, while a subagent still at work (a supervisor waiting on its worker) keeps
// its own status; a provider-native parent shows its root turn's status
// (lineage-parent.ts); the heading counts what is running. A row ends in one
// item: an agent's compact elapsed time, else its status; a running T3-owned
// subagent trades it for Stop on hover.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { subagentTitle } from '../timeline/timeline-events';
import { canStopNativeSubagent, canStopWorkflow, projectedWorkflowAgents, workflowBody, workflowGroups } from './lineage-workflows';

type Kind = 'fork' | 'subagent' | 'transfer';
type Edge = { source: string; target: string; kind: Kind; status: string | null };
type Node = { id: string; thread: Obj | null; missing: boolean };
type Graph = { nodes: Map<string, Node>; edges: Edge[] };
export type LineageRow = {
  id: string; title: string; icon: string; driver: string; status: string; dot: string; group: string; index: number;
  mergeTarget: boolean; elapsed: string; agent: boolean; missing: boolean; hint: string; failed: boolean; canStop: boolean;
  kind: string; workflowId: string; expanded: boolean; phaseId: string; phaseOpen: boolean;
  stopOp: string; stopTarget: string; count: string; modelLine: string; preview: string;
};

const SETTLED_AGENT = new Set(['completed', 'failed', 'error', 'cancelled', 'interrupted', 'idle']);
const LIVE_WORK = new Set(['pending', 'running', 'waiting']);
/** isOrchestrationV2WorkActive. */
const workActive = (status: unknown) => LIVE_WORK.has(str(status));
export const LINEAGE_INITIAL_COUNT = 6;
export const LINEAGE_PAGE_COUNT = 12;

export const parentOf = (thread: Obj) => obj(thread.forkedFrom).type === 'run' ? str(obj(thread.forkedFrom).threadId) : str(obj(thread.lineage).parentThreadId);
const statusOf = (thread: Obj | null | undefined) => thread ? (str(thread.activityRunStatus) || str(thread.status) || null) : null;

/** deriveThreadRelationshipGraph: shells first (most authoritative), then the open thread's projection. */
export function relationshipGraph(threads: Obj[], projection: Obj | null): Graph {
  const byId = new Map<string, Obj>();
  for (const thread of threads) if (str(thread.id) && !byId.has(str(thread.id))) byId.set(str(thread.id), thread);
  const nodes = new Map<string, Node>([...byId.values()].map(thread => [str(thread.id), { id: str(thread.id), thread, missing: false }]));
  const edges = new Map<string, Edge>();
  const add = (edge: Edge) => {
    for (const id of [edge.source, edge.target]) if (!nodes.has(id)) nodes.set(id, { id, thread: null, missing: true });
    edges.set(`${edge.source}\u001f${edge.target}\u001f${edge.kind}`, edge.kind === 'subagent' && edge.status === 'rolled_back' ? { ...edge, status: 'interrupted' } : edge);
  };
  for (const thread of byId.values()) {
    const parent = parentOf(thread);
    if (!parent) continue;
    add({ source: parent, target: str(thread.id), kind: obj(thread.lineage).relationshipToParent === 'subagent' ? 'subagent' : 'fork', status: statusOf(thread) });
  }
  if (projection) {
    const owner = str(obj(projection.thread).id);
    for (const agent of arr(projection.subagents)) {
      const child = str(agent.childThreadId);
      if (!owner || !child) continue;
      // The row describes the child conversation, including queued follow-ups and their outcomes;
      // a child without an app run, or a task still at work, uses the task.
      const shell = byId.get(child);
      add({ source: owner, target: child, kind: 'subagent',
        status: str(shell?.activityRunStatus) || (shell?.latestRunId != null && !workActive(agent.status) ? str(shell.status) : str(agent.status)) || null });
    }
    for (const transfer of arr(projection.contextTransfers)) {
      const source = str(transfer.sourceThreadId), target = str(transfer.targetThreadId);
      if (!source || !target || source === target) continue;
      add({ source, target, kind: 'transfer', status: str(transfer.status) || null });
    }
  }
  return { nodes, edges: [...edges.values()] };
}

/** immediateThreadRelationships: one row per related thread, the first edge wins. */
export function immediateRelationships(graph: Graph, threadId: string): { threadId: string; edge: Edge }[] {
  const seen = new Set<string>(), rows: { threadId: string; edge: Edge }[] = [];
  for (const edge of graph.edges) {
    const related = edge.source === threadId ? edge.target : edge.target === threadId ? edge.source : '';
    if (!related || seen.has(related)) continue;
    seen.add(related);
    rows.push({ threadId: related, edge });
  }
  return rows;
}

export const isParentRelationship = (edge: Edge, current: string) => edge.kind !== 'transfer' && edge.target === current;

/** threadRelationshipRowStatus: an incoming parent row shows its own activity. */
export function rowStatus(graph: Graph, row: { threadId: string; edge: Edge }): string | null {
  if (row.edge.kind === 'transfer' || row.threadId === row.edge.target) return row.edge.status;
  return statusOf(graph.nodes.get(row.threadId)?.thread);
}

/** threadRelationshipStatusLabel. */
export function statusLabel(status: string | null): string {
  switch (status) {
    case 'preparing': case 'starting': return 'Starting';
    case 'running': case 'in_progress': return 'Running';
    case 'pending': case 'queued': return 'Queued';
    case 'waiting': case 'blocked': return 'Waiting';
    case 'completed': return 'Done';
    case 'failed': case 'error': return 'Failed';
    case 'cancelled': case 'interrupted': return 'Stopped';
    case 'rolled_back': return 'Reverted';
    case 'resolved_native': return 'Resolved (native)';
    case 'resolved_portable': return 'Resolved (portable)';
    case 'consumed': return 'Consumed';
    case 'superseded': return 'Superseded';
    case 'idle': return 'Idle';
    default: return 'Unknown';
  }
}

/** ThreadRelationshipIcon's status badge: info, destructive, success, or muted/45. */
export function statusDot(status: string | null): string {
  if (status === 'running' || status === 'in_progress' || status === 'pending' || status === 'waiting') return 'light-dark(#2b7fff, #51a2ff)';
  if (status === 'failed' || status === 'error') return 'light-dark(#fb2c36, #fb414a)';
  if (status === 'completed') return '#00bc7d';
  return 'light-dark(#71717b73, #a1a1a173)';
}

/** resolveMergeBackTargetThreadId. */
export function mergeTargetOf(projection: Obj | null): string {
  const thread = obj(projection?.thread);
  if (!projection || obj(thread.lineage).relationshipToParent !== 'fork') return '';
  return parentOf(thread);
}

/** resolveLatestMergeBackRun: the newest waiting/completed run, unless a newer run is active. */
export function latestMergeBackRun(projection: Obj | null): Obj | null {
  if (!projection) return null;
  const runs = arr(projection.runs);
  const latest = runs.reduce<Obj | null>((best, run) => ['waiting', 'completed'].includes(str(run.status)) && (best === null || Number(run.ordinal) > Number(best.ordinal)) ? run : best, null);
  if (!latest) return null;
  return runs.some(run => Number(run.ordinal) > Number(latest.ordinal) && ['preparing', 'starting', 'running'].includes(str(run.status))) ? null : latest;
}

/** AgentElapsed's formatElapsedSeconds. */
export function formatElapsed(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds)), minutes = Math.floor(seconds / 60);
  if (minutes === 0) return `${seconds}s`;
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return `${minutes}m ${String(seconds % 60).padStart(2, '0')}s`;
  return `${hours}h ${String(minutes % 60).padStart(2, '0')}m`;
}

/** formatCompactElapsedSeconds: "45s", "12m", "1.5h", "14h" for tight rows; rounds down. */
export function formatCompactElapsed(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  if (minutes >= 600) return `${Math.floor(minutes / 60)}h`;
  return `${Math.floor(minutes / 6) / 10}h`;
}

/**
 * currentSubagent: the row's timer and hover card follow the child's current work, including a
 * queued follow-up whose provider turn has not started; the task's output stays with its own run.
 * A task still at work is current as it is.
 */
export function currentSubagent(agent: Obj | undefined, child: Obj | null | undefined): Obj | undefined {
  if (!agent || !child || workActive(agent.status)) return agent;
  const live = str(child.activityRunStatus) || (child.status === 'queued' ? 'queued' : '');
  const requestedAt = Date.parse(str(child.latestRunRequestedAt));
  const newerRun = Number.isFinite(requestedAt) && requestedAt > Date.parse(str(agent.completedAt ?? agent.startedAt ?? agent.updatedAt));
  if (!live && !newerRun) return agent;
  const status = live || str(child.status);
  const startedAt = live ? child.activityRunStartedAt : child.latestRunStartedAt, completedAt = live ? null : child.latestRunCompletedAt;
  return { ...agent, status: status === 'preparing' || status === 'starting' || status === 'queued' ? 'pending' : status === 'rolled_back' ? 'interrupted' : status,
    startedAt: startedAt ? str(startedAt) : null, completedAt: completedAt ? str(completedAt) : null, progress: null, result: null, error: null };
}

/** deriveSubagentElapsedMs, formatted (`compact` for Lineage's rows); '' when the agent has no start (or a settled one no end). */
export function agentElapsed(agent: Obj, now: number, compact = false): string {
  if (!agent.startedAt) return '';
  const end = LIVE_WORK.has(str(agent.status)) ? now : agent.completedAt ? Date.parse(str(agent.completedAt)) : NaN;
  const start = Date.parse(str(agent.startedAt));
  if (!Number.isFinite(start) || !Number.isFinite(end)) return '';
  return (compact ? formatCompactElapsed : formatElapsed)(Math.max(0, end - start) / 1000);
}

/** A running subagent this app started (not one the provider runs natively) stops by interrupting its child thread's turn. */
export const canStopSubagent = (agent: Obj | undefined) => !!agent && agent.origin === 'app_owned' && !!agent.startedAt && LIVE_WORK.has(str(agent.status));

const createdAt = (node: Node | undefined) => { const at = Date.parse(str(node?.thread?.createdAt)); return Number.isFinite(at) ? at : null; };

/**
 * The Lineage section: heading, ordered and grouped rows, and the hidden "Previous agents" count.
 * `nativeParent`: the open thread's provider-native parent and its root turn's status (lineage-parent.ts).
 */
export function lineageView(threads: Obj[], projection: Obj | null, threadId: string, providers: Obj[], now: number,
  expanded: string[] = [], phasesOpen: string[] = [], phasesClosed: string[] = [], writable = true,
  nativeParent: { threadId: string; status: string } | null = null) {
  const graph = relationshipGraph(threads, projection);
  const mergeTarget = mergeTargetOf(projection);
  const agents = new Map(arr(projection?.subagents).filter(agent => str(agent.childThreadId)).map(agent => [str(agent.childThreadId), agent]));
  const workflows = new Map(workflowGroups(projectedWorkflowAgents(arr(projection?.subagents))).map(group => [group.workflow.id, group]));
  // A workflow coordinator owns its phases; a child conversation's follow-up does not change that workflow's status.
  for (const edge of graph.edges) {
    const raw = agents.get(edge.target);
    if (edge.kind === 'subagent' && raw && workflows.has(str(raw.id))) edge.status = str(raw.status);
  }
  const pin = (row: { threadId: string; edge: Edge }) => isParentRelationship(row.edge, threadId) ? 0 : row.threadId === mergeTarget ? 1 : 2;
  // orderWebThreadLineageRows: parent, merge target, then newest first; missing shells sink.
  const ordered = immediateRelationships(graph, threadId).sort((left, right) => {
    const rank = pin(left) - pin(right);
    if (rank) return rank;
    const a = createdAt(graph.nodes.get(left.threadId)), b = createdAt(graph.nodes.get(right.threadId));
    if (a !== b) { if (a === null) return 1; if (b === null) return -1; return b - a; }
    return left.threadId < right.threadId ? -1 : left.threadId > right.threadId ? 1 : 0;
  });
  const groupOf = (edge: Edge) => edge.kind !== 'subagent' || isParentRelationship(edge, threadId) ? 'related' : SETTLED_AGENT.has(edge.status ?? '') ? 'previous' : 'active';
  const counters: Record<string, number> = { related: 0, active: 0, previous: 0 };
  const rows: LineageRow[] = ordered.map(({ threadId: id, edge }) => {
    const node = graph.nodes.get(id), thread = node?.thread ?? null;
    const subagent = edge.kind === 'subagent', parent = isParentRelationship(edge, threadId);
    // A native coordinator has no app run of its own: its row reads its root turn, unless a follow-up run is live.
    const status = (parent && id === nativeParent?.threadId ? str(thread?.activityRunStatus) || nativeParent.status : '')
      || rowStatus(graph, { threadId: id, edge });
    const raw = subagent && !parent ? agents.get(id) : undefined;
    const workflow = raw ? workflows.get(str(raw.id)) : undefined;
    const agent = workflow ? raw : currentSubagent(raw, thread);
    const provider = providers.find(entry => str(entry.instanceId) === (str(agent?.providerInstanceId) || str(thread?.providerInstanceId)));
    const driver = subagent && !parent ? str(agent?.driver) || str(provider?.driver) : '';
    const relationship = edge.kind === 'transfer' ? 'Context transfer' : subagent ? (edge.source === threadId ? 'Subagent' : 'Parent agent') : (edge.source === threadId ? 'Fork' : 'Parent thread');
    const rawTitle = workflow?.workflow.workflowName ?? (str(thread?.title) || str(agent?.title) || id);
    const group = groupOf(edge), failed = status === 'failed' || status === 'error';
    return {
      id, title: subagent ? subagentTitle(rawTitle) : rawTitle, icon: parent ? 'corner-left-up' : subagent ? 'bot' : 'git-fork', driver,
      status: statusLabel(status), dot: statusDot(status), group, index: counters[group]!++, mergeTarget: id === mergeTarget,
      // One trailing item keeps room for the title: the badge already carries status, so a timed agent shows its time.
      elapsed: agent && !failed ? agentElapsed(agent, now, true) : '', agent: !!agent, missing: node?.missing === true,
      hint: node?.missing ? 'This related thread is unavailable' : `Open ${relationship.toLowerCase()} in this chat`,
      failed, canStop: writable && (workflow ? canStopWorkflow(workflow.workflow, writable, driver) : canStopSubagent(agent) || canStopNativeSubagent(agent, raw)),
      kind: workflow ? 'workflow' : 'thread', workflowId: workflow?.workflow.id ?? '', expanded: !!workflow && expanded.includes(workflow.workflow.id),
      phaseId: '', phaseOpen: false, stopOp: workflow ? 'shell:stop-workflow' : 'shell:stop-subagent',
      stopTarget: workflow?.workflow.id ?? id, count: '', modelLine: '', preview: '',
    };
  });
  // Subagents without a child thread yet have no row, so they are counted on their own.
  const running = arr(projection?.subagents).filter(agent => !str(agent.childThreadId) && agent.status === 'running').length
    + ordered.filter(({ edge }) => groupOf(edge) === 'active' && edge.status === 'running').length;
  const previous = rows.filter(row => row.group === 'previous');
  // Expansion is flattened before Contract renders it: phase and member rows share their header's pagination index.
  const lineage = rows.flatMap(row => {
    const group = workflows.get(row.workflowId);
    if (!group || !row.expanded) return [row];
    const body = workflowBody(group, threads, providers, now);
    const memberRow = (member: ReturnType<typeof workflowBody>['unphased'][number]): LineageRow => ({
      ...row, id: member.id, title: member.title, kind: 'member', workflowId: row.workflowId, icon: 'bot', driver: member.driver,
      status: member.status, dot: member.dot, elapsed: member.elapsed, missing: member.disabled, failed: member.card.tone === 'error',
      canStop: false, mergeTarget: false, stopTarget: '', hint: member.childThreadId, modelLine: member.card.modelLine, preview: member.card.preview,
    });
    const entries: LineageRow[] = [row];
    for (const phase of body.phases) {
      const phaseOpen = phasesOpen.includes(phase.id) || (!phasesClosed.includes(phase.id) && phase.running);
      entries.push({ ...row, id: phase.id, title: phase.title, kind: 'phase', phaseId: phase.id, phaseOpen, canStop: false,
        mergeTarget: false, elapsed: phase.elapsed, count: phase.count, status: phase.status, failed: phase.failed });
      if (phaseOpen) entries.push(...phase.members.map(memberRow));
    }
    entries.push(...body.unphased.map(memberRow));
    if (body.empty) entries.push({ ...row, id: `${row.workflowId}:empty`, kind: 'empty', title: 'No agents yet', canStop: false });
    return entries;
  });
  const mergeRun = latestMergeBackRun(projection);
  const parentTitle = mergeTarget ? str(graph.nodes.get(mergeTarget)?.thread?.title) : '';
  return {
    lineageTitle: running > 0 ? `Lineage · ${running} running` : 'Lineage', lineage, showLineage: rows.length > 0 || running > 0,
    previousCount: previous.length, previousFailed: ordered.filter(({ edge }) => groupOf(edge) === 'previous' && (edge.status === 'failed' || edge.status === 'error')).length,
    mergeRunId: mergeRun ? str(mergeRun.id) : '', mergeTargetId: mergeTarget, mergeSourceId: mergeTarget ? threadId : '',
    mergeLabel: parentTitle ? `Merge back to ${parentTitle}` : 'Merge back to source conversation',
    mergeHint: !mergeRun ? 'Complete a run in this fork before merging it back' : parentTitle ? `Merge this conversation back into ${parentTitle}` : 'Merge this conversation back into its source',
  };
}
