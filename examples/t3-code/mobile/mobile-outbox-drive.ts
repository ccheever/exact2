// Source365aa87982 use-thread-outbox-drain; Contract owns the clock and each invocation.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { obj } from './shared/domain';
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
  client.connection, client.configLive, client.config, client.scopes, client.shellLive, client.shell.sequence]);
const orderedRows = (client: T3Client) => [...mobileOutboxSnapshot(client).rows].sort((a, b) => a.record.createdAt.localeCompare(b.record.createdAt));
const sameThread = (a: MobileOutboxRecord, b: MobileOutboxRecord) => a.origin === b.origin && a.environmentId === b.environmentId && a.threadId === b.threadId;
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
    const sameEnvironment = record.environmentId === client.environmentId;
    const threadKey = JSON.stringify([record.origin, record.environmentId, record.threadId]);
    const first = !threads.has(threadKey); threads.add(threadKey);
    const wait = Math.max(1, current.retryAt - (Number.isFinite(now) ? now : 0));
    const waits = current.result?.status === 'waiting' && current.waitingFor === stamp;
    const editing = editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === owner);
    if (first && (!next || wait < delay) && !drive.busy && queue.complete && editors.ready && !editing && sameEnvironment && !row.held && row.status === 'confirmed'
      && automatic(current.result) && !waits) {
      next = JSON.stringify({ owner: ownerOf(record), signature: current.signature, sequence: current.sequence });
      delay = wait;
    }
    return { owner, environmentId: record.environmentId, threadId: record.threadId, messageId: record.messageId,
      title: record.text.trim().split('\n')[0]?.slice(0, 100) || 'Pending task', text: record.text,
      status: drive.busy === owner ? 'sending' : editing ? 'editing' : current.result?.status ?? (row.held ? 'editing' : 'queued'),
      reason: !editors.ready ? 'Read saved pending edits before sending.' : editing ? 'Saved edits must be resolved before this task sends.' : current.result?.reason ?? '', held: row.held || editing,
      canRetry: (first || finalAcknowledged(current, record)) && editors.ready && !editing && !drive.busy && !row.held && !!current.result && current.result.status !== 'delivered' };
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
  if (!row || row.record.environmentId !== client.environmentId || row.held || row.status !== 'confirmed') return { revision: client.revision, message: 'The pending task is no longer ready.' };
  const editors = mobilePendingTaskEditorsSnapshot(client);
  if (!editors.ready || editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === identity(row.record)))
    return { revision: client.revision, message: 'Resolve saved pending edits before sending this task.' };
  const current = attempt(drive, row), id = identity(row.record);
  const cleanupOnly = predecessor(client, row.record);
  if (cleanupOnly && !finalAcknowledged(current, row.record))
    return { revision: client.revision, message: 'Resolve the earlier pending message in this thread before sending this task.' };
  if (!manual && (parsed.signature !== current.signature || parsed.sequence !== current.sequence
    || !automatic(current.result) || current.retryAt > now)) return { revision: client.revision, message: '' };
  drive.busy = id; client.revision++;
  let admissionRefused = false;
  // Inventory and editor ownership may change across the foreground pass's
  // awaits. Local ACK cleanup is harmless to ordering; new wire work is not.
  const orderedNative: Native = { available: native.available, watch: topic => native.watch(topic), later(request) {
    if (!localRecovery(request)) {
      const editors = mobilePendingTaskEditorsSnapshot(client);
      if (!editors.ready || editors.markers.some(marker => mobilePendingTaskEditorKey(marker.owner) === id)) {
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
    current.waitingFor = admissionRefused ? '' : connection(client);
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

/** Bridge the durable ACK/removal to the shell event without a blank thread.
 * These are presentation copies only; they never authorize another dispatch. */
export function mobileOutboxDriveCompleted(client: T3Client): MobileOutboxRecord[] {
  return [...state(client).attempts.values()].flatMap(attempt => {
    const result = attempt.result, record = result?.status === 'delivered' ? result.delivery?.operation?.record : undefined;
    if (!record?.creation || attempt.bridgeReconciled) return [];
    if (record.environmentId === client.environmentId && client.shell.threads.some(thread => thread.id === record.threadId)) {
      attempt.bridgeReconciled = true; return [];
    }
    return [JSON.parse(JSON.stringify(record))];
  });
}
