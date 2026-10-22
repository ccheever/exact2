// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/shell-lineage.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The thread details panel's Lineage section (MIT reference, see LICENSE-T3:
// packages/client-runtime/src/state/threadRelationships.ts, threadWorkflows.ts
// resolveLatestMergeBackRun, apps/web/src/components/chat/
// ThreadRelationshipsControl.tsx, ThreadRelationshipIcon.tsx, AgentElapsed.tsx;
// upstream d3071275d5): the open thread's immediate relationships (fork parent,
// forks, subagents, context transfers) from the thread shells and its own
// projection. A settled subagent whose child thread has a live run shows that
// run; the heading counts what is running.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { subagentTitle } from './timeline-events';

type Kind = 'fork' | 'subagent' | 'transfer';
type Edge = { source: string; target: string; kind: Kind; status: string | null };
type Node = { id: string; thread: Obj | null; missing: boolean };
type Graph = { nodes: Map<string, Node>; edges: Edge[] };
export type LineageRow = {
  id: string; title: string; icon: string; driver: string; status: string; dot: string; group: string; index: number;
  mergeTarget: boolean; elapsed: string; agent: boolean; missing: boolean; hint: string;
};

const SETTLED_AGENT = new Set(['completed', 'failed', 'error', 'cancelled', 'interrupted', 'idle']);
const LIVE_WORK = new Set(['pending', 'running', 'waiting']);
export const LINEAGE_INITIAL_COUNT = 6;
export const LINEAGE_PAGE_COUNT = 12;

const parentOf = (thread: Obj) => obj(thread.forkedFrom).type === 'run' ? str(obj(thread.forkedFrom).threadId) : str(obj(thread.lineage).parentThreadId);
const statusOf = (thread: Obj | null | undefined) => thread ? (str(thread.activityRunStatus) || str(thread.status) || null) : null;

/** deriveThreadRelationshipGraph: shells first (most authoritative), then the open thread's projection. */
export function relationshipGraph(threads: Obj[], projection: Obj | null): Graph {
  const byId = new Map<string, Obj>();
  for (const thread of threads) if (str(thread.id) && !byId.has(str(thread.id))) byId.set(str(thread.id), thread);
  const nodes = new Map<string, Node>([...byId.values()].map(thread => [str(thread.id), { id: str(thread.id), thread, missing: false }]));
  const edges = new Map<string, Edge>();
  const add = (edge: Edge) => {
    for (const id of [edge.source, edge.target]) if (!nodes.has(id)) nodes.set(id, { id, thread: null, missing: true });
    edges.set(`${edge.source}\u001f${edge.target}\u001f${edge.kind}`, edge);
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
      // A live run on the child thread outranks the settled task's status.
      add({ source: owner, target: child, kind: 'subagent', status: str(byId.get(child)?.activityRunStatus) || str(agent.status) || null });
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
  return 'light-dark(#71717b73, #81818173)';
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

/** liveSubagent: a child thread's live run replaces the settled task's timer and output. */
export function liveSubagent(agent: Obj | undefined, child: Obj | null | undefined): Obj | undefined {
  const live = str(child?.activityRunStatus);
  if (!agent || !live) return agent;
  return { ...agent, status: live === 'running' || live === 'waiting' ? live : 'pending', startedAt: child?.activityRunStartedAt ? str(child.activityRunStartedAt) : null,
    completedAt: null, progress: null, result: null, error: null };
}

/** deriveSubagentElapsedMs, formatted; '' when the agent has no start (or a settled one no end). */
export function agentElapsed(agent: Obj, now: number): string {
  if (!agent.startedAt) return '';
  const end = LIVE_WORK.has(str(agent.status)) ? now : agent.completedAt ? Date.parse(str(agent.completedAt)) : NaN;
  const start = Date.parse(str(agent.startedAt));
  if (!Number.isFinite(start) || !Number.isFinite(end)) return '';
  return formatElapsed(Math.max(0, end - start) / 1000);
}

const createdAt = (node: Node | undefined) => { const at = Date.parse(str(node?.thread?.createdAt)); return Number.isFinite(at) ? at : null; };

/** The Lineage section: heading, ordered and grouped rows, and the hidden "Previous agents" count. */
export function lineageView(threads: Obj[], projection: Obj | null, threadId: string, providers: Obj[], now: number) {
  const graph = relationshipGraph(threads, projection);
  const mergeTarget = mergeTargetOf(projection);
  const agents = new Map(arr(projection?.subagents).filter(agent => str(agent.childThreadId)).map(agent => [str(agent.childThreadId), agent]));
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
    const status = rowStatus(graph, { threadId: id, edge });
    const agent = liveSubagent(subagent && !parent ? agents.get(id) : undefined, thread);
    const provider = providers.find(entry => str(entry.instanceId) === (str(agent?.providerInstanceId) || str(thread?.providerInstanceId)));
    const driver = subagent && !parent ? str(agent?.driver) || str(provider?.driver) : '';
    const relationship = edge.kind === 'transfer' ? 'Context transfer' : subagent ? (edge.source === threadId ? 'Subagent' : 'Parent agent') : (edge.source === threadId ? 'Fork' : 'Parent thread');
    const rawTitle = str(thread?.title) || str(agent?.title) || id;
    const group = groupOf(edge);
    return {
      id, title: subagent ? subagentTitle(rawTitle) : rawTitle, icon: parent ? 'corner-left-up' : subagent ? 'bot' : 'git-fork', driver,
      status: statusLabel(status), dot: statusDot(status), group, index: counters[group]!++, mergeTarget: id === mergeTarget,
      elapsed: agent ? agentElapsed(agent, now) : '', agent: !!agent, missing: node?.missing === true,
      hint: node?.missing ? 'This related thread is unavailable' : `Open ${relationship.toLowerCase()} in this chat`,
    };
  });
  // Subagents without a child thread yet have no row, so they are counted on their own.
  const running = arr(projection?.subagents).filter(agent => !str(agent.childThreadId) && agent.status === 'running').length
    + ordered.filter(({ edge }) => groupOf(edge) === 'active' && edge.status === 'running').length;
  const previous = rows.filter(row => row.group === 'previous');
  const mergeRun = latestMergeBackRun(projection);
  const parentTitle = mergeTarget ? str(graph.nodes.get(mergeTarget)?.thread?.title) : '';
  return {
    lineageTitle: running > 0 ? `Lineage · ${running} running` : 'Lineage', lineage: rows, showLineage: rows.length > 0 || running > 0,
    previousCount: previous.length, previousFailed: ordered.filter(({ edge }) => groupOf(edge) === 'previous' && (edge.status === 'failed' || edge.status === 'error')).length,
    mergeRunId: mergeRun ? str(mergeRun.id) : '', mergeTargetId: mergeTarget, mergeSourceId: mergeTarget ? threadId : '',
    mergeLabel: parentTitle ? `Merge back to ${parentTitle}` : 'Merge back to source conversation',
    mergeHint: !mergeRun ? 'Complete a run in this fork before merging it back' : parentTitle ? `Merge this conversation back into ${parentTitle}` : 'Merge this conversation back into its source',
  };
}
