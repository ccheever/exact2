// Pinned365aa87982 use-thread-outbox-drain: merge once into the real composer owners.
// @ref llp/1109.005-composer-and-transcript.decision.md#terminal-draft-attachment-handoff
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
import { messageContext } from './shared/composer-editor';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { adoptTerminalContexts } from './shared/terminal-integrations';
import { branchState } from './shared/r4-git-branch';
import { applyStaged } from './shared/composer-controls';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileNewTaskDraftStore, mobileNewTaskDraftIsKey, type MobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileDraftAttachmentIds, mobileDraftAttachmentRecord, mobileDraftAttachmentForget } from './draft-attachment-order';
import { mobileOutboxDecode, type MobileOutboxAttachment, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxRecoveryKey, mobileOutboxRecoveryMergeContent, type MobileOutboxRecoveryKind } from './mobile-outbox-recovery-content';
import { mobileOutboxDraftHandoffProjection } from './mobile-outbox-draft-handoff';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const fail = (reason: string): never => { throw new ClientError(reason, 'retained'); };
const draftStore = (client: T3Client): MobileNewTaskDraftStore => (client.local as unknown as { mobileNewTaskDrafts?: MobileNewTaskDraftStore }).mobileNewTaskDrafts
  ?? { version: 1, records: {}, receipts: {}, claims: {}, fileReleases: [] };
const ownerFor = (record: MobileOutboxRecord) => `outbox:${JSON.stringify([record.origin, record.environmentId, record.commandId])}`;
export interface MobileOutboxRecoveryDraftChange { key: string; origin: string; environmentId: string; before: Obj; after: Obj; terminalBefore: Obj }
/** Exact local projection; no handles, normalization, persistence or native ownership. */
export function mobileOutboxRecoveryDraftCapture(client: T3Client, key: string): Obj {
  return mobileOutboxDraftHandoffProjection({ ...client.local, mobileNewTaskDrafts: draftStore(client) as unknown as Obj }, key);
}
function choices(record: MobileOutboxRecord): Obj {
  return { ...(record.modelSelection ? { providerId: record.modelSelection.instanceId, modelId: record.modelSelection.model,
    modelOptions: record.modelSelection.options ?? [] } : {}),
    ...(record.runtimeMode === undefined ? {} : { runtimeMode: record.runtimeMode }),
    ...(record.interactionMode === undefined ? {} : { interactionMode: record.interactionMode }) };
}
function attachments(client: T3Client, key: string, record: MobileOutboxRecord): MobileOutboxAttachment[] {
  const images: MobileOutboxAttachment[] = (client.local.snapshotDrafts[key] ?? []).map(image => ({
    id: str(image.id), kind: 'image', name: str(image.name), mimeType: str(image.mimeType), sizeBytes: Number(image.sizeBytes),
    uploadId: str(image.uploadId), status: image.uploadId ? 'ready' : 'staged',
    ...(image.uploadId ? { uploadEnvironmentId: record.environmentId } : {}), ...(image.source === undefined ? {} : { source: obj(image.source) }) }));
  const files: MobileOutboxAttachment[] = draftFiles(client.local).filter(file => file.draftKey === key).map(file => {
    if (file.environmentId !== record.environmentId) fail('A destination attachment belongs to another environment.');
    return { id: file.id, kind: 'file', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
      uploadId: file.attachmentId, status: file.status, contextId: file.contextId, source: file.source,
      ...(file.attachmentId ? { uploadEnvironmentId: record.environmentId } : {}),
      ...(file.videoWidth === undefined ? {} : { videoWidth: file.videoWidth }), ...(file.videoHeight === undefined ? {} : { videoHeight: file.videoHeight }) };
  });
  const byId = new Map([...images, ...files].map(file => [file.id, file]));
  if (byId.size !== images.length + files.length) fail('The destination has conflicting attachment identities.');
  return mobileDraftAttachmentIds(client, key).map(id => byId.get(id)!);
}
/** Build before mutating. A saved handoff must resume its proof, never call this merge again. */
export function mobileOutboxRecoveryDraftPrepare(client: T3Client, input: MobileOutboxRecord,
  kind: MobileOutboxRecoveryKind): MobileOutboxRecoveryDraftChange {
  if (!client.preferencesLoaded || input.origin !== mobileQueuedEditOrigin(client) || input.environmentId !== client.environmentId)
    return fail('Read this environment’s saved drafts before recovering the message.');
  const decoded = mobileOutboxDecode(input); if (!decoded.ok) return fail(decoded.error);
  const record = decoded.record, key = mobileOutboxRecoveryKey(record, kind), before = mobileOutboxRecoveryDraftCapture(client, key);
  const independent = key.startsWith('new-task:'), store = draftStore(client), metadata = store.records[key];
  if (independent && (!mobileNewTaskDraftIsKey(key) || !record.creation)) return fail('The restored task identity is invalid.');
  if (Object.values(store.claims).includes(key) || Object.values(store.receipts).some(value => obj(value).key === key))
    return fail('Resolve the destination’s captured send before restoring another message.');
  if (metadata && (!independent || metadata.origin !== record.origin || metadata.environmentId !== record.environmentId
    || metadata.projectId !== record.creation?.projectId || metadata.createdAt !== record.createdAt))
    return fail('The destination draft belongs to another saved task.');
  if (independent && !metadata && (key in client.local.drafts || key in client.local.snapshotDrafts
    || draftFiles(client.local).some(file => file.draftKey === key) || key in client.local.composerControls.contexts))
    return fail('The restored task already has content with unresolved ownership.');
  const records = new Map<string, Obj>();
  // Live producers contribute to the focused composer only. Frozen owners win over caches.
  if (client.draftKey === key) for (const entry of arr(messageContext(client, str(before.text))?.records)) records.set(str(entry.contextId), entry);
  for (const raw of Object.values(obj(before.recovered))) {
    const marker = obj(raw);
    if (marker.origin !== record.origin || marker.environmentId !== record.environmentId) return fail('The destination context belongs to another environment.');
    for (const entry of arr(obj(marker.context).records)) records.set(str(entry.contextId), entry);
  }
  if (metadata?.context !== undefined) {
    const context = obj(metadata.context);
    if (context.version !== 1 || !Array.isArray(context.records)) return fail('The destination context is invalid.');
    for (const entry of arr(context.records)) records.set(str(entry.contextId), entry);
  }
  const merged = mobileOutboxRecoveryMergeContent(record, { text: str(before.text), attachments: attachments(client, key, record),
    ...(records.size ? { context: { version: 1, records: [...records.values()] } } : {}) }, kind);
  if (!merged.ok) return fail(merged.reason);
  const content = merged.content, after = copy(before), originalImages = arr(before.images), originalFiles = arr(before.files);
  after.text = content.text;
  after.images = content.attachments.filter(file => file.kind === 'image').map(file =>
    originalImages.find(image => str(image.id).toLowerCase() === file.id.toLowerCase()) ?? {
      id: file.id, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes, uploadId: file.uploadId,
      ...(file.source === undefined ? {} : { source: file.source }) });
  after.files = content.attachments.flatMap(file => file.kind === 'file' ? [
    originalFiles.find(existing => str(existing.id).toLowerCase() === file.id.toLowerCase()) ?? {
      id: file.id, contextId: file.contextId, draftKey: key, environmentId: record.environmentId, name: file.name,
      mimeType: file.mimeType, sizeBytes: file.sizeBytes, source: file.source, attachmentId: file.uploadId,
      status: file.uploadId ? 'ready' : 'staged', ...(file.videoWidth === undefined ? {} : { videoWidth: file.videoWidth }),
      ...(file.videoHeight === undefined ? {} : { videoHeight: file.videoHeight }) }] : []);
  after.order = content.attachments.map(file => file.id);
  const selected = choices(record);
  if (independent) {
    const creation = record.creation!;
    const workspace = { envMode: creation.workspaceMode, branch: creation.branch ?? '', worktreePath: creation.worktreePath ?? '' };
    after.metadata = { ...(metadata ?? {}), key, origin: record.origin, environmentId: record.environmentId,
      projectId: creation.projectId, createdAt: record.createdAt, revision: (metadata?.revision ?? 0) + 1,
      choices: { ...metadata?.choices, ...selected }, branchChoice: { ...workspace, kind: 'explicit', startFromOrigin: creation.startFromOrigin ?? false },
      ...(content.context ? { context: content.context } : { context: { version: 1, records: [] } }) };
    after.workspace = workspace;
  }
  const owner = ownerFor(record);
  after.recovered = { ...obj(after.recovered), [owner]: { key, owner, origin: record.origin, environmentId: record.environmentId,
    revision: 1, context: content.context ?? null, attachments: content.attachments.map(file => ({ id: file.id, uploadId: file.uploadId })),
    // Partial settings survive offline recovery without inventing missing thread defaults.
    ...(!independent && Object.keys(selected).length ? { choices: selected } : {}) } };
  return copy({ key, origin: record.origin, environmentId: record.environmentId, before, after, terminalBefore: obj(obj(client.local).terminalContexts) });
}
function apply(client: T3Client, key: string, projection: Obj): void {
  const saved = copy(projection), store = mobileNewTaskDraftStore(client);
  if (str(saved.text)) client.local.drafts[key] = str(saved.text); else delete client.local.drafts[key];
  if (arr(saved.images).length) client.local.snapshotDrafts[key] = arr(saved.images); else delete client.local.snapshotDrafts[key];
  setDraftFiles(client.local, [...draftFiles(client.local).filter(file => file.draftKey !== key), ...arr(saved.files) as unknown as DraftFile[]]);
  if (saved.metadata === null) delete store.records[key]; else store.records[key] = saved.metadata as unknown as typeof store.records[string];
  if (saved.workspace === null) delete client.local.composerControls.contexts[key];
  else client.local.composerControls.contexts[key] = saved.workspace as unknown as typeof client.local.composerControls.contexts[string];
  if (saved.staged === null) delete client.local.composerControls.staged[key];
  else client.local.composerControls.staged[key] = saved.staged as unknown as typeof client.local.composerControls.staged[string];
  const others = Object.fromEntries(Object.entries(obj(obj(client.local).mobileRecoveredDrafts)).filter(([, marker]) => obj(marker).key !== key));
  Object.assign(client.local, { mobileRecoveredDrafts: { ...others, ...obj(saved.recovered) } });
  if ((saved.order as string[]).length) mobileDraftAttachmentRecord(client, key, saved.order as string[]); else mobileDraftAttachmentForget(client, key);
  const terminals = { ...obj(obj(client.local).terminalContexts) };
  for (const marker of Object.values(obj(saved.recovered))) for (const record of arr(obj(obj(marker).context).records))
    if (record.kind === 'terminal' && typeof record.text === 'string') terminals[str(record.contextId)] = record;
  adoptTerminalContexts(client.local, { terminalContexts: terminals });
  const branch = obj(obj(saved.metadata).branchChoice);
  if (key.startsWith('new-task:')) {
    if (typeof branch.startFromOrigin === 'boolean') branchState(client).origin.set(key, branch.startFromOrigin);
    else branchState(client).origin.delete(key);
  }
  client.revision++;
}
/** Synchronous CAS, then caller publishes the handoff marker and awaits persistence.
 * This never clears source/editor ownership and never proves native durability. */
export function mobileOutboxRecoveryDraftAdopt(client: T3Client, change: MobileOutboxRecoveryDraftChange): boolean {
  if (change.origin !== mobileQueuedEditOrigin(client) || change.environmentId !== client.environmentId) return false;
  if (canonical(mobileOutboxRecoveryDraftCapture(client, change.key)) !== canonical(change.before)) return false;
  apply(client, change.key, change.after); return true;
}
/** A stale native removal can undo only the exact untouched contribution. */
export function mobileOutboxRecoveryDraftRollback(client: T3Client, change: MobileOutboxRecoveryDraftChange): boolean {
  if (change.origin !== mobileQueuedEditOrigin(client) || change.environmentId !== client.environmentId) return false;
  if (canonical(mobileOutboxRecoveryDraftCapture(client, change.key)) !== canonical(change.after)) return false;
  const terminals = { ...obj(obj(client.local).terminalContexts) };
  for (const marker of Object.values(obj(change.after.recovered))) for (const record of arr(obj(obj(marker).context).records)) {
    const id = str(record.contextId);
    if (record.kind !== 'terminal' || canonical(terminals[id]) !== canonical(record)) continue;
    if (Object.hasOwn(change.terminalBefore, id)) terminals[id] = change.terminalBefore[id]; else delete terminals[id];
  }
  apply(client, change.key, change.before); adoptTerminalContexts(client.local, { terminalContexts: terminals }); return true;
}
/** Run after real thread selection is read. Missing queued picks preserve actual
 * current/staged values; consuming this plain marker prevents later user picks being reset. */
export function mobileOutboxRecoveryDraftApplyChoices(client: T3Client): void {
  if (!client.threadId || !client.providerId || !client.modelId) return;
  const key = client.draftKey, markers = obj(obj(client.local).mobileRecoveredDrafts);
  for (const raw of Object.values(markers)) {
    const marker = obj(raw), selected = obj(marker.choices);
    if (marker.key !== key || marker.origin !== mobileQueuedEditOrigin(client) || marker.environmentId !== client.environmentId || !Object.keys(selected).length) continue;
    const prior = client.local.composerControls.staged[key] ?? { providerId: client.providerId, modelId: client.modelId,
      options: client.modelOptions, runtimeMode: client.runtimeMode, interactionMode: client.interactionMode };
    client.local.composerControls.staged[key] = { ...prior,
      ...(typeof selected.providerId === 'string' && typeof selected.modelId === 'string'
        ? { providerId: selected.providerId, modelId: selected.modelId, options: arr(selected.modelOptions) } : {}),
      ...(typeof selected.runtimeMode === 'string' ? { runtimeMode: selected.runtimeMode } : {}),
      ...(typeof selected.interactionMode === 'string' ? { interactionMode: selected.interactionMode } : {}) };
    delete marker.choices; applyStaged(client); client.revision++;
  }
}
