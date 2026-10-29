import { newTaskEditorProduce, type NewTaskProducerIntent, type NewTaskProducerEffect, type NewTaskProducerResult } from './new-task-editor-producers';
import { mobileVoiceSnapshot, mobileVoiceSelection } from './voice-data';
import type { T3Client } from './shared/client';
import { mobileDraftRecoveryHydrated, type MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskFlowCurrent, mobileNewTaskFlowOwns, mobileNewTaskRoute } from './new-task-flow';
import { mobileNewTaskDraftBranchRevision, mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftLookup, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskContextObserve, mobileNewTaskContextAttach, mobileNewTaskContextProject,
  type MobileNewTaskAttachmentCommit, type MobileNewTaskAttachmentProjection, type MobileNewTaskContextGuard } from './mobile-new-task-context';
import { mobilePickerIntakeRequest, mobilePickerIntakeInvoke, type PickerIntakeRequest,
  type PickerIntakeFile, type PickerIntakePick, type PickerIntakeFinish } from './composer-picker-intake-io';
import { mobileFileHoldsCreate, mobileFileHoldReserve, mobileFileHoldAcquire, mobileFileHoldHeld,
  mobileFileHoldRelease, mobileFileHoldReleaseNeeded, mobileFileHoldsRetire, type FileHoldLedger } from './composer-file-holds-io';
import { mobileComposerCountAttachmentsAfterSelection } from './composer-context-insertion';
import { mobileComposerPickerFileLimit } from './composer-picker';
import type { OrdinaryInventoryAttachment } from './composer-attachment-publication';
import { localId, workspaceCwd } from './shared/composer-editor';
import { arr, obj, str } from './shared/domain';
import { mobileComposerDocument, mobileComposerFileIcon, type ComposerInlineToken } from './composer-editor-document';
import { mobileComposerProvider } from './composer-command-query';
import { resolveProviderSkillsForCwd } from './composer-command-model';
import { draftFiles, fileContextRecords } from './shared/composer-editor-files';
import { contextReferences } from './shared/composer-editor-menu';
import { mobileDraftAttachmentIds } from './draft-attachment-order';
import { mobileComposerTarget, type MobileComposerTarget } from './composer-target';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { fleet } from './shared/settings-b-fleet';
import { mobileComposerEditorAccept, mobileComposerEditorAdmit, mobileComposerEditorControlled,
  mobileComposerEditorDecodeEvent, mobileComposerEditorDecodeResume, type ComposerEditorResume, type ComposerEditorEvent, type ComposerEditorState,
  type ComposerMountedIdentity } from './composer-editor-state';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface NewTaskEditorRoute { owner: string; visit: string; location: string; editorId: string }
export type NewTaskEditorView =
  | { kind: 'held'; node: null; editable: false; message: string }
  | { kind: 'mounting' | 'editing' | 'frozen'; node: { key: string; configuration: string }; editable: boolean; message: string };
export type NewTaskEditorIntent = { kind: 'observe'; payload: unknown } | { kind: 'retry' }
  | { kind: 'pick'; source: 'photos' | 'files' } | NewTaskProducerIntent
  | { kind: 'claim'; id: string };
export interface NewTaskEditorResult { accepted: boolean; message: string; effect?: NewTaskProducerEffect }
interface DraftIdentity { key: string; createdAt: string; origin: string; environmentId: string; projectId: string }
interface ReceiptSource {
  id: string; identity: ComposerMountedIdentity; event: ComposerEditorEvent;
  retired: boolean; fingerprint: string;
}
interface OperationIdentity { id: string; revision: number }
type FrozenReceipt = ReceiptSource & { kind: 'frozen'; operation: OperationIdentity };
type Receipt = FrozenReceipt | ReceiptSource & { kind: 'retired' };
interface PickCapture {
  source: 'photos' | 'files'; intent?: NewTaskProducerIntent; route: NewTaskEditorRoute; draft: DraftIdentity; identity: ComposerMountedIdentity;
  target: MobileComposerTarget; threadEpoch: number; catalog: string; revision: number;
  eventCount: number; value: string; selection: { start: number; end: number };
}
type PickWork =
  | { phase: 'allocating'; capture: PickCapture; revision: number; attempt: number }
  | { phase: 'freezing'; capture: PickCapture; operation: OperationIdentity; attempt: number }
  | { phase: 'frozen'; capture: PickCapture; operation: OperationIdentity; receipt: FrozenReceipt;
      source: MobileNewTaskAttachmentProjection | null; continuation: Continuation; handback: Handback | null; attempt: number };
type FrozenPick = Extract<PickWork, { phase: 'frozen' }>;
interface ResumeRequest {
  readonly op: 'composerEditorHandoff'; readonly action: 'resume'; readonly generation: number;
  readonly identity: ComposerMountedIdentity; readonly operationId: string; readonly operationRevision: number; readonly receiptId: string;
  readonly document: { readonly value: string; readonly selection: { readonly start: number; readonly end: number };
    readonly tokensJson: string; readonly isNativeEcho: false };
}
interface Handback { request: ResumeRequest; newest: ComposerEditorEvent | null; proof: ComposerEditorResume | null }
interface CompletedCycle { operationRevision: number; frozenEventCount: number; receiptId: string; proof: ComposerEditorResume | null }
type Continuation =
  | { kind: 'ready' }
  | { kind: 'producing' }
  | { kind: 'voice'; message: string }
  | { kind: 'produced'; result: NewTaskProducerResult; after: MobileNewTaskAttachmentProjection | null; saved: boolean }
  | { kind: 'intake'; request: PickerIntakePick; dispatched: boolean }
  | { kind: 'staged'; request: PickerIntakePick; files: readonly PickerIntakeFile[] }
  | { kind: 'published'; request: PickerIntakePick; files: readonly PickerIntakeFile[]; commit: MobileNewTaskAttachmentCommit }
  | { kind: 'finishing'; request: PickerIntakeFinish; files: readonly PickerIntakeFile[]; commit: MobileNewTaskAttachmentCommit }
  | { kind: 'cancelling'; request: PickerIntakeRequest; files: readonly PickerIntakeFile[] | null }
  | { kind: 'terminal'; outcome: 'finished'; request: PickerIntakeFinish;
      files: readonly PickerIntakeFile[]; commit: MobileNewTaskAttachmentCommit }
  | { kind: 'terminal'; outcome: 'cancelled' | 'absent'; files: readonly PickerIntakeFile[] | null };
interface Publication { receipt: Receipt; revision: number; slotReleased: boolean }
interface FinalizationSnapshot { receipts: Receipt[]; reserved: ComposerMountedIdentity[] }
interface ReleaseCandidate { entry: Entry; publication: Publication; operation: PickWork | null; attempt: number }
type Progress =
  | { kind: 'unpublished' }
  | { kind: 'published'; publication: Publication }
  | { kind: 'durable'; publication: Publication; ackRead: number | null }
  | { kind: 'saved'; publication: Publication }
  | { kind: 'acknowledged'; receipt: Receipt };
interface Entry {
  draft: DraftIdentity; route: NewTaskEditorRoute; state: ComposerEditorState; revision: number;
  custody: 'native' | 'recovery' | 'frozen'; progress: Progress; dirty: boolean;
  attempt: number; problem: string; notice: string; operation: PickWork | null; operationRevision: number; holds: FileHoldLedger | null;
  generation: number; catalog: string; threadEpoch: number; confirmed: readonly ComposerInlineToken[]; tokensJson: string;
  completed: CompletedCycle | null; effect: NewTaskProducerEffect | null;
}
interface Ledger {
  entries: Entry[]; serial: number; acceptedRead: number; problem: string; wakeUsed: boolean;
}
const ledgers = new WeakMap<MobileDraftClient, Ledger>();
let identitySerial = 0;
const CAPACITY = 8, MAX_COUNT = Number.MAX_SAFE_INTEGER;
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
const token = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const count = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const uuid = (value: unknown): value is string => typeof value === 'string'
  && /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(value);
const held = (message: string): NewTaskEditorView => ({ kind: 'held', node: null, editable: false, message });
function ledger(client: MobileDraftClient): Ledger {
  let value = ledgers.get(client);
  if (!value) { value = { entries: [], serial: 0, acceptedRead: 0, problem: '', wakeUsed: false }; ledgers.set(client, value); }
  return value;
}
function sameIdentity(a: ComposerMountedIdentity, b: ComposerMountedIdentity): boolean {
  return a.owner === b.owner && a.editorId === b.editorId && a.routeVisit === b.routeVisit
    && a.renderEpoch === b.renderEpoch && a.mountId === b.mountId;
}
function mounted(entry: Entry): ComposerMountedIdentity { return { ...entry.state.identity, mountId: entry.state.mountId }; }
function sameStable(a: DraftIdentity, b: DraftIdentity): boolean {
  return a.key === b.key && a.createdAt === b.createdAt && a.origin === b.origin && a.environmentId === b.environmentId;
}
function registry(client: MobileDraftClient, entry: Entry) {
  const draft = mobileNewTaskDraftLookup(client, entry.draft.key);
  if (draft && !entry.operation && entry.progress.kind === 'unpublished' && entry.custody === 'native'
    && entry.state.value === (client.local.drafts[entry.draft.key] ?? '')
    && mobileNewTaskDraftBranchRevision(client, entry.draft.key, entry.revision) === draft.revision) entry.revision = draft.revision;
  return draft && sameStable(entry.draft, draft) && draft.projectId === entry.draft.projectId ? draft : null;
}
function currentRoute(client: MobileDraftClient, route: NewTaskEditorRoute): boolean {
  const parsed = mobileNewTaskRoute(route.location);
  return token(route.owner) && token(route.visit) && token(route.editorId) && parsed.context === 'draft'
    && !parsed.unsupported && !parsed.pendingTaskId && !mobileNewTaskDraftIsPendingKey(parsed.draftId)
    && mobileNewTaskFlowCurrent(route.owner, route.visit, route.location, client)
    && mobileNewTaskFlowOwns(route.owner, route.visit, client);
}
function admission(client: MobileDraftClient, route: NewTaskEditorRoute): { draft: DraftIdentity; owner: string; revision: number } | null {
  if (!mobileDraftRecoveryHydrated(client) || !currentRoute(client, route)) return null;
  const draft = mobileNewTaskDraftCurrent(client), target = mobileComposerTarget(client);
  if (!draft || mobileNewTaskDraftIsPendingKey(draft.key) || target.kind !== 'ordinary' || target.threadId
    || target.key !== draft.key || target.incarnation !== draft.createdAt || !count(client.generation)
    || !token(target.owner) || (client.local.drafts[draft.key] ?? '').length > 1_000_000) return null;
  return { draft: { key: draft.key, createdAt: draft.createdAt, origin: draft.origin,
    environmentId: draft.environmentId, projectId: draft.projectId }, owner: target.owner, revision: draft.revision };
}
function matchingRoute(entry: Entry, route: NewTaskEditorRoute): boolean {
  return entry.route.owner === route.owner && entry.route.visit === route.visit && entry.route.location === route.location
    && entry.route.editorId === route.editorId;
}
function activeEntry(client: MobileDraftClient, route: NewTaskEditorRoute): Entry | null {
  const captured = admission(client, route);
  if (!captured) return null;
  return ledger(client).entries.find(entry => entry.progress.kind !== 'acknowledged' && matchingRoute(entry, route)
    && entry.state.identity.owner === captured.owner && sameStable(entry.draft, captured.draft)
    && entry.draft.projectId === captured.draft.projectId) ?? null;
}
function liveEntry(client: MobileDraftClient, entry: Entry): boolean {
  return activeEntry(client, entry.route) === entry && client.generation === entry.generation
    && client.threadEpoch === entry.threadEpoch && mobileCacheCatalogIdentity(fleet.saved, entry.draft.environmentId) === entry.catalog;
}
function usableHolds(entry: Entry) {
  return entry.holds ? Object.keys(entry.holds.entries).flatMap(id => {
    const held = mobileFileHoldHeld(entry.holds!, id); return held ? [held] : [];
  }) : [];
}
function savedFiles(client: MobileDraftClient, entry: Entry) {
  return draftFiles(client.local).filter(file => file.draftKey === entry.draft.key && file.source === 'attached');
}
function filesReady(client: MobileDraftClient, entry: Entry): boolean {
  const held = usableHolds(entry);
  return savedFiles(client, entry).every(file => held.some(binding => equal(binding.request.file, file)
    || equal({ ...binding.request.file, status: 'staged', attachmentId: '' }, { ...file, status: 'staged', attachmentId: '' })));
}
async function prepareFiles(client: MobileDraftClient, entry: Entry, native: Native): Promise<void> {
  if (!entry.state.mountId || entry.operation || entry.progress.kind !== 'unpublished' || entry.problem || !liveEntry(client, entry)) return;
  const files = savedFiles(client, entry), revision = entry.revision;
  const current = () => liveEntry(client, entry) && !entry.operation && entry.progress.kind === 'unpublished'
    && registry(client, entry)?.revision === revision && equal(savedFiles(client, entry), files);
  try {
    for (const file of files) {
      if (!current()) return;
      entry.holds ??= mobileFileHoldsCreate(mounted(entry), entry.generation, { kind: 'new-task', origin: entry.draft.origin,
        environmentId: entry.draft.environmentId, projectId: entry.draft.projectId, draftKey: entry.draft.key, createdAt: entry.draft.createdAt });
      const holds = entry.holds;
      if (usableHolds(entry).some(held => equal({ ...held.request.file, status: 'staged', attachmentId: '' },
        { ...file, status: 'staged', attachmentId: '' }))) continue;
      const prior = Object.values(holds.entries).find(hold => hold.desired === 'retain' && !hold.intake && equal(hold.request.file, file));
      const request = prior?.request ?? mobileFileHoldReserve(holds, file, localId());
      await mobileFileHoldAcquire(holds, request.requestId, native);
      if (!current()) return;
      if (!mobileFileHoldHeld(holds, request.requestId)) throw new ClientError('Wait for the saved attachment bytes to be retained.');
    }
    if (current()) entry.notice = '';
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) entry.notice = error instanceof Error ? error.message : 'The saved attachment bytes are not ready.';
  }
}
function projectDocument(client: MobileDraftClient, entry: Entry, value: string, after?: MobileNewTaskAttachmentProjection) {
  const draft = after?.record ?? registry(client, entry), context = mobileNewTaskContextProject(value, draft?.context);
  const files = after?.files ?? draftFiles(client.local).filter(file => file.draftKey === entry.draft.key);
  const images = after?.images ?? client.local.snapshotDrafts[entry.draft.key] ?? [];
  const references = new Set(contextReferences(value).filter(ref => ref.kind === 'file').map(ref => ref.id));
  const records = new Map(fileContextRecords(files.filter(file => references.has(file.contextId))
    .map(file => ({ ...file, attachmentId: file.id }))).map(row => [str(row.contextId), row]));
  for (const row of context.ok ? context.context?.records ?? [] : []) records.set(str(row.contextId), row);
  const cwd = workspaceCwd(client), provider = liveEntry(client, entry)
    ? mobileComposerProvider(arr(client.config.providers).find(p => p.instanceId === client.providerId), cwd) : null;
  return mobileComposerDocument({ value, context: context.ok && records.size ? { version: 1, records: [...records.values()] } : undefined,
    environmentId: entry.draft.environmentId, attachments: [...images.map(row => ({ id: str(row.id) })), ...files.map(file => ({ id: file.id }))],
    skills: provider ? resolveProviderSkillsForCwd(provider, cwd) : [], confirmedTokens: entry.confirmed,
    iconUri: path => `t3-bundled-icon:${mobileComposerFileIcon(path)}` });
}
function updateDocument(client: MobileDraftClient, entry: Entry): void {
  const document = projectDocument(client, entry, entry.state.value);
  entry.confirmed = document.confirmedTokens; entry.tokensJson = document.tokensJson;
}
function publishText(client: MobileDraftClient, entry: Entry, value: string): boolean {
  const guard: MobileNewTaskContextGuard = { key: entry.draft.key, createdAt: entry.draft.createdAt, revision: entry.revision };
  const destination = { origin: entry.draft.origin, environmentId: entry.draft.environmentId, projectId: entry.draft.projectId };
  const result = mobileNewTaskContextObserve(client, { guard, destination, identity: mounted(entry),
    generation: entry.generation, text: value, usableHolds: usableHolds(entry) });
  if (result.kind === 'refused') return false;
  entry.revision = result.revision; entry.dirty = true;
  return true;
}
function observe(client: MobileDraftClient, entry: Entry, payload: unknown): boolean {
  if (!mobileDraftRecoveryHydrated(client)) return false;
  const proof = mobileComposerEditorDecodeResume(payload);
  if (proof) return acceptResume(client, entry, proof);
  const work = entry.operation;
  if (work?.phase === 'frozen' && work.handback) {
    const handback = work.handback;
    const event = mobileComposerEditorDecodeEvent(payload);
    if (!event || !['text', 'selection', 'focus', 'blur'].includes(event.kind) || event.pendingCommand
      || !sameIdentity(event, mounted(entry)) || event.eventCount <= work.receipt.event.eventCount + 1) return false;
    if (handback.newest && event.eventCount <= handback.newest.eventCount) return equal(event, handback.newest);
    handback.newest = event;
    return handback.proof ? acceptResume(client, entry, handback.proof) : true;
  }
  if (entry.custody !== 'native' || entry.progress.kind !== 'unpublished' || entry.problem || entry.operation) return false;
  const event = mobileComposerEditorDecodeEvent(payload), draft = registry(client, entry);
  if (!event || event.pendingCommand || event.kind === 'commandApplied' || event.kind === 'commandRejected'
    || event.eventCount < 1 || !draft || draft.revision !== entry.revision || !liveEntry(client, entry)) return false;
  const accepted = mobileComposerEditorAccept(entry.state, event);
  if (!accepted.accepted) return false;
  if (!entry.state.mountId && ledger(client).entries.some(previous => previous !== entry && previous.state.mountId === event.mountId)) return false;
  if (accepted.duplicate) return true;
  if (event.value !== entry.state.value) {
    if (!filesReady(client, entry) || !publishText(client, entry, event.value)) return false;
  }
  entry.state = accepted.state;
  updateDocument(client, entry);
  return true;
}

export function mobileNewTaskEditorView(client: MobileDraftClient, route: NewTaskEditorRoute, latch: unknown = ''): NewTaskEditorView {
  const owner = ledger(client), capture = admission(client, route);
  if (!capture) return held('Wait for the current new task draft to be ready.');
  if (!owner.acceptedRead) return held('Wait for the previous editor sources to be checked.');
  let entry = activeEntry(client, route);
  if (entry && latch) observe(client, entry, latch);
  if (entry?.operation?.phase === 'frozen' && !entry.operation.receipt.retired
    && !(entry.progress.kind === 'saved' && entry.progress.publication.slotReleased))
    return presentation(entry, false, 'frozen', entry.problem || entry.notice || owner.problem || 'The editor source is held for the attachment operation.');
  if (entry?.operation && entry.operation.phase !== 'frozen' && entry.custody === 'native'
    && entry.progress.kind === 'unpublished' && !entry.problem)
    return presentation(entry, true, 'editing', owner.problem);
  if (owner.problem) return held(owner.problem);
  if (!entry) {
    owner.entries = owner.entries.filter(previous => previous.state.mountId);
    if (owner.entries.some(previous => previous.progress.kind !== 'acknowledged' && sameStable(previous.draft, capture.draft)))
      return held('Save the previous editor source before opening this draft.');
    if (owner.entries.length >= CAPACITY) {
      const completed = owner.entries.findIndex(previous => previous.progress.kind === 'acknowledged');
      if (completed < 0) return held('Save the previous editor sources before opening another editor.');
      owner.entries.splice(completed, 1);
    }
    if (identitySerial >= MAX_COUNT) return held('Restart the app before opening another editor.');
    const previous = owner.entries.findLast(candidate => sameStable(candidate.draft, capture.draft));
    const text = client.local.drafts[capture.draft.key] ?? '', renderEpoch = `new-task-editor:${++identitySerial}`;
    entry = { draft: capture.draft, route: { ...route }, revision: capture.revision, custody: 'native',
      state: mobileComposerEditorAdmit(null, { owner: capture.owner, editorId: route.editorId, routeVisit: route.visit, renderEpoch },
        { value: text, selection: previous?.state.value === text ? { ...previous.state.selection } : { start: text.length, end: text.length } }),
      progress: { kind: 'unpublished' }, dirty: false, attempt: 0, problem: '', notice: '', operation: null, operationRevision: 0, holds: null,
      generation: client.generation, catalog: mobileCacheCatalogIdentity(fleet.saved, capture.draft.environmentId), threadEpoch: client.threadEpoch,
      confirmed: [], tokensJson: '[]', completed: null, effect: null };
    owner.entries.push(entry);
    updateDocument(client, entry);
    if (latch) observe(client, entry, latch);
  }
  if (entry.problem || entry.custody !== 'native' || entry.progress.kind !== 'unpublished')
    return held(entry.problem || 'Save the previous editor source before editing.');
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision || !liveEntry(client, entry)) return held('The saved draft changed. Keep the previous editor source.');
  const editable = !!entry.state.mountId && filesReady(client, entry);
  return presentation(entry, editable, editable ? 'editing' : 'mounting', entry.notice);
}
function presentation(entry: Entry, editable: boolean, kind: 'mounting' | 'editing' | 'frozen', message: string): NewTaskEditorView {
  const control = mobileComposerEditorControlled(entry.state, entry.tokensJson);
  return { kind, editable, message, node: {
    key: JSON.stringify([entry.route.editorId, entry.state.identity.renderEpoch]),
    configuration: JSON.stringify({ ...control, active: true, editable, readOnly: false,
      focusIntent: { serial: '', attempt: 0, operation: 'none' } }),
  } };
}

function decodeReceipt(input: unknown): Receipt | null {
  if (!record(input) || !uuid(input.receiptId) || !record(input.identity) || !record(input.event)
    || typeof input.retired !== 'boolean') return null;
  const raw = input.event, identity = input.identity;
  if (raw.pendingCommand !== undefined || !['retired', 'frozen'].includes(String(raw.kind))
    || raw.kind === 'retired' && !input.retired) return null;
  const event = mobileComposerEditorDecodeEvent({ ...raw, kind: 'text' });
  if (!event || event.eventCount < 1 || !token(identity.owner) || !token(identity.editorId) || !token(identity.routeVisit)
    || !token(identity.renderEpoch) || !token(identity.mountId)) return null;
  const mount: ComposerMountedIdentity = { owner: identity.owner, editorId: identity.editorId, routeVisit: identity.routeVisit,
    renderEpoch: identity.renderEpoch, mountId: identity.mountId };
  if (!sameIdentity(mount, event)) return null;
  const kind = raw.kind === 'retired' ? 'retired' : 'frozen';
  if (kind === 'frozen' && (!uuid(raw.operationId) || !count(raw.operationRevision) || event.composing)) return null;
  const fingerprint = JSON.stringify([mount, kind, event.eventCount, event.value, event.selection, event.composing, event.focused,
    kind === 'frozen' ? [raw.operationId, raw.operationRevision] : null]);
  const source = { id: input.receiptId, identity: mount, event, retired: input.retired, fingerprint };
  if (kind === 'frozen') {
    if (!uuid(raw.operationId) || !count(raw.operationRevision)) return null;
    return { ...source, kind, operation: { id: raw.operationId, revision: raw.operationRevision } };
  }
  return { ...source, kind };
}
function decodeSnapshot(input: unknown): FinalizationSnapshot | null {
  if (!record(input) || !Array.isArray(input.receipts) || input.receipts.length > CAPACITY
    || !Array.isArray(input.reserved) || input.reserved.length > CAPACITY) return null;
  const reserved: ComposerMountedIdentity[] = [];
  for (const value of input.reserved) {
    if (!record(value) || !token(value.owner) || !token(value.editorId) || !token(value.routeVisit)
      || !token(value.renderEpoch) || !token(value.mountId) || reserved.some(previous => previous.mountId === value.mountId)) return null;
    reserved.push({ owner: value.owner, editorId: value.editorId, routeVisit: value.routeVisit,
      renderEpoch: value.renderEpoch, mountId: value.mountId });
  }
  const receipts: Receipt[] = [];
  for (const value of input.receipts) {
    const receipt = decodeReceipt(value);
    if (!receipt || !reserved.some(identity => sameIdentity(identity, receipt.identity))) return null;
    const duplicate = receipts.find(previous => previous.id === receipt.id || sameIdentity(previous.identity, receipt.identity));
    if (duplicate) {
      if (duplicate.id !== receipt.id || duplicate.fingerprint !== receipt.fingerprint) return null;
      duplicate.retired ||= receipt.retired;
    } else receipts.push(receipt);
  }
  return { receipts, reserved };
}
function receiptOf(entry: Entry): Receipt | null {
  return entry.progress.kind === 'unpublished' ? entry.operation?.phase === 'frozen' ? entry.operation.receipt : null : entry.progress.kind === 'acknowledged'
    ? entry.progress.receipt : entry.progress.publication.receipt;
}
function publish(client: MobileDraftClient, entry: Entry, receipt: Receipt): boolean {
  if (entry.completed && (receipt.id === entry.completed.receiptId || receipt.event.eventCount <= entry.completed.frozenEventCount
    || receipt.kind === 'frozen' && receipt.operation.revision <= entry.completed.operationRevision)) return false;
  const previous = receiptOf(entry);
  if (previous) {
    if (previous.id !== receipt.id || previous.fingerprint !== receipt.fingerprint) {
      entry.problem = 'The retained editor source changed unexpectedly.'; return false;
    }
    previous.retired ||= receipt.retired;
    if (entry.progress.kind === 'saved' && previous.retired) releaseSlot(ledger(client), entry, entry.progress.publication);
    if (entry.progress.kind !== 'unpublished') return entry.progress.kind === 'published' || entry.progress.kind === 'durable';
  }
  if (entry.problem) return false;
  if (receipt.kind === 'frozen') {
    const work = entry.operation;
    if (!work || work.phase === 'allocating' || work.operation.id !== receipt.operation.id
      || work.operation.revision !== receipt.operation.revision || !sameIdentity(work.capture.identity, receipt.identity)) {
      entry.custody = 'frozen'; entry.problem = 'The editor source belongs to an unfinished operation.'; return false;
    }
    if (work.phase !== 'frozen')
      entry.operation = { ...work, phase: 'frozen', receipt, source: null, continuation: { kind: 'ready' }, handback: null, attempt: 0 };
    entry.custody = 'frozen';
  } else if (entry.custody === 'frozen' || !receipt.retired) {
    entry.custody = 'frozen'; entry.problem = 'The editor source belongs to an unfinished operation.'; return false;
  }
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision || receipt.event.eventCount <= entry.state.eventCount) {
    entry.problem = 'The draft changed before its final editor source was saved.'; return false;
  }
  const accepted = mobileComposerEditorAccept(entry.state, receipt.event);
  if (!accepted.accepted || !publishText(client, entry, receipt.event.value)) return false;
  entry.dirty = false; entry.custody = receipt.kind === 'frozen' ? 'frozen' : 'recovery';
  entry.state = accepted.state; updateDocument(client, entry);
  entry.progress = { kind: 'published', publication: { receipt, revision: entry.revision, slotReleased: false } };
  if (entry.operation?.phase === 'frozen') entry.operation.source = projection(client, entry);
  return true;
}
function releaseSlot(owner: Ledger, entry: Entry, publication: Publication): void {
  if (publication.slotReleased) return;
  publication.slotReleased = true;
  if (entry.holds) mobileFileHoldsRetire(entry.holds);
  // A refused factory has no native handle. Only a new keyed seed can retry it.
  owner.entries = owner.entries.filter(candidate => candidate === entry || candidate.state.mountId
    || candidate.operation || candidate.progress.kind !== 'unpublished');
}
function acknowledge(owner: Ledger, entry: Entry, publication: Publication): void {
  entry.progress = publication.receipt.kind === 'frozen' && entry.operation?.phase === 'frozen'
    ? { kind: 'saved', publication } : { kind: 'acknowledged', receipt: publication.receipt };
  if (publication.receipt.retired) releaseSlot(owner, entry, publication);
}
function releaseCandidates(owner: Ledger, serial: number): ReleaseCandidate[] {
  return owner.entries.flatMap(entry => {
    const progress = entry.progress;
    return progress.kind === 'saved' || progress.kind === 'durable' && progress.ackRead !== null && serial > progress.ackRead
      ? [{ entry, publication: progress.publication, operation: entry.operation, attempt: entry.attempt }] : [];
  });
}
function reconcileSnapshot(client: MobileDraftClient, owner: Ledger, serial: number,
  snapshot: FinalizationSnapshot, candidates: ReleaseCandidate[]): void {
  for (const candidate of candidates) {
    const { entry, publication } = candidate, draft = registry(client, entry), progress = entry.progress;
    if (!owner.entries.includes(entry) || entry.problem || entry.attempt !== candidate.attempt
      || entry.operation !== candidate.operation || (!draft || draft.revision < publication.revision)
        && !(entry.operation?.phase === 'frozen' && entry.operation.continuation.kind === 'produced' && entry.operation.continuation.result.leave)
      || progress.kind !== 'durable' && progress.kind !== 'saved' || progress.publication !== publication) continue;
    if (progress.kind === 'durable') {
      if (progress.ackRead === null || serial <= progress.ackRead
        || snapshot.receipts.some(receipt => receipt.id === publication.receipt.id)) continue;
      entry.attempt++; acknowledge(owner, entry, publication);
    }
    if (entry.progress.kind === 'saved' && !snapshot.reserved.some(identity => sameIdentity(identity, publication.receipt.identity)))
      releaseSlot(owner, entry, publication);
  }
}
function validCompletion(client: MobileDraftClient, owner: Ledger, entry: Entry, attempt: number, publication: Publication, serial: number): boolean {
  const draft = registry(client, entry);
  return serial === owner.acceptedRead && mobileDraftRecoveryHydrated(client) && owner.entries.includes(entry) && entry.attempt === attempt && !entry.problem && !!draft
    && draft.revision >= publication.revision && (entry.progress.kind === 'published' || entry.progress.kind === 'durable')
    && entry.progress.publication === publication;
}
async function settle(client: MobileDraftClient, owner: Ledger, entry: Entry, native: Native, storage: Files, serial: number) {
  const progress = entry.progress;
  if (progress.kind !== 'published' && progress.kind !== 'durable') return;
  const publication = progress.publication, attempt = ++entry.attempt;
  if (!validCompletion(client, owner, entry, attempt, publication, serial)) return;
  try {
    if (progress.kind === 'published') {
      await client.persist(storage);
      if (!validCompletion(client, owner, entry, attempt, publication, serial)) return;
      entry.progress = { kind: 'durable', publication, ackRead: null };
    }
    if (!validCompletion(client, owner, entry, attempt, publication, serial) || entry.progress.kind !== 'durable') return;
    entry.progress.ackRead = serial;
    const generation = client.generation;
    const response = await bridgeReply(native, { op: 'composerEditorFinalizations', generation, action: 'acknowledge', receiptId: publication.receipt.id });
    if (!validCompletion(client, owner, entry, attempt, publication, serial)) return;
    if (!response.ok || response.generation !== generation || !record(response.value) || typeof response.value.acknowledged !== 'boolean')
      throw new ClientError('The final editor source acknowledgment could not be confirmed.');
    if (response.value.acknowledged) acknowledge(owner, entry, publication);
    else await wake(owner, native, client.generation);
    if (owner.entries.includes(entry) && entry.attempt === attempt && serial === owner.acceptedRead) entry.notice = '';
  } catch (error) {
    if (letGo(error)) throw error;
    if (validCompletion(client, owner, entry, attempt, publication, serial)) {
      entry.notice = 'The final editor source is retained until it can be saved and acknowledged.';
      await wake(owner, native, client.generation);
    }
  }
}
async function wake(owner: Ledger, native: Native, generation: number): Promise<void> {
  if (owner.wakeUsed) return;
  owner.wakeUsed = true;
  try { await bridgeReply(native, { op: 'r10Wake', topic: 't3.notify', generation }); }
  catch (error) { if (letGo(error)) throw error; }
}
function pickCurrent(client: MobileDraftClient, entry: Entry, capture: PickCapture): boolean {
  const target = mobileComposerTarget(client);
  return mobileDraftRecoveryHydrated(client) && currentRoute(client, capture.route)
    && sameIdentity(mounted(entry), capture.identity) && target.owner === capture.target.owner
    && client.threadEpoch === capture.threadEpoch && client.generation === capture.target.generation
    && mobileCacheCatalogIdentity(fleet.saved, capture.target.environmentId) === capture.catalog
    && sameStable(entry.draft, capture.draft) && registry(client, entry)?.revision === capture.revision
    && entry.revision === capture.revision && entry.state.eventCount === capture.eventCount;
}
async function freeze(client: MobileDraftClient, owner: Ledger, entry: Entry,
  work: Extract<PickWork, { phase: 'freezing' }>, native: Native, storage: Files): Promise<void> {
  const generation = work.capture.target.generation, attempt = ++work.attempt, serial = owner.acceptedRead;
  const current = () => owner.entries.includes(entry) && entry.operation === work && work.attempt === attempt
    && serial === owner.acceptedRead && mobileDraftRecoveryHydrated(client);
  try {
    const response = await bridgeReply(native, { op: 'composerEditorHandoff', generation, identity: work.capture.identity,
      action: 'freeze', operationId: work.operation.id, operationRevision: work.operation.revision, retireUndo: work.capture.intent?.kind === 'remove' });
    if (!current()) return;
    if (!response.ok || response.generation !== generation || !record(response.value))
      throw new ClientError('The editor handoff is retained until its frozen source can be confirmed.');
    const receipt = decodeReceipt(response.value.receipt);
    if (!receipt || receipt.kind !== 'frozen' || !sameIdentity(receipt.identity, work.capture.identity)
      || receipt.operation.id !== work.operation.id || receipt.operation.revision !== work.operation.revision)
      throw new ClientError('The frozen editor source does not match the attachment operation.');
    if (publish(client, entry, receipt)) {
      const sourceAttempt = entry.attempt + 1;
      await settle(client, owner, entry, native, storage, serial);
      if (entry.attempt === sourceAttempt && serial === owner.acceptedRead) await continuePick(client, owner, entry, native, storage);
    }
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) {
      entry.notice = error instanceof Error ? error.message : 'The editor handoff is retained.';
      await wake(owner, native, client.generation);
    }
  }
}
async function releasePendingFiles(entry: Entry, native: Native, current: () => boolean): Promise<void> {
  const holds = entry.holds;
  if (!holds || holds.retired) return;
  for (const hold of Object.values(holds.entries)) {
    if (hold.desired !== 'release' || hold.phase === 'released') continue;
    await mobileFileHoldRelease(holds, hold.request.requestId, native);
    if (!current() || entry.holds !== holds) return;
  }
}
async function continuePick(client: MobileDraftClient, owner: Ledger, entry: Entry, native: Native, storage: Files): Promise<void> {
  await releasePendingFiles(entry, native, () => owner.entries.includes(entry));
  const work = entry.operation;
  if (!work || entry.problem) return;
  if (work.phase === 'frozen') {
    if (work.capture.intent) await advanceProducer(client, owner, entry, work, native, storage);
    else await advanceFrozen(client, owner, entry, work, native, storage);
    if (entry.progress.kind === 'saved' && entry.progress.publication.slotReleased
      && (work.continuation.kind === 'terminal' && work.continuation.outcome === 'finished' || work.continuation.kind === 'produced' && work.continuation.saved)) {
      entry.progress = { kind: 'acknowledged', receipt: work.receipt }; entry.operation = null; return;
    }
    if (entry.operation === work) await resume(client, owner, entry, work, native, storage);
    return;
  }
  if (entry.progress.kind !== 'unpublished') return;
  if (!pickCurrent(client, entry, work.capture)) {
    if (work.phase === 'allocating') entry.operation = null;
    return;
  }
  if (work.phase === 'freezing') { await freeze(client, owner, entry, work, native, storage); return; }
  const attempt = ++work.attempt, serial = owner.acceptedRead;
  const current = () => owner.entries.includes(entry) && entry.operation === work && work.attempt === attempt
    && serial === owner.acceptedRead;
  const ids = await client.ids(native, 1).catch(error => {
    if (letGo(error)) throw error;
    if (current() && pickCurrent(client, entry, work.capture))
      entry.notice = error instanceof Error ? error.message : 'The attachment identifier could not be allocated.';
    return null;
  });
  if (!current()) return;
  if (!pickCurrent(client, entry, work.capture)) { entry.operation = null; return; }
  if (!ids) return;
  const id = ids[0];
  if (!uuid(id)) { entry.notice = 'The attachment operation identifier is invalid.'; return; }
  const identified: Extract<PickWork, { phase: 'freezing' }> = {
    phase: 'freezing', capture: work.capture, operation: { id: id.toLowerCase(), revision: work.revision }, attempt: 0,
  };
  entry.operation = identified;
  await freeze(client, owner, entry, identified, native, storage);
}
function equal(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a)) return Array.isArray(b) && a.length === b.length && a.every((value, index) => equal(value, b[index]));
  if (!record(a) || !record(b)) return false;
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(key => Object.hasOwn(b, key) && equal(a[key], b[key]));
}
function intakeBase(work: FrozenPick, action: 'status' | 'cancel' | 'finish') {
  return { op: 'composerPickerIntake', action, generation: work.capture.target.generation,
    identity: work.capture.identity, operationId: work.operation.id };
}
function projection(client: MobileDraftClient, entry: Entry): MobileNewTaskAttachmentProjection | null {
  const work = entry.operation;
  if (work?.phase !== 'frozen') return null;
  try {
    const draft = mobileNewTaskDraftStore(client).records[entry.draft.key];
    if (!draft || !sameStable(draft, entry.draft) || draft.projectId !== entry.draft.projectId) return null;
    const ids = mobileDraftAttachmentIds(client, draft.key);
    const request = mobilePickerIntakeRequest({ ...intakeBase(work, 'finish'), publication: {
      after: { record: draft, text: client.local.drafts[draft.key] ?? '', images: client.local.snapshotDrafts[draft.key] ?? [],
        files: draftFiles(client.local).filter(file => file.draftKey === draft.key), attachmentIds: ids,
        attachmentOrder: ids.length ? ids : null }, acceptedIds: [], discardedIds: [],
    } });
    return request.action === 'finish' && 'record' in request.publication.after ? request.publication.after : null;
  } catch { return null; }
}
function ownsProjection(current: MobileNewTaskAttachmentProjection | null, expected: MobileNewTaskAttachmentProjection): boolean {
  return !!current && sameStable(current.record, expected.record) && current.record.projectId === expected.record.projectId
    && current.record.revision >= expected.record.revision && current.text === expected.text
    && Object.hasOwn(current.record, 'context') === Object.hasOwn(expected.record, 'context')
    && equal(current.record.context, expected.record.context) && equal(current.images, expected.images)
    && equal(current.files, expected.files) && equal(current.attachmentIds, expected.attachmentIds)
    && equal(current.attachmentOrder, expected.attachmentOrder);
}
function liveIntake(client: MobileDraftClient, entry: Entry, work: FrozenPick): boolean {
  const c = work.capture, target = mobileComposerTarget(client);
  return !!work.source && entry.progress.kind === 'saved' && !entry.progress.publication.slotReleased
    && !work.receipt.retired && currentRoute(client, c.route) && matchingRoute(entry, c.route)
    && sameIdentity(mounted(entry), c.identity) && target.owner === c.target.owner
    && client.generation === c.target.generation && client.threadEpoch === c.threadEpoch
    && mobileCacheCatalogIdentity(fleet.saved, c.target.environmentId) === c.catalog
    && ownsProjection(projection(client, entry), work.source);
}
function issuedAttachments(entry: Entry, files: readonly PickerIntakeFile[]): OrdinaryInventoryAttachment[] {
  return files.map(row => row.kind === 'image' ? { type: 'image', id: row.id,
    image: { id: row.id, name: row.name, mimeType: row.mimeType, sizeBytes: row.sizeBytes } } : {
    type: 'file', id: row.id, file: { id: row.id, contextId: row.id, draftKey: entry.draft.key,
      environmentId: entry.draft.environmentId, name: row.name, mimeType: row.mimeType, sizeBytes: row.sizeBytes,
      source: 'attached', attachmentId: '', status: 'staged',
      ...(row.videoWidth === undefined ? {} : { videoWidth: row.videoWidth }),
      ...(row.videoHeight === undefined ? {} : { videoHeight: row.videoHeight }) },
  });
}
function cancellation(work: FrozenPick, files: readonly PickerIntakeFile[] | null): Continuation {
  return { kind: 'cancelling', request: mobilePickerIntakeRequest(intakeBase(work, 'cancel')), files };
}
function terminal(entry: Entry, work: FrozenPick, next: Extract<Continuation, { kind: 'terminal' }>): void {
  work.continuation = next;
  const accepted = next.outcome === 'finished' ? next.request.publication.acceptedIds : [];
  entry.notice = '';
  if (entry.holds) for (const hold of Object.values(entry.holds.entries)) {
    if (hold.intake?.operationId === work.operation.id && !accepted.includes(hold.request.file.id))
      mobileFileHoldReleaseNeeded(entry.holds, hold.request.requestId);
  }
}
function resumeMatches(work: FrozenPick, proof: ComposerEditorResume): boolean {
  const request = work.handback?.request, event = proof.snapshot;
  return !!request && proof.operationId === request.operationId && proof.operationRevision === request.operationRevision
    && proof.receiptId === request.receiptId && sameIdentity(event, request.identity)
    && event.eventCount === work.receipt.event.eventCount + 1 && event.value === request.document.value
    && equal(event.selection, request.document.selection);
}
function completeCycle(entry: Entry, work: FrozenPick, proof: ComposerEditorResume | null): void {
  entry.completed = { operationRevision: work.operation.revision, frozenEventCount: work.receipt.event.eventCount,
    receiptId: work.receipt.id, proof };
  entry.operation = null; entry.progress = { kind: 'unpublished' }; entry.custody = 'native'; entry.notice = '';
}
function acceptResume(client: MobileDraftClient, entry: Entry, proof: ComposerEditorResume): boolean {
  if (!mobileDraftRecoveryHydrated(client)) return false;
  if (entry.completed && proof.operationRevision <= entry.completed.operationRevision)
    return equal(entry.completed.proof, proof);
  const work = entry.operation;
  if (entry.problem || work?.phase !== 'frozen' || !work.handback || !resumeMatches(work, proof)) return false;
  const handback = work.handback;
  if (handback.proof && !equal(handback.proof, proof)) return false;
  const resumed = mobileComposerEditorAccept(entry.state, proof.snapshot);
  if (!resumed.accepted) return false;
  const newest = handback.newest;
  if (newest && newest.eventCount <= proof.snapshot.eventCount) return false;
  const accepted = newest ? mobileComposerEditorAccept(resumed.state, newest) : resumed;
  if (!accepted.accepted) return false;
  handback.proof = proof;
  const expected = handbackSource(work);
  if (!expected || !ownsProjection(projection(client, entry), expected)) return false;
  entry.revision = registry(client, entry)!.revision;
  if (accepted.state.value !== expected.text && !publishText(client, entry, accepted.state.value)) return false;
  entry.state = accepted.state;
  completeCycle(entry, work, proof); updateDocument(client, entry);
  return true;
}
function acceptRetirement(client: MobileDraftClient, entry: Entry, receipt: Receipt): boolean {
  const work = entry.operation;
  if (!mobileDraftRecoveryHydrated(client) || entry.problem || work?.phase !== 'frozen' || !work.handback || receipt.kind !== 'retired' || !receipt.retired
    || receipt.id === work.receipt.id || !sameIdentity(receipt.identity, work.capture.identity)
    || receipt.event.eventCount <= work.receipt.event.eventCount + 1
    || receipt.event.eventCount <= (work.handback.newest?.eventCount ?? -1)) return false;
  const expected = handbackSource(work);
  if (!expected || !ownsProjection(projection(client, entry), expected)) return false;
  const accepted = mobileComposerEditorAccept(entry.state, receipt.event);
  entry.revision = registry(client, entry)!.revision;
  if (!accepted.accepted || !publishText(client, entry, receipt.event.value)) return false;
  completeCycle(entry, work, work.handback.proof);
  entry.state = accepted.state; entry.custody = 'recovery'; entry.dirty = false; updateDocument(client, entry);
  entry.progress = { kind: 'published', publication: { receipt, revision: entry.revision, slotReleased: false } };
  return true;
}
function handbackSource(work: FrozenPick): MobileNewTaskAttachmentProjection | null {
  const phase = work.continuation;
  return phase.kind === 'produced' ? phase.after : phase.kind === 'terminal' && phase.outcome === 'finished' ? phase.commit.after : work.source;
}
async function advanceProducer(client: MobileDraftClient, owner: Ledger, entry: Entry, work: FrozenPick, native: Native, storage: Files): Promise<void> {
  if (entry.progress.kind !== 'saved' || entry.problem || !work.source || !work.capture.intent) return;
  let phase = work.continuation;
  if (phase.kind === 'producing') return;
  const current = () => owner.entries.includes(entry) && entry.operation === work && work.continuation === phase;
  const finish = (result: NewTaskProducerResult) => {
    const after = projection(client, entry);
    if (after) entry.revision = after.record.revision;
    phase = { kind: 'produced', result, after, saved: false }; work.continuation = phase;
  };
  try {
    if (phase.kind === 'ready') {
      if (!liveIntake(client, entry, work)) {
        terminal(entry, work, { kind: 'terminal', outcome: 'cancelled', files: null }); return;
      }
      phase = { kind: 'producing' }; work.continuation = phase;
      const result = await newTaskEditorProduce(client, work.capture.route, work.capture.target, work.capture.intent,
        work.receipt.event.selection, native, storage);
      if (!current()) return;
      if (result.voice) { phase = { kind: 'voice', message: result.message }; work.continuation = phase; return; }
      finish(result);
    }
    if (phase.kind === 'voice') {
      if (mobileVoiceSnapshot(work.capture.target.editorOwner, client).phase !== 'idle') return;
      finish({ message: phase.message, leave: false, voice: false, effect: null,
        selection: mobileVoiceSelection(client, work.capture.target) });
    }
    if (phase.kind === 'produced' && !phase.saved) {
      if (!phase.result.leave && (!phase.after || !ownsProjection(projection(client, entry), phase.after)))
        throw new ClientError('The producer source changed before it could be saved.');
      await client.persist(storage);
      if (!current()) return;
      phase.saved = true;
      entry.notice = phase.result.message;
      if (phase.result.effect) entry.effect = { ...phase.result.effect, id: work.operation.id };
      if (phase.after) {
        entry.state = { ...entry.state, value: phase.after.text, selection: phase.result.selection ?? work.receipt.event.selection };
        updateDocument(client, entry);
      }
    }
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) {
      entry.notice = error instanceof Error ? error.message : 'The producer source is retained.';
      // A started producer is never invoked twice on an uncertain result.
      if (phase.kind === 'producing') finish({ message: entry.notice, leave: false, voice: false, selection: null, effect: null });
    }
  }
}

export function mobileNewTaskEditorOwnsVoice(client: T3Client, key: string): boolean {
  return ledgers.get(client as MobileDraftClient)?.entries.some(entry => entry.draft.key === key
    && entry.operation?.phase === 'frozen' && entry.operation.capture.intent?.kind === 'voice'
    && entry.progress.kind === 'saved') ?? false;
}
/** Non-editor producers may write only after every native source for this key has settled. */
export function mobileNewTaskEditorAllowsWriter(client: T3Client, key: string, producer: 'external' | 'voice' = 'external'): boolean {
  const owner = ledgers.get(client as MobileDraftClient);
  return !owner || owner.entries.every(entry => entry.draft.key !== key || entry.progress.kind === 'acknowledged'
    || producer === 'voice' && entry.operation?.phase === 'frozen' && entry.operation.capture.intent?.kind === 'voice'
      && entry.progress.kind === 'saved');
}
export function mobileNewTaskEditorEffect(client: MobileDraftClient, route: NewTaskEditorRoute): NewTaskProducerEffect | null {
  return ledger(client).entries.findLast(entry => matchingRoute(entry, route) && entry.effect)?.effect ?? null;
}
export function mobileNewTaskEditorFlushKey(client: MobileDraftClient, route: NewTaskEditorRoute): string {
  const entry = activeEntry(client, route);
  return entry?.dirty ? `${entry.state.identity.renderEpoch}:${entry.state.eventCount}` : '';
}

async function saveObservation(client: MobileDraftClient, entry: Entry, storage: Files): Promise<void> {
  if (!entry.dirty || entry.progress.kind !== 'unpublished') return;
  const attempt = ++entry.attempt, revision = entry.revision, eventCount = entry.state.eventCount;
  await client.persist(storage);
  if (entry.attempt === attempt && entry.custody === 'native' && entry.revision === revision && entry.state.eventCount === eventCount
    && registry(client, entry)?.revision === revision) entry.dirty = false;
}
async function resume(client: MobileDraftClient, owner: Ledger, entry: Entry, work: FrozenPick, native: Native, storage: Files): Promise<void> {
  const terminal = work.continuation;
  if (!mobileDraftRecoveryHydrated(client) || entry.problem || entry.progress.kind !== 'saved'
    || (terminal.kind !== 'terminal' || terminal.outcome === 'absent')
      && (terminal.kind !== 'produced' || !terminal.saved || terminal.result.leave)) return;
  if (!work.handback) {
    const expected = handbackSource(work);
    if (!expected || !liveEntry(client, entry) || !currentRoute(client, work.capture.route)
      || !sameIdentity(mounted(entry), work.capture.identity) || work.receipt.retired || entry.progress.publication.slotReleased
      || !ownsProjection(projection(client, entry), expected) || !equal(registry(client, entry)?.branchChoice, expected.record.branchChoice)
      || !filesReady(client, entry)) return;
    const projected = projectDocument(client, entry, expected.text, expected);
    const selection = terminal.kind === 'produced' ? terminal.result.selection ?? work.receipt.event.selection
      : terminal.kind === 'terminal' && terminal.outcome === 'finished' ? terminal.commit.selection ?? work.receipt.event.selection : work.receipt.event.selection;
    const document = Object.freeze({ value: expected.text, selection: Object.freeze({ ...selection }),
      tokensJson: projected.tokensJson, isNativeEcho: false as const });
    const request: ResumeRequest = Object.freeze({ op: 'composerEditorHandoff', action: 'resume', generation: work.capture.target.generation,
      identity: Object.freeze({ ...work.capture.identity }), operationId: work.operation.id, operationRevision: work.operation.revision,
      receiptId: work.receipt.id, document });
    entry.confirmed = projected.confirmedTokens;
    work.handback = { request, newest: null, proof: null };
  }
  const handback = work.handback, request = handback.request, attempt = ++work.attempt;
  const current = () => mobileDraftRecoveryHydrated(client) && owner.entries.includes(entry)
    && entry.operation === work && work.handback === handback && work.attempt === attempt;
  try {
    if (!handback.proof) {
      const response = await bridgeReply(native, request);
      if (!current()) return;
      if (!response.ok || response.generation !== request.generation || !record(response.value))
        throw new ClientError('The editor handback is retained until its resume can be confirmed.');
      const proof = mobileComposerEditorDecodeResume(response.value.event);
      if (!proof || !acceptResume(client, entry, proof)) throw new ClientError('The resumed editor does not match its saved handback.');
    } else if (!acceptResume(client, entry, handback.proof)) throw new ClientError('The resumed source is retained until it can be published.');
    await saveObservation(client, entry, storage);
  } catch (error) {
    if (letGo(error)) throw error;
    if (current() || entry.completed?.operationRevision === work.operation.revision && entry.dirty)
      entry.notice = error instanceof Error ? error.message : 'The editor handback is retained.';
  }
}
async function savePublication(client: MobileDraftClient, entry: Entry, work: FrozenPick,
  phase: Extract<Continuation, { kind: 'published' }>, storage: Files, current: () => boolean): Promise<PickerIntakeFinish | null> {
  let confirmed: PickerIntakeFinish | null = null;
  const unchanged = () => current() && ownsProjection(projection(client, entry), phase.commit.after);
  if (!unchanged()) throw new ClientError('The attachment draft changed. Its saved publication remains retained.');
  await client.persist({ fs: { ...storage.fs, atomicWriteFile: async (path, bytes) => {
    if (!unchanged()) throw new ClientError('The attachment draft changed before saving.');
    if (path !== 'app:/data/t3-code.json') throw new ClientError('The attachment draft save destination changed.');
    const document: unknown = JSON.parse(new TextDecoder().decode(bytes));
    if (!record(document) || !record(document.mobileNewTaskDrafts) || !record(document.mobileNewTaskDrafts.records)
      || !record(document.drafts) || !record(document.snapshotDrafts) || !Array.isArray(document.composerFiles)
      || !record(document.mobileAttachmentOrder)) throw new ClientError('The saved attachment document is invalid.');
    const key = entry.draft.key, order = document.mobileAttachmentOrder[key] ?? null;
    const candidate = mobilePickerIntakeRequest({ ...intakeBase(work, 'finish'), publication: {
      after: { record: document.mobileNewTaskDrafts.records[key], text: document.drafts[key] ?? '',
        images: document.snapshotDrafts[key] ?? [], files: document.composerFiles.filter(file => record(file) && file.draftKey === key),
        attachmentIds: order ?? [], attachmentOrder: order },
      acceptedIds: phase.commit.acceptedIds, discardedIds: phase.commit.discardedIds,
    } });
    if (candidate.action !== 'finish' || !('record' in candidate.publication.after)
      || !ownsProjection(candidate.publication.after, phase.commit.after)) throw new ClientError('The saved attachment content changed.');
    const { after, acceptedIds, discardedIds } = candidate.publication;
    if (!equal(phase.files.filter(file => after.attachmentIds.includes(file.id)).map(file => file.id), acceptedIds)
      || !equal(phase.files.filter(file => !after.attachmentIds.includes(file.id)).map(file => file.id), discardedIds))
      throw new ClientError('The saved picker partition changed.');
    for (const file of phase.files.filter(file => acceptedIds.includes(file.id))) {
      const row = file.kind === 'image' ? after.images.find(row => row.id === file.id) : after.files.find(row => row.id === file.id);
      if (!row || ['name', 'mimeType', 'sizeBytes', ...(file.kind === 'file' ? ['videoWidth', 'videoHeight'] : [])]
        .some(key => obj(row)[key] !== obj(file)[key])) throw new ClientError('The saved picker bytes changed lane or metadata.');
    }
    await storage.fs.atomicWriteFile(path, bytes);
    if (unchanged()) confirmed = candidate;
  } } });
  return confirmed;
}
async function advanceFrozen(client: MobileDraftClient, owner: Ledger, entry: Entry, work: FrozenPick,
  native: Native, storage: Files): Promise<void> {
  if (entry.progress.kind !== 'saved' || entry.problem || !work.source) return;
  const attempt = ++work.attempt, serial = owner.acceptedRead;
  let phase = work.continuation;
  const current = () => owner.entries.includes(entry) && entry.operation === work && work.continuation === phase
    && work.attempt === attempt && owner.acceptedRead === serial && mobileDraftRecoveryHydrated(client);
  const transition = (next: Continuation) => { work.continuation = next; phase = next; };
  try {
    if (phase.kind === 'ready') {
      if (!liveIntake(client, entry, work)) { terminal(entry, work, { kind: 'terminal', outcome: 'cancelled', files: null }); return; }
      const source = work.source, context = mobileNewTaskContextProject(source.text, source.record.context);
      if (!context.ok) throw new ClientError(context.error);
      const attachments: OrdinaryInventoryAttachment[] = [
        ...source.images.map(image => ({ type: 'image' as const, id: str(image.id), image })),
        ...source.files.map(file => ({ type: 'file' as const, id: file.id, file }))];
      const references = new Set(contextReferences(source.text).filter(ref => ref.kind === 'file').map(ref => ref.id));
      const records = new Map(fileContextRecords(source.files.filter(file => references.has(file.contextId))
        .map(file => ({ ...file, attachmentId: file.id }))).map(row => [str(row.contextId), row]));
      for (const row of context.context?.records ?? []) records.set(str(row.contextId), row);
      const remaining = Math.max(0, 100 - mobileComposerCountAttachmentsAfterSelection({ text: source.text,
        context: { version: 1, records: [...records.values()] }, attachments }, { text: source.text, ...work.receipt.event.selection }));
      const fileLimit = mobileComposerPickerFileLimit(client, work.capture.source);
      const capabilities = obj(obj(client.config.environment).capabilities);
      if (!remaining || work.capture.source === 'files' && !fileLimit
        || work.capture.source === 'photos' && capabilities.attachmentUploads !== true) {
        entry.notice = 'This draft cannot accept these attachments.'; return;
      }
      const request = mobilePickerIntakeRequest({ ...intakeBase(work, 'status'), action: 'pick',
        target: { kind: 'new-task', origin: entry.draft.origin, environmentId: entry.draft.environmentId,
          projectId: entry.draft.projectId, draftKey: entry.draft.key, createdAt: entry.draft.createdAt },
        document: { incarnation: entry.draft.createdAt, revision: source.record.revision },
        source: work.capture.source, remaining, fileLimit });
      if (request.action !== 'pick') return;
      transition({ kind: 'intake', request, dispatched: false });
    }
    if (phase.kind === 'intake') {
      const intake = phase, live = liveIntake(client, entry, work);
      if (!intake.dispatched && !live) { terminal(entry, work, { kind: 'terminal', outcome: 'cancelled', files: null }); return; }
      const request = intake.dispatched ? mobilePickerIntakeRequest(intakeBase(work, 'status')) : intake.request;
      intake.dispatched = true;
      let answer = await mobilePickerIntakeInvoke(request, native, current);
      if (!current()) return;
      if (answer.value?.status === 'absent') {
        if (entry.progress.kind === 'saved' && entry.progress.publication.slotReleased) {
          terminal(entry, work, { kind: 'terminal', outcome: 'absent', files: null }); return;
        }
        if (!liveIntake(client, entry, work)) throw new ClientError('The original picker outcome is still unresolved.');
        answer = await mobilePickerIntakeInvoke(intake.request, native, current);
        if (!current()) return;
      }
      const value = answer.value;
      if (!value) throw new ClientError(answer.message || 'The picker outcome is still unresolved.');
      if (value.files.length > intake.request.remaining) throw new ClientError('The picker inventory exceeds its captured capacity.');
      if (value.status === 'cancelled') { terminal(entry, work, { kind: 'terminal', outcome: 'cancelled', files: value.files }); return; }
      if (value.status === 'staged') transition({ kind: 'staged', request: intake.request, files: value.files });
      else if (value.status === 'picking' && !liveIntake(client, entry, work)) transition(cancellation(work, null));
      else throw new ClientError(value.error || 'The picker is still pending.');
    }
    if (phase.kind === 'staged') {
      const staged = phase;
      if (!liveIntake(client, entry, work) || !staged.files.length) transition(cancellation(work, staged.files));
      else {
        const incoming = issuedAttachments(entry, staged.files);
        entry.holds ??= mobileFileHoldsCreate(staged.request.identity, staged.request.generation, staged.request.target);
        const holds = entry.holds;
        const files = [...work.source.files.filter(file => file.source === 'attached').map(file => ({ file, intake: false })),
          ...incoming.flatMap(row => row.type === 'file' ? [{ file: row.file, intake: true }] : [])];
        for (const candidate of files) {
          if (!current()) return;
          if (!liveIntake(client, entry, work)) break;
          if (!candidate.intake && usableHolds(entry).some(held => equal({ ...held.request.file, status: 'staged', attachmentId: '' },
            { ...candidate.file, status: 'staged', attachmentId: '' }))) continue;
          const previous = Object.values(holds.entries).find(hold => hold.desired === 'retain' && equal(hold.request.file, candidate.file)
            && (hold.intake?.operationId ?? '') === (candidate.intake ? work.operation.id : ''));
          const request = previous?.request ?? mobileFileHoldReserve(holds, candidate.file, localId(),
            candidate.intake ? { operationId: work.operation.id } : undefined);
          if (!mobileFileHoldHeld(holds, request.requestId)) await mobileFileHoldAcquire(holds, request.requestId, native);
          if (!current()) return;
          if (!liveIntake(client, entry, work)) break;
          if (!mobileFileHoldHeld(holds, request.requestId)) throw new ClientError('The attachment bytes are retained until their holds are confirmed.');
        }
        if (!liveIntake(client, entry, work)) transition(cancellation(work, staged.files));
        else {
          const draft = registry(client, entry);
          if (!draft) return;
          const result = mobileNewTaskContextAttach(client, { guard: { key: draft.key, createdAt: draft.createdAt, revision: draft.revision },
            destination: entry.draft, insertion: { text: work.receipt.event.value, ...work.receipt.event.selection }, incoming });
          if (result.kind === 'refused') { transition(cancellation(work, staged.files)); }
          else if (!result.commit.acceptedIds.length) transition(cancellation(work, staged.files));
          else { entry.revision = result.commit.after.record.revision;
            transition({ kind: 'published', request: staged.request, files: staged.files, commit: result.commit }); }
        }
      }
    }
    if (phase.kind === 'published') {
      const published = phase, request = await savePublication(client, entry, work, published, storage, current);
      if (!current()) return;
      if (!request) throw new ClientError('The attachment draft has no confirmed saved publication.');
      transition({ kind: 'finishing', request, files: published.files, commit: published.commit });
    }
    if (phase.kind === 'finishing' || phase.kind === 'cancelling') {
      const settling = phase, answer = await mobilePickerIntakeInvoke(settling.request, native, current);
      if (!current()) return;
      if (!answer.value || settling.files && !equal(answer.value.files, settling.files))
        throw new ClientError(answer.message || 'The issued picker inventory could not be confirmed.');
      if (settling.kind === 'finishing' && answer.value.status === 'finished')
        terminal(entry, work, { ...settling, kind: 'terminal', outcome: 'finished' });
      else if (settling.kind === 'cancelling' && answer.value.status === 'cancelled')
        terminal(entry, work, { kind: 'terminal', outcome: 'cancelled', files: answer.value.files });
      else throw new ClientError('The attachment terminal outcome is still unresolved.');
      phase = work.continuation;
    }
    if (phase.kind === 'terminal') await releasePendingFiles(entry, native, current);
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) entry.notice = error instanceof Error ? error.message : 'The attachment operation is retained.';
  }
}
async function recover(client: MobileDraftClient, native: Native, storage: Files): Promise<void> {
  const owner = ledger(client), serial = ++owner.serial, generation = client.generation;
  const candidates = releaseCandidates(owner, serial);
  try {
    const response = await bridgeReply(native, { op: 'composerEditorFinalizations', generation, action: 'read' });
    if (serial < owner.acceptedRead || !mobileDraftRecoveryHydrated(client)) return;
    if (!response.ok || response.generation !== generation) throw new ClientError('The retained editor sources could not be read.');
    const snapshot = decodeSnapshot(response.value);
    if (!snapshot) throw new ClientError('The retained editor sources are invalid.');
    const { receipts } = snapshot;
    owner.acceptedRead = serial; owner.problem = '';
    for (const receipt of receipts) {
      const entry = owner.entries.find(candidate => sameIdentity(mounted(candidate), receipt.identity));
      if (!entry) { owner.problem = 'An editor source has no captured draft owner.'; continue; }
      if (!acceptRetirement(client, entry, receipt) && !publish(client, entry, receipt)) continue;
      try { await settle(client, owner, entry, native, storage, serial); }
      catch (error) {
        if (letGo(error)) throw error;
        if (serial === owner.acceptedRead) { owner.problem = 'The final editor source is retained until it can be saved.'; await wake(owner, native, client.generation); }
      }
      if (serial < owner.acceptedRead) return;
    }
    reconcileSnapshot(client, owner, serial, snapshot, candidates);
    for (const entry of owner.entries) {
      try {
        await prepareFiles(client, entry, native);
        await continuePick(client, owner, entry, native, storage);
        await saveObservation(client, entry, storage);
      }
      catch (error) {
        if (letGo(error)) throw error;
        if (serial === owner.acceptedRead) {
          entry.notice = 'The editor handoff is retained until it can be confirmed.';
          await wake(owner, native, client.generation);
        }
      }
      if (serial < owner.acceptedRead) return;
    }
    if (!receipts.length && owner.entries.every(entry => entry.progress.kind === 'acknowledged')) owner.wakeUsed = false;
  } catch (error) {
    if (letGo(error)) throw error;
    if (serial < owner.acceptedRead) return;
    owner.problem = error instanceof Error ? error.message : 'The final editor source is retained.';
    await wake(owner, native, client.generation);
  }
}

export async function mobileNewTaskEditorRefresh(client: MobileDraftClient, native: Native | null | undefined, storage: Files): Promise<void> {
  if (native?.available) native.watch('t3.composer-finalizations');
  await client.refresh(native, storage);
  if (native?.available && mobileDraftRecoveryHydrated(client)) await recover(client, native, storage);
}

export async function mobileNewTaskEditorAction(client: MobileDraftClient, route: NewTaskEditorRoute, intent: NewTaskEditorIntent,
  native: Native | null | undefined, storage: Files): Promise<NewTaskEditorResult> {
  if (intent.kind === 'retry') {
    const owner = ledger(client); owner.wakeUsed = false;
    if (native?.available && mobileDraftRecoveryHydrated(client)) {
      native.watch('t3.composer-finalizations'); await recover(client, native, storage);
    }
    return { accepted: true, message: owner.problem };
  }
  if (intent.kind === 'claim') {
    const entry = ledger(client).entries.find(candidate => matchingRoute(candidate, route) && candidate.effect?.id === intent.id);
    if (!entry?.effect || !mobileNewTaskFlowCurrent(route.owner, route.visit, route.location, client))
      return { accepted: false, message: '' };
    const effect = entry.effect; entry.effect = null; return { accepted: true, message: '', effect };
  }
  let entry = activeEntry(client, route);
  if (intent.kind === 'observe') {
    const event = mobileComposerEditorDecodeResume(intent.payload)?.snapshot ?? mobileComposerEditorDecodeEvent(intent.payload);
    if (event) entry = ledger(client).entries.find(candidate =>
      (sameIdentity(mounted(candidate), event) || candidate === entry && event.kind === 'ready' && !candidate.state.mountId
        && sameIdentity({ ...mounted(candidate), mountId: event.mountId }, event))
      && (candidate === entry || candidate.operation?.phase === 'frozen' && candidate.operation.handback !== null || candidate.completed !== null)) ?? null;
  }
  if (intent.kind !== 'observe') {
    if (entry && native?.available) await prepareFiles(client, entry, native);
    if (!entry || !native?.available || entry.operation || entry.problem || ledger(client).problem
      || entry.progress.kind !== 'unpublished' || entry.custody !== 'native' || !entry.state.mountId
      || entry.state.composing || entry.state.pendingCommand || entry.operationRevision >= MAX_COUNT
      || registry(client, entry)?.revision !== entry.revision || !liveEntry(client, entry) || !filesReady(client, entry))
      return { accepted: false, message: 'The editor is not ready for an attachment operation.' };
    if (intent.kind === 'preview' && intent.token) {
      let raw: Record<string, unknown>; try { raw = obj(JSON.parse(intent.token)); } catch { return { accepted: false, message: 'The attachment event is invalid.' }; }
      if (!sameIdentity(raw as unknown as ComposerMountedIdentity, mounted(entry)) || raw.eventCount !== entry.state.eventCount
        || raw.value !== entry.state.value || !token(raw.richEventId)) return { accepted: false, message: 'The attachment source changed.' };
    }
    const capture: PickCapture = { source: intent.kind === 'pick' ? intent.source : 'files',
      ...(intent.kind === 'pick' ? {} : { intent: JSON.parse(JSON.stringify(intent)) as NewTaskProducerIntent }), route: { ...route }, draft: { ...entry.draft }, identity: mounted(entry),
      target: mobileComposerTarget(client), threadEpoch: client.threadEpoch,
      catalog: mobileCacheCatalogIdentity(fleet.saved, entry.draft.environmentId), revision: entry.revision,
      eventCount: entry.state.eventCount, value: entry.state.value, selection: { ...entry.state.selection } };
    if (!pickCurrent(client, entry, capture)) return { accepted: false, message: 'The attachment editor changed.' };
    const owner = ledger(client);
    entry.attempt++; entry.operation = { phase: 'allocating', capture, revision: ++entry.operationRevision, attempt: 0 };
    try { await continuePick(client, owner, entry, native, storage); }
    catch (error) {
      if (letGo(error)) throw error;
      entry.notice = error instanceof Error ? error.message : 'The editor handoff is retained.';
      await wake(owner, native, client.generation);
    }
    return { accepted: true, message: entry.problem || entry.notice || owner.problem };
  }
  if (!entry || !observe(client, entry, intent.payload)) return { accepted: false, message: 'The editor observation is no longer current.' };
  if (native?.available) await prepareFiles(client, entry, native);
  if (entry.dirty) {
    try {
      await saveObservation(client, entry, storage);
    } catch (error) {
      if (letGo(error)) throw error;
      return { accepted: true, message: 'The draft could not be saved. Its editor source is retained.' };
    }
  }
  return { accepted: true, message: '' };
}
