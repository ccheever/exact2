import { requestComposerField } from './composer-field-request';
// Attached files that are not images (ChatComposer addComposerAttachments,
// composerAttachmentFiles.ts): within the server's staging limit a dropped, pasted or
// picked file is copied into the device's staged files (composer-attach-stage.ts) and
// its inline file chip goes in at the caret (insertAttachmentReferences); the send
// uploads it and binds it like a folded paste (composer-editor-files.ts).
import { removeStaged } from './composer-staged-copies';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { contextId, contextLabel, contextLink, contextReferences, stripContextReferences } from './composer-editor-menu';
import { composerInput } from './requests';
import { questionDraftKey } from './question-attachments';
import { MAX_FILE_BYTES, draftFiles, fileChipLink, fileStagingLimit, formatAttachmentSize, holdDraftFiles, referencedFiles, setDraftFiles, type DraftFile } from './composer-editor-files';

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
 * One plain file staged for a chip (`kind: "file"`, composer-attach-stage.ts): the draft file
 * it becomes, or the reference's reason it was refused.
 */
export function acceptAttachedFile(client: T3Client, file: Obj, limit = attachStagingLimit(client), draftKey = client.draftKey, environmentId = client.environmentId): { file: DraftFile | null; error: string } {
  const name = str(file.name, 'file'), size = Number(file.sizeBytes) || 0, id = str(file.id);
  if (limit <= 0) return { file: null, error: 'This server does not support file attachments.' };
  if (size <= 0) return { file: null, error: `'${name}' is empty or could not be read.` };
  if (size > limit) return { file: null, error: fileTooLargeMessage(name, limit) };
  if (!/^[a-f0-9-]{36}$/i.test(id)) return { file: null, error: `'${name}' is empty or could not be read.` };
  const staged: DraftFile = { id, contextId: contextId('file', id), draftKey, environmentId, name,
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
export async function insertFileChips(client: T3Client, native: Native, files: DraftFile[], images: Obj[] = [], owner = ''): Promise<boolean> {
  const links = [...files.map(fileChipLink), ...images.map(imageChipLink)];
  if (!links.length) return true;
  if (owner && owner !== client.snapshotOwner) return false;
  const text = links.join(' '), key = files[0]?.draftKey ?? client.draftKey;
  if (key === client.draftKey) {
    const release = holdDraftFiles(client.local, key);
    const reply = await requestComposerField(native, { op: 'editorInsert', text, ...(owner ? { owner } : {}) }, { applied: release, refused: release }).catch(() => null);
    return reply?.accepted === true;
  }
  appendChipLinks(client, key, links);
  return true;
}

/** ensureInlineContextReferences: chips for a draft with no editor join the end of its stored prompt. */
export function appendChipLinks(client: T3Client, draftKey: string, links: readonly string[]): void {
  const prompt = client.local.drafts[draftKey] ?? '', missing = links.filter(link => !prompt.includes(link));
  if (missing.length) client.local.drafts[draftKey] = `${prompt}${prompt && !/\s$/.test(prompt) ? ' ' : ''}${missing.join(' ')} `;
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

/** buildMessageContext binds every uploaded shelf image, including those without an inline chip. */
export function imageContextRecords(client: T3Client, images: Obj[] = client.snapshotDrafts): Obj[] {
  return images.filter(image => str(image.uploadId)).map(image => ({ version: 1, kind: 'image', contextId: contextId('image', str(image.id)),
    label: contextLabel(str(image.name), 'image'), attachmentId: str(image.uploadId), name: str(image.name), mimeType: str(image.mimeType, 'image/png'), sizeBytes: Number(image.sizeBytes) || 0 }));
}

/**
 * countReservedAttachments: the draft's shelf images and the files its prompt references; while the composer answers a
 * question, every question of its request, whose answer takes them together: their shelves and the files their answers reference.
 */
export function reservedAttachments(client: T3Client): number {
  const question = client.attachmentKey === client.draftKey ? undefined : composerInput(client);
  if (!question) return client.snapshotDrafts.length + referencedFiles(client.local, client.draftKey, client.draft).length;
  const { environmentId, threadId } = client, requestId = question.input.requestId;
  return question.input.questions.reduce((total, entry) => total + (client.local.snapshotDrafts[questionDraftKey(environmentId, threadId, requestId, entry.id)] ?? []).length
    + referencedFiles(client.local, client.draftKey, client.answers[`${requestId}::${entry.id}`]?.custom ?? '').length, 0);
}

/** countReservedAttachments for the draft files were given to (`key`, its thread draft `threadKey`), which the composer may
 *  no longer show: its shelf, and the files its stored prompt references. */
export function reservedAttachmentsIn(client: T3Client, key: string, threadKey: string): number {
  if (key === client.attachmentKey && threadKey === client.draftKey) return reservedAttachments(client);
  return (client.local.snapshotDrafts[key] ?? []).length + (key === threadKey ? referencedFiles(client.local, threadKey, client.local.drafts[threadKey] ?? '').length : 0);
}

/** A staged copy no draft will reference (a pick past the attachment limit). */
export async function releaseStagedFile(_native: Native, file: Obj): Promise<void> {
  if (file.kind === 'file' && str(file.id)) await removeStaged(str(file.id));
}
