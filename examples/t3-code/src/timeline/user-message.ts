// packages/client-runtime/src/userMessage.ts at T3 Code v0.1.6 (MIT; LICENSE-T3).
import { str, type Obj } from '../client/domain';
import { contextLinks, contextTarget, replaceLinks } from '../composer/context-links';
import { isAttachmentOnlyPrompt } from '../composer/attachment-only-prompt';

const LEGACY_AUTOMATION_PREFIX = /^\[Triggered by schedule task: [^\r\n]+\]\r?\n\r?\n/;
const LEGACY_AUTOMATION_MESSAGE_ID = /^scheduled-task-message:(.+):\d+:(?:scheduled|manual)$/;
/**
 * resolveUserMessagePresentation: who sent a user message the user
 * did not type. Older scheduled messages carry their attribution in the prompt itself; a restart
 * continuation the server sent as the agent is T3 Code's.
 */
export function userMessagePresentation(item: Obj): { text: string; attribution: '' | 'automation' | 'agent' | 't3code'; scheduledTaskId: string } {
  const text = str(item.text);
  if (typeof item.scheduledTaskId === 'string') return { text, attribution: 'automation', scheduledTaskId: item.scheduledTaskId };
  const legacyPrefix = LEGACY_AUTOMATION_PREFIX.exec(text);
  const legacyTaskId = legacyPrefix ? LEGACY_AUTOMATION_MESSAGE_ID.exec(str(item.messageId))?.[1] : undefined;
  if (legacyPrefix && (legacyTaskId !== undefined || item.createdBy === 'agent')) return { text: text.slice(legacyPrefix[0].length), attribution: 'automation', scheduledTaskId: legacyTaskId ?? '' };
  // The agent needs words to start a turn; people sent only the attachments.
  return { text: isAttachmentOnlyPrompt(text) ? '' : text, attribution: item.createdBy !== 'agent' ? '' : item.creationSource === 'server' ? 't3code' : 'agent', scheduledTaskId: '' };
}

/** A context link's kind, where its href is `t3-context://v1/<kind>/<anything>` (replaceComposerContextReferences reads any). */
const LINK_KIND = /^t3-context:\/\/v1\/([a-z][a-z0-9-]{0,39})\/[^\s)]{1,128}$/i;
/**
 * An automation's prompt body (AutomationMessageRow's UserMessageBody, given no context renderer): each context
 * reference reads as its label, or its id with none, in plain text (ChatMarkdown's `<span>{label}</span>`).
 */
export function plainContextReferences(text: string): string {
  return replaceLinks(text, contextLinks(text), link => {
    const target = contextTarget(link.href);
    return target ? (link.label.trim() || target.id).replace(/[\\`*_[\]()<>!|~#]/g, '\\$&') : null;
  });
}
/**
 * TimelineUserMessageRow's folded line (T3 Code 7c3b3b3605ef): an automation's task title, else its prompt's first line;
 * another agent's first non-empty line, each context reference read as its label (replaceComposerContextReferences).
 */
export function foldedMessageSummary(attribution: string, text: string, taskTitle: string): string {
  if (attribution === 'automation') return taskTitle || (text.trim().split(/\r?\n/, 1)[0] ?? '');
  const plain = replaceLinks(text, contextLinks(text), link => {
    const kind = LINK_KIND.exec(link.href)?.[1];
    return kind === undefined ? null : link.label.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim() || kind;
  });
  return plain.split(/\r?\n/).find(line => line.trim().length > 0)?.trim() ?? '';
}
