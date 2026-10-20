// One attachment's staging, adapted from T3 Code 9c161462c9 (MIT; see LICENSE-T3): chat/composerAttachmentFiles.ts
// (classifyComposerAttachmentFile, inferImageMimeTypeFromName), lib/imageCompression.ts (prepareImageForAttachment,
// MAX_COMPRESSIBLE_SOURCE_BYTES) and packages/contracts chatAttachment.ts (PROVIDER_SEND_TURN_MAX_IMAGE_BYTES). A file
// arrives as a `doc:` handle (a drop or a paste) or as a picked `app:/tmp/picked/` copy (Attach files). An image becomes
// a PNG draft beside SnapShot's, which the shelf shows and the send uploads (client.ts uploadSnapshots): a PNG within
// the edge and the byte limit keeps its bytes, any other image is encoded by `compressImage` as a PNG, which keeps its
// alpha. A plain file within the server's limit is copied to `composer-files/<id>` for its chip; a video's or audio
// file's typed link, and a video's first frame, are the module's (`composerAttachMedia`, T3ComposerAttach.swift).
import { bridgeReply, type Native } from '../client/protocol';
import type { Obj } from '../client/domain';
import { appFiles } from '../media/save-file';
import { uuid } from '../transport/transport-wire';
import { MAX_FILE_BYTES } from './composer-editor-files';
import { videoMimeType } from './composer-video';
import { IMAGE_ATTACHMENT_MAX_EDGE } from './image-attachment-size';

/** One file to attach: its path, its name, and the MIME type its source recorded ("" when none did). */
export type AttachSource = { path: string; name: string; type: string };

export const PROVIDER_SEND_TURN_MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const MAX_COMPRESSIBLE_SOURCE_BYTES = 50 * 1024 * 1024;
/** compressImage's largest `maxDimension`: a GIF or WebP keeps its size (the reference fits only PNG and JPEG). */
const UNFITTED_EDGE = 8192;
export const DRAFTS = 'app:/data/snapshots/drafts';
export const STAGED = 'app:/data/composer-files';
const SCRATCH = 'app:/tmp/composer-stage';

/** The app's storage operations staging uses (storage.d.ts). */
interface StageFiles {
  readFile(path: string): Promise<ArrayBuffer>;
  writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  rm(path: string): Promise<unknown>;
  stat(path: string): Promise<{ size: number; isFile: boolean; isDirectory: boolean }>;
  copyFile(from: string, to: string): Promise<unknown>;
  compressImage(from: string, to: string, options: { maxDimension: number; maxBytes: number; type: 'image/png' }): Promise<{ size: number }>;
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

/** classifyComposerAttachmentFile: a supported image, an image type the provider cannot take, or a plain file. */
export function classifyAttachment(name: string, type: string): 'image' | 'file' | 'unsupported-image' {
  if (isHeic(name, type)) return 'image';
  if (generic(type) && IMAGE_MIME_BY_EXTENSION[extension(name)]) return 'image';
  if (!type.toLowerCase().startsWith('image/')) return 'file';
  return SUPPORTED_IMAGE_TYPES.has(type.toLowerCase()) ? 'image' : 'unsupported-image';
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

/** The pixel size in a PNG's IHDR, or null for bytes that are not a PNG. */
export function pngSize(bytes: Uint8Array): { width: number; height: number } | null {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (bytes.length < 24 || signature.some((byte, index) => bytes[index] !== byte)) return null;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return { width: view.getUint32(16), height: view.getUint32(20) };
}

async function mkdir(fs: StageFiles, path: string): Promise<void> { try { await fs.mkdir(path, { recursive: true }); } catch { /* it exists */ } }

/** `from` copied to `to`: an app file in place, a document by its bytes (`doc:` paths refuse copyFile). */
async function copy(fs: StageFiles, from: string, to: string): Promise<void> {
  if (from.startsWith('app:/')) await fs.copyFile(from, to);
  else await fs.writeFile(to, new Uint8Array(await fs.readFile(from)));
}

/** A name without path separators, for the scratch copy compressImage reads. */
const scratchName = (name: string) => name.replace(/[/:\\]/g, '-').slice(0, 200) || 'image';

/**
 * prepareImageForAttachment into a PNG draft: `{kind: "image", id, name, sizeBytes}` (`downscalable: false` for a GIF
 * or WebP, which the upload leaves at its size too), else `image-unreadable` or `image-too-large`.
 */
async function stageImage(fs: StageFiles, source: AttachSource, size: number): Promise<Obj> {
  const name = source.name, base = name.replace(/\.[^.]*$/, '') || 'image';
  if (size > MAX_COMPRESSIBLE_SOURCE_BYTES) return { kind: 'image-too-large', name, sizeBytes: size };
  const type = isHeic(name, source.type) ? 'image/heic' : generic(source.type) ? IMAGE_MIME_BY_EXTENSION[extension(name)] ?? '' : source.type.toLowerCase();
  const downscalable = type !== 'image/gif' && type !== 'image/webp';
  const id = uuid(), out = `${DRAFTS}/${id}.png`;
  let from = source.path, scratch = '';
  try {
    await mkdir(fs, DRAFTS);
    let bytes: Uint8Array | null = null;
    if (!from.startsWith('app:/')) {
      // compressImage reads app files only.
      bytes = new Uint8Array(await fs.readFile(from));
      scratch = `${SCRATCH}/${id}`;
      await mkdir(fs, scratch);
      from = `${scratch}/${scratchName(name)}`;
      await fs.writeFile(from, bytes);
    } else if (type === 'image/png') bytes = new Uint8Array(await fs.readFile(from));
    const png = bytes && pngSize(bytes);
    let written: number;
    if (bytes && png && Math.max(png.width, png.height) <= IMAGE_ATTACHMENT_MAX_EDGE && bytes.byteLength <= PROVIDER_SEND_TURN_MAX_IMAGE_BYTES) {
      await fs.writeFile(out, bytes);
      written = bytes.byteLength;
    } else {
      const encoded = await fs.compressImage(from, out, { maxDimension: downscalable ? IMAGE_ATTACHMENT_MAX_EDGE : UNFITTED_EDGE, maxBytes: PROVIDER_SEND_TURN_MAX_IMAGE_BYTES, type: 'image/png' });
      written = encoded.size;
    }
    return { kind: 'image', id, name: `${base}.png`, sizeBytes: written, ...(downscalable ? {} : { downscalable: false }) };
  } catch (error) {
    const code = String((error as { code?: unknown } | null)?.code ?? '');
    return { kind: code === 'unfit' || code === 'too-large' ? 'image-too-large' : 'image-unreadable', name, sizeBytes: size };
  } finally {
    if (scratch) { await fs.rm(from).catch(() => undefined); await fs.rm(scratch).catch(() => undefined); }
  }
}

/** A plain file: copied into composer-files/ when the server takes files of its size (`fileLimit`, 0 when it takes none). */
async function stageFile(fs: StageFiles, native: Native, source: AttachSource, size: number, fileLimit: number): Promise<Obj> {
  const name = source.name, mimeType = videoMimeType({ name, mimeType: source.type }) ?? (source.type || 'application/octet-stream');
  const file: Obj = { kind: 'file', name, sizeBytes: size, mimeType };
  if (size <= 0 || fileLimit <= 0 || size > Math.min(fileLimit, MAX_FILE_BYTES)) return file;
  const id = uuid();
  try {
    await mkdir(fs, STAGED);
    await copy(fs, source.path, `${STAGED}/${id}`);
  } catch { return { kind: 'unreadable', name, sizeBytes: size }; }
  file.id = id;
  const video = videoMimeType({ name, mimeType }) !== null;
  if (video || mimeType.startsWith('audio/')) {
    const media = await bridgeReply(native, { op: 'composerAttachMedia', id, ext: extension(name), video }).catch(() => null);
    const value = media?.ok ? media.value as Obj : {};
    if (Number(value.videoWidth) > 0 && Number(value.videoHeight) > 0) { file.videoWidth = value.videoWidth; file.videoHeight = value.videoHeight; }
  }
  return file;
}

/**
 * One file as addAttachedFiles takes it (`kind` image, file, unsupported-image, image-unreadable, image-too-large,
 * unreadable or folder). `fileLimit`: the server's file staging limit (attachStagingLimit).
 */
export async function stageAttachment(native: Native, source: AttachSource, fileLimit: number): Promise<Obj> {
  const fs = files();
  let size: number;
  try {
    const stat = await fs.stat(source.path);
    if (stat.isDirectory) return { kind: 'folder', name: source.name, path: source.path };
    size = stat.size;
  } catch { return { kind: 'unreadable', name: source.name, sizeBytes: 0 }; }
  const kind = classifyAttachment(source.name, source.type);
  if (kind === 'unsupported-image') return { kind, name: source.name, sizeBytes: size };
  return kind === 'image' ? stageImage(fs, source, size) : stageFile(fs, native, source, size, fileLimit);
}
