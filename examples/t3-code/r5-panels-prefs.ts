// Lane r5-panels: the Files surface preferences the reference keeps in
// localStorage (MIT reference, see LICENSE-T3: components/files/FilePreviewPanel.tsx
// FILE_EXPLORER_STORAGE_KEY "t3code.fileExplorerOpen" (default true),
// RENDER_MARKDOWN_STORAGE_KEY "t3code.renderMarkdown" (default false),
// RENDER_TABLE_STORAGE_KEY "t3code.renderTable" (default true)), kept inside the
// one versioned preference file the client persists (app:/data/t3-code.json)
// under `files`. Word wrap is the client setting `wordWrap`, already persisted
// with clientSettings. The client writes the file after every command.
import { obj, type Obj } from './domain';
import { adoptRightPanels } from './r10-device-panels';

// Lane r10-device: RENDER_BROWSER_FILE_STORAGE_KEY "t3code.renderBrowserFile" (default true), HTML pages.
export type FilesPrefs = { explorer: boolean; renderMarkdown: boolean; renderTable: boolean; renderBrowserFile?: boolean };
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
  (next as { files?: FilesPrefs }).files = prefs;
  adoptRightPanels(next, saved); // lane r10-device: the right panels, read with the Files preferences
}
