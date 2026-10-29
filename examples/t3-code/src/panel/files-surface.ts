import { invalidateFileMetadata } from '../shared/file-metadata';
// The Files surface (MIT reference, see LICENSE-T3:
// components/files/FilePreviewPanel.tsx, FileBrowserPanel.tsx, useDirectoryEntries.ts,
// FileBreadcrumbs.tsx, fileSaveCoordinator.ts, filePreviewMode.ts and @pierre/trees'
// compact tree; T3 Code 7c3b3b3605ef): the workspace tree over `projects.listEntries` (one folder
// at a time, collapsed folders keep their children), path search over `projects.searchEntries`,
// the preview over `projects.readFile` with syntax colour, and edits written back with
// `projects.writeFile` (latest contents win; a save in flight is followed by the newest). A
// folder crumb moves the browser to its folder (`navigateFolderIn`), and a host folder outside
// the workspace gets a tree of its own, listed with that folder as `cwd`.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { announce } from '../shell/announce'; // "Opened in <editor>"
import { editorLabel } from '../shared/editors';
import { fileIconToken } from '../shared/timeline-files';
import { lineTokens } from '../highlight/timeline-diff-syntax';
import { EDITORS, lastEditor, preferredEditor, rememberEditor } from '../details/shell-details';
import { workspaceOf, panelKey, panelState, navigateFolderIn, type Surface, type PanelState } from './panel-surfaces';
import { breadcrumbPathOf, fileBreadcrumbs, isHostPath, isListableHostFolder, isWindowsAbsolutePath, joinHostPath, retainedBreadcrumbTrail, trailChildOf } from './files-path';
import { markdownEnv, messageChips, type MarkdownEnv } from '../timeline/message-chips'; // the chat renderer's chips and settings
import { messageCodeBlocks } from '../highlight/timeline-highlight';
import { filesMermaid, type MermaidDiagramView } from '../timeline/timeline-mermaid';
import { timelineView } from '../timeline/timeline-presentation';
import { filesPrefs, type FilesPrefs } from './files-prefs';
import { crumbsMask } from './files-crumbs-scroll'; // the trail scrolled to its end
import { crumbsOffset, noteFirstRead, settleCrumbs, settleMounted, sourceGutter } from './files-crumbs-settle'; // where the trail settles
import { audioPage, forgetAudioPage, htmlPage, htmlToggleLabel, isBrowserPreviewFile, isHtmlPath, isPdfPath } from './files-rendered-html'; // rendered HTML, PDFs and audio
import { isMarkdownPreviewFile, isMermaidPreviewFile, isWorkspaceAudioPreviewPath, isWorkspaceDocumentPath, maskMarkdownFrontmatter } from './file-preview-kind';
import { attachmentOp, attachmentView, emptyAttachment, forgetPreview, type AttachmentMeta, type AttachmentView, type PreviewSource } from './attachment-surface'; // a document's or a binary file's body
import { closedFullFile, filesViewer, filesViewerHidden, filesViewerOp, refreshFilesViewer, type FullFileView } from './files-viewer'; // a PDF, a table or a full file in the viewer page
import { emptyViewerFrame, type ViewerFrameView } from './viewer-frames';
import { mermaidFile } from './mermaid-file'; // a Mermaid file's diagram
import { openInlineDocumentImage } from '../timeline/timeline-attachments'; // the expanded image dialog
import { openFileInPreview } from '../browser/browser-links'; // "Open file in preview browser"
import { activeRef } from '../terminal/terminal-drawer-view';
import { ensureDraftThreadId } from '../composer/draft-thread-id';
import { crumbsMounting, loadBegin, loadEnd, missingFolders, noteReveal, revealStale } from './files-crumbs-mount'; // a mounting preview settles at the end
import { loadSshAliases, openInEditorHere, openInView, remoteOpenFor } from '../connections/remote-open'; // remote Open (OpenInPicker)
import { fileComment, fileCommentLines, fileCommentOpen, focusedFileDraft, type FileLine } from '../diff/diff-file-comments'; // diff-review: line comments on the preview
import { filesMediaView, markdownMediaChips, markdownMediaUrls, NO_MEDIA, type MediaView } from '../media/media-views'; // image and video files with their menu
import { letGo } from '../client/let-go';
import { filesTreeChoice, filesTreeMenuRows } from './context-menu-actions';
import { markdownFileMenuRows, withFileLinkMenus, type FileLinkMenuContext } from '../timeline/file-link-menu';
import type { MenuRow } from '../sidebar/sidebar-menu';
import { availableEditorIds } from '../shared/context-menus';
import { isWorkspaceImagePreviewPath, isWorkspaceVideoPreviewPath } from '../media/media-source';
import { openMarkdownMediaPreview } from '../timeline/timeline-attachments';
import { fileLinkCopyPaths, fileLinkEditorTarget, resolveFileLinkHostPath, splitPosition } from '../timeline/workspace-file-lookup'; // the file a chat link means
import { mediaFrameSource } from '../media/media-frame'; // a rendered page's frame source

export type TreeRow = { id: string; path: string; name: string; depth: number; directory: boolean; expanded: boolean; selected: boolean; token: string; ignored: boolean; loading: boolean; guides: { id: string; left: number }[] };
/** A crumb: `navigable` moves the browser to its folder (FolderBreadcrumb), `descendant` is below the current one (kept
 *  from where the browser came up), `menu` puts the folder's contents behind the chevron after it (FolderContentsMenu). */
export type Crumb = { id: string; label: string; path: string; current: boolean; navigable: boolean; descendant: boolean; menu: boolean; last: boolean };
export type CodeRun = { id: string; text: string; syntax: string };
export type CodeLine = { id: string; number: string; runs: CodeRun[] };
export type EditorChoice = { id: string; label: string; selected: boolean };
export type CrumbEntry = { id: string; path: string; label: string; directory: boolean; token: string; current: boolean; ignored: boolean };
/** `status` stands for the entries (loading, empty, or a failed first listing, which `statusRetry` makes a retry row);
 *  `retry` follows the entries when their refresh failed (FileBreadcrumbs "Refresh failed. Retry"). */
export type CrumbMenu = { open: boolean; root: string; back: string; backPath: string; dir: string; status: string; statusRetry: boolean; retry: string; entries: CrumbEntry[] };
export type FilesView = {
  cwd: string; project: string; ready: boolean; loading: boolean; searching: boolean; error: string; query: string; truncated: boolean;
  /** A search that matched nothing hides the tree (`noMatches`) and, once settled, says so (`emptySearch`); "Loading files…"
   *  stays a screen reader's alone while rows hold their place (`loadingQuiet`). */
  noMatches: boolean; emptySearch: boolean; loadingQuiet: boolean;
  rows: TreeRow[]; hasDirectories: boolean; allExpanded: boolean; explorer: boolean; showExplorer: boolean;
  /** The tree rows' one context popover (context-menu-actions.ts filesTreeMenuRows). */
  treeMenu: MenuRow[];
  /** The subheader shows: a file or folder, or the explorer the crumbs walked up to. */
  subheader: boolean; explorerToggle: boolean;
  /** A read error's attempted path and, for a workspace file, the workspace folder (FilePreviewPanel's read error). */
  path: string; preview: string; previewError: string; errorPath: string; errorCwd: string; crumbs: Crumb[]; lines: FileLine[]; commentOpen: boolean; text: string; textKey: string;
  gutter: number; wrap: boolean; rawText: boolean; openInBrowser: boolean; truncatedNote: string; canRender: boolean; rendered: boolean; renderLabel: string; renderIcon: string;
  editable: boolean; pending: boolean; editorId: string; editorLabel: string; editorShow: boolean; editorHint: string; editorUnavailable: string; editors: EditorChoice[]; absolutePath: string;
  // FileMarkdownPreview is ChatMarkdown. `markdownSource` is renderMarkdown's one
  // message (app.contract filesMarkdown; kind "file" when its task checkboxes may write the file), `md` its chips and
  // settings, `code` its fences' colours, `diagrams` its Mermaid fences, `mdUrls` its host-path media and `codeCopied` /
  // `codeCopyNonce` Copy code's check.
  markdownSource: MarkdownSource[]; md: MarkdownEnv; code: ReturnType<typeof messageCodeBlocks>; diagrams: MermaidDiagramView[]; mdUrls: { id: string; url: string; fill: string; hover: string; border: string; ink: string }[];
  codeCopied: string; codeCopyNonce: number; editing: boolean; editorText: string; editPress: string; crumbMenu: CrumbMenu; crumbsMask: string; crumbsOffset: number;
  /** A rendered page's frame source, or an audio file's signed URL. */
  url: string; media: MediaView;
  /** A Mermaid file the reference will not lay out names why (MermaidViewer); `diagrams` holds the file's diagram otherwise. */
  mermaidNote: string;
  /** Bumped when a change from outside replaced the editor's text. */
  editorRevision: number;
  /** The file shown, by environment, thread and path, as FilePreviewPanel keys its audio player. */
  fileKey: string;
  /** A document, or a file read as binary: AttachmentFilePreview under "File" (`preview == "attachment"`). */
  attachment: AttachmentView;
  /** A PDF, a rendered table or a text file past 1 MB in the viewer page (`preview == "viewer"`, files-viewer.ts). */
  viewer: ViewerFrameView; fullFile: FullFileView;
};
export type MarkdownSource = { id: string; kind: string; title: string; body: string };
type Entry = { path: string; kind: 'file' | 'directory'; ignored: boolean };
type Read = { contents: string; byteLength: number; truncated: boolean; error: string; notFile: boolean; attempted: string; errorCwd: string; binary?: boolean };
/** A host folder's own tree (FileBrowserPanel `hostFolder`): listed with the folder as `cwd`, searched over what is loaded. */
type Tree = { dirs: Map<string, Entry[]>; errors: Map<string, string>; requested: Set<string>; expanded: Set<string>; loadingDirs: Set<string>; query: string; revealed: string };
type Edit = { contents: string; revision: number; confirmed: number; saving: boolean; error: string };
type FilesState = {
  key: string; dirs: Map<string, Entry[]>; errors: Map<string, string>; requested: Set<string>; expanded: Set<string>; expandAll: boolean;
  // A folder's first load (not a refresh) shows a spinner in its row (useDirectoryEntries loadingDirectories).
  loadingDirs: Set<string>; hosts: Map<string, Tree>; searching: boolean;
  query: string; search: { query: string; entries: Entry[]; truncated: boolean; error: string } | null;
  // The mounted explorer the query belongs to: its thread and tab (FileBrowserPanel's `query` is component state).
  queryOwner: string;
  reads: Map<string, Read>; edits: Map<string, Edit>; loading: number; editing: string; editorText: string; crumb: { root: string; dir: string } | null;
  /** The workspace mutation the last view saw (ChatView's workspaceMutationId); undefined before the first view. */
  mutation: string | null | undefined;
  /** The workspace mutation each path was last read under (revisionedQuery's readRevisions): a file shown after a change
   *  reads again, the open tab's and every other tab's. */
  readRevisions: Map<string, string | null>;
  /** Counts the times a change from outside replaced the editor's text (files-surface.contract keys the editor on it). */
  editorRevision: number;
  /** Audio URLs whose player reported an error (WorkspaceAudioPreview's failedUrl). */
  audioFailed: Set<string>;
  /** The document or binary file shown in the attachment body, whose controls its `att-*` ops reach. */
  attachment: { meta: AttachmentMeta; source: PreviewSource } | null;
  /** Counts the explorer's Refresh presses (FilePreviewPanel's fileRefreshRevision): the attachment body is keyed on it. */
  refreshRevision: number;
  /** FilePreviewPanel's `handledReveal`: the line request the rendered toggle answered, after which it renders. */
  handledReveal: { path: string; requestId: number } | null;
};
// Preferences the reference keeps in localStorage (t3code.fileExplorerOpen, t3code.renderMarkdown, t3code.renderTable),
// persisted in the client's preference file (files-prefs.ts).
const states = new WeakMap<T3Client, FilesState>();
/** The last press that began editing, as the preview numbered it (files-surface.contract FilePreview `beginEdit`); kept
 * across workspaces, so a press's mark never outlives the module's answer to it. */
const editPresses = new WeakMap<T3Client, string>();
export const PATH_SEARCH_LIMIT = 200;
const EXPAND_ALL_CAP = 200;

const prefsOf = (client: T3Client): FilesPrefs => filesPrefs(client);
export function filesState(client: T3Client): FilesState {
  const { cwd } = workspaceOf(client), key = `${client.environmentId}|${cwd}`;
  let state = states.get(client);
  if (!state || state.key !== key) {
    // Another environment or workspace: the body shown in the last one unmounts.
    if (state?.attachment) forgetPreview(client, state.attachment.source.key);
    state = { key, dirs: new Map(), errors: new Map(), requested: new Set(), expanded: new Set(), expandAll: false, loadingDirs: new Set(), hosts: new Map(), searching: false, query: '', search: null, queryOwner: '', reads: new Map(), edits: new Map(), loading: 0, editing: '', editorText: '', crumb: null, mutation: undefined, readRevisions: new Map(), editorRevision: 0, audioFailed: new Set(), attachment: null, refreshRevision: 0, handledReveal: null };
    states.set(client, state);
  }
  return state;
}
const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf('/')));
const baseName = (path: string) => path.slice(path.lastIndexOf('/') + 1);
const message = (error: unknown) => error instanceof Error && error.message ? error.message : 'Unable to load folder.';
export const isMarkdownPath = isMarkdownPreviewFile;
/** filePreviewReadErrorMessage: a read failure described by its code without the platform's cause; an older server's own message. */
export function filePreviewReadErrorMessage(context: Readonly<Record<string, string>>, fallback: string): string {
  switch (context.failure) {
    case 'path_not_file': return 'The path is a directory or special file, not a regular file.';
    case 'binary_file': return 'The file is binary and cannot be displayed as text.';
    case 'workspace_path_outside_root': return 'The requested path is outside the workspace.';
    case 'resolved_path_outside_root': return 'The path resolves to a location outside the workspace.';
    // A realpath failure can mean a missing path, permissions, or another I/O error.
    case 'operation_failed': return context.operation === 'realpath-workspace-root' ? 'The workspace folder could not be accessed.' : 'The file could not be accessed or read. It may be missing or inaccessible.';
    default: return fallback;
  }
}
export const isTablePath = (path: string) => /\.(?:csv|tsv)$/i.test(path);
const isHost = isHostPath;
function hostTree(state: FilesState, folder: string): Tree {
  let tree = state.hosts.get(folder);
  if (!tree) { tree = { dirs: new Map(), errors: new Map(), requested: new Set(), expanded: new Set(), loadingDirs: new Set(), query: '', revealed: '' }; state.hosts.set(folder, tree); }
  return tree;
}
/** A host folder listing's entry path (`/`-separated, relative to the folder) and back. */
function hostRelative(folder: string, path: string): string {
  const separator = isWindowsAbsolutePath(folder) ? '\\' : '/', base = folder.endsWith(separator) ? folder : `${folder}${separator}`;
  return path.startsWith(base) ? path.slice(base.length).split(separator).join('/') : path;
}
/** Pierre's order: folders before files, then a case-insensitive natural name order. */
export function sortEntries(entries: Entry[]): Entry[] {
  return [...entries].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === 'directory' ? -1 : 1)
    || baseName(a.path).localeCompare(baseName(b.path), undefined, { sensitivity: 'base', numeric: true }));
}
const toEntry = (value: Obj): Entry | null => {
  const path = str(value.path).replace(/\/+$/, '');
  return path ? { path, kind: value.kind === 'directory' ? 'directory' : 'file', ignored: value.ignored === true } : null;
};

/** useDirectoryEntries.load: one folder's immediate children, kept when it collapses; `host` lists a host folder's tree. */
async function loadDirectory(client: T3Client, native: Native, path: string, refresh = false, host = ''): Promise<void> {
  const state = filesState(client), cwd = host || workspaceOf(client).cwd, tree = host ? hostTree(state, host) : state;
  if (!cwd || (!refresh && tree.dirs.has(path))) return;
  const key = host ? `host:${host}|${path}` : `dir:${path}`;
  tree.requested.add(path); loadBegin(state, key); // counted by what is asked
  if (!refresh) tree.loadingDirs.add(path);
  try {
    const result = await client.restAccess(native).request('projects.listEntries', { cwd, directoryPath: path });
    if (filesState(client) !== state) return;
    const entries = arr(result.entries).map(toEntry).filter((entry): entry is Entry => !!entry && parentOf(entry.path) === path);
    tree.dirs.set(path, sortEntries(entries)); tree.errors.delete(path);
  } catch (error) {
    if (filesState(client) === state && !letGo(error)) tree.errors.set(path, message(error));
  } finally { loadEnd(state, key); if (!refresh) tree.loadingDirs.delete(path); }
}
export async function ensureTree(client: T3Client, native: Native): Promise<void> {
  await loadDirectory(client, native, '');
}
/** projectFilesQueryState: the file's contents (a folder answers "not a file", which keeps the tree). */
async function readFile(client: T3Client, native: Native, path: string): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  if (!cwd) return;
  if (!state.reads.has(path)) noteFirstRead(state, path); // an uncached read settles the crumbs short
  // Even a failed read counts as read under this revision: the next change, not the next view, asks again.
  state.readRevisions.set(path, workspaceMutationId(client));
  loadBegin(state, `read:${path}`);
  try {
    // A missing t3.json is not a failed read (getProjectFileQueryAtom's allowMissing); it reads as absent.
    const result = await client.restAccess(native).request('projects.readFile', { cwd, relativePath: path, ...(path === 't3.json' ? { allowMissing: true } : {}) });
    if (filesState(client) !== state) return;
    // An edit made while this read was out is saved next, and its text is what the file holds then.
    if (pendingPaths(client).has(path)) return;
    if (result.exists === false) { state.reads.set(path, { contents: '', byteLength: 0, truncated: false, error: `'${path}' does not exist.`, notFile: false, attempted: '', errorCwd: '' }); return; }
    const previous = state.reads.get(path), contents = str(result.contents);
    state.reads.set(path, { contents, byteLength: num(result.byteLength), truncated: result.truncated === true, error: '', notFile: false, attempted: '', errorCwd: '' });
    // EditableFileSurface: contents changed from outside (the agent's edit) replace the editor's text, whose edits are
    // all saved; the editor mounts again on them.
    if (state.editing === path && previous && !previous.error && previous.contents !== contents) { state.editorText = contents; state.editorRevision++; }
  } catch (error) {
    if (filesState(client) !== state || letGo(error)) return;
    // projectFilesQueryState: the decoded ProjectReadFileError (its failure, and the path it tried), else the message.
    const context = error instanceof ClientError ? error.context : {}, text = filePreviewReadErrorMessage(context, message(error));
    const notFile = context.failure ? context.failure === 'path_not_file' : /not a file|path_not_file|is a directory/i.test(text);
    // A failed refresh keeps the contents last read (useProjectFileQuery's retained value), so the preview and its
    // Open in (`isFile`) stay as they were; only "not a file" replaces them.
    const previous = state.reads.get(path);
    if (!notFile && previous && !previous.error) {
      // It still exposes the error (projectFilesQueryState's readError): a file now read as binary is the attachment
      // body's, whose branch comes before the text's.
      if (context.failure === 'binary_file') state.reads.set(path, { ...previous, binary: true });
      return;
    }
    state.reads.set(path, { contents: '', byteLength: 0, truncated: false, error: text, notFile, attempted: context.resolvedPath || context.operationPath || '', errorCwd: context.cwd || cwd, binary: context.failure === 'binary_file' });
  } finally { loadEnd(state, `read:${path}`); }
}
/** Opening a file reads it and reveals it: each ancestor folder loads and expands (FileBrowserPanel's reveal). */
export async function ensureFile(client: T3Client, native: Native, path: string, reveal: boolean, fromTree = false): Promise<void> {
  const state = filesState(client);
  state.crumb = null;
  const tasks: Promise<void>[] = [];
  if ((!state.reads.has(path) || reveal) && state.editing !== path) tasks.push(readFile(client, native, path));
  if (state.editing && state.editing !== path) state.editing = '';
  if (!isHost(path)) {
    const segments = path.split('/');
    for (let index = 0; index < segments.length; index++) {
      const folder = segments.slice(0, index).join('/');
      tasks.push(loadDirectory(client, native, folder));
      if (reveal && index > 0) state.expanded.add(folder);
    }
    // An outside open (palette, link) closes the tree's search; a row picked in the search keeps it.
    if (reveal && !fromTree) { state.query = ''; state.search = null; }
  }
  await Promise.all(tasks);
  // A path that turned out to be a folder is revealed and expanded in the tree instead; a host folder lists its own tree.
  if (reveal && state.reads.get(path)?.notFile && !isHost(path)) { state.expanded.add(path); await loadDirectory(client, native, path); }
}
export function reconcileFiles(panel: PanelState): void {
  const before = panel.surfaces.length;
  panel.surfaces = panel.surfaces.filter(entry => entry.kind !== 'files' && entry.kind !== 'file');
  if (panel.surfaces.length === before) return;
  if (!panel.surfaces.some(entry => entry.id === panel.active)) panel.active = panel.surfaces[panel.surfaces.length - 1]?.id ?? '';
  if (!panel.surfaces.length) panel.visible = false;
}

async function search(client: T3Client, native: Native, query: string): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  state.query = query;
  const trimmed = query.trim().slice(0, 256);
  if (!trimmed || !cwd) { state.search = null; state.searching = false; return; }
  loadBegin(state, 'search'); state.searching = true;
  try {
    const result = await client.restAccess(native).request('projects.searchEntries', { cwd, query: trimmed, limit: PATH_SEARCH_LIMIT });
    if (filesState(client) !== state || state.query !== query) return;
    state.search = { query: trimmed, entries: arr(result.entries).map(toEntry).filter((entry): entry is Entry => !!entry), truncated: result.truncated === true, error: '' };
  } catch (error) {
    if (filesState(client) === state && state.query === query && !letGo(error)) state.search = { query: trimmed, entries: [], truncated: false, error: message(error) };
  } finally { loadEnd(state, 'search'); if (state.query === query) state.searching = false; }
}

/** Folders expand all opens: expanding lists a folder, and an ignored one such as `node_modules` can hold tens of
 *  thousands of entries, so ignored folders are left for a person to open (buildTreeEntries expandableDirectoryPaths). */
const expandableFolders = (state: FilesState) => [...state.dirs.values()].flat().filter(entry => entry.kind === 'directory' && !entry.ignored);
/** setAllDirectoriesExpanded, re-applied as folders load (the reference's expandAll effect), up to a bound. */
async function expandAll(client: T3Client, native: Native): Promise<void> {
  const state = filesState(client);
  for (let round = 0; round < 32; round++) {
    const folders = expandableFolders(state).map(entry => entry.path);
    const fresh = folders.filter(path => !state.dirs.has(path)).slice(0, EXPAND_ALL_CAP - state.dirs.size);
    for (const path of folders) state.expanded.add(path);
    if (!fresh.length || state.dirs.size >= EXPAND_ALL_CAP) return;
    for (let index = 0; index < fresh.length; index += 4) await Promise.all(fresh.slice(index, index + 4).map(path => loadDirectory(client, native, path)));
  }
}

/** The editor's change: the latest contents are written; a save in flight is followed by the newest (fileSaveCoordinator). */
export async function editFile(client: T3Client, native: Native, path: string, contents: string): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  if (!cwd || isHost(path)) return;
  const edit = state.edits.get(path) ?? { contents, revision: 0, confirmed: 0, saving: false, error: '' };
  edit.contents = contents; edit.revision++;
  state.edits.set(path, edit);
  const read = state.reads.get(path);
  if (read) state.reads.set(path, { ...read, contents });
  if (edit.saving) return;
  while (edit.confirmed !== edit.revision) {
    const revision = edit.revision, text = edit.contents;
    edit.saving = true;
    try {
      invalidateFileMetadata(client, path, cwd);
      await client.restAccess(native).request('projects.writeFile', { cwd, relativePath: path, contents: text }, true);
      invalidateFileMetadata(client, path, cwd);
      edit.confirmed = revision; edit.error = '';
    } catch (error) {
      if (letGo(error)) { edit.saving = false; throw error; }
      edit.error = message(error); edit.saving = false;
      pushToast(client, { kind: 'error', title: 'Failed to save file', description: edit.error, stacked: true });
      return;
    }
    edit.saving = false;
  }
}
/** setMarkdownTaskChecked (filePreviewMode.ts): the marker at `markerOffset` checked or cleared; anything else unchanged. */
export function setMarkdownTaskChecked(markdown: string, markerOffset: number, checked: boolean): string {
  if (markerOffset < 0 || markdown[markerOffset] !== '[' || !/[ xX]/.test(markdown[markerOffset + 1] ?? '') || markdown[markerOffset + 2] !== ']') return markdown;
  return `${markdown.slice(0, markerOffset + 1)}${checked ? 'x' : ' '}${markdown.slice(markerOffset + 2)}`;
}
/** RenderedMarkdownSurface onTaskListChange: the open file's task toggled in its latest contents and saved like an edit
 * (`value` = "<marker offset>\t<checked>", from markdown.contract TaskCheckbox). */
async function toggleTask(client: T3Client, native: Native, value: string): Promise<void> {
  const panel = panelState(client), active = panel.surfaces.find(entry => entry.id === panel.active);
  if (active?.kind !== 'file' || !isMarkdownPath(active.path) || isHost(active.path)) return;
  const read = filesState(client).reads.get(active.path);
  if (!read || read.error || read.truncated) return;
  const [offset, checked] = value.split('\t');
  const next = setMarkdownTaskChecked(read.contents, Number(offset), checked === 'true');
  if (next !== read.contents) await editFile(client, native, active.path, next);
}
/** Paths with an unconfirmed edit: their tabs carry the pending dot. */
export function pendingPaths(client: T3Client): Set<string> {
  const state = states.get(client);
  return new Set(state ? [...state.edits].filter(([, edit]) => edit.confirmed !== edit.revision).map(([path]) => path) : []);
}

/** FilePreviewPanel's place for a Files or file surface: its path (the workspace root itself is the explorer), whether
 *  it is a folder, the host folder it lists, and the crumbs' current path and trail (`null` without a subheader). */
type Place = { path: string; folder: boolean; hostFolder: string; crumbPath: string | null; trail: string | null };
function placeOf(client: T3Client, state: FilesState, active: Surface): Place {
  const root = workspaceOf(client).cwd.replace(/[\\/]+$/, ''), requested = active.kind === 'file' ? active.path : '';
  const path = requested === '.' || (!!root && requested.replace(/[\\/]+$/, '') === root) ? '' : requested;
  const read = path ? state.reads.get(path) : undefined, trail = active.folderTrail;
  // The trail only continues below folders, so a surface with one is a folder before its read settles.
  const folder = !!path && (!!read?.notFile || (trail !== undefined && read === undefined));
  // The explorer shows crumbs only when they walked the browser up to it.
  const crumbPath = active.kind === 'file' || active.kind === 'files' ? (path || (trail === undefined ? null : '')) : null;
  return { path, folder, hostFolder: folder && isHost(path) ? path : '', crumbPath, trail: crumbPath === null ? null : retainedBreadcrumbTrail(trail, crumbPath) };
}
const activeSurface = (client: T3Client) => { const panel = panelState(client); return panel.surfaces.find(entry => entry.id === panel.active); };
/**
 * FilePreviewPanel's renderMarkdown / renderMermaid / renderTable / renderBrowserFile: the preference, once a request to
 * open the file at a line is answered (`revealHandled`); until then the source shows, where the line is.
 */
function renderedShown(client: T3Client, state: FilesState, path: string, active: Surface | undefined): boolean {
  const preferences = prefsOf(client);
  const preferred = (isMarkdownPath(path) && preferences.renderMarkdown) || (isMermaidPreviewFile(path) && preferences.renderMermaid !== false)
    || (isTablePath(path) && preferences.renderTable) || (isHtmlPath(path) && preferences.renderBrowserFile !== false);
  const revealHandled = !active || !(active.line > 0) || (state.handledReveal?.path === path && state.handledReveal.requestId === active.reveal);
  return !!preferred && revealHandled;
}
/** The host folder the active surface lists, or ''. */
function activeHostFolder(client: T3Client): string {
  const active = activeSurface(client);
  return active ? placeOf(client, filesState(client), active).hostFolder : '';
}
/** FilePreviewPanel navigateToFolder: a file keeps its tab and its folder opens beside it; a folder browser moves in
 *  place (rightPanelStore.navigateFolder). The trail's end keeps its own spelling (`C:/a` or `C:\a`), so it finds its tab. */
async function navigateCrumb(client: T3Client, native: Native, crumbPath: string): Promise<void> {
  const panel = panelState(client), active = activeSurface(client), state = filesState(client);
  if (!active) return;
  const place = placeOf(client, state, active);
  if (place.trail === null) return;
  const path = breadcrumbPathOf(place.trail) === crumbPath ? place.trail : crumbPath, open = new Set(panel.surfaces.map(entry => entry.id));
  const surface = navigateFolderIn(panel, active.id, path, retainedBreadcrumbTrail(place.trail, path), !!place.path && !place.folder);
  state.crumb = null;
  // A new tab reveals its path; a tab that already showed it is only switched to.
  if (surface?.kind === 'file') await ensureFile(client, native, surface.path, !open.has(surface.id));
  else if (surface) await ensureTree(client, native);
}

/** `shelllocal:surface-files-*`. */
export async function filesLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op === 'link-pick') { const newline = value.indexOf('\n'); await markdownFilePick(client, native, id, value.slice(0, newline), { lookup: value.slice(newline + 1) }); return ''; } // link-menus.contract
  if (op === 'row-pick') { await filesTreeChoice(client, native, id, value); return ''; } // context-menu-actions.ts
  const state = filesState(client), preferences = prefsOf(client);
  // A host folder's rows carry host paths (FileBrowserPanel panelPath); its tree is keyed by the folder's own entries.
  const host = op === 'toggle' || op === 'search' || op === 'search-key' || op === 'refresh' ? activeHostFolder(client) : '';
  if (op === 'toggle') {
    const tree = host && isHost(id) ? hostTree(state, host) : state, path = tree === state ? id : hostRelative(host, id);
    if (tree.expanded.has(path)) { tree.expanded.delete(path); if (tree === state) state.expandAll = false; }
    else { tree.expanded.add(path); await loadDirectory(client, native, path, false, tree === state ? '' : host); }
    return '';
  }
  // A host folder can be a home directory, so its search filters the folders already loaded instead of indexing on the server.
  if (op === 'search') { if (host) hostTree(state, host).query = value; else await search(client, native, value); return ''; }
  if (op === 'crumb-go') { await navigateCrumb(client, native, id === '.' ? '' : id); return ''; }
  if (op.startsWith('comment-')) return fileComment(client, native, op.slice(8), id, value, state.reads.get(id)?.contents ?? '', panelKey(client)); // diff-file-comments.ts
  // FileSearchField: its Escape closes the search (closeSearch); the field blurs itself (FilesExplorer searchKey).
  if (op === 'search-key') { if (value === 'Escape') { if (host) hostTree(state, host).query = ''; else await search(client, native, ''); } return ''; }
  if (op === 'begin-edit') {
    if (value) editPresses.set(client, value);
    const read = state.reads.get(id);
    if (read && !read.error && !read.truncated && !isHost(id)) { state.editing = id; state.editorText = read.contents; }
    return '';
  }
  if (op === 'end-edit') { if (state.editing === id) state.editing = ''; return ''; }
  if (op === 'refresh') {
    const visited = [...(host ? hostTree(state, host) : state).requested];
    for (let index = 0; index < visited.length; index += 4) await Promise.all(visited.slice(index, index + 4).map(path => loadDirectory(client, native, path, true, host)));
    if (!host && state.query.trim()) await search(client, native, state.query);
    const active = panelState(client).surfaces.find(entry => entry.id === panelState(client).active);
    // FilePreviewPanel's Refresh: the body mounts afresh (fileRefreshRevision); a document is not read as text.
    state.refreshRevision++;
    refreshFilesViewer(client);
    if (active?.kind === 'file' && !isWorkspaceDocumentPath(active.path) && !state.edits.get(active.path)?.saving) await readFile(client, native, active.path);
    return '';
  }
  if (op === 'expand-all') {
    const expanded = !(state.expandAll || allExpanded(state));
    state.expandAll = expanded;
    if (expanded) await expandAll(client, native);
    else state.expanded.clear();
    return '';
  }
  if (op === 'explorer') { preferences.explorer = !preferences.explorer; return ''; }
  if (op === 'render') {
    // FilePreviewPanel's toggle: it flips what shows, and turning rendering on answers a pending line request.
    const active = activeSurface(client), pressed = !renderedShown(client, state, id, active);
    if (isMermaidPreviewFile(id)) preferences.renderMermaid = pressed;
    else if (isMarkdownPath(id)) preferences.renderMarkdown = pressed;
    else if (isHtmlPath(id)) preferences.renderBrowserFile = pressed;
    else preferences.renderTable = pressed;
    state.handledReveal = pressed && active ? { path: id, requestId: active.reveal } : null;
    return '';
  }
  // PreviewImageContent's press: the expanded image dialog over the picture (`id` its URL, `value` its name).
  if (op === 'image-open') { if (/^https?:/i.test(id)) openInlineDocumentImage(client, id, value, 'file-image-zoom'); return ''; }
  // The attachment body's controls (Save file, Try again, a font's failure) on the document shown.
  if (op.startsWith('att-')) { if (state.attachment) await attachmentOp(client, native, state.attachment.meta, state.attachment.source, op.slice(4), 0); return ''; }
  // The viewer page's messages and the Full file subheader (files-viewer.ts).
  if (op.startsWith('viewer') || op.startsWith('full-')) { await filesViewerOp(client, native, op, 0); return ''; }
  // WorkspaceAudioPreview: the player's error for this URL, and Try again, which signs the file afresh.
  if (op === 'audio-error') { if (id) state.audioFailed.add(id); return ''; }
  if (op === 'audio-retry') { state.audioFailed.clear(); forgetAudioPage(client, workspaceOf(client).cwd, id); return ''; }
  if (op === 'open-browser') { await openInPreviewBrowser(client, native, id); return ''; }
  if (op === 'wrap') { client.local.clientSettings.wordWrap = client.local.clientSettings.wordWrap === false; return ''; }
  if (op === 'edit') { await editFile(client, native, id, value); return ''; }
  if (op === 'task') { await toggleTask(client, native, value); return ''; }
  // DirectoryBreadcrumb: a folder crumb opens its listing; folders navigate inside it, Back climbs toward the crumb.
  // The crumb's popover says when its menu opens and closes; another crumb's menu opening may come before the
  // first one's closing, which then leaves the new listing alone.
  if (op === 'crumb') { const root = id === '.' ? '' : id; state.crumb = { root, dir: root }; await loadDirectory(client, native, root); return ''; }
  if (op === 'crumb-dir') { if (state.crumb) { state.crumb.dir = id; await loadDirectory(client, native, id); } return ''; }
  // The menu's "Refresh failed. Retry": its folder listed again.
  if (op === 'crumb-refresh') { if (state.crumb?.dir === id) await loadDirectory(client, native, id, true); return ''; }
  if (op === 'crumb-close') { if (!id || state.crumb?.root === (id === '.' ? '' : id)) state.crumb = null; return ''; }
  if (op === 'open-editor') {
    const choices = await editorFor(client, native);
    if (!choices.editorShow) return '';
    if (choices.editorUnavailable) { pushToast(client, { kind: 'error', title: 'Unable to open in editor', description: choices.editorUnavailable }); return ''; }
    const editor = value || choices.editorId;
    if (!editor || !choices.editors.some(choice => choice.id === editor)) { pushToast(client, { kind: 'error', title: 'Unable to open in editor', description: `No available editor can open ${id}.` }); return ''; }
    // Remote Open as the details card does (remote-open.ts). OpenInPicker announces the primary press's open
    // ("Opened in <editor>"), as the ⌘O shortcut's, which names no editor either; a choice from its menu (`value`) says nothing.
    if (await openInEditorHere(client, native, id, editor, openTarget(client, state))) { rememberEditor(client, editor); if (!value) announce(client, `Opened in ${editorLabel(editor)}`); }
    return '';
  }
  return '';
}

/**
 * A row of ChatMarkdown's file-link menu (file-link-menu.ts markdownFileMenuRows) for the link `target` (its full path),
 * acted on only while the menu, built again now, still offers it. Open, reveal and the copies act on the file a bare
 * name or a repeated project prefix means (`lookup`, workspace-file-lookup.ts), and a copy says what it copied.
 */
export async function markdownFilePick(client: T3Client, native: Native, target: string, picked: string, context: FileLinkMenuContext & { lookup?: string } = {}): Promise<void> {
  if (!target) return;
  const environment = client.environmentId;
  await loadSshAliases(native);
  if (!markdownFileMenuRows(client, target, context).some(row => row.id === picked)) return;
  const editor = preferredEditor(availableEditorIds(client.config), lastEditor(client)) || null;
  const cwd = context.cwd ?? workspaceOf(client).cwd;
  // openMarkdownMedia: the expanded media dialog with the link's media (timeline-attachments.ts).
  if (picked === 'preview-media') {
    try { await openMarkdownMediaPreview(client, native, target, cwd); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Media unavailable', description: mediaUnavailableDescription(client, error), stacked: true }); }
    return;
  }
  const hostPath = () => resolveFileLinkHostPath(client, native, target, context.lookup ?? '', cwd);
  // openMarkdownFileInPreview: the file in a new Browser tab beside the thread (a draft gets its thread id first).
  if (picked === 'open-in-browser') {
    try {
      if (!client.threadId && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native);
      const ref = activeRef(client);
      if (!ref) throw new Error('Environment is not connected.');
      await openFileInPreview(client, native, ref, splitPosition(target).path, cwd);
    } catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Unable to open file in browser', description: error instanceof Error && error.message ? error.message : 'An error occurred.', stacked: true }); }
    return;
  }
  if (picked === 'open' || picked === 'reveal') {
    const failed = (description: string): void => { pushToast(client, { kind: 'error', title: picked === 'open' ? 'Unable to open file' : 'Unable to reveal file', description, stacked: true }); };
    try {
      const host = await hostPath();
      if (picked === 'reveal') { await client.restAccess(native).request('shell.openInEditor', { cwd: host.absolutePath, editor: 'file-manager', reveal: true }); return; }
      // useOpenInPreferredEditor: resolveAndPersistPreferredEditor, then open.
      if (!editor) return failed(`No available editor can open ${target} in environment ${environment}.`);
      rememberEditor(client, editor);
      if (!await openInEditorHere(client, native, fileLinkEditorTarget(target, host), editor)) failed('An error occurred.');
    } catch (error) { if (letGo(error)) throw error; failed(error instanceof Error && error.message ? error.message : 'An error occurred.'); }
    return;
  }
  if (picked !== 'copy-relative' && picked !== 'copy-full') return;
  const title = picked === 'copy-relative' ? 'Relative path' : 'Full path';
  try {
    const paths = fileLinkCopyPaths(target, await hostPath(), cwd), text = picked === 'copy-relative' ? paths.relativePath : paths.fullPath;
    const result = obj(await client.restAccess(native).call({ op: 'copyText', text }));
    if (result.copied === false) throw new Error('Clipboard API unavailable.');
    pushToast(client, { kind: 'success', title: `${title} copied`, description: text });
  } catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: `Failed to copy ${title.toLowerCase()}`, description: error instanceof Error && error.message ? error.message : 'An error occurred.', stacked: true }); }
}
/**
 * "Media unavailable"'s description: the error's own message, but media missing on another machine names that
 * machine and the path its server checked (ChatMarkdown openMarkdownMedia, T3 Code 74541bab17).
 */
export function mediaUnavailableDescription(client: T3Client, error: unknown): string {
  const description = error instanceof Error && error.message ? error.message : 'The file could not be loaded. It may have been moved or deleted.';
  const context = error instanceof ClientError ? error.context : {};
  if (!(error instanceof ClientError) || error.kind !== 'AssetWorkspaceAssetNotFoundError' || context.resourceTag !== 'media-file' || !client.environmentId) return description;
  const remote = remoteOpenFor(client);
  return remote.state.mode === 'local-exec' ? description : `The file was not found on ${remote.connectionLabel} at ${context.checkedPath || context.resourcePath}.`;
}

/**
 * What the preview's Open in hands the editor. A file opens as a file in a remote editor (useProjectFileQuery
 * `isFile`: its contents read, or it is binary). Finder reveals a file instead of opening it in its default app once
 * its read has settled, since a chat-linked folder reads as a file until then (FilePreviewPanel revealsInFileManager).
 */
function openTarget(client: T3Client, state: FilesState): { isFile: boolean; reveal: boolean } {
  const active = activeSurface(client), place = active ? placeOf(client, state, active) : null;
  const read = place?.path ? state.reads.get(place.path) : undefined;
  if (!place || !read || place.folder) return { isFile: false, reveal: false };
  return { isFile: !read.notFile && (!read.error || read.binary === true), reveal: client.config.shellRevealInFileManager === true };
}
function allExpanded(state: FilesState): boolean {
  const folders = expandableFolders(state);
  return folders.length > 0 && folders.every(entry => state.expanded.has(entry.path));
}
async function editorFor(client: T3Client, native: Native) {
  const raw: unknown[] = Array.isArray(client.config.availableEditors) ? client.config.availableEditors : [];
  const available = raw.filter((value): value is string => typeof value === 'string' && EDITORS.some(([id]) => id === value));
  const openIn = await openInView(client, native, available, workspaceOf(client).projectName);
  const editorId = preferredEditor(openIn.editors, '');
  const label = (id: string) => EDITORS.find(([candidate]) => candidate === id)?.[1] ?? id;
  return { editorId, editorLabel: editorId ? `Open in ${label(editorId)}` : 'Open in editor', editorShow: openIn.show, editorHint: openIn.hint, editorUnavailable: openIn.unavailable,
    editors: openIn.unavailable ? [] : openIn.editors.map(id => ({ id, label: label(id), selected: id === editorId })) };
}

/** The tree as visible rows: expanded folders' children, single-folder chains flattened (flattenEmptyDirectories). */
export function treeRows(dirs: ReadonlyMap<string, Entry[]>, expanded: ReadonlySet<string>, selected: string, loadingDirs: ReadonlySet<string> = new Set()): TreeRow[] {
  const rows: TreeRow[] = [];
  const visit = (folder: string, depth: number) => {
    for (const entry of dirs.get(folder) ?? []) {
      let path = entry.path, name = baseName(entry.path);
      if (entry.kind === 'directory') {
        // A loaded folder whose only child is a folder shows as one "a/b" row.
        for (let children = dirs.get(path); children && children.length === 1 && children[0]!.kind === 'directory'; children = dirs.get(path)) {
          path = children[0]!.path; name = `${name}/${baseName(path)}`;
        }
      }
      const directory = entry.kind === 'directory', open = directory && expanded.has(path);
      // The selected path is revealed and selected, a folder too (FileBrowserPanel's reveal); an open folder still
      // loading its children shows a spinner in its row (renderRowDecoration).
      rows.push({ id: path, path, name, depth, directory, expanded: open, selected: path === selected, token: directory ? '' : fileIconToken(path), ignored: entry.ignored, loading: open && loadingDirs.has(path),
        guides: Array.from({ length: depth }, (_, level) => ({ id: String(level), left: Math.round((13.6 + level * 18.7) * 10) / 10 })) });
      if (open) visit(path, depth + 1);
    }
  };
  visit('', 0);
  return rows;
}
/** The tree's own search over the server's fuzzy results: rows whose name (or, for a path query, path) holds the text. */
export function searchMatches(entries: Entry[], query: string): Entry[] {
  const needle = query.trim().toLowerCase();
  return entries.filter(entry => (needle.includes('/') ? entry.path : baseName(entry.path)).toLowerCase().includes(needle));
}
/** fileTreeSearchMode "hide-non-matches": the matches and their folders, all open. */
export function searchRows(entries: Entry[], selected: string): TreeRow[] {
  const all = new Map<string, Entry>();
  for (const entry of entries) {
    all.set(entry.path, entry);
    const segments = entry.path.split('/');
    for (let index = 1; index < segments.length; index++) {
      const folder = segments.slice(0, index).join('/');
      if (!all.has(folder)) all.set(folder, { path: folder, kind: 'directory', ignored: false });
    }
  }
  const dirs = new Map<string, Entry[]>();
  for (const entry of all.values()) { const parent = parentOf(entry.path); dirs.set(parent, [...(dirs.get(parent) ?? []), entry]); }
  for (const [key, list] of dirs) dirs.set(key, sortEntries(list));
  const expanded = new Set([...all.values()].filter(entry => entry.kind === 'directory').map(entry => entry.path));
  return treeRows(dirs, expanded, selected).map(row => row.directory ? { ...row, expanded: true } : row);
}
export const closedCrumbs = (): CrumbMenu => ({ open: false, root: '', back: '', backPath: '', dir: '', status: '', statusRetry: false, retry: '', entries: [] });
/** The open folder menu: its folder's entries (folders first), Back while inside a subfolder, under the crumb's chevron. */
function crumbMenu(state: FilesState, project: string, current: string, trail: string): CrumbMenu {
  const menu = state.crumb;
  if (!menu) return closedCrumbs();
  const list = crumbs(project, current, trail), index = list.findIndex(crumb => crumb.menu && (crumb.path === '.' ? '' : crumb.path) === menu.root);
  if (index < 0) return closedCrumbs();
  const parent = menu.dir ? menu.dir.slice(0, Math.max(0, menu.dir.lastIndexOf('/'))) : null;
  const canBack = menu.dir !== menu.root && parent !== null && (menu.root === '' || parent === menu.root || parent.startsWith(`${menu.root}/`));
  const entries = state.dirs.get(menu.dir), failed = state.errors.has(menu.dir);
  return { open: true, root: list[index]!.id, back: canBack ? `Back to ${parent ? baseName(parent) : project}` : '', backPath: canBack ? parent! : '', dir: menu.dir,
    status: entries === undefined ? (failed ? 'Retry loading folder' : 'Loading folder…') : entries.length ? '' : 'This folder is empty.',
    statusRetry: entries === undefined && failed, retry: entries !== undefined && failed ? 'Refresh failed. Retry' : '',
    entries: (entries ?? []).map(entry => ({ id: entry.path, path: entry.path, label: baseName(entry.path), directory: entry.kind === 'directory', token: entry.kind === 'directory' ? '' : fileIconToken(entry.path), current: entry.kind === 'file' && entry.path === current, ignored: entry.ignored })) };
}
/**
 * FileBreadcrumbs over `trail` (the current path, or a deeper one the browser came up from): the project, each folder
 * and the file. Each folder crumb moves the browser there and the chevron after it lists the folder's entries; a host
 * path outside the workspace has no contents menu, and a host crumb the server cannot list (a UNC server) is a label.
 * The workspace root's crumb is ".".
 */
export function crumbs(project: string, path: string, trail = path): Crumb[] {
  const host = isHost(path), list = fileBreadcrumbs(project || 'Workspace', trail);
  const currentIndex = list.findIndex(crumb => crumb.path === breadcrumbPathOf(path));
  return list.map((crumb, index) => ({
    id: crumb.path || '.', label: crumb.label, path: crumb.path || '.', current: index === currentIndex,
    navigable: index !== currentIndex && !(host && !isListableHostFolder(crumb.path)), descendant: index > currentIndex,
    menu: !host && index < list.length - 1, last: index === list.length - 1,
  }));
}
function runsKey(tokens: readonly { text: string; cls: string }[] | undefined, line: string): string {
  let hash = 0;
  const text = (tokens ?? []).map(token => `${token.cls}\u0001${token.text}`).join('\u0002') || line;
  for (let at = 0; at < text.length; at++) hash = (hash * 31 + text.charCodeAt(at)) | 0;
  return `${text.length}:${(hash >>> 0).toString(36)}`;
}
/** Pierre's File: numbered lines with Shiki-class runs (the diff panel's highlighter). */
export function codeLines(path: string, contents: string): CodeLine[] {
  const lines = contents.split(/\r\n|\r|\n/);
  const tokens = lineTokens(lines, path);
  // A line is keyed by its runs too, so a line whose text or colours change (another file, a
  // retokenized one) remounts: a text flow repainted in place kept spans of the file shown before it.
  return lines.map((line, index) => ({ id: `${index + 1}:${runsKey(tokens[index], line)}`, number: String(index + 1),
    runs: (tokens[index]?.length ? tokens[index]! : line ? [{ text: line, cls: '' as const }] : []).map((token, at) => ({ id: String(at), text: token.text, syntax: token.cls })) }));
}

/** FilePreviewPanel handleOpenInBrowser: the file in a new Browser tab; a failure is a stacked toast. */
async function openInPreviewBrowser(client: T3Client, native: Native, path: string): Promise<void> {
  const { cwd } = workspaceOf(client);
  if (!path || !cwd) return;
  const absolute = isHost(path) ? path : `${cwd.replace(/\/+$/, '')}/${path}`;
  try {
    // The reference's draft has its thread id from the start; here it is allocated now (as the Browser surface does).
    if (!client.threadId && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native);
    const ref = activeRef(client);
    if (!ref) return;
    await openFileInPreview(client, native, ref, absolute, cwd);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to open file in browser', description: error instanceof Error && error.message ? error.message : 'An error occurred.', stacked: true });
  }
}

/**
 * FileMarkdownPreview: ChatMarkdown over the file's text, relative links and images resolved against the file's own
 * folder (`imageBaseDir`), ```mermaid fences drawn as diagrams; its task checkboxes write the file unless it is
 * read-only (RenderedMarkdownSurface readOnly). Its media sign through
 * the thread, or a draft's own id (FilePreviewPanel's `threadRef` is the draft's), and their references are relative
 * to the workspace (`cwd`), not the folder.
 */
/**
 * ChatView's workspaceMutationId: the newest settled file change or command (a command can change files without naming
 * them) and the newest checkpoint's capture time, or null before either. A rendered Markdown file's images sign again
 * when it changes.
 */
export function workspaceMutationId(client: T3Client): string | null {
  const projection = obj(client.projection), items = arr(projection.visibleTurnItems).map(row => obj(row.item));
  const settled = items.findLast(item => (item.type === 'command_execution' || item.type === 'file_change') && !['pending', 'running', 'waiting'].includes(str(item.status)));
  const checkpoint = arr(projection.checkpoints).findLast(entry => entry.appRunOrdinal != null && entry.runId != null);
  const itemId = settled ? str(settled.id) : null, capturedAt = checkpoint ? str(checkpoint.capturedAt) || null : null;
  return itemId === null && capturedAt === null ? null : JSON.stringify([itemId, capturedAt]);
}
async function renderedMarkdown(client: T3Client, native: Native, path: string, absolute: string, source: string, editable: boolean, now: number) {
  // A leading YAML frontmatter block is blanked, not drawn as a rule and a heading; the task offsets still name the file.
  const text = maskMarkdownFrontmatter(source);
  const folder = absolute.slice(0, Math.max(0, absolute.lastIndexOf('/'))) || '/', scope = { cwd: workspaceOf(client).cwd };
  const copy = timelineView(client).codeCopy, copied = copy.threadId === client.threadId;
  const signs = markdownMediaChips(text, folder, scope).some(chip => chip.icon === 'environment');
  const threadId = client.threadId || (signs && client.draftKey && client.ready ? await ensureDraftThreadId(client, native).catch(error => { if (letGo(error)) throw error; return ''; }) : '');
  const urls = await markdownMediaUrls(client, native, folder, now, [text], { ...scope, threadId, mutationId: workspaceMutationId(client) });
  const diagrams = filesMermaid(client, text);
  return {
    markdownSource: [{ id: `file:${path}`, kind: editable ? 'file' : 'assistant', title: '', body: text }],
    md: { ...markdownEnv(client), chips: withFileLinkMenus(client, messageChips({ text }, folder, [], `files:${absolute}`, scope, client)) }, // the owner names the document
    code: messageCodeBlocks(text), diagrams, mdUrls: urls.map(entry => ({ ...entry, fill: '', hover: '', border: '', ink: '' })),
    codeCopied: copied ? copy.text : '', codeCopyNonce: copied ? copy.nonce : 0,
  };
}
/** How many lines filesView numbers for `path` while its preview is the source text (`preview == "code"`), else 0. */
function codePreviewLines(client: T3Client, path: string): number {
  const state = states.get(client), { cwd } = workspaceOf(client);
  if (!state || state.key !== `${client.environmentId}|${cwd}` || !path || state.editing === path) return 0;
  const read = state.reads.get(path), preferences = prefsOf(client), local = !isHost(path);
  if (!read || read.error || (read.notFile && local) || isWorkspaceVideoPreviewPath(path) || isWorkspaceImagePreviewPath(path) || isWorkspaceAudioPreviewPath(path) || isPdfPath(path)) return 0;
  if (renderedShown(client, state, path, activeSurface(client))) return 0;
  return read.contents.split(/\r\n|\r|\n/).length;
}
/**
 * A Files preview comment draft on show with the focus holds ⌘↩, as the Diff's does (diff.ts
 * draftHoldsCommandEnter; composer-presentation.ts gives the composer no send chords then). The host sends no blur when a
 * focused view leaves the tree (LLP 1008), so it holds only on the panel it was taken on, while that panel shows the
 * draft's file as source with the draft's last line (fileCommentLines draws the card there).
 */
export function fileDraftHoldsCommandEnter(client: T3Client): boolean {
  const draft = focusedFileDraft(client);
  if (!draft || draft.key !== panelKey(client)) return false;
  const panel = panelState(client), active = panel.surfaces.find(entry => entry.id === panel.active);
  return panel.visible && active?.kind === 'file' && active.path === draft.path && draft.endLine <= codePreviewLines(client, draft.path);
}

export const emptyFiles = (): FilesView => ({
  cwd: '', project: '', ready: false, loading: false, searching: false, error: '', query: '', truncated: false, noMatches: false, emptySearch: false, loadingQuiet: false, rows: [], hasDirectories: false, allExpanded: false, treeMenu: [],
  explorer: true, showExplorer: true, subheader: false, explorerToggle: false, path: '', preview: '', previewError: '', errorPath: '', errorCwd: '', crumbs: [], lines: [], commentOpen: false, text: '', textKey: '', gutter: 0, wrap: true, rawText: false, openInBrowser: false,
  truncatedNote: '', canRender: false, rendered: false, renderLabel: '', renderIcon: '', editable: false, pending: false, editorId: '', editorLabel: '', editorShow: false, editorHint: '', editorUnavailable: '', editors: [], absolutePath: '',
  markdownSource: [], md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [], threadGlyphs: [], runCommands: [] }, code: [], diagrams: [], mdUrls: [], codeCopied: '', codeCopyNonce: 0, editing: false, editorText: '', editPress: '', crumbMenu: closedCrumbs(), crumbsMask: 'none', crumbsOffset: -1, url: '', media: NO_MEDIA,
  mermaidNote: '', editorRevision: 0, fileKey: '', attachment: emptyAttachment(), viewer: emptyViewerFrame(), fullFile: closedFullFile(),
});

/** FileBrowserPanel's `useState("")`: each file tab mounts its own explorer, so its search starts empty and ends with it. */
function clearSearch(state: FilesState): void {
  state.query = ''; state.search = null; state.searching = false;
  for (const tree of state.hosts.values()) tree.query = '';
}
/** The Files surface is not shown (another tab, a hidden panel): its explorer unmounts, and the search with it. */
export function filesHidden(client: T3Client): void {
  const state = states.get(client);
  if (state && state.queryOwner) { clearSearch(state); state.queryOwner = ''; }
  // Only the active surface is mounted: the attachment body goes with it, and mounts afresh when Files shows again.
  if (state?.attachment) { forgetPreview(client, state.attachment.source.key); state.attachment = null; }
}
export async function filesView(client: T3Client, native: Native, active: Surface, now = 0): Promise<FilesView> {
  const state = filesState(client), preferences = prefsOf(client), { cwd, projectName } = workspaceOf(client);
  const owner = `${panelKey(client)}|${active.id}`;
  // A search typed before this explorer's first view (no owner yet) is its own; another tab's is not.
  if (state.queryOwner !== owner) { if (state.queryOwner) clearSearch(state); state.queryOwner = owner; }
  if (!state.dirs.has('') && !state.errors.has('')) await ensureTree(client, native);
  // A command or a file change settled since the last view: what Files shows is read again.
  const mutation = workspaceMutationId(client), changed = state.mutation !== undefined && mutation !== state.mutation;
  state.mutation = mutation;
  if (changed) await readAfterMutation(client, native, active);
  const shownPlace = placeOf(client, state, active), path = shownPlace.path;
  if (crumbsMounting(client, client.threadId)) settleMounted(state, path);
  const cold = settleCrumbs(state, path); // where the crumbs settle
  if (path && !state.reads.has(path)) await ensureFile(client, native, path, true);
  else if (path && revealStale(state, path, now)) await ensureFile(client, native, path, true); // a reveal whose answer was abandoned asks again
  else if (path && readStale(client, state, path, mutation, shownPlace.folder)) await readFile(client, native, path);
  noteReveal(client, !!path && missingFolders(state, path).length > 0);
  const read = path ? state.reads.get(path) : undefined;
  const { folder, hostFolder, crumbPath, trail } = placeOf(client, state, active);
  const previewPath = folder ? '' : path;
  // A host folder's own tree: its root, then the entry that leads toward the trail, revealed and expanded once.
  const tree = hostFolder ? hostTree(state, hostFolder) : null, selected = hostFolder ? trailChildOf(hostFolder, trail ?? hostFolder) ?? '' : path;
  if (tree && !tree.dirs.has('') && !tree.errors.has('')) await loadDirectory(client, native, '', false, hostFolder);
  if (tree && selected && tree.revealed !== trail) { tree.revealed = trail ?? ''; tree.expanded.add(selected); await loadDirectory(client, native, selected, false, hostFolder); }
  const markdown = isMarkdownPath(previewPath), mermaid = isMermaidPreviewFile(previewPath), table = isTablePath(previewPath), html = isHtmlPath(previewPath);
  const rendered = renderedShown(client, state, previewPath, active);
  // FilePreviewPanel renderBrowserFile: a PDF always shows as its document (BrowserDocumentFrame's PdfViewer, in the
  // viewer page), a page when rendered; one outside the workspace is signed as a `media-file` (workspaceAssetResource).
  const pdf = isPdfPath(previewPath), frame = (html && rendered) || pdf;
  // The explorer the crumbs walked up to opens the workspace folder in an editor.
  const absolute = path ? (isHost(path) ? path : `${cwd.replace(/\/+$/, '')}/${path}`) : crumbPath === '' ? cwd : '';
  // A rendered page waits for its signed URL, not for the read. Its URL carries the workspace revision, so a change
  // reloads it (WorkspaceBrowserPreview's revisionSuffix and documentKey).
  const revision = mutation ?? '';
  const minted = frame ? await htmlPage(client, native, cwd, absolute, revision, now) : { url: '', error: '' };
  const source = frame && minted.url ? (pdf ? { src: minted.url, error: '' } : await mediaFrameSource('html', minted.url)) : null;
  const page = { url: source?.src ?? '', error: minted.error || source?.error || '' };
  // An image or video renders from its signed URL, never from the read (FilePreviewPanel isImage / isVideo). An image's
  // URL carries the workspace revision, so a change loads it again (WorkspaceImagePreview's revisionSuffix); a video is
  // signed again after a change and loads again only for a new revision of its file, when the server reports one
  // (WorkspaceVideoPreview's fileRevision, media-views.ts), since the workspace's would reload it after every command.
  const media = previewPath ? await filesMediaView(client, native, previewPath, absolute, cwd, revision, now) : NO_MEDIA;
  // WorkspaceAudioPreview: an audio file plays from its `media-file` URL; a refused URL or the player's error is a failure.
  const audio = !!previewPath && !media.kind && !isWorkspaceVideoPreviewPath(previewPath) && isWorkspaceAudioPreviewPath(previewPath);
  const sound = audio ? await audioPage(client, native, cwd, absolute, revision, now) : null;
  const soundFailed = !!sound && (!!sound.error || (!!sound.url && state.audioFailed.has(sound.url)));
  // A document (an Office file, an archive, a database, a font), or a file read as binary, is AttachmentFilePreview's.
  const document = !!previewPath && !media.kind && !audio && !frame && (isWorkspaceDocumentPath(previewPath) || read?.binary === true);
  const attachment = document ? await filesAttachment(client, native, state, previewPath, absolute, isWorkspaceDocumentPath(previewPath), mutation, now) : null;
  if (!document && state.attachment) { forgetPreview(client, state.attachment.source.key); state.attachment = null; } // its body unmounts
  const text = read?.contents ?? '';
  // WorkspaceLargeTextPreview: text past 1 MB streams whole as "Full file", but in a draft and for the kinds whose
  // rendering reads the first megabyte (Markdown, Mermaid, a page), which keep the 1 MB notice.
  const draft = !client.threadId;
  const fullFile = !!previewPath && !media.kind && !audio && !frame && !document && !!read && !read.error && read.truncated && !draft && !markdown && !mermaid && !html;
  const tableShown = !!previewPath && table && rendered && !fullFile && !!read && !read.error;
  const viewing = (pdf && !!page.url) || fullFile || tableShown;
  const lines = previewPath && !document && !viewing && read && !read.error ? codeLines(previewPath, text) : [];
  const editable = !!previewPath && !media.kind && !audio && !pdf && !document && !!read && !read.error && !read.truncated && !isHost(previewPath);
  const rows = tree ? hostRows(tree, hostFolder, selected)
    : state.query.trim() && state.search ? searchRows(searchMatches(state.search.entries, state.query), path) : treeRows(state.dirs, state.expanded, path, state.loadingDirs);
  const shown = tree ?? state;
  const reachable = new Set(['', ...[...shown.dirs.values()].flat().filter(entry => entry.kind === 'directory').map(entry => entry.path)]);
  const error = [...shown.errors].find(([folderPath]) => reachable.has(folderPath))?.[1] ?? (tree ? '' : state.search?.error ?? '');
  const editors = await editorFor(client, native);
  // shouldShowFileExplorer: never beside a host file; a folder is the explorer alone.
  const showExplorer = !isHost(previewPath) && (preferences.explorer || !previewPath);
  // FileBrowserPanel hasNoSearchMatches: a workspace search counts once the server answered it; a host folder's filters what is loaded.
  const query = tree ? tree.query : state.query, searching = !tree && state.searching, ready = shown.dirs.has('');
  const noMatches = !!query.trim() && rows.length === 0 && (!!tree || !!state.search);
  const viewer = viewing ? await filesViewer(client, native, {
    kind: pdf ? 'pdf' : fullFile ? 'full' : 'table', path: previewPath, absolute, name: previewPath.split(/[\\/]/).pop() || previewPath, revision, url: page.url, text,
    delimiter: table && rendered ? (/\.tsv$/i.test(previewPath) ? '\t' : ',') : null, revealLine: active.line > 0 ? active.line : null, revealRequestId: active.reveal,
  }, now) : null;
  const preview = !previewPath ? '' : media.kind ? 'media' : audio ? (soundFailed ? 'audio-error' : sound?.url ? 'audio' : 'loading')
    : frame ? (page.error ? 'error' : page.url ? (pdf ? 'viewer' : 'html') : 'loading') : viewer ? (viewer.error ? (fullFile ? 'full-error' : 'error') : 'viewer') : document ? 'attachment' : read === undefined ? 'loading' : read.error ? 'error'
      : rendered ? (markdown ? 'markdown' : 'mermaid') : 'code';
  if (preview !== 'viewer') filesViewerHidden(client);
  const rich = preview === 'markdown' ? await renderedMarkdown(client, native, previewPath, absolute, text, editable, now) : null;
  const diagram = preview === 'mermaid' ? mermaidFile(client, text) : null;
  const readError = !frame && !media.kind && !audio && !document && !!read?.error;
  return {
    cwd, project: hostFolder || projectName, ready, loading: state.loading > 0, searching, error, query,
    truncated: !tree && !!state.query.trim() && !!state.search?.truncated, rows,
    noMatches, emptySearch: noMatches && ready && !searching && !error, loadingQuiet: rows.length > 0 && !noMatches,
    // A host folder can be a home directory, so nothing walks it whole: no expand-all.
    hasDirectories: !tree && [...state.dirs.values()].flat().some(entry => entry.kind === 'directory'),
    allExpanded: state.expandAll || allExpanded(state), explorer: preferences.explorer, showExplorer, treeMenu: filesTreeMenuRows(client),
    subheader: crumbPath !== null, explorerToggle: !!previewPath && !isHost(previewPath),
    path, preview,
    previewError: frame ? page.error : viewer?.error ? viewer.error : read?.error && !folder && !document ? read.error : '',
    errorPath: readError ? read!.attempted : '', errorCwd: readError && !isHost(path) ? read!.errorCwd : '',
    crumbs: crumbPath !== null && trail !== null ? crumbs(projectName, crumbPath, trail) : [], lines: fileCommentLines(client, previewPath, text, lines, editable && state.editing === path), commentOpen: fileCommentOpen(client), text, textKey: `${path}:${active.reveal}`,
    gutter: sourceGutter(lines.length), wrap: client.local.clientSettings.wordWrap !== false,
    // FilePreviewPanel: word wrap reaches only the text bodies (`showsRawText`), a full file's source among them; a page or
    // a PDF opens in the Browser (`canOpenInBrowser`).
    rawText: preview === 'code' || (fullFile && !(table && rendered)), openInBrowser: !!previewPath && client.available === true && media.kind !== 'video' && isBrowserPreviewFile(previewPath),
    truncatedNote: previewPath && !frame && !document && !media.kind && read?.truncated && (draft || markdown || mermaid || html) ? `Preview limited to the first 1 MB of a ${read.byteLength.toLocaleString('en-US')} byte file.` : '',
    canRender: markdown || mermaid || table || html, rendered,
    renderLabel: mermaid ? (rendered ? 'Show Mermaid source' : 'Show rendered diagram') : markdown ? (rendered ? 'Show markdown source' : 'Show rendered markdown') : table ? (rendered ? 'Show source' : 'Show table') : html ? htmlToggleLabel(rendered) : '',
    renderIcon: rendered ? 'code' : table ? 'table' : 'eye', editable, pending: pendingPaths(client).has(path),
    ...editors, absolutePath: absolute,
    // A notebook shown under "File" parses its markdown cells in the same answer.
    ...(rich ?? { markdownSource: attachment?.markdownSource ?? [], md: attachment?.notebook.open ? attachment.md : emptyFiles().md, code: attachment?.notebook.open ? attachment.code : [], diagrams: diagram?.diagrams ?? (attachment?.notebook.open ? attachment.diagrams : []), mdUrls: [], codeCopied: '', codeCopyNonce: 0 }),
    editing: editable && state.editing === path, editorText: state.editing === path ? state.editorText : '', editPress: editPresses.get(client) ?? '', editorRevision: state.editorRevision,
    crumbMenu: crumbPath !== null && trail !== null ? crumbMenu(state, projectName, crumbPath, trail) : closedCrumbs(), crumbsMask: crumbPath !== null ? crumbsMask(client.presentation) : 'none',
    crumbsOffset: path !== '' && previewPath !== '' && !document && !viewing && !!read && !read.error && !rendered && !pdf ? crumbsOffset(client.presentation ?? {}, cold, true) : -1,
    url: frame ? page.url : sound?.url ?? '', media, mermaidNote: diagram?.note ?? '', fileKey: previewPath ? `${panelKey(client)}:${absolute}` : '',
    attachment: attachment ?? emptyAttachment(),
    viewer: preview === 'viewer' && viewer ? viewer.viewer : emptyViewerFrame(), fullFile: preview === 'viewer' && viewer ? viewer.full : closedFullFile(),
  };
}

/**
 * After a workspace change, the tree's listed folders and the search read again (FileBrowserPanel's
 * useWorkspaceMutationRefresh); the file shown reads again through readStale.
 */
async function readAfterMutation(client: T3Client, native: Native, active: Surface): Promise<void> {
  const state = filesState(client), place = placeOf(client, state, active), host = place.hostFolder;
  const visited = [...(host ? hostTree(state, host) : state).requested];
  for (let index = 0; index < visited.length; index += 4) await Promise.all(visited.slice(index, index + 4).map(path => loadDirectory(client, native, path, true, host)));
  if (!host && state.query.trim()) await search(client, native, state.query);
}

/**
 * Whether the path shown was read before the latest workspace change (revisionedQuery's `stale`), so it reads again: a
 * file in any tab, one open in the editor included, and a folder, so it notices if the path becomes a file. A file with
 * a save pending keeps its text (`selectedFilePending`); media, PDFs and documents never show their text, and a
 * document's body mounts afresh on the change instead.
 */
function readStale(client: T3Client, state: FilesState, path: string, mutation: string | null, folder: boolean): boolean {
  if (!state.readRevisions.has(path) || state.readRevisions.get(path) === mutation || pendingPaths(client).has(path)) return false;
  return folder || (!isWorkspaceImagePreviewPath(path) && !isWorkspaceVideoPreviewPath(path) && !isWorkspaceAudioPreviewPath(path) && !isPdfPath(path) && !isWorkspaceDocumentPath(path));
}

/**
 * FilePreviewPanel's AttachmentFilePreview for a document, or a file read as binary (origin "File"): signed as a
 * `media-file`, a binary one as an attachment, and a draft's document as its `draft-workspace-file`. Keyed on the
 * workspace revision and the explorer's Refresh, as the reference's is, so either mounts it afresh and the one before goes.
 */
async function filesAttachment(client: T3Client, native: Native, state: FilesState, path: string, absolute: string, isDocument: boolean, mutation: string | null, now: number): Promise<AttachmentView> {
  const meta: AttachmentMeta = { id: absolute, name: path.split(/[\\/]/).pop() || path, mimeType: 'application/octet-stream', sizeBytes: 0 };
  const draft = !client.threadId;
  // A draft signs a binary file under its own thread id (threadRef.threadId), which the server needs non-empty.
  if (draft && !isDocument && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native).catch(error => { if (letGo(error)) throw error; });
  const resource = draft && isDocument ? { _tag: 'draft-workspace-file', cwd: workspaceOf(client).cwd, path: absolute }
    : { _tag: 'media-file', threadId: client.threadId || activeRef(client)?.threadId || '', path: absolute, ...(isDocument ? {} : { disposition: 'attachment' }) };
  const key = `files:${panelKey(client)}:${absolute}:${mutation ?? ''}:${state.refreshRevision}`;
  if (state.attachment && state.attachment.source.key !== key) forgetPreview(client, state.attachment.source.key);
  const source: PreviewSource = { id: `file:${path}`, key, origin: 'File', dialog: false, local: null, resource };
  state.attachment = { meta, source };
  return attachmentView(client, native, meta, now, source);
}

/** A host folder's rows: its tree, or while searching the loaded entries that match, each named by its host path. */
function hostRows(tree: Tree, folder: string, selected: string): TreeRow[] {
  const loaded = [...tree.dirs.values()].flat();
  const rows = tree.query.trim() ? searchRows(searchMatches(loaded, tree.query), selected) : treeRows(tree.dirs, tree.expanded, selected, tree.loadingDirs);
  return rows.map(row => ({ ...row, id: joinHostPath(folder, row.path), path: joinHostPath(folder, row.path) }));
}
