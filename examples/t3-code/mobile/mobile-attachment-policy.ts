// Pinned365aa87982 shared/image and mobile attachmentUpload wire policy.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { Obj } from './shared/domain';

export interface MobileAttachmentPolicyInput {
  kind: 'image' | 'file'; name: string; mimeType: string;
}
export interface MobileAttachmentReferenceInput extends MobileAttachmentPolicyInput {
  sizeBytes: number; source?: unknown;
}
const imageTypes = ['image/gif', 'image/jpeg', 'image/png', 'image/webp'];
const imageExtensions = new Map([
  ['gif', 'image/gif'], ['jpeg', 'image/jpeg'], ['jpg', 'image/jpeg'],
  ['png', 'image/png'], ['webp', 'image/webp'],
]);
const genericTypes = new Set(['application/octet-stream', 'binary/octet-stream', 'application/unknown']);
function imageMimeType(file: MobileAttachmentPolicyInput): string | null {
  const mime = file.mimeType.split(';', 1)[0]?.trim().toLowerCase() ?? '';
  if (imageTypes.includes(mime)) return mime;
  if (mime.startsWith('image/') || (mime !== '' && !genericTypes.has(mime))) return null;
  const dot = file.name.lastIndexOf('.');
  return dot < 0 ? null : imageExtensions.get(file.name.slice(dot + 1).trim().toLowerCase()) ?? null;
}

/** Explicit non-image MIME wins over a filename. Files with unknown MIME may
 * become images, but an image-picker entry with no supported MIME is refused. */
export function mobileComposerAttachmentWireKindAndMime(file: MobileAttachmentPolicyInput): {
  type: 'image' | 'file'; mimeType: string;
} {
  const inferred = imageMimeType(file);
  if (file.kind !== 'image' && inferred === null) return { type: 'file', mimeType: file.mimeType };
  const mimeType = imageTypes.find(type => type === file.mimeType.toLowerCase() || type === inferred);
  if (!mimeType) throw new Error(`Unsupported image type for '${file.name}'.`);
  return { type: 'image', mimeType };
}

/** Modern source uploadedReference omits image source, even a valid snapshot.
 * Local file source strings become the source contract's tagged pasted-text. */
export function mobileUploadedAttachmentReference(file: MobileAttachmentReferenceInput, id: string): Obj {
  const wire = mobileComposerAttachmentWireKindAndMime(file);
  const fields = { type: wire.type, id, name: file.name, mimeType: wire.mimeType, sizeBytes: file.sizeBytes };
  const pasted = file.source === 'pasted-text' ||
    (file.source !== null && typeof file.source === 'object' && !Array.isArray(file.source) &&
      (file.source as Obj)._tag === 'pasted-text');
  return wire.type === 'file' && pasted ? { ...fields, source: { _tag: 'pasted-text' } } : fields;
}
