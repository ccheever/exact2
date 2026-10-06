// Sent attachments in user messages, adapted from T3 Code (MIT, see
// LICENSE-T3): MessagesTimeline.tsx UserTimelineRow (a two-column image grid
// at most 210pt wide, a SnapShot spanning both columns in a 208×112 frame with
// its app and window title, then the file list), assetUrls.ts (signed
// `assets.createUrl` URLs for `attachment` resources) and ExpandedImageDialog
// (one image at a time with its name and position, previous and next).
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { fileIconToken } from './timeline-files';
import { assetUrl } from './settings-b-icons';
import { imageChipInks } from './r4-timeline-chips';
import { videoMimeType } from './r4-composer-video'; // lane r6-media: sent videos (UserVideoAttachment)
import { markdownMediaUrls } from './media-views'; // media-actions: the transcript's host-path media

export interface MessageImage { id: string; name: string; snapshot: boolean; appName: string; appInitial: string; windowTitle: string; accessible: boolean; video: boolean }
export interface MessageFile { id: string; name: string; icon: string }

/** A user message's images (annotation previews stay out, as the reference filters them) and files.
 *  lane r6-media: a sent video is a grid tile after the images (UserVideoAttachment), never a file row. */
export function messageAttachments(item: Obj): { images: MessageImage[]; attachFiles: MessageFile[] } {
  const images: MessageImage[] = [], attachFiles: MessageFile[] = [], videos: MessageImage[] = [];
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
    else if (attachment.type === 'file') attachFiles.push({ id, name, icon: fileIconToken(name) });
  }
  return { images: [...images, ...videos], attachFiles };
}

const isSentVideo = (attachment: Obj) => videoMimeType({ name: str(attachment.name), mimeType: str(attachment.mimeType) }) !== null;
const urls = new Map<string, { url: string; expiresAt: number }>();
/** Signed preview URLs for the selected thread's sent images (useAssetUrls), cached until shortly before they expire. */
type AttachmentUrl = { id: string; url: string; fill: string; hover: string; border: string; ink: string };
export async function attachmentUrls(client: T3Client, native: Native | null | undefined, now: number): Promise<{ items: AttachmentUrl[] }> {
  if (!native?.available || client.connection !== 'connected' || !client.threadId) return { items: [] };
  const wanted: Obj[] = [];
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item);
    if (item.type !== 'user_message') continue;
    for (const attachment of arr(item.attachments)) if (attachment.type === 'image' && str(attachment.id) && !str(attachment.name).startsWith('preview-annotation-')) wanted.push(attachment);
    // lane r6-media: buildAttachmentVideoAsset signs a sent video under its video MIME type.
    for (const attachment of arr(item.attachments)) if (attachment.type === 'file' && str(attachment.id) && isSentVideo(attachment)) wanted.push({ ...attachment, mimeType: videoMimeType({ name: str(attachment.name), mimeType: str(attachment.mimeType) }) });
  }
  // composer-fidelity G12a: queued messages' images (the queued rows' thumbnails and the edit's kept attachments).
  const queuedMessages = new Set(arr(client.projection.runs).filter(run => run.status === 'queued').map(run => str(run.userMessageId)));
  for (const message of arr(client.projection.messages)) if (queuedMessages.has(str(message.id))) for (const attachment of arr(message.attachments)) if (attachment.type === 'image' && str(attachment.id)) wanted.push(attachment);
  const items: { id: string; url: string }[] = [], failed: { id: string; url: string }[] = [], videoIds = new Set(wanted.filter(entry => entry.type === 'file').map(entry => str(entry.id)));
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
  // media-actions: the visible messages' Markdown media on host paths (`media:<path>`, `media-failed:<path>`).
  const thread = obj(client.projection.thread), project = (client.shell?.projects ?? []).find(entry => entry.id === (thread.projectId ?? client.projectId));
  const root = str(thread.worktreePath) || str(project?.workspaceRoot);
  const media = (await markdownMediaUrls(client, native, root, now)).map(item => ({ ...item, ...NO_INKS }));
  return { items: [...await withAccents(client, native, items, videoIds), ...failed.map(item => ({ ...item, ...NO_INKS })), ...media] };
}

/** media-actions: the URL a sent attachment shows now (its Save and Copy image source), or null. */
export function cachedAttachmentUrl(client: T3Client, id: string): string | null {
  return urls.get(JSON.stringify([client.generation, client.environmentId, id]))?.url ?? null;
}
/** media-actions: Retry video signs the attachment again (useAssetUrlRefresh). */
export function forgetAttachmentUrl(client: T3Client, id: string): void { urls.delete(JSON.stringify([client.generation, client.environmentId, id])); }

interface Preview { threadId: string; messageId: string; imageId: string }
const previews = new WeakMap<T3Client, Preview | null>();
function messageImages(client: T3Client, messageId: string, selected = ''): MessageImage[] {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === messageId);
  const all = messageAttachments(obj(row?.item)).images, video = all.find(image => image.id === selected && image.video);
  // lane r6-media: buildAttachmentVideoPreview opens a video alone; buildExpandedImagePreview steps through the images.
  return video ? [video] : all.filter(image => !image.video);
}
/** ExpandedImageDialog: open on an image, step to its neighbours, close. */
export function imagePreviewAction(client: T3Client, op: string, id: string, value: string): string {
  const current = previews.get(client);
  if (op === 'image-close') { previews.set(client, null); return ''; }
  if (op === 'image-open') {
    if (!messageImages(client, value, id).some(image => image.id === id)) throw new ClientError('That image is no longer available.');
    previews.set(client, { threadId: client.threadId, messageId: value, imageId: id });
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
export function imagePreviewView(client: T3Client) {
  const current = previews.get(client);
  const images = current && current.threadId === client.threadId ? messageImages(client, current.messageId, current.imageId) : [];
  const index = current ? images.findIndex(image => image.id === current.imageId) : -1;
  const image = index >= 0 ? images[index]! : null;
  return { imagePreviewId: image?.id ?? '', imagePreviewName: image?.name ?? '', imagePreviewPosition: image && images.length > 1 ? `(${index + 1}/${images.length})` : '',
    imagePreviewPrevious: !!image && images.length > 1, imagePreviewNext: !!image && images.length > 1, imagePreviewVideo: !!image?.video };
}

// ImageChipButton tints its chip with the picture's average colour (lane r4-timeline).
const accents = new Map<string, { fill: string; hover: string; border: string; ink: string }>();
const NO_INKS = { fill: '', hover: '', border: '', ink: '' };
async function withAccents(client: T3Client, native: Native, items: { id: string; url: string }[], videoIds = new Set<string>()): Promise<AttachmentUrl[]> {
  const out: AttachmentUrl[] = [];
  for (const item of items) {
    if (videoIds.has(item.id)) { out.push({ ...item, ...NO_INKS }); continue; } // a video has no image chip to tint
    const key = JSON.stringify([client.environmentId, item.id]);
    if (!accents.has(key)) {
      const reply = await client.restAccess(native).call({ op: 'imageAccent', url: item.url, key }).catch(() => ({}));
      accents.set(key, imageChipInks(str(obj(reply).accent)) ?? NO_INKS);
      if (accents.size > 256) accents.delete(accents.keys().next().value!);
    }
    out.push({ ...item, ...accents.get(key)! });
  }
  return out;
}
