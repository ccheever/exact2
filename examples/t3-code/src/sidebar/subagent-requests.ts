// A delegated subagent waiting on the user shows on the threads above it,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 9c161462c9:
// packages/client-runtime/src/state/threadShell.ts (isChildRunLive,
// subagentRequest, pendingSubagentRequestsByThread, childThreadInputsAtom,
// childThreadInputsTitle). Subagent rows are hidden in the sidebar, so their
// parents carry the request: status, notifications, snooze and the composer.
import { obj, str, type Obj } from '../client/domain';
import { isProviderSubagent } from '../composer/composer-controls-subagent';

export type PendingSubagentRequest = 'approval' | 'user_input';

/**
 * An async question outlives the subagent's run, but once that run ends the
 * parent no longer waits on it. Servers that omit activityRunStatus fall back
 * to the latest run's status.
 */
export function isChildRunLive(thread: Obj): boolean {
  return thread.activityRunStatus !== undefined ? thread.activityRunStatus !== null
    : ['preparing', 'starting', 'running', 'waiting'].includes(str(thread.status));
}

/** What a subagent's live run waits on the user for. Provider-native subagents ask on their parent, so they are skipped. */
export function subagentRequest(thread: Obj): PendingSubagentRequest | null {
  const kind = thread.pendingRuntimeRequest == null ? undefined : str(obj(thread.pendingRuntimeRequest).kind);
  if (kind === undefined || kind === 'auth_refresh' || obj(thread.lineage).relationshipToParent !== 'subagent' || isProviderSubagent(thread) || !isChildRunLive(thread)) return null;
  return kind === 'user_input' ? 'user_input' : 'approval';
}

const byArray = new WeakMap<Obj[], Map<string, PendingSubagentRequest>>();
/**
 * Threads above a subagent that waits on the user, by what it waits on. Every
 * thread up to the visible one carries it; an approval outranks a question.
 */
export function pendingSubagentRequestsByThread(threads: Obj[]): Map<string, PendingSubagentRequest> {
  const cached = byArray.get(threads);
  if (cached) return cached;
  const requests = new Map<string, PendingSubagentRequest>();
  let byId: Map<string, Obj> | undefined;
  for (const child of threads) {
    const request = subagentRequest(child);
    if (request === null) continue;
    byId ??= new Map(threads.map(thread => [str(thread.id), thread]));
    const seen = new Set<string>([str(child.id)]);
    let current: Obj | undefined = child;
    while (current && obj(current.lineage).relationshipToParent === 'subagent') {
      const parentId = str(obj(current.lineage).parentThreadId);
      if (!parentId || seen.has(parentId)) break;
      seen.add(parentId);
      if (requests.get(parentId) !== 'approval') requests.set(parentId, request);
      current = byId.get(parentId);
    }
  }
  byArray.set(threads, requests);
  return requests;
}

// The request each listed thread carries, refreshed whenever a list is read (a parent keeps its object when only a child changes).
const marks = new WeakMap<Obj, PendingSubagentRequest>();
export function noteSubagentRequests(threads: Obj[]): void {
  const requests = pendingSubagentRequestsByThread(threads);
  for (const thread of threads) {
    const request = requests.get(str(thread.id));
    if (request) marks.set(thread, request); else marks.delete(thread);
  }
}
/** The request a thread carries for a subagent below it: a fleet copy's own field, else the last noted list's. */
export function pendingSubagentRequest(thread: Obj): PendingSubagentRequest | null {
  const own = thread.pendingSubagentRequest;
  if (own === 'approval' || own === 'user_input') return own;
  return marks.get(thread) ?? null;
}

export type ChildInput = { id: string; title: string; approval: boolean };
/** childThreadInputsAtom: the subagents below a thread (at any depth) that wait on the user, in shell order. */
export function childThreadInputs(threads: Obj[], threadId: string): ChildInput[] {
  const children = new Map<string, Obj[]>();
  for (const thread of threads) {
    const lineage = obj(thread.lineage), parent = str(lineage.parentThreadId);
    if (!parent || lineage.relationshipToParent !== 'subagent') continue;
    children.set(parent, [...children.get(parent) ?? [], thread]);
  }
  const seen = new Set([threadId]), pending = [threadId], found: ChildInput[] = [];
  for (const parent of pending) {
    for (const child of children.get(parent) ?? []) {
      const id = str(child.id);
      if (seen.has(id)) continue;
      seen.add(id); pending.push(id);
      const request = subagentRequest(child);
      if (request) found.push({ id, title: str(child.title, 'Subagent'), approval: request === 'approval' });
    }
  }
  return found;
}

/** childThreadInputsTitle. */
export function childThreadInputsTitle(children: ChildInput[]): string {
  const approvals = children.filter(child => child.approval).length;
  const need = approvals === 0 ? 'input' : approvals === children.length ? 'approval' : 'you';
  return children.length === 1 ? `Subagent needs ${need}` : `${children.length} subagents need ${need}`;
}
