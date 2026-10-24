import { requestComposerField } from '../composer/composer-field-request';
// What the Files tree row and pull request number menus do (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3): FileBrowserPanel.tsx showEntryContextMenu with
// fileContextMenu.ts useFileContextMenu.activate, and pullRequestLinkContextMenu.ts
// showPullRequestLinkContextMenu. Items come from context-menus.ts. Both are Contract context
// popovers (files-surface.contract FilesExplorer, pages-prs.contract PrLinkMenu), whose rows
// name their pick here.
import type { T3Client } from '../client/client';
import { bridgeReply, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { letGo } from '../client/let-go';
import { fileLink } from '../composer/composer-editor-menu';
import { workspaceOf } from './panel-surfaces';
import { buildFileContextMenuItems, fileActionFailureTitle, fileContextMenuCapabilities, fileTreeContextMenuItems, menuItemIds,
  pullRequestLinkContextMenuItems, resolveFileContextMenuAbsolutePath } from '../shared/context-menus';
import { menuRows, type MenuItem, type MenuRow } from '../sidebar/sidebar-menu';
import { openExternal } from '../client/external-open';

/**
 * The Files tree's one context popover (files-surface.contract FilesExplorer `files-tree-menu`): showEntryContextMenu's
 * items. A tree path is relative and has no `..`, so every row of one workspace resolves to an absolute path, or none
 * without a workspace root, and they share the menu.
 */
export function filesTreeMenuRows(client: T3Client): MenuRow[] {
  const environmentId = client.environmentId || null, { cwd } = workspaceOf(client);
  return menuRows(fileTreeItems(cwd !== '', environmentId, client));
}
const fileTreeItems = (hasAbsolutePath: boolean, environmentId: string | null, client: T3Client): MenuItem[] =>
  fileTreeContextMenuItems(buildFileContextMenuItems({ hasAbsolutePath, capabilities: fileContextMenuCapabilities(client.config, environmentId) }));

/** A choice from a Files tree row's menu (`shelllocal:surface-files-row-pick`, id = the row's workspace path, value = the item). */
export async function filesTreeChoice(client: T3Client, native: Native, relativePath: string, picked: string): Promise<void> {
  const path = relativePath.replace(/\/+$/, '');
  if (!path) return;
  const environmentId = client.environmentId || null, { cwd } = workspaceOf(client);
  const target = { environmentId, filePath: path, workspaceRoot: cwd || undefined };
  const absolutePath = resolveFileContextMenuAbsolutePath(target);
  if (!menuItemIds(fileTreeItems(absolutePath !== null, environmentId, client)).includes(picked)) return;
  const mention = fileLink(path);
  if (picked === 'copy-mention') {
    const reply = await bridgeReply(native, { op: 'copyText', text: mention });
    if (reply.ok) pushToast(client, { kind: 'success', title: 'Mention copied', description: path });
    else pushToast(client, { kind: 'error', title: 'Failed to copy mention', description: reply.error?.message || 'An error occurred.' });
    return;
  }
  if (picked === 'add-to-chat') {
    // insertTextAtEnd(`${mention} `, { ensureLeadingBoundary: true }): the end of the prompt, a space before it where words would join.
    const reply = await requestComposerField(native, { op: 'editorEdit', text: `${mention} `, pad: true });
    if (reply.accepted) return;
    pushToast(client, reply.reason === 'no composer'
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
 * A pull request number's menu choice (`pageslocal:pr-link-pick` from the list, `chatlocal:pr-link-pick` from the detail
 * header; value = "<choice>\n<provider> <url>", pages-prs.contract PrLinkMenu): Copy link, or Open on the host. The host is
 * named by the row: the number belongs to whichever host it was read from.
 */
export async function pullRequestLinkPick(client: T3Client, native: Native, value: string): Promise<void> {
  const newline = value.indexOf('\n'), picked = newline < 0 ? '' : value.slice(0, newline), link = value.slice(newline + 1);
  const space = link.indexOf(' '), url = space < 0 ? link : link.slice(space + 1);
  if (!/^https?:\/\//i.test(url) || !pullRequestLinkContextMenuItems('').some(item => item.id === picked)) return;
  try {
    if (picked === 'copy-link') {
      const reply = await bridgeReply(native, { op: 'copyText', text: url });
      if (!reply.ok) throw new Error(reply.error?.message);
    } else {
      if (!openExternal(url)) throw new Error('not opened');
    }
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: picked === 'copy-link' ? 'Could not copy the link' : 'Could not open the link' });
  }
}
