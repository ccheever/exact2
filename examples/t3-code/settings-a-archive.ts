// Settings → Archive row context menu (lane settings-a). Reference:
// SettingsPanels.tsx handleArchivedThreadContextMenu — api.contextMenu.show at the
// pointer with Unarchive and a destructive Delete; Delete goes through
// useThreadActions confirmAndDeleteThread, which asks first only while the
// "Delete confirmation" device setting is on (ConfirmDialogHost, in-app).
import type { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';

type Pending = { scope: string; title: string };
const pending = new WeakMap<T3Client, Pending>();
type Archive = { manageArchivedThread(native: Native, storage: Files, op: string, scope: string): Promise<void> };
const archive = (client: T3Client) => client as unknown as Archive;
const failure = (error: unknown) => error instanceof Error ? error.message : 'An error occurred.';

export const ARCHIVE_MENU = [{ id: 'unarchive', label: 'Unarchive' }, { id: 'delete', label: 'Delete', destructive: true }];

/** The delete confirmation the root dialog shows, or an empty one. */
export function archiveConfirmation(client: T3Client): { id: string; title: string } {
  const entry = pending.get(client);
  return entry ? { id: entry.scope, title: entry.title } : { id: '', title: '' };
}

/** rest:archive-menu (right-click), rest:archive-delete (the dialog's Confirm) and rest:archive-cancel. */
export async function archiveCommand(client: T3Client, native: Native, storage: Files, op: string, scope: string, title: string): Promise<string> {
  if (op === 'archive-cancel') { pending.delete(client); return ''; }
  if (op === 'archive-delete') {
    const entry = pending.get(client);
    pending.delete(client);
    if (!entry || entry.scope !== scope) throw new ClientError('That archived thread is no longer available.');
    return remove(client, native, storage, scope);
  }
  if (op !== 'archive-menu') throw new ClientError(`Unknown settings action: ${op}`);
  if (!scope) throw new ClientError('That archived thread is no longer available.');
  const reply = await client.restAccess(native).call({ op: 'contextMenu', items: ARCHIVE_MENU });
  const clicked = typeof reply.clicked === 'string' ? reply.clicked : null;
  if (clicked === 'unarchive') {
    try { await archive(client).manageArchivedThread(native, storage, 'unarchive-thread', scope); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Failed to unarchive thread', description: failure(error) }); }
    return '';
  }
  if (clicked === 'delete') {
    const prefs = (client.local as unknown as { clientSettings?: { confirmThreadDelete?: boolean } }).clientSettings;
    if (prefs?.confirmThreadDelete !== false) { pending.set(client, { scope, title: title || 'this thread' }); return ''; }
    return remove(client, native, storage, scope);
  }
  return '';
}
async function remove(client: T3Client, native: Native, storage: Files, scope: string): Promise<string> {
  try { await archive(client).manageArchivedThread(native, storage, 'delete-archived-thread', scope); }
  catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Failed to delete thread', description: failure(error) }); }
  return '';
}
