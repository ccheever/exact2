// Source365aa87982 NewTaskDraftScreen.handleStart and buildPendingTaskMessage.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { mobileNewTaskDraftPresentation } from './mobile-new-task-drafts';
import { arr, obj, str, type Obj } from './shared/domain';
import { contextId, contextLabel, contextReferences } from './shared/composer-editor-menu';
import { MAX_FILE_BYTES } from './shared/composer-editor-files';
import { mobileModelSelectionUnavailable } from './model-availability';
import { mobileOutboxTitle } from './mobile-outbox-wire';
import { mobileOutboxEncode, type MobileOutboxAttachment, type MobileOutboxRecord,
  type MobileOutboxModelSelection, type MobileOutboxRuntimeMode } from './mobile-outbox-model';

export type MobileOutboxDraftPresentation = NonNullable<ReturnType<typeof mobileNewTaskDraftPresentation>>;
export type MobileOutboxCaptureMetadata = Pick<MobileOutboxRecord, 'threadId' | 'messageId' | 'commandId' | 'createdAt'>;
export interface MobileOutboxCaptureFacts {
  /** All facts belong to this exact presentation stamp, not a subsequently selected composer. */
  key: string; origin: string; environmentId: string; projectId: string;
  projectTitle?: string; projectCwd?: string;
  config: Obj | null; selectedModel: MobileOutboxModelSelection | null; defaultRuntimeMode: MobileOutboxRuntimeMode;
  connected: boolean; canOperate: boolean; planPreferenceLoaded: boolean; planModeEnabled: boolean;
  workspace: { canChoose: boolean; mode: 'local' | 'worktree'; worktreePath: string | null;
    /** Includes a source-selected automatic worktree base. undefined means provenance
     * is unresolved; null means untouched local checkout or no selected base. */
    explicitBranch: string | null | undefined; currentCheckoutBranch: string | null; startFromOrigin: boolean };
  /** Existing context owner supplies payloads; unsupported or unresolved records block capture. */
  context?: Obj;
  /** Source upload states are not native byte-existence or durability evidence. */
  uploadStates?: Record<string, 'uploading' | 'ready' | 'failed'>;
  /** Required for an existing remote upload ID; do not infer its owner from current selection. */
  uploadOwners?: Record<string, { origin: string; environmentId: string }>;
  blockReason?: string;
}
export type MobileOutboxCaptureResult = { status: 'blocked'; reason: string } | {
  status: 'ready'; record: MobileOutboxRecord;
  draftCapture: MobileOutboxDraftPresentation;
  presentation: { queuesInsteadOfStarting: boolean; title: string };
};
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function fail(reason: string): never { throw new Error(reason); }
const bounded = (value: unknown, max: number) => typeof value === 'string' && value.length <= max;
const nonempty = (value: unknown, max = Infinity) => bounded(value, max) && String(value).trim() === value && String(value).length > 0;
const integer = (value: unknown) => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const id = (value: unknown) => nonempty(value, 128) && /^[a-z0-9_-]+$/i.test(String(value));

/** Pinned resolveSelectableModelSelection; defaults are supplied by the existing display owner. */
function selectedModel(draft: MobileOutboxDraftPresentation, facts: MobileOutboxCaptureFacts): MobileOutboxModelSelection | null {
  const choices = draft.choices;
  let explicit: MobileOutboxModelSelection | null = choices?.providerId && choices.modelId
    ? { instanceId: choices.providerId, model: choices.modelId,
      ...(choices.modelOptions === undefined ? {} : { options: choices.modelOptions as MobileOutboxModelSelection['options'] }) } : null;
  if (explicit && facts.config) {
    const provider = arr(facts.config.providers).find(value => value.instanceId === explicit!.instanceId);
    const driver = str(provider?.driver, str(obj(obj(obj(facts.config.settings).providerInstances)[explicit.instanceId]).driver));
    if (driver !== 'antigravity' && (!provider?.enabled || !provider.installed || obj(provider.auth).status === 'unauthenticated')) explicit = null;
  }
  return explicit ?? facts.selectedModel;
}
function uploadable(file: MobileOutboxAttachment, config: Obj | null): boolean {
  const caps = obj(obj(config?.environment).capabilities), max = obj(caps.fileAttachments).maxUploadBytes;
  return caps.attachmentUploads === true && (file.kind === 'image' ||
    caps.fileAttachments !== undefined && file.sizeBytes <= Math.min(Number(max), MAX_FILE_BYTES));
}
function attachments(draft: MobileOutboxDraftPresentation, facts: MobileOutboxCaptureFacts): MobileOutboxAttachment[] {
  const items: MobileOutboxAttachment[] = [];
  const stampUpload = (localId: string, uploadId: string) => {
    if (!uploadId) return {};
    const owner = facts.uploadOwners?.[localId];
    if (owner?.environmentId !== draft.environmentId || owner.origin !== draft.origin)
      fail('The attachment upload owner is unresolved. Keep this draft until its original upload is verified.');
    return { uploadEnvironmentId: owner.environmentId };
  };
  for (const image of draft.images) {
    const localId = str(image.id), uploadId = str(image.uploadId);
    items.push({ id: localId, kind: 'image', name: str(image.name), mimeType: str(image.mimeType), sizeBytes: Number(image.sizeBytes),
      uploadId, status: uploadId ? 'ready' : 'staged', ...stampUpload(localId, uploadId),
      ...(image.source === undefined ? {} : { source: image.source as Obj }) });
  }
  for (const file of draft.files) {
    if (file.draftKey !== draft.key || file.environmentId !== draft.environmentId) fail('An attachment belongs to a different draft.');
    if (file.source !== 'attached') fail('Save the local file bytes before queuing this draft.');
    items.push({ id: file.id, kind: 'file', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
      uploadId: file.attachmentId, status: file.attachmentId ? 'ready' : 'staged', ...stampUpload(file.id, file.attachmentId),
      contextId: file.contextId, source: file.source,
      ...(file.videoWidth === undefined ? {} : { videoWidth: file.videoWidth }),
      ...(file.videoHeight === undefined ? {} : { videoHeight: file.videoHeight }) });
  }
  if (items.length > 100) fail('Remove attachments until there are at most 100.');
  const indexed = new Map(items.map(file => [file.id, file]));
  if (indexed.size !== items.length || draft.attachmentIds.length !== items.length ||
    new Set(draft.attachmentIds).size !== items.length || draft.attachmentIds.some(localId => !indexed.has(localId)))
    fail('The attachment order no longer matches this draft.');
  return draft.attachmentIds.map(localId => indexed.get(localId)!);
}

/** Narrow source-schema subset supported by current app context producers.
 * Element/preview/future kinds need their own verified capture owner; no boolean bypass. */
function contextRecordValid(record: Obj): boolean {
  if (record.version !== 1 || !id(record.contextId) || !bounded(record.label, 200)) return false;
  switch (record.kind) {
    case 'image': case 'file': return id(record.attachmentId) && nonempty(record.name, 255) &&
      nonempty(record.mimeType, 100) && integer(record.sizeBytes);
    case 'thread': return nonempty(record.environmentId) && nonempty(record.threadId) && bounded(record.title, 200);
    case 'terminal': return nonempty(record.terminalId, 255) && nonempty(record.terminalLabel, 255) &&
      integer(record.lineStart) && integer(record.lineEnd) && Number(record.lineEnd) >= Number(record.lineStart) && bounded(record.text, 64000);
    case 'mention': return nonempty(record.path, 2048);
    case 'skill': return nonempty(record.name, 255);
    case 'review-comment': {
      const pr = record.pullRequest === undefined ? null : obj(record.pullRequest);
      return nonempty(record.sectionId, 255) && bounded(record.sectionTitle, 2048) && nonempty(record.filePath, 2048) &&
        integer(record.startIndex) && integer(record.endIndex) && Number(record.endIndex) >= Number(record.startIndex) &&
        bounded(record.rangeLabel, 2048) && bounded(record.text, 16000) && bounded(record.diff, 32000) &&
        (record.fenceLanguage === undefined || bounded(record.fenceLanguage, 64)) && (!pr || integer(pr.number) && Number(pr.number) > 0 &&
          ['title', 'url', 'headBranch', 'baseBranch'].every(key => bounded(pr[key], 2048)) &&
          ['open', 'closed', 'merged'].includes(str(pr.state)) && typeof pr.isDraft === 'boolean');
    }
    default: return false;
  }
}
function messageContext(draft: MobileOutboxDraftPresentation, files: MobileOutboxAttachment[], supplied: Obj | undefined): Obj | undefined {
  if (supplied && (supplied.version !== 1 || !Array.isArray(supplied.records))) fail('The draft context is invalid.');
  const records = arr(supplied?.records).map(record => ({ ...record })), byId = new Map<string, Obj>();
  if (supplied && records.length !== (supplied.records as unknown[]).length) fail('The draft context is invalid.');
  for (const record of records) {
    if (!contextRecordValid(record) || byId.has(str(record.contextId))) fail('This draft contains unsupported or invalid context.');
    if (record.kind === 'file' || record.kind === 'image') {
      const file = files.find(file => file.id === record.attachmentId);
      if (!file) fail('The draft context references an attachment it no longer owns.');
      if (record.kind !== file.kind || record.name !== file.name || record.mimeType !== file.mimeType || record.sizeBytes !== file.sizeBytes)
        fail('The attachment context does not match its local owner.');
    }
    byId.set(str(record.contextId), record);
  }
  for (const reference of contextReferences(draft.text)) {
    const existing = byId.get(reference.id);
    if (existing) { if (existing.kind !== reference.kind) fail('The draft context kind does not match its reference.'); continue; }
    const file = files.find(file => reference.id === (file.kind === 'file' ? file.contextId : contextId('image', file.id)) &&
      reference.kind === file.kind);
    if (!file) fail('A context reference cannot be resolved. Keep the draft and restore its context first.');
    const record = { version: 1, contextId: reference.id, kind: file.kind, label: contextLabel(file.name, file.kind),
      attachmentId: file.id, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes };
    if (!contextRecordValid(record)) fail('An attachment context exceeds the supported limits.');
    records.push(record); byId.set(reference.id, record);
  }
  // contracts/src/composerContext.ts:284–299 bounds serialized record characters, not bytes.
  if (records.length > 200 || JSON.stringify(records).length > 16000000) fail('This draft has too much context to send.');
  return supplied || records.length ? { version: 1, records } : undefined;
}

/** Pure capture only. Ready means serializable mapping, NOT native byte existence,
 * upload verification, durable enqueue or permission to clear the original draft.
 * Caller must recheck the live owner after asynchronous metadata/fact preparation. */
export function mobileCaptureNewTaskOutbox(draft: MobileOutboxDraftPresentation,
  facts: MobileOutboxCaptureFacts, metadata: MobileOutboxCaptureMetadata): MobileOutboxCaptureResult {
  try {
    if (facts.key !== draft.key || facts.origin !== draft.origin || facts.environmentId !== draft.environmentId || facts.projectId !== draft.projectId)
      fail('The draft destination changed before it could be captured.');
    if (facts.blockReason) fail(facts.blockReason);
    if (facts.connected && !facts.canOperate) fail('This connection cannot start tasks.');
    const text = draft.text.trim(), model = selectedModel(draft, facts);
    if (!text) fail('Enter a task before starting the thread.');
    if (!model) fail('Choose a model before queuing this task.');
    if (facts.connected && mobileModelSelectionUnavailable(facts.config, model as unknown as Obj))
      fail('Antigravity model unavailable. Set it up on web or desktop, or choose another model.');
    const files = attachments(draft, facts);
    if (facts.connected && files.some(file => uploadable(file, facts.config) && facts.uploadStates?.[file.id] === 'failed'))
      fail('Retry or remove the failed attachment');
    const queuesInsteadOfStarting = !facts.connected || files.some(file => uploadable(file, facts.config) &&
      !['ready', 'failed'].includes(facts.uploadStates?.[file.id] ?? ''));
    const workspace = facts.workspace, mode = workspace.canChoose ? workspace.mode : 'local';
    if (workspace.canChoose && mode === 'local' && queuesInsteadOfStarting && workspace.explicitBranch === undefined && draft.workspace?.branch)
      fail('Confirm the selected branch before queuing this task.');
    const explicit = workspace.canChoose ? workspace.explicitBranch === undefined ? draft.workspace?.branch || null : workspace.explicitBranch : null;
    const branch = explicit !== null ? explicit : mode === 'local' && workspace.canChoose && !queuesInsteadOfStarting ? workspace.currentCheckoutBranch : null;
    if (mode === 'worktree' && !branch) fail('Select a base branch before creating a worktree.');
    const provider = arr(facts.config?.providers).find(value => value.instanceId === model.instanceId);
    const interactionMode = provider?.showInteractionModeToggle === false || !facts.planPreferenceLoaded || !facts.planModeEnabled
      ? 'default' : draft.choices?.interactionMode ?? 'default';
    const context = messageContext(draft, files, facts.context);
    const record = mobileOutboxEncode({ schemaVersion: 1, origin: draft.origin, environmentId: draft.environmentId,
      threadId: metadata.threadId, messageId: metadata.messageId, commandId: metadata.commandId, createdAt: metadata.createdAt,
      text, attachments: files, ...(context ? { context } : {}), modelSelection: model,
      runtimeMode: (draft.choices?.runtimeMode ?? facts.defaultRuntimeMode) as MobileOutboxRuntimeMode,
      interactionMode: interactionMode as MobileOutboxRecord['interactionMode'],
      creation: { projectId: draft.projectId, ...(facts.projectTitle === undefined ? {} : { projectTitle: facts.projectTitle }),
        ...(facts.projectCwd === undefined ? {} : { projectCwd: facts.projectCwd }), workspaceMode: mode, branch,
        worktreePath: workspace.canChoose && mode === 'local' ? workspace.worktreePath : null,
        ...(workspace.startFromOrigin ? { startFromOrigin: true } : {}) } });
    return { status: 'ready', record, draftCapture: copy(draft),
      presentation: { queuesInsteadOfStarting, title: mobileOutboxTitle(text, files) } };
  } catch (error) { return { status: 'blocked', reason: error instanceof Error ? error.message : 'The draft could not be captured.' }; }
}
