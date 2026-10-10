// Pinned365aa87982 attachmentUpload + resolveQueuedEditPayload. Ordered mobile
// attachments use existing native byte stores and shared authenticated RPC.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { fileStagingLimit } from './shared/composer-editor-files';
import { mobileQueuedEditLookup, mobileQueuedEditPersist } from './queued-edit-state';
import { queuedEditState, type MobileQueuedEditAttachment, type MobileQueuedEditSession } from './queued-edit-memory';
import { mobileComposerAttachmentWireKindAndMime } from './mobile-attachment-policy';
export function queuedEditImageMime(file: Pick<MobileQueuedEditAttachment, 'name' | 'mimeType'>): string {
  const wire = mobileComposerAttachmentWireKindAndMime({ ...file, kind: 'file' });
  return wire.type === 'image' ? wire.mimeType : '';
}
export function queuedEditResolvePayload(edit: MobileQueuedEditSession, uploaded: Obj[]) {
  if (uploaded.length !== edit.attachments.length) throw new ClientError('Retry or remove failed uploads before saving.');
  const attachments = [...edit.existingAttachments, ...uploaded];
  if (attachments.length > 100) throw new ClientError('You can attach up to 100 attachments per message.');
  const rebound = new Map(edit.attachments.map((file, index) => [file.id, str(uploaded[index]?.id)]));
  const live = new Set(attachments.map(file => str(file.id)).filter(Boolean));
  const records = arr(edit.context?.records).map(record => 'attachmentId' in record
    ? { ...record, attachmentId: rebound.get(str(record.attachmentId)) || record.attachmentId } : record)
    .filter(record => !('attachmentId' in record) || live.has(str(record.attachmentId)));
  return { attachments, ...(records.length ? { context: { version: 1, records } } : {}) };
}
/** Caller owns selected session/grants and guards every native await. Persisted
 * uploaded IDs are verified, not assumed usable after a previous failed save. */
export async function mobileQueuedEditUpload(owner: string, native: Native, client: T3Client): Promise<Obj[]> {
  const captured = mobileQueuedEditLookup(owner, client);
  if (!captured) throw new ClientError('That queued edit has ended.', 'superseded');
  const config = obj(obj(client.config.environment).capabilities), limit = fileStagingLimit(config);
  if (captured.attachments.length + captured.existingAttachments.length > 100) throw new ClientError('You can attach up to 100 attachments per message.');
  const uploaded: Obj[] = [], minted: string[] = [], generation = client.generation, environmentId = client.environmentId;
  const current = () => {
    const edit = mobileQueuedEditLookup(owner, client);
    if (!edit || client.generation !== generation || client.environmentId !== environmentId) throw new ClientError('The queued edit changed.', 'superseded');
    return edit;
  };
  try {
    for (const file of captured.attachments) {
      current();
      const imageMime = queuedEditImageMime(file), image = file.kind === 'image' || !!imageMime;
      if (image && !imageMime) throw new ClientError(`Unsupported image type for '${file.name}'.`);
      if (image && file.sizeBytes > 10 * 1024 * 1024) throw new ClientError(`'${file.name}' is too large to attach. Images can be up to 10 MB.`);
      if (!image && (!limit || file.sizeBytes > limit)) throw new ClientError(!limit ? 'This server does not support file attachments.' : `'${file.name}' exceeds the attachment limit.`);
      const fields = { name: file.name, mimeType: image ? imageMime : file.mimeType, sizeBytes: file.sizeBytes };
      let id = file.uploadId;
      if (id) {
        try { await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: id } }); current(); }
        catch (error) { if (error instanceof ClientError && error.kind === 'AssetAttachmentNotFoundError') id = ''; else throw error; }
      }
      if (!id) {
        const bytes = await client.call(native, { op: file.kind === 'image' ? 'snapshotDraftRead' : 'composerAttachRead', id: file.id }); current();
        if (!str(bytes.base64) || Number(bytes.sizeBytes) !== file.sizeBytes) throw new ClientError(`'${file.name}' is no longer available. Attach it again.`);
        if (image && config.attachmentUploads !== true) {
          const persisted = await client.rpc(native, 'assets.persistChatAttachments', { threadId: captured.threadId, messageId: captured.messageId,
            attachments: [{ type: 'image', ...fields, dataUrl: `data:${fields.mimeType};base64,${str(bytes.base64)}` }] }, true);
          current(); const stored = arr(persisted.attachments);
          if (stored.length !== 1 || !str(stored[0]?.id)) throw new ClientError('The server did not persist the queued image.');
          id = str(stored[0]!.id);
        } else {
          const upload = await client.rpc(native, 'attachments.createUploadUrl', { ...fields, ...(image ? {} : { type: 'file' }) }, true); current();
          id = str(upload.attachmentId);
          if (!id || !str(upload.relativeUrl).startsWith('/api/attachments/upload/')) throw new ClientError('The server returned an invalid attachment upload.');
          minted.push(id);
          await client.call(native, { op: 'uploadAttachment', path: upload.relativeUrl, base64: bytes.base64, contentType: fields.mimeType }, generation, true); current();
        }
        const live = current();
        queuedEditState(client).sessions.set(owner, { ...live, revision: live.revision + 1,
          attachments: live.attachments.map(entry => entry.id === file.id ? { ...entry, uploadId: id, status: 'ready' } : entry) });
        await mobileQueuedEditPersist(owner, native, client); current();
      }
      uploaded.push({ type: image ? 'image' : 'file', id, ...fields });
    }
    current(); return uploaded;
  } catch (error) {
    // Delete only uploads this attempt minted and never delete IDs which were
    // durably adopted by the dedicated record. Retained server IDs are untouched.
    if (!letGo(error) && client.generation === generation && client.environmentId === environmentId) {
      const retained = new Set(mobileQueuedEditLookup(owner, client)?.attachments.map(file => file.uploadId));
      for (const id of minted) if (!retained.has(id)) {
        try { await client.rpc(native, 'attachments.delete', { attachmentId: id }, true); } catch { /* Pending uploads expire server-side. */ }
      }
    }
    throw error;
  }
}
