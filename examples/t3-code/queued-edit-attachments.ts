// Editing a queued message together with its attachments (task
// composer-fidelity, G12a), adapted from T3 Code 1e2ecbd975 (MIT); see
// LICENSE-T3. Sources: apps/web/src/components/chat/queuedMessageEdit.ts
// (prepareQueuedEditAttachments, recoverQueuedMessageEdit), ChatView.tsx
// (beginEditingQueuedRun, removeEditingQueuedAttachment, the queued-edit save
// path and its lost-run toasts) and packages/client-runtime/src/operations/
// commands.ts (editQueuedRun: `queued-run.edit` with the full attachment list
// and the merged context).
// Changes: the edit borrows the thread's composer, so the thread's own text and
// SnapShot images wait in a stash (no second draft target); new images upload
// through the client's SnapShot uploader and new files through the composer's
// file uploads (composer-editor-files.ts), which already return stored
// attachments, so no `assets.persistChatAttachments` round trip is needed.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { composerFileAttachments, referencedFiles } from './composer-editor-files';
import { messageContext } from './composer-editor';

/** PROVIDER_SEND_TURN_MAX_ATTACHMENTS. */
export const MAX_ATTACHMENTS = 100;
/** ATTACHMENT_ONLY_BOOTSTRAP_PROMPT: the text an attachment-only message sends. */
export const ATTACHMENT_ONLY_PROMPT = '[User attached one or more files without additional text. Respond using the conversation context and the attached files.]';
export const SAVE_FAILED = 'Could not save the edited queued message.';

/**
 * prepareQueuedEditAttachments: kept attachments, then the new images, then
 * the new files. A file whose upload is missing fails the save instead of
 * being dropped.
 */
export async function prepareQueuedEditAttachments(input: { existing: Obj[]; images: Obj[]; files: Obj[];
  uploadImages: (images: Obj[]) => Promise<Obj[]>; uploadFiles: (files: Obj[]) => Promise<Obj[]> }): Promise<Obj[]> {
  const files = input.files.length ? await input.uploadFiles(input.files) : [];
  if (files.length !== input.files.length) throw new ClientError('Retry or remove failed uploads before saving.');
  const images = input.images.length ? await input.uploadImages(input.images) : [];
  if (images.length !== input.images.length) throw new ClientError('Retry or remove failed uploads before saving.');
  return [...input.existing, ...images, ...files];
}

/**
 * The edit's context: the message's records whose attachment is still kept
 * (records without an attachment always stay), then the new prompt's records;
 * a later record with the same context id wins.
 */
export function mergedEditContext(original: Obj | undefined, kept: Obj[], next: Obj | undefined): Obj {
  const ids = new Set(kept.map(attachment => str(attachment.id)));
  const records = [...arr(obj(original).records).filter(record => !('attachmentId' in record) || ids.has(str(record.attachmentId))), ...arr(obj(next).records)];
  return { version: 1, records: [...new Map(records.map(record => [str(record.contextId), record])).values()] };
}

/**
 * recoverQueuedMessageEdit: a dirty edit (text changed, or new attachments)
 * moves into the thread's draft when that draft is empty ("kept"); otherwise
 * it is dropped ("discarded"); an untouched edit is "clean".
 */
export function recoverQueuedMessageEdit(input: { prompt: string; images: number; files: number; originalText: string; threadHasContent: boolean }): 'kept' | 'discarded' | 'clean' {
  const dirty = input.prompt !== input.originalText || input.images > 0 || input.files > 0;
  if (dirty && !input.threadHasContent) return 'kept';
  return dirty ? 'discarded' : 'clean';
}

// ── The edit session (the composer borrowed from the thread) ───────────────

export type QueuedEditing = { key: string; runId: string; messageId: string; originalText: string; stash: string; stashImages: Obj[]; existing: Obj[]; context: Obj | undefined; saving: boolean };

/** beginEditingQueuedRun: the message's text and attachments load; the thread's draft waits. */
export function startEdit(client: T3Client, key: string, run: Obj, text: string, previous: QueuedEditing | undefined): QueuedEditing {
  const message = arr(client.projection.messages).find(candidate => candidate.id === run.userMessageId);
  const draftKey = client.draftKey;
  const stash = previous ? previous.stash : client.local.drafts[draftKey] ?? '';
  const stashImages = previous ? previous.stashImages : client.local.snapshotDrafts[draftKey] ?? [];
  client.local.drafts[draftKey] = text;
  client.local.snapshotDrafts[draftKey] = [];
  return { key, runId: str(run.id), messageId: str(run.userMessageId), originalText: text, stash, stashImages,
    existing: arr(message?.attachments).map(attachment => ({ ...attachment })), context: message?.context ? obj(message.context) : undefined, saving: false };
}
/** The thread's own draft comes back (cancel, a save, or a discarded edit). */
export function restoreThreadDraft(client: T3Client, editing: QueuedEditing): void {
  client.local.drafts[client.draftKey] = editing.stash;
  client.local.snapshotDrafts[client.draftKey] = editing.stashImages;
}
/** removeEditingQueuedAttachment. */
export function removeExisting(editing: QueuedEditing | undefined, attachmentId: string): void {
  if (editing) editing.existing = editing.existing.filter(attachment => str(attachment.id) !== attachmentId);
}

/** The edit's run left the queue: keep a dirty edit in an empty thread draft, else drop it, with the reference's toasts. */
export function endLostEdit(client: T3Client, editing: QueuedEditing): void {
  const prompt = client.local.drafts[client.draftKey] ?? '';
  const outcome = recoverQueuedMessageEdit({ prompt, images: (client.local.snapshotDrafts[client.draftKey] ?? []).length,
    files: referencedFiles(client.local, client.draftKey, prompt).length, originalText: editing.originalText,
    threadHasContent: editing.stash.trim() !== '' || editing.stashImages.length > 0 });
  if (outcome === 'kept') { pushToast(client, { kind: 'info', title: 'Queued message is no longer queued', description: 'Your unsaved edit was kept in the composer.' }); return; }
  restoreThreadDraft(client, editing);
  if (outcome === 'discarded') pushToast(client, { kind: 'warning', title: 'Queued message is no longer queued', description: 'Your unsaved edit was discarded.' });
}

/**
 * The send button saves the edit: at most 100 attachments in all; an empty
 * text with attachments sends the attachment-only prompt; nothing at all is a
 * no-op. `queued-run.edit` carries the full attachment list and the context.
 */
export async function saveEdit(client: T3Client, native: Native, storage: Files, editing: QueuedEditing, value: string,
  uploadImages: () => Promise<Obj[]>, stillQueued: () => boolean): Promise<boolean> {
  if (editing.saving) return false;
  const text = (value || client.local.drafts[client.draftKey] || '').trim();
  const images = client.local.snapshotDrafts[client.draftKey] ?? [], files = referencedFiles(client.local, client.draftKey, text);
  if (editing.existing.length + images.length + files.length > MAX_ATTACHMENTS) throw new ClientError(`A message can have at most ${MAX_ATTACHMENTS} attachments.`);
  if (!text && !editing.existing.length && !images.length && !files.length) return false;
  if (!stillQueued()) throw new ClientError('That queued message already started or was removed.');
  editing.saving = true;
  try {
    const attachments = await prepareQueuedEditAttachments({ existing: editing.existing, images, files,
      uploadImages: async () => uploadImages(), uploadFiles: async () => composerFileAttachments(client, native, text) });
    const context = mergedEditContext(editing.context, editing.existing, messageContext(client, text));
    const access = client.restAccess(native);
    const [commandId] = await access.ids(1);
    try {
      await access.dispatch(storage, { type: 'queued-run.edit', commandId, threadId: client.threadId, runId: editing.runId,
        text: text || ATTACHMENT_ONLY_PROMPT, attachments, ...(arr(context.records).length ? { context } : {}) }, 'Update queued message');
    } catch (error) {
      if (error instanceof ClientError && error.kind === 'superseded') throw error;
      throw new ClientError(SAVE_FAILED);
    }
  } finally { editing.saving = false; }
  return true;
}

/** The composer's view of the edit: its kept attachments (removable) and whether a save is in flight. */
export function editView(editing: QueuedEditing | undefined) {
  return { queueEditAttachments: (editing?.existing ?? []).map(attachment => ({ id: str(attachment.id), name: str(attachment.name, 'Attachment'), image: attachment.type === 'image' })),
    queueEditSaving: !!editing?.saving };
}
