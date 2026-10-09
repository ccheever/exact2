// Pinned365aa87982 NewTaskDraftScreen import and use-composer-drafts merge.
// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import type { T3Client } from './shared/client';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { contextId } from './shared/composer-editor-menu';
import { draftFiles, fileChipLink, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { mobileDraftAttachmentIds, mobileDraftAttachmentRecord } from './draft-attachment-order';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftPresentation, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskContextGuard, mobileNewTaskContextRead, mobileNewTaskContextWrite } from './mobile-new-task-context';
import { mobileNewTaskTransferGuardAcquire, mobileNewTaskTransferGuardRead, mobileNewTaskTransferGuardAssert, mobileNewTaskTransferGuardRelease } from './new-task-transfer-guard';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { incomingShareAttachment, incomingShareEntry, incomingShareUUID, incomingShareSelection, incomingShareMergedText,
  type IncomingShareEntry, type IncomingShareDestination, type IncomingShareImport } from './incoming-share-model';
import { mobileIncomingShareImports, mobileIncomingShareImportRemember } from './incoming-share-imports';

async function invoke(native: Native, action: string, input: Record<string, unknown>) {
  const response = await bridgeReply(native, { op: 'mobileIncomingShares', action, ...input });
  if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
  const value = obj(response.value);
  if (!incomingShareUUID(value.adoptionId) || typeof value.consumed !== 'boolean' || !Array.isArray(value.attachments))
    throw new ClientError('The share inbox returned an invalid adoption.', 'protocol');
  return { adoptionId: value.adoptionId, consumed: value.consumed,
    entry: value.entry === null ? null : incomingShareEntry(value.entry), attachments: value.attachments.map(incomingShareAttachment) };
}
export interface IncomingShareAdoptionResult { status: 'imported' | 'pending' | 'retained'; message: string; warnings: string[]; revision: number }
/** The caller owns the route. This operation additionally captures the exact draft,
 * holds the existing transfer lease, and uses only this answer's native handles.
 * A failed/abandoned save retains both the inbox and the local receipt for retry. */
export async function mobileIncomingShareAdopt(client: T3Client, nativeInput: Native, storage: Files,
  input: { entry: IncomingShareEntry; current: () => boolean }): Promise<IncomingShareAdoptionResult> {
  const result = (status: IncomingShareAdoptionResult['status'], message = '', warnings: string[] = []) => ({ status, message, warnings, revision: client.revision });
  const draft = mobileNewTaskDraftCurrent(client);
  if (!client.preferencesLoaded || !draft || mobileNewTaskDraftIsPendingKey(draft.key) || client.threadId || client.busy || client.pending)
    return result('retained', 'Choose an available new-task draft before importing shared content.');
  if (!nativeInput.available) return result('retained', 'Open T3 Code on your iPhone or iPad to import shared content.');
  const destination: IncomingShareDestination = { draftKey: draft.key, environmentId: draft.environmentId, projectId: draft.projectId, origin: draft.origin };
  const lease = mobileNewTaskTransferGuardAcquire(client, draft.key);
  if (!lease) return result('retained', 'Wait for this draft’s current operation to finish.');
  const native = letGoAware(nativeInput), stamp = canonical([client.generation, client.threadEpoch, destination]);
  let expected = canonical(mobileNewTaskDraftPresentation(client, draft.key));
  let checked = false;
  const assertCurrent = () => {
    const current = mobileNewTaskDraftCurrent(client);
    if (!input.current() || !current || current.key !== draft.key || client.threadId
      || stamp !== canonical([client.generation, client.threadEpoch, { draftKey: current.key, environmentId: current.environmentId, projectId: current.projectId, origin: current.origin }])
      || expected !== canonical(mobileNewTaskDraftPresentation(client, draft.key)))
      throw new ClientError('The draft changed during share import. Its inbox copy is retained.', 'superseded');
    if (checked) mobileNewTaskTransferGuardAssert(client, lease);
  };
  try {
    const entry = incomingShareEntry(input.entry);
    assertCurrent();
    const receipts = mobileIncomingShareImports(client)[draft.key] ?? {};
    const context = mobileNewTaskContextRead(client, draft.key);
    if (!context.ok) throw new ClientError(context.error);
    const held = mobileNewTaskDraftStore(client);
    if (Object.values(held.claims).includes(draft.key) || Object.values(held.receipts).some(raw => obj(raw).key === draft.key))
      throw new ClientError('Resolve this draft’s captured launch before importing shared content.');
    const existingIds = [...(client.local.snapshotDrafts[draft.key] ?? []).map(image => String(image.id)),
      ...draftFiles(client.local).filter(file => file.draftKey === draft.key).map(file => file.id)];
    const selection = incomingShareSelection(entry, client.configLive ? client.config : null, existingIds);
    const previous = Object.values(receipts).find(receipt => receipt.shareId === entry.id && receipt.instanceId === entry.instanceId);
    if (selection.status === 'pending' && !previous) return result('pending', 'Waiting for this server’s file attachment support.');
    await mobileNewTaskTransferGuardRead(client, native, lease, input.current); checked = true; assertCurrent();
    if (previous) {
      if (previous.createdAt !== entry.createdAt || canonical(previous.destination) !== canonical(destination))
        throw new ClientError('The saved share import belongs to another draft.', 'protocol');
      const request = { shareId: entry.id, adoptionId: previous.adoptionId, destination };
      let consumed;
      try { consumed = await invoke(native, 'consume', request); }
      catch (error) {
        if (letGo(error)) throw error;
        assertCurrent();
        // A failed first write may leave only a local receipt. Native verifies
        // its complete draft projection before admitting this retry write.
        await client.persist(storage); assertCurrent();
        consumed = await invoke(native, 'consume', request);
      }
      assertCurrent();
      if (!consumed.consumed || consumed.adoptionId !== previous.adoptionId) throw new ClientError('The saved share import needs inbox cleanup.');
      return result('imported', '', selection.status === 'ready' ? selection.warnings : entry.warnings);
    }
    if (selection.status !== 'ready') return result('pending', 'Waiting for this server’s file attachment support.');
    const reserved = await invoke(native, 'reserve', { shareId: entry.id, destination }); assertCurrent();
    if (reserved.consumed || canonical(reserved.entry) !== canonical(entry)) throw new ClientError('The shared content changed. Reopen the inbox item.');
    const request = { shareId: entry.id, adoptionId: reserved.adoptionId, destination };
    {
      const alreadyImported = Object.values(receipts).some(receipt => receipt.shareId === entry.id);
      const selected = alreadyImported ? [] : selection.attachments;
      const staged = await invoke(native, 'stage', { ...request, attachmentIds: selected.map(file => file.id) }); assertCurrent();
      if (staged.adoptionId !== reserved.adoptionId || staged.consumed || canonical(staged.entry) !== canonical(entry)
        || canonical(staged.attachments) !== canonical(selected)) throw new ClientError('The shared attachment ownership changed.', 'protocol');
      const freshSelection = incomingShareSelection(entry, client.configLive ? client.config : null, existingIds);
      if (freshSelection.status !== 'ready' || canonical(freshSelection.attachments) !== canonical(selection.attachments))
        throw new ClientError('Server attachment support changed. Retry the share import.');
      const images = selected.filter(file => file.kind === 'image').map(({ kind: _kind, ...file }) => file);
      const files: DraftFile[] = selected.filter(file => file.kind === 'file').map(({ kind: _kind, ...file }) => ({ ...file,
        draftKey: draft.key, environmentId: draft.environmentId, contextId: contextId('file', file.id), source: 'attached', attachmentId: '', status: 'staged' }));
      let text = alreadyImported ? client.local.drafts[draft.key] ?? '' : incomingShareMergedText(client.local.drafts[draft.key] ?? '', entry.text);
      if (files.length) text = `${text}${text && !/\s$/.test(text) ? ' ' : ''}${files.map(fileChipLink).join(' ')} `;
      if (text.length > 1_000_000) throw new ClientError('The combined draft exceeds 1,000,000 characters. The inbox copy is retained.');
      const order = mobileDraftAttachmentIds(client, draft.key);
      const guard = mobileNewTaskContextGuard(client, draft.key);
      if (!guard || !mobileNewTaskContextWrite(client, guard, text)) throw new ClientError('The draft changed before import.');
      client.local.snapshotDrafts[draft.key] = [...(client.local.snapshotDrafts[draft.key] ?? []), ...images];
      setDraftFiles(client.local, [...draftFiles(client.local), ...files]);
      mobileDraftAttachmentRecord(client, draft.key, [...order, ...selected.map(file => file.id)]);
      const receipt: IncomingShareImport = { version: 1, shareId: entry.id, instanceId: entry.instanceId, adoptionId: reserved.adoptionId,
        createdAt: entry.createdAt, destination, attachmentIds: selected.map(file => file.id) };
      mobileIncomingShareImportRemember(client, receipt);
      expected = canonical(mobileNewTaskDraftPresentation(client, draft.key));
      await client.persist(storage); assertCurrent();
    }
    const consumed = await invoke(native, 'consume', request); assertCurrent();
    if (!consumed.consumed || consumed.adoptionId !== reserved.adoptionId) throw new ClientError('The share import is saved. Retry inbox cleanup.');
    return result('imported', '', selection.warnings);
  } catch (error) {
    if (letGo(error)) throw error;
    return result('retained', error instanceof Error ? error.message : 'The shared content could not be imported.');
  } finally { mobileNewTaskTransferGuardRelease(client, lease); client.revision++; }
}
