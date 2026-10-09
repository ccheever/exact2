// The Files surface (lane r4-surfaces; MIT reference, see LICENSE-T3:
// components/files/FilePreviewPanel.tsx, FileBrowserPanel.tsx, useDirectoryEntries.ts,
// FileBreadcrumbs.tsx, fileSaveCoordinator.ts, filePreviewMode.ts and @pierre/trees'
// compact tree): the workspace tree over `projects.listEntries` (one folder at a time,
// collapsed folders keep their children), path search over `projects.searchEntries`,
// the preview over `projects.readFile` with syntax colour, and edits written back with
// `projects.writeFile` (latest contents win; a save in flight is followed by the newest).
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { pushToast } from './toast';
import { fileIconToken } from './timeline-files';
import { lineTokens } from './timeline-diff-syntax';
import { EDITORS, lastEditor, preferredEditor, rememberEditor } from './shell-details';
import { workspaceOf, panelKey, panelState, type Surface, type PanelState } from './r4-surfaces-panel';
import { tableRows } from './r4-surfaces-render';
import { markdownEnv, messageChips, type MarkdownEnv } from './r4-timeline-chips'; // markdown-links-and-files-preview: the chat renderer's chips and settings
import { messageCodeBlocks } from './timeline-highlight';
import { timelineView } from './timeline-presentation';
import { textWidth } from './pages-text-width';
import { filesPrefs, type FilesPrefs } from './r5-panels-prefs';
import { anchorFrame } from './r6-polish-measure';
import { crumbsMask, crumbsShift } from './r7-polish-crumbs'; // lane r7-polish: the trail scrolled to its end
import { crumbsOffset, noteFirstRead, settleCrumbs, settleMounted, sourceGutter } from './r9-device-crumbs'; // lane r9-device: where the trail settles
import { contentRevision, htmlPage, htmlToggleLabel, isBrowserPreviewFile, isHtmlPath, isPdfPath } from './r10-device-files-html'; // lane r10-device: rendered HTML
import { openFileInPreview } from './browser-links'; // right-panel-launcher-and-files: "Open file in preview browser"
import { activeRef } from './terminal-drawer-view';
import { ensureDraftThreadId } from './r7-handoff-thread';
import { crumbsMounting, loadBegin, loadEnd, missingFolders, noteReveal, revealStale } from './r10-device-crumbs'; // lane r10-device: a mounting preview settles at the end
import { canUseMarkdownFileShellActions, loadSshAliases, openInEditorHere, openInView, remoteOpenFor } from './remote-open'; // remote Open (OpenInPicker)
import { fileComment, fileCommentLines, fileCommentOpen, focusedFileDraft, type FileLine } from './diff-file-comments'; // diff-review: line comments on the preview
import { filesMediaView, markdownMediaUrls, NO_MEDIA, type MediaView } from './media-views'; // media-actions: image and video files with their menu
import { letGo } from './let-go';
import { filesTreeMenu, showContextMenu } from './context-menu-actions'; // context-menu-gaps
import { availableEditorIds, markdownFileMenuItems, revealLabelFor } from './context-menus';
import { isWorkspaceImagePreviewPath, isWorkspaceVideoPreviewPath, mediaMimeTypeFromExtension } from './media-source';
import { openMarkdownMediaPreview } from './timeline-attachments';

export type TreeRow = { id: string; path: string; name: string; depth: number; directory: boolean; expanded: boolean; selected: boolean; token: string; ignored: boolean; guides: { id: string; left: number }[] };
export type Crumb = { id: string; label: string; path: string; current: boolean; directory: boolean };
export type CodeRun = { id: string; text: string; syntax: string };
export type CodeLine = { id: string; number: string; runs: CodeRun[] };
export type EditorChoice = { id: string; label: string; selected: boolean };
export type CrumbEntry = { id: string; path: string; label: string; directory: boolean; token: string; current: boolean; ignored: boolean };
export type CrumbMenu = { open: boolean; root: string; x: number; back: string; backPath: string; status: string; entries: CrumbEntry[] };
export type FilesView = {
  cwd: string; project: string; ready: boolean; loading: boolean; error: string; query: string; truncated: boolean;
  rows: TreeRow[]; hasDirectories: boolean; allExpanded: boolean; explorer: boolean; showExplorer: boolean;
  path: string; preview: string; previewError: string; crumbs: Crumb[]; lines: FileLine[]; commentOpen: boolean; text: string; textKey: string;
  gutter: number; wrap: boolean; rawText: boolean; openInBrowser: boolean; truncatedNote: string; canRender: boolean; rendered: boolean; renderLabel: string; renderIcon: string;
  editable: boolean; pending: boolean; editorId: string; editorLabel: string; editorShow: boolean; editorHint: string; editorUnavailable: string; editors: EditorChoice[]; absolutePath: string;
  // markdown-links-and-files-preview PA-3: FileMarkdownPreview is ChatMarkdown. `markdownSource` is renderMarkdown's one
  // message (app.contract filesMarkdown; kind "file" when its task checkboxes may write the file), `md` its chips and
  // settings, `code` its fences' colours, `mdUrls` its host-path media and `codeCopied` / `codeCopyNonce` Copy code's check.
  markdownSource: MarkdownSource[]; md: MarkdownEnv; code: ReturnType<typeof messageCodeBlocks>; diagrams: never[]; mdUrls: { id: string; url: string; fill: string; hover: string; border: string; ink: string }[];
  codeCopied: string; codeCopyNonce: number; table: { id: string; header: boolean; cells: CodeRun[] }[]; editing: boolean; editorText: string; editorsOpen: boolean; crumbMenu: CrumbMenu; crumbsMask: string; crumbsOffset: number;
  url: string; media: MediaView;
};
export type MarkdownSource = { id: string; kind: string; title: string; body: string };
type Entry = { path: string; kind: 'file' | 'directory'; ignored: boolean };
type Read = { contents: string; byteLength: number; truncated: boolean; error: string; notFile: boolean };
type Edit = { contents: string; revision: number; confirmed: number; saving: boolean; error: string };
type FilesState = {
  key: string; dirs: Map<string, Entry[]>; errors: Map<string, string>; requested: Set<string>; expanded: Set<string>; expandAll: boolean;
  query: string; search: { query: string; entries: Entry[]; truncated: boolean; error: string } | null;
  reads: Map<string, Read>; edits: Map<string, Edit>; loading: number; editing: string; editorText: string; editorsOpen: boolean; crumb: { root: string; dir: string } | null;
};
// Preferences the reference keeps in localStorage (t3code.fileExplorerOpen, t3code.renderMarkdown, t3code.renderTable),
// persisted in the client's preference file (lane r5-panels: r5-panels-prefs.ts).
const states = new WeakMap<T3Client, FilesState>();
export const PATH_SEARCH_LIMIT = 200;
const EXPAND_ALL_CAP = 200;

const prefsOf = (client: T3Client): FilesPrefs => filesPrefs(client);
export function filesState(client: T3Client): FilesState {
  const { cwd } = workspaceOf(client), key = `${client.environmentId}|${cwd}`;
  let state = states.get(client);
  if (!state || state.key !== key) {
    state = { key, dirs: new Map(), errors: new Map(), requested: new Set(), expanded: new Set(), expandAll: false, query: '', search: null, reads: new Map(), edits: new Map(), loading: 0, editing: '', editorText: '', editorsOpen: false, crumb: null };
    states.set(client, state);
  }
  return state;
}
const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf('/')));
const baseName = (path: string) => path.slice(path.lastIndexOf('/') + 1);
const message = (error: unknown) => error instanceof Error && error.message ? error.message : 'Unable to load folder.';
export const isMarkdownPath = (path: string) => /\.(?:md|mdx)$/i.test(path);
export const isTablePath = (path: string) => /\.(?:csv|tsv)$/i.test(path);
const isAbsolute = (path: string) => path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path);
/** Pierre's order: folders before files, then a case-insensitive natural name order. */
export function sortEntries(entries: Entry[]): Entry[] {
  return [...entries].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === 'directory' ? -1 : 1)
    || baseName(a.path).localeCompare(baseName(b.path), undefined, { sensitivity: 'base', numeric: true }));
}
const toEntry = (value: Obj): Entry | null => {
  const path = str(value.path).replace(/\/+$/, '');
  return path ? { path, kind: value.kind === 'directory' ? 'directory' : 'file', ignored: value.ignored === true } : null;
};

/** useDirectoryEntries.load: one folder's immediate children, kept when it collapses. */
async function loadDirectory(client: T3Client, native: Native, path: string, refresh = false): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  if (!cwd || (!refresh && state.dirs.has(path))) return;
  state.requested.add(path); loadBegin(state, `dir:${path}`); // lane r10-device: counted by what is asked
  try {
    const result = await client.restAccess(native).request('projects.listEntries', { cwd, directoryPath: path });
    if (filesState(client) !== state) return;
    const entries = arr(result.entries).map(toEntry).filter((entry): entry is Entry => !!entry && parentOf(entry.path) === path);
    state.dirs.set(path, sortEntries(entries)); state.errors.delete(path);
  } catch (error) {
    if (filesState(client) === state && !letGo(error)) state.errors.set(path, message(error));
  } finally { loadEnd(state, `dir:${path}`); }
}
export async function ensureTree(client: T3Client, native: Native): Promise<void> {
  await loadDirectory(client, native, '');
}
/** projectFilesQueryState: the file's contents (a folder answers "not a file", which keeps the tree). */
async function readFile(client: T3Client, native: Native, path: string): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  if (!cwd) return;
  if (!state.reads.has(path)) noteFirstRead(state, path); // lane r9-device: an uncached read settles the crumbs short
  loadBegin(state, `read:${path}`);
  try {
    const result = await client.restAccess(native).request('projects.readFile', { cwd, relativePath: path });
    if (filesState(client) !== state) return;
    state.reads.set(path, { contents: str(result.contents), byteLength: num(result.byteLength), truncated: result.truncated === true, error: '', notFile: false });
  } catch (error) {
    if (filesState(client) !== state || letGo(error)) return;
    const text = message(error);
    state.reads.set(path, { contents: '', byteLength: 0, truncated: false, error: text, notFile: /not a file|path_not_file|is a directory/i.test(text) });
  } finally { loadEnd(state, `read:${path}`); }
}
/** Opening a file reads it and reveals it: each ancestor folder loads and expands (FileBrowserPanel's reveal). */
export async function ensureFile(client: T3Client, native: Native, path: string, reveal: boolean, fromTree = false): Promise<void> {
  const state = filesState(client);
  state.crumb = null;
  const tasks: Promise<void>[] = [];
  if ((!state.reads.has(path) || reveal) && state.editing !== path) tasks.push(readFile(client, native, path));
  if (state.editing && state.editing !== path) state.editing = '';
  if (!isAbsolute(path)) {
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
  // A path that turned out to be a folder is revealed and expanded in the tree instead.
  if (reveal && state.reads.get(path)?.notFile) { state.expanded.add(path); await loadDirectory(client, native, path); }
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
  if (!trimmed || !cwd) { state.search = null; return; }
  loadBegin(state, 'search');
  try {
    const result = await client.restAccess(native).request('projects.searchEntries', { cwd, query: trimmed, limit: PATH_SEARCH_LIMIT });
    if (filesState(client) !== state || state.query !== query) return;
    state.search = { query: trimmed, entries: arr(result.entries).map(toEntry).filter((entry): entry is Entry => !!entry), truncated: result.truncated === true, error: '' };
  } catch (error) {
    if (filesState(client) === state && state.query === query && !letGo(error)) state.search = { query: trimmed, entries: [], truncated: false, error: message(error) };
  } finally { loadEnd(state, 'search'); }
}

/** setAllDirectoriesExpanded, re-applied as folders load (the reference's expandAll effect), up to a bound. */
async function expandAll(client: T3Client, native: Native): Promise<void> {
  const state = filesState(client);
  for (let round = 0; round < 32; round++) {
    const folders = [...state.dirs.values()].flat().filter(entry => entry.kind === 'directory').map(entry => entry.path);
    const fresh = folders.filter(path => !state.dirs.has(path)).slice(0, EXPAND_ALL_CAP - state.dirs.size);
    for (const path of folders) state.expanded.add(path);
    if (!fresh.length || state.dirs.size >= EXPAND_ALL_CAP) return;
    for (let index = 0; index < fresh.length; index += 4) await Promise.all(fresh.slice(index, index + 4).map(path => loadDirectory(client, native, path)));
  }
}

/** The editor's change: the latest contents are written; a save in flight is followed by the newest (fileSaveCoordinator). */
export async function editFile(client: T3Client, native: Native, path: string, contents: string): Promise<void> {
  const state = filesState(client), { cwd } = workspaceOf(client);
  if (!cwd || isAbsolute(path)) return;
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
      await client.restAccess(native).request('projects.writeFile', { cwd, relativePath: path, contents: text }, true);
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
  if (active?.kind !== 'file' || !isMarkdownPath(active.path) || isAbsolute(active.path)) return;
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

/** `shelllocal:surface-files-*`. */
export async function filesLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op === 'link-menu') { await markdownFileMenu(client, native, id); return ''; }
  if (op === 'row-menu') { await filesTreeMenu(client, native, id); return ''; } // context-menu-actions.ts
  const state = filesState(client), preferences = prefsOf(client);
  if (op === 'toggle') {
    if (state.expanded.has(id)) { state.expanded.delete(id); state.expandAll = false; }
    else { state.expanded.add(id); await loadDirectory(client, native, id); }
    return '';
  }
  if (op === 'search') { await search(client, native, value); return ''; }
  if (op.startsWith('comment-')) return fileComment(client, native, op.slice(8), id, value, state.reads.get(id)?.contents ?? '', panelKey(client)); // diff-file-comments.ts
  if (op === 'search-key') { if (value === 'Escape') await search(client, native, ''); return ''; }
  if (op === 'begin-edit') {
    const read = state.reads.get(id);
    if (read && !read.error && !read.truncated && !isAbsolute(id)) { state.editing = id; state.editorText = read.contents; }
    return '';
  }
  if (op === 'end-edit') { if (state.editing === id) state.editing = ''; return ''; }
  if (op === 'refresh') {
    const visited = [...state.requested];
    for (let index = 0; index < visited.length; index += 4) await Promise.all(visited.slice(index, index + 4).map(path => loadDirectory(client, native, path, true)));
    if (state.query.trim()) await search(client, native, state.query);
    const active = panelState(client).surfaces.find(entry => entry.id === panelState(client).active);
    if (active?.kind === 'file' && !state.edits.get(active.path)?.saving) await readFile(client, native, active.path);
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
    if (isMarkdownPath(id)) preferences.renderMarkdown = !preferences.renderMarkdown;
    else if (isHtmlPath(id)) preferences.renderBrowserFile = preferences.renderBrowserFile === false;
    else preferences.renderTable = !preferences.renderTable;
    return '';
  }
  if (op === 'open-browser') { await openInPreviewBrowser(client, native, id); return ''; }
  if (op === 'wrap') { client.local.clientSettings.wordWrap = client.local.clientSettings.wordWrap === false; return ''; }
  if (op === 'edit') { await editFile(client, native, id, value); return ''; }
  if (op === 'task') { await toggleTask(client, native, value); return ''; }
  if (op === 'editors') { state.editorsOpen = !state.editorsOpen; return ''; }
  // DirectoryBreadcrumb: a folder crumb opens its listing; folders navigate inside it, Back climbs toward the crumb.
  if (op === 'crumb') { const root = id === '.' ? '' : id; state.crumb = state.crumb?.root === root ? null : { root, dir: root }; if (state.crumb) await loadDirectory(client, native, root); return ''; }
  if (op === 'crumb-dir') { if (state.crumb) { state.crumb.dir = id; await loadDirectory(client, native, id); } return ''; }
  if (op === 'crumb-close') { state.crumb = null; return ''; }
  if (op === 'open-editor') {
    state.editorsOpen = false;
    const choices = await editorFor(client, native);
    if (!choices.editorShow) return '';
    if (choices.editorUnavailable) { pushToast(client, { kind: 'error', title: 'Unable to open in editor', description: choices.editorUnavailable }); return ''; }
    const editor = value || choices.editorId;
    if (!editor || !choices.editors.some(choice => choice.id === editor)) { pushToast(client, { kind: 'error', title: 'Unable to open in editor', description: `No available editor can open ${id}.` }); return ''; }
    if (await openInEditorHere(client, native, id, editor)) rememberEditor(client, editor); // remote Open as the details card does (remote-open.ts)
    return '';
  }
  return '';
}

/**
 * ChatMarkdown's file-link menu (context-menus.ts markdownFileMenuItems): "Preview media" for a
 * media file, the open item named for the preferred editor and the server-worded reveal only for
 * a resolved local environment, then the copies.
 */
export async function markdownFileMenu(client: T3Client, native: Native, target: string): Promise<void> {
  if (!target) return;
  const environment = client.environmentId, origin = client.origin;
  await loadSshAliases(native);
  const remote = remoteOpenFor(client);
  const canOpen = canUseMarkdownFileShellActions(environment || null, remote.state.mode, remote.resolved);
  const available = availableEditorIds(client.config);
  const editor = preferredEditor(available, lastEditor(client)) || null;
  const filePath = target.replace(/:\d+(?::\d+)?$/, ''), name = baseName(filePath), dot = name.lastIndexOf('.');
  const canPreviewMedia = !!client.threadId && dot >= 0 && mediaMimeTypeFromExtension(name.slice(dot)) !== null;
  const items = markdownFileMenuItems({ canPreviewMedia, canOpen, preferredEditor: editor, revealLabel: revealLabelFor(client.config, environment || null) });
  const picked = await showContextMenu(client, native, items);
  if (!picked || client.environmentId !== environment || client.origin !== origin) return;
  // openMarkdownMedia: the expanded media dialog with the link's media (timeline-attachments.ts).
  if (picked === 'preview-media') {
    try { await openMarkdownMediaPreview(client, native, target, workspaceOf(client).cwd); }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Media unavailable', description: error instanceof Error ? error.message : 'The file could not be loaded. It may have been moved or deleted.', stacked: true }); }
    return;
  }
  // Recheck after the native menu closes; a menu from another route cannot execute there.
  const current = remoteOpenFor(client);
  if (picked === 'open' || picked === 'reveal') {
    if (!canUseMarkdownFileShellActions(environment || null, current.state.mode, current.resolved)) return;
    const failed = (description: string): void => { pushToast(client, { kind: 'error', title: picked === 'open' ? 'Unable to open file' : 'Unable to reveal file', description, stacked: true }); };
    try {
      if (picked === 'reveal') { await client.restAccess(native).request('shell.openInEditor', { cwd: filePath, editor: 'file-manager', reveal: true }); return; }
      // useOpenInPreferredEditor: resolveAndPersistPreferredEditor, then open.
      if (!editor) return failed(`No available editor can open ${target} in environment ${environment}.`);
      rememberEditor(client, editor);
      if (!await openInEditorHere(client, native, target, editor)) failed('An error occurred.');
    } catch (error) { if (letGo(error)) throw error; failed(error instanceof Error && error.message ? error.message : 'An error occurred.'); }
    return;
  }
  const root = workspaceOf(client).cwd.replace(/\/+$/, '');
  const text = picked === 'copy-relative' && root && target.startsWith(`${root}/`) ? target.slice(root.length + 1) : target;
  await client.restAccess(native).call({ op: 'copyText', text });
}

function allExpanded(state: FilesState): boolean {
  const folders = [...state.dirs.values()].flat().filter(entry => entry.kind === 'directory');
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
export function treeRows(dirs: ReadonlyMap<string, Entry[]>, expanded: ReadonlySet<string>, selected: string): TreeRow[] {
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
      rows.push({ id: path, path, name, depth, directory, expanded: open, selected: !directory && path === selected, token: directory ? '' : fileIconToken(path), ignored: entry.ignored,
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
export const closedCrumbs = (): CrumbMenu => ({ open: false, root: '', x: 0, back: '', backPath: '', status: '', entries: [] });
/** The open crumb's menu: its folder's entries (folders first), Back while inside a subfolder, placed under the crumb. */
function crumbMenu(state: FilesState, project: string, current: string, presentation: Obj = {}): CrumbMenu {
  const menu = state.crumb;
  if (!menu || !current) return closedCrumbs();
  const trail = crumbs(project, current), index = trail.findIndex(crumb => (crumb.path === '.' ? '' : crumb.path) === menu.root);
  if (index < 0) return closedCrumbs();
  // 12pt subheader inset; a crumb after the first starts with its 14pt chevron and 4pt margins. r6-polish: the
  // crumb's laid-out place (hooked `crumb:<path>`, r6-polish-measure.ts); the advance table only before it reports.
  const placed = anchorFrame(presentation, `crumb:${trail[index]!.id}`);
  const x = placed ? 12 + crumbsShift(presentation) + placed[0] + (index > 0 ? 22 : 0) : 12 + trail.slice(0, index).reduce((sum, crumb) => sum + textWidth(crumb.label, 12) + 4 + 22, 0);
  const parent = menu.dir ? menu.dir.slice(0, Math.max(0, menu.dir.lastIndexOf('/'))) : null;
  const canBack = menu.dir !== menu.root && parent !== null && (menu.root === '' || parent === menu.root || parent.startsWith(`${menu.root}/`));
  const entries = state.dirs.get(menu.dir);
  return { open: true, root: trail[index]!.id, x: Math.round(x * 10) / 10, back: canBack ? `Back to ${parent ? baseName(parent) : project}` : '', backPath: canBack ? parent! : '',
    status: entries === undefined ? (state.errors.has(menu.dir) ? 'Retry loading folder' : 'Loading folder…') : entries.length ? '' : 'This folder is empty.',
    entries: (entries ?? []).map(entry => ({ id: entry.path, path: entry.path, label: baseName(entry.path), directory: entry.kind === 'directory', token: entry.kind === 'directory' ? '' : fileIconToken(entry.path), current: entry.path === current, ignored: entry.ignored })) };
}
/** FileBreadcrumbs: the project, each folder and the current file. */
export function crumbs(project: string, path: string): Crumb[] {
  const segments = path.split('/').filter(Boolean);
  return [{ id: '.', label: project || 'Workspace', path: '.', current: false, directory: true },
    ...segments.map((segment, index) => ({ id: segments.slice(0, index + 1).join('/'), label: segment, path: segments.slice(0, index + 1).join('/'), current: index === segments.length - 1, directory: index < segments.length - 1 }))];
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
  // lane r12-render: a line is keyed by its runs too, so a line whose text or colours change (another file, a
  // retokenized one) remounts: a text flow repainted in place kept spans of the file shown before it.
  return lines.map((line, index) => ({ id: `${index + 1}:${runsKey(tokens[index], line)}`, number: String(index + 1),
    runs: (tokens[index]?.length ? tokens[index]! : line ? [{ text: line, cls: '' as const }] : []).map((token, at) => ({ id: String(at), text: token.text, syntax: token.cls })) }));
}

/** FilePreviewPanel handleOpenInBrowser: the file in a new Browser tab; a failure is a stacked toast. */
async function openInPreviewBrowser(client: T3Client, native: Native, path: string): Promise<void> {
  const { cwd } = workspaceOf(client);
  if (!path || !cwd) return;
  const absolute = isAbsolute(path) ? path : `${cwd.replace(/\/+$/, '')}/${path}`;
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
 * folder (`imageBaseDir`); its task checkboxes write the file unless it is read-only (RenderedMarkdownSurface readOnly).
 */
async function renderedMarkdown(client: T3Client, native: Native, path: string, absolute: string, text: string, editable: boolean, now: number) {
  const folder = absolute.slice(0, Math.max(0, absolute.lastIndexOf('/'))) || '/';
  const copy = timelineView(client).codeCopy, copied = copy.threadId === client.threadId;
  const urls = await markdownMediaUrls(client, native, folder, now, [text]);
  return {
    markdownSource: [{ id: `file:${path}`, kind: editable ? 'file' : 'assistant', title: '', body: text }],
    md: { ...markdownEnv(client), chips: messageChips({ text }, folder, []) },
    code: messageCodeBlocks(text), mdUrls: urls.map(entry => ({ ...entry, fill: '', hover: '', border: '', ink: '' })),
    codeCopied: copied ? copy.text : '', codeCopyNonce: copied ? copy.nonce : 0,
  };
}
/** How many lines filesView numbers for `path` while its preview is the source text (`preview == "code"`), else 0. */
function codePreviewLines(client: T3Client, path: string): number {
  const state = states.get(client), { cwd } = workspaceOf(client);
  if (!state || state.key !== `${client.environmentId}|${cwd}` || !path || state.editing === path) return 0;
  const read = state.reads.get(path), preferences = prefsOf(client), local = !isAbsolute(path);
  if (!read || read.error || (read.notFile && local) || isWorkspaceVideoPreviewPath(path) || isWorkspaceImagePreviewPath(path) || (local && isPdfPath(path))) return 0;
  if ((isMarkdownPath(path) && preferences.renderMarkdown) || (isTablePath(path) && preferences.renderTable) || (local && isHtmlPath(path) && preferences.renderBrowserFile !== false)) return 0;
  return read.contents.split(/\r\n|\r|\n/).length;
}
/**
 * audit-wave-followups FU-3: a Files preview comment draft on show with the focus holds ⌘↩, as the Diff's does (diff.ts
 * draftHoldsCommandEnter; composer-presentation.ts gives the Send button no chords then). The host sends no blur when a
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
  cwd: '', project: '', ready: false, loading: false, error: '', query: '', truncated: false, rows: [], hasDirectories: false, allExpanded: false,
  explorer: true, showExplorer: true, path: '', preview: '', previewError: '', crumbs: [], lines: [], commentOpen: false, text: '', textKey: '', gutter: 0, wrap: true, rawText: false, openInBrowser: false,
  truncatedNote: '', canRender: false, rendered: false, renderLabel: '', renderIcon: '', editable: false, pending: false, editorId: '', editorLabel: '', editorShow: false, editorHint: '', editorUnavailable: '', editors: [], absolutePath: '',
  markdownSource: [], md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [], runCommands: [] }, code: [], diagrams: [], mdUrls: [], codeCopied: '', codeCopyNonce: 0, table: [], editing: false, editorText: '', editorsOpen: false, crumbMenu: closedCrumbs(), crumbsMask: 'none', crumbsOffset: -1, url: '', media: NO_MEDIA,
});

export async function filesView(client: T3Client, native: Native, active: Surface, now = 0): Promise<FilesView> {
  const state = filesState(client), preferences = prefsOf(client), { cwd, projectName } = workspaceOf(client);
  if (!state.dirs.has('') && !state.errors.has('')) await ensureTree(client, native);
  const path = active.kind === 'file' ? active.path : '';
  if (crumbsMounting(client, client.threadId)) settleMounted(state, path); // lane r10-device
  const cold = settleCrumbs(state, path); // lane r9-device: where the crumbs settle
  if (path && !state.reads.has(path)) await ensureFile(client, native, path, true);
  else if (path && revealStale(state, path, now)) await ensureFile(client, native, path, true); // lane r10-device: a reveal whose answer was abandoned asks again
  noteReveal(client, !!path && missingFolders(state, path).length > 0);
  const read = path ? state.reads.get(path) : undefined;
  const folder = !!read?.notFile && !isAbsolute(path);
  const previewPath = folder ? '' : path;
  const markdown = isMarkdownPath(previewPath), table = isTablePath(previewPath), html = !isAbsolute(previewPath) && isHtmlPath(previewPath);
  const rendered = (markdown && preferences.renderMarkdown) || (table && preferences.renderTable) || (html && preferences.renderBrowserFile !== false);
  // FilePreviewPanel renderBrowserFile: a PDF always shows as its document (BrowserDocumentFrame; here the module's PDFView, X29), a page when rendered.
  const pdf = !isAbsolute(previewPath) && isPdfPath(previewPath), frame = (html && rendered) || pdf;
  const absolute = path ? (isAbsolute(path) ? path : `${cwd.replace(/\/+$/, '')}/${path}`) : '';
  // lane r10-device: a rendered page waits for its signed URL, not for the read.
  const page = frame ? await htmlPage(client, native, cwd, absolute, previewPath, read && !read.error ? contentRevision(read.contents) : '', now) : { url: '', error: '' };
  // media-actions: an image or video renders from its signed URL, never from the read (FilePreviewPanel isImage / isVideo).
  const media = previewPath ? await filesMediaView(client, native, previewPath, absolute, cwd, '', now) : NO_MEDIA;
  const text = read?.contents ?? '';
  const lines = previewPath && read && !read.error ? codeLines(previewPath, text) : [];
  const editable = !!previewPath && !media.kind && !pdf && !!read && !read.error && !read.truncated && !isAbsolute(previewPath);
  const rows = state.query.trim() && state.search ? searchRows(searchMatches(state.search.entries, state.query), path) : treeRows(state.dirs, state.expanded, path);
  const reachable = new Set(['', ...[...state.dirs.values()].flat().filter(entry => entry.kind === 'directory').map(entry => entry.path)]);
  const error = [...state.errors].find(([folderPath]) => reachable.has(folderPath))?.[1] ?? state.search?.error ?? '';
  const editors = await editorFor(client, native);
  const parsedTable = previewPath && table && rendered && read && !read.error ? tableRows(previewPath, text) : null;
  const showExplorer = !isAbsolute(path) && (preferences.explorer || !previewPath);
  const preview = !previewPath ? '' : media.kind ? 'media' : frame ? (page.error ? 'error' : page.url ? (pdf ? 'pdf' : 'html') : 'loading') : read === undefined ? 'loading' : read.error ? 'error' : rendered ? (markdown ? 'markdown' : 'table') : 'code';
  const rich = preview === 'markdown' ? await renderedMarkdown(client, native, previewPath, absolute, text, editable, now) : null;
  return {
    cwd, project: projectName, ready: state.dirs.has(''), loading: state.loading > 0, error, query: state.query,
    truncated: !!state.query.trim() && !!state.search?.truncated, rows, hasDirectories: [...state.dirs.values()].flat().some(entry => entry.kind === 'directory'),
    allExpanded: state.expandAll || allExpanded(state), explorer: preferences.explorer, showExplorer,
    path, preview,
    previewError: frame ? page.error : read?.error && !folder ? read.error : '', crumbs: path ? crumbs(projectName, path) : [], lines: fileCommentLines(client, previewPath, text, lines, editable && state.editing === path), commentOpen: fileCommentOpen(client), text, textKey: `${path}:${active.reveal}`,
    gutter: sourceGutter(lines.length), wrap: client.local.clientSettings.wordWrap !== false,
    // FilePreviewPanel: word wrap reaches only the text bodies (`showsRawText`); a page or a PDF opens in the Browser (`canOpenInBrowser`).
    rawText: preview === 'code', openInBrowser: !!previewPath && client.available === true && media.kind !== 'video' && isBrowserPreviewFile(previewPath),
    truncatedNote: previewPath && !frame && read?.truncated ? `Preview limited to the first 1 MB of a ${read.byteLength.toLocaleString('en-US')} byte file.`
      : parsedTable?.truncated ? 'Table limited to the first 100 rows and 30 columns. Switch to source for the rest.' : '',
    canRender: markdown || table || html, rendered, renderLabel: markdown ? (rendered ? 'Show markdown source' : 'Show rendered markdown') : table ? (rendered ? 'Show source' : 'Show table') : html ? htmlToggleLabel(rendered) : '',
    renderIcon: rendered ? 'code' : table ? 'table' : 'eye', editable, pending: pendingPaths(client).has(path),
    ...editors, absolutePath: absolute,
    ...(rich ?? { markdownSource: [], md: emptyFiles().md, code: [], mdUrls: [], codeCopied: '', codeCopyNonce: 0 }), diagrams: [], table: parsedTable?.rows ?? [],
    editing: editable && state.editing === path, editorText: state.editing === path ? state.editorText : '', editorsOpen: state.editorsOpen && !!path,
    crumbMenu: crumbMenu(state, projectName, path, client.presentation), crumbsMask: path ? crumbsMask(client.presentation) : 'none',
    crumbsOffset: path !== '' && previewPath !== '' && !!read && !read.error && !rendered && !pdf ? crumbsOffset(client.presentation ?? {}, cold, true) : -1,
    url: frame ? page.url : '', media,
  };
}
