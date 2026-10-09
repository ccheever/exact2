// media-actions: what each media surface hands the Contract `MediaActions` wrapper and its
// failure states (T3 Code 1e2ecbd975, MIT; see LICENSE-T3):
// - Files: FilePreviewPanel.tsx WorkspaceImagePreview (a `workspace-file` asset, "Unable to load
//   workspace image." with the menu) and WorkspaceVideoPreview (a `media-file` asset in
//   MediaVideoPlayer: the 16:9 "Video unavailable · <name>" slot, Retry video re-signs the URL);
// - chat Markdown: ChatMarkdown.tsx's `img` renderer (classifyMarkdownImageSource and
//   mediaKindFromPath: a direct URL loads as written, a host path through a `media-file` asset,
//   anything else is the "Image unavailable · <alt>" label), ChatMarkdownAssetImage and
//   ChatMarkdownVideo;
// - a video's `error` (MediaVideoPlayer onError) and Retry (onRetry, "Retrying…" while it runs).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { assetUrl } from './settings-b-icons';
import { wakeShell } from './r10-connect-timing';
import { revisedUrl } from './r10-device-files-html';
import { mediaFileReference, mediaUrlReference } from './media-reference';
import { classifyMarkdownImageSource, isWorkspaceImagePreviewPath, isWorkspaceVideoPreviewPath, mediaKindFromPath, normalizeMarkdownLinkDestination } from './media-source';
import { encodeMediaSource, externalWebLinkHost, mediaTooltip, openMediaLink, resolveProtocolRelativeMediaUrl, showMediaMenu, type MediaActionSource } from './media-actions';
import { letGo } from './let-go';

/** A media slot as Contract draws it (media-actions.contract MediaView). */
export type MediaView = {
  kind: string; state: string; url: string; name: string; tip: string; source: string; failText: string;
  openUrl: string; openLabel: string; openIcon: string; retryKey: string; retrying: boolean;
};
export const NO_MEDIA: MediaView = { kind: '', state: '', url: '', name: '', tip: '', source: '', failText: '', openUrl: '', openLabel: '', openIcon: '', retryKey: '', retrying: false };

type MediaState = { failed: Set<string>; retrying: Set<string> };
const states = new WeakMap<T3Client, MediaState>();
function stateOf(client: T3Client): MediaState {
  let state = states.get(client);
  if (!state) { state = { failed: new Set(), retrying: new Set() }; states.set(client, state); }
  return state;
}
/** A video whose player reported `error` for this URL (MediaVideoPlayer failedSrc). */
/** Whether a player or a picture reported an error for this URL (a `video` or `image` `error`, exact2 #121). */
export const mediaFailed = (client: T3Client, url: string): boolean => !!url && stateOf(client).failed.has(url);

// Signed URLs a surface minted (useAssetUrlState), re-minted after five minutes as the HTML page is.
type Minted = { url: string; at: number; error: boolean; resource: Obj };
const minted = new WeakMap<T3Client, Map<string, Minted>>();
const STALE_URL_MS = 5 * 60_000;
function mintedOf(client: T3Client): Map<string, Minted> {
  let map = minted.get(client);
  if (!map) { map = new Map(); minted.set(client, map); }
  return map;
}
async function mint(client: T3Client, native: Native | null | undefined, key: string, resource: Obj, now: number, force = false): Promise<Minted | undefined> {
  const map = mintedOf(client);
  let entry = map.get(key);
  const stale = !!entry && !entry.error && now > 0 && entry.at > 0 && now - entry.at > STALE_URL_MS;
  if ((!entry || stale || force) && native?.available && client.ready && client.connection === 'connected') {
    try {
      const result = obj(await client.rpc(native, 'assets.createUrl', { resource }));
      const url = assetUrl(client.origin, str(result.relativeUrl));
      entry = { url, at: now, error: !url, resource };
    } catch (error) { if (letGo(error)) throw error; entry = { url: '', at: now, error: true, resource }; }
    map.set(key, entry);
    if (map.size > 256) map.delete(map.keys().next().value!);
  }
  return entry;
}

/** The Files surface's media file: a signed image or video and its actions, or NO_MEDIA for any other file. */
export async function filesMediaView(client: T3Client, native: Native | null | undefined, previewPath: string, absolutePath: string, cwd: string, revision: string, now = 0): Promise<MediaView> {
  const kind = isWorkspaceVideoPreviewPath(previewPath) ? 'video' : isWorkspaceImagePreviewPath(previewPath) ? 'image' : '';
  if (!kind || !previewPath || !absolutePath) return NO_MEDIA;
  const resource = client.threadId ? { _tag: kind === 'video' ? 'media-file' : 'workspace-file', threadId: client.threadId, path: absolutePath } : { _tag: 'draft-workspace-file', cwd, path: previewPath };
  const key = `files:${JSON.stringify([client.environmentId, client.threadId, cwd, absolutePath, kind])}`;
  const entry = await mint(client, native, key, resource, now);
  const url = entry && !entry.error ? revisedUrl(entry.url, revision) : '';
  const source: MediaActionSource = { kind, name: previewPath, src: url || null, reference: mediaFileReference(absolutePath, cwd), asset: { resource } };
  // WorkspaceImagePreview / WorkspaceVideoPreview: a refused signature, or the picture's or player's onError for this URL.
  const failed = !!entry?.error || mediaFailed(client, url);
  const open = kind === 'video' ? openMediaLink({ src: url || null }) : null;
  return {
    kind, state: !entry ? 'loading' : failed ? 'failed' : 'ready', url, name: previewPath, tip: mediaTooltip(source), source: encodeMediaSource(source),
    failText: kind === 'video' ? `Video unavailable · ${previewPath}` : 'Unable to load workspace image.',
    openUrl: open?.url ?? '', openLabel: open?.label ?? '', openIcon: open?.icon ?? '', retryKey: kind === 'video' ? key : '', retrying: stateOf(client).retrying.has(key),
  };
}

// Chat Markdown: an image line (`![alt](src)` alone, the only image block markdown.rs draws).
const IMAGE_LINE = /^!\[([^\]\n]*)\]\((.*)\)$/;
// markdown_links.rs's link resolver (ChatMarkdown's markdownUrlTransform over react-markdown's
// defaultUrlTransform, T3 Code 1e2ecbd975, MIT): the same rule on both sides of the chip match.
/** The anchor of a destination the transform emptied (`[parser](fixture.txt:3)`): link text, no target. */
export const NO_HREF = 't3-anchor:';
const SAFE_SCHEMES = /^(?:https?|ircs?|mailto|xmpp)$/i;
/** The scheme defaultUrlTransform reads: the text before a colon that precedes any `/`, `?` or `#`. */
export function urlScheme(href: string): string | null {
  const colon = href.indexOf(':');
  if (colon < 0) return null;
  for (const mark of ['/', '?', '#']) { const at = href.indexOf(mark); if (at >= 0 && at < colon) return null; }
  return href.slice(0, colon);
}
const windowsDrive = (href: string) => /^[A-Za-z](?::|%3a)(?:[\\/]|%2f|%5c)/i.test(href);
/** markdown_links.rs link_href: a reference as written, an emptied anchor, a file link as `t3-file:`, a kept URL, or nothing. */
export function markdownLinkHref(href: string): string {
  const lower = href.toLowerCase();
  if (lower.startsWith('t3-context://') || lower.startsWith('t3-citation://')) return href;
  const scheme = urlScheme(href);
  if (!lower.startsWith('file:') && !windowsDrive(href) && scheme !== null && !SAFE_SCHEMES.test(scheme)) return NO_HREF;
  if (lower.startsWith('file:') || windowsDrive(href) || (!!href && !href.startsWith('#') && !href.startsWith('//') && scheme === null)) return `t3-file:${href}`;
  return !href || href.startsWith('#') ? '' : href;
}
/** markdown.rs's resolver for an image's src: as a link, but an emptied source loads nothing. */
export function markdownImageHref(src: string): string {
  const href = markdownLinkHref(src);
  return href === NO_HREF ? '' : href;
}
/** The image lines of a message, as markdown.rs reads them (src trimmed and cut at a title's quote). */
export function markdownImages(text: string): { alt: string; src: string; href: string }[] {
  if (!text.includes('![')) return [];
  const out: { alt: string; src: string; href: string }[] = [];
  for (const line of text.split('\n')) {
    const match = IMAGE_LINE.exec(line.trim());
    if (!match) continue;
    let src = match[2]!.trim();
    const cut = src.search(/["']/);
    if (cut >= 0) src = src.slice(0, cut).trim();
    if (src.startsWith('t3-context://')) continue; // a context reference is a chip, not media
    out.push({ alt: match[1]!, src, href: markdownImageHref(src) });
  }
  return out;
}
export type MarkdownMedia = { href: string; kind: 'image' | 'video'; alt: string; access: 'direct' | 'environment' | 'blocked'; key: string; source: MediaActionSource | null; originalUrl: string };
/** ChatMarkdown's `img` renderer for one image line: where its bytes load from and its actions. */
export function markdownMedia(alt: string, src: string, href: string, root: string): MarkdownMedia {
  const classified = classifyMarkdownImageSource(normalizeMarkdownLinkDestination(src), root || undefined);
  const kind = mediaKindFromPath(src) ?? 'image', name = alt || kind;
  if (classified._tag === 'Direct') {
    const mediaSrc = resolveProtocolRelativeMediaUrl(classified.uri), reference = mediaUrlReference(classified.uri);
    return { href, kind, alt, access: 'direct', key: mediaSrc, originalUrl: externalWebLinkHost(classified.uri) !== null ? classified.uri : '',
      source: { kind, name, src: mediaSrc, ...(reference ? { reference } : {}) } };
  }
  if (classified._tag === 'WorkspaceFile') {
    const reference = mediaFileReference(classified.path, root || undefined);
    return { href, kind, alt, access: 'environment', key: `media:${classified.path}`, originalUrl: '',
      source: { kind, name, src: null, reference, asset: { resource: { _tag: 'media-file', path: classified.path } }, ...(reference.relativePath ? { openFile: reference.relativePath } : {}) } };
  }
  return { href, kind, alt, access: 'blocked', key: '', originalUrl: '', source: null };
}

/** The chips that carry a message's media to markdown.contract (matched by the image block's href). */
export function markdownMediaChips(text: string, root: string): { href: string; kind: string; label: string; size: string; tip: string; detail: string; icon: string; target: string }[] {
  const seen = new Set<string>();
  return markdownImages(text).flatMap(image => {
    if (seen.has(image.href)) return [];
    seen.add(image.href);
    const media = markdownMedia(image.alt, image.src, image.href, root);
    return [{ href: image.href, kind: 'media', label: image.alt, size: media.kind, tip: media.source ? mediaTooltip(media.source) : '', detail: media.source ? encodeMediaSource(media.source) : '',
      icon: media.access, target: media.key }];
  });
}

/** Signed URLs for the visible messages' host-path media (`media:<path>`; `media-failed:<path>` when refused). */
export async function markdownMediaUrls(client: T3Client, native: Native, root: string, now: number, texts?: string[]): Promise<{ id: string; url: string }[]> {
  const out: { id: string; url: string }[] = [];
  if (!client.threadId) return out;
  const wanted = new Map<string, MarkdownMedia>();
  // markdown-links-and-files-preview: a rendered Markdown file passes its own text (FileMarkdownPreview's images).
  for (const text of texts ?? arr(client.projection.visibleTurnItems).map(row => str(obj(row.item).text))) {
    for (const image of markdownImages(text)) {
      const media = markdownMedia(image.alt, image.src, image.href, root);
      if (media.access === 'environment' && media.source?.asset) wanted.set(media.key, media);
    }
  }
  for (const [key, media] of [...wanted].slice(-32)) {
    const resource = { ...media.source!.asset!.resource, threadId: client.threadId };
    const entry = await mint(client, native, `md:${client.environmentId}|${client.threadId}|${key}`, resource, now);
    if (!entry) continue;
    out.push(entry.error ? { id: `media-failed:${key.slice('media:'.length)}`, url: '' } : { id: key, url: entry.url });
  }
  return out;
}

type Hooks = { urlOf?: (id: string) => string | null; openFile?: (relativePath: string) => Promise<void>; forgetAttachment?: (id: string) => void };
/** `chatlocal:media-*`: the menu, a player's error, and Retry video. */
export async function mediaLocal(client: T3Client, native: Native, op: string, id: string, value: string, hooks: Hooks = {}): Promise<string> {
  const state = stateOf(client);
  if (op === 'menu') { await showMediaMenu(client, native, id, value, hooks); return ''; }
  if (op === 'video-error' || op === 'image-error') { if (id) state.failed.add(id); if (state.failed.size > 64) state.failed.delete(state.failed.values().next().value!); return ''; }
  if (op === 'retry') {
    if (!id || state.retrying.has(id)) return '';
    state.retrying.add(id);
    await wakeShell(native);
    try {
      if (value) state.failed.delete(value);
      if (id.startsWith('attachment:')) hooks.forgetAttachment?.(id.slice('attachment:'.length));
      else {
        // useAssetUrlRefresh: sign the same resource again while "Retrying…" shows.
        const entry = mintedOf(client).get(id);
        if (entry) await mint(client, native, id, entry.resource, entry.at, true);
      }
    } finally { state.retrying.delete(id); }
    return '';
  }
  return '';
}
