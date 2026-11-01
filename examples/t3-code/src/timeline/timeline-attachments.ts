import { toolImageAction, toolImagePreview, toolImageUrls } from './tool-images';
import { contextLinks, contextTarget, replaceLinks } from '../composer/context-links';
import { contextLink } from '../composer/composer-editor-menu';
// Sent attachments in user messages, adapted from T3 Code (MIT, see
// LICENSE-T3): MessagesTimeline.tsx UserTimelineRow (a two-column image grid
// at most 210pt wide, a SnapShot spanning both columns in a 208×112 frame with
// its app and window title, then the file list), assetUrls.ts (signed
// `assets.createUrl` URLs for `attachment` resources) and ExpandedImageDialog
// (one image at a time with its name and position, previous and next).
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { fileIconToken } from '../shared/timeline-files';
import { assetUrl } from '../settings/settings-b-icons';
import { videoMimeType } from '../composer/composer-video'; // sent videos (UserVideoAttachment)
import { markdownDocument, markdownGallery, markdownMedia, markdownMediaUrls, type GalleryImage, type MediaScope } from '../media/media-views'; // the transcript's host-path media
import { letGo } from '../client/let-go';
import { encodeMediaSource, mediaTooltip } from '../media/media-actions';
import { syncInlineDocuments } from './inline-documents';
import { draftThreadId } from '../composer/draft-thread-id';
import { MEDIA_CACHE } from '../media/media-frame';

export interface MessageImage { id: string; name: string; snapshot: boolean; appName: string; appInitial: string; windowTitle: string; accessible: boolean; video: boolean }
/** A sent file: `label` its name ("Pasted text" for a paste), `detail` its card's line ("TXT · 1.20 KB", a paste's size
 *  alone), `note` its list row's line (a paste's size), and `sending` while the server has not stored it ("Sending…"). */
export interface MessageFile { id: string; name: string; icon: string; label: string; detail: string; note: string; sending: boolean }

/** lib/formatBytes.ts (v0.1.6): the unit picked after rounding, so 1023.6 KB reads "1.00 MB". */
export function formatBytes(value: number): string {
  const scaled = (next: number) => next.toFixed(next >= 100 ? 0 : next >= 10 ? 1 : 2);
  if (Math.round(value) < 1_024) return `${Math.round(value)} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let next = value / 1_024, unit = 0;
  while (Number(scaled(next)) >= 1_024 && unit < units.length - 1) { next /= 1_024; unit += 1; }
  return `${scaled(next)} ${units[unit]}`;
}
/** userMessageFileLabel: a paste reads "Pasted text", any other file its name. */
export const userMessageFileLabel = (attachment: Obj): string => obj(attachment.source)._tag === 'pasted-text' ? 'Pasted text' : str(attachment.name);
function messageFile(attachment: Obj): MessageFile {
  const name = str(attachment.name), pasted = obj(attachment.source)._tag === 'pasted-text', sending = attachment.downloadable === false;
  const size = formatBytes(Number(attachment.sizeBytes) || 0), extension = /\.([a-z0-9]{1,8})$/i.exec(name)?.[1]?.toUpperCase() ?? 'File';
  return { id: str(attachment.id), name, icon: fileIconToken(name), label: userMessageFileLabel(attachment),
    detail: sending ? 'Sending…' : pasted ? size : `${extension} · ${size}`, note: sending ? 'Sending…' : pasted ? size : '', sending };
}

/** A user message's images (annotation previews stay out, as the reference filters them), audio and files.
 *  A sent video is a grid tile after the images (UserVideoAttachment), a downloadable sound an audio chip
 *  (InlineAudioPreview), never a file row. */
export function messageAttachments(item: Obj): { images: MessageImage[]; attachFiles: MessageFile[]; audio: MessageFile[] } {
  const images: MessageImage[] = [], attachFiles: MessageFile[] = [], videos: MessageImage[] = [], audio: MessageFile[] = [];
  for (const attachment of arr(item.attachments)) {
    const id = str(attachment.id), name = str(attachment.name);
    if (!id) continue;
    if (attachment.type === 'image') {
      if (name.startsWith('preview-annotation-')) continue;
      const source = obj(attachment.source), snapshot = source.kind === 'snap-shot';
      const appName = snapshot ? str(source.appName) : '';
      images.push({ id, name, snapshot, appName, appInitial: appName.slice(0, 1).toUpperCase(), windowTitle: snapshot ? str(source.windowTitle) || 'Captured window' : '',
        accessible: snapshot && (!!source.accessibility || !!str(source.accessibleText).trim()), video: false });
    } else if (attachment.type === 'file' && isSentVideo(attachment)) videos.push({ id, name, snapshot: false, appName: '', appInitial: '', windowTitle: '', accessible: false, video: true });
    else if (attachment.type === 'file' && isSentAudio(attachment)) audio.push(messageFile(attachment));
    else if (attachment.type === 'file') attachFiles.push(messageFile(attachment));
  }
  return { images: [...images, ...videos], attachFiles, audio };
}

/**
 * How TimelineUserMessageRow shapes a sent message that is not folded (v0.1.6): images or videos sent without words show
 * as media with no bubble (`media`), and sent with words sit above the bubble (`mediaAbove`); files or a voice note sent
 * without words show as file cards (`files`) or a bubble-shaped audio chip (`audio`). `layout` is the standalone media's:
 * a lone image keeps its own shape (`lone`), several share one rounded collage (`collage`), and videos or SnapShots
 * among them sit in a 20rem stack (`stack`). `files` is the files to draw: every file for cards, else those the prose
 * has no chip for. `copy` is what Copy takes from a wordless media message: its media's references.
 */
export function userMessageShape(item: Obj, text: string, attached: ReturnType<typeof messageAttachments>, chipped: ReadonlySet<string>, folded: boolean) {
  const images = attached.images.filter(image => !image.video), videos = attached.images.filter(image => image.video);
  const unchipped = attached.attachFiles.filter(file => !chipped.has(file.id));
  const unknown = arr(item.attachments).filter(attachment => attachment.type !== 'image' && attachment.type !== 'file').length;
  const records = new Map(arr(obj(item.context).records).map(record => [str(record.contextId), record]));
  // The prose with the references to `ids` dropped and every other reference read as its label.
  const wordsBesides = (ids: ReadonlySet<string>) => replaceLinks(text, contextLinks(text), link => {
    const target = contextTarget(link.href), record = target ? records.get(target.id) : undefined;
    return record && (str(record.kind) === 'file' || str(record.kind) === 'image') && ids.has(str(record.attachmentId)) ? '' : link.label;
  }).trim();
  const media = images.length + videos.length;
  const mediaOnly = !folded && text.trim() === '' && attached.audio.length === 0 && unchipped.length === 0 && unknown === 0 && media > 0;
  const mediaAbove = !folded && !mediaOnly && media > 0;
  const fileCards = !folded && attached.attachFiles.length > 0 && media + attached.audio.length + unknown === 0
    && wordsBesides(new Set(attached.attachFiles.map(file => file.id))) === '';
  const audioOnly = !folded && attached.audio.length > 0 && media + attached.attachFiles.length + unknown === 0
    && wordsBesides(new Set(attached.audio.map(file => file.id))) === '';
  const standalone = mediaOnly || mediaAbove;
  const layout = !standalone ? '' : videos.length === 0 && images.length === 1 ? 'lone'
    : videos.length === 0 && images.length > 1 && images.every(image => !image.snapshot) ? 'collage' : 'stack';
  const mediaIds = new Set(attached.images.map(image => image.id));
  const copy = mediaOnly ? arr(obj(item.context).records)
    .filter(record => (str(record.kind) === 'image' || str(record.kind) === 'file') && mediaIds.has(str(record.attachmentId)))
    .map(record => contextLink(str(record.kind), str(record.contextId), str(record.label))).join('\n') : '';
  return { shape: mediaOnly ? 'media' : fileCards ? 'files' : audioOnly ? 'audio' : '', mediaAbove, layout, files: fileCards ? attached.attachFiles : unchipped, copy };
}

const isSentVideo = (attachment: Obj) => videoMimeType({ name: str(attachment.name), mimeType: str(attachment.mimeType) }) !== null;
/**
 * filePreviewKind "audio" (shared/filePreview.ts): an audio MIME type, or a sound's extension under a generic one (none,
 * application/octet-stream or text/plain), the MIME type read without its parameters.
 */
export function isSentAudio(attachment: Obj): boolean {
  const mime = str(attachment.mimeType).split(';', 1)[0]!.trim().toLowerCase();
  const generic = !mime || mime === 'application/octet-stream' || mime === 'text/plain';
  return attachment.downloadable !== false && (mime.startsWith('audio/') || generic && /\.(mp3|wav|ogg|oga|flac|aac|m4a|opus|aiff|weba)$/i.test(str(attachment.name)));
}
const GENERIC_MIME_TYPES = new Set(['application/octet-stream', 'binary/octet-stream', 'application/unknown']);
/** shared/image.ts isSvgAttachment: an SVG MIME type, or a `.svg` name under a generic one. */
export function isSvgAttachment(attachment: Obj): boolean {
  const mime = str(attachment.mimeType).split(';', 1)[0]!.trim().toLowerCase();
  return mime === 'image/svg+xml' || (!mime || GENERIC_MIME_TYPES.has(mime)) && /\.svg$/i.test(str(attachment.name));
}
/** Whether `id` is a sent SVG file of the shown thread, which its chip opens as a file, not in the image dialog. */
export function isSentSvg(client: T3Client, id: string): boolean {
  return !!id && arr(client.projection.visibleTurnItems).some(row => obj(row.item).type === 'user_message'
    && arr(obj(row.item).attachments).some(attachment => attachment.type === 'file' && str(attachment.id) === id && isSvgAttachment(attachment)));
}
// SvgFileChip's failedPreviewUrl: per sent SVG (keyed as `urls` is), the signed URL whose picture did not load.
const failedSvgs = new WeakMap<T3Client, Map<string, string>>();
const svgKey = (client: T3Client, id: string) => JSON.stringify([client.generation, client.environmentId, id]);
/** A chip's thumbnail at `url` did not load. */
export function noteChipPreviewFailed(client: T3Client, id: string, url: string): void {
  if (!url || !isSentSvg(client, id)) return;
  const failed = failedSvgs.get(client) ?? new Map<string, string>();
  failed.set(svgKey(client, id), url); failedSvgs.set(client, failed);
}
/** Whether a sent SVG's chip is its file chip: its picture failed at the URL it still has (a URL signed again is tried). */
export function svgPreviewFailed(client: T3Client, id: string): boolean {
  const key = svgKey(client, id), failed = failedSvgs.get(client)?.get(key);
  return failed !== undefined && urls.get(key)?.url === failed;
}
/**
 * A sent sound's reference leaves the prompt: TimelineUserMessageRow's renderContextReference returns null for a file
 * reference whose attachment plays in its audio chip, so only the text around it draws.
 */
export function withoutSoundReferences(text: string, item: Obj): string {
  const sounds = new Set(arr(item.attachments).filter(attachment => attachment.type === 'file' && isSentAudio(attachment)).map(attachment => str(attachment.id)));
  if (!sounds.size || !text.includes('](t3-context://v1/file/')) return text;
  const records = new Map(arr(obj(item.context).records).map(record => [str(record.contextId), record]));
  return replaceLinks(text, contextLinks(text), link => {
    const target = contextTarget(link.href), record = target?.kind === 'file' ? records.get(target.id) : undefined;
    return record && str(record.kind) === 'file' && sounds.has(str(record.attachmentId)) ? '' : null;
  });
}
const urls = new Map<string, { url: string; expiresAt: number }>();
/** Signed preview URLs for the selected thread's sent images (useAssetUrls), cached until shortly before they expire. */
type AttachmentUrl = { id: string; url: string };
export async function attachmentUrls(client: T3Client, native: Native | null | undefined, now: number, systemScheme?: string): Promise<{ items: AttachmentUrl[] }> {
  if (!native?.available || client.connection !== 'connected') return { items: [] };
  await syncInlineDocuments(client, native, now, systemScheme);
  if (!client.threadId) return { items: await galleryUrls(client, native, now, new Set()) }; // a draft's Files preview
  const wanted: Obj[] = [];
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item);
    if (item.type !== 'user_message') continue;
    for (const attachment of arr(item.attachments)) if (attachment.type === 'image' && str(attachment.id) && !str(attachment.name).startsWith('preview-annotation-')) wanted.push(attachment);
    // buildAttachmentVideoAsset signs a sent video under its video MIME type.
    for (const attachment of arr(item.attachments)) if (attachment.type === 'file' && str(attachment.id) && isSentVideo(attachment)) wanted.push({ ...attachment, mimeType: videoMimeType({ name: str(attachment.name), mimeType: str(attachment.mimeType) }) });
    // SvgFileChip: a sent SVG's chip shows the picture, signed as one (a file the server does not hand out has none).
    for (const attachment of arr(item.attachments)) if (attachment.type === 'file' && str(attachment.id) && attachment.downloadable !== false && isSvgAttachment(attachment)) wanted.push({ ...attachment, mimeType: 'image/svg+xml' });
    // InlineAudioPreview: a sent sound plays from its inline attachment resource.
    for (const attachment of arr(item.attachments)) if (attachment.type === 'file' && str(attachment.id) && !isSentVideo(attachment) && isSentAudio(attachment)) wanted.push(attachment);
  }
  // Queued messages' images (the queued rows' thumbnails and the edit's kept attachments).
  const queuedMessages = new Set(arr(client.projection.runs).filter(run => run.status === 'queued').map(run => str(run.userMessageId)));
  for (const message of arr(client.projection.messages)) if (queuedMessages.has(str(message.id))) for (const attachment of arr(message.attachments)) if (attachment.type === 'image' && str(attachment.id)) wanted.push(attachment);
  const items: { id: string; url: string }[] = [], failed: { id: string; url: string }[] = [];
  for (const attachment of wanted.slice(-64)) {
    const id = str(attachment.id), key = JSON.stringify([client.generation, client.environmentId, id]);
    const cached = urls.get(key);
    if (cached && (!now || cached.expiresAt - 60_000 > now)) { items.push({ id, url: cached.url }); continue; }
    try {
      const result = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: id,
        ...(str(attachment.name) ? { fileName: str(attachment.name) } : {}), ...(str(attachment.mimeType) ? { mimeType: str(attachment.mimeType) } : {}), disposition: 'inline' } }));
      const url = assetUrl(client.origin, str(result.relativeUrl));
      if (!url) continue;
      urls.set(key, { url, expiresAt: Number(result.expiresAt) || 0 });
      if (urls.size > 256) urls.delete(urls.keys().next().value!);
      items.push({ id, url });
    } catch { failed.push({ id: `failed:${id}`, url: '' }); /* An unavailable preview shows the image's name instead; the dialog says it is unavailable. */ }
  }
  // The visible messages' Markdown media on host paths (`media:<path>`, `media-failed:<path>`).
  const thread = obj(client.projection.thread), project = (client.shell?.projects ?? []).find(entry => entry.id === (thread.projectId ?? client.projectId));
  const root = str(thread.worktreePath) || str(project?.workspaceRoot);
  const media: AttachmentUrl[] = await markdownMediaUrls(client, native, root, now);
  // The linked media's signed URL (signed again after Retry video; a refusal is `failed:<key>`).
  const preview = activeMediaPreview(client);
  if (preview && !preview.url) {
    try { preview.url = await signMediaFile(client, native, preview.path); } catch (error) { if (letGo(error)) throw error; }
  }
  if (preview) media.push(preview.url ? { id: preview.key, url: preview.url } : { id: `failed:${preview.key}`, url: '' });
  media.push(...await galleryUrls(client, native, now, new Set(media.map(item => item.id))));
  return { items: [...items, ...failed, ...media, ...await toolImageUrls(client, native, now)] };
}
/** The open Markdown gallery's URLs the transcript's do not hold: a web image as written, a host path signed through the
 *  thread (a draft's own in Files) with the document's folder and workspace, as its view signed it. */
async function galleryUrls(client: T3Client, native: Native, now: number, known: Set<string>): Promise<AttachmentUrl[]> {
  const gallery = activeGallery(client), out: AttachmentUrl[] = [];
  if (!gallery) return out;
  const add = (entry: { id: string; url: string }) => { if (!known.has(entry.id)) { known.add(entry.id); out.push(entry); } };
  for (const item of gallery.items) if (item.access === 'direct') add({ id: item.key, url: item.key });
  if (gallery.signThread) for (const entry of await markdownMediaUrls(client, native, gallery.root, now, [gallery.text], { ...gallery.scope, threadId: gallery.signThread })) add(entry);
  return out;
}

/** The URL a sent attachment shows now (its Save and Copy image source), or null. */
export function cachedAttachmentUrl(client: T3Client, id: string): string | null {
  return urls.get(JSON.stringify([client.generation, client.environmentId, id]))?.url ?? null;
}
/** Retry video signs the attachment again (useAssetUrlRefresh). */
export function forgetAttachmentUrl(client: T3Client, id: string): void { urls.delete(JSON.stringify([client.generation, client.environmentId, id])); }

interface Preview { threadId: string; messageId: string; imageId: string }
const previews = new WeakMap<T3Client, Preview | null>();
/** A Markdown image's expanded preview (ChatMarkdown expandableMarkdownImageProps and
 *  markdownImageGallery): its document's images in order, the one shown, and the thread its host paths sign through. */
type Gallery = { threadId: string; signThread: string; text: string; root: string; scope: MediaScope; items: GalleryImage[]; index: number; opener: string };
const galleries = new WeakMap<T3Client, Gallery | null>();
const activeGallery = (client: T3Client): Gallery | null => {
  const gallery = galleries.get(client);
  if (gallery && gallery.threadId !== client.threadId) { galleries.set(client, null); return null; }
  return gallery ?? null;
};
function messageImages(client: T3Client, messageId: string, selected = ''): MessageImage[] {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === messageId);
  const all = messageAttachments(obj(row?.item)).images, video = all.find(image => image.id === selected && image.video);
  // buildAttachmentVideoPreview opens a video alone; buildExpandedImagePreview steps through the images.
  return video ? [video] : all.filter(image => !image.video);
}
/** An inline HTML render's image opens the same expanded-image dialog as Markdown. */
export function openInlineDocumentImage(client: T3Client, src: string, name: string, opener: string): void {
  if (!/^(?:https?:\/\/|data:image\/)/i.test(src)) throw new ClientError('That image URL is not supported.');
  const source = { kind: 'image' as const, name, src };
  previews.set(client, null); mediaPreviews.set(client, null);
  galleries.set(client, { threadId: client.threadId, signThread: '', text: '', root: '', scope: {}, items: [{ key: src, name, access: 'direct', source }], index: 0, opener });
}
/**
 * An archive entry's picture, written to the media cache: the dialog shows that file. Its Save and Copy image stay
 * disabled, as the native media actions fetch only http(s) and data: URLs.
 */
export function openCachedImage(client: T3Client, path: string, name: string, opener: string): void {
  if (!path.startsWith(`${MEDIA_CACHE}/`)) throw new ClientError('That image is no longer available.');
  previews.set(client, null); mediaPreviews.set(client, null);
  galleries.set(client, { threadId: client.threadId, signThread: '', text: '', root: '', scope: {}, items: [{ key: path, name, access: 'direct', source: { kind: 'image', name, src: null } }], index: 0, opener });
}
/** ExpandedImageDialog: open on an image, step to its neighbours, close. */
export function imagePreviewAction(client: T3Client, op: string, id: string, value: string): string {
  if (toolImageAction(client, op, id, value)) { previews.set(client, null); galleries.set(client, null); mediaPreviews.set(client, null); return ''; }
  const current = previews.get(client);
  if (op === 'image-close') { previews.set(client, null); mediaPreviews.set(client, null); galleries.set(client, null); return ''; }
  if (op === 'image-open') {
    if (!messageImages(client, value, id).some(image => image.id === id)) throw new ClientError('That image is no longer available.');
    previews.set(client, { threadId: client.threadId, messageId: value, imageId: id });
    galleries.set(client, null);
    return '';
  }
  // The key identifies the source; the parsed document's occurrence selects this image even when a URL repeats.
  if (op === 'image-md-open') {
    const [owner = '', opener = '', occurrence = ''] = value.split('\n'), index = Number(occurrence);
    const document = markdownDocument(owner), items = document ? markdownGallery(document) : [];
    if (!document || !opener || !occurrence || !Number.isInteger(index) || items[index]?.key !== id) throw new ClientError('That image is no longer available.');
    previews.set(client, null); mediaPreviews.set(client, null);
    galleries.set(client, { threadId: client.threadId, signThread: client.threadId || draftThreadId(client), text: document.text, root: document.root, scope: document.scope, items, index,
      opener });
    return '';
  }
  const gallery = activeGallery(client);
  if (op === 'image-step' && gallery) {
    // ExpandedImagePreview wraps around in both directions.
    gallery.index = (((gallery.index + (value === 'previous' ? -1 : 1)) % gallery.items.length) + gallery.items.length) % gallery.items.length;
    return '';
  }
  if (op === 'image-step' && current) {
    const images = messageImages(client, current.messageId, current.imageId), index = images.findIndex(image => image.id === current.imageId);
    // ExpandedImageDialog wraps around in both directions.
    const next = images.length ? images[(((index + (value === 'previous' ? -1 : 1)) % images.length) + images.length) % images.length] : undefined;
    if (next) current.imageId = next.id;
    return '';
  }
  throw new ClientError(`Unknown image action: ${op}`);
}
// ChatMarkdown's "Preview media" on a file link (resolveMarkdownMediaPreview): the
// media is signed on its environment as a `media-file` of the shown thread first (a failure is the caller's
// "Media unavailable" toast and no dialog), then the same dialog shows it alone (markdownImageGallery finds
// no inline image for a link). Leaving the thread drops it, as the reference resets its preview.
type MediaPreview = { threadId: string; path: string; key: string; name: string; video: boolean; root: string; url: string };
const mediaPreviews = new WeakMap<T3Client, MediaPreview | null>();
async function signMediaFile(client: T3Client, native: Native, path: string): Promise<string> {
  if (!native.available || client.connection !== 'connected' || !client.threadId) throw new ClientError('Reconnect to this environment and open the media again.');
  const result = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'media-file', path, threadId: client.threadId } }));
  const url = assetUrl(client.origin, str(result.relativeUrl));
  if (!url) throw new ClientError('The environment returned an invalid media URL.');
  return url;
}
export async function openMarkdownMediaPreview(client: T3Client, native: Native, filePath: string, root: string): Promise<void> {
  const path = filePath.replace(/:\d+(?::\d+)?$/, '');
  const media = markdownMedia('', path, '', root);
  if (media.access !== 'environment' || !media.source) throw new ClientError('Reconnect to this environment and open the media again.');
  const threadId = client.threadId, url = await signMediaFile(client, native, path);
  if (client.threadId !== threadId) return;
  previews.set(client, null);
  galleries.set(client, null);
  mediaPreviews.set(client, { threadId, path, key: media.key, name: path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1), video: media.kind === 'video', root, url });
}
const activeMediaPreview = (client: T3Client): MediaPreview | null => {
  const media = mediaPreviews.get(client);
  if (media && media.threadId !== client.threadId) { mediaPreviews.set(client, null); return null; }
  return media ?? null;
};
/** Retry video on the linked media: sign it again (useAssetUrlRefresh). */
export function forgetMediaPreviewUrl(client: T3Client, key: string): boolean {
  const media = activeMediaPreview(client);
  if (!media || media.key !== key) return false;
  media.url = '';
  return true;
}
export function imagePreviewView(client: T3Client) {
  const tool = toolImagePreview(client);
  if (tool) return tool;
  const media = activeMediaPreview(client);
  if (media) {
    const source = markdownMedia(media.name, media.path, '', media.root).source;
    return { imagePreviewId: media.key, imagePreviewName: media.name, imagePreviewPosition: '', imagePreviewPrevious: false, imagePreviewNext: false,
      imagePreviewVideo: media.video, imagePreviewSource: source ? encodeMediaSource({ ...source, name: media.name }) : '', imagePreviewTip: source ? mediaTooltip({ ...source, name: media.name }) : media.name, imagePreviewReturn: '' };
  }
  const gallery = activeGallery(client);
  if (gallery) {
    const item = gallery.items[gallery.index]!, many = gallery.items.length > 1;
    return { imagePreviewId: item.key, imagePreviewName: item.name, imagePreviewPosition: many ? `(${gallery.index + 1}/${gallery.items.length})` : '', imagePreviewPrevious: many,
      imagePreviewNext: many, imagePreviewVideo: false, imagePreviewSource: encodeMediaSource(item.source), imagePreviewTip: mediaTooltip(item.source),
      imagePreviewReturn: gallery.opener };
  }
  const current = previews.get(client);
  const images = current && current.threadId === client.threadId ? messageImages(client, current.messageId, current.imageId) : [];
  const index = current ? images.findIndex(image => image.id === current.imageId) : -1;
  const image = index >= 0 ? images[index]! : null;
  return { imagePreviewId: image?.id ?? '', imagePreviewName: image?.name ?? '', imagePreviewPosition: image && images.length > 1 ? `(${index + 1}/${images.length})` : '',
    imagePreviewPrevious: !!image && images.length > 1, imagePreviewNext: !!image && images.length > 1, imagePreviewVideo: !!image?.video, imagePreviewSource: image ? `attachment:${image.id}` : '',
    imagePreviewTip: image?.name ?? '', imagePreviewReturn: '' };
}

