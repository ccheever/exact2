import { mobileDraftRecoveryHydrated, type MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskFlowCurrent, mobileNewTaskFlowOwns, mobileNewTaskRoute } from './new-task-flow';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { mobileNewTaskContextWrite, type MobileNewTaskContextGuard } from './mobile-new-task-context';
import { mobileComposerTarget, type MobileComposerTarget } from './composer-target';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { fleet } from './shared/settings-b-fleet';
import { mobileComposerEditorAccept, mobileComposerEditorAdmit, mobileComposerEditorControlled,
  mobileComposerEditorDecodeEvent, type ComposerEditorEvent, type ComposerEditorState,
  type ComposerMountedIdentity } from './composer-editor-state';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface NewTaskEditorRoute { owner: string; visit: string; location: string; editorId: string }
export type NewTaskEditorView =
  | { kind: 'held'; node: null; editable: false; message: string }
  | { kind: 'mounting' | 'editing' | 'frozen'; node: { key: string; configuration: string }; editable: boolean; message: string };
export type NewTaskEditorIntent = { kind: 'observe'; payload: unknown } | { kind: 'retry' }
  | { kind: 'pick'; source: 'photos' | 'files' };
export interface NewTaskEditorResult { accepted: boolean; message: string }
interface DraftIdentity { key: string; createdAt: string; origin: string; environmentId: string; projectId: string }
interface ReceiptSource {
  id: string; identity: ComposerMountedIdentity; event: ComposerEditorEvent;
  retired: boolean; fingerprint: string;
}
interface OperationIdentity { id: string; revision: number }
type FrozenReceipt = ReceiptSource & { kind: 'frozen'; operation: OperationIdentity };
type Receipt = FrozenReceipt | ReceiptSource & { kind: 'retired' };
interface PickCapture {
  source: 'photos' | 'files'; route: NewTaskEditorRoute; draft: DraftIdentity; identity: ComposerMountedIdentity;
  target: MobileComposerTarget; threadEpoch: number; catalog: string; revision: number;
  eventCount: number; value: string; selection: { start: number; end: number };
}
type PickWork =
  | { phase: 'allocating'; capture: PickCapture; revision: number; attempt: number }
  | { phase: 'freezing'; capture: PickCapture; operation: OperationIdentity }
  | { phase: 'frozen'; capture: PickCapture; operation: OperationIdentity; receipt: FrozenReceipt };
interface Publication { receipt: Receipt; revision: number; slotReleased: boolean }
type Progress =
  | { kind: 'unpublished' }
  | { kind: 'published'; publication: Publication }
  | { kind: 'durable'; publication: Publication; ackRead: number | null }
  | { kind: 'saved'; publication: Publication }
  | { kind: 'acknowledged'; receipt: Receipt };
interface Entry {
  draft: DraftIdentity; route: NewTaskEditorRoute; state: ComposerEditorState; revision: number;
  custody: 'native' | 'recovery' | 'frozen'; progress: Progress; dirty: boolean;
  attempt: number; problem: string; operation: PickWork | null; operationRevision: number;
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
function observe(client: MobileDraftClient, entry: Entry, payload: unknown): boolean {
  if (entry.custody !== 'native' || entry.progress.kind !== 'unpublished' || entry.problem || entry.operation) return false;
  const event = mobileComposerEditorDecodeEvent(payload), draft = registry(client, entry);
  if (!event || event.pendingCommand || event.kind === 'commandApplied' || event.kind === 'commandRejected'
    || event.eventCount < 1 || !draft || draft.revision !== entry.revision) return false;
  const accepted = mobileComposerEditorAccept(entry.state, event);
  if (!accepted.accepted) return false;
  if (!entry.state.mountId && ledger(client).entries.some(previous => previous !== entry && previous.state.mountId === event.mountId)) return false;
  if (accepted.duplicate) return true;
  if (event.value !== entry.state.value) {
    const guard: MobileNewTaskContextGuard = { key: entry.draft.key, createdAt: entry.draft.createdAt, revision: entry.revision };
    if (!mobileNewTaskContextWrite(client, guard, event.value)) return false;
    entry.revision++; entry.dirty = true;
  }
  entry.state = accepted.state;
  return true;
}

export function mobileNewTaskEditorView(client: MobileDraftClient, route: NewTaskEditorRoute, latch: unknown = ''): NewTaskEditorView {
  const owner = ledger(client), capture = admission(client, route);
  if (!capture) return held('Wait for the current new task draft to be ready.');
  if (!owner.acceptedRead) return held('Wait for the previous editor sources to be checked.');
  let entry = activeEntry(client, route);
  if (entry?.operation?.phase === 'frozen' && !entry.operation.receipt.retired)
    return presentation(entry, false, 'frozen', entry.problem || owner.problem || 'The editor source is held for the attachment operation.');
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
    const text = client.local.drafts[capture.draft.key] ?? '', renderEpoch = `new-task-editor:${++identitySerial}`;
    entry = { draft: capture.draft, route: { ...route }, revision: capture.revision, custody: 'native',
      state: mobileComposerEditorAdmit(null, { owner: capture.owner, editorId: route.editorId, routeVisit: route.visit, renderEpoch },
        { value: text, selection: { start: text.length, end: text.length } }),
      progress: { kind: 'unpublished' }, dirty: false, attempt: 0, problem: '', operation: null, operationRevision: 0 };
    owner.entries.push(entry);
  }
  if (latch) observe(client, entry, latch);
  if (entry.problem || entry.custody !== 'native' || entry.progress.kind !== 'unpublished')
    return held(entry.problem || 'Save the previous editor source before editing.');
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision) return held('The saved draft changed. Keep the previous editor source.');
  const editable = !!entry.state.mountId;
  return presentation(entry, editable, editable ? 'editing' : 'mounting', '');
}
function presentation(entry: Entry, editable: boolean, kind: 'mounting' | 'editing' | 'frozen', message: string): NewTaskEditorView {
  const control = mobileComposerEditorControlled(entry.state);
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
function decodeBatch(input: unknown): Receipt[] | null {
  if (!record(input) || !Array.isArray(input.receipts) || input.receipts.length > CAPACITY) return null;
  const receipts: Receipt[] = [];
  for (const value of input.receipts) {
    const receipt = decodeReceipt(value);
    if (!receipt) return null;
    const duplicate = receipts.find(previous => previous.id === receipt.id || sameIdentity(previous.identity, receipt.identity));
    if (duplicate) {
      if (duplicate.id !== receipt.id || duplicate.fingerprint !== receipt.fingerprint) return null;
      duplicate.retired ||= receipt.retired;
    } else receipts.push(receipt);
  }
  return receipts;
}
function receiptOf(entry: Entry): Receipt | null {
  return entry.progress.kind === 'unpublished' ? entry.operation?.phase === 'frozen' ? entry.operation.receipt : null : entry.progress.kind === 'acknowledged'
    ? entry.progress.receipt : entry.progress.publication.receipt;
}
function publish(client: MobileDraftClient, entry: Entry, receipt: Receipt): boolean {
  const previous = receiptOf(entry);
  if (previous) {
    if (previous.id !== receipt.id || previous.fingerprint !== receipt.fingerprint) {
      entry.problem = 'The retained editor source changed unexpectedly.'; return false;
    }
    previous.retired ||= receipt.retired;
    if (entry.progress.kind === 'saved') releaseSlot(ledger(client), entry, entry.progress.publication);
    return entry.progress.kind === 'published' || entry.progress.kind === 'durable';
  }
  if (entry.problem) return false;
  if (receipt.kind === 'frozen') {
    const work = entry.operation;
    if (!work || work.phase === 'allocating' || work.operation.id !== receipt.operation.id
      || work.operation.revision !== receipt.operation.revision || !sameIdentity(work.capture.identity, receipt.identity)) {
      entry.custody = 'frozen'; entry.problem = 'The editor source belongs to an unfinished operation.'; return false;
    }
    entry.operation = { ...work, phase: 'frozen', receipt }; entry.custody = 'frozen';
  } else if (entry.custody === 'frozen' || !receipt.retired) {
    entry.custody = 'frozen'; entry.problem = 'The editor source belongs to an unfinished operation.'; return false;
  }
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision || receipt.event.eventCount <= entry.state.eventCount) {
    entry.problem = 'The draft changed before its final editor source was saved.'; return false;
  }
  const guard = { key: entry.draft.key, createdAt: entry.draft.createdAt, revision: entry.revision };
  if (!mobileNewTaskContextWrite(client, guard, receipt.event.value)) return false;
  entry.revision++; entry.dirty = false; entry.custody = receipt.kind === 'frozen' ? 'frozen' : 'recovery';
  const accepted = mobileComposerEditorAccept(entry.state, receipt.event);
  if (accepted.accepted) entry.state = accepted.state;
  entry.progress = { kind: 'published', publication: { receipt, revision: entry.revision, slotReleased: false } };
  return true;
}
function releaseSlot(owner: Ledger, entry: Entry, publication: Publication): void {
  if (!publication.receipt.retired || publication.slotReleased) return;
  publication.slotReleased = true;
  // A refused factory has no native handle. Only a new keyed seed can retry it.
  owner.entries = owner.entries.filter(candidate => candidate === entry || candidate.state.mountId
    || candidate.operation || candidate.progress.kind !== 'unpublished');
}
function acknowledge(owner: Ledger, entry: Entry, publication: Publication): void {
  entry.progress = publication.receipt.kind === 'frozen' && entry.operation?.phase === 'frozen'
    ? { kind: 'saved', publication } : { kind: 'acknowledged', receipt: publication.receipt };
  releaseSlot(owner, entry, publication);
}
function validCompletion(client: MobileDraftClient, owner: Ledger, entry: Entry, attempt: number, publication: Publication): boolean {
  const draft = registry(client, entry);
  return mobileDraftRecoveryHydrated(client) && owner.entries.includes(entry) && entry.attempt === attempt && !entry.problem && !!draft
    && draft.revision >= publication.revision && (entry.progress.kind === 'published' || entry.progress.kind === 'durable')
    && entry.progress.publication === publication;
}
async function settle(client: MobileDraftClient, owner: Ledger, entry: Entry, native: Native, storage: Files, serial: number) {
  const progress = entry.progress;
  if (progress.kind !== 'published' && progress.kind !== 'durable') return;
  const publication = progress.publication, attempt = ++entry.attempt;
  if (!validCompletion(client, owner, entry, attempt, publication)) return;
  if (progress.kind === 'published') {
    await client.persist(storage);
    if (!validCompletion(client, owner, entry, attempt, publication)) return;
    entry.progress = { kind: 'durable', publication, ackRead: null };
  }
  if (!validCompletion(client, owner, entry, attempt, publication) || entry.progress.kind !== 'durable') return;
  entry.progress.ackRead = serial;
  const generation = client.generation;
  const response = await bridgeReply(native, { op: 'composerEditorFinalizations', generation, action: 'acknowledge', receiptId: publication.receipt.id });
  if (!validCompletion(client, owner, entry, attempt, publication)) return;
  if (!response.ok || response.generation !== generation || !record(response.value) || typeof response.value.acknowledged !== 'boolean')
    throw new ClientError('The final editor source acknowledgment could not be confirmed.');
  if (response.value.acknowledged) acknowledge(owner, entry, publication);
  else await wake(owner, native, client.generation);
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
  const generation = work.capture.target.generation;
  const response = await bridgeReply(native, { op: 'composerEditorHandoff', generation, identity: work.capture.identity,
    action: 'freeze', operationId: work.operation.id, operationRevision: work.operation.revision, retireUndo: false }).catch(error => {
    if (letGo(error) || owner.entries.includes(entry) && entry.operation === work) throw error;
    return null;
  });
  if (!response || !owner.entries.includes(entry) || entry.operation !== work) return;
  if (!response.ok || response.generation !== generation || !record(response.value))
    throw new ClientError('The editor handoff is retained until its frozen source can be confirmed.');
  const receipt = decodeReceipt(response.value.receipt);
  if (!receipt || receipt.kind !== 'frozen' || !sameIdentity(receipt.identity, work.capture.identity)
    || receipt.operation.id !== work.operation.id || receipt.operation.revision !== work.operation.revision)
    throw new ClientError('The frozen editor source does not match the attachment operation.');
  if (publish(client, entry, receipt)) await settle(client, owner, entry, native, storage, owner.acceptedRead);
}
async function continuePick(client: MobileDraftClient, owner: Ledger, entry: Entry, native: Native, storage: Files): Promise<void> {
  const work = entry.operation;
  if (!work || work.phase === 'frozen' || entry.progress.kind !== 'unpublished' || entry.problem) return;
  if (!pickCurrent(client, entry, work.capture)) {
    if (work.phase === 'allocating') entry.operation = null;
    return;
  }
  if (work.phase === 'freezing') { await freeze(client, owner, entry, work, native, storage); return; }
  const attempt = ++work.attempt, ids = await client.ids(native, 1).catch(error => {
    if (letGo(error) || owner.entries.includes(entry) && entry.operation === work && work.attempt === attempt
      && pickCurrent(client, entry, work.capture)) throw error;
    return null;
  });
  if (!owner.entries.includes(entry) || entry.operation !== work || work.attempt !== attempt) return;
  if (!pickCurrent(client, entry, work.capture)) { entry.operation = null; return; }
  if (!ids) return;
  const id = ids[0];
  if (!uuid(id)) throw new ClientError('The attachment operation identifier is invalid.');
  const identified: Extract<PickWork, { phase: 'freezing' }> = {
    phase: 'freezing', capture: work.capture, operation: { id: id.toLowerCase(), revision: work.revision },
  };
  entry.operation = identified;
  await freeze(client, owner, entry, identified, native, storage);
}
async function recover(client: MobileDraftClient, native: Native, storage: Files): Promise<void> {
  const owner = ledger(client), serial = ++owner.serial, generation = client.generation;
  try {
    const response = await bridgeReply(native, { op: 'composerEditorFinalizations', generation, action: 'read' });
    if (serial < owner.acceptedRead || !mobileDraftRecoveryHydrated(client)) return;
    if (!response.ok || response.generation !== generation) throw new ClientError('The retained editor sources could not be read.');
    const receipts = decodeBatch(response.value);
    if (!receipts) throw new ClientError('The retained editor sources are invalid.');
    owner.acceptedRead = serial; owner.problem = '';
    for (const entry of owner.entries) {
      const progress = entry.progress;
      if (progress.kind === 'durable' && progress.ackRead !== null && serial > progress.ackRead
        && !receipts.some(receipt => receipt.id === progress.publication.receipt.id) && registry(client, entry)) {
        entry.attempt++; acknowledge(owner, entry, progress.publication);
      }
    }
    for (const receipt of receipts) {
      const entry = owner.entries.find(candidate => sameIdentity(mounted(candidate), receipt.identity));
      if (!entry) { owner.problem = 'An editor source has no captured draft owner.'; continue; }
      if (!publish(client, entry, receipt)) continue;
      try { await settle(client, owner, entry, native, storage, serial); }
      catch (error) {
        if (letGo(error)) throw error;
        if (serial === owner.acceptedRead) { owner.problem = 'The final editor source is retained until it can be saved.'; await wake(owner, native, client.generation); }
      }
      if (serial < owner.acceptedRead) return;
    }
    for (const entry of owner.entries) {
      try { await continuePick(client, owner, entry, native, storage); }
      catch (error) {
        if (letGo(error)) throw error;
        if (serial === owner.acceptedRead) {
          owner.problem = 'The editor handoff is retained until it can be confirmed.';
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
  const entry = activeEntry(client, route);
  if (intent.kind === 'pick') {
    if (!entry || !native?.available || entry.operation || entry.problem || ledger(client).problem
      || entry.progress.kind !== 'unpublished' || entry.custody !== 'native' || !entry.state.mountId
      || entry.state.composing || entry.state.pendingCommand || entry.operationRevision >= MAX_COUNT
      || registry(client, entry)?.revision !== entry.revision)
      return { accepted: false, message: 'The editor is not ready for an attachment operation.' };
    const capture: PickCapture = { source: intent.source, route: { ...route }, draft: { ...entry.draft }, identity: mounted(entry),
      target: mobileComposerTarget(client), threadEpoch: client.threadEpoch,
      catalog: mobileCacheCatalogIdentity(fleet.saved, entry.draft.environmentId), revision: entry.revision,
      eventCount: entry.state.eventCount, value: entry.state.value, selection: { ...entry.state.selection } };
    if (!pickCurrent(client, entry, capture)) return { accepted: false, message: 'The attachment editor changed.' };
    const owner = ledger(client);
    entry.attempt++; entry.operation = { phase: 'allocating', capture, revision: ++entry.operationRevision, attempt: 0 };
    try { await continuePick(client, owner, entry, native, storage); }
    catch (error) {
      if (letGo(error)) throw error;
      owner.problem = error instanceof Error ? error.message : 'The editor handoff is retained.';
      await wake(owner, native, client.generation);
    }
    return { accepted: true, message: entry.problem || owner.problem || 'The editor source is held for the attachment operation.' };
  }
  if (!entry || !observe(client, entry, intent.payload)) return { accepted: false, message: 'The editor observation is no longer current.' };
  if (entry.dirty) {
    const attempt = ++entry.attempt, revision = entry.revision, eventCount = entry.state.eventCount;
    try {
      await client.persist(storage);
      if (entry.attempt === attempt && entry.custody === 'native' && entry.revision === revision && entry.state.eventCount === eventCount
        && registry(client, entry)?.revision === revision) entry.dirty = false;
    } catch (error) {
      if (letGo(error)) throw error;
      return { accepted: true, message: 'The draft could not be saved. Its editor source is retained.' };
    }
  }
  return { accepted: true, message: '' };
}
