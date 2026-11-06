// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/media-actions.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// media-actions: the menu, tooltip and byte actions every image and video carries (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3): apps/web/src/components/media/MediaActions.tsx,
// OpenMediaLink.tsx, mediaContent.ts and packages/client-runtime/src/mediaActions.ts.
//
// A surface hands the Contract `MediaActions` wrapper (media-actions.contract) its source as
// text: a JSON `MediaActionSource` its view built, or `attachment:<id>` for a sent attachment
// the transcript draws. The wrapper's right click, Menu key or Shift+F10 runs
// `medialocal:menu` here, which shows the native menu (T3MediaActions.swift `mediaMenu`) and runs
// the pick: copy a path or URL, open the file in the Files surface, save, or copy the image. An
// asset is signed again at action time (`assets.createUrl`), so a player's URL is never replaced.
//
// Desktop Save: the reference's anchor download (`downloadMedia`) reaches Electron's default
// download handling (the main window installs no `will-download`; only the Browser preview does,
// apps/desktop/src/preview/Manager.ts:3570 "Electron opens a native Save dialog for a download with
// no save path"). So Save fetches the bytes, then shows the Save panel as a sheet on the window,
// named with the media's file name and opening in the last folder used (Downloads at first); the
// "Download started" toast shows once the panel is up, as the anchor click returns at once.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { dismissToast, pushToast, updateToast } from './toast';
import { assetUrl } from './settings-b-icons';
import { wakeShell } from './r10-connect-timing';
import { mediaReferenceFileName, type MediaReference } from './media-reference';
import { letGo } from './let-go';

/** Menu action ids shared by every client so labels and handlers line up across surfaces. */
export type MediaActionId = 'copy-full-path' | 'copy-relative-path' | 'copy-url' | 'open-file' | 'save' | 'copy-image';
export type MediaMenuItem = { id: MediaActionId; label: string; disabled?: boolean };

export interface MediaActionSource {
  readonly kind: 'image' | 'video';
  readonly name: string;
  readonly src: string | null;
  readonly reference?: MediaReference;
  /** An `assets.createUrl` resource; a `media-file` without a thread takes the shown thread's. */
  readonly asset?: { readonly resource: Obj };
  /** The workspace-relative path "Open in file viewer" opens (the reference's onOpenFile). */
  readonly openFile?: string;
}

export const encodeMediaSource = (source: MediaActionSource): string => JSON.stringify(source);

export function mediaFileName(source: MediaActionSource): string {
  return (source.reference && mediaReferenceFileName(source.reference)) || source.name || source.kind;
}

/** The tooltip (code style): the path, the URL, or the name. */
export function mediaTooltip(source: MediaActionSource): string {
  const reference = source.reference;
  return reference?.kind === 'file' ? reference.path : reference?.url ?? source.name;
}

/**
 * MediaActions showMenu's items. The native host always has a clipboard, so "Copy image" is never
 * disabled for want of one (declared deviation: the reference disables it in a browser without
 * `navigator.clipboard.write`, which the desktop app always has).
 */
export function mediaMenuItems(source: MediaActionSource, canCopyImage = true): MediaMenuItem[] {
  const noun = source.kind === 'image' ? 'image' : 'video';
  const unavailable = source.src === null && source.asset === undefined;
  const reference = source.reference, items: MediaMenuItem[] = [];
  if (reference?.kind === 'file') {
    items.push({ id: 'copy-full-path', label: 'Copy full path' });
    if (reference.relativePath) items.push({ id: 'copy-relative-path', label: 'Copy relative path' });
  } else if (reference?.kind === 'url') items.push({ id: 'copy-url', label: 'Copy URL' });
  if (source.openFile) items.push({ id: 'open-file', label: 'Open in file viewer' });
  items.push({ id: 'save', label: `Save ${noun}`, disabled: unavailable });
  if (source.kind === 'image') items.push({ id: 'copy-image', label: 'Copy image', disabled: unavailable || !canCopyImage });
  return items;
}

/** "Could not <item label in lower case>", as the reference titles a failed pick. */
export const mediaFailureTitle = (items: readonly MediaMenuItem[], action: string): string =>
  `Could not ${items.find(item => item.id === action)?.label.toLowerCase() ?? 'complete media action'}`;

/** Resolves web references without inheriting the desktop renderer's custom app scheme. */
export function resolveProtocolRelativeMediaUrl(src: string, pageProtocol = 'https:'): string {
  if (!src.startsWith('//')) return src;
  return `${pageProtocol === 'http:' ? 'http:' : 'https:'}${src}`;
}

/** resolveExternalWebLinkHost: the http(s) host of a link, or null. */
export function externalWebLinkHost(href: string | undefined): string | null {
  if (!href) return null;
  try {
    const url = new URL(href.startsWith('//') ? `https:${href}` : href);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.hostname || null : null;
  } catch { return null; }
}

export type OpenMediaLinkView = { url: string; label: string; icon: 'external-link' | 'download' };
/** OpenMediaLink: "Open original" for an authored web URL, "Download video" for a blob, else "Open in browser". */
export function openMediaLink(props: { originalUrl?: string; src?: string | null }): OpenMediaLinkView | null {
  const originalUrl = externalWebLinkHost(props.originalUrl) !== null ? props.originalUrl : undefined;
  const source = originalUrl ?? props.src;
  if (!source) return null;
  const url = resolveProtocolRelativeMediaUrl(source);
  let isBlob = false;
  try {
    const protocol = new URL(url).protocol;
    if (protocol !== 'http:' && protocol !== 'https:' && protocol !== 'blob:') return null;
    isBlob = protocol === 'blob:';
  } catch { return null; }
  return { url, label: originalUrl ? 'Open original' : isBlob ? 'Download video' : 'Open in browser', icon: isBlob ? 'download' : 'external-link' };
}

const FAILED = 'The media action failed.';
const errorText = (error: unknown) => (error instanceof Error && error.message ? error.message : FAILED);

/** A sent attachment the transcript draws, by id (`attachment:<id>`): the image's shown URL, a video's signed asset. */
function attachmentSource(client: T3Client, id: string, urlOf: (id: string) => string | null): MediaActionSource | null {
  const items = [...arr(client.projection.visibleTurnItems).map(row => obj(row.item)), ...arr(client.projection.messages)];
  for (const item of items) for (const attachment of arr(item.attachments)) {
    if (str(attachment.id) !== id) continue;
    const name = str(attachment.name), mimeType = str(attachment.mimeType);
    if (attachment.type === 'image') return { kind: 'image', name: name || 'Media', src: urlOf(id) };
    const video = /^video\//i.test(mimeType) || /\.(?:avi|m4v|mkv|mov|mp4|ogv|webm)$/i.test(name);
    if (attachment.type === 'file' && video) {
      return { kind: 'video', name, src: urlOf(id), asset: { resource: { _tag: 'attachment', attachmentId: id, fileName: name, ...(mimeType ? { mimeType } : {}) } } };
    }
  }
  return null;
}

/** The source a wrapper names: its JSON, or a sent attachment's id. */
export function decodeMediaSource(client: T3Client, text: string, urlOf: (id: string) => string | null = () => null): MediaActionSource | null {
  if (text.startsWith('attachment:')) return attachmentSource(client, text.slice('attachment:'.length), urlOf);
  try {
    const value = JSON.parse(text) as MediaActionSource;
    return value && (value.kind === 'image' || value.kind === 'video') && typeof value.name === 'string' ? { ...value, src: typeof value.src === 'string' ? value.src : null } : null;
  } catch { return null; }
}

/** useMediaActions actionUrl: the authored source, or a freshly signed asset URL. */
export async function mediaActionUrl(client: T3Client, native: Native, source: MediaActionSource): Promise<string> {
  if (!source.asset) {
    if (!source.src) throw new Error('This media is unavailable. Try reopening the preview.');
    return resolveProtocolRelativeMediaUrl(source.src);
  }
  if (client.connection !== 'connected' || !client.origin) throw new Error('Reconnect to this environment and try again.');
  const resource = { ...source.asset.resource };
  if (resource._tag === 'media-file' && !resource.threadId) resource.threadId = client.threadId;
  const result = obj(await client.rpc(native, 'assets.createUrl', { resource }));
  const url = assetUrl(client.origin, str(result.relativeUrl));
  if (!url) throw new Error('The environment returned an invalid media URL.');
  return url;
}

async function nativeValue(client: T3Client, native: Native, request: Obj): Promise<Obj> {
  const value = obj(await client.restAccess(native).call(request));
  if (value.ok === false) throw new Error(str(value.message) || FAILED);
  return value;
}

const openMenus = new WeakSet<T3Client>();
type Hooks = { urlOf?: (id: string) => string | null; openFile?: (relativePath: string) => Promise<void> };

/** MediaActions showMenu: one menu at a time; the pick runs with its progress and failure toasts. */
export async function showMediaMenu(client: T3Client, native: Native, sourceText: string, anchor: string, hooks: Hooks = {}): Promise<string> {
  if (!native.available || openMenus.has(client)) return '';
  openMenus.add(client);
  let failureTitle = 'Could not open media menu', progress = 0;
  try {
    const source = decodeMediaSource(client, sourceText, hooks.urlOf);
    if (!source) throw new Error(FAILED);
    const noun = source.kind === 'image' ? 'image' : 'video', reference = source.reference;
    const items = mediaMenuItems(source);
    const reply = await nativeValue(client, native, { op: 'mediaMenu', items, ...(anchor === 'key' ? { anchor: 'bottom-left' } : {}) });
    const action = str(reply.id);
    if (!action || !items.some(item => item.id === action && !item.disabled)) return '';
    failureTitle = mediaFailureTitle(items, action);
    const text = action === 'copy-full-path' && reference?.kind === 'file' ? reference.path
      : action === 'copy-relative-path' && reference?.kind === 'file' ? reference.relativePath
        : action === 'copy-url' && reference?.kind === 'url' ? reference.url : undefined;
    if (text !== undefined) {
      await nativeValue(client, native, { op: 'mediaCopyText', text });
      pushToast(client, { kind: 'success', title: action === 'copy-url' ? 'URL copied' : 'Path copied' });
    } else if (action === 'open-file') {
      if (source.openFile && hooks.openFile) await hooks.openFile(source.openFile);
    } else if (action === 'save' || action === 'copy-image') {
      progress = pushToast(client, { kind: 'loading', title: action === 'save' ? `Preparing ${noun} download…` : 'Copying image…' });
      await wakeShell(native);
      const url = await mediaActionUrl(client, native, source);
      await nativeValue(client, native, action === 'save' ? { op: 'mediaSave', url, name: mediaFileName(source) } : { op: 'mediaCopyImage', url });
      updateToast(client, progress, { kind: 'success', title: action === 'save' ? 'Download started' : 'Image copied', description: '', timeoutMs: 5000 });
    }
    return action;
  } catch (error) {
    if (letGo(error)) { if (progress) dismissToast(client, progress); throw error; }
    const toast = { kind: 'error' as const, title: failureTitle, description: errorText(error) };
    if (progress) updateToast(client, progress, { ...toast, timeoutMs: 5000 });
    else pushToast(client, { ...toast, stacked: true });
    return '';
  } finally { openMenus.delete(client); }
}
