// What each media surface hands the Contract `MediaActions` wrapper and its
// failure states (T3 Code 1e2ecbd975, MIT; see LICENSE-T3):
// - Files: FilePreviewPanel.tsx WorkspaceImagePreview (a `workspace-file` asset, "Unable to load
//   workspace image." with the menu) and WorkspaceVideoPreview (a `media-file` asset in
//   MediaVideoPlayer: the 16:9 "Video unavailable · <name>" slot, Retry video re-signs the URL);
// - chat Markdown: ChatMarkdown.tsx's `img` renderer (classifyMarkdownImageSource and
//   mediaKindFromPath: a direct URL loads as written, a host path through a `media-file` asset,
//   anything else is the "Image unavailable · <alt>" label), ChatMarkdownAssetImage and
//   ChatMarkdownVideo;
// - a video's `error` (MediaVideoPlayer onError) and Retry (onRetry, "Retrying…" while it runs);
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { assetUrl } from '../settings/settings-b-icons';
import { wakeShell } from '../client/command-waits';
import { revisedUrl } from '../panel/files-rendered-html';
import { mediaFileReference, mediaUrlReference } from './media-reference';
import { classifyMarkdownImageSource, fileBasename, isWorkspaceImagePreviewPath, isWorkspaceVideoPreviewPath, markdownImageSourceFragment, mediaKindFromPath, normalizeMarkdownLinkDestination, splitFilePathPosition, splitMarkdownLinkSearchAndHash } from './media-source';
import { encodeMediaSource, externalWebLinkHost, mediaTooltip, openMediaLink, resolveProtocolRelativeMediaUrl, showMediaMenu, type MediaActionSource } from './media-actions';
import { letGo } from '../client/let-go';
import { chatFileLink } from '../timeline/markdown-file-links';
import { workspaceLookupPath } from '../timeline/workspace-file-lookup';
import { fileIconToken } from '../shared/timeline-files';

/** A media slot as Contract draws it (media-actions.contract MediaView). */
/** `revision`: what a video's URL names, the file's own revision or else the workspace's, which its player holds its loaded
 *  source by ('' for none). */
export type MediaView = {
  kind: string; state: string; url: string; name: string; tip: string; source: string; failText: string;
  openUrl: string; openLabel: string; openIcon: string; retryKey: string; retrying: boolean; revision: string;
};
export const NO_MEDIA: MediaView = { kind: '', state: '', url: '', name: '', tip: '', source: '', failText: '', openUrl: '', openLabel: '', openIcon: '', retryKey: '', retrying: false, revision: '' };

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

// The workspace mutation each preview's image or Files video was last signed for (useAssetUrlRefresh's handled token);
// null after the connection dropped, so the next connected view signs it again.
const handledMutations = new WeakMap<T3Client, Map<string, string | null>>();
function refreshDue(client: T3Client, id: string, mutationId: string | null): boolean {
  let handled = handledMutations.get(client);
  if (!handled) { handled = new Map(); handledMutations.set(client, handled); }
  if (client.connection !== 'connected') { handled.set(id, null); return false; }
  const token = JSON.stringify(mutationId), last = handled.get(id);
  if (last === token) return false;
  handled.set(id, token);
  if (handled.size > 256) handled.delete(handled.keys().next().value!);
  // The first view shares the signature it is about to mint (only the initial mount shares a pending request).
  return last !== undefined || mintedOf(client).has(id);
}

// Signed URLs a surface minted (useAssetUrlState), re-minted after five minutes as the HTML page is. `fileRevision`: the
// file's identity and contents, which a newer server reports (AssetCreateUrlResult), unlike the URL, which every mint signs
// anew.
type Minted = { url: string; at: number; error: boolean; resource: Obj; fileRevision?: string };
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
      const url = assetUrl(client.origin, str(result.relativeUrl)), fileRevision = str(result.fileRevision).trim().slice(0, 256);
      entry = { url, at: now, error: !url, resource, ...(fileRevision ? { fileRevision } : {}) };
    } catch (error) { if (letGo(error)) throw error; entry = { url: '', at: now, error: true, resource }; }
    map.set(key, entry);
    if (map.size > 256) map.delete(map.keys().next().value!);
  }
  return entry;
}

/**
 * The Files surface's media file: a signed image or video and its actions, or NO_MEDIA for any other file. `revision` is
 * the workspace's (workspaceMutationId), and an image's URL carries it. A video is signed again each time it moves
 * (WorkspaceVideoPreview's useAssetUrlState), and its URL names the file's own revision when the server reports one, else
 * the workspace's; its player (media-actions.contract FilesVideo) holds the source it loaded while that revision holds
 * and takes a new one once playback pauses or ends (MediaVideoPlayer).
 */
export async function filesMediaView(client: T3Client, native: Native | null | undefined, previewPath: string, absolutePath: string, cwd: string, revision: string, now = 0): Promise<MediaView> {
  const kind = isWorkspaceVideoPreviewPath(previewPath) ? 'video' : isWorkspaceImagePreviewPath(previewPath) ? 'image' : '';
  if (!kind || !previewPath || !absolutePath) return NO_MEDIA;
  // workspaceAssetResource: the server signs `workspace-file` only below the workspace root, so a picture opened from
  // outside it (an absolute link in the thread) is a `media-file`, as a video always is.
  const inside = mediaFileReference(absolutePath, cwd).relativePath !== undefined;
  const resource = client.threadId ? { _tag: kind === 'video' || !inside ? 'media-file' : 'workspace-file', threadId: client.threadId, path: absolutePath } : { _tag: 'draft-workspace-file', cwd, path: absolutePath };
  const key = `files:${JSON.stringify([client.environmentId, client.threadId, cwd, absolutePath, kind])}`;
  const entry = await mint(client, native, key, resource, now, kind === 'video' && !!revision && refreshDue(client, key, revision));
  // The revision goes in the URL too, since two mints in the same millisecond sign identical URLs.
  const videoRevision = kind === 'video' ? entry?.fileRevision || revision : '';
  const url = !entry || entry.error ? '' : kind === 'video' ? revisedUrl(entry.url, videoRevision, entry.fileRevision ? 'file-revision' : 'workspace-revision') : revisedUrl(entry.url, revision);
  const source: MediaActionSource = { kind, name: previewPath, src: url || null, reference: mediaFileReference(absolutePath, cwd), asset: { resource } };
  // WorkspaceImagePreview / WorkspaceVideoPreview: a refused signature, or the picture's or player's onError for this URL.
  const failed = !!entry?.error || mediaFailed(client, url);
  const open = kind === 'video' ? openMediaLink({ src: url || null }) : null;
  return {
    kind, state: !entry ? 'loading' : failed ? 'failed' : 'ready', url, name: previewPath, tip: mediaTooltip(source), source: encodeMediaSource(source),
    failText: kind === 'video' ? `Video unavailable · ${previewPath}` : 'Unable to load workspace image.',
    openUrl: open?.url ?? '', openLabel: open?.label ?? '', openIcon: open?.icon ?? '', retryKey: kind === 'video' ? key : '', retrying: stateOf(client).retrying.has(key),
    revision: videoRevision,
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
const reference = (lower: string) => lower.startsWith('t3-context://') || lower.startsWith('t3-citation://');
/** markdown_links.rs kept_by_transform: references, Windows drive paths and `file:` URLs pass, and any href without a scheme or with a safe one. */
function keptByTransform(href: string): boolean {
  const lower = href.toLowerCase();
  if (reference(lower) || lower.startsWith('t3-thread://v1/') || lower.startsWith('file:') || windowsDrive(href)) return true;
  const scheme = urlScheme(href);
  return scheme === null || SAFE_SCHEMES.test(scheme);
}
/**
 * markdown_links.rs link_href: a reference as written, an emptied anchor, a file link (chatFileLink, T3 Code
 * 7c3b3b3605ef) as `t3-file:`, else the href as written (a fragment, a bare name, a web URL or a network path).
 */
export function markdownLinkHref(href: string): string {
  if (reference(href.toLowerCase())) return href;
  if (!keptByTransform(href)) return NO_HREF;
  return chatFileLink(href) ? `t3-file:${href}` : href;
}
/**
 * markdown_links.rs media_href for an image's src: a file source (a `file:` URL, a Windows drive path, or one with no
 * scheme that is not a fragment or protocol-relative) as `t3-file:`; an emptied source or a fragment loads nothing.
 */
export function markdownImageHref(src: string): string {
  const lower = src.toLowerCase();
  if (reference(lower)) return src;
  if (!keptByTransform(src) || src.startsWith('#')) return '';
  const file = lower.startsWith('file:') || windowsDrive(src) || (!!src && !src.startsWith('//') && urlScheme(src) === null);
  return file ? `t3-file:${src}` : src;
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
    const span = lineMarks(trimmed).imageSpan(0);
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
const leading = (text: string, marker: string, from = 0) => { let end = from; while (text[end] === marker) end++; return end - from; };
/** The run href of an image inside a line of text (markdown_media.rs IMAGE_MARK). */
export const INLINE_IMAGE = 't3-image:';
/** The index of the first of the ascending `positions` that is `from` or after (`positions.length` with none). */
function firstFrom(positions: number[], from: number): number {
  let low = 0, high = positions.length;
  while (low < high) { const middle = (low + high) >> 1; if (positions[middle]! < from) low = middle + 1; else high = middle; }
  return low;
}
/**
 * One line's brackets, parentheses and backtick runs, each found once (markdown_media.rs LineMarks), so each `![`, `[`
 * and run of backticks it holds is read in constant time: a line of many unclosed `![` or `(`, or of images inside an
 * unclosed link's text, reads in linear time, where reading on from each to the line's end did not.
 */
function lineMarks(line: string) {
  const opens: number[] = [], closes: number[] = [], unclosed: number[] = [], brackets: number[] = [];
  // The `)` that balances the `(` at each position, every parenthesis counted, as an image's destination counts them.
  const parens = new Map<number, number>();
  // The start of each run of backticks, by its length.
  const ticks = new Map<number, number[]>();
  // Whether the `[` at each position starts a link's text: its matching `]`, escapes and code spans aside, is followed by `(`.
  const linkTexts = new Map<number, boolean>();
  for (let at = 0; at < line.length;) {
    const character = line[at];
    if (character === '`') {
      const run = leading(line, '`', at), starts = ticks.get(run);
      if (starts) starts.push(at);
      else ticks.set(run, [at]);
      at += run;
      continue;
    }
    if (character === '[') opens.push(at);
    else if (character === ']') closes.push(at);
    else if (character === '(') unclosed.push(at);
    else if (character === ')') { const open = unclosed.pop(); if (open !== undefined) parens.set(open, at); }
    at++;
  }
  // markdown_media.rs code_span_end: past the next run of as many backticks, or, with none, past its own run, which is
  // then text.
  const codeSpanEnd = (at: number) => {
    const run = leading(line, '`', at), starts = ticks.get(run) ?? [], next = starts[firstFrom(starts, at + run)];
    return next === undefined ? at + run : next + run;
  };
  for (let at = 0; at < line.length;) {
    const character = line[at];
    if (character === '\\') at += 2;
    else if (character === '`') at = codeSpanEnd(at);
    else if (character === '[') brackets.push(at++);
    else if (character === ']') { const open = brackets.pop(); if (open !== undefined) linkTexts.set(open, line[at + 1] === '('); at++; }
    else at++;
  }
  return {
    codeSpanEnd,
    /** `![alt](inside)` at `at`: the first `]` after its `![`, followed by a `(` whose `)` balances it. */
    imageSpan(at: number): { close: number; end: number } | null {
      const close = closes[firstFrom(closes, at + 2)];
      const end = close !== undefined && line[close + 1] === '(' ? parens.get(close + 1) : undefined;
      return close !== undefined && end !== undefined ? { close, end } : null;
    },
    /** Whether a `[` lies from `from` up to `to`. */
    opensWithin: (from: number, to: number) => (opens[firstFrom(opens, from)] ?? to) < to,
    startsLinkText: (open: number) => linkTexts.get(open) === true,
  };
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
    const trimmed = line.trimStart(); // the fence checks count markers from its start and trim the rest, a CRLF's `\r` too
    if (fence) {
      const count = leading(trimmed, fence.marker);
      if (count >= fence.width && !trimmed.slice(count).trim()) fence = null;
      continue;
    }
    const first = trimmed[0] ?? '';
    if ((first === '`' || first === '~') && leading(trimmed, first) >= 3) { fence = { marker: first, width: leading(trimmed, first) }; continue; }
    // An image's `]` is followed by its `(`: a line with no `](` holds none, and is not read past these searches.
    if (!line.includes('![') || !line.includes('](')) continue;
    const marks = lineMarks(line), opened: number[] = [];
    let at = 0;
    while (at < line.length) {
      const character = line[at];
      if (character === '\\') at += 2;
      else if (character === '`') at = marks.codeSpanEnd(at);
      else if (character === '[') { opened.push(at); at++; }
      else if (character === ']') { opened.pop(); at++; }
      else if (character === '!' && line[at + 1] === '[') {
        const span = marks.imageSpan(at);
        if (!span || marks.opensWithin(at + 2, span.close)) { at++; continue; }
        const src = inlineTarget(line.slice(span.close + 2, span.end));
        const linked = opened.length > 0 && marks.startsLinkText(opened[opened.length - 1]!);
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
/**
 * `kind` "audio" is a sound (InlineAudioPreview: its `resource` signed for a host file, else a direct `key`); "file" a file
 * that is no media, drawn as its file chip (`file`).
 */
export type MarkdownMedia = {
  href: string; kind: 'image' | 'video' | 'audio' | 'file'; alt: string; access: 'direct' | 'environment' | 'blocked'; key: string; fragment: string;
  source: MediaActionSource | null; originalUrl: string; resource?: Obj; name?: string; openPath?: string; file?: MarkdownFileChip;
};
/** A file chip's face and press (markdown.contract's file link chips): its label, full path, Pierre icon and the path a lookup resolves. */
export type MarkdownFileChip = { label: string; tip: string; icon: string; target: string };
/** ChatMarkdown's `img` renderer for one image: where its bytes load from and its actions. */
export function markdownMedia(alt: string, src: string, href: string, root: string, scope: MediaScope = {}): MarkdownMedia {
  const classified = classifyMarkdownImageSource(normalizeMarkdownLinkDestination(src), root || undefined);
  const mediaKind = mediaKindFromPath(src), cwd = scope.cwd ?? root;
  // T3 Code 7c3b3b3605ef (#21363): a sound plays in its audio chip, and a host file of no media kind is its file chip.
  if (mediaKind === 'audio') return markdownAudio(alt, src, href, classified, scope);
  if (mediaKind === null && classified._tag === 'WorkspaceFile' && chatFileLink(src)) {
    const file = markdownFileChip(src, classified.path, root, cwd);
    return { href, kind: 'file', alt, access: 'environment', key: file.tip, fragment: '', originalUrl: '', source: null, file };
  }
  const kind = mediaKind ?? 'image', name = alt || kind;
  if (classified._tag === 'Direct') {
    const mediaSrc = resolveProtocolRelativeMediaUrl(classified.uri), reference = mediaUrlReference(classified.uri);
    return { href, kind, alt, access: 'direct', key: mediaSrc, fragment: '', originalUrl: externalWebLinkHost(classified.uri) !== null ? classified.uri : '',
      source: { kind, name, src: mediaSrc, ...(reference ? { reference } : {}) } };
  }
  // ChatMarkdownAssetImage: a `media-file` of the thread, the source's fragment on its signed URL, its reference and
  // "Open in file viewer" relative to `cwd` (workspaceRoot); with no thread, ChatMarkdownImageFallback.
  if (classified._tag === 'WorkspaceFile' && scope.thread !== false) {
    const reference = mediaFileReference(classified.path, cwd || undefined), fragment = markdownImageSourceFragment(src), resource = { _tag: 'media-file', path: classified.path };
    return { href, kind, alt, access: 'environment', key: `media:${classified.path}${fragment}`, fragment, originalUrl: '', resource,
      source: { kind, name, src: null, reference, asset: { resource }, ...(reference.relativePath ? { openFile: reference.relativePath } : {}) } };
  }
  return { href, kind, alt, access: 'blocked', key: '', fragment: '', originalUrl: '', source: null };
}
/**
 * ChatMarkdown's audio: named by its file (else its alt, else "Audio"); a host file plays from its signed `media-file`
 * and opens beside the thread, a web URL plays as written, and anything else is "Audio unavailable · <alt>".
 */
function markdownAudio(alt: string, src: string, href: string, classified: ReturnType<typeof classifyMarkdownImageSource>, scope: MediaScope): MarkdownMedia {
  const filePath = classified._tag === 'WorkspaceFile' ? classified.path : null;
  const name = fileBasename(filePath ?? src.split(/[?#]/, 1)[0] ?? '') || alt || 'Audio';
  const common = { href, kind: 'audio', alt, fragment: '', source: null, originalUrl: '', name } as const;
  if (filePath !== null && scope.thread !== false) return { ...common, access: 'environment', key: `media:${filePath}`, resource: { _tag: 'media-file', path: filePath }, openPath: filePath };
  if (classified._tag === 'Direct') return { ...common, access: 'direct', key: resolveProtocolRelativeMediaUrl(classified.uri) };
  return { ...common, access: 'blocked', key: '' };
}
/** fileLinkChip's face for an image source: fileLinkLabel (the name, then its line), the full path, its icon and lookup path. */
function markdownFileChip(src: string, resolved: string, root: string, cwd: string): MarkdownFileChip {
  const { hash } = splitMarkdownLinkSearchAndHash(normalizeMarkdownLinkDestination(src)), { path, line, column } = splitFilePathPosition(resolved, hash);
  const label = [fileBasename(path), line ? `L${line}${column ? `:C${column}` : ''}` : ''].filter(Boolean).join(' · ');
  return { label, tip: line ? `${path}:${line}${column ? `:${column}` : ''}` : path, icon: fileIconToken(path), target: workspaceLookupPath(src, root, cwd) };
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
    // A file chip carries its label, full path, Pierre icon and lookup path as a link chip does; a sound its name and the file it opens.
    const file = media.file;
    if (file) return [{ href: image.href, kind: 'media', label: file.label, size: 'file', tip: file.tip, detail: '', icon: file.icon, target: file.target }];
    if (media.kind === 'audio') return [{ href: image.href, kind: 'media', label: image.alt, size: 'audio', tip: media.name ?? '', detail: media.openPath ?? '', icon: media.access, target: media.key }];
    return [{ href: image.href, kind: 'media', label: image.alt, size: media.kind, tip: media.source ? mediaTooltip(media.source) : '', detail: media.source ? encodeMediaSource(media.source) : '',
      icon: media.access, target: media.key }];
  });
}

/**
 * Signed URLs for the visible messages' host-path media (`media:<path>`; `media-failed:<path>` when refused, with
 * `media-noaccess:<path>` too when the session lacks `filesystem:read`, which the host requires: blockedAssetUrlState
 * skips the request so the label can say to pair again), or a given document's. `threadId` names the thread they sign through (a draft's own id in Files, as the reference's draft
 * thread ref; the server signs an absolute `media-file` path without looking the thread up).
 * A rendered Markdown file passes its workspace's `mutationId` (ChatMarkdownAssetImage at 74541bab17): when it changes,
 * or the connection comes back, each image is signed again so an edited file's picture reloads; a video or a sound is not.
 */
export async function markdownMediaUrls(client: T3Client, native: Native, root: string, now: number, texts?: string[], scope: MediaScope & { threadId?: string; mutationId?: string | null } = {}): Promise<{ id: string; url: string }[]> {
  const out: { id: string; url: string }[] = [];
  const threadId = scope.threadId ?? client.threadId;
  if (!threadId) return out;
  const wanted = new Map<string, MarkdownMedia>();
  // A rendered Markdown file passes its own text (FileMarkdownPreview's images).
  for (const text of texts ?? arr(client.projection.visibleTurnItems).map(row => str(obj(row.item).text))) {
    for (const image of allImages(text)) {
      const media = markdownMedia(image.alt, image.src, image.href, root, scope);
      if (media.access === 'environment' && media.resource) wanted.set(media.key, media);
    }
  }
  // An unread grant (no scopes yet) may still ask the host, as useFilesystemReadAccess's pending state does not refuse.
  const blocked = client.ready && !client.scopes?.includes('filesystem:read');
  for (const [key, media] of [...wanted].slice(-32)) {
    if (blocked) { out.push({ id: `media-failed:${key.slice('media:'.length)}`, url: '' }, { id: `media-noaccess:${key.slice('media:'.length)}`, url: '' }); continue; }
    const resource = { ...media.resource!, threadId }, id = `md:${client.environmentId}|${threadId}|${key}`;
    // InlineAudioPreview signs a sound once, as a video is.
    const entry = await mint(client, native, id, resource, now, scope.mutationId !== undefined && media.kind === 'image' && refreshDue(client, id, scope.mutationId));
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
  // A Markdown sound's name opens its file beside the thread (InlineAudioPreview's onOpen); a web URL's opens nothing.
  if (op === 'open-file') { if (id) await hooks.openFile?.(id); return ''; }
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
