// What each media surface hands the Contract `MediaActions` wrapper and its
// failure states (T3 Code 1e2ecbd975, MIT; see LICENSE-T3):
// - Files: FilePreviewPanel.tsx WorkspaceImagePreview (a `workspace-file` asset, "Unable to load
//   workspace image." with the menu) and WorkspaceVideoPreview (a `media-file` asset in
//   MediaVideoPlayer: the 16:9 "Video unavailable · <name>" slot, Retry video re-signs the URL);
// - chat Markdown: ChatMarkdown.tsx's `img` renderer (classifyMarkdownImageSource and
//   mediaKindFromPath: a direct URL loads as written, a host path through a `media-file` asset,
//   anything else is the "Image unavailable · <alt>" label), ChatMarkdownAssetImage and
//   ChatMarkdownVideo;
// - a video's `error` (MediaVideoPlayer onError) and Retry (onRetry, "Retrying…" while it runs).
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { assetUrl } from '../settings/settings-b-icons';
import { wakeShell } from '../client/command-waits';
import { revisedUrl } from '../panel/files-rendered-html';
import { mediaFileReference, mediaUrlReference } from './media-reference';
import { classifyMarkdownImageSource, isWorkspaceImagePreviewPath, isWorkspaceVideoPreviewPath, markdownImageSourceFragment, mediaKindFromPath, normalizeMarkdownLinkDestination } from './media-source';
import { encodeMediaSource, externalWebLinkHost, mediaTooltip, openMediaLink, resolveProtocolRelativeMediaUrl, showMediaMenu, type MediaActionSource } from './media-actions';
import { letGo } from '../client/let-go';

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
/** markdown_links.rs link_href: a reference as written, an emptied anchor, a file link as `t3-file:`, a kept URL or fragment, or nothing. */
export function markdownLinkHref(href: string): string {
  const lower = href.toLowerCase();
  if (lower.startsWith('t3-context://') || lower.startsWith('t3-citation://')) return href;
  const scheme = urlScheme(href);
  if (!lower.startsWith('file:') && !windowsDrive(href) && scheme !== null && !SAFE_SCHEMES.test(scheme)) return NO_HREF;
  if (lower.startsWith('file:') || windowsDrive(href) || (!!href && !href.startsWith('#') && !href.startsWith('//') && scheme === null)) return `t3-file:${href}`;
  return href; // a fragment (`#notes`) stays the anchor's href
}
/** markdown.rs's resolver for an image's src: as a link, but an emptied source or a fragment loads nothing. */
export function markdownImageHref(src: string): string {
  const href = markdownLinkHref(src);
  return href === NO_HREF || href.startsWith('#') ? '' : href;
}
/**
 * The image lines of a message, as markdown.rs reads them (src trimmed and cut at a title's quote): one image spanning
 * the line (markdown_media.rs sole_image_line). A line of several images (a badge row) or an image and text is inline.
 */
export function markdownImages(text: string): { alt: string; src: string; href: string }[] {
  if (!text.includes('![')) return [];
  const out: { alt: string; src: string; href: string }[] = [];
  for (const line of text.split('\n')) {
    const trimmed = line.trim(), match = IMAGE_LINE.exec(trimmed);
    if (!match) continue;
    const span = imageSpan(trimmed, 0);
    if (span && span.end !== trimmed.length - 1) continue;
    let src = match[2]!.trim();
    const cut = src.search(/["']/);
    if (cut >= 0) src = src.slice(0, cut).trim();
    if (src.startsWith('t3-context://')) continue; // a context reference is a chip, not media
    out.push({ alt: match[1]!, src, href: markdownImageHref(src) });
  }
  return out;
}
/** markdown_parse's inline `target`: the destination trimmed, cut at a title's quote, its angle brackets dropped. */
function inlineTarget(inside: string): string {
  let target = inside.trim();
  const cut = target.search(/["']/);
  if (cut >= 0) target = target.slice(0, cut).trim();
  return target.replace(/^<+/, '').replace(/>+$/, '');
}
/** `![alt](inside)` at `at`: the `]` and the `)` that close it (markdown_media.rs image_span). */
function imageSpan(line: string, at: number): { close: number; end: number } | null {
  const close = line.indexOf(']', at + 2);
  if (close < 0 || line[close + 1] !== '(') return null;
  let depth = 0;
  for (let index = close + 1; index < line.length; index++) {
    if (line[index] === '(') depth++;
    else if (line[index] === ')' && --depth === 0) return { close, end: index };
  }
  return null;
}
const leading = (text: string, marker: string) => { let count = 0; while (text[count] === marker) count++; return count; };
/** The run href of an image inside a line of text (markdown_media.rs IMAGE_MARK). */
export const INLINE_IMAGE = 't3-image:';
/** markdown_media.rs code_span_end: where the code span whose backticks start at `at` ends (unclosed: its backticks). */
function codeSpanEnd(line: string, at: number): number {
  const run = leading(line.slice(at), '`');
  let from = at + run;
  for (;;) {
    const found = line.indexOf('`'.repeat(run), from);
    if (found < 0) return at + run;
    const length = leading(line.slice(found), '`');
    if (length === run) return found + run;
    from = found + length;
  }
}
/** markdown_media.rs starts_link_text: the `[` at `open` starts a link's text (its matching `]` is followed by `(`). */
function startsLinkText(line: string, open: number): boolean {
  for (let at = open + 1, depth = 1; at < line.length;) {
    const character = line[at];
    if (character === '\\') at += 2;
    else if (character === '`') at = codeSpanEnd(line, at);
    else if (character === '[') { depth++; at++; }
    else if (character === ']') { if (--depth === 0) return line[at + 1] === '('; at++; }
    else at++;
  }
  return false;
}
export type InlineImage = { alt: string; src: string; href: string; linked: boolean };
/**
 * The images inside a line of text or a table cell, which markdown_media.rs makes media runs (ChatMarkdownImage;
 * standalone when alone in its paragraph, list item or quote): outside fences and code spans, not one whose alt text
 * holds a `[`. `linked`: inside a link's text (MarkdownAnchor's `<a>` around the `<img>`). An image line may be either
 * (a block, or inline where it continues a paragraph), so it is listed here too.
 */
export function markdownInlineImages(text: string): InlineImage[] {
  if (!text.includes('![')) return [];
  const out: InlineImage[] = [];
  let fence: { marker: string; width: number } | null = null;
  for (const line of text.split('\n')) {
    const trimmed = line.trimStart().replace(/\r$/, '');
    if (fence) {
      const count = leading(trimmed, fence.marker);
      if (count >= fence.width && !trimmed.slice(count).trim()) fence = null;
      continue;
    }
    const first = trimmed[0] ?? '';
    if ((first === '`' || first === '~') && leading(trimmed, first) >= 3) { fence = { marker: first, width: leading(trimmed, first) }; continue; }
    if (!line.includes('![')) continue;
    const opened: number[] = [];
    let at = 0;
    while (at < line.length) {
      const character = line[at];
      if (character === '\\') at += 2;
      else if (character === '`') at = codeSpanEnd(line, at);
      else if (character === '[') { opened.push(at); at++; }
      else if (character === ']') { opened.pop(); at++; }
      else if (character === '!' && line[at + 1] === '[') {
        const span = imageSpan(line, at);
        if (!span || line.slice(at + 2, span.close).includes('[')) { at++; continue; }
        const src = inlineTarget(line.slice(span.close + 2, span.end));
        const linked = opened.length > 0 && startsLinkText(line, opened[opened.length - 1]!);
        if (!src.startsWith('t3-context://')) out.push({ alt: line.slice(at + 2, span.close), src, href: `${INLINE_IMAGE}${markdownImageHref(src)}`, linked });
        at = span.end + 1;
      } else at++;
    }
  }
  return out;
}
/** A message's image lines and inline images, each with the href its block or run keeps. */
const allImages = (text: string) => [...markdownImages(text), ...markdownInlineImages(text)];

/**
 * Where a document's media resolve (ChatMarkdown's props): `root` is `imageBaseDir ?? cwd`, the folder a relative
 * source joins; `cwd` the workspace a media reference is relative to (its "Copy relative path" and "Open in file
 * viewer"; FileMarkdownPreview passes the workspace while `imageBaseDir` is the file's folder); `thread` whether there is
 * a thread to sign a host path through (AttachmentFilePreview's ChatMarkdown has none: such media is unavailable).
 */
export type MediaScope = { cwd?: string; thread?: boolean };
export type MarkdownMedia = { href: string; kind: 'image' | 'video'; alt: string; access: 'direct' | 'environment' | 'blocked'; key: string; fragment: string; source: MediaActionSource | null; originalUrl: string };
/** ChatMarkdown's `img` renderer for one image: where its bytes load from and its actions. */
export function markdownMedia(alt: string, src: string, href: string, root: string, scope: MediaScope = {}): MarkdownMedia {
  const classified = classifyMarkdownImageSource(normalizeMarkdownLinkDestination(src), root || undefined);
  const kind = mediaKindFromPath(src) ?? 'image', name = alt || kind, cwd = scope.cwd ?? root;
  if (classified._tag === 'Direct') {
    const mediaSrc = resolveProtocolRelativeMediaUrl(classified.uri), reference = mediaUrlReference(classified.uri);
    return { href, kind, alt, access: 'direct', key: mediaSrc, fragment: '', originalUrl: externalWebLinkHost(classified.uri) !== null ? classified.uri : '',
      source: { kind, name, src: mediaSrc, ...(reference ? { reference } : {}) } };
  }
  // ChatMarkdownAssetImage: a `media-file` of the thread, the source's fragment on its signed URL, its reference and
  // "Open in file viewer" relative to `cwd` (workspaceRoot); with no thread, ChatMarkdownImageFallback.
  if (classified._tag === 'WorkspaceFile' && scope.thread !== false) {
    const reference = mediaFileReference(classified.path, cwd || undefined), fragment = markdownImageSourceFragment(src);
    return { href, kind, alt, access: 'environment', key: `media:${classified.path}${fragment}`, fragment, originalUrl: '',
      source: { kind, name, src: null, reference, asset: { resource: { _tag: 'media-file', path: classified.path } }, ...(reference.relativePath ? { openFile: reference.relativePath } : {}) } };
  }
  return { href, kind, alt, access: 'blocked', key: '', fragment: '', originalUrl: '', source: null };
}

/** The Markdown documents on screen whose images open the expanded preview, by the owner
 *  their chips carry (a message row's id, `files:<path>`, `attachment:<id>`): ChatMarkdown's markdownImageGallery scope. */
export type MarkdownDocument = { text: string; root: string; scope: MediaScope };
const documents = new Map<string, MarkdownDocument>();
export function noteMarkdownDocument(owner: string, text: string, root: string, scope: MediaScope = {}): void {
  if (!owner || !text.includes('![')) return;
  documents.delete(owner);
  documents.set(owner, { text, root, scope });
  if (documents.size > 256) documents.delete(documents.keys().next().value!);
}
export const markdownDocument = (owner: string): MarkdownDocument | null => documents.get(owner) ?? null;
export type GalleryImage = { key: string; name: string; access: MarkdownMedia['access']; source: MediaActionSource };
/** markdownImageGallery: the document's images in order (an image that can load, not a video, a table cell's too; not one
 *  inside a link's text, whose press is the link's), each as ExpandedImagePreview names it (`alt.trim() || "image"`). */
export function markdownGallery(document: MarkdownDocument): GalleryImage[] {
  return markdownInlineImages(document.text).filter(image => !image.linked).flatMap(image => {
    const media = markdownMedia(image.alt, image.src, image.href, document.root, document.scope);
    return media.kind === 'image' && media.access !== 'blocked' && media.source ? [{ key: media.key, name: image.alt.trim() || 'image', access: media.access, source: media.source }] : [];
  });
}

/** The chips that carry a message's media to markdown.contract (matched by the image block's or media run's href). */
export function markdownMediaChips(text: string, root: string, scope: MediaScope = {}): { href: string; kind: string; label: string; size: string; tip: string; detail: string; icon: string; target: string }[] {
  const seen = new Set<string>();
  return allImages(text).flatMap(image => {
    if (seen.has(image.href)) return [];
    seen.add(image.href);
    const media = markdownMedia(image.alt, image.src, image.href, root, scope);
    return [{ href: image.href, kind: 'media', label: image.alt, size: media.kind, tip: media.source ? mediaTooltip(media.source) : '', detail: media.source ? encodeMediaSource(media.source) : '',
      icon: media.access, target: media.key }];
  });
}

/**
 * Signed URLs for the visible messages' host-path media (`media:<path>`; `media-failed:<path>` when refused, with
 * `media-noaccess:<path>` too when the session lacks `filesystem:read`, which the host requires: blockedAssetUrlState
 * skips the request so the label can say to pair again), or a given document's. `threadId` names the thread they sign through (a draft's own id in Files, as the reference's draft
 * thread ref; the server signs an absolute `media-file` path without looking the thread up).
 */
export async function markdownMediaUrls(client: T3Client, native: Native, root: string, now: number, texts?: string[], scope: MediaScope & { threadId?: string } = {}): Promise<{ id: string; url: string }[]> {
  const out: { id: string; url: string }[] = [];
  const threadId = scope.threadId ?? client.threadId;
  if (!threadId) return out;
  const wanted = new Map<string, MarkdownMedia>();
  // A rendered Markdown file passes its own text (FileMarkdownPreview's images).
  for (const text of texts ?? arr(client.projection.visibleTurnItems).map(row => str(obj(row.item).text))) {
    for (const image of allImages(text)) {
      const media = markdownMedia(image.alt, image.src, image.href, root, scope);
      if (media.access === 'environment' && media.source?.asset) wanted.set(media.key, media);
    }
  }
  // An unread grant (no scopes yet) may still ask the host, as useFilesystemReadAccess's pending state does not refuse.
  const blocked = client.ready && !client.scopes?.includes('filesystem:read');
  for (const [key, media] of [...wanted].slice(-32)) {
    if (blocked) { out.push({ id: `media-failed:${key.slice('media:'.length)}`, url: '' }, { id: `media-noaccess:${key.slice('media:'.length)}`, url: '' }); continue; }
    const resource = { ...media.source!.asset!.resource, threadId };
    const entry = await mint(client, native, `md:${client.environmentId}|${threadId}|${key}`, resource, now);
    if (!entry) continue;
    out.push(entry.error ? { id: `media-failed:${key.slice('media:'.length)}`, url: '' } : { id: key, url: entry.url + media.fragment });
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
