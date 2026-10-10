// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/chat-commands.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
import type { T3Client } from './client';
import { obj, str } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { retryWorkspacePreparation } from './r11-upstream-retry';

/**
 * Sidebar row actions (SidebarThreadRow's Settle and Un-settle). Each targets
 * the row's own thread, never the selected one, and is refused when that thread
 * left the shell or the server cannot settle threads.
 */
export async function chatCommand(client: T3Client, native: Native, storage: Files, op: string, threadId: string, value = ''): Promise<string> {
  if (op === 'retry-preparation') return retryWorkspacePreparation(client, native, storage, value); // lane r11-upstream (737993303d)
  const thread = client.shell.threads.find(candidate => candidate.id === threadId);
  if (!thread) throw new ClientError('That thread is no longer available.');
  const capabilities = obj(obj(client.config.environment).capabilities);
  if (capabilities[op.endsWith('snooze') ? 'threadSnooze' : 'threadSettlement'] === false) {
    throw new ClientError(op.endsWith('snooze') ? 'This server does not support snoozing threads.' : 'This server does not support settling threads.');
  }
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  if (op === 'settle') {
    await access.dispatch(storage, { type: 'thread.settle', commandId, threadId }, `Settle ${str(thread.title, 'thread')}`);
    return '';
  }
  if (op === 'unsettle') {
    await access.dispatch(storage, { type: 'thread.unsettle', commandId, threadId, reason: 'user' }, `Un-settle ${str(thread.title, 'thread')}`);
    return '';
  }
  if (op === 'snooze') {
    if (!Number.isFinite(Date.parse(value))) throw new ClientError('Choose when to bring this thread back.');
    await access.dispatch(storage, { type: 'thread.snooze', commandId, threadId, snoozedUntil: new Date(value).toISOString() }, `Snooze ${str(thread.title, 'thread')}`);
    return '';
  }
  if (op === 'unsnooze') {
    await access.dispatch(storage, { type: 'thread.unsnooze', commandId, threadId, reason: 'user' }, `Wake ${str(thread.title, 'thread')}`);
    return '';
  }
  throw new ClientError(`Unknown thread action: ${op}`);
}
