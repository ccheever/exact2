// The file a chat file link names (MIT reference, see LICENSE-T3: T3 Code 74541bab17
// packages/client-runtime/src/workspaceBasenameLookup.ts, apps/web/src/markdownFileHostPath.ts,
// markdown-links.ts resolveMarkdownFileLinkMeta's workspaceLookupPath, ChatMarkdown.tsx findWorkspaceFileMatch and
// openFileInPanel, packages/shared/src/path.ts workspaceRelativeFilePath, client-runtime mediaActions.ts
// formatFilePathForClipboard). A link written from the workspace root can name a file by its bare name, or repeat a
// subfolder project's own folders (`apps/web/src/x.ts` in `/repo/apps/web`); the workspace search finds the file it
// means. Opening it, its editor and Finder actions and its copied paths all use that file.
import type { T3Client } from '../client/client';
import { arr, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { isAbsolutePath, isWindowsAbsolutePath } from '../panel/files-path';

export const WORKSPACE_BASENAME_LOOKUP_LIMIT = 25;
type Candidate = { path: string; kind: 'file' | 'directory' };

// One counter for every caller: they all open the same panel, so the newest click wins whichever lookup it started.
let latestLookup = 0;
export function claimWorkspaceBasenameLookup(): () => boolean {
  const claimed = ++latestLookup;
  return () => claimed === latestLookup;
}

const basenameOf = (path: string) => path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
export function needsWorkspaceBasenameLookup(relativePath: string): boolean {
  const trimmed = relativePath.trim();
  return trimmed !== '.' && trimmed !== '..' && trimmed.length > 0 && !trimmed.includes('/') && !trimmed.includes('\\');
}
/** The one file with this name: an exact match first, else the only case-insensitive one. */
export function pickWorkspaceBasenameMatch(basename: string, entries: readonly Candidate[]): string | null {
  const target = basename.trim();
  if (!target) return null;
  const files = entries.filter(entry => entry.kind === 'file');
  const exact = files.find(entry => basenameOf(entry.path) === target);
  if (exact) return exact.path;
  const folded = files.filter(entry => basenameOf(entry.path).toLowerCase() === target.toLowerCase());
  return folded.length === 1 ? folded[0]!.path : null;
}
/** A repository-relative path with the subfolder project's trailing folders taken off its front, or null. */
export function stripRepeatedWorkspacePrefix(relativePath: string, cwd: string): string | null {
  if (isAbsolutePath(relativePath)) return null;
  const pathSegments = relativePath.split(/[\\/]+/), cwdSegments = cwd.split(/[\\/]+/).filter(Boolean);
  const fold = isWindowsAbsolutePath(cwd) ? (segment: string | undefined) => segment?.toLowerCase() : (segment: string | undefined) => segment;
  for (let count = Math.min(cwdSegments.length, pathSegments.length - 1); count > 0; count--) {
    if (cwdSegments.slice(-count).every((segment, index) => fold(segment) === fold(pathSegments[index]))) return pathSegments.slice(count).join('/') || null;
  }
  return null;
}
export function pickStrippedWorkspaceMatch(strippedPath: string, cwd: string, entries: readonly Candidate[]): string | null {
  const caseInsensitive = isWindowsAbsolutePath(cwd);
  const key = (path: string) => { const normalized = path.replaceAll('\\', '/'); return caseInsensitive ? normalized.toLowerCase() : normalized; };
  return entries.find(entry => entry.kind === 'file' && key(entry.path) === key(strippedPath))?.path ?? null;
}

/** workspaceRelativeFilePath: `path` inside `root` with `/`, "." for the root itself, null outside it. */
export function workspaceRelativeFilePath(path: string, root: string | null | undefined): string | null {
  if (!root) return null;
  const strip = (value: string) => /^\/[A-Za-z]:[\\/]/.test(value) ? value.slice(1) : value;
  const normalizedPath = strip(path.replaceAll('\\', '/')), normalizedRoot = strip(root.replaceAll('\\', '/')).replace(/\/+$/, '');
  const fold = isWindowsAbsolutePath(strip(root)) ? (value: string) => value.toLowerCase() : (value: string) => value;
  if (fold(normalizedPath).replace(/\/+$/, '') === fold(normalizedRoot)) return '.';
  if (!fold(normalizedPath).startsWith(`${fold(normalizedRoot)}/`)) return null;
  const relativePath = normalizedPath.slice(normalizedRoot.length + 1);
  // `/repo/../x` starts with the root but leaves it: a host path, which the server refuses as a workspace one.
  let depth = 0;
  for (const segment of relativePath.split('/')) {
    if (segment === '..') depth -= 1;
    else if (segment.length > 0 && segment !== '.') depth += 1;
    if (depth < 0) return null;
  }
  return relativePath;
}
/** resolveWorkspaceFilePath: a workspace-relative path on the host, in the workspace's own separators. */
export function resolveWorkspaceFilePath(path: string, root: string): string {
  if (isAbsolutePath(path) || path.startsWith('~/') || path.startsWith('~\\')) return path;
  const base = root.replace(/[/\\]+$/, '');
  return isWindowsAbsolutePath(root) ? `${base}\\${path.replaceAll('/', '\\')}` : `${base}/${path}`;
}
/** formatFilePathForClipboard: a path in its host's separators, which can differ from this Mac's. */
export function formatFilePathForClipboard(path: string, hostPath = path): string {
  return isWindowsAbsolutePath(hostPath) ? path.replaceAll('/', '\\') : path;
}

const POSITION = /^(.*?):(\d+)(?::(\d+))?$/;
/** A link target split into its path and the `:line[:column]` after it. */
export function splitPosition(target: string): { path: string; position: string } {
  const match = POSITION.exec(target);
  return match && !/^[A-Za-z]$/.test(match[1]!) ? { path: match[1]!, position: target.slice(match[1]!.length) } : { path: target, position: '' };
}
/**
 * resolveMarkdownFileLinkMeta's `workspaceLookupPath`: what the link wrote, when it was written relative to the
 * workspace root (`base`, the folder links resolve from, is the workspace) and is not absolute; else ''.
 */
export function workspaceLookupPath(href: string, base: string, cwd: string): string {
  if (!cwd || workspaceRelativeFilePath(base, cwd) !== '.') return '';
  let path = href.trim().replace(/^<|>$/g, '').replace(/[?#].*$/, '');
  try { path = decodeURIComponent(path); } catch { /* keep the raw spelling */ }
  path = splitPosition(path).path.trim();
  return !path || /^file:/i.test(path) || isAbsolutePath(path) ? '' : path;
}

/**
 * findWorkspaceFileMatch: the workspace file a bare name or a repeated-prefix path means, or null. The search can be
 * truncated or still scanning, so a stripped match is used only when the path as written does not exist.
 */
export async function findWorkspaceFileMatch(client: T3Client, native: Native, lookupPath: string, cwd: string): Promise<string | null> {
  if (!lookupPath || !cwd || !client.environmentId || !client.scopes?.includes('filesystem:read')) return null;
  const isBasename = needsWorkspaceBasenameLookup(lookupPath), stripped = isBasename ? null : stripRepeatedWorkspacePrefix(lookupPath, cwd);
  if (!isBasename && stripped === null) return null;
  try {
    const access = client.restAccess(native);
    const result = await access.request('projects.searchEntries', { cwd, query: stripped ?? lookupPath, limit: WORKSPACE_BASENAME_LOOKUP_LIMIT, kind: 'file' });
    const entries = arr(result.entries).map(entry => ({ path: str(entry.path), kind: entry.kind === 'directory' ? 'directory' as const : 'file' as const })).filter(entry => entry.path);
    if (stripped === null) return pickWorkspaceBasenameMatch(lookupPath, entries);
    const match = pickStrippedWorkspaceMatch(stripped, cwd, entries);
    if (match === null) return null;
    const metadata = await access.request('filesystem.getMetadata', { paths: [resolveWorkspaceFilePath(lookupPath, cwd)] });
    return Array.isArray(metadata.entries) && metadata.entries[0] === null ? match : null;
  } catch (error) { if (letGo(error)) throw error; return null; }
}

/** The link's file on the host (resolveMarkdownFileHostPath): the lookup's match, else the link's own path. */
export type FileLinkHostPath = { absolutePath: string; matched: boolean };
export async function resolveFileLinkHostPath(client: T3Client, native: Native, target: string, lookupPath: string, cwd: string): Promise<FileLinkHostPath> {
  const match = lookupPath && cwd ? await findWorkspaceFileMatch(client, native, lookupPath, cwd) : null;
  return match && cwd ? { absolutePath: resolveWorkspaceFilePath(match, cwd), matched: true } : { absolutePath: splitPosition(target).path, matched: false };
}
/** markdownFileEditorTarget: the matched file at the link's position, else the link's target. */
export function fileLinkEditorTarget(target: string, host: FileLinkHostPath): string {
  return host.matched ? `${host.absolutePath}${splitPosition(target).position}` : target;
}
/** markdownFileCopyPaths, in the host's separators: the full path, and the path inside the thread's workspace. */
export function fileLinkCopyPaths(target: string, host: FileLinkHostPath, cwd: string): { fullPath: string; relativePath: string } {
  const fullPath = fileLinkEditorTarget(target, host), { path, position } = splitPosition(target);
  const relativePath = `${workspaceRelativeFilePath(host.absolutePath, cwd) ?? path}${position}`;
  return { fullPath: formatFilePathForClipboard(fullPath, target), relativePath: formatFilePathForClipboard(relativePath, target) };
}
