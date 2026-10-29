// Source365aa87982 composerContext.ts and use-composer-drafts.ts; app-owned full-key payloads.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { obj, str, type Obj, type Json } from './shared/domain';
import { contextId, contextLabel, contextLink, contextReferences, stripContextReferences } from './shared/composer-editor-menu';
import { setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { mobileDraftAttachmentRecord } from './draft-attachment-order';
import type { OrdinaryInventoryAttachment } from './composer-attachment-publication';
import type { ComposerMountedIdentity } from './composer-editor-state';
import type { HeldFile, NewTaskFileHoldTarget } from './composer-file-holds-io';
import { mobileComposerFileScope } from './composer-file-scope';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileComposerInsertContext, type MobileComposerInsertion } from './composer-context-insertion';
import type { ComposerPickerDraft, ComposerPickerPlan } from './composer-picker-plan';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileNewTaskDraftClone as clone, mobileNewTaskDraftStore, mobileNewTaskDraftIsKey,
  mobileNewTaskDraftIsPendingKey, type MobileNewTaskDraft } from './mobile-new-task-drafts';

export interface MobileMessageContext { version: 1; records: Obj[] }
export interface MobileNewTaskContextGuard { key: string; createdAt: string; revision: number }
export type MobileNewTaskContextResult = { ok: true; context: MobileMessageContext | undefined } | { ok: false; error: string };
const invalid = () => ({ ok: false as const, error: 'This draft contains unsupported or invalid context.' });

/** Source ordered dependency pass. Repeated references need only one identity here. */
export function mobileReferencedComposerContext(text: string, context?: MobileMessageContext): MobileMessageContext | undefined {
  if (!context) return undefined;
  const ids = new Set(contextReferences(text).map(reference => reference.id));
  for (const record of context.records) {
    if (ids.has(str(record.contextId)) && record.kind === 'preview-annotation' && record.screenshotContextId)
      ids.add(str(record.screenshotContextId));
  }
  const records = context.records.filter(record => ids.has(str(record.contextId)));
  if (records.length === context.records.length) return context;
  return records.length ? { version: 1, records } : undefined;
}

/** Pinned context history retains max(200, current live payload count) records. */
export function mobileCreateContextHistory() {
  const records = new Map<string, Obj>();
  const restore = (text: string, current?: MobileMessageContext): MobileMessageContext | undefined => {
    for (const record of current?.records ?? []) {
      records.delete(str(record.contextId)); records.set(str(record.contextId), clone(record));
    }
    const limit = Math.max(200, current?.records.length ?? 0);
    while (records.size > limit) records.delete(records.keys().next().value!);
    return mobileReferencedComposerContext(text, { version: 1, records: [...records.values()] });
  };
  // Batch publication must inspect dependency records without changing undo history.
  return Object.assign(restore, { snapshot: (): Obj[] => [...records.values()].map(record => clone(record)) });
}
function decode(raw: unknown): MobileNewTaskContextResult {
  if (raw === undefined) return { ok: true, context: undefined };
  const value = obj(raw);
  if (value.version !== 1 || !Array.isArray(value.records)) return invalid();
  const records: Obj[] = [], ids = new Set<string>();
  for (const rawRecord of value.records) {
    const record = obj(rawRecord);
    if (!mobileContextRecordValid(record) || ids.has(str(record.contextId))) return invalid();
    ids.add(str(record.contextId)); records.push(record);
  }
  return { ok: true, context: { version: 1, records } };
}
/** Validate before pruning, so removing a link never launders malformed saved payloads.
 * Attachment synthesis and missing-reference checks remain the capture owner's responsibility. */
export function mobileNewTaskContextProject(text: string, raw: unknown): MobileNewTaskContextResult {
  const decoded = decode(raw);
  if (!decoded.ok) return decoded;
  const context = mobileReferencedComposerContext(text, decoded.context);
  if (context && (context.records.length > 200 || JSON.stringify(context.records).length > 16000000))
    return { ok: false, error: 'This draft has too much context to send.' };
  return { ok: true, context: context ? clone(context) : undefined };
}
export function mobileNewTaskContextRead(client: T3Client, key: string): MobileNewTaskContextResult {
  const record = mobileNewTaskDraftStore(client).records[key];
  return record ? mobileNewTaskContextProject(client.local.drafts[key] ?? '', record.context) : invalid();
}
export function mobileNewTaskContextGuard(client: T3Client, key: string): MobileNewTaskContextGuard | null {
  const record = mobileNewTaskDraftStore(client).records[key];
  return record ? { key, createdAt: record.createdAt, revision: record.revision } : null;
}
// Keying by the actual registry record makes hydration/discard discard its undo history too.
const histories = new WeakMap<MobileNewTaskDraft, ReturnType<typeof mobileCreateContextHistory>>();
function detachedHistory(record: MobileNewTaskDraft) {
  const history = mobileCreateContextHistory(), prior = histories.get(record)?.snapshot();
  if (prior) history('', { version: 1, records: prior });
  return history;
}
/** Atomically write the producer's text and frozen payload. The caller owns route/focus admission.
 * Ordinary typing preserves invalid raw context; producer insertion/removal must not overwrite it. */
export function mobileNewTaskContextWrite(client: T3Client, guard: MobileNewTaskContextGuard, text: string,
  added?: Obj, removeId = ''): boolean {
  const record = mobileNewTaskDraftStore(client).records[guard.key];
  if (!record || record.createdAt !== guard.createdAt || record.revision !== guard.revision) return false;
  const decoded = decode(record.context), producer = added !== undefined || !!removeId;
  if ((!decoded.ok && producer) || (added !== undefined && !mobileContextRecordValid(added))) return false;
  let next: MobileMessageContext | undefined;
  if (decoded.ok) {
    const history = histories.get(record) ?? mobileCreateContextHistory();
    const restored = history(text, decoded.context);
    if (producer) {
      const records = new Map((restored?.records ?? []).map(entry => [str(entry.contextId), entry]));
      if (removeId) records.delete(removeId);
      if (added) records.set(str(added.contextId), clone(added));
      next = mobileReferencedComposerContext(text, { version: 1, records: [...records.values()] });
      if ((next?.records.length ?? 0) > 200) return false;
    } else next = restored;
    histories.set(record, history);
    if (next === undefined) delete record.context;
    else record.context = clone(next);
  }
  client.local.drafts[guard.key] = text;
  record.revision++; client.revision++;
  return true;
}

export interface MobileNewTaskAttachmentBatch {
  readonly guard: MobileNewTaskContextGuard;
  readonly destination: Pick<MobileNewTaskDraft, 'origin' | 'environmentId' | 'projectId'>;
  readonly insertion: MobileComposerInsertion;
  /** Issued intake rows whose holds the caller already owns. This operation grants no byte authority. */
  readonly incoming: readonly OrdinaryInventoryAttachment[];
}
export interface MobileNewTaskAttachmentProjection {
  readonly record: MobileNewTaskDraft;
  readonly text: string;
  readonly images: readonly Obj[];
  readonly files: readonly DraftFile[];
  readonly attachmentIds: readonly string[];
  readonly attachmentOrder: readonly string[] | null;
}
export interface MobileNewTaskAttachmentCommit {
  readonly applied: boolean;
  readonly after: MobileNewTaskAttachmentProjection;
  readonly selection: { readonly start: number; readonly end: number } | null;
  readonly acceptedIds: readonly string[];
  readonly discardedIds: readonly string[];
  readonly removed: readonly OrdinaryInventoryAttachment[];
}
export type MobileNewTaskAttachmentResult =
  | { kind: 'committed'; commit: MobileNewTaskAttachmentCommit }
  | { kind: 'refused'; reason: 'stale' | 'invalid-target' | 'invalid-context' | 'invalid-inventory' | 'exhausted' | 'unsupported' };

const plain = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object'
  && !Array.isArray(value) && [Object.prototype, null].includes(Object.getPrototypeOf(value));
const nonempty = (value: unknown): value is string => typeof value === 'string' && value.length > 0;
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(nonempty);
const uuid = (value: string) => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);

/** Serialization must not silently drop a malformed saved value before validation. */
function batchJSON(value: unknown, parents = new Set<object>()): value is Json {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if ((!Array.isArray(value) && !plain(value)) || parents.has(value)) return false;
  const descriptors = Object.getOwnPropertyDescriptors(value), keys = Reflect.ownKeys(value);
  if (keys.some(key => typeof key !== 'string')) return false;
  if (Array.isArray(value) && (keys.length !== value.length + 1
    || !Array.from({ length: value.length }, (_, index) => Object.hasOwn(value, index)).every(Boolean))) return false;
  parents.add(value);
  const valid = Object.entries(descriptors).every(([key, descriptor]) => Array.isArray(value) && key === 'length'
    || descriptor.enumerable && 'value' in descriptor && batchJSON(descriptor.value, parents));
  parents.delete(value);
  return valid;
}
function batchImage(value: unknown): value is Obj {
  return plain(value) && batchJSON(value) && nonempty(value.id) && nonempty(value.name) && nonempty(value.mimeType)
    && integer(value.sizeBytes) && (!Object.hasOwn(value, 'uploadId') || typeof value.uploadId === 'string');
}
function batchFile(value: unknown): value is DraftFile {
  return plain(value) && batchJSON(value) && nonempty(value.id) && nonempty(value.contextId)
    && nonempty(value.draftKey) && nonempty(value.environmentId) && nonempty(value.name) && nonempty(value.mimeType)
    && nonempty(value.source) && integer(value.sizeBytes) && typeof value.attachmentId === 'string'
    && (value.status === 'staged' || value.status === 'ready' && nonempty(value.attachmentId))
    && ['videoWidth', 'videoHeight'].every(key => !Object.hasOwn(value, key)
      || typeof value[key] === 'number' && Number.isFinite(value[key]) && value[key] > 0);
}
interface BatchInventory {
  images: Record<string, Obj[]>;
  files: DraftFile[];
  orders: Record<string, string[]>;
  imageReleases: string[];
  fileReleases: string[];
}
function batchInventory(local: Record<string, unknown>, fileReleases: unknown): BatchInventory | null {
  const rawImages = local.snapshotDrafts, rawFiles = local.composerFiles === undefined ? [] : local.composerFiles,
    rawOrders = local.mobileAttachmentOrder === undefined ? {} : local.mobileAttachmentOrder;
  if (!plain(rawImages) || !Array.isArray(rawFiles) || !plain(rawOrders) || !strings(local.snapshotReleases)
    || !strings(fileReleases) || !batchJSON([rawImages, rawFiles, rawOrders, local.snapshotReleases, fileReleases])) return null;
  const images: Record<string, Obj[]> = {}, files: DraftFile[] = [], orders: Record<string, string[]> = {};
  const ids = new Map<string, Set<string>>(), contextIds = new Map<string, Set<string>>();
  const unique = (map: Map<string, Set<string>>, key: string, id: string) => {
    const values = map.get(key) ?? new Set<string>();
    if (values.has(id)) return false;
    values.add(id); map.set(key, values); return true;
  };
  for (const [key, rows] of Object.entries(rawImages)) {
    if (!key || !Array.isArray(rows) || !rows.every(batchImage)) return null;
    for (const row of rows) if (!unique(ids, key, str(row.id))
      || !unique(contextIds, key, contextId('image', str(row.id)))) return null;
    Object.defineProperty(images, key, { value: clone(rows), enumerable: true, writable: true, configurable: true });
  }
  for (const row of rawFiles) {
    if (!batchFile(row) || !unique(ids, row.draftKey, row.id) || !unique(contextIds, row.draftKey, row.contextId)) return null;
    files.push(clone(row));
  }
  for (const [key, order] of Object.entries(rawOrders)) {
    if (!key || !strings(order) || new Set(order).size !== order.length) return null;
    Object.defineProperty(orders, key, { value: [...order], enumerable: true, writable: true, configurable: true });
  }
  return { images, files, orders, imageReleases: [...local.snapshotReleases], fileReleases: [...fileReleases] };
}
function batchAttachments(inventory: BatchInventory, key: string): OrdinaryInventoryAttachment[] {
  const rows: OrdinaryInventoryAttachment[] = [
    ...(inventory.images[key] ?? []).map(image => ({ type: 'image' as const, id: str(image.id), image })),
    ...inventory.files.filter(file => file.draftKey === key).map(file => ({ type: 'file' as const, id: file.id, file }))];
  const byId = new Map(rows.map(row => [row.id, row]));
  return [...new Set([...(inventory.orders[key] ?? []), ...byId.keys()])].flatMap(id => {
    const row = byId.get(id); return row ? [row] : [];
  });
}
function batchAttachmentRecord(attachment: OrdinaryInventoryAttachment): Obj {
  const row = attachment.type === 'image' ? attachment.image : attachment.file;
  return { version: 1, kind: attachment.type, contextId: attachment.type === 'image'
    ? contextId('image', attachment.id) : attachment.file.contextId,
    label: contextLabel(str(row.name), attachment.type), attachmentId: attachment.id,
    name: str(row.name), mimeType: str(row.mimeType), sizeBytes: Number(row.sizeBytes) };
}
function batchIncoming(raw: unknown, record: MobileNewTaskDraft,
  current: readonly OrdinaryInventoryAttachment[], records: readonly Obj[]): OrdinaryInventoryAttachment[] | null {
  if (!Array.isArray(raw) || !batchJSON(raw)) return null;
  const incoming: OrdinaryInventoryAttachment[] = [];
  const ids = new Set(current.map(row => row.id)), contextIds = new Set(records.map(row => str(row.contextId)));
  for (const attachment of current) contextIds.add(str(batchAttachmentRecord(attachment).contextId));
  for (const value of raw) {
    if (!plain(value) || !nonempty(value.id) || !uuid(value.id) || ids.has(value.id)) return null;
    let attachment: OrdinaryInventoryAttachment;
    if (value.type === 'image') {
      if (!batchImage(value.image) || value.image.id !== value.id) return null;
      attachment = { type: 'image', id: value.id, image: clone(value.image) };
    } else {
      if (value.type !== 'file' || !batchFile(value.file) || value.file.id !== value.id
        || value.file.draftKey !== record.key || value.file.environmentId !== record.environmentId
        || value.file.source !== 'attached' || value.file.status !== 'staged' || value.file.attachmentId !== '') return null;
      attachment = { type: 'file', id: value.id, file: clone(value.file) };
    }
    const context = batchAttachmentRecord(attachment), id = str(context.contextId);
    if (!mobileContextRecordValid(context) || contextIds.has(id)) return null;
    ids.add(attachment.id); contextIds.add(id);
    incoming.push(attachment);
  }
  return incoming;
}
function batchContextCompatible(text: string, context: MobileMessageContext | undefined,
  attachments: readonly OrdinaryInventoryAttachment[], history: readonly Obj[]): boolean {
  const records = context?.records ?? [];
  for (const ref of contextReferences(text)) {
    if (!records.some(record => record.contextId === ref.id && record.kind === ref.kind)) return false;
  }
  for (const record of records) {
    if (record.kind === 'preview-annotation' && record.screenshotContextId
      && !records.some(image => image.contextId === record.screenshotContextId && image.kind === 'image')) return false;
    if (record.kind !== 'image' && record.kind !== 'file') continue;
    const attachment = attachments.find(row => row.id === record.attachmentId);
    if (!attachment) {
      if (record.kind === 'image' && history.some(saved => JSON.stringify(saved) === JSON.stringify(record))) continue;
      return false;
    }
    const expected = batchAttachmentRecord(attachment);
    if (attachment.type !== record.kind || record.kind === 'file' && expected.contextId !== record.contextId
      || ['name', 'mimeType', 'sizeBytes'].some(field => record[field] !== expected[field])) return false;
  }
  return true;
}

/** A later file can free a selected file even when an earlier shelf-only prefix is full.
 * Evaluate whole prefixes, retaining picker order while references use file/image order. */
function planNewTaskAttachments(draft: ComposerPickerDraft, context: MobileMessageContext,
  incoming: readonly OrdinaryInventoryAttachment[], insertion: MobileComposerInsertion): ComposerPickerPlan {
  const unchanged: ComposerPickerPlan = { draft, selection: null, accepted: [], rejected: incoming, removed: [], applied: false };
  const imageReferences = stripContextReferences(insertion.text).trim().length > 0;
  for (let count = Math.min(100, incoming.length); count > 0; count--) {
    const accepted = incoming.slice(0, count), referenced = [...accepted.filter(row => row.type === 'file'),
      ...accepted.filter(row => row.type === 'image' && imageReferences)], records = referenced.map(batchAttachmentRecord);
    if (records.length === 0) {
      if (draft.attachments.length + count <= 100 && (draft.context?.records.length ?? 0) <= 200
        && JSON.stringify(draft.context?.records ?? []).length <= 16_000_000)
        return { draft: { ...draft, attachments: [...draft.attachments, ...accepted] }, selection: null,
          accepted, rejected: incoming.slice(count), removed: [], applied: true };
      continue;
    }
    const next = mobileComposerInsertContext({ ...draft, context }, { attachments: accepted,
      text: records.map(record => contextLink(str(record.kind), str(record.contextId), str(record.label))).join(' '),
      context: { version: 1, records } }, insertion);
    if (!next || next.draft.attachments.length > 100 || next.draft.text.length > 1_000_000
      || JSON.stringify(next.draft.context?.records ?? []).length > 16_000_000) continue;
    return { draft: next.draft, selection: next.selection, accepted, rejected: incoming.slice(count), removed: next.removed, applied: true };
  }
  return unchanged;
}

/** Captured destination publication only. Persistence, finish receipts and byte cleanup belong to its producer. */
export function mobileNewTaskContextAttach(client: T3Client, batch: MobileNewTaskAttachmentBatch): MobileNewTaskAttachmentResult {
  const refuse = (reason: Extract<MobileNewTaskAttachmentResult, { kind: 'refused' }>['reason']): MobileNewTaskAttachmentResult => ({ kind: 'refused', reason });
  const { guard, destination, insertion } = batch;
  if (!mobileNewTaskDraftIsKey(guard.key) || mobileNewTaskDraftIsPendingKey(guard.key)) return refuse('invalid-target');
  const local = obj(client.local), rawStore = local.mobileNewTaskDrafts;
  if (!plain(rawStore) || rawStore.version !== 1 || !plain(rawStore.records) || !plain(rawStore.receipts)
    || !plain(rawStore.claims) || !Object.values(rawStore.claims).every(value => typeof value === 'string')
    || !strings(rawStore.fileReleases)) return refuse('invalid-inventory');
  const store = mobileNewTaskDraftStore(client), record = store.records[guard.key];
  if (!record || record.key !== guard.key || record.createdAt !== guard.createdAt || record.revision !== guard.revision
    || record.origin !== destination.origin || record.environmentId !== destination.environmentId
    || record.projectId !== destination.projectId) return refuse('stale');
  if (!nonempty(record.environmentId) || !nonempty(record.projectId) || !/^https?:\/\//.test(record.origin)
    || !Number.isFinite(Date.parse(record.createdAt))) return refuse('invalid-target');
  if (!integer(record.revision) || record.revision === Number.MAX_SAFE_INTEGER
    || !integer(client.revision) || client.revision === Number.MAX_SAFE_INTEGER) return refuse('exhausted');
  const decoded = decode(record.context);
  if (!decoded.ok || record.context !== undefined && !batchJSON(record.context)) return refuse('invalid-context');
  if (!batchJSON(rawStore)) return refuse('invalid-inventory');
  if (!plain(local.drafts)) return refuse('invalid-context');
  const text = local.drafts[guard.key] === undefined ? '' : local.drafts[guard.key];
  if (typeof text !== 'string' || text.length > 1_000_000 || typeof insertion.text !== 'string'
    || insertion.text.length > 1_000_000 || !Number.isSafeInteger(insertion.start) || !Number.isSafeInteger(insertion.end)) return refuse('invalid-context');
  const inventory = batchInventory(local, rawStore.fileReleases);
  if (!inventory || inventory.files.some(file => file.draftKey === guard.key && file.environmentId !== destination.environmentId)) return refuse('invalid-inventory');
  const attachments = batchAttachments(inventory, guard.key), prior = histories.get(record)?.snapshot() ?? [];
  if (!batchContextCompatible('', decoded.context, attachments, prior)) return refuse('invalid-context');
  const merged = new Map(prior.map(row => [str(row.contextId), row]));
  for (const row of decoded.context?.records ?? []) merged.set(str(row.contextId), clone(row));
  for (const ref of contextReferences(text)) {
    if (merged.has(ref.id)) continue;
    const attachment = attachments.find(row => row.type === ref.kind && batchAttachmentRecord(row).contextId === ref.id);
    if (attachment) merged.set(ref.id, batchAttachmentRecord(attachment));
  }
  const context: MobileMessageContext = { version: 1, records: [...merged.values()] };
  if (!batchContextCompatible(text, mobileReferencedComposerContext(text, context), attachments, prior)) return refuse('invalid-context');
  const incoming = batchIncoming(batch.incoming, record, attachments, context.records);
  if (!incoming) return refuse('invalid-inventory');
  const plan = planNewTaskAttachments({ text, context: decoded.context, attachments }, context, incoming, insertion);
  if (plan.selection && !batchContextCompatible(plan.draft.text, mobileReferencedComposerContext(plan.draft.text, plan.draft.context), plan.draft.attachments, prior))
    return refuse('invalid-context');
  if (plan.removed.some(row => !uuid(row.id) || row.type === 'file' && row.file.source !== 'attached')) return refuse('unsupported');
  const nextRecord = clone(record), images = plan.applied
    ? plan.draft.attachments.flatMap(row => row.type === 'image' ? [row.image] : []) : inventory.images[guard.key] ?? [],
    files = plan.applied ? plan.draft.attachments.flatMap(row => row.type === 'file' ? [row.file] : [])
      : inventory.files.filter(file => file.draftKey === guard.key), attachmentIds = plan.draft.attachments.map(row => row.id);
  if (plan.applied) {
    nextRecord.revision++;
    if (plan.draft.context === undefined) delete nextRecord.context; else nextRecord.context = clone(plan.draft.context);
  }
  const commit: MobileNewTaskAttachmentCommit = clone({ applied: plan.applied,
    after: { record: nextRecord, text: plan.draft.text, images, files, attachmentIds,
      attachmentOrder: plan.applied ? attachmentIds : inventory.orders[guard.key] ?? null }, selection: plan.selection,
    acceptedIds: plan.accepted.map(row => row.id), discardedIds: plan.rejected.map(row => row.id), removed: plan.removed });
  if (!plan.applied) return { kind: 'committed', commit };
  const history = mobileCreateContextHistory();
  if (plan.selection) {
    history('', { version: 1, records: prior });
    history(plan.draft.text, context);
    history(plan.draft.text, plan.draft.context);
  }
  const nextImages = { ...inventory.images, [guard.key]: clone(images) }, nextFiles = [
    ...inventory.files.filter(file => file.draftKey !== guard.key), ...clone(files)];
  for (const row of plan.removed) {
    const releases = row.type === 'image' ? inventory.imageReleases : inventory.fileReleases;
    if (!releases.includes(row.id)) releases.push(row.id);
  }
  client.local.drafts[guard.key] = plan.draft.text;
  client.local.snapshotDrafts = nextImages;
  setDraftFiles(client.local, nextFiles);
  client.local.snapshotReleases = inventory.imageReleases;
  store.fileReleases = inventory.fileReleases;
  mobileDraftAttachmentRecord(client, guard.key, attachmentIds);
  if (nextRecord.context === undefined) delete record.context; else record.context = nextRecord.context;
  record.revision = nextRecord.revision;
  if (plan.selection) histories.set(record, history);
  client.revision++;
  return { kind: 'committed', commit };
}

export interface MobileNewTaskObservation {
  readonly guard: MobileNewTaskContextGuard;
  readonly destination: MobileNewTaskAttachmentBatch['destination'];
  readonly identity: ComposerMountedIdentity;
  readonly generation: number;
  readonly text: string;
  readonly usableHolds: readonly HeldFile[];
}
export type MobileNewTaskObservationResult =
  | { kind: 'committed'; revision: number; after: MobileNewTaskAttachmentProjection; removed: readonly OrdinaryInventoryAttachment[] }
  | { kind: 'refused'; reason: 'stale' | 'invalid-target' | 'invalid-inventory' | 'invalid-context' | 'exhausted' | 'unusable-hold' };

/** The caller supplies IO-usable holds and admits the captured mount. This only publishes local state. */
export function mobileNewTaskContextObserve(client: T3Client, input: MobileNewTaskObservation): MobileNewTaskObservationResult {
  const refuse = (reason: Extract<MobileNewTaskObservationResult, { kind: 'refused' }>['reason']): MobileNewTaskObservationResult => ({ kind: 'refused', reason });
  const { guard, destination, identity, generation, text, usableHolds } = input;
  const target: NewTaskFileHoldTarget = { kind: 'new-task', ...destination, draftKey: guard.key, createdAt: guard.createdAt };
  if (!mobileNewTaskDraftIsKey(guard.key) || mobileNewTaskDraftIsPendingKey(guard.key)
    || !plain(identity) || !['owner', 'editorId', 'routeVisit', 'renderEpoch', 'mountId'].every(key => nonempty(identity[key]))
    || !integer(generation) || mobileComposerFileScope(identity.owner, target)?.generation !== generation) return refuse('invalid-target');
  const local = obj(client.local), rawStore = local.mobileNewTaskDrafts;
  if (!plain(rawStore) || rawStore.version !== 1 || !plain(rawStore.records) || !plain(rawStore.receipts)
    || !plain(rawStore.claims) || !Object.values(rawStore.claims).every(value => typeof value === 'string')
    || !batchJSON(rawStore) || !plain(local.drafts)) return refuse('invalid-inventory');
  const store = mobileNewTaskDraftStore(client), record = store.records[guard.key];
  if (!record || record.key !== guard.key || record.createdAt !== guard.createdAt || record.revision !== guard.revision
    || record.origin !== destination.origin || record.environmentId !== destination.environmentId
    || record.projectId !== destination.projectId) return refuse('stale');
  if (!integer(record.revision) || record.revision === Number.MAX_SAFE_INTEGER
    || !integer(client.revision) || client.revision === Number.MAX_SAFE_INTEGER) return refuse('exhausted');
  if (typeof text !== 'string' || typeof (local.drafts[guard.key] ?? '') !== 'string') return refuse('invalid-context');
  const inventory = batchInventory(local, rawStore.fileReleases);
  if (!inventory || inventory.files.some(file => file.draftKey === guard.key && file.environmentId !== destination.environmentId))
    return refuse('invalid-inventory');
  const attachments = batchAttachments(inventory, guard.key), decoded = decode(record.context), nextRecord = clone(record);
  let nextAttachments = attachments, removed: OrdinaryInventoryAttachment[] = [];
  let history: ReturnType<typeof mobileCreateContextHistory> | undefined;
  if (decoded.ok) {
    if (!Array.isArray(usableHolds) || !batchJSON([...usableHolds])) return refuse('unusable-hold');
    const held = new Map<string, DraftFile>();
    const equal = (a: unknown, b: unknown) => canonical(a) === canonical(b);
    const metadata = (file: DraftFile) => ({ ...file, attachmentId: '', status: 'staged' });
    for (const binding of usableHolds) {
      const request = binding?.request, receipt = binding?.receipt;
      if (!request || !receipt || request.op !== 'composerFileHold' || request.action !== 'acquire'
        || request.generation !== generation || !equal(request.identity, identity) || !equal(request.target, target)
        || !uuid(request.requestId) || !equal(receipt.identity, identity) || receipt.requestId !== request.requestId
        || !uuid(receipt.holdId) || !uuid(receipt.fileIdentity) || !batchFile(request.file)
        || request.file.source !== 'attached' || request.file.draftKey !== guard.key
        || request.file.environmentId !== destination.environmentId || receipt.id !== request.file.id
        || receipt.sizeBytes !== request.file.sizeBytes) return refuse('unusable-hold');
      const previous = held.get(request.file.id);
      if (previous && !equal(metadata(previous), metadata(request.file))) return refuse('unusable-hold');
      held.set(request.file.id, request.file);
    }
    for (const context of decoded.context?.records ?? []) {
      if (context.kind !== 'file') continue;
      const live = attachments.find(row => row.id === context.attachmentId);
      const file = live?.type === 'file' ? live.file : held.get(str(context.attachmentId));
      if (!file || live && live.type !== 'file' || file.contextId !== context.contextId
        || ['name', 'mimeType', 'sizeBytes'].some(field => obj(file)[field] !== context[field])) return refuse('invalid-context');
    }
    history = detachedHistory(record);
    const nextContext = history(text, decoded.context);
    const previousIds = new Set((decoded.context?.records ?? []).filter(row => row.kind === 'file').map(row => str(row.attachmentId)));
    const retainedIds = new Set((nextContext?.records ?? []).filter(row => row.kind === 'file').map(row => str(row.attachmentId)));
    removed = attachments.filter(row => row.type === 'file' && previousIds.has(row.id) && !retainedIds.has(row.id));
    for (const row of removed) {
      if (row.type !== 'file' || !held.has(row.id) || !equal(metadata(held.get(row.id)!), metadata(row.file))) return refuse('unusable-hold');
    }
    nextAttachments = attachments.filter(row => !removed.includes(row));
    for (const context of nextContext?.records ?? []) {
      if (context.kind !== 'file') continue;
      let attachment = nextAttachments.find(row => row.id === context.attachmentId);
      if (!attachment) {
        const file = held.get(str(context.attachmentId));
        if (!file) return refuse('unusable-hold');
        attachment = { type: 'file', id: file.id, file: { ...clone(file), attachmentId: '', status: 'staged' } };
        nextAttachments.push(attachment);
      }
      const expected = batchAttachmentRecord(attachment);
      if (attachment.type !== 'file' || expected.contextId !== context.contextId
        || ['name', 'mimeType', 'sizeBytes'].some(field => expected[field] !== context[field])) return refuse('invalid-context');
    }
    const contextIds = nextAttachments.map(row => str(batchAttachmentRecord(row).contextId));
    if (new Set(contextIds).size !== contextIds.length) return refuse('invalid-inventory');
    if (nextContext === undefined) delete nextRecord.context; else nextRecord.context = clone(nextContext);
  }
  nextRecord.revision++;
  const files = nextAttachments.flatMap(row => row.type === 'file' ? [row.file] : []);
  const attachmentIds = nextAttachments.map(row => row.id), changedInventory = removed.length > 0
    || files.length !== inventory.files.filter(file => file.draftKey === guard.key).length;
  const after: MobileNewTaskAttachmentProjection = clone({ record: nextRecord, text,
    images: inventory.images[guard.key] ?? [], files, attachmentIds,
    attachmentOrder: changedInventory ? attachmentIds : inventory.orders[guard.key] ?? null });
  if (changedInventory) {
    setDraftFiles(client.local, [...inventory.files.filter(file => file.draftKey !== guard.key), ...clone(files)]);
    mobileDraftAttachmentRecord(client, guard.key, attachmentIds);
    store.fileReleases = [...new Set([...inventory.fileReleases, ...removed.map(row => row.id)])];
  }
  client.local.drafts[guard.key] = text;
  if (Object.hasOwn(nextRecord, 'context')) record.context = nextRecord.context; else delete record.context;
  record.revision = nextRecord.revision;
  if (history) histories.set(record, history);
  client.revision++;
  return { kind: 'committed', revision: record.revision, after, removed: clone(removed) };
}
