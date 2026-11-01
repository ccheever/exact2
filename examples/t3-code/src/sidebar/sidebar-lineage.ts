// Adapted from T3 Code 74541bab17 (MIT); see LICENSE-T3.
// Subagent threads (ChatView.tsx parentThreadLink, MessagesTimeline.tsx list
// header; MIT, see LICENSE-T3): the sidebar never lists them
// (sidebar-model.ts sidebarVisible), so a subagent's timeline opens with the
// "Subagent of <parent>" divider whose action opens the parent thread. It
// reuses the timeline's system-divider row ("fork" kind: the pill selects its
// target thread), and its nearest listed ancestor's row stays highlighted.
import { isListedThreadLineage, obj, str, type Obj } from '../client/domain';
import type { Message } from '../client/domain';
import type { T3Client } from '../client/client';

/**
 * ChatView parentThreadLink, the header's parent crumb (ChatHeader.tsx): a subagent thread links its parent by
 * the parent's title ("Parent thread" until the shell has it). One entry keyed by the parent, else none.
 */
export function parentThreadLink(client: T3Client): { id: string; label: string }[] {
  const thread = client.threadId ? client.shell.threads.find(entry => entry.id === client.threadId) : undefined;
  const lineage = obj(thread?.lineage), parentId = str(lineage.parentThreadId);
  if (lineage.relationshipToParent !== 'subagent' || !parentId) return [];
  const parent = client.shell.threads.find(entry => entry.id === parentId);
  return [{ id: parentId, label: str(parent?.title, 'Parent thread') }];
}

export function subagentLead(client: T3Client): Message[] {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  const lineage = obj(thread?.lineage), parentId = str(lineage.parentThreadId);
  if (!thread || lineage.relationshipToParent !== 'subagent' || !parentId) return [];
  const parent = client.shell.threads.find(entry => entry.id === parentId);
  // TimelineSystemDivider: the bot glyph, "Subagent of" and the parent's title after a middle dot.
  return [{ id: `subagent-of-${str(thread.id)}`, kind: 'fork', icon: 'bot', title: 'Subagent of', body: `Subagent of · ${str(parent?.title, 'Parent thread')}`, detail: str(parent?.title, 'Parent thread'),
    actionLabel: 'Open parent thread', targetId: parentId, createdAt: str(thread.createdAt) } as Message];
}

/**
 * Sidebar.logic.ts resolveSidebarRowThreadId: the row that stands for the open thread. A subagent or side chat has no
 * row, so its nearest listed ancestor stands in; a broken chain (a missing parent, a cycle) resolves to the thread
 * itself, which then matches no row.
 */
export function resolveSidebarRowThreadId(threadId: string, threads: readonly Obj[]): string {
  const find = (id: string) => threads.find(thread => thread.id === id);
  const visited = new Set<string>();
  let thread = find(threadId);
  while (thread !== undefined && !isListedThreadLineage(thread.lineage)) {
    const parentId = obj(thread.lineage).parentThreadId;
    if (parentId == null || visited.has(str(thread.id))) return threadId;
    visited.add(str(thread.id));
    thread = find(str(parentId));
  }
  return thread ? str(thread.id) : threadId;
}
/** The open thread's row (routeRowThreadKey): the open thread, or the ancestor standing in for an open subagent. */
export const routeRowThreadId = (client: T3Client): string => client.threadId ? resolveSidebarRowThreadId(client.threadId, client.shell.threads) : '';
