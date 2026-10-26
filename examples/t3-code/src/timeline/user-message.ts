// packages/client-runtime/src/userMessage.ts at T3 Code 74541bab17 (MIT; LICENSE-T3).
import { str, type Obj } from '../client/domain';

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
  return { text, attribution: item.createdBy !== 'agent' ? '' : item.creationSource === 'server' ? 't3code' : 'agent', scheduledTaskId: '' };
}
