// Pinned365aa87982 attachmentUpload.ts and use-thread-outbox-drain.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fileStagingLimit } from './shared/composer-editor-files';
import { mobileSessionGrants } from './mobile-grants';
import { mobileOutboxCapture, mobileOutboxConfirmQueued, mobileOutboxSnapshot, mobileOutboxUpdate,
  type MobileOutboxCapture, type MobileOutboxOutcome } from './mobile-outbox';
import type { MobileOutboxAttachment, MobileOutboxRecord } from './mobile-outbox-model';
import { mobileComposerAttachmentWireKindAndMime, mobileUploadedAttachmentReference } from './mobile-attachment-policy';

/** Inline bytes exist only during preparation. Source materializes them inside
 * startThreadTurn, after context serialization. They are not adopted upload IDs. */
export type MobileOutboxUploadAttachment =
  { localId: string; kind: 'reference'; attachment: Obj } |
  { localId: string; kind: 'inline-image'; attachment: Obj };
export type MobileOutboxUploadResult =
  { status: 'ready'; capture: MobileOutboxCapture; record: MobileOutboxRecord;
    attachments: MobileOutboxUploadAttachment[]; pendingAttachmentIds: string[] } |
  { status: 'abandoned' | 'blocked'; reason: string } |
  { status: 'uncertain'; mutationId: string; reason: string };
const stale = () => new ClientError('The pending task or connection changed.', 'outbox-stale');
const sameCapture = (left: MobileOutboxCapture | null, right: MobileOutboxCapture) => !!left &&
  left.token === right.token && left.localRevision === right.localRevision && left.nativeRevision === right.nativeRevision;
const validId = (id: string) => id.length > 0 && id.length <= 128 && /^[a-z0-9_-]+$/i.test(id);

/** One caller-owned foreground pass. Existing native FIFO/CAS owns adoption;
 * this function retains no handle or Promise after returning and never sends a turn. */
export async function mobileOutboxPrepareAttachments(client: T3Client, handle: Native,
  captured: MobileOutboxCapture): Promise<MobileOutboxUploadResult> {
  const native = letGoAware(handle), generation = client.generation, activeOrigin = client.origin;
  const environmentId = client.environmentId, config = JSON.stringify(client.config), scopes = JSON.stringify(client.scopes);
  let capture = { ...captured }, adopted = false, mutation: MobileOutboxOutcome | null = null;
  let endpointVerified = false;
  const minted: string[] = [], prepared: MobileOutboxUploadAttachment[] = [], pending: string[] = [];
  const initial = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === capture.messageId);
  if (!initial) return { status: 'abandoned', reason: 'That pending task no longer exists.' };
  const record = initial.record, capabilities = obj(obj(client.config.environment).capabilities);
  const sameEndpoint = () => client.generation === generation && client.origin === activeOrigin && client.environmentId === environmentId
    && record.environmentId === environmentId && client.connection === 'connected';
  function current() {
    const snapshot = mobileOutboxSnapshot(client), row = snapshot.rows.find(row => row.record.messageId === capture.messageId);
    if (!sameEndpoint() || !client.configLive || JSON.stringify(client.config) !== config || JSON.stringify(client.scopes) !== scopes
      || !snapshot.complete || !row || row.held || row.status !== 'confirmed' || !sameCapture(mobileOutboxCapture(client, capture.messageId), capture)
      || snapshot.outcomes.some(outcome => outcome.messageId === capture.messageId && ['unknown', 'uncertain'].includes(outcome.status))) throw stale();
    return row;
  }
  async function confirm() {
    current();
    if (!await mobileOutboxConfirmQueued(client, native, capture)) throw stale();
    return current();
  }
  async function bytes(file: MobileOutboxAttachment) {
    current();
    const value = await client.call(native, { op: file.kind === 'image' ? 'snapshotDraftRead' : 'composerAttachRead', id: file.id }, generation);
    current();
    if (!str(value.base64) || value.sizeBytes !== file.sizeBytes) throw new ClientError(`'${file.name}' is no longer available. Attach it again.`);
    return str(value.base64);
  }
  async function cleanup() {
    // An unknown/uncertain CAS may already own every minted ID. Its existing
    // mutation status must settle before anyone decides to release those IDs.
    if (adopted || mutation && ['unknown', 'uncertain'].includes(mutation.status) || !endpointVerified) return;
    for (const id of minted) for (let attempt = 0; attempt < 2 && sameEndpoint(); attempt++) {
      try { await client.request(native, 'attachments.delete', { attachmentId: id }, generation, true); break; }
      catch (error) { if (letGo(error) || error instanceof ClientError && error.kind === 'AssetAttachmentNotFoundError') break; }
    }
  }
  try {
    await confirm();
    const status = await client.call(native, { op: 'status' }, generation); current();
    if (status.environmentId !== environmentId || status.origin !== activeOrigin || (str(status.homeOrigin) || activeOrigin) !== record.origin) throw stale();
    endpointVerified = true;
    const session = await client.http(native, '/api/auth/session', generation); current();
    if (!mobileSessionGrants(session, 'orchestration:operate')) throw new ClientError('This connection cannot upload attachments.');
    if (record.attachments.length > 100) throw new ClientError('You can attach up to 100 attachments per message.');
    const limit = fileStagingLimit(capabilities);
    let imageBytes = 0;
    // Source validates Files-selected images as files before promoting their wire kind.
    for (const file of record.attachments) {
      if (file.kind === 'file' && (!limit || file.sizeBytes > limit)) throw new ClientError(!limit
        ? 'This server does not support file attachments.' : `'${file.name}' exceeds the attachment limit.`);
      const wire = file.kind === 'image' && capabilities.attachmentUploads !== true
        ? { type: 'image', mimeType: file.mimeType } : mobileComposerAttachmentWireKindAndMime(file);
      if (wire.type === 'image') imageBytes += file.sizeBytes;
      if (!file.name || file.name.trim() !== file.name || file.name.length > 255 || !wire.mimeType || wire.mimeType.length > 100
        || !Number.isSafeInteger(file.sizeBytes) || file.sizeBytes < 1 || wire.type === 'image' && file.sizeBytes > 10 * 1024 * 1024)
        throw new ClientError(`'${file.name}' cannot be uploaded.`);
    }
    if (imageBytes > 80 * 1024 * 1024) throw new ClientError('The attached images exceed the message limit.');
    for (const file of record.attachments) {
      current();
      if (file.kind === 'image' && capabilities.attachmentUploads !== true) {
        const base64 = await bytes(file);
        prepared.push({ localId: file.id, kind: 'inline-image', attachment: { type: 'image', name: file.name,
          mimeType: file.mimeType, sizeBytes: file.sizeBytes, dataUrl: `data:${file.mimeType};base64,${base64}` } });
        continue;
      }
      const wire = mobileComposerAttachmentWireKindAndMime(file);
      let id = file.uploadId;
      if (id) {
        if (file.uploadEnvironmentId !== environmentId || !validId(id)) throw new ClientError('The saved upload does not belong to this environment.');
        try { await client.request(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: id } }, generation); current(); }
        catch (error) {
          current();
          if (error instanceof ClientError && error.kind === 'AssetAttachmentNotFoundError') id = '';
          else throw error;
        }
      }
      if (!id) {
        const base64 = await bytes(file);
        const upload = await client.request(native, 'attachments.createUploadUrl', { name: file.name, mimeType: wire.mimeType,
          sizeBytes: file.sizeBytes, ...(wire.type === 'file' ? { type: 'file' } : {}) }, generation, true);
        id = str(upload.attachmentId);
        // Remember a valid minted ID before checking whether the row changed.
        if (validId(id)) minted.push(id);
        current();
        if (!validId(id) || !str(upload.relativeUrl).startsWith('/api/attachments/upload/')) throw new ClientError('The server returned an invalid attachment upload.');
        await client.call(native, { op: 'uploadAttachment', path: upload.relativeUrl, base64, contentType: wire.mimeType,
          expectedOrigin: record.origin, expectedEnvironmentId: environmentId }, generation, true);
        current();
      }
      prepared.push({ localId: file.id, kind: 'reference', attachment: mobileUploadedAttachmentReference(file, id) });
      pending.push(id);
    }
    await confirm();
    const attachments = record.attachments.map((file, index) => prepared[index]!.kind === 'inline-image' ? file :
      { ...file, uploadId: str(prepared[index]!.attachment.id), uploadEnvironmentId: environmentId, status: 'ready' as const });
    if (JSON.stringify(attachments) !== JSON.stringify(record.attachments)) {
      mutation = await mobileOutboxUpdate(client, native, { ...record, attachments },
        { expectedToken: capture.token, expectedRevision: capture.nativeRevision! }, true);
      if (['unknown', 'uncertain'].includes(mutation.status)) return { status: 'uncertain', mutationId: mutation.mutationId,
        reason: 'Resolve the saved upload outcome before retrying.' };
      if (mutation.status !== 'committed') { await cleanup(); return { status: 'abandoned', reason: mutation.message || 'The pending task changed during upload.' }; }
      adopted = true;
      const next = mobileOutboxCapture(client, capture.messageId);
      if (!next || next.token !== mutation.mutationId) throw stale();
      capture = next;
    }
    const row = await confirm();
    return { status: 'ready', capture, record: row.record, attachments: prepared, pendingAttachmentIds: pending };
  } catch (error) {
    if (!letGo(error)) await cleanup();
    return { status: letGo(error) || error instanceof ClientError && error.kind === 'outbox-stale' ? 'abandoned' : 'blocked', reason: error instanceof Error ? error.message : String(error) };
  }
}
