// T3 Code 9c161462c9 (MIT, see LICENSE-T3): state/fileMetadata.ts, useFileMetadata.ts,
// WorkspaceEntryIcon.tsx. The filesystem supplies entry kind; spelling never guesses a folder.
import { composerNow } from '../composer/composer-controls';
import type { T3Client } from '../client/client';
import { arr, obj, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { fileIconToken } from './timeline-files';
import { markdownLinks } from '../timeline/message-chips';
import { markdownLinkHref } from '../media/media-views';
import { letGo } from '../client/let-go';

export type FileMetadata = { kind: 'file' | 'directory' | 'other'; byteLength?: number; mimeType?: string };
type Entry = { value: FileMetadata | null; expires: number; failed?: true };
type Store = { generation: number; environment: string; thread: string; cache: Map<string, Entry>; retained: Map<string, Entry>; inFlight: Set<string>; version: number; composerPaths: string[] };
const stores = new WeakMap<object, Store>();
const MAX_CACHE = 1024, STALE_MS = 60_000;
function store(client: T3Client): Store {
  let value = stores.get(client);
  if (!value || value.generation !== client.generation || value.environment !== client.environmentId) {
    value = { generation: client.generation, environment: client.environmentId, thread: client.draftKey, cache: new Map(), retained: new Map(), inFlight: new Set(), version: 0, composerPaths: [] };
    stores.set(client, value);
  }
  if (value.thread !== client.draftKey) { value.thread = client.draftKey; value.retained.clear(); value.composerPaths = []; }
  return value;
}
/** Relative paths need an explicit workspace; absolute/~/ paths keep their host meaning. */
export function metadataPath(path: string, cwd = ''): string | null {
  let value = path.trim().replace(/^file:\/\//i, '').replace(/#L(\d+)(?:C(\d+))?$/i, ':$1').replace(/:(\d+)(?::\d+)?$/, '').replace(/\\/g, '/');
  try { value = decodeURIComponent(value); } catch { /* A literal percent in a host path is valid. */ }
  if (!value || /^[a-z][a-z0-9+.-]*:/i.test(value) && !/^[a-z]:\//i.test(value)) return null;
  if (!value.startsWith('/') && !value.startsWith('~/') && !/^[a-z]:\//i.test(value)) {
    if (!cwd || !/^(?:\/|~\/|[a-z]:[\\/])/i.test(cwd)) return null;
    value = `${cwd.replace(/\\/g, '/').replace(/\/+$/, '')}/${value}`;
  }
  const prefix = value.startsWith('~/') ? '~/' : value.startsWith('/') ? '/' : value.slice(0, 3);
  const parts: string[] = [];
  for (const part of value.slice(prefix.length).split('/')) {
    if (!part || part === '.') continue;
    if (part === '..') parts.pop(); else parts.push(part);
  }
  const normalized = prefix + parts.join('/');
  return normalized.length <= 512 ? normalized : null;
}
function cwdOf(client: T3Client): string {
  return str(obj(obj(client.projection).thread).worktreePath) || str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);
}
/** Shared by prompt mentions and Markdown file chips; context records carry their original paths. */
export function textFilePaths(text: string): string[] {
  return [...markdownLinks(text).filter(link => !link.image && markdownLinkHref(link.href).startsWith('t3-file:')).map(link => link.href),
    ...[...text.matchAll(/(?:^|\s)@([^\s\[\]<>"'`]+)/g)].map(match => match[1]!)];
}
export function visibleFilePaths(client: T3Client): string[] {
  const cwd = cwdOf(client), paths = [...textFilePaths(str(client.draft)), ...store(client).composerPaths];
  for (const message of arr(obj(client.projection).messages)) {
    paths.push(...textFilePaths(str(message.text)));
    for (const record of arr(obj(message.context).records)) if (record.kind === 'mention') paths.push(str(record.path));
  }
  return [...new Set(paths.map(path => metadataPath(path, cwd)).filter((path): path is string => path !== null))];
}
function known(client: T3Client, path: string, now: number): Entry | undefined {
  const value = store(client), entry = value.retained.get(path) ?? value.cache.get(path);
  if (entry && (value.retained.has(path) || entry.expires > now)) { if (!entry.failed) value.retained.set(path, entry); return entry; }
  return undefined;
}
export function fileMetadataNeeded(client: T3Client, now = composerNow(client)): boolean {
  if (!client.ready || !client.scopes?.some(scope => scope === 'filesystem:read' || scope === 'orchestration:read')) return false;
  const value = store(client);
  return visibleFilePaths(client).some(path => !value.inFlight.has(path) && !known(client, path, now));
}
function decode(value: unknown): FileMetadata | null {
  const entry = obj(value);
  if (!['file', 'directory', 'other'].includes(str(entry.kind))) return null;
  return { kind: entry.kind as FileMetadata['kind'], ...(typeof entry.byteLength === 'number' && Number.isSafeInteger(entry.byteLength) && entry.byteLength >= 0 ? { byteLength: entry.byteLength } : {}), ...(str(entry.mimeType) ? { mimeType: str(entry.mimeType) } : {}) };
}
/** One RPC of at most 64 paths; stale generations or invalidations never seed a waiting chip. */
export async function refreshFileMetadata(client: T3Client, native: Native, now = composerNow(client)): Promise<boolean> {
  if (!fileMetadataNeeded(client, now)) return false;
  const value = store(client), version = value.version;
  const paths = visibleFilePaths(client).filter(path => !value.inFlight.has(path) && !known(client, path, now)).slice(0, 64);
  paths.forEach(path => value.inFlight.add(path));
  try {
    const result = await client.restAccess(native).request('filesystem.getMetadata', { paths });
    if (store(client) !== value || value.version !== version) return true;
    paths.forEach((path, index) => {
      const entry = { value: decode(arrays(result.entries)[index]), expires: now + STALE_MS };
      value.cache.delete(path); value.cache.set(path, entry); value.retained.set(path, entry);
      if (value.cache.size > MAX_CACHE) value.cache.delete(value.cache.keys().next().value!);
    });
  } catch (error) {
    if (letGo(error)) throw error;
    if (store(client) === value && value.version === version) for (const path of paths) value.cache.set(path, { value: null, expires: now + STALE_MS, failed: true });
  } finally { paths.forEach(path => value.inFlight.delete(path)); }
  return true;
}
const arrays = (value: unknown): unknown[] => Array.isArray(value) ? value : [];
/** A successful write invalidates both the shared and retained metadata before another query can read it. */
export function invalidateFileMetadata(client: T3Client, path: string, cwd = cwdOf(client)): void {
  const normalized = metadataPath(path, cwd);
  if (!normalized) return;
  const value = store(client); value.version++; value.cache.delete(normalized); value.retained.delete(normalized);
}
const MIME_EXT: Record<string, string> = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/gif': 'gif', 'image/webp': 'webp', 'application/pdf': 'pdf', 'application/zip': 'zip', 'application/vnd.sqlite3': 'sqlite', 'text/x-python': 'py', 'text/javascript': 'js', 'text/x-ruby': 'rb', 'text/x-shellscript': 'sh' };
export function workspaceFileIcon(client: T3Client, path: string, cwd = cwdOf(client), now = composerNow(client)): string {
  const absolute = metadataPath(path, cwd), entry = client.ready && absolute ? known(client, absolute, now)?.value : null;
  if (entry?.kind === 'directory') return 'folder';
  const token = fileIconToken(path);
  return token === 'default' && entry?.mimeType && MIME_EXT[entry.mimeType] ? fileIconToken(`file.${MIME_EXT[entry.mimeType]}`) : token;
}
export function composerFileIcons(client: T3Client, paths: string[]): Record<string, string> {
  return Object.fromEntries(paths.map(path => { const icon = workspaceFileIcon(client, path); return [path, icon === 'folder' ? 'folder' : `pierre:${icon}`]; }));
}

export function noteComposerFilePaths(client: T3Client, paths: string[]): void { store(client).composerPaths = [...new Set(paths)]; }
