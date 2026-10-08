// Pinned365aa87982 NewTaskFlowProvider: original pending identity, save before departure.
// @ref llp/1109.005-composer-and-transcript.decision.md#pending-task-editor-save-and-restart-recovery
import type { MobileDraftClient } from './mobile-draft-recovery';
import { ClientError, bridgeReply, type Native, type Files } from './shared/protocol';
import { letGo } from './shared/let-go';
import { obj, str } from './shared/domain';
import { queuedEditRefreshOrigin, mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileOutboxDriveSnapshot } from './mobile-outbox-drive';
import { mobileOutboxRead, mobileOutboxSnapshot } from './mobile-outbox';
import { mobilePendingTaskEditorKey, mobilePendingTaskEditorsSnapshot, type MobilePendingTaskMarker } from './mobile-pending-task-state';
import { mobilePendingTaskEditorOpen, mobilePendingTaskEditorSave, mobilePendingTaskEditorFinish,
  type MobilePendingTaskEditorResult } from './mobile-pending-task-editor';
import { mobileNewTaskDraftBind, mobileNewTaskDraftOwned, mobileNewTaskDraftBoundKey, mobileNewTaskDraftChoicesRestore,
  mobileNewTaskDraftUnbind, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';

export interface MobileNewTaskPendingRoute { environmentId: string; projectId: string; pendingTaskId: string }
export interface MobileNewTaskPendingInput { flowOwner: string; current(): boolean }
const stale = () => new ClientError('The pending task route changed.', 'superseded');
const stamp = (client: MobileDraftClient) => JSON.stringify([client.origin, mobileQueuedEditOrigin(client), client.environmentId,
  client.generation, client.projectId, client.threadId, client.threadEpoch]);
const ownerOf = (value: MobileOutboxWireOwner): MobileOutboxWireOwner => ({ origin: value.origin,
  environmentId: value.environmentId, threadId: value.threadId, messageId: value.messageId, commandId: value.commandId });

/** The URL selects saved ownership; it cannot supply command IDs or a new origin.
 * Foreign environments are focused from saved identity without awaiting a network connection. */
export async function mobileNewTaskPendingOpen(client: MobileDraftClient, native: Native, storage: Files,
  route: MobileNewTaskPendingRoute, input: MobileNewTaskPendingInput): Promise<MobilePendingTaskEditorResult> {
  const before = stamp(client), current = () => input.current() && stamp(client) === before;
  if (!route.pendingTaskId || !route.projectId || !route.environmentId || !current()) throw stale();
  if (!await mobileOutboxRead(client, native)) throw new ClientError('Read complete pending task storage before editing.');
  if (!current()) throw stale();
  const markers = mobilePendingTaskEditorsSnapshot(client);
  if (!markers.ready) throw new ClientError('Wait for saved pending editors to load.');
  const matches = markers.markers.filter(marker => marker.owner.messageId === route.pendingTaskId);
  const row = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === route.pendingTaskId);
  if (matches.length > 1) throw new ClientError('The saved pending task ownership is ambiguous.');
  const saved = matches[0], record = saved?.baseline.record ?? row?.record;
  if (!record?.creation || record.environmentId !== route.environmentId || record.creation.projectId !== route.projectId
    || saved && mobilePendingTaskEditorKey(saved.owner) !== mobilePendingTaskEditorKey(record))
    throw new ClientError('This pending task does not belong to the selected environment and project.');
  if (record.environmentId !== client.environmentId) {
    if (client.busy || client.pending || mobileOutboxDriveSnapshot(client, 0).busy)
      throw new ClientError('Wait for the current send before switching pending tasks.');
    const catalog = await bridgeReply(native, { op: 'environments' });
    if (!current()) throw stale();
    const entries = obj(catalog.value).saved;
    const destination = catalog.ok && Array.isArray(entries) ? entries.map(obj).filter(entry => entry.enabled !== false
      && entry.environmentId === record.environmentId && (entry.homeOrigin ?? entry.origin) === record.origin) : [];
    if (destination.length !== 1 || !str(destination[0].origin))
      throw new ClientError('Reconnect the saved environment before editing this pending task.');
    // Only a currently paired exact home may be focused. A queue URL or stale
    // row cannot nominate credentials, a replacement home, or another project.
    const reply = await bridgeReply(native, { op: 'mobileSelectSavedEnvironment', origin: record.origin,
      environmentId: record.environmentId, generation: client.generation });
    if (!current()) throw stale();
    if (!reply.ok) throw new ClientError(reply.error?.message || 'Could not open the saved environment.');
    client.adoptStatus(obj(reply.value), reply.generation);
    if (client.environmentId !== record.environmentId) throw new ClientError('The saved environment identity changed.');
    const focused = JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch]);
    await queuedEditRefreshOrigin(native, client);
    if (!input.current() || focused !== JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch]))
      throw stale();
  }
  if (record.origin !== mobileQueuedEditOrigin(client))
    throw new ClientError('This pending task does not belong to the selected environment and project.');
  // openDraft performs existing subscription teardown. No project is fabricated,
  // no checkout occurs, and its temporary defaults never replace captured choices.
  mobileNewTaskDraftUnbind(client);
  const endpoint = JSON.stringify([client.origin, mobileQueuedEditOrigin(client), client.environmentId, client.generation]);
  const checkEndpoint = () => {
    if (!input.current() || endpoint !== JSON.stringify([client.origin, mobileQueuedEditOrigin(client), client.environmentId, client.generation])) throw stale();
  };
  const scoped: Native = { available: native.available, watch(topic) { checkEndpoint(); native.watch(topic); }, async later(request) {
    checkEndpoint(); const value = await native.later(request); checkEndpoint(); return value;
  } };
  const opening = client.openDraft(scoped, route.projectId), openingSelection = stamp(client);
  await opening;
  if (!input.current() || stamp(client) !== openingSelection) throw stale();
  client.projectId = route.projectId;
  const selected = stamp(client), selectedCurrent = () => input.current() && stamp(client) === selected;
  const result = await mobilePendingTaskEditorOpen(client, native, storage, { owner: ownerOf(record), current: selectedCurrent });
  if (!selectedCurrent()) throw stale();
  if (result.status === 'ready' && result.marker) {
    if (!mobileNewTaskDraftBind(client, result.marker.draftKey, input.flowOwner)) throw stale();
    mobileNewTaskDraftChoicesRestore(client, result.marker.draftKey);
  }
  return result;
}

/** Save while bound, then finish under a new unbound scope. Only finished permits
 * navigation. A late answer never rebinds a successor flow. */
export async function mobileNewTaskPendingClose(client: MobileDraftClient, native: Native, storage: Files,
  marker: MobilePendingTaskMarker, input: MobileNewTaskPendingInput): Promise<MobilePendingTaskEditorResult> {
  const selected = stamp(client), current = () => input.current() && stamp(client) === selected;
  const bound = () => current() && mobileNewTaskDraftOwned(client, marker.draftKey, input.flowOwner);
  if (!bound()) throw stale();
  const saved = await mobilePendingTaskEditorSave(client, native, storage, { owner: marker.owner, expected: marker, current: bound });
  if (!bound()) throw stale();
  if (saved.status !== 'saved' || !saved.marker || !saved.fingerprint) return saved;
  mobileNewTaskDraftUnbind(client, input.flowOwner);
  if (mobileNewTaskDraftBoundKey(client)) throw stale();
  const unbound = () => current() && !mobileNewTaskDraftBoundKey(client);
  let repair = true;
  try {
    return await mobilePendingTaskEditorFinish(client, native, storage,
      { owner: saved.marker.owner, expected: saved.marker, fingerprint: saved.fingerprint, current: unbound });
  } catch (error) {
    if (letGo(error)) repair = false;
    throw error;
  } finally {
    // Successful Finish removed the metadata. Failed/lost completion retains it.
    // Rebinding is permitted only for this exact still-current route and session.
    const live = mobilePendingTaskEditorsSnapshot(client).markers.find(item => mobilePendingTaskEditorKey(item.owner) === mobilePendingTaskEditorKey(marker.owner));
    if (repair && unbound() && live?.session === saved.marker.session && mobileNewTaskDraftLookup(client, live.draftKey)) {
      if (mobileNewTaskDraftBind(client, live.draftKey, input.flowOwner)) mobileNewTaskDraftChoicesRestore(client, live.draftKey);
    }
  }
}
