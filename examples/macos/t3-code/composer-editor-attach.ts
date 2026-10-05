// Attach files for files that are not images (ChatComposer addComposerAttachments,
// composerAttachmentFiles.ts): within the server's staging limit a picked file is
// copied into the device's staged files (T3ComposerAttach.swift) and its inline
// file chip goes in at the caret (insertAttachmentReferences); the send uploads
// it and binds it like a folded paste (composer-editor-files.ts).
import type { T3Client } from './client';
import { bridgeReply, type Native } from './protocol';
import { obj, str, type Obj } from './domain';
import { contextId, contextLabel, contextLink, contextReferences, stripContextReferences } from './composer-editor-menu';
import { MAX_FILE_BYTES, draftFiles, fileChipLink, fileStagingLimit, formatAttachmentSize, referencedFiles, setDraftFiles, type DraftFile } from './composer-editor-files';

/** fileAttachmentStagingLimit: unknown capabilities stage up to 50 MB (the send then waits); 0 when the server takes no files. */
export function attachStagingLimit(client: T3Client): number {
  const capabilities = obj(obj(client.config.environment).capabilities);
  if (capabilities.attachmentUploads === undefined && capabilities.fileAttachments === undefined) return MAX_FILE_BYTES;
  return fileStagingLimit(capabilities);
}

/** fileAttachmentTooLargeMessage (client-runtime attachments.ts). */
export function fileTooLargeMessage(name: string, maxUploadBytes: number): string {
  const size = maxUploadBytes >= 1024 * 1024 && maxUploadBytes % (1024 * 1024) === 0 ? `${maxUploadBytes / (1024 * 1024)} MB`
    : maxUploadBytes >= 1024 && maxUploadBytes % 1024 === 0 ? `${maxUploadBytes / 1024} KB`
    : `${maxUploadBytes} ${maxUploadBytes === 1 ? 'byte' : 'bytes'}`;
  return `'${name}' exceeds the ${size} attachment limit.`;
}

/**
 * One picked plain file (`kind: "file"` from composerAttachPick): the draft file
 * it becomes, or the reference's reason it was refused.
 */
export function acceptAttachedFile(client: T3Client, file: Obj, limit = attachStagingLimit(client)): { file: DraftFile | null; error: string } {
  const name = str(file.name, 'file'), size = Number(file.sizeBytes) || 0, id = str(file.id);
  if (limit <= 0) return { file: null, error: 'This server does not support file attachments.' };
  if (size <= 0) return { file: null, error: `'${name}' is empty or could not be read.` };
  if (size > limit) return { file: null, error: fileTooLargeMessage(name, limit) };
  if (!/^[a-f0-9-]{36}$/i.test(id)) return { file: null, error: `'${name}' is empty or could not be read.` };
  const staged: DraftFile = { id, contextId: contextId('file', id), draftKey: client.draftKey, environmentId: client.environmentId, name,
    mimeType: str(file.mimeType, 'application/octet-stream'), sizeBytes: size, source: 'attached', attachmentId: '', status: 'staged' };
  setDraftFiles(client.local, [...draftFiles(client.local), staged]);
  return { file: staged, error: '' };
}

/**
 * imageAttachmentsGetChips: an image picked while the prompt has prose also
 * gets an inline image chip; with none it lives on the shelf alone.
 */
export function imagesGetChips(client: T3Client): boolean {
  return stripContextReferences(client.draft).trim().length > 0;
}

/**
 * insertAttachmentReferences: the files' chips, then the images' chips, at the
 * caret, separated by spaces, with a leading boundary; without an editor they
 * join the end of the stored draft (ensureInlineContextReferences).
 */
export async function insertFileChips(client: T3Client, native: Native, files: DraftFile[], images: Obj[] = []): Promise<void> {
  const links = [...files.map(fileChipLink), ...images.map(imageChipLink)];
  if (!links.length) return;
  const text = links.join(' ');
  const reply = await bridgeReply(native, { op: 'editorInsert', text }).catch(() => null);
  if (reply?.ok && obj(reply.value).applied === true) return;
  const key = files[0]?.draftKey ?? client.draftKey, prompt = client.local.drafts[key] ?? '';
  client.local.drafts[key] = `${prompt}${prompt && !/\s$/.test(prompt) ? ' ' : ''}${text} `;
}

// ── Image chips (imageContextReference, ImageChipButton) ───────────────────

/** `![name](t3-context://v1/image/image_<draft id>)` for one shelf image. */
export function imageChipLink(image: Obj): string { return contextLink('image', contextId('image', str(image.id)), str(image.name, 'image')); }

/** The shelf images the prompt's image chips name, in prompt order. */
export function referencedImages(client: T3Client, text: string): Obj[] {
  const images = client.snapshotDrafts;
  return contextReferences(text).flatMap(reference => {
    if (reference.kind !== 'image') return [];
    const image = images.find(candidate => contextId('image', str(candidate.id)) === reference.id);
    return image ? [image] : [];
  });
}

/** Chip kinds for the native editor: `image/<id>` → "image\t<size>\t<draft id>" (its thumbnail and average colour). */
export function imageChipContexts(client: T3Client, text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const image of referencedImages(client, text)) out[`image/${contextId('image', str(image.id))}`] = `image\t${formatAttachmentSize(Number(image.sizeBytes) || 0)}\t${str(image.id)}`;
  return out;
}

/** attachmentContextRecord for each image chip whose image is uploaded (the send uploads the shelf first). */
export function imageContextRecords(client: T3Client, text: string): Obj[] {
  return referencedImages(client, text).filter(image => str(image.uploadId)).map(image => ({ version: 1, kind: 'image', contextId: contextId('image', str(image.id)),
    label: contextLabel(str(image.name), 'image'), attachmentId: str(image.uploadId), name: str(image.name), mimeType: str(image.mimeType, 'image/png'), sizeBytes: Number(image.sizeBytes) || 0 }));
}

/** countReservedAttachments: the draft's shelf images and the files its prompt references. */
export function reservedAttachments(client: T3Client): number {
  return client.snapshotDrafts.length + referencedFiles(client.local, client.draftKey, client.draft).length;
}

/** A staged copy no draft will reference (a pick past the attachment limit). */
export async function releaseStagedFile(native: Native, file: Obj): Promise<void> {
  if (file.kind === 'file' && str(file.id)) await bridgeReply(native, { op: 'composerAttachRemove', id: str(file.id) }).catch(() => undefined);
}
