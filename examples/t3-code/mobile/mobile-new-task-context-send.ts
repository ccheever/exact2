// Keep the existing direct Send path correct while durable queue integration remains separate.
// @ref llp/1109.005-composer-and-transcript.decision.md#foreground-capture-facts
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
import { withMessageContext } from './shared/composer-editor';
import { composerFileRecords, draftFiles } from './shared/composer-editor-files';
import { imageContextRecords } from './shared/composer-editor-attach';
import { contextReferences } from './shared/composer-editor-menu';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsKey } from './mobile-new-task-drafts';
import { mobileNewTaskContextProject } from './mobile-new-task-context';

function owned(client: T3Client, text: string): Obj[] | null {
  if (!mobileNewTaskDraftIsKey(client.draftKey)) return null;
  const draft = mobileNewTaskDraftCurrent(client);
  if (!draft) throw new ClientError('The draft changed.', 'superseded');
  const projected = mobileNewTaskContextProject(text, draft.context);
  if (!projected.ok) throw new ClientError(projected.error);
  const records = projected.context?.records ?? [];
  for (const ref of contextReferences(text)) {
    if (['image', 'file'].includes(ref.kind)) continue;
    const record = records.find(record => record.contextId === ref.id && record.kind === ref.kind);
    if (!record || record.kind === 'terminal' && !str(record.text).trim()) throw new ClientError('Restore the saved context for this draft before sending.');
  }
  return records;
}
/** Independent terminal records are durable payloads, never a live-terminal cache lookup. */
export function mobileNewTaskDirectText(client: T3Client, text: string): { text: string; empty: boolean } | null {
  return owned(client, text) === null ? null : { text, empty: false };
}
/** Capture only content identity: the existing upload path changes attachment metadata itself. */
export function mobileNewTaskSendGuard(client: T3Client): () => void {
  const identity = () => {
    const draft = mobileNewTaskDraftCurrent(client);
    return draft ? JSON.stringify([draft.key, draft.createdAt, draft.revision, draft.origin, draft.environmentId, draft.projectId,
      draft.context, client.local.drafts[draft.key] ?? '']) : '';
  };
  const captured = mobileNewTaskDraftIsKey(client.draftKey) ? identity() : null;
  return () => { if (captured !== null && (!captured || identity() !== captured)) throw new ClientError('The draft context changed before sending.', 'superseded'); };
}
export function mobileNewTaskMessageContext(client: T3Client, payload: Obj, text: string): Obj {
  const original = owned(client, text);
  if (original === null) return withMessageContext(client, payload, text);
  const body = obj(payload.initialMessage).text !== undefined ? obj(payload.initialMessage) : payload;
  const sent = arr(body.attachments), images = client.local.snapshotDrafts[client.draftKey] ?? [];
  const files = draftFiles(client.local).filter(file => file.draftKey === client.draftKey);
  const records = original.map(record => {
    if (!['image', 'file'].includes(str(record.kind))) return record;
    const local = record.kind === 'image' ? images.find(image => image.id === record.attachmentId) ?? images.find(image => image.uploadId === record.attachmentId)
      : files.find(file => file.id === record.attachmentId) ?? files.find(file => file.attachmentId === record.attachmentId);
    const uploadId = record.kind === 'image' ? str(obj(local).uploadId) : str(obj(local).attachmentId);
    if (!local || !['name', 'mimeType', 'sizeBytes'].every(field => obj(local)[field] === record[field]) || !uploadId || !sent.some(file => file.id === uploadId)) throw new ClientError('The attachment context could not be verified.');
    return { ...record, attachmentId: uploadId };
  });
  for (const record of [...composerFileRecords(client, text), ...imageContextRecords(client, text)]) {
    if (!records.some(entry => entry.contextId === record.contextId)) records.push(record);
  }
  for (const ref of contextReferences(text)) if (!records.some(record => record.kind === ref.kind && record.contextId === ref.id))
    throw new ClientError('The draft context could not be resolved.');
  if (!records.length) return payload;
  const context = { version: 1, records };
  return body === payload ? { ...payload, context } : { ...payload, initialMessage: { ...body, context } };
}
