// The Files surface preferences the reference keeps in localStorage (MIT reference, see LICENSE-T3:
// components/files/FilePreviewPanel.tsx, FileExplorerPane.tsx; T3 Code 2359643cff): FILE_EXPLORER_STORAGE_KEY
// "t3code.fileExplorerOpen" (default true), "t3code.fileExplorerWidth" (the explorer beside a file, default 256),
// RENDER_MARKDOWN_STORAGE_KEY "t3code.renderMarkdown" (default false), RENDER_TABLE_STORAGE_KEY "t3code.renderTable"
// (default true) and "t3code.renderMermaid" (default true), kept inside the one versioned preference file the client
// persists (app:/data/t3-code.json) under `files`. Word wrap's default is the client setting `wordWrap`, persisted with
// clientSettings; a preview's wrap button changes only that preview, for the session (files-surface.ts,
// attachment-surface.ts). The client writes the file after every command.
import { obj, type Obj } from '../client/domain';
import { adoptRightPanels } from './saved-panel';
import { adoptHideTestFiles } from '../diff/hide-test-files';
import { adoptThreadDetailsItems } from '../details/thread-details-items';

// RENDER_BROWSER_FILE_STORAGE_KEY "t3code.renderBrowserFile" (default true), HTML pages; renderMermaid, `.mmd` diagrams.
export type FilesPrefs = { explorer: boolean; renderMarkdown: boolean; renderTable: boolean; renderBrowserFile?: boolean; renderMermaid?: boolean; explorerWidth?: number };
type Holder = { local: object };

export const defaultFilesPrefs = (): FilesPrefs => ({ explorer: true, renderMarkdown: false, renderTable: true });

/** The live prefs object on the client's preference record (created on first use). */
export function filesPrefs(owner: Holder): FilesPrefs {
  const local = owner.local as { files?: FilesPrefs };
  if (!local.files || typeof local.files !== 'object') local.files = defaultFilesPrefs();
  return local.files;
}

/** load(): carry the saved `files` record into a fresh preference record; a missing or malformed key keeps its default. */
export function adoptFilesPrefs(next: object, saved: Obj): void {
  const value = obj(saved.files), prefs = defaultFilesPrefs();
  if (typeof value.explorer === 'boolean') prefs.explorer = value.explorer;
  if (typeof value.renderMarkdown === 'boolean') prefs.renderMarkdown = value.renderMarkdown;
  if (typeof value.renderTable === 'boolean') prefs.renderTable = value.renderTable;
  if (typeof value.renderBrowserFile === 'boolean') prefs.renderBrowserFile = value.renderBrowserFile;
  if (typeof value.renderMermaid === 'boolean') prefs.renderMermaid = value.renderMermaid;
  if (typeof value.explorerWidth === 'number' && Number.isFinite(value.explorerWidth)) prefs.explorerWidth = value.explorerWidth;
  (next as { files?: FilesPrefs }).files = prefs;
  adoptRightPanels(next, saved); // the right panels, read with the Files preferences
  adoptHideTestFiles(next, saved); // Hide test files in the Changes panel and a pull request's Code tab (hide-test-files.ts)
  adoptThreadDetailsItems(next, saved); // the sections the thread details card shows (thread-details-items.ts)
}
