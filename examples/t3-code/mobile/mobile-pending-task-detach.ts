// @ref llp/1109.005-composer-and-transcript.decision.md#separate-draft-for-newer-retained-edits
import type { MobileDraftClient } from './mobile-draft-recovery';
import { obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
import { branchState } from './shared/r4-git-branch';
import { draftFiles, setDraftFiles } from './shared/composer-editor-files';
import { mobileNewTaskDraftBoundKey, mobileNewTaskDraftIsKey, mobileNewTaskDraftLookup, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobilePendingTaskDraftFingerprint } from './mobile-pending-task-draft';
import { mobilePendingTaskEditorKey, mobilePendingTaskEditorsRemove, mobilePendingTaskEditorsSnapshot,
  type MobilePendingTaskMarker, type MobilePendingTaskExpected } from './mobile-pending-task-state';

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const keyFor = (session: string) => `new-task:restored-editor-${session}`;
const contextOwner = (input: MobilePendingTaskExpected) => `outbox-editor:${JSON.stringify([input.owner.origin, input.owner.environmentId, input.owner.commandId, input.session])}`;
const changed = () => new ClientError('The newer saved editor changed. Reopen it before recovering.');

/** Retry after a lost preference reply finds an existing move, never recreates it.
 * The caller still has to verify the original completed native handoff. */
export function mobilePendingTaskDetachedKey(client: MobileDraftClient, input: MobilePendingTaskExpected): string {
  const key = keyFor(input.session), context = obj(obj(obj(client.local).mobileRecoveredDrafts)[contextOwner(input)]);
  const source = obj(context.pendingEditorDetached), draft = mobileNewTaskDraftLookup(client, key);
  return draft && context.key === key && draft.origin === input.owner.origin && draft.environmentId === input.owner.environmentId
    && context.origin === draft.origin && context.environmentId === draft.environmentId && obj(context.creation).projectId === draft.projectId
    && source.session === input.session && source.draftKey === `new-task:pending-${input.owner.messageId}` && str(source.fingerprint) ? key : '';
}

/** One synchronous local move precedes one atomic preference write. No queue or
 * receipt is changed. A failed write leaves the moved content live for retry;
 * a cold restart sees either the original editor or the complete ordinary draft. */
export function mobilePendingTaskDetach(client: MobileDraftClient, marker: MobilePendingTaskMarker, fingerprint: string): string {
  const live = mobilePendingTaskEditorsSnapshot(client).markers.find(item => mobilePendingTaskEditorKey(item.owner) === mobilePendingTaskEditorKey(marker.owner));
  if (!client.preferencesLoaded || !live || live.session !== marker.session || live.revision !== marker.revision || live.pending
    || mobileNewTaskDraftBoundKey(client) || client.pendingTaskCleanup || mobilePendingTaskDraftFingerprint(client, marker.draftKey) !== fingerprint) throw changed();
  const key = keyFor(marker.session), sourceKey = marker.draftKey, store = mobileNewTaskDraftStore(client), metadata = store.records[sourceKey];
  if (!mobileNewTaskDraftIsKey(key) || !metadata || metadata.origin !== marker.owner.origin || metadata.environmentId !== marker.owner.environmentId
    || metadata.projectId !== marker.baseline.record.creation?.projectId || metadata.createdAt !== marker.baseline.record.createdAt)
    throw new ClientError('The newer editor has incomplete saved ownership. Keep it for recovery.');
  const controls = client.local.composerControls, fields = ['contexts', 'staged', 'draftThreads', 'balance'] as const;
  const orders = obj(obj(client.local).mobileAttachmentOrder), contexts = obj(obj(client.local).mobileRecoveredDrafts);
  if (store.records[key] || key in client.local.drafts || key in client.local.snapshotDrafts || key in orders
    || draftFiles(client.local).some(file => file.draftKey === key) || fields.some(field => key in obj(controls[field]))
    || Object.values(store.claims).some(value => value === key || value === sourceKey)
    || Object.values(store.receipts).some(value => [key, sourceKey].includes(str(obj(value).key)))
    || Object.values(contexts).some(value => obj(value).key === key))
    throw new ClientError('The separate recovery draft already has an owner. Its content was kept unchanged.');
  // Clone every source before changing any owner; unreadable data cannot leave a partial move.
  const nextMetadata = copy({ ...metadata, key }), text = client.local.drafts[sourceKey];
  const images = client.local.snapshotDrafts[sourceKey] === undefined ? undefined : copy(client.local.snapshotDrafts[sourceKey]);
  const files = copy(draftFiles(client.local)).map(file => file.draftKey === sourceKey ? { ...file, draftKey: key } : file);
  const nextControls = Object.fromEntries(fields.map(field => [field, controls[field]?.[sourceKey] === undefined ? undefined : copy(controls[field]![sourceKey])])) as Obj;
  const order = orders[sourceKey] === undefined ? undefined : copy(orders[sourceKey]);
  const nextContexts = copy(contexts);
  for (const value of Object.values(nextContexts)) if (obj(value).key === sourceKey) obj(value).key = key;
  nextContexts[contextOwner(marker)] = { owner: contextOwner(marker), revision: marker.revision, key, origin: metadata.origin, environmentId: metadata.environmentId,
    context: copy(obj(metadata.context)), creation: copy(obj(marker.baseline.record.creation)), nativeRetired: true,
    pendingEditorDetached: { session: marker.session, draftKey: sourceKey, fingerprint } };
  const origin = branchState(client).origin.get(sourceKey);
  if (!mobilePendingTaskEditorsRemove(client, marker)) throw changed();
  delete store.records[sourceKey]; store.records[key] = nextMetadata;
  delete client.local.drafts[sourceKey]; if (text !== undefined) client.local.drafts[key] = text;
  delete client.local.snapshotDrafts[sourceKey]; if (images !== undefined) client.local.snapshotDrafts[key] = images;
  setDraftFiles(client.local, files);
  for (const field of fields) {
    const values = obj(controls[field]); delete values[sourceKey];
    if (nextControls[field] !== undefined) values[key] = nextControls[field];
  }
  const nextOrders = { ...orders }; delete nextOrders[sourceKey]; if (order !== undefined) nextOrders[key] = order;
  Object.assign(client.local, { mobileAttachmentOrder: nextOrders, mobileRecoveredDrafts: nextContexts });
  branchState(client).origin.delete(sourceKey); if (origin !== undefined) branchState(client).origin.set(key, origin);
  client.revision++; return key;
}
