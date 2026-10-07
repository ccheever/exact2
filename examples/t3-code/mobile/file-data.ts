// Pinned365aa87982 ThreadFilesRouteScreen, FileTreeBrowser and SourceFileSurface.
// @ref llp/1107.006-review-and-files.decision.md#ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { ensureFile, ensureTree, filesLocal, filesState } from './shared/r4-surfaces-files';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { fileIconToken } from './shared/timeline-files';
import { assertReviewOwner, reviewFileAccess, reviewNative, reviewOwner } from './review-owner';
import { buildFileTree, flattenFileTree, type ProjectEntry } from './file-tree-model';
import { reviewLineTokens, type ReviewRow, reviewRow } from './review-model';

export interface FileTreeItem { id: string; path: string; name: string; directory: boolean; expanded: boolean; selected: boolean;
  ignored: boolean; depth: number; loaded: boolean; count: string; token: string }
export interface FileTreeSnapshot { owner: string; revision: number; title: string; query: string; selectedPath: string;
  rows: FileTreeItem[]; loading: boolean; error: string; truncated: boolean; emptyTitle: string; emptyDetail: string }
export interface FileSnapshot { owner: string; revision: number; path: string; title: string; subtitle: string;
  loading: boolean; error: string; contents: string; rows: ReviewRow[]; truncated: boolean; notice: string;
  markdown: boolean; initialRowId: string }
type ReadLane = 'tree' | 'file';
interface Access { owner: string; allowed: boolean; permissionError: string; revocation: number;
  tree: { checking: boolean; error: string; serial: number }; file: { checking: boolean; error: string; serial: number } }
const accesses = new WeakMap<T3Client, Access>();
function accessOf(client: T3Client): Access {
  const owner = reviewOwner(client); let access = accesses.get(client);
  if (!access || access.owner !== owner) {
    access = { owner, allowed: false, permissionError: '', revocation: 0,
      tree: { checking: false, error: '', serial: 0 }, file: { checking: false, error: '', serial: 0 } };
    accesses.set(client, access);
  }
  return access;
}
const message = (error: unknown) => error instanceof Error ? error.message : 'Files unavailable';
/** Tree and source panes may read concurrently. Newer requests supersede only
 * their own pane; an actual permission failure revokes both panes and all older
 * in-flight authorizations. Shared directory/file caches and RPC stay unchanged. */
async function allow(client: T3Client, nativeInput: Native | null | undefined, owner: string, lane: ReadLane) {
  assertReviewOwner(client, owner);
  const access = accessOf(client), request = access[lane], serial = ++request.serial, revocation = access.revocation;
  request.checking = true; request.error = '';
  const check = () => {
    assertReviewOwner(client, owner);
    if (serial !== request.serial || revocation !== access.revocation)
      throw new ClientError('A newer files request or permission change replaced this one.', 'superseded');
  };
  try {
    if (!nativeInput?.available || !client.ready || !workspaceOf(client).cwd)
      throw new ClientError('Connect and select a workspace to view its files.');
    const guarded = reviewNative(client, letGoAware(mobileNative(nativeInput)), owner);
    const native = { ...guarded, assertCurrent: check, later: async (input: Parameters<Native['later']>[0]) => {
      check(); const result = await guarded.later(input); check(); return result;
    } };
    const allowed = await reviewFileAccess(client, native);
    check();
    if (!allowed) throw new ClientError('This connection cannot read host files.');
    access.allowed = true; access.permissionError = '';
    return native;
  } catch (error) {
    if (serial === request.serial && accessOf(client) === access && !letGo(error)) {
      access.allowed = false; access.permissionError = message(error); access.revocation++;
    }
    throw error;
  } finally { if (serial === request.serial) request.checking = false; }
}

/** Merge only directories reachable from the actual root; search adds its own results. */
export function mobileFileTreeProjection(client: T3Client, selectedPath: string): FileTreeItem[] {
  const state = filesState(client), entries = new Map<string, ProjectEntry>(), seen = new Set<string>();
  if (state.query.trim()) for (const entry of state.search?.entries ?? []) entries.set(entry.path, entry);
  const visit = (path: string) => {
    if (seen.has(path)) return; seen.add(path);
    for (const entry of state.dirs.get(path) ?? []) {
      entries.set(entry.path, entry);
      if (entry.kind === 'directory') visit(entry.path);
    }
  };
  visit('');
  const expanded = new Set(state.expanded);
  const parts = selectedPath.split('/'); for (let index = 1; index < parts.length; index++) expanded.add(parts.slice(0, index).join('/'));
  return flattenFileTree({ nodes: buildFileTree([...entries.values()]), expanded, searchQuery: state.query }).map(({ node, depth }) => ({
    id: node.path, path: node.path, name: node.name, directory: node.kind === 'directory', expanded: expanded.has(node.path), selected: node.path === selectedPath,
    ignored: node.ignored === true, depth, loaded: state.dirs.has(node.path), count: String(node.children.length), token: fileIconToken(node.path) }));
}
export function mobileFilesSnapshot(selectedPath = '', client: T3Client = mobileClient): FileTreeSnapshot {
  const access = accessOf(client), state = filesState(client), query = state.query;
  const error = access.permissionError || access.tree.error || (access.allowed ? state.errors.get('') || (query.trim() ? state.search?.error : '') || [...state.errors.values()][0] || '' : '');
  return { owner: access.owner, revision: client.revision, title: workspaceOf(client).projectName || 'Files', query, selectedPath,
    rows: access.allowed ? mobileFileTreeProjection(client, selectedPath) : [], loading: access.tree.checking || state.loading > 0,
    error, truncated: access.allowed && !!query.trim() && state.search?.truncated === true,
    emptyTitle: error ? 'Files unavailable' : 'No files found', emptyDetail: error || (query.trim() ? 'Try a different search.' : 'The workspace is empty.') };
}
/** Search input is debounced 200 ms by the root. The shared request clamps to256 and limit200. */
export async function mobileFilesRead(query: string, selectedPath: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const access = accessOf(client), owner = access.owner;
  try {
    const native = await allow(client, nativeInput, owner, 'tree');
    await ensureTree(client, native);
    native.assertCurrent();
    // A file deep link must reveal and load its ancestry in the persistent tree.
    const parts = selectedPath.split('/').filter(Boolean);
    for (let index = 1; index < parts.length; index++) {
      const directory = parts.slice(0, index).join('/');
      if (!filesState(client).expanded.has(directory)) await filesLocal(client, native, 'toggle', directory, '');
      native.assertCurrent();
    }
    if (query !== filesState(client).query || query.trim() && !filesState(client).search) await filesLocal(client, native, 'search', '', query);
    native.assertCurrent();
  } catch (error) { if (letGo(error)) throw error; if (accessOf(client) === access) access.tree.error = message(error); }
  finally { client.revision++; }
  return mobileFilesSnapshot(selectedPath, client);
}
export async function mobileFilesAction(owner: string, op: string, path: string, selectedPath: string, nativeInput: Native | null | undefined,
  client: T3Client = mobileClient) {
  let error = '';
  try {
    assertReviewOwner(client, owner);
    const native = await allow(client, nativeInput, owner, 'tree'), state = filesState(client);
    if (op === 'toggle') {
      if (!mobileFileTreeProjection(client, selectedPath).some(row => row.path === path && row.directory)) throw new ClientError('That directory is no longer available.');
      await filesLocal(client, native, 'toggle', path, '');
    } else if (op === 'refresh') await filesLocal(client, native, 'refresh', '', '');
    else if (op === 'search') await filesLocal(client, native, 'search', '', path);
    else if (op === 'reveal') {
      const parts = path.split('/');
      for (let index = 1; index < parts.length; index++) {
        const directory = parts.slice(0, index).join('/');
        if (!state.expanded.has(directory)) await filesLocal(client, native, 'toggle', directory, '');
      }
    } else throw new ClientError('Unsupported files action.');
    native.assertCurrent();
  } catch (cause) { if (letGo(cause)) throw cause; error = message(cause); if (reviewOwner(client) === owner) accessOf(client).tree.error = error; }
  finally { client.revision++; }
  return { message: error, data: mobileFilesSnapshot(selectedPath, client) };
}

export function mobileFileSnapshot(path: string, dark = false, initialLine = 0, client: T3Client = mobileClient): FileSnapshot {
  const access = accessOf(client), state = filesState(client), read = access.allowed ? state.reads.get(path) : undefined;
  const contents = (read?.contents ?? '').replace(/\r\n?/g, '\n'), lines = contents.split('\n');
  const target = initialLine > 0 ? Math.min(Math.floor(initialLine) - 1, Math.max(0, lines.length - 1)) : -1;
  const tokens = read && !read.error ? reviewLineTokens(lines.map((content, at) => ({ change: 'context', content, oldLineNumber: null, newLineNumber: at + 1 })), path, dark) : [];
  const parent = path.slice(0, Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'), 0));
  const absolute = path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path) || path.startsWith('\\\\');
  return { owner: access.owner, revision: client.revision, path, title: path.split(/[\\/]/).at(-1) || 'File',
    subtitle: absolute ? parent : [workspaceOf(client).projectName, parent].filter(Boolean).join(' · '),
    loading: access.file.checking || state.loading > 0, error: access.permissionError || access.file.error || read?.error || '', contents,
    rows: read && !read.error ? lines.map((text, at) => ({ ...reviewRow(`source-line:${at}`, 'line'), path, text: text.replace(/\t/g, '    '),
      oldNumber: '', newNumber: String(at + 1), lineIndex: at, change: 'context', selected: at === target, tokens: tokens[at] ?? [] })) : [],
    truncated: read?.truncated === true, notice: read?.truncated ? 'Preview limited to the first 1 MB of a truncated file.' : '',
    markdown: /\.(md|mdx)$/i.test(path), initialRowId: target >= 0 ? `source-line:${target}` : '' };
}
export async function mobileFileRead(path: string, nativeInput: Native | null | undefined, dark = false, initialLine = 0, refresh = false,
  client: T3Client = mobileClient) {
  const access = accessOf(client), owner = access.owner;
  try {
    if (!path || path.includes('\0')) throw new ClientError('Choose a file to view.');
    const native = await allow(client, nativeInput, owner, 'file');
    // ensureFile performs projects.readFile through shared client, and marks directory results.
    await ensureFile(client, native, path, refresh);
    native.assertCurrent();
  } catch (error) { if (letGo(error)) throw error; if (accessOf(client) === access) access.file.error = message(error); }
  finally { client.revision++; }
  return mobileFileSnapshot(path, dark, initialLine, client);
}
