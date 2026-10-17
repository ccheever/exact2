// Source365aa87982 use-thread-outbox-drain; Contract owns the clock and each invocation.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { mobileOutboxBackgroundSaved } from './mobile-outbox-connection';
import { fleet, environmentKey } from './shared/settings-b-fleet';
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { mobileOutboxRead, mobileOutboxSnapshot, type MobileOutboxRow } from './mobile-outbox';
import { mobileOutboxDeliverOne, type MobileOutboxForegroundResult } from './mobile-outbox-foreground';
import { mobileOutboxRetryDelay, type MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobilePendingTaskEditorsSnapshot, mobilePendingTaskEditorKey } from './mobile-pending-task-state';

interface Attempt {
  signature: string; sequence: number; tries: number; retryAt: number; waitingFor: string;
  result?: MobileOutboxForegroundResult;
  bridgeReconciled?: boolean;
}
interface Drive { busy: string; attempts: Map<string, Attempt> }
const drives = new WeakMap<T3Client, Drive>();
function state(client: T3Client) {
  let drive = drives.get(client);
  if (!drive) { drive = { busy: '', attempts: new Map() }; drives.set(client, drive); }
  return drive;
}
const ownerOf = (record: MobileOutboxRecord): MobileOutboxWireOwner => ({ origin: record.origin,
  environmentId: record.environmentId, threadId: record.threadId, messageId: record.messageId, commandId: record.commandId });
const identity = (record: MobileOutboxRecord) => JSON.stringify(ownerOf(record));
const connection = (client: T3Client) => JSON.stringify([client.generation, client.origin, client.environmentId,
  client.connection, client.configLive, client.config, client.scopes, client.shellLive, client.shell.sequence, fleet.revision, fleet.saved.map(saved => [saved.origin, saved.environmentId, saved.enabled]),
  [...fleet.entries.values()].map(entry => [entry.key, entry.generation, entry.phase, entry.synchronized, entry.config, entry.scopes, entry.shell.sequence])]);
const orderedRows = (client: T3Client) => [...mobileOutboxSnapshot(client).rows].sort((a, b) => a.record.createdAt.localeCompare(b.record.createdAt));
const sameThread = (a: MobileOutboxRecord, b: MobileOutboxRecord) => a.origin === b.origin && a.environmentId === b.environmentId && a.threadId === b.threadId;
/** The native gate is authoritative; avoid scheduling known unfinished draft retirement. */
function retiringDraft(client:T3Client,record:MobileOutboxRecord):boolean {
  return mobileOutboxSnapshot(client).threadTransfers.some(claim=>claim.origin===record.origin
    &&claim.environmentId===record.environmentId&&claim.threadId===record.threadId&&claim.messageId===record.messageId
    &&claim.commandId===record.commandId&&claim.state!=='completed');
}
function predecessor(client: T3Client, record: MobileOutboxRecord): boolean {
  for (const row of orderedRows(client)) {
    if (identity(row.record) === identity(record)) return false;
    if (sameThread(row.record, record)) return true;
  }
  return false;
}
function finalAcknowledged(current: Attempt, record: MobileOutboxRecord): boolean {
  const receipt = current.result?.delivery?.operation;
  return receipt?.state === 'acknowledged' && receipt.stage === 'start-turn' && receipt.operationId === record.commandId
    && identity(receipt.record) === identity(record);
}
function localRecovery(request: unknown): boolean {
  const value = obj(request), action = String(value.action ?? '');
  return value.op === 'mobileOutbox' && ['read', 'acknowledge'].includes(action)
    || value.op === 'mobileOutboxDelivery' && ['status', 'recover', 'complete'].includes(action)
    || value.op === 'mobileOutboxInline' && ['lookup', 'status', 'recover'].includes(action);
}
const signature = (row: MobileOutboxRow) => JSON.stringify([row.token, row.nativeRevision, row.status, row.held]);
const automatic = (result?: MobileOutboxForegroundResult) => !result || ['retry', 'waiting'].includes(result.status);
function attempt(drive: Drive, row: MobileOutboxRow): Attempt {
  const key = identity(row.record), stamp = signature(row);
  let value = drive.attempts.get(key);
  // A changed queue row merits another discovery pass, never a rebuilt uncertain command.
  if (!value || value.signature !== stamp && drive.busy !== key) {
    value = { signature: stamp, sequence: (value?.sequence ?? 0) + 1, tries: 0, retryAt: 0, waitingFor: '' };
    drive.attempts.set(key, value);
  }
  return value;
}
export interface MobileOutboxDriveSnapshot {
  initialized: boolean; complete: boolean; busy: boolean; count: number; next: string; delay: number;
  items: Array<{ owner: string; environmentId: string; threadId: string; messageId: string; title: string;
    text: string; status: string; reason: string; held: boolean; canRetry: boolean }>;
}
/** Plain render state only. Native journals own retries; this map owns transient backoff. */
export function mobileOutboxDriveSnapshot(client: T3Client, now: number): MobileOutboxDriveSnapshot {
  const queue = mobileOutboxSnapshot(client), drive = state(client), stamp = connection(client);
  const editors = mobilePendingTaskEditorsSnapshot(client);
  let next = '', delay = 0;
  const rows = orderedRows(client);
  const threads = new Set<string>();
  const items = rows.map(row => {
    const record = row.record, owner = identity(record), current = attempt(drive, row);
    const rediscover = current.result?.status === 'waiting' && current.waitingFor === '';
    const sameEnvironment = record.environmentId === client.environmentId || mobileOutboxBackgroundSaved(record) || rediscover;
    const threadKey = JSON.stringify([record.origin, record.environmentId, record.threadId]);
    const first = !threads.has(threadKey); threads.add(threadKey);
    const wait = Math.max(1, current.retryAt - (Number.isFinite(now) ? now : 0));
    const waits = current.result?.status === 'waiting' && current.waitingFor === stamp;
    const editing = editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === owner), retiring=retiringDraft(client,record);
    if (first && (!next || wait < delay) && !drive.busy && queue.complete && editors.ready && !editing && !retiring && sameEnvironment && !row.held && row.status === 'confirmed'
      && automatic(current.result) && !waits) {
      next = JSON.stringify({ owner: ownerOf(record), signature: current.signature, sequence: current.sequence });
      delay = wait;
    }
    return { owner, environmentId: record.environmentId, threadId: record.threadId, messageId: record.messageId,
      title: record.text.trim().split('\n')[0]?.slice(0, 100) || 'Pending task', text: record.text,
      status: drive.busy === owner ? 'sending' : retiring ? 'retirement-pending' : editing ? 'editing' : current.result?.status ?? (row.held ? 'editing' : 'queued'),
      reason: retiring ? 'Finish saving this message before delivery.' : !editors.ready ? 'Read saved pending edits before sending.' : editing ? 'Saved edits must be resolved before this task sends.' : current.result?.reason ?? '', held: row.held || editing,
      canRetry: (first || finalAcknowledged(current, record)) && editors.ready && !editing && !retiring && !drive.busy && !row.held && !!current.result && current.result.status !== 'delivered' };
  });
  return { initialized: queue.initialized, complete: queue.complete, busy: !!drive.busy, count: rows.length, next, delay, items };
}
export async function mobileOutboxDriveRead(client: T3Client, native: Native | null | undefined) {
  const complete = await mobileOutboxRead(client, native);
  return { revision: client.revision, message: complete ? '' : 'Pending tasks could not be fully loaded. Their saved content has been kept.' };
}
/** Called by a root-owned mutation with a current Native ticket. A route switch
 * does not change its record owner. No Promise or Native is retained between calls. */
export async function mobileOutboxDriveRun(client: T3Client, native: Native, key: string, now: number, manual = false) {
  const drive = state(client);
  if (drive.busy) return { revision: client.revision, message: '' };
  if (!Number.isFinite(now) || now <= 0) return { revision: client.revision, message: 'Wait for the app clock before sending.' };
  let parsed: { owner?: MobileOutboxWireOwner; signature?: string; sequence?: number };
  try { parsed = JSON.parse(key); } catch { return { revision: client.revision, message: 'The pending task changed.' }; }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return { revision: client.revision, message: 'The pending task changed.' };
  const owner = manual ? parsed as MobileOutboxWireOwner : parsed.owner;
  const row = mobileOutboxSnapshot(client).rows.find(item => identity(item.record) === JSON.stringify(owner));
  if (!row || row.held || row.status !== 'confirmed') return { revision: client.revision, message: 'The pending task is no longer ready.' };
  if(retiringDraft(client,row.record))return {revision:client.revision,message:'Finish saving this message before delivery.'};
  const editors = mobilePendingTaskEditorsSnapshot(client);
  if (!editors.ready || editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === identity(row.record)))
    return { revision: client.revision, message: 'Resolve saved pending edits before sending this task.' };
  const current = attempt(drive, row), id = identity(row.record);
  if (row.record.environmentId !== client.environmentId && !mobileOutboxBackgroundSaved(row.record)
    && !(current.result?.status === 'waiting' && current.waitingFor === ''))
    return { revision: client.revision, message: 'The saved environment is no longer available.' };
  const cleanupOnly = predecessor(client, row.record);
  if (cleanupOnly && !finalAcknowledged(current, row.record))
    return { revision: client.revision, message: 'Resolve the earlier pending message in this thread before sending this task.' };
  if (!manual && (parsed.signature !== current.signature || parsed.sequence !== current.sequence
    || !automatic(current.result) || current.retryAt > now)) return { revision: client.revision, message: '' };
  const admittedConnection = connection(client);
  drive.busy = id; client.revision++;
  let admissionRefused = false;
  // Inventory and editor ownership may change across the foreground pass's
  // awaits. Local ACK cleanup is harmless to ordering; new wire work is not.
  const orderedNative: Native = { available: native.available, watch: topic => native.watch(topic), later(request) {
    if (!localRecovery(request)) {
      const editors = mobilePendingTaskEditorsSnapshot(client);
      if (retiringDraft(client,row.record) || !editors.ready || editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === id)) {
        admissionRefused = true; throw new ClientError('Resolve saved pending edits before sending this task.', 'stale');
      }
      if (cleanupOnly || predecessor(client, row.record)) {
        admissionRefused = true; throw new ClientError('Resolve the earlier pending message in this thread before sending this task.', 'stale');
      }
    }
    return native.later(request);
  } };
  try {
    const receipt = current.result?.delivery?.operation;
    const cleanupRetry = manual && receipt?.state === 'acknowledged' && receipt.cleanup
      && ['failed', 'not-started'].includes(receipt.cleanup.phase) ? receipt.revision : undefined;
    const result = await mobileOutboxDeliverOne(client, orderedNative, owner!, { recover: true,
      ...(cleanupRetry === undefined ? {} : { retryCleanupRevision: cleanupRetry }) });
    current.result = result; current.sequence++; current.tries++;
    current.retryAt = now + mobileOutboxRetryDelay(current.tries);
    // A wire reply may have become a terminal native receipt while its transport
    // disappeared. Discover it once before parking on unchanged connection facts.
    current.waitingFor = admissionRefused || admittedConnection !== connection(client) ? '' : connection(client);
    // The pass may have adopted uploaded descriptors. Keep its outcome attached
    // to that new row, so an unresolved final command cannot become an auto retry.
    const after = mobileOutboxSnapshot(client).rows.find(item => identity(item.record) === id);
    if (after) current.signature = signature(after);
    return { revision: client.revision, message: manual ? result.reason : '' };
  } catch (error) {
    if (letGo(error)) throw error;
    current.result = { status: 'recovery-required', reason: error instanceof Error ? error.message : 'Resolve this pending task before retrying.' };
    current.sequence++;
    return { revision: client.revision, message: manual ? current.result.reason : '' };
  } finally { drive.busy = ''; client.revision++; }
}

/** The source's global creation outcome ends at the first shell turn. A mounted
 * route separately retains its prompt until the matching detail takes over. */
export function mobileOutboxCreationShell(client: T3Client, record: MobileOutboxRecord): Obj | null {
  if (client.environmentId === record.environmentId)
    return client.shell.threads.find(thread => thread.id === record.threadId) ?? null;
  const entry = fleet.entries.get(environmentKey(record.origin, record.environmentId));
  return entry && entry.phase === 'connected' && entry.synchronized === entry.generation
    ? entry.shell.threads.find(thread => thread.id === record.threadId) ?? null : null;
}
function shellStarted(thread: Obj | null): boolean {
  return !!thread && (thread.latestRun != null || ['failed', 'cancelled', 'interrupted'].includes(String(obj(thread.runtime).status)));
}
/** Bridge the durable ACK/removal to the shell event without a blank thread.
 * These are presentation copies only; they never authorize another dispatch. */
export function mobileOutboxDriveCompleted(client: T3Client): MobileOutboxRecord[] {
  return [...state(client).attempts.values()].flatMap(attempt => {
    const result = attempt.result, record = result?.status === 'delivered' ? result.delivery?.operation?.record : undefined;
    if (!record?.creation || attempt.bridgeReconciled) return [];
    if (shellStarted(mobileOutboxCreationShell(client, record))) {
      attempt.bridgeReconciled = true; return [];
    }
    return [JSON.parse(JSON.stringify(record))];
  });
}

/** Ordinary ACK-to-echo presentation uses the captured message identity. Only the
 * matching thread's authoritative feed retires it; changing routes does not. */
export function mobileOutboxThreadCompleted(client:T3Client,origin:string,echoes:ReadonlySet<string>):MobileOutboxRecord[] {
  return [...state(client).attempts.values()].flatMap(attempt=>{
    const result=attempt.result,record=result?.status==='delivered'?result.delivery?.operation?.record:undefined;
    if(!record||record.creation||attempt.bridgeReconciled||record.origin!==origin
      ||record.environmentId!==client.environmentId||record.threadId!==client.threadId)return [];
    if(echoes.has(record.messageId)){attempt.bridgeReconciled=true;return []}
    return [JSON.parse(JSON.stringify(record)) as MobileOutboxRecord];
  });
}

/** Local terminal discovery for root-owned draft publication. The original
 * receipt is revalidated by recovery; this transient view authorizes no removal. */
export function mobileOutboxDriveRecoverable(client: T3Client): MobileOutboxRecord[] {
  const rows = orderedRows(client);
  return [...state(client).attempts.values()].flatMap(attempt => {
    const receipt = attempt.result?.delivery?.operation;
    if (!receipt || receipt.stage !== 'start-turn' || !(receipt.state === 'rejected'
      || receipt.state === 'acknowledged' && receipt.record.creation && receipt.cleanup?.phase === 'edited')) return [];
    const row = rows.find(row => identity(row.record) === identity(receipt.record));
    return row ? [JSON.parse(JSON.stringify(row.record)) as MobileOutboxRecord] : [];
  });
}
