// Pinned365aa87982 NewTaskDraftScreen: restore the pre-import draft before release.
// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import type { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileOutboxRecoveryDraftCapture as capture, mobileOutboxRecoveryDraftAdopt as adopt } from './mobile-outbox-recovery-draft';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftStore, mobileNewTaskDraftChoicesRestore } from './mobile-new-task-drafts';
import { mobileNewTaskTransferGuardAcquire, mobileNewTaskTransferGuardRead, mobileNewTaskTransferGuardAssert, mobileNewTaskTransferGuardRelease } from './new-task-transfer-guard';
import { mobileIncomingShareImports, mobileIncomingShareImportForget } from './incoming-share-imports';
import { incomingShareEntry, incomingShareObject, incomingShareUUID, type IncomingShareEntry } from './incoming-share-model';

type Ownership = { entry: IncomingShareEntry; key: string; baseline: Obj; local: Obj };
const ownership = new WeakMap<T3Client, Map<string, Ownership>>();
/** Plain values only. A failed write can leave native with the baseline and local
 * with the merge; this snapshot allows cancellation without overwriting disk. */
export function mobileIncomingShareCancelCapture(client: T3Client, entry: IncomingShareEntry, adoptionId: string, baseline: Obj): void {
  let records = ownership.get(client); if (!records) { records = new Map(); ownership.set(client, records); }
  if (records.has(adoptionId)) return;
  const key = mobileNewTaskDraftCurrent(client)!.key;
  records.set(adoptionId, { entry, key, baseline: JSON.parse(JSON.stringify(baseline)), local: capture(client, key) });
}
export function mobileIncomingShareCancelMerged(client: T3Client, adoptionId: string): void {
  const saved = ownership.get(client)?.get(adoptionId); if (saved) saved.local = capture(client, saved.key);
}
export function mobileIncomingShareCancelForget(client: T3Client, adoptionId: string): void { ownership.get(client)?.delete(adoptionId); }
function restoredProjection(raw: unknown, destination: { draftKey: string; environmentId: string; projectId: string; origin: string }): Obj {
  const fields = ['text', 'images', 'files', 'metadata', 'staged', 'workspace', 'order', 'recovered'];
  const invalid = () => { throw new ClientError('The share inbox returned an invalid restored draft.', 'protocol'); };
  if (!incomingShareObject(raw) || Object.keys(raw).length !== fields.length || fields.some(field => !Object.hasOwn(raw, field))) return invalid();
  const metadata = obj(raw.metadata);
  if (typeof raw.text !== 'string' || !Array.isArray(raw.images) || !raw.images.every(incomingShareObject)
    || !Array.isArray(raw.files) || !raw.files.every(file => incomingShareObject(file) && file.draftKey === destination.draftKey)
    || !Array.isArray(raw.order) || !raw.order.every(id => typeof id === 'string') || !incomingShareObject(raw.recovered)
    || Object.values(raw.recovered).some(value => !incomingShareObject(value) || value.key !== destination.draftKey)
    || ['staged', 'workspace'].some(field => raw[field] !== null && !incomingShareObject(raw[field]))
    || metadata.key !== destination.draftKey || ['environmentId', 'projectId', 'origin'].some(field => metadata[field] !== destination[field as keyof typeof destination])
    || !Number.isSafeInteger(metadata.revision) || Number(metadata.revision) < 0 || typeof metadata.createdAt !== 'string') return invalid();
  return JSON.parse(JSON.stringify(raw));
}
export interface IncomingShareCancellationResult { status: 'cancelled' | 'retained'; message: string; revision: number }
/** Native restores and fsyncs one captured draft under its preference mutex. The
 * client adopts that reply with a local CAS, never a stale whole-document save. */
export async function mobileIncomingShareCancel(client: T3Client, nativeInput: Native, _storage: Files,
  input: { entry: IncomingShareEntry; adoptionId: string; current: () => boolean }): Promise<IncomingShareCancellationResult> {
  const result = (status: IncomingShareCancellationResult['status'], message = '') => ({ status, message, revision: client.revision });
  const draft = mobileNewTaskDraftCurrent(client);
  if (!client.preferencesLoaded || !draft || mobileNewTaskDraftIsPendingKey(draft.key) || client.threadId || client.busy || client.pending)
    return result('retained', 'Choose this share’s new-task draft before cancelling its import.');
  if (!nativeInput.available) return result('retained', 'Open T3 Code on your iPhone or iPad to cancel this import.');
  const lease = mobileNewTaskTransferGuardAcquire(client, draft.key);
  if (!lease) return result('retained', 'Wait for this draft’s current operation to finish.');
  const native = letGoAware(nativeInput), before = capture(client, draft.key);
  const destination = { draftKey: draft.key, environmentId: draft.environmentId, projectId: draft.projectId, origin: draft.origin };
  const stamp = canonical([client.generation, client.threadEpoch, destination]);
  let checked = false;
  const assertCurrent = () => {
    const current = mobileNewTaskDraftCurrent(client);
    if (!input.current() || !current || client.threadId || canonical(before) !== canonical(capture(client, draft.key))
      || stamp !== canonical([client.generation, client.threadEpoch, { draftKey: current.key, environmentId: current.environmentId, projectId: current.projectId, origin: current.origin }]))
      throw new ClientError('The draft changed during cancellation. Its newer content is retained.', 'superseded');
    if (checked) mobileNewTaskTransferGuardAssert(client, lease);
  };
  try {
    const entry = incomingShareEntry(input.entry);
    if (!incomingShareUUID(input.adoptionId)) throw new ClientError('The share import identity is invalid.', 'protocol');
    assertCurrent();
    const store = mobileNewTaskDraftStore(client);
    if (Object.values(store.claims).includes(draft.key) || Object.values(store.receipts).some(raw => obj(raw).key === draft.key))
      throw new ClientError('Resolve this draft’s captured launch before cancelling its share import.');
    const receipts = mobileIncomingShareImports(client), receipt = receipts[draft.key]?.[input.adoptionId];
    if (receipt && (receipt.shareId !== entry.id || receipt.instanceId !== entry.instanceId || canonical(receipt.destination) !== canonical(destination)))
      throw new ClientError('The saved import belongs to different shared content.', 'protocol');
    const saved = ownership.get(client)?.get(input.adoptionId);
    if (saved && (saved.key !== draft.key || canonical(saved.entry) !== canonical(entry) || canonical(saved.local) !== canonical(before)))
      throw new ClientError('This draft has changed since import. Keep its newer content before cancelling.');
    const receiptStamp = canonical(receipts);
    await mobileNewTaskTransferGuardRead(client, native, lease, input.current); checked = true; assertCurrent();
    if (receiptStamp !== canonical(mobileIncomingShareImports(client))) throw new ClientError('The share import ownership changed.', 'superseded');
    const response = await bridgeReply(native, { op: 'mobileIncomingShares', action: 'cancel', shareId: entry.id,
      adoptionId: input.adoptionId, destination, expectedDraft: saved?.baseline ?? before });
    assertCurrent();
    if (receiptStamp !== canonical(mobileIncomingShareImports(client))) throw new ClientError('The share import ownership changed.', 'superseded');
    if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
    const value = obj(response.value);
    if (value.adoptionId !== input.adoptionId || value.cancelled !== true || value.consumed !== false)
      throw new ClientError('The share import could not be cancelled. Its draft and inbox ownership are retained.');
    const after = restoredProjection(value.restoredDraft, destination);
    if (!adopt(client, { key: draft.key, origin: draft.origin, environmentId: draft.environmentId, before, after, terminalBefore: {} }))
      throw new ClientError('The draft changed during cancellation. Its newer content is retained.', 'superseded');
    mobileIncomingShareImportForget(client, draft.key, input.adoptionId);
    mobileNewTaskDraftChoicesRestore(client, draft.key);
    ownership.get(client)?.delete(input.adoptionId);
    return result('cancelled');
  } catch (error) {
    if (letGo(error)) throw error;
    return result('retained', error instanceof Error ? error.message : 'The share import could not be cancelled.');
  } finally { mobileNewTaskTransferGuardRelease(client, lease); client.revision++; }
}
