// One attachment's staging, adapted from T3 Code (MIT; see LICENSE-T3): chat/composerAttachmentFiles.ts at 7c3b3b3605ef
// (classifyComposerAttachmentFile, composerFileAttachmentMimeType, inferImageMimeTypeFromName), packages/shared
// mediaPreview.ts (isWorkspaceImagePreviewPath), and at 9c161462c9 lib/imageCompression.ts (prepareImageForAttachment,
// compressImageToByteLimit, MAX_COMPRESSIBLE_SOURCE_BYTES) and packages/contracts chatAttachment.ts
// (PROVIDER_SEND_TURN_MAX_IMAGE_BYTES). A file arrives as a `doc:` handle (a drop or a paste) or as a picked
// `app:/tmp/picked/` copy (Attach files). An image becomes a draft beside SnapShot's, which the shelf shows and the send
// uploads (client.ts uploadSnapshots): a PNG, JPEG, GIF or WebP within its limits keeps its bytes and type; a PNG or
// JPEG over 2000 px is fitted in its own type; one over 10 MB, and a HEIC, is encoded by `compressImage`, as a PNG
// when it can hold transparency and else as a JPEG (the reference's WebP has no encoder here). A plain file within the
// server's limit is copied into `composer-files/` for its chip, a video or audio file under its typed name
// (composer-staged-copies.ts stagedName), and a video's display size is read from its header (videoFrameSize).
import type { Obj } from '../client/domain';
import { appFiles } from '../media/save-file';
import { uuid } from '../transport/transport-wire';
import { HOST_READ_BYTES, MAX_FILE_BYTES, isSvgAttachment } from './composer-editor-files';
import { videoFrameSize, videoMimeType } from './composer-video';
import { IMAGE_ATTACHMENT_MAX_EDGE } from './image-attachment-size';
import { DRAFTS, STAGED, stagedName } from './composer-staged-copies';
import { imageHeader } from './composer-image-header';

/** One file to attach: its path, its name, and the MIME type its source recorded ("" when none did). */
export type AttachSource = { path: string; name: string; type: string };

export const PROVIDER_SEND_TURN_MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const MAX_COMPRESSIBLE_SOURCE_BYTES = 50 * 1024 * 1024;
export { DRAFTS, STAGED } from './composer-staged-copies';
const SCRATCH = 'app:/tmp/composer-stage';

/** The app's storage operations staging uses (storage.d.ts). */
interface StageFiles {
  readFile(path: string): Promise<ArrayBuffer>;
  writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  rm(path: string): Promise<unknown>;
  stat(path: string): Promise<{ size: number; isFile: boolean; isDirectory: boolean }>;
  copyFile(from: string, to: string): Promise<unknown>;
  compressImage(from: string, to: string, options: { maxDimension: number; maxBytes: number; type: 'image/png' | 'image/jpeg' }): Promise<{ size: number }>;
}
const files = (): StageFiles => (appFiles() as unknown as { fs: StageFiles }).fs;

/** The MIME type macOS names for an extension (UTType preferredMIMEType), as the picker reports it; "" for others. */
const MIME_BY_EXTENSION: Record<string, string> = {
  txt: 'text/plain', text: 'text/plain', md: 'text/markdown', markdown: 'text/markdown', csv: 'text/csv', tsv: 'text/tab-separated-values',
  json: 'application/json', yaml: 'application/x-yaml', yml: 'application/x-yaml', xml: 'application/xml', html: 'text/html', htm: 'text/html',
  css: 'text/css', js: 'text/javascript', mjs: 'text/javascript', py: 'text/x-python-script', rb: 'text/x-ruby-script', pl: 'text/x-perl-script',
  php: 'text/php', rtf: 'text/rtf', ics: 'text/calendar', vcf: 'text/vcard', pdf: 'application/pdf', zip: 'application/zip',
  gz: 'application/x-gzip', tar: 'application/x-tar', epub: 'application/epub+zip', dmg: 'application/x-apple-diskimage',
  doc: 'application/msword', docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', xls: 'application/vnd.ms-excel',
  xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', ppt: 'application/vnd.ms-powerpoint',
  pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp', heic: 'image/heic', heif: 'image/heif',
  tif: 'image/tiff', tiff: 'image/tiff', bmp: 'image/bmp', svg: 'image/svg+xml', ico: 'image/vnd.microsoft.icon', avif: 'image/avif', jxl: 'image/jxl',
  psd: 'image/vnd.adobe.photoshop', mp4: 'video/mp4', m4v: 'video/x-m4v', mov: 'video/quicktime', webm: 'video/webm', avi: 'video/avi', ogv: 'video/ogg',
  mp3: 'audio/mpeg', m4a: 'audio/x-m4a', wav: 'audio/vnd.wave', aac: 'audio/aac', flac: 'audio/flac', ogg: 'audio/ogg', oga: 'audio/ogg',
  opus: 'audio/ogg', aif: 'audio/aiff', aiff: 'audio/aiff',
};
const IMAGE_MIME_BY_EXTENSION: Record<string, string> = { gif: 'image/gif', jpeg: 'image/jpeg', jpg: 'image/jpeg', png: 'image/png', webp: 'image/webp' };
const SUPPORTED_IMAGE_TYPES = new Set(['image/gif', 'image/jpeg', 'image/png', 'image/webp']);
const extension = (name: string): string => { const dot = name.lastIndexOf('.'); return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase(); };
const generic = (type: string) => type === '' || type.toLowerCase() === 'application/octet-stream';

/** A dropped or pasted file's MIME type, which nothing recorded: its extension's. */
export function mimeTypeOfName(name: string): string { return MIME_BY_EXTENSION[extension(name)] ?? ''; }

/** isHeicImageFile. */
export function isHeic(name: string, type: string): boolean {
  return /^image\/hei(?:c|f)$/i.test(type) || (generic(type) && /\.(?:heic|heif)$/i.test(name));
}

const baseType = (type: string) => type.split(';', 1)[0]!.trim().toLowerCase();
/** A camera's raw DNG: an image by type, though no provider takes it as one. */
const isDng = (name: string, type: string) => name.toLowerCase().endsWith('.dng') || ['image/dng', 'image/x-dng', 'image/x-adobe-dng'].includes(baseType(type));
/** WORKSPACE_IMAGE_PREVIEW_EXTENSIONS: the image types the file viewer shows. */
const PREVIEWABLE_IMAGE_EXTENSIONS = ['avif', 'bmp', 'gif', 'ico', 'heic', 'heif', 'jpeg', 'jpg', 'png', 'svg', 'tif', 'tiff', 'webp'];

/**
 * classifyComposerAttachmentFile: a supported image, an image type the provider cannot take, or a plain file. An SVG,
 * a DNG, and an image the viewer can show but providers refuse (AVIF, BMP, ICO, TIFF) attach as files.
 */
export function classifyAttachment(name: string, type: string): 'image' | 'file' | 'unsupported-image' {
  if (isSvgAttachment(name, type) || isDng(name, type)) return 'file';
  if (isHeic(name, type)) return 'image';
  if (generic(type) && IMAGE_MIME_BY_EXTENSION[extension(name)]) return 'image';
  if (!type.toLowerCase().startsWith('image/')) return 'file';
  if (SUPPORTED_IMAGE_TYPES.has(type.toLowerCase())) return 'image';
  return PREVIEWABLE_IMAGE_EXTENSIONS.includes(extension(name)) ? 'file' : 'unsupported-image';
}

/** composerFileAttachmentMimeType: what a file attachment is sent as; image detection downstream reads it. */
export function fileAttachmentMimeType(name: string, type: string): string {
  if (isDng(name, type)) return 'image/x-adobe-dng';
  if (isSvgAttachment(name, type)) return 'image/svg+xml';
  return videoMimeType({ name, mimeType: type }) ?? (type || 'application/octet-stream');
}

/** A `doc:/<n>/<name>` handle's name. */
export function handleName(path: string): string {
  const rest = path.startsWith('doc:/') ? path.slice(5) : path;
  return rest.slice(rest.indexOf('/') + 1) || 'file';
}

/** A dropped or pasted handle, its type from its name. */
export function handleSource(path: string): AttachSource {
  const name = handleName(path);
  return { path, name, type: mimeTypeOfName(name) };
}

async function mkdir(fs: StageFiles, path: string): Promise<void> { try { await fs.mkdir(path, { recursive: true }); } catch { /* it exists */ } }


/** A name without path separators, for the scratch copy compressImage reads. */
const scratchName = (name: string) => name.replace(/[/:\\]/g, '-').slice(0, 200) || 'image';

/** fileNameForMimeType: the name with the extension of the type it was encoded as. */
const renamed = (name: string, type: string) => `${name.replace(/\.[^.]*$/, '') || 'image'}.${type === 'image/png' ? 'png' : 'jpg'}`;

/**
 * prepareImageForAttachment into a draft: `{kind: "image", id, name, mimeType, sizeBytes}` (`downscalable: false` for
 * a GIF or WebP, which the upload leaves as it is), else `image-unreadable` or `image-too-large`. The draft is
 * `drafts/<id>.png` whatever its type: its readers decode by content (T3SnapShot.swift).
 */
async function stageImage(fs: StageFiles, source: AttachSource, size: number, id: string): Promise<Obj> {
  const name = source.name;
  if (size > MAX_COMPRESSIBLE_SOURCE_BYTES) return { kind: 'image-too-large', name, sizeBytes: size };
  const out = `${DRAFTS}/${id}.png`;
  let from = source.path, scratch = '';
  try {
    await mkdir(fs, DRAFTS);
    // Its header is read through JavaScript, which fails past HOST_READ_BYTES: a larger image is re-encoded without
    // it, in its own type when that is PNG and else as a JPEG, a dropped one from the app copy compressImage reads.
    if (size > HOST_READ_BYTES) {
      if (!from.startsWith('app:/')) {
        scratch = `${SCRATCH}/${id}`;
        await mkdir(fs, scratch);
        from = `${scratch}/${scratchName(name)}`;
        await fs.copyFile(source.path, from);
      }
      const type = source.type === 'image/png' || /\.png$/i.test(name) ? 'image/png' : 'image/jpeg';
      const encoded = await fs.compressImage(from, out, { maxDimension: IMAGE_ATTACHMENT_MAX_EDGE, maxBytes: PROVIDER_SEND_TURN_MAX_IMAGE_BYTES, type });
      return { kind: 'image', id, name: type === 'image/png' ? name : renamed(name, type), mimeType: type, sizeBytes: encoded.size };
    }
    const bytes = new Uint8Array(await fs.readFile(from));
    const header = imageHeader(bytes);
    const fits = header?.type === 'image/png' || header?.type === 'image/jpeg';
    const over = !!header && fits && Math.max(header.width, header.height) > IMAGE_ATTACHMENT_MAX_EDGE;
    if (header && !over && bytes.byteLength <= PROVIDER_SEND_TURN_MAX_IMAGE_BYTES) {
      await fs.writeFile(out, bytes);
      return { kind: 'image', id, name, mimeType: header.type, sizeBytes: bytes.byteLength, ...(fits ? {} : { downscalable: false }) };
    }
    // fitImageToMaxEdge keeps a PNG or JPEG in its own type; a HEIC converts to JPEG; another over the byte limit is
    // re-encoded, keeping transparency as PNG, within 2000 px as reencodeWithinBudget caps it. compressImage reads app
    // files only.
    const type = !header ? (isHeic(name, source.type) ? 'image/jpeg' : 'image/png')
      : header.type === 'image/jpeg' ? 'image/jpeg' : (header.type === 'image/png' && over) || header.alpha ? 'image/png' : 'image/jpeg';
    if (!from.startsWith('app:/')) {
      scratch = `${SCRATCH}/${id}`;
      await mkdir(fs, scratch);
      from = `${scratch}/${scratchName(name)}`;
      await fs.writeFile(from, bytes);
    }
    const encoded = await fs.compressImage(from, out, { maxDimension: IMAGE_ATTACHMENT_MAX_EDGE, maxBytes: PROVIDER_SEND_TURN_MAX_IMAGE_BYTES, type });
    return { kind: 'image', id, name: header?.type === type ? name : renamed(name, type), mimeType: type, sizeBytes: encoded.size };
  } catch (error) {
    await fs.rm(out).catch(() => undefined);
    const code = String((error as { code?: unknown } | null)?.code ?? '');
    return { kind: code === 'unfit' || code === 'too-large' ? 'image-too-large' : 'image-unreadable', name, sizeBytes: size };
  } finally {
    if (scratch) { await fs.rm(from).catch(() => undefined); await fs.rm(scratch).catch(() => undefined); }
  }
}

/**
 * ComposerImageThumbnail: a 256 px PNG thumbnail of the taken image for its preparing tile, or '' (the tile shows the
 * name) for a source past 50 MB, as prepareImageForAttachment refuses those, or one that cannot be read. compressImage
 * reads only an app file, so a dropped or pasted image is copied first and the copy goes once the thumbnail is made.
 */
export async function previewSource(source: AttachSource, imageId: string): Promise<string> {
  const fs = files(), dir = `${SCRATCH}/preview-${imageId}`, thumbnail = `${dir}/thumbnail.png`;
  let copy = '';
  try {
    const { size } = await fs.stat(source.path);
    if (!(size > 0) || size > MAX_COMPRESSIBLE_SOURCE_BYTES) return '';
    await mkdir(fs, dir);
    if (!source.path.startsWith('app:/')) { copy = `${dir}/${scratchName(source.name)}`; await fs.copyFile(source.path, copy); }
    await fs.compressImage(copy || source.path, thumbnail, { maxDimension: 256, maxBytes: 256 * 1024, type: 'image/png' });
    return thumbnail;
  } catch { return ''; } finally { if (copy) await fs.rm(copy).catch(() => undefined); }
}
/** The thumbnail `previewSource` made, removed with its folder once its tile is gone. */
export async function releasePreview(path: string): Promise<void> {
  if (!path.startsWith(`${SCRATCH}/preview-`)) return;
  const fs = files();
  await fs.rm(path).catch(() => undefined);
  await fs.rm(path.slice(0, path.lastIndexOf('/'))).catch(() => undefined);
}

/** A plain file: copied into composer-files/ when the server takes files of its size (`fileLimit`, 0 when it takes none). */
async function stageFile(fs: StageFiles, source: AttachSource, size: number, fileLimit: number): Promise<Obj> {
  const name = source.name, mimeType = fileAttachmentMimeType(name, source.type);
  const file: Obj = { kind: 'file', name, sizeBytes: size, mimeType };
  if (size <= 0 || fileLimit <= 0 || size > Math.min(fileLimit, MAX_FILE_BYTES)) return file;
  const id = uuid(), to = `${STAGED}/${stagedName({ id, name, mimeType })}`;
  try {
    await mkdir(fs, STAGED);
    // The host copies a picked (`app:`) or dropped (`doc:`) file; its bytes never enter JavaScript.
    await fs.copyFile(source.path, to);
  } catch { return { kind: 'unreadable', name, sizeBytes: size }; }
  file.id = id;
  if (videoMimeType({ name, mimeType }) !== null) {
    // `moov` may follow the media data, and storage.fs reads whole files only, so the header costs a read of the
    // staged copy within HOST_READ_BYTES; a larger video goes without its display size.
    const bytes = size <= HOST_READ_BYTES ? new Uint8Array(await fs.readFile(to).catch(() => new ArrayBuffer(0))) : new Uint8Array(0);
    const frame = videoFrameSize(bytes);
    if (frame) { file.videoWidth = frame.width; file.videoHeight = frame.height; }
  }
  return file;
}

/** What a source is before staging: a folder, a file of its size, or unreadable. */
export async function statSource(source: AttachSource): Promise<{ folder: boolean; size: number } | null> {
  try { const stat = await files().stat(source.path); return { folder: stat.isDirectory, size: stat.size }; } catch { return null; }
}

/**
 * One file of `size` bytes as addAttachedFiles takes it (`kind` image, file, unsupported-image, image-unreadable,
 * image-too-large or unreadable). `fileLimit`: the server's file staging limit (attachStagingLimit); `imageId` the id
 * an image's draft takes (its preparing tile's), a new one when absent.
 */
export async function stageAttachment(source: AttachSource, size: number, fileLimit: number, imageId = ''): Promise<Obj> {
  const kind = classifyAttachment(source.name, source.type);
  if (kind === 'unsupported-image') return { kind, name: source.name, sizeBytes: size };
  return kind === 'image' ? stageImage(files(), source, size, imageId || uuid()) : stageFile(files(), source, size, fileLimit);
}
