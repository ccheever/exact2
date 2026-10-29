import { mobileDraftRecoveryHydrated, type MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskFlowCurrent, mobileNewTaskFlowOwns, mobileNewTaskRoute } from './new-task-flow';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { mobileNewTaskContextWrite, type MobileNewTaskContextGuard } from './mobile-new-task-context';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerEditorAccept, mobileComposerEditorAdmit, mobileComposerEditorControlled,
  mobileComposerEditorDecodeEvent, type ComposerEditorEvent, type ComposerEditorState,
  type ComposerMountedIdentity } from './composer-editor-state';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface NewTaskEditorRoute { owner: string; visit: string; location: string; editorId: string }
export type NewTaskEditorView =
  | { kind: 'held'; node: null; editable: false; message: string }
  | { kind: 'mounting' | 'editing'; node: { key: string; configuration: string }; editable: boolean; message: string };
export type NewTaskEditorIntent = { kind: 'observe'; payload: unknown } | { kind: 'retry' };
export interface NewTaskEditorResult { accepted: boolean; message: string }
interface DraftIdentity { key: string; createdAt: string; origin: string; environmentId: string; projectId: string }
interface Receipt {
  id: string; identity: ComposerMountedIdentity; event: ComposerEditorEvent;
  kind: 'retired' | 'frozen'; retired: boolean; fingerprint: string;
}
interface Publication { receipt: Receipt; revision: number }
type Progress =
  | { kind: 'unpublished' }
  | { kind: 'published'; publication: Publication }
  | { kind: 'durable'; publication: Publication; ackRead: number | null }
  | { kind: 'acknowledged'; receipt: Receipt };
interface Entry {
  draft: DraftIdentity; route: NewTaskEditorRoute; state: ComposerEditorState; revision: number;
  custody: 'native' | 'recovery' | 'frozen'; progress: Progress; dirty: boolean;
  attempt: number; problem: string;
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
  if (entry.custody !== 'native' || entry.progress.kind !== 'unpublished' || entry.problem) return false;
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
  if (owner.problem) return held(owner.problem);
  let entry = activeEntry(client, route);
  if (!entry) {
    owner.entries = owner.entries.filter(previous => previous.state.mountId && previous.progress.kind !== 'acknowledged');
    if (owner.entries.some(previous => previous.progress.kind !== 'acknowledged' && sameStable(previous.draft, capture.draft)))
      return held('Save the previous editor source before opening this draft.');
    if (owner.entries.length >= CAPACITY) return held('Save the previous editor sources before opening another editor.');
    if (identitySerial >= MAX_COUNT) return held('Restart the app before opening another editor.');
    const text = client.local.drafts[capture.draft.key] ?? '', renderEpoch = `new-task-editor:${++identitySerial}`;
    entry = { draft: capture.draft, route: { ...route }, revision: capture.revision, custody: 'native',
      state: mobileComposerEditorAdmit(null, { owner: capture.owner, editorId: route.editorId, routeVisit: route.visit, renderEpoch },
        { value: text, selection: { start: text.length, end: text.length } }),
      progress: { kind: 'unpublished' }, dirty: false, attempt: 0, problem: '' };
    owner.entries.push(entry);
  }
  if (latch) observe(client, entry, latch);
  if (entry.problem || entry.custody !== 'native' || entry.progress.kind !== 'unpublished')
    return held(entry.problem || 'Save the previous editor source before editing.');
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision) return held('The saved draft changed. Keep the previous editor source.');
  const editable = !!entry.state.mountId, control = mobileComposerEditorControlled(entry.state);
  return { kind: editable ? 'editing' : 'mounting', editable, message: '', node: {
    key: JSON.stringify([route.editorId, entry.state.identity.renderEpoch]),
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
  return { id: input.receiptId, identity: mount, event, kind, retired: input.retired, fingerprint };
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
  return entry.progress.kind === 'unpublished' ? null : entry.progress.kind === 'acknowledged'
    ? entry.progress.receipt : entry.progress.publication.receipt;
}
function publish(client: MobileDraftClient, entry: Entry, receipt: Receipt): boolean {
  const previous = receiptOf(entry);
  if (previous) {
    if (previous.id !== receipt.id || previous.fingerprint !== receipt.fingerprint || previous.retired && !receipt.retired) {
      entry.problem = 'The retained editor source changed unexpectedly.'; return false;
    }
    return entry.progress.kind !== 'acknowledged';
  }
  if (entry.custody === 'frozen' || receipt.kind === 'frozen' || !receipt.retired) {
    entry.custody = 'frozen'; entry.problem = 'The editor source belongs to an unfinished operation.'; return false;
  }
  const draft = registry(client, entry);
  if (!draft || draft.revision !== entry.revision || receipt.event.eventCount <= entry.state.eventCount) {
    entry.problem = 'The draft changed before its final editor source was saved.'; return false;
  }
  const guard = { key: entry.draft.key, createdAt: entry.draft.createdAt, revision: entry.revision };
  if (!mobileNewTaskContextWrite(client, guard, receipt.event.value)) return false;
  entry.revision++; entry.dirty = false; entry.custody = 'recovery';
  entry.progress = { kind: 'published', publication: { receipt, revision: entry.revision } };
  return true;
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
  if (response.value.acknowledged) entry.progress = { kind: 'acknowledged', receipt: publication.receipt };
  else await wake(owner, native, client.generation);
}
async function wake(owner: Ledger, native: Native, generation: number): Promise<void> {
  if (owner.wakeUsed) return;
  owner.wakeUsed = true;
  try { await bridgeReply(native, { op: 'r10Wake', topic: 't3.notify', generation }); }
  catch (error) { if (letGo(error)) throw error; }
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
        entry.attempt++; entry.progress = { kind: 'acknowledged', receipt: progress.publication.receipt };
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
