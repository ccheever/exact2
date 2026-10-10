// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/context-menu-actions.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// context-menu-gaps: what the Files tree row and pull request number menus do (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3): FileBrowserPanel.tsx showEntryContextMenu with
// fileContextMenu.ts useFileContextMenu.activate, and pullRequestLinkContextMenu.ts
// showPullRequestLinkContextMenu. Items come from context-menus.ts; the module's
// `contextMenu` op (T3ContextMenu.swift) shows them at the pointer and names the pick.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { fileLink } from './composer-editor-menu';
import { workspaceOf } from './r4-surfaces-panel';
import { buildFileContextMenuItems, fileActionFailureTitle, fileContextMenuCapabilities, fileTreeContextMenuItems, menuItemIds, openOnHostLabel,
  pullRequestLinkContextMenuItems, resolveFileContextMenuAbsolutePath } from './context-menus';
import type { MenuItem } from './sidebar-menu';

/** The module's menu at the pointer; the picked id, or '' when it closed without a choice. */
export async function showContextMenu(client: T3Client, native: Native, items: MenuItem[]): Promise<string> {
  const result = obj(await client.restAccess(native).call({ op: 'contextMenu', items: items as unknown as Obj[] }));
  const picked = str(result.clicked);
  return menuItemIds(items).includes(picked) ? picked : '';
}

/** A Files tree row's right-click (`shelllocal:surface-files-row-menu`, id = the row's workspace path). */
export async function filesTreeMenu(client: T3Client, native: Native, relativePath: string): Promise<void> {
  const path = relativePath.replace(/\/+$/, '');
  if (!path) return;
  const environmentId = client.environmentId || null, origin = client.origin, { cwd } = workspaceOf(client);
  const target = { environmentId, filePath: path, workspaceRoot: cwd || undefined };
  const absolutePath = resolveFileContextMenuAbsolutePath(target);
  const fileItems = buildFileContextMenuItems({ hasAbsolutePath: absolutePath !== null, capabilities: fileContextMenuCapabilities(client.config, environmentId) });
  const picked = await showContextMenu(client, native, fileTreeContextMenuItems(fileItems));
  // A menu from another environment cannot act here.
  if (!picked || (client.environmentId || null) !== environmentId || client.origin !== origin) return;
  const mention = fileLink(path);
  if (picked === 'copy-mention') {
    const reply = await bridgeReply(native, { op: 'copyText', text: mention });
    if (reply.ok) pushToast(client, { kind: 'success', title: 'Mention copied', description: path });
    else pushToast(client, { kind: 'error', title: 'Failed to copy mention', description: reply.error?.message || 'An error occurred.' });
    return;
  }
  if (picked === 'add-to-chat') {
    // insertTextAtEnd(`${mention} `, { ensureLeadingBoundary: true }): the end of the prompt, a space before it where words would join.
    const reply = await bridgeReply(native, { op: 'editorEdit', text: `${mention} `, pad: true });
    const value = obj(reply.value);
    if (reply.ok && value.applied === true) return;
    pushToast(client, value.reason === 'no composer'
      ? { kind: 'error', title: 'Unable to add to chat', description: 'Open a chat for this project and try again.' }
      : { kind: 'error', title: 'Unable to add to chat', description: "The chat isn't ready to accept input right now." });
    return;
  }
  if (absolutePath === null || environmentId === null) return;
  const reveal = picked === 'reveal-in-folder';
  const editor = picked === 'open' || reveal ? 'file-manager' : picked.slice('editor:'.length);
  try {
    await client.restAccess(native).request('shell.openInEditor', { cwd: absolutePath, editor, ...(reveal ? { reveal: true } : {}) });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: fileActionFailureTitle(picked), description: absolutePath });
  }
}

/**
 * A pull request number's right-click (`pageslocal:pr-link-menu`, value = "<provider> <url>"):
 * Copy link, then Open on the host. The host is named by the caller: the number belongs to
 * whichever host it was read from.
 */
export async function pullRequestLinkMenu(client: T3Client, native: Native, value: string): Promise<void> {
  const space = value.indexOf(' ');
  const provider = space < 0 ? '' : value.slice(0, space), url = space < 0 ? value : value.slice(space + 1);
  if (!/^https?:\/\//i.test(url)) return;
  let picked = '';
  try { picked = await showContextMenu(client, native, pullRequestLinkContextMenuItems(openOnHostLabel(provider))); }
  catch (error) { if (letGo(error)) throw error; return; } // a menu that could not show has already cost the right-click
  if (!picked) return;
  try {
    if (picked === 'copy-link') {
      const reply = await bridgeReply(native, { op: 'copyText', text: url });
      if (!reply.ok) throw new Error(reply.error?.message);
    } else {
      const result = await client.call(native, { op: 'terminalOpenExternal', url });
      if (result.opened !== true) throw new Error('not opened');
    }
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: picked === 'copy-link' ? 'Could not copy the link' : 'Could not open the link' });
  }
}
