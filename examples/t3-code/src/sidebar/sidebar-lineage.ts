// Adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3.
// Subagent threads (ChatView.tsx parentThreadLink, MessagesTimeline.tsx list
// header; MIT, see LICENSE-T3): the sidebar never lists them
// (sidebar-model.ts sidebarVisible), so a subagent's timeline opens with the
// "Subagent of <parent>" divider whose action opens the parent thread. It
// reuses the timeline's system-divider row ("fork" kind: the pill selects its
// target thread).
import { obj, str } from '../client/domain';
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
