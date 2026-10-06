// Window-shell commands (MIT reference: hooks/useThreadActionMenu.ts,
// hooks/useThreadActions.ts, chat/ChatHeader.tsx rename, ProviderUpdate*
// "Update", OpenInPicker) and the toasts the reference raises around the
// main window's actions. `shell:` ops write to the server through the
// client's generation-guarded access; `shelllocal:` ops never do.
import { cloneCommand } from './project-clones-live';
import { closeThreadTerminals } from './terminal-drawer-view'; // terminal-drawer
import { automationCommand } from './scheduled-tasks-commands';
import type { T3Client } from './client';
import { pushToast } from './toast';
import { obj, str } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { rememberEditor, detailsKey } from './shell-details';
import { toggleInline } from './shell-prefs';
import { markCopied } from './shell';
import { gitShellCommand, GIT_FAILURE_TITLES } from './r4-git-route';
import { surfaceLocal, surfaceCommand } from './r4-surfaces-panel';
import { openInEditorHere } from './remote-open'; // remote Open (OpenInPicker)

function threadOf(client: T3Client, threadId: string) {
  const thread = client.shell.threads.find(candidate => candidate.id === threadId);
  if (!thread) throw new ClientError('That thread is no longer available.');
  return thread;
}

/** resolveRenameCommit: trim, reject empty (the caller toasts), skip unchanged. */
export function resolveRenameCommit(title: string, originalTitle: string): { action: 'commit'; title: string } | { action: 'reject-empty' } | { action: 'noop' } {
  const trimmed = title.trim();
  if (trimmed.length === 0) return { action: 'reject-empty' };
  if (trimmed === originalTitle) return { action: 'noop' };
  return { action: 'commit', title: trimmed };
}

export async function shellCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op.startsWith('surface-')) return surfaceCommand(client, native, storage, op.slice(8), id, value); // r4-surfaces-panel.ts
  if (op.startsWith('clone-')) return cloneCommand(client, native, storage, op, id, value); // project-clones-live.ts
  if (op.startsWith('automation-')) return automationCommand(client, native, op.slice(11), id, value); // scheduled-tasks-commands.ts
  const access = client.restAccess(native);
  if (op === 'provider-update') {
    // server.updateProvider per one-click candidate, in order (runUpdates),
    // under the running/failed/updated toasts (ProviderUpdateLaunchNotification.logic.ts).
    const providers = (Array.isArray(client.config.providers) ? client.config.providers : []).map(obj);
    const ids = id.split(',').filter(Boolean), one = ids.length === 1;
    pushToast(client, { kind: 'loading', title: one ? 'Updating provider' : 'Updating providers', description: 'Running provider update command.', timeoutMs: 0, hideCopy: true, key: 'provider-update' });
    try {
      for (const instanceId of ids) {
        const provider = providers.find(entry => str(entry.instanceId) === instanceId);
        if (!provider) throw new ClientError('That provider is no longer available.');
        await access.request('server.updateProvider', { provider: str(provider.driver), instanceId }, true);
      }
    } catch (error) {
      pushToast(client, { kind: 'error', title: one ? 'Provider update failed' : 'Provider updates failed', description: messageOf(error), timeoutMs: 0, stacked: true, key: 'provider-update' });
      return '';
    }
    pushToast(client, { kind: 'success', title: one ? 'Provider updated' : 'Provider updates finished',
      description: one ? 'New sessions will use the updated provider.' : 'New sessions will use the updated providers.', timeoutMs: 3000, hideCopy: true, key: 'provider-update' });
    return '';
  }
  if (op === 'open-editor') {
    if (!id) throw new ClientError('This thread does not have a workspace path to open.');
    // OpenInPicker: remotely a deep link to this Mac's editor, never an editor run on the other machine (remote-open.ts).
    if (await openInEditorHere(client, native, id, value)) rememberEditor(client, value);
    return '';
  }
  // lane r4-git: the card's branch picker, Git actions and dialogs (r4-git-route.ts).
  if (op.startsWith('git-')) return gitShellCommand(client, native, storage, op.slice(4), id, value);
  const thread = threadOf(client, id);
  const title = str(thread.title, 'thread');
  if (op === 'rename') {
    const resolution = resolveRenameCommit(value, str(thread.title));
    if (resolution.action === 'reject-empty') { pushToast(client, { kind: 'warning', title: 'Thread title cannot be empty' }); return ''; }
    if (resolution.action === 'noop') return '';
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'thread.metadata.update', commandId, threadId: id, title: resolution.title }, `Rename ${title}`);
    return '';
  }
  if (op === 'merge-back') {
    // ThreadRelationshipsPanel merge(): thread.merge_back from the fork's latest finished run, then open the target.
    const [targetThreadId = '', runId = ''] = value.split('|');
    if (!targetThreadId || !runId) throw new ClientError('Complete a run in this fork before merging it back');
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'thread.merge_back', commandId, createdBy: 'user', creationSource: 'web', sourceThreadId: id, targetThreadId,
      sourcePoint: { type: 'run', runId } }, `Merge ${title} back`);
    await client.openSelected(native, targetThreadId);
    return '';
  }
  const [commandId] = await access.ids(1);
  const payloads: Record<string, [string, Record<string, unknown>]> = {
    pin: ['Pin', { type: 'thread.pin' }], unpin: ['Unpin', { type: 'thread.unpin' }],
    'mark-unread': ['Mark unread', { type: 'thread.mark-unread' }],
    'regenerate-title': ['Regenerate title of', { type: 'thread.metadata.update', regenerateTitle: true }],
    'auto-settle': ['Change auto-settle of', { type: 'thread.auto-settle.set', enabled: value !== 'false' }],
    archive: ['Archive', { type: 'thread.archive' }], delete: ['Delete', { type: 'thread.delete' }],
  };
  const entry = payloads[op];
  if (!entry) throw new ClientError(`Unknown thread action: ${op}`);
  if (op === 'archive' && (thread.activeRunId || ['preparing', 'starting', 'running', 'waiting'].includes(str(thread.status)))) {
    throw new ClientError('Stop the running turn before archiving this thread.');
  }
  if (op === 'delete') await closeThreadTerminals(client, native, id); // terminal-drawer: useThreadActions closes the thread's terminals first
  await access.dispatch(storage, { ...entry[1], commandId, threadId: id }, `${entry[0]} ${title}`);
  return '';
}

/** Local window actions: clipboard copies with the reference's toasts. */
export async function shellLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const copy = async (text: string) => {
    const response = obj(await native.later({ op: 'copyText', text, generation: client.generation }));
    if (response.ok !== true) throw new ClientError('Could not copy to the clipboard.');
  };
  if (op === 'copy-error') { await copy(value); if (Number(id) > 0) markCopied(client, Number(id)); return ''; }
  // ThreadDetailsControl in the inline presentation: toggles this thread's docked card (remembered).
  if (op === 'details-inline') { toggleInline(client, detailsKey(client)); return ''; }
  if (op.startsWith('surface-')) return surfaceLocal(client, native, op.slice(8), id, value); // r4-surfaces-panel.ts
  const thread = client.shell.threads.find(candidate => candidate.id === id);
  if (op === 'copy-path') {
    const project = client.shell.projects.find(candidate => candidate.id === (thread?.projectId ?? client.projectId));
    const path = str(thread?.worktreePath) || str(project?.workspaceRoot);
    if (!path) { pushToast(client, { kind: 'error', title: 'Path unavailable', description: 'This thread does not have a workspace path to copy.', stacked: true }); return ''; }
    try { await copy(path); } catch (error) { pushToast(client, { kind: 'error', title: 'Failed to copy path', description: messageOf(error) }); return ''; }
    pushToast(client, { kind: 'success', title: 'Path copied', description: path });
    return '';
  }
  if (op === 'copy-branch') {
    const branch = str(thread?.branch);
    if (!branch) return '';
    try { await copy(branch); } catch (error) { pushToast(client, { kind: 'error', title: 'Failed to copy branch', description: messageOf(error) }); return ''; }
    pushToast(client, { kind: 'success', title: 'Branch copied', description: branch });
    return '';
  }
  if (op === 'copy-thread-id') {
    if (!thread) throw new ClientError('That thread is no longer available.');
    try { await copy(str(thread.id)); } catch (error) { pushToast(client, { kind: 'error', title: 'Failed to copy thread ID', description: messageOf(error) }); return ''; }
    pushToast(client, { kind: 'success', title: 'Thread ID copied', description: str(thread.id) });
    return '';
  }
  throw new ClientError(`Unknown window action: ${op}`);
}
const messageOf = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';

/** The reference's failure toast titles for main-window actions (Sidebar.tsx, ChatView.tsx, useThreadActionMenu.ts). */
const FAILURE_TITLES: Record<string, string> = {
  'chat:settle': 'Failed to settle thread', 'chat:unsettle': 'Failed to un-settle thread', unsettle: 'Failed to un-settle thread',
  'chat:unsnooze': 'Failed to wake thread', 'chat:snooze': 'Failed to snooze thread',
  'shell:pin': 'Failed to pin thread', 'shell:unpin': 'Failed to unpin thread', 'shell:rename': 'Failed to rename thread',
  'shell:archive': 'Failed to archive thread', 'shell:delete': 'Failed to delete thread',
  'shell:regenerate-title': 'Failed to regenerate thread title', 'shell:auto-settle': 'Failed to update auto-settle',
  'shell:mark-unread': 'Failed to mark thread unread', 'shell:provider-update': 'Provider update failed',
  'shell:open-editor': 'Unable to open editor', 'shell:git-action': 'Git action failed', 'copy-diagnostic': 'Could not copy trace ID',
};

/**
 * Route a failed command to the toast the reference shows instead of the
 * transcript banner. Returns true when the failure was toasted.
 */
export function shellFailure(client: T3Client, op: string, message: string): boolean {
  if (!message) return false;
  const title = FAILURE_TITLES[op] ?? GIT_FAILURE_TITLES[op];
  if (title) { pushToast(client, { kind: 'error', title, description: message, stacked: true }); return true; }
  if (op === 'send' && client.connection !== 'connected') {
    pushToast(client, { kind: 'warning', title: 'Not connected: message not sent', description: 'Reconnecting to the environment. Try again once it is connected.', stacked: true });
    return true;
  }
  if (op === 'send' && message === 'Choose or add a project first.') {
    pushToast(client, { kind: 'warning', title: 'Choose a project first', description: 'This draft no longer points to an available project.', stacked: true });
    return true;
  }
  return false;
}

const decode = (part: string) => { try { return decodeURIComponent(part); } catch { return part; } };
function formParams(value: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of value.split('&')) { const at = part.indexOf('='); if (part) out[decode(at < 0 ? part : part.slice(0, at))] = at < 0 ? '' : decode(part.slice(at + 1)); }
  return out;
}

/**
 * Settings pages toast their failures instead of drawing them inline
 * (ScheduledTasksSettings, KeybindingsSettings, SettingsPanels, archived
 * threads). Returns the toast's title when one was raised; the caller marks
 * the command's message `toasted:` so the form stays open without its own
 * error text.
 */
export function settingsFailure(client: T3Client, op: string, id: string, value: string, message: string): string {
  if (!message) return '';
  let title = '', description = message;
  const form = formParams(value);
  if (op === 'rest:task' && form.action === 'save') {
    if (/^Scheduled task is incomplete|^Enter a task name and prompt/.test(message)) { title = 'Scheduled task is incomplete'; description = 'Add a title, prompt, project, and model.'; }
    else if (/interval of at least one minute/.test(message)) { title = 'Invalid interval'; description = 'Enter an interval of at least one minute.'; }
    else if (/existing checkout path/.test(message)) { title = 'Checkout path is required'; description = 'Enter the path of the checkout to run in.'; }
    else { title = 'Could not save scheduled task'; description = message.replace(/^Could not save scheduled task: /, ''); }
  } else if (op === 'rest:task' || op === 'task-toggle' || op === 'task-delete' || op === 'task-run') title = 'Could not update scheduled task';
  else if (op === 'task-save') title = 'Could not save scheduled task';
  else if (op === 'rest:keybinding' || op === 'keybinding-save' || op === 'keybinding-remove') title = form.action === 'remove' || op === 'keybinding-remove' ? 'Unable to remove keybinding' : 'Unable to save keybinding';
  else if (op === 'rest:keybinding-open') title = 'Unable to open keybindings file';
  else if (op === 'unarchive-thread') title = 'Failed to unarchive thread';
  else if (op === 'delete-archived-thread') title = 'Failed to delete thread';
  else if (op === 'settings-core' && id.startsWith('textGenerationModelSelection:')) title = 'Text generation model not saved';
  else if (op === 'settings-core' && id.startsWith('sourceControlWriterModelSelection:')) title = 'Source control writer model not saved';
  if (!title) return '';
  pushToast(client, { kind: 'error', title, description, stacked: true });
  return title;
}

/**
 * Successes the reference announces (⇧⌘C copy-thread: "Thread ID copied";
 * ⌥⇧⌘A appearance.cycle, the only device-setting write of appearanceMode:
 * an untyped 1.5 s "Appearance: <mode>" that replaces the previous one).
 */
export function shellSuccess(client: T3Client, op: string, value: string, message: string, id = ''): void {
  if (op === 'device-setting' && id === 'appearanceMode' && !message) {
    const label = value === 'light' ? 'Light' : value === 'dark' ? 'Dark' : 'System';
    pushToast(client, { kind: 'info', title: `Appearance: ${label}`, timeoutMs: 1500, leading: 'none', key: 'appearance-cycle' });
  }
  if (op === 'restlocal:copy-thread' && message === 'Copied thread ID') {
    pushToast(client, { kind: 'success', title: 'Thread ID copied', description: client.threadId });
  }
  if (op === 'copy-diagnostic' && message === 'Copied trace ID') pushToast(client, { kind: 'success', title: 'Trace ID copied', description: value });
}
