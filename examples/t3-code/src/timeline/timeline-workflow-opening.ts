// A workflow coordinator's thread opens with the workflow's phases and agents in place of its script, adapted from
// T3 Code 74541bab17 (MIT, see LICENSE-T3): chat/WorkflowOpeningBlock.tsx, ChatView.tsx (workflowOpening) and
// client-runtime state/subagentRuntime.ts (isWorkflowScriptMessage, resolveWorkflowOpening,
// workflowOpeningParentThreadId, workflowOpeningHeadline). The coordinator row lives on the parent thread: its detail is
// the kept one while it runs (keep-alive.ts), and otherwise a bounded snapshot fetched before the snapshot that needs it.
import type { T3Client } from '../client/client';
import { arr, obj, str, threadSnapshot, type Activity, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { keptThread } from '../client/keep-alive';
import { letGo } from '../client/let-go';
import { isProviderSubagent } from '../composer/composer-controls-subagent';
import { composerNow } from '../composer/composer-controls';
import { projectedWorkflowAgents, terminalWorkflowAgent, workflowGroups, type WorkflowAgent, type WorkflowGroup } from '../details/lineage-workflows';
import { shouldCollapseUserMessage } from './message-chips';
import { subagent, type SubagentContext } from './timeline-events';
import { literalFindRanges } from './thread-find';

// Workflow scripts open with their `meta` export, after any leading comments.
const WORKFLOW_SCRIPT_START = /^\s*(?:(?:\/\/[^\n]*|\/\*[\s\S]*?\*\/)\s*)*export\s+const\s+meta\b/;

/** A user message that is the script a provider-native workflow runs; only threads recorded before the mark read the text. */
export function isWorkflowScriptMessage(message: Obj): boolean {
  return message.role === 'user' && (typeof message.workflowScript === 'boolean' ? message.workflowScript : WORKFLOW_SCRIPT_START.test(str(message.text)));
}
const firstUserMessage = (projection: Obj) => arr(projection.messages).find(message => message.role === 'user');

/** The parent whose subagent rows describe this thread's workflow, or null when it does not open with a workflow script. */
export function workflowOpeningParentThreadId(projection: Obj): string | null {
  const thread = obj(projection.thread), first = firstUserMessage(projection);
  if (!isProviderSubagent(thread) || !first || !isWorkflowScriptMessage(first)) return null;
  return str(obj(thread.lineage).parentThreadId) || null;
}

export interface WorkflowOpening {
  messageId: string; script: string; name: string | null; description: string | null; driver: string; providerInstanceId: string;
  /** Null until the parent thread's coordinator row is loaded. */
  group: WorkflowGroup | null;
}
/** resolveWorkflowOpening: the first user message read as the workflow's plan; `parentSubagents` null while the parent loads. */
export function resolveWorkflowOpening(projection: Obj, parentSubagents: Obj[] | null): WorkflowOpening | null {
  const thread = obj(projection.thread);
  if (!isProviderSubagent(thread)) return null;
  const first = firstUserMessage(projection);
  if (!first || first.workflowScript === false) return null;
  const coordinator = parentSubagents?.find(entry => entry.childThreadId === thread.id);
  if (coordinator?.workflow == null && !isWorkflowScriptMessage(first)) return null;
  return { messageId: str(first.id), script: str(first.text), name: coordinator?.workflow == null ? null : str(obj(coordinator.workflow).name) || null,
    description: typeof coordinator?.title === 'string' ? coordinator.title : null, driver: str(coordinator?.driver), providerInstanceId: str(coordinator?.providerInstanceId),
    group: coordinator?.workflow == null ? null : workflowGroups(projectedWorkflowAgents([coordinator]))[0] ?? null };
}

/** workflowOpeningHeadline: the workflow, what it does, and how its agents stand. */
export function workflowOpeningHeadline(opening: WorkflowOpening): { title: string; description: string | null; agents: string | null } {
  const title = opening.name ?? opening.description ?? 'Workflow';
  const members = opening.group ? [...opening.group.phases.flatMap(phase => phase.members), ...opening.group.unphasedMembers] : [];
  const counts = new Map<string, number>();
  for (const member of members) {
    const key = terminalWorkflowAgent(member.status) ? member.status === 'completed' ? 'done' : member.status === 'failed' ? 'failed' : 'stopped' : 'working';
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const agents = members.length === 0 ? null
    : `${members.length} ${members.length === 1 ? 'agent' : 'agents'} · ${['working', 'done', 'failed', 'stopped'].filter(key => counts.has(key)).map(key => `${counts.get(key)} ${key}`).join(' · ')}`;
  return { title, description: opening.description !== null && opening.description !== title ? opening.description : null, agents };
}

/** The script as a fenced block, so it renders like any other code in the thread. */
export function scriptMarkdown(script: string): string {
  const longest = Math.max(2, ...(script.match(/`+/g) ?? []).map(run => run.length));
  const fence = '`'.repeat(longest + 1);
  return `${fence}javascript\n${script.trimEnd()}\n${fence}`;
}

/**
 * WorkflowMemberRow as the timeline's subagent row: its avatar is the coordinator's provider, its line the error, result or
 * progress of a settled member and the progress of a live one. Phase headings ride the same list (`tone` "phase": `label`
 * the title, `result` settled/total, `detail` "first" on the first); `tone` "gap" separates the unphased members.
 */
export function workflowOpeningRows(opening: WorkflowOpening, context: SubagentContext): Activity[] {
  if (!opening.group || !opening.providerInstanceId) return [];
  const member = (agent: WorkflowAgent): Activity => {
    const settled = terminalWorkflowAgent(agent.status);
    return subagent({ id: agent.id, title: agent.title, status: agent.status, result: settled ? agent.error ?? agent.result ?? agent.progress : null,
      progress: settled ? null : agent.progress, startedAt: agent.startedAt, completedAt: agent.completedAt, childThreadId: agent.childThreadId,
      model: agent.model, driver: opening.driver, providerInstanceId: opening.providerInstanceId, origin: 'provider_native' }, context);
  };
  const heading = (id: string, label: string, result: string, first: boolean): Activity =>
    ({ id, label, result, detail: first ? 'first' : '', tone: 'phase', body: '', icon: '', output: '', failed: false, timestamp: '' });
  const rows: Activity[] = [];
  for (const phase of opening.group.phases.filter(entry => entry.members.length > 0)) {
    rows.push(heading(`phase:${phase.index}`, phase.title, `${phase.settledCount}/${phase.members.length}`, rows.length === 0), ...phase.members.map(member));
  }
  if (opening.group.unphasedMembers.length > 0) {
    if (rows.length > 0) rows.push({ ...heading('unphased', '', '', false), tone: 'gap' });
    rows.push(...opening.group.unphasedMembers.map(member));
  }
  return rows;
}

/** A line in highlight parts: each match of the thread find's query marked (FindParts `find` 1). */
export function findLine(id: string, text: string, query: string) {
  const ranges = literalFindRanges(text, query), cuts = [...new Set([0, text.length, ...ranges.flatMap(range => [range.start, range.end])])].sort((a, b) => a - b);
  const parts = cuts.slice(1).map((end, index) => ({ start: cuts[index]!, end })).filter(part => part.end > part.start)
    .map((part, index) => ({ id: String(index), text: text.slice(part.start, part.end), find: ranges.some(range => part.start >= range.start && part.end <= range.end) ? 1 : 0, at: part.start }));
  return { id, parts: parts.length ? parts : [{ id: '0', text, find: 0, at: 0 }] };
}

/**
 * The opening's timeline row in place of the script's user message. Its title, description and phase titles are thread
 * find text (`data-thread-find-text` upstream): `findLines` marks the query's matches in them. The script's own matches are
 * the Markdown's, as any message's.
 */
export function workflowOpeningMessage(opening: WorkflowOpening, context: SubagentContext, query = '') {
  const headline = workflowOpeningHeadline(opening), body = scriptMarkdown(opening.script), activities = workflowOpeningRows(opening, context);
  return { kind: 'workflow', title: headline.title, detail: headline.description ?? '', actionLabel: headline.agents ?? '', body,
    collapsible: shouldCollapseUserMessage(opening.script), activities,
    findLines: [findLine('title', headline.title, query), findLine('detail', headline.description ?? '', query),
      ...activities.filter(row => row.tone === 'phase').map(row => findLine(row.id, row.label, query))] };
}

// ── The parent's subagent rows ─────────────────────────────────────────────
type ParentDetail = { parentId: string; stamp: string; fetchedAt: number; subagents: Obj[] | null };
const parents = new WeakMap<object, ParentDetail>();
const REFETCH_MS = 3_000;

/** The parent's subagent rows: its kept live detail, else the last fetched snapshot (null before it arrives). */
export function workflowParentSubagents(client: T3Client, parentId: string): Obj[] | null {
  const kept = keptThread(client, parentId);
  if (kept) return arr(kept.projection.subagents);
  const cached = parents.get(client);
  return cached?.parentId === parentId ? cached.subagents : null;
}

/** The selected thread's opening, or null (ChatView workflowOpening). */
export function workflowOpening(client: T3Client): WorkflowOpening | null {
  const parentId = client.threadId ? workflowOpeningParentThreadId(client.projection) : null;
  return parentId === null ? null : resolveWorkflowOpening(client.projection, workflowParentSubagents(client, parentId));
}

/**
 * Before a snapshot: fetch the parent's bounded detail when the open thread opens with a workflow script and the parent
 * is not kept live, again when the parent's shell moves (at most every three seconds), so its agents' states follow.
 */
export async function syncWorkflowOpening(client: T3Client, native: Native | null | undefined, now = composerNow(client)): Promise<void> {
  if (!native?.available || !client.threadId) return;
  const parentId = workflowOpeningParentThreadId(client.projection);
  if (parentId === null || keptThread(client, parentId)) return;
  const stamp = str(client.shell.threads.find(thread => thread.id === parentId)?.updatedAt);
  const cached = parents.get(client);
  if (cached?.parentId === parentId && (cached.stamp === stamp || now - cached.fetchedAt < REFETCH_MS)) return;
  parents.set(client, { parentId, stamp, fetchedAt: now, subagents: cached?.parentId === parentId ? cached.subagents : null });
  try {
    const snapshot = threadSnapshot(await client.restAccess(native).http(`/api/orchestration/threads/${encodeURIComponent(parentId)}/bounded`));
    if (parents.get(client)?.parentId === parentId) parents.set(client, { parentId, stamp, fetchedAt: now, subagents: arr(snapshot.projection.subagents) });
  } catch (error) { if (letGo(error)) throw error; }
}
