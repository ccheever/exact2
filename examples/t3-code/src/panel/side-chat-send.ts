// `/side` and `/btw` sent from the composer (T3 Code 2359643cff, MIT, see LICENSE-T3: useComposerSend's side chat
// branch, composer-logic.ts parseSideChatCommand): a side chat starts with the rest of the message, or a toast says why not.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { composerInput } from '../composer/requests';
import { queuedEdit } from '../composer/composer-controls-queue';
import { contextLinks } from '../composer/context-links';
import { parseSideChatCommand, sideChatDisabledReason, sideChatsOffered } from './side-chat';
import { openNewSideChat } from './panel-surfaces';

/** True when the message was a side chat command, sent or refused; false lets the send go on. */
export async function sideChatSend(client: T3Client, native: Native, value: string): Promise<boolean> {
  const promptAtSend = value || client.draft;
  const command = queuedEdit(client) ? null : parseSideChatCommand(promptAtSend);
  if (!command) return false;
  const answering = !!composerInput(client), reason = sideChatDisabledReason(client);
  // Where side chats are unavailable, `/side` text can still be a literal answer.
  if (reason !== null) {
    if (answering) return false;
    pushToast(client, { kind: 'warning', stacked: true, title: 'Side chat not started', description: reason });
    return true;
  }
  if (!sideChatsOffered(client) || !client.shell.threads.some(thread => thread.id === client.threadId)) return false;
  if (client.snapshotDrafts.length > 0 || contextLinks(promptAtSend).length > 0) {
    pushToast(client, { kind: 'warning', stacked: true, title: 'Side chats take text only', description: 'Remove the attachments and context, then send it again.' });
    return true;
  }
  const draftKey = client.draftKey;
  const started = await openNewSideChat(client, native, command.prompt);
  // Clear only what was sent; the user may have edited the draft or left meanwhile.
  if (started && client.draftKey === draftKey && (client.local.drafts[draftKey] ?? '') === promptAtSend) client.local.drafts[draftKey] = '';
  return true;
}
