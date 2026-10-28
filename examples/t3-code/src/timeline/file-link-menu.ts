// ChatMarkdown's file-link menu rows (MIT reference, see LICENSE-T3, T3 Code 74541bab17:
// apps/web/src/components/ChatMarkdown.tsx MarkdownFileLink's onContextMenu; the items are context-menus.ts
// markdownFileMenuItems). The reference builds them when the menu opens; here each file chip carries them
// (`ChipView.menu`), so the chip's `contextmenu` can hand them to its context popover before the host
// presents it (link-menus.contract). files-surface.ts markdownFilePick acts on the row picked.
import type { T3Client } from '../client/client';
import { canUseMarkdownFileShellActions, remoteOpenFor } from '../connections/remote-open';
import { lastEditor, preferredEditor } from '../details/shell-details';
import { mediaMimeTypeFromExtension } from '../media/media-source';
import { workspaceOf } from '../panel/panel-surfaces';
import { availableEditorIds, markdownFileMenuItems, revealLabelFor } from '../shared/context-menus';
import type { ChipView } from './message-chips';
import { can } from '../connections/command-permissions';
import { isBrowserPreviewFile } from '../panel/files-rendered-html';
import { activeRef } from '../terminal/terminal-drawer-view';
import { splitPosition, workspaceRelativeFilePath } from './workspace-file-lookup';

/** Where a file link sits: `cwd` is the workspace its relative path is read in (the thread's by default); a link with no
 *  thread beside it (a pull request page) has `previewMedia` false, so neither media preview nor the Browser. */
export type FileLinkMenuContext = { cwd?: string; previewMedia?: boolean };

/**
 * The rows for the file link `target` (its full path, `:line` allowed): "Preview media" for a media file in a thread,
 * the open item named for the preferred editor and the server-worded reveal only where shell actions are allowed (a
 * resolved local environment, which needs the saved SSH aliases read: timeline-prepare.ts reads them before a
 * snapshot), "Open in integrated browser" for a page or a PDF where the thread's Browser can open (ChatMarkdown's
 * threadRef, canOperatePreview and isPreviewAvailableFor; a new thread's draft gets its id when it opens), then the
 * copies, the relative one for a file inside the workspace.
 */
export function markdownFileMenuRows(client: T3Client, target: string, context: FileLinkMenuContext = {}): { id: string; label: string }[] {
  if (!target) return [];
  const environment = client.environmentId || null, remote = remoteOpenFor(client);
  const canOpen = canUseMarkdownFileShellActions(environment, remote.state.mode, remote.resolved);
  const editor = preferredEditor(availableEditorIds(client.config), lastEditor(client)) || null;
  const filePath = splitPosition(target).path, name = filePath.slice(filePath.lastIndexOf('/') + 1), dot = name.lastIndexOf('.');
  const cwd = context.cwd ?? workspaceOf(client).cwd;
  const canPreviewMedia = context.previewMedia !== false && !!client.threadId && dot >= 0 && mediaMimeTypeFromExtension(name.slice(dot)) !== null;
  const thread = !!activeRef(client) || (!client.threadId && !!client.environmentId && !!client.projectId);
  const canOpenInBrowser = context.previewMedia !== false && thread && client.available === true && can(client, 'preview:operate') && isBrowserPreviewFile(filePath);
  return markdownFileMenuItems({ canPreviewMedia, canOpen, canOpenInBrowser, preferredEditor: editor, revealLabel: revealLabelFor(client.config, environment),
    canCopyRelative: workspaceRelativeFilePath(filePath, cwd) !== null }).map(item => ({ id: item.id, label: item.label }));
}

/**
 * Each file link chip's rows (a chip's `tip` is the link's full path), an image of a file of no media kind's too (its
 * file chip, media-views.ts markdownMediaChips); every other chip has none.
 */
export function withFileLinkMenus(client: T3Client, chips: ChipView[], context: FileLinkMenuContext = {}): ChipView[] {
  for (const chip of chips) chip.menu = chip.kind === 'link' || (chip.kind === 'media' && chip.size === 'file') ? markdownFileMenuRows(client, chip.tip, context) : [];
  return chips;
}
