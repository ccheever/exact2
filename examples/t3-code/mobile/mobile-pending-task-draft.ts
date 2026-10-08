// Pinned365aa87982 new-task-flow-provider: hydrate once; rebuild under original task IDs.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { branchState } from './shared/r4-git-branch';
import { mobileDraftAttachmentIds, mobileDraftAttachmentRecord } from './draft-attachment-order';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftPresentation, mobileNewTaskDraftRemoveMetadata,
  mobileNewTaskDraftSelectedBranch, mobileNewTaskDraftStore, mobileNewTaskDraftQueueFiles } from './mobile-new-task-drafts';
import { mobileCaptureNewTaskOutbox } from './mobile-outbox-capture';
import { mobileOutboxDecode, mobileOutboxEncode, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobilePendingTaskEditorKey, type MobilePendingTaskMarker } from './mobile-pending-task-state';

const keyFor = (record: MobileOutboxRecord) => `new-task:pending-${record.messageId}`;
const reasonOf = (error: unknown) => error instanceof Error ? error.message : 'The pending task draft could not be read.';

/** A lossless local comparison, including undefined and malformed raw context. JSON
 * alone collapses undefined/null, NaN/null and missing/undefined object properties. */
function stamp(value: unknown, ancestors = new Set<object>()): string {
  if (value === null) return 'null';
  if (typeof value === 'undefined') return 'undefined';
  if (typeof value === 'string' || typeof value === 'boolean') return `${typeof value}:${JSON.stringify(value)}`;
  if (typeof value === 'number') return `number:${Object.is(value, -0) ? '-0' : String(value)}`;
  if (typeof value !== 'object' || ancestors.has(value)) throw new Error('The draft contains unreadable local data.');
  ancestors.add(value);
  const result = Array.isArray(value) ? `[${Array.from(value, item => stamp(item, ancestors)).join(',')}]`
    : `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => `${JSON.stringify(key)}:${stamp(item, ancestors)}`).join(',')}}`;
  ancestors.delete(value); return result;
}
/** Caller captures this before an await and supplies it to conditional cleanup.
 * This is a content guard, not native ownership, a session CAS, or durability proof. */
export function mobilePendingTaskDraftFingerprint(client: T3Client, key: string): string | null {
  const metadata = mobileNewTaskDraftStore(client).records[key];
  if (!metadata) return null;
  try { return stamp({ metadata, text: client.local.drafts[key], images: client.local.snapshotDrafts[key],
    files: draftFiles(client.local).filter(file => file.draftKey === key),
    order: obj(obj(client.local).mobileAttachmentOrder)[key], workspace: client.local.composerControls.contexts[key],
    origin: branchState(client).origin.get(key), staged: client.local.composerControls.staged[key],
    balance: client.local.composerControls.balance?.[key], thread: client.local.composerControls.draftThreads?.[key] }); }
  catch { return null; }
}
export function mobilePendingTaskDraftAdopt(client: T3Client, input: MobileOutboxRecord): { draftKey: string; adopted: boolean; reason: string } {
  const draftKey = keyFor(input), refused = (reason: string) => ({ draftKey, adopted: false, reason });
  const decoded = mobileOutboxDecode(input);
  if (!decoded.ok) return refused(decoded.error);
  const record = decoded.record, creation = record.creation;
  if (!creation) return refused('This queued message is not a pending task.');
  if (!client.preferencesLoaded) return refused('Read saved drafts before opening the pending task.');
  const store = mobileNewTaskDraftStore(client), existing = store.records[draftKey];
  if (existing) {
    if (existing.origin !== record.origin || existing.environmentId !== record.environmentId || existing.projectId !== creation.projectId || existing.createdAt !== record.createdAt)
      return refused('The pending editor belongs to a different saved task.');
    if (!branchState(client).origin.has(draftKey)) branchState(client).origin.set(draftKey, existing.branchChoice?.startFromOrigin ?? creation.startFromOrigin ?? false);
    return { draftKey, adopted: false, reason: '' };
  }
  if (draftKey in client.local.drafts || draftKey in client.local.snapshotDrafts || draftFiles(client.local).some(file => file.draftKey === draftKey)
    || draftKey in client.local.composerControls.contexts || draftKey in obj(obj(client.local).mobileAttachmentOrder)
    || Object.values(store.claims).includes(draftKey) || Object.values(store.receipts).some(raw => obj(raw).key === draftKey))
    return refused('Saved editor content has unresolved ownership.');
  try {
    mobileNewTaskDraftCreate(client, { id: `pending-${record.messageId}`, environmentId: record.environmentId,
      projectId: creation.projectId, origin: record.origin, createdAt: record.createdAt,
      choices: { ...(record.modelSelection ? { providerId: record.modelSelection.instanceId, modelId: record.modelSelection.model,
        modelOptions: record.modelSelection.options?.map(option => ({ ...option })) ?? [] } : {}),
        ...(record.runtimeMode === undefined ? {} : { runtimeMode: record.runtimeMode }),
        ...(record.interactionMode === undefined ? {} : { interactionMode: record.interactionMode }) } });
    const metadata = store.records[draftKey], workspace = { envMode: creation.workspaceMode,
      branch: creation.branch ?? '', worktreePath: creation.worktreePath ?? '' };
    metadata.branchChoice = { ...workspace, kind: 'explicit', startFromOrigin: creation.startFromOrigin ?? false };
    if (record.context !== undefined) metadata.context = record.context;
    client.local.drafts[draftKey] = record.text;
    client.local.snapshotDrafts[draftKey] = record.attachments.flatMap(file => file.kind === 'image' ? [{ id: file.id,
      name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes, uploadId: file.uploadId,
      ...(file.source === undefined ? {} : { source: file.source }) }] : []);
    const files: DraftFile[] = record.attachments.flatMap(file => file.kind === 'file' ? [{ id: file.id, contextId: file.contextId,
      draftKey, environmentId: record.environmentId, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
      source: file.source, attachmentId: file.uploadId, status: file.uploadId ? 'ready' : 'staged',
      ...(file.videoWidth === undefined ? {} : { videoWidth: file.videoWidth }),
      ...(file.videoHeight === undefined ? {} : { videoHeight: file.videoHeight }) }] : []);
    setDraftFiles(client.local, [...draftFiles(client.local), ...files]);
    client.local.composerControls.contexts[draftKey] = workspace;
    branchState(client).origin.set(draftKey, creation.startFromOrigin ?? false);
    mobileDraftAttachmentRecord(client, draftKey, record.attachments.map(file => file.id));
    return { draftKey, adopted: true, reason: '' };
  } catch (error) { return refused(reasonOf(error)); }
}
export type MobilePendingTaskDraftBuild = { status: 'ready'; record: MobileOutboxRecord; fingerprint: string } | { status: 'blocked'; reason: string };
/** Pure recapture uses stored editor picks and original queue identity. The caller
 * separately owns current marker/session admission, native CAS and byte verification. */
export function mobilePendingTaskDraftBuildRecord(client: T3Client, marker: MobilePendingTaskMarker): MobilePendingTaskDraftBuild {
  const blocked = (reason: string): MobilePendingTaskDraftBuild => ({ status: 'blocked', reason });
  try {
    const decoded = mobileOutboxDecode(marker.baseline.record);
    if (!decoded.ok) return blocked(decoded.error);
    const baseline = decoded.record, creation = baseline.creation;
    if (!creation || marker.draftKey !== keyFor(baseline) || mobilePendingTaskEditorKey(marker.owner) !== mobilePendingTaskEditorKey(baseline))
      return blocked('The pending editor no longer matches its original task.');
    const fingerprint = mobilePendingTaskDraftFingerprint(client, marker.draftKey);
    if (fingerprint === null) return blocked('The pending editor content is missing or unreadable.');
    const draft = mobileNewTaskDraftPresentation(client, marker.draftKey);
    if (!draft || draft.origin !== baseline.origin || draft.environmentId !== baseline.environmentId || draft.projectId !== creation.projectId || draft.createdAt !== baseline.createdAt)
      return blocked('The pending editor destination changed.');
    // Validate the raw context before the presentation's JSON copy can normalize it.
    draft.context = mobileNewTaskDraftStore(client).records[marker.draftKey].context;
    const workspace = draft.workspace;
    if (!workspace || (workspace.envMode !== 'local' && workspace.envMode !== 'worktree')) return blocked('Restore the pending task workspace before saving.');
    const uploadOwners: Record<string, { origin: string; environmentId: string }> = {};
    const current = [...draft.images.map(image => ({ id: str(image.id), uploadId: str(image.uploadId), kind: 'image',
      name: str(image.name), mimeType: str(image.mimeType), sizeBytes: Number(image.sizeBytes) })),
      ...draft.files.map(file => ({ id: file.id, uploadId: file.attachmentId, kind: 'file', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes }))];
    for (const item of current) {
      if (!item.uploadId) continue;
      const original = baseline.attachments.find(file => file.id === item.id && file.kind === item.kind && file.uploadId === item.uploadId
        && file.name === item.name && file.mimeType === item.mimeType && file.sizeBytes === item.sizeBytes && file.uploadEnvironmentId === baseline.environmentId);
      if (!original) return blocked('The attachment upload owner is unresolved. Keep this draft until its original upload is verified.');
      uploadOwners[item.id] = { origin: baseline.origin, environmentId: baseline.environmentId };
    }
    const origin = branchState(client).origin.get(draft.key) ?? draft.branchChoice?.startFromOrigin ?? creation.startFromOrigin ?? false;
    const captured = mobileCaptureNewTaskOutbox(draft, { key: draft.key, origin: draft.origin, environmentId: draft.environmentId,
      projectId: draft.projectId, projectTitle: creation.projectTitle, projectCwd: creation.projectCwd,
      config: null, selectedModel: null, defaultRuntimeMode: baseline.runtimeMode ?? 'approval-required',
      connected: false, canOperate: true, planPreferenceLoaded: true, planModeEnabled: true, uploadOwners,
      workspace: { canChoose: true, mode: workspace.envMode, worktreePath: workspace.worktreePath || null,
        explicitBranch: mobileNewTaskDraftSelectedBranch(draft), currentCheckoutBranch: null, startFromOrigin: origin } }, baseline);
    if (captured.status !== 'ready') return captured;
    const record = captured.record;
    if (!record.creation) return blocked('The captured task is missing its workspace.');
    record.creation.worktreePath = workspace.worktreePath || null;
    if (creation.startFromOrigin !== undefined || branchState(client).origin.has(draft.key)) record.creation.startFromOrigin = origin;
    if (baseline.dispatchMode !== undefined) record.dispatchMode = baseline.dispatchMode;
    if (draft.choices?.runtimeMode === undefined && baseline.runtimeMode === undefined) delete record.runtimeMode;
    if (draft.choices?.interactionMode === undefined && baseline.interactionMode === undefined) delete record.interactionMode;
    if (record.modelSelection && baseline.modelSelection?.options === undefined && !record.modelSelection.options?.length) delete record.modelSelection.options;
    return { status: 'ready', record: mobileOutboxEncode(record), fingerprint };
  } catch (error) { return blocked(reasonOf(error)); }
}
/** Native queue/receipt ownership must already be resolved by the caller. Release
 * intents use existing owners; root persists first and filters live queue references. */
export function mobilePendingTaskDraftCleanup(client: T3Client, key: string, expectedFingerprint: string): boolean {
  if (!key.startsWith('new-task:pending-') || !expectedFingerprint || mobilePendingTaskDraftFingerprint(client, key) !== expectedFingerprint) return false;
  const store = mobileNewTaskDraftStore(client);
  if (Object.values(store.claims).includes(key) || Object.values(store.receipts).some(raw => obj(raw).key === key)) return false;
  for (const image of client.local.snapshotDrafts[key] ?? []) if (str(image.id) && !client.local.snapshotReleases.includes(str(image.id))) client.local.snapshotReleases.push(str(image.id));
  mobileNewTaskDraftQueueFiles(client, draftFiles(client.local).filter(file => file.draftKey === key));
  delete client.local.drafts[key]; delete client.local.snapshotDrafts[key];
  setDraftFiles(client.local, draftFiles(client.local).filter(file => file.draftKey !== key));
  mobileNewTaskDraftRemoveMetadata(client, key); branchState(client).origin.delete(key); client.revision++;
  return true;
}

/** Project the real serialized preference document before an awaited write. Live
 * owners stay intact until the caller confirms durable write + unchanged session,
 * then invokes Cleanup synchronously. The pending marker remains root-owned. */
export function mobilePendingTaskDraftCleanupDocument(client: T3Client, key: string, expectedFingerprint: string, document: Obj): Obj | null {
  if (!key.startsWith('new-task:pending-') || !expectedFingerprint || mobilePendingTaskDraftFingerprint(client, key) !== expectedFingerprint || document.version !== 1) return null;
  const live = mobileNewTaskDraftStore(client), saved = obj(document.mobileNewTaskDrafts);
  const dictionary = (value: unknown): value is Obj => !!value && typeof value === 'object' && !Array.isArray(value);
  if (Object.values(live.claims).includes(key) || Object.values(live.receipts).some(raw => obj(raw).key === key)
    || saved.version !== 1 || !dictionary(saved.records) || !dictionary(saved.claims) || !dictionary(saved.receipts)
    || Object.values(saved.claims).includes(key) || Object.values(saved.receipts).some(raw => obj(raw).key === key)
    || !Array.isArray(saved.fileReleases) || !Array.isArray(document.snapshotReleases)
    || !dictionary(document.drafts) || !dictionary(document.snapshotDrafts) || !dictionary(document.composerControls)
    || !dictionary(document.mobileAttachmentOrder) || !Array.isArray(document.composerFiles)) return null;
  try {
    // Compare JSON projections because this argument has already crossed shared
    // persist's serialization boundary. The separate live guard is lossless.
    const serialized = (value: unknown) => value === undefined ? 'undefined' : stamp(JSON.parse(JSON.stringify(value)));
    const same = (left: unknown, right: unknown) => serialized(left) === serialized(right);
    const files = draftFiles(client.local).filter(file => file.draftKey === key), order = mobileDraftAttachmentIds(client, key);
    if (!same(saved.records[key], live.records[key]) || !same(document.drafts[key], client.local.drafts[key])
      || !same(document.snapshotDrafts[key], client.local.snapshotDrafts[key])
      || !same(document.composerFiles.filter(file => obj(file).draftKey === key), files)
      || !same(document.mobileAttachmentOrder[key], order.length ? order : undefined)) return null;
    for (const field of ['contexts', 'staged', 'draftThreads', 'balance'] as const) {
      if (document.composerControls[field] !== undefined && !dictionary(document.composerControls[field])) return null;
      if (!same(obj(document.composerControls[field])[key], client.local.composerControls[field]?.[key])) return null;
    }
    const next = obj(JSON.parse(JSON.stringify(document))), metadata = obj(next.mobileNewTaskDrafts);
    delete obj(next.drafts)[key]; delete obj(next.snapshotDrafts)[key]; delete obj(next.mobileAttachmentOrder)[key];
    delete obj(metadata.records)[key];
    for (const field of ['contexts', 'staged', 'draftThreads', 'balance']) delete obj(obj(next.composerControls)[field])[key];
    next.composerFiles = document.composerFiles.filter(file => obj(file).draftKey !== key).map(file => JSON.parse(JSON.stringify(file)));
    next.snapshotReleases = [...new Set([...document.snapshotReleases,
      ...(client.local.snapshotDrafts[key] ?? []).map(image => str(image.id)).filter(Boolean)])];
    metadata.fileReleases = [...new Set([...saved.fileReleases, ...files.filter(file => file.source === 'attached').map(file => file.id)])];
    return next;
  } catch { return null; }
}
