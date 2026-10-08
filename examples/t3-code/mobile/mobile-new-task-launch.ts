// Captured-draft cleanup around the shared sender, not a second pending/outbox owner.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client, Pending } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileNewTaskDraftStore, mobileNewTaskDraftClone, mobileNewTaskDraftCurrent, mobileNewTaskDraftBoundKey,
  mobileNewTaskDraftHasContent, mobileNewTaskDraftRemoveMetadata, mobileNewTaskDraftQueueFiles, mobileNewTaskDraftIsKey } from './mobile-new-task-drafts';

interface Receipt {
  version: 1; key: string; environmentId: string; origin: string; projectId: string; threadId: string;
  commandId: string; revision: number; text: string; payload: string; images: Obj[]; files: DraftFile[];
}
export interface MobileNewTaskLaunchCapture { slot: string; key: string; environmentId: string; generation: number; origin: string; retry: boolean }
const active = new WeakMap<T3Client, Set<string>>();
const slots = (client: T3Client) => { let slots = active.get(client); if (!slots) { slots = new Set(); active.set(client, slots); } return slots; };
const slotFor = (environmentId: string, pending: Pending) => JSON.stringify([environmentId, str(pending.payload.commandId)]);
const launch = (pending: Pending) => pending.method === 'orchestration.launchThread';
const equal = (a: unknown, b: unknown) => canonical(a) === canonical(b);
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().filter(key => (value as Record<string, unknown>)[key] !== undefined)
    .map(key => `${JSON.stringify(key)}:${canonical((value as Record<string, unknown>)[key])}`).join(',')}}`;
  return JSON.stringify(value) ?? 'null';
}
function receipt(client: T3Client, pending: Pending, environmentId: string): Receipt | null {
  const store = mobileNewTaskDraftStore(client), slot = slotFor(environmentId, pending), raw = obj(store.receipts[slot]), record = store.records[str(raw.key)];
  if (!launch(pending) || raw.version !== 1 || !mobileNewTaskDraftIsKey(str(raw.key)) || store.claims[slot] !== raw.key
    || raw.environmentId !== environmentId || raw.origin !== mobileQueuedEditOrigin(client) || raw.commandId !== pending.payload.commandId
    || raw.projectId !== pending.payload.projectId || raw.threadId !== pending.payload.threadId || raw.payload !== canonical(pending.payload)
    || pending.threadId !== pending.payload.threadId || obj(pending.payload.initialMessage).text !== pending.text
    || raw.text !== pending.text || !Number.isSafeInteger(raw.revision) || Number(raw.revision) < 0
    || !Array.isArray(raw.images) || !Array.isArray(raw.files) || !record || record.environmentId !== environmentId
    || record.projectId !== raw.projectId || record.origin !== raw.origin) return null;
  const sent = arr(obj(pending.payload.initialMessage).attachments);
  if (arr(raw.images).length !== raw.images.length || arr(raw.files).length !== raw.files.length
    || arr(raw.images).some(image => !str(image.id) || !str(image.uploadId) || !sent.some(file => file.id === image.uploadId))
    || arr(raw.files).some(file => !str(file.id) || file.draftKey !== raw.key || file.environmentId !== environmentId
      || !str(file.attachmentId) || !sent.some(sent => sent.id === file.attachmentId))) return null;
  return raw as unknown as Receipt;
}
export function mobileNewTaskLaunchSlotEnvironment(slot: string): string | null {
  try { const value: unknown = JSON.parse(slot); return Array.isArray(value) && value.length === 2 && typeof value[0] === 'string' && typeof value[1] === 'string' ? value[0] : null; }
  catch { return null; }
}
function owned(client: T3Client, pending: Pending, environmentId: string): boolean {
  const store = mobileNewTaskDraftStore(client), slot = slotFor(environmentId, pending);
  return launch(pending) && (slot in store.receipts || slot in store.claims
    || [...Object.keys(store.receipts), ...Object.keys(store.claims)].some(key => mobileNewTaskLaunchSlotEnvironment(key) === environmentId));
}
const invalid = () => new ClientError('The saved draft launch cannot be verified. Its draft and pending operation have been retained.');
export function mobileNewTaskLaunchPrepare(client: T3Client, pending: Pending): MobileNewTaskLaunchCapture | null {
  if (!launch(pending)) return null;
  const environmentId = client.environmentId, slot = slotFor(environmentId, pending), store = mobileNewTaskDraftStore(client);
  const prior = client.local.pending[environmentId], retry = !!prior && prior.method === pending.method && prior.payload.commandId === pending.payload.commandId;
  let saved = receipt(client, pending, environmentId);
  if (owned(client, pending, environmentId)) {
    if (!saved || !retry || !equal(prior?.payload, pending.payload)) throw invalid();
  } else {
    const bound = mobileNewTaskDraftBoundKey(client), current = mobileNewTaskDraftCurrent(client);
    if (!bound) return null;
    if (!current || retry || prior || !str(pending.payload.commandId) || !str(pending.payload.threadId)
      || pending.threadId !== pending.payload.threadId || obj(pending.payload.initialMessage).text !== pending.text
      || pending.payload.projectId !== current.projectId || pending.text !== (client.local.drafts[current.key] ?? '')) throw invalid();
    const sent = arr(obj(pending.payload.initialMessage).attachments);
    saved = { version: 1, key: current.key, environmentId, origin: current.origin, projectId: current.projectId,
      threadId: str(pending.payload.threadId), commandId: str(pending.payload.commandId), revision: current.revision, text: pending.text,
      payload: canonical(pending.payload), images: mobileNewTaskDraftClone((client.local.snapshotDrafts[current.key] ?? []).filter(image => sent.some(file => file.id === image.uploadId))),
      files: mobileNewTaskDraftClone(draftFiles(client.local).filter(file => file.draftKey === current.key && sent.some(sent => sent.id === file.attachmentId))) };
    store.receipts[slot] = saved; store.claims[slot] = current.key;
  }
  if (slots(client).has(environmentId)) throw new ClientError('This draft launch is already in progress.');
  slots(client).add(environmentId);
  return { slot, key: saved.key, environmentId, generation: client.generation, origin: client.origin, retry };
}
/** Recheck after the shared durable save and immediately before its request. */
export function mobileNewTaskLaunchBeforeRequest(client: T3Client, pending: Pending, capture: MobileNewTaskLaunchCapture): void {
  if (client.environmentId !== capture.environmentId || client.generation !== capture.generation || client.origin !== capture.origin
    || !receipt(client, pending, capture.environmentId) || (!capture.retry && mobileNewTaskDraftCurrent(client)?.key !== capture.key))
    throw new ClientError('The draft launch owner changed before dispatch.', 'superseded');
}
export function mobileNewTaskLaunchEnd(client: T3Client, capture: MobileNewTaskLaunchCapture): void {
  slots(client).delete(capture.environmentId);
  const store = mobileNewTaskDraftStore(client);
  // Missing pending is not success. Only release the unused receipt, never content.
  if (!client.local.pending[capture.environmentId] && capture.slot in store.receipts) {
    delete store.receipts[capture.slot]; delete store.claims[capture.slot];
  }
}
/** Local new-flow selection may proceed while this validated captured launch waits. */
export function mobileNewTaskLaunchPendingOwned(client: T3Client): boolean {
  return !!client.pending && !!receipt(client, client.pending, client.environmentId);
}
/** False holds malformed/foreign independent records without invoking default cleanup. */
export function mobileNewTaskLaunchCanReconcile(client: T3Client): boolean {
  const pending = client.pending;
  if (!pending || !owned(client, pending, client.environmentId)) return true;
  if (slots(client).has(client.environmentId)) return false;
  if (receipt(client, pending, client.environmentId)) return true;
  client.error = invalid().message; return false;
}
/** Return true for every owned launch, even invalid ones. Never fall through to
 * project-slot cleanup, and never throw inside the shared ACK path. */
export function mobileNewTaskLaunchFinish(client: T3Client, pending: Pending, environmentId: string): boolean {
  if (!owned(client, pending, environmentId)) return false;
  const saved = receipt(client, pending, environmentId), currentPending = client.local.pending[environmentId];
  if (!saved || !currentPending || currentPending.method !== pending.method || !equal(currentPending.payload, pending.payload)) {
    client.error = invalid().message; return true;
  }
  const store = mobileNewTaskDraftStore(client), record = store.records[saved.key]!;
  if (record.revision === saved.revision && client.local.drafts[saved.key] === saved.text) delete client.local.drafts[saved.key];
  client.local.snapshotDrafts[saved.key] = (client.local.snapshotDrafts[saved.key] ?? []).filter(image => {
    if (!saved.images.some(sent => equal(sent, image))) return true;
    client.local.snapshotReleases.push(str(image.id)); return false;
  });
  const removed = draftFiles(client.local).filter(file => saved.files.some(sent => equal(sent, file)));
  setDraftFiles(client.local, draftFiles(client.local).filter(file => !removed.includes(file))); mobileNewTaskDraftQueueFiles(client, removed);
  if (!mobileNewTaskDraftHasContent(client, saved.key)) mobileNewTaskDraftRemoveMetadata(client, saved.key);
  else record.revision++;
  const uncertain = `${pending.description} may have reached T3. Check the synchronized thread, then retry only if needed.`;
  if (client.error === uncertain) client.error = '';
  delete client.local.pending[environmentId]; delete store.receipts[slotFor(environmentId, pending)]; delete store.claims[slotFor(environmentId, pending)];
  client.revision++; return true;
}
/** Pending captured bytes remain owned even if the editor removes their chip. */
export function mobileNewTaskLaunchProtectedImages(client: T3Client): Set<string> {
  return new Set(Object.values(mobileNewTaskDraftStore(client).receipts).flatMap(raw => arr(obj(raw).images).map(image => str(image.id))));
}
export async function mobileNewTaskDraftFlushFiles(client: T3Client, native: Native, storage: Files): Promise<void> {
  const store = mobileNewTaskDraftStore(client); if (!store.fileReleases.length) return;
  try { await client.persist(storage); } catch { return; }
  const referenced = new Set([...draftFiles(client.local).map(file => file.id),
    ...Object.values(store.receipts).flatMap(raw => arr(obj(raw).files).map(file => str(file.id)))]);
  for (const id of [...new Set(store.fileReleases)]) {
    if (referenced.has(id)) continue;
    try { await client.call(native, { op: 'composerAttachRemove', id }); store.fileReleases = store.fileReleases.filter(value => value !== id); }
    catch { /* Keep the durable retry until a later existing refresh. */ }
  }
  try { await client.persist(storage); } catch { /* A repeated local removal is harmless. */ }
}
