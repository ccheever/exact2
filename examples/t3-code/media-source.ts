// media-actions: what a piece of authored media is and how its bytes are reached (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3): packages/client-runtime/src/mediaSource.ts with the parts of
// markdownImages.ts (classifyMarkdownImageSource, markdownImageSourceFragment), markdownLinks.ts
// (normalizeMarkdownLinkDestination, splitMarkdownLinkSearchAndHash, parseFileUrlHref,
// splitFilePathPosition, fileBasename) and packages/shared/src/filePreview.ts (mediaMimeType,
// mediaMimeTypeFromExtension, the workspace preview paths) it reads.
import { videoMimeType } from './r4-composer-video';
import { isWindowsAbsolutePath, mediaFileReference, mediaReferenceFileName, mediaUrlReference, safeDecodeURIComponent, type MediaReference } from './media-reference';

const IMAGE_MIME_TYPE_BY_EXTENSION = new Map([['.avif', 'image/avif'], ['.gif', 'image/gif'], ['.ico', 'image/x-icon'], ['.jpeg', 'image/jpeg'], ['.jpg', 'image/jpeg'], ['.png', 'image/png'], ['.svg', 'image/svg+xml'], ['.webp', 'image/webp']]);
export const WORKSPACE_IMAGE_PREVIEW_EXTENSIONS = ['.avif', '.gif', '.ico', '.jpeg', '.jpg', '.png', '.svg', '.webp'] as const;

/** Classifies a literal filesystem extension, without URL decoding or suffix removal. */
export function mediaMimeTypeFromExtension(extension: string): string | null {
  if (!/^\.[a-z0-9]+$/i.test(extension)) return null;
  return IMAGE_MIME_TYPE_BY_EXTENSION.get(extension.toLowerCase()) ?? videoMimeType({ name: `media${extension}`, mimeType: '' });
}

/** Classifies an authored media path or URL. Filesystem validation uses the literal extension. */
export function mediaMimeType(path: string): string | null {
  const trimmed = path.trim();
  const source = trimmed.startsWith('<') && trimmed.endsWith('>') ? trimmed.slice(1, -1) : trimmed;
  const dataMimeType = /^data:((?:image|video)\/[\w.+-]+)[;,]/i.exec(source)?.[1];
  if (dataMimeType) return dataMimeType.toLowerCase();
  let sourcePath = source.split(/[?#]/, 1)[0] ?? '';
  if (/^(?:https?:|file:|\/\/)/i.test(source)) {
    try { sourcePath = new URL(source, 'https://media.invalid').pathname; } catch { return null; }
  }
  try { sourcePath = decodeURIComponent(sourcePath); } catch { /* A literal percent character is valid in a filename. */ }
  const basename = sourcePath.split(/[\\/]/).pop() ?? '';
  const extensionIndex = basename.lastIndexOf('.');
  return extensionIndex < 0 ? null : mediaMimeTypeFromExtension(basename.slice(extensionIndex));
}

export function mediaKindFromPath(path: string): 'image' | 'video' | null {
  const mimeType = mediaMimeType(path);
  if (mimeType === null) return null;
  return mimeType.startsWith('video/') ? 'video' : 'image';
}

export const isWorkspaceImagePreviewPath = (path: string): boolean => WORKSPACE_IMAGE_PREVIEW_EXTENSIONS.some(extension => path.toLowerCase().endsWith(extension));
/** File viewers receive literal filesystem paths, not Markdown URLs. */
export const isWorkspaceVideoPreviewPath = (path: string): boolean => videoMimeType({ name: path, mimeType: '' }) !== null;

// markdownLinks.ts
export function normalizeMarkdownLinkDestination(value: string): string {
  const trimmed = value.trim();
  return trimmed.startsWith('<') && trimmed.endsWith('>') ? trimmed.slice(1, -1) : trimmed;
}
/** Browser URL parsers write `C:/foo` as `/C:/foo` for file URLs. */
export const stripSlashPrefixedWindowsDrive = (path: string): string => (/^\/[A-Za-z]:[\\/]/.test(path) ? path.slice(1) : path);
export function splitMarkdownLinkSearchAndHash(value: string): { readonly path: string; readonly hash: string } {
  const hashIndex = value.indexOf('#');
  const pathWithSearch = hashIndex >= 0 ? value.slice(0, hashIndex) : value;
  const hash = hashIndex >= 0 ? value.slice(hashIndex) : '';
  const queryIndex = pathWithSearch.indexOf('?');
  return { path: queryIndex >= 0 ? pathWithSearch.slice(0, queryIndex) : pathWithSearch, hash };
}
/** A `file:` URL as a host path, still percent-encoded; a non-localhost authority is a UNC share. */
export function parseFileUrlHref(href: string): { readonly path: string; readonly hash: string } | null {
  try {
    const parsed = new URL(href);
    if (parsed.protocol.toLowerCase() !== 'file:') return null;
    const uncHostname = parsed.hostname.toLowerCase() === 'localhost' ? '' : parsed.hostname;
    const path = uncHostname ? `\\\\${uncHostname}${parsed.pathname.replace(/\//g, '\\')}` : parsed.pathname;
    if (path.length === 0) return null;
    return { path: stripSlashPrefixedWindowsDrive(path), hash: parsed.hash };
  } catch { return null; }
}
export function splitFilePathPosition(path: string, hash = ''): { readonly path: string; readonly line?: number; readonly column?: number } {
  const suffixMatch = path.match(/:(\d+)(?::(\d+))?$/);
  const match = suffixMatch ?? hash.match(/^#L(\d+)(?:C(\d+))?$/i);
  if (!match?.[1]) return { path };
  const line = Number.parseInt(match[1], 10);
  const column = match[2] === undefined ? undefined : Number.parseInt(match[2], 10);
  return { path: suffixMatch ? path.slice(0, -suffixMatch[0].length) : path, ...(line > 0 ? { line } : {}), ...(column !== undefined && column > 0 ? { column } : {}) };
}
export function fileBasename(path: string): string {
  const trimmed = path.replace(/[/\\]+$/, '');
  if (trimmed.length === 0) return path;
  const separatorIndex = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
  return separatorIndex >= 0 ? trimmed.slice(separatorIndex + 1) : trimmed;
}

// markdownImages.ts
export type MarkdownImageSource = { readonly _tag: 'Direct'; readonly uri: string } | { readonly _tag: 'WorkspaceFile'; readonly path: string } | { readonly _tag: 'Blocked' };
export const markdownImageSourceFragment = (source: string): string => splitMarkdownLinkSearchAndHash(normalizeMarkdownLinkDestination(source)).hash;
function joinWorkspacePath(workspaceRoot: string, relativePath: string): string {
  const separator = isWindowsAbsolutePath(workspaceRoot) ? '\\' : '/';
  const root = workspaceRoot.replace(/[\\/]+$/, '');
  const path = relativePath.replace(/[\\/]/g, separator).replace(/^[\\/]+/, '');
  return `${root}${separator}${path}`;
}
/** Where a markdown image or video source's bytes load from; a host path never reaches the image component unsigned. */
export function classifyMarkdownImageSource(value: string | null | undefined, workspaceRoot?: string | null): MarkdownImageSource {
  if (value === null || value === undefined) return { _tag: 'Blocked' };
  const source = normalizeMarkdownLinkDestination(value);
  if (source.length === 0 || source.startsWith('#') || source.startsWith('?')) return { _tag: 'Blocked' };
  if (/^(?:https?:|data:|blob:|\/\/)/i.test(source)) return { _tag: 'Direct', uri: source };
  if (/^file:/i.test(source)) {
    const target = parseFileUrlHref(source);
    return target === null ? { _tag: 'Blocked' } : { _tag: 'WorkspaceFile', path: stripSlashPrefixedWindowsDrive(safeDecodeURIComponent(target.path)) };
  }
  const path = stripSlashPrefixedWindowsDrive(safeDecodeURIComponent(splitMarkdownLinkSearchAndHash(source).path));
  if (path.length === 0) return { _tag: 'Blocked' };
  if (path.startsWith('/') || isWindowsAbsolutePath(path)) return { _tag: 'WorkspaceFile', path };
  if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(path) || path.startsWith('~/') || path.startsWith('~\\')) return { _tag: 'Blocked' };
  if (!workspaceRoot) return { _tag: 'Blocked' };
  return { _tag: 'WorkspaceFile', path: joinWorkspacePath(workspaceRoot, path) };
}

// mediaSource.ts
export type MediaSourceResource = { readonly _tag: 'media-file'; readonly threadId: string; readonly path: string };
export type ResolvedMediaSource = {
  readonly kind: 'image' | 'video'; readonly mimeType: string; readonly name: string; readonly reference?: MediaReference; readonly srcFragment: string;
} & ({ readonly access: 'direct'; readonly uri: string } | { readonly access: 'environment'; readonly resource: MediaSourceResource } | { readonly access: 'unavailable' });
export interface ResolveMediaSourceInput {
  readonly threadId: string | undefined;
  readonly workspaceRoot?: string | null | undefined;
  readonly resolvedFilePath?: string | undefined;
  /** Image syntax can target an endpoint without a recognizable extension. */
  readonly imageEmbed?: boolean | undefined;
}

export function resolveMediaSource(source: string, input: ResolveMediaSourceInput): ResolvedMediaSource | null {
  const classified: MarkdownImageSource = input.resolvedFilePath === undefined ? classifyMarkdownImageSource(source, input.workspaceRoot) : { _tag: 'WorkspaceFile', path: input.resolvedFilePath };
  if (classified._tag === 'Blocked') return null;
  const path = classified._tag === 'Direct' ? splitMarkdownLinkSearchAndHash(classified.uri).path : splitFilePathPosition(classified.path).path;
  // Local paths have already been decoded. Do not interpret literal #, ?, or % characters again.
  const basename = fileBasename(path);
  const extensionIndex = basename.lastIndexOf('.');
  const detectedMimeType = classified._tag === 'Direct' ? mediaMimeType(classified.uri) : extensionIndex < 0 ? null : mediaMimeTypeFromExtension(basename.slice(extensionIndex));
  const mimeType = detectedMimeType ?? (input.imageEmbed ? 'image/*' : null);
  if (mimeType === null) return null;
  const kind = mimeType.startsWith('video/') ? 'video' : 'image';
  const reference = classified._tag === 'Direct' ? mediaUrlReference(classified.uri) : mediaFileReference(path, input.workspaceRoot);
  const name = (reference && mediaReferenceFileName(reference)) || basename || kind;
  const common = { kind, mimeType, name, ...(reference ? { reference } : {}), srcFragment: markdownImageSourceFragment(source) } as const;
  if (classified._tag === 'Direct') return { ...common, access: 'direct', uri: classified.uri };
  if (input.threadId === undefined) return { ...common, access: 'unavailable' };
  return { ...common, access: 'environment', resource: { _tag: 'media-file', threadId: input.threadId, path } };
}
