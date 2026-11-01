// A delegated subagent or a side chat waiting on the user shows on the threads above it,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 2359643cff:
// packages/client-runtime/src/state/threadShell.ts (isChildRunLive,
// subagentRequest, sideChatRequest, pendingSubagentRequestsByThread, childThreadInputsAtom,
// childThreadInputsTitle). Subagent and side chat rows are hidden in the sidebar, so their
// parents carry the request: status, notifications, snooze and the composer.
import { obj, str, type Obj } from '../client/domain';
import { isProviderSubagent } from '../composer/composer-controls-subagent';
import { pendingInput } from './sidebar-model';

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

/**
 * A side chat asks its user directly, so questions roll up to the parent as well as approvals, when its own row would
 * show them.
 */
export function sideChatRequest(thread: Obj): PendingSubagentRequest | null {
  const kind = thread.pendingRuntimeRequest == null ? undefined : str(obj(thread.pendingRuntimeRequest).kind);
  if (kind === undefined || kind === 'auth_refresh' || obj(thread.lineage).sideChat !== true) return null;
  if (kind !== 'user_input') return 'approval';
  return pendingInput(thread) ? 'user_input' : null;
}

const byArray = new WeakMap<Obj[], Map<string, PendingSubagentRequest>>();
/**
 * Threads above a subagent that waits on the user for an approval. Every
 * thread up to the visible one carries it. The coordinator answers a child's
 * question, so a question flags no thread above it.
 */
export function pendingSubagentRequestsByThread(threads: Obj[]): Map<string, PendingSubagentRequest> {
  const cached = byArray.get(threads);
  if (cached) return cached;
  const requests = new Map<string, PendingSubagentRequest>();
  let byId: Map<string, Obj> | undefined;
  for (const child of threads) {
    if (obj(child.lineage).sideChat === true) {
      const parentId = str(obj(child.lineage).parentThreadId), request = sideChatRequest(child);
      if (parentId && request && requests.get(parentId) !== 'approval') requests.set(parentId, request);
      continue;
    }
    const request = subagentRequest(child);
    if (request !== 'approval') continue;
    byId ??= new Map(threads.map(thread => [str(thread.id), thread]));
    const seen = new Set<string>([str(child.id)]);
    let current: Obj | undefined = child;
    while (current && (obj(current.lineage).relationshipToParent === 'subagent' || obj(current.lineage).sideChat === true)) {
      const parentId = str(obj(current.lineage).parentThreadId);
      if (!parentId || seen.has(parentId)) break;
      seen.add(parentId);
      requests.set(parentId, request);
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

export type ChildInput = { id: string; title: string; approval: boolean; sideChat: boolean };
/** childThreadInputsAtom: the subagents and side chats below a thread (at any depth) that wait on the user, in shell order. */
export function childThreadInputs(threads: Obj[], threadId: string): ChildInput[] {
  const children = new Map<string, Obj[]>();
  for (const thread of threads) {
    const lineage = obj(thread.lineage), parent = str(lineage.parentThreadId);
    if (!parent || (lineage.relationshipToParent !== 'subagent' && lineage.sideChat !== true)) continue;
    children.set(parent, [...children.get(parent) ?? [], thread]);
  }
  const seen = new Set([threadId]), pending = [threadId], found: ChildInput[] = [];
  for (const parent of pending) {
    for (const child of children.get(parent) ?? []) {
      const id = str(child.id);
      if (seen.has(id)) continue;
      seen.add(id); pending.push(id);
      const sideChat = obj(child.lineage).sideChat === true, request = sideChat ? sideChatRequest(child) : subagentRequest(child);
      if (request) found.push({ id, title: str(child.title, sideChat ? 'Side chat' : 'Subagent'), approval: request === 'approval', sideChat });
    }
  }
  return found;
}

/** childThreadInputsTitle. */
export function childThreadInputsTitle(children: ChildInput[]): string {
  const approvals = children.filter(child => child.approval).length;
  const need = approvals === 0 ? 'input' : approvals === children.length ? 'approval' : 'you';
  const sideChats = children.filter(child => child.sideChat).length;
  const noun = sideChats === children.length ? 'side chat' : sideChats === 0 ? 'subagent' : 'thread';
  return children.length === 1 ? `${noun === 'side chat' ? 'Side chat' : 'Subagent'} needs ${need}` : `${children.length} ${noun}s need ${need}`;
}
/** childInputNowItem's waitingLabel: what the scroll-to-end button names (ChatView scrollToEndSummary). */
export function childThreadInputsWaiting(children: ChildInput[]): string {
  return children.every(child => child.sideChat) ? 'Side chat waiting' : 'Subagent waiting';
}
