// Editing a queued message together with its attachments,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 7c3b3b3605ef (the attachment-only prompt at 2359643cff):
// apps/web/src/components/chat/queuedMessageEdit.ts (prepareQueuedEditAttachments,
// recoverQueuedMessageEdit), chat/useQueuedRunEdit.ts (beginEdit, removeEditingAttachment,
// submitQueuedRunEdit, the run-left-queue toasts) and packages/client-runtime/src/
// operations/commands.ts (editQueuedRun: `queued-run.edit` with the full attachment list
// and the merged context).
// Changes: the edit borrows its thread's composer, so the thread's own text and
// SnapShot images wait in a stash (no second draft target); new images upload
// through the client's SnapShot uploader and new files through the composer's
// file uploads (composer-editor-files.ts), which already return stored
// attachments, so no `assets.persistChatAttachments` round trip is needed.
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import type { T3Client } from '../client/client';
import { composerFileAttachments, referencedFiles } from './composer-editor-files';
import { messageContext } from './composer-editor';
import { restoreAttachmentOnlyPrompt, stripAttachmentOnlyPrompt } from './attachment-only-prompt';
export { ATTACHMENT_ONLY_PROMPT } from './attachment-only-prompt';

/** PROVIDER_SEND_TURN_MAX_ATTACHMENTS. */
export const MAX_ATTACHMENTS = 100;
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

/** An open edit: `key` is its thread's draft key, and `paused` while a save it started is in flight. */
export type QueuedEditing = { key: string; environmentId: string; threadId: string; runId: string; messageId: string; originalText: string;
  stash: string; stashImages: Obj[]; existing: Obj[]; context: Obj | undefined; paused: boolean };

/** beginEdit: the message's text and attachments load; the thread's draft waits (a previous edit's stash carries over). */
export function startEdit(client: T3Client, run: Obj, text: string, previous: QueuedEditing | undefined): QueuedEditing {
  const message = arr(client.projection.messages).find(candidate => candidate.id === run.userMessageId);
  const draftKey = client.draftKey;
  const stash = previous ? previous.stash : client.local.drafts[draftKey] ?? '';
  const stashImages = previous ? previous.stashImages : client.local.snapshotDrafts[draftKey] ?? [];
  // An attachment-only message opens with an empty field, not the prompt it was sent with.
  const editText = stripAttachmentOnlyPrompt(text);
  client.local.drafts[draftKey] = editText;
  client.local.snapshotDrafts[draftKey] = [];
  return { key: draftKey, environmentId: client.environmentId, threadId: client.threadId, runId: str(run.id), messageId: str(run.userMessageId), originalText: editText,
    stash, stashImages, existing: arr(message?.attachments).map(attachment => ({ ...attachment })), context: message?.context ? obj(message.context) : undefined, paused: false };
}
/** The thread's own draft comes back (cancel, a save, or a discarded edit), wherever the user is. */
export function restoreThreadDraft(client: T3Client, editing: QueuedEditing): void {
  client.local.drafts[editing.key] = editing.stash;
  client.local.snapshotDrafts[editing.key] = editing.stashImages;
}
/** removeEditingQueuedAttachment. */
export function removeExisting(editing: QueuedEditing | undefined, attachmentId: string): void {
  if (editing) editing.existing = editing.existing.filter(attachment => str(attachment.id) !== attachmentId);
}

/**
 * The edit's run left the queue (it started or was removed): a changed edit stays as the thread's draft when that draft
 * is empty, else it is dropped. Edits outlive navigation, so the toast names the thread: the user may be anywhere by now.
 */
export function endLostEdit(client: T3Client, editing: QueuedEditing): void {
  const prompt = client.local.drafts[editing.key] ?? '';
  const outcome = recoverQueuedMessageEdit({ prompt, images: (client.local.snapshotDrafts[editing.key] ?? []).length,
    files: referencedFiles(client.local, editing.key, prompt).length, originalText: editing.originalText,
    threadHasContent: editing.stash.trim() !== '' || editing.stashImages.length > 0 });
  if (outcome !== 'kept') restoreThreadDraft(client, editing);
  if (outcome === 'clean') return;
  const title = client.environmentId === editing.environmentId ? client.shell.threads.find(thread => thread.id === editing.threadId)?.title : undefined;
  const where = typeof title === 'string' ? ` in “${title}”` : '';
  pushToast(client, { kind: outcome === 'kept' ? 'info' : 'warning', title: 'Queued message is no longer queued',
    description: outcome === 'kept' ? `Your unsaved edit${where} was kept in the composer.` : `Your unsaved edit${where} was discarded.` });
}

/**
 * submitQueuedRunEdit: at most 100 attachments in all; an empty text (or "Ultrathink:" alone) with attachments sends the
 * attachment-only prompt; nothing at all is a no-op (false). `queued-run.edit` carries the full attachment list and the context; the save's
 * result decides the edit's fate, even when the run starts meanwhile.
 */
export async function saveEdit(client: T3Client, native: Native, storage: Files, editing: QueuedEditing, value: string,
  uploadImages: () => Promise<Obj[]>): Promise<boolean> {
  const text = (value || client.local.drafts[editing.key] || '').trim();
  const images = client.local.snapshotDrafts[editing.key] ?? [], files = referencedFiles(client.local, editing.key, text);
  if (editing.existing.length + images.length + files.length > MAX_ATTACHMENTS) throw new ClientError(`A message can have at most ${MAX_ATTACHMENTS} attachments.`);
  if (!text && !editing.existing.length && !images.length && !files.length) return false;
  const attachments = await prepareQueuedEditAttachments({ existing: editing.existing, images, files,
    uploadImages: async () => uploadImages(), uploadFiles: async () => composerFileAttachments(client, native, text) });
  const context = mergedEditContext(editing.context, editing.existing, messageContext(client, text));
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  try {
    await access.dispatch(storage, { type: 'queued-run.edit', commandId, threadId: editing.threadId, runId: editing.runId,
      text: restoreAttachmentOnlyPrompt(text, attachments.length > 0), attachments, ...(arr(context.records).length ? { context } : {}) }, 'Update queued message');
  } catch (error) {
    if (error instanceof ClientError && error.kind === 'superseded') throw error;
    throw new ClientError(SAVE_FAILED);
  }
  return true;
}

/** The composer's view of the edit: its kept attachments (removable) and whether its save is in flight. */
export function editView(editing: QueuedEditing | undefined, saving: boolean) {
  return { queueEditAttachments: (editing?.existing ?? []).map(attachment => ({ id: str(attachment.id), name: str(attachment.name, 'Attachment'), image: attachment.type === 'image' })),
    queueEditSaving: saving };
}
