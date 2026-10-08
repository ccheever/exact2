// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Invocation-only bridge. The native journal owns attempts, outcomes and recovery.
import type { T3Client } from './shared/client';
import type { Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileOutboxCapture, mobileOutboxSnapshot, mobileOutboxCompleteDelivery, mobileOutboxDecodeOutcome,
  type MobileOutboxCapture } from './mobile-outbox';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import type { MobileOutboxWireRequest } from './mobile-outbox-wire';
import { mobileOutboxMaterializeInline } from './mobile-outbox-inline';
import { mobileOutboxInlineDecode, type MobileOutboxInlineReceipt } from './mobile-outbox-inline-delivery';

export interface MobileOutboxDeliveryReceipt {
  kind: 'outbox'; operationId: string; revision: number; origin: string; environmentId: string;
  messageId: string; threadId: string; rowToken: string; rowRevision: number; record: MobileOutboxRecord;
  stage: MobileOutboxWireRequest['stage']; method: MobileOutboxWireRequest['method']; payload: Obj;
  attachmentIDs: string[]; state: 'reserved' | 'retired' | 'issued' | 'uncertain' | 'acknowledged' | 'rejected';
  retiredRevision?: number; attemptRevision?: number; attemptPreviousState?: 'reserved' | 'uncertain' | 'rejected';
  result?: unknown; error?: Obj;
  cleanup?: MobileOutboxDeliveryCleanup;
  inlineSource?: { operationId: string; ackRevision: number; payloadDigest: string };
}
export interface MobileOutboxDeliveryCleanup {
  ackRevision: number; intentRevision: number; phase: 'pending' | 'settings' | 'removed' | 'edited' | 'failed' | 'uncertain' | 'not-started';
  mutationId?: string; ownerEpoch?: string; outcome: Obj | null;
}
export interface MobileOutboxDeliveryCompletion extends MobileOutboxDeliveryStatus {
  operation: MobileOutboxDeliveryReceipt; cleanup: Exclude<MobileOutboxDeliveryCleanup['phase'], 'pending'>; outcome: Obj | null;
}
export interface MobileOutboxDeliveryStatus { operation: MobileOutboxDeliveryReceipt | null; durable: boolean }
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const object = (value: unknown): value is Obj => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = (value: unknown, minimum = 0): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum;
const text = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.length <= 4096 && value.trim() === value;
const invalid = (): never => { throw new ClientError('The native delivery receipt is invalid. Read its saved status.', 'protocol', true); };
const keys = ['kind', 'operationId', 'revision', 'origin', 'environmentId', 'messageId', 'threadId', 'rowToken', 'rowRevision',
  'record', 'stage', 'method', 'payload', 'attachmentIDs', 'state', 'retiredRevision', 'attemptRevision', 'attemptPreviousState', 'result', 'error', 'cleanup', 'inlineSource'];
function json(value: unknown, ancestors = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (!object(value) && !Array.isArray(value) || ancestors.has(value)) return false;
  ancestors.add(value);
  const valid = Object.values(value).every(item => json(item, ancestors));
  ancestors.delete(value); return valid;
}

/** Validate reply ownership and lifecycle. Native remains the command-schema authority;
 * this object is never sent as a replacement payload by the retry API. */
export function mobileOutboxDeliveryDecode(raw: unknown, operationId: string): MobileOutboxDeliveryReceipt {
  if (!object(raw) || !json(raw) || Object.keys(raw).some(key => !keys.includes(key)) || raw.kind !== 'outbox'
    || !text(operationId) || raw.operationId !== operationId || !integer(raw.revision, 1) || !integer(raw.rowRevision, 1)
    || !text(raw.rowToken) || !object(raw.payload) || !Array.isArray(raw.attachmentIDs)
    || !['reserved', 'retired', 'issued', 'uncertain', 'acknowledged', 'rejected'].includes(String(raw.state))) return invalid();
  const decoded = mobileOutboxDecode(raw.record);
  if (!decoded.ok) return invalid();
  const record = decoded.record, payload = raw.payload;
  if (['origin', 'environmentId', 'messageId', 'threadId'].some(key => raw[key] !== record[key as keyof MobileOutboxRecord])
    || canonical(raw.attachmentIDs) !== canonical(record.attachments.map(file => file.id))
    || payload.commandId !== operationId || payload.threadId !== record.threadId) return invalid();
  if (raw.stage === 'settings-sync') {
    const field = payload.type === 'thread.runtime-mode.set' ? 'runtime-mode'
      : payload.type === 'thread.interaction-mode.set' ? 'interaction-mode' : null;
    if (!field || record.creation || raw.method !== 'orchestration.dispatchCommand' || operationId !== `${record.commandId}:${field}`) return invalid();
  } else if (raw.stage === 'start-turn') {
    if (operationId !== record.commandId || (record.creation ? raw.method !== 'orchestration.launchThread'
      || !object(payload.initialMessage) || payload.initialMessage.messageId !== record.messageId
      : raw.method !== 'orchestration.dispatchCommand' || payload.type !== 'message.dispatch' || payload.messageId !== record.messageId)) return invalid();
  } else return invalid();
  if ('inlineSource' in raw) {
    const source = raw.inlineSource;
    if (raw.stage !== 'start-turn' || !object(source) || Object.keys(source).length !== 3
      || Object.keys(source).some(key => !['operationId', 'ackRevision', 'payloadDigest'].includes(key))
      || typeof source.operationId !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(source.operationId)
      || source.operationId === operationId || !integer(source.ackRevision, 3)
      || typeof source.payloadDigest !== 'string' || !/^[0-9a-f]{64}$/.test(source.payloadDigest)) return invalid();
  }
  const prior = raw.retiredRevision ?? 0;
  if (!integer(prior) || 'retiredRevision' in raw && !integer(raw.retiredRevision, 1) || prior > Number.MAX_SAFE_INTEGER - 2) return invalid();
  if ('cleanup' in raw) {
    const cleanup = raw.cleanup;
    if (raw.state !== 'acknowledged' || !object(cleanup) || !integer(raw.attemptRevision, 1)
      || cleanup.ackRevision !== raw.attemptRevision + 1 || !integer(cleanup.intentRevision, Number(cleanup.ackRevision) + 1)
      || raw.revision < cleanup.intentRevision || !('outcome' in cleanup) || cleanup.outcome !== null && !object(cleanup.outcome)) return invalid();
    const cleanupKeys = ['ackRevision', 'intentRevision', 'phase', 'outcome'];
    if (raw.stage === 'settings-sync') {
      if (cleanup.phase !== 'settings' || cleanup.outcome !== null) return invalid();
    } else {
      cleanupKeys.push('mutationId', 'ownerEpoch');
      if (!text(cleanup.mutationId) || !text(cleanup.ownerEpoch) || !cleanup.mutationId.startsWith(`${cleanup.ownerEpoch}:`)
        || !['pending', 'removed', 'edited', 'failed', 'uncertain', 'not-started'].includes(String(cleanup.phase))) return invalid();
    }
    if (Object.keys(cleanup).length !== cleanupKeys.length || Object.keys(cleanup).some(key => !cleanupKeys.includes(key))) return invalid();
    if (cleanup.outcome !== null) {
      try { mobileOutboxDecodeOutcome(cleanup.outcome, record.messageId, String(cleanup.mutationId)); } catch { return invalid(); }
    }
    const result = cleanup.outcome;
    if (['pending', 'not-started', 'settings'].includes(String(cleanup.phase)) ? result !== null
      : cleanup.phase === 'removed' ? !object(result) || result.status !== 'committed' || result.record !== null || canonical(result.removed) !== canonical(record)
      : cleanup.phase === 'edited' ? !object(result) || result.status !== 'stale'
      : cleanup.phase === 'failed' ? !object(result) || result.status !== 'failed'
      : result !== null && (!object(result) || result.status !== 'uncertain')) return invalid();
  }
  if ('attemptRevision' in raw) {
    const previous = raw.attemptPreviousState, attempt = raw.attemptRevision;
    if (!integer(attempt, prior + 2) || !['reserved', 'uncertain', 'rejected'].includes(String(previous))
      || !raw.cleanup && raw.revision !== attempt + (raw.state === 'issued' ? 0 : raw.state === 'retired' ? 2 : 1)
      || ['reserved', 'retired'].includes(String(raw.state)) && previous !== 'reserved'
      || raw.state === 'rejected' && previous === 'uncertain' || previous === 'rejected' && raw.stage !== 'settings-sync') return invalid();
    if (raw.state === 'issued' ? 'result' in raw || 'error' in raw
      : raw.state === 'acknowledged' ? !('result' in raw) || 'error' in raw : 'result' in raw || !object(raw.error)) return invalid();
  } else if (!['reserved', 'retired'].includes(String(raw.state)) || 'attemptPreviousState' in raw || 'result' in raw || 'error' in raw
    || raw.revision !== prior + (raw.state === 'reserved' ? 1 : 2)) return invalid();
  return copy(raw) as unknown as MobileOutboxDeliveryReceipt;
}
const mutable = new Set(['revision', 'state', 'attemptRevision', 'attemptPreviousState', 'result', 'error', 'cleanup']);
function identity(receipt: MobileOutboxDeliveryReceipt): string {
  return canonical(Object.fromEntries(Object.entries(receipt).filter(([key]) => !mutable.has(key))));
}
function status(raw: unknown, id: string, expected?: MobileOutboxDeliveryReceipt): MobileOutboxDeliveryStatus {
  if (!object(raw) || typeof raw.durable !== 'boolean') return invalid();
  if (raw.operation === null) { if (raw.durable || expected) return invalid(); return { operation: null, durable: false }; }
  const operation = mobileOutboxDeliveryDecode(raw.operation, id);
  if (expected && (identity(operation) !== identity(expected) || operation.revision < expected.revision)) return invalid();
  return { operation, durable: raw.durable };
}
function native(handle: Native | null | undefined): Native {
  if (!handle?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to deliver pending tasks.');
  return letGoAware(handle);
}
async function local(handle: Native | null | undefined, request: Obj): Promise<unknown> {
  const reply = await bridgeReply(native(handle), { op: 'mobileOutboxDelivery', ...request });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  return reply.value;
}
export async function mobileOutboxDeliveryStatus(handle: Native | null | undefined, operationId: string): Promise<MobileOutboxDeliveryStatus> {
  if (!text(operationId)) return invalid();
  return status(await local(handle, { action: 'status', operationId }), operationId);
}
export async function mobileOutboxDeliveryRecover(handle: Native | null | undefined, input: MobileOutboxDeliveryReceipt): Promise<MobileOutboxDeliveryStatus> {
  const receipt = mobileOutboxDeliveryDecode(input, input.operationId);
  const result = status(await local(handle, { action: 'recover', operationId: receipt.operationId, revision: receipt.revision }), receipt.operationId, receipt);
  if (!result.durable || canonical(result.operation) !== canonical(receipt)) return invalid();
  return result;
}
export async function mobileOutboxDeliveryRetire(handle: Native | null | undefined, input: MobileOutboxDeliveryReceipt): Promise<MobileOutboxDeliveryStatus> {
  const receipt = mobileOutboxDeliveryDecode(input, input.operationId);
  if (!['reserved', 'retired'].includes(receipt.state)) throw new ClientError('Resolve this command before retiring its reservation.', 'outbox-stale');
  const result = status(await local(handle, { action: 'retire', operationId: receipt.operationId, revision: receipt.revision }), receipt.operationId, receipt);
  const expected = { ...receipt, state: 'retired', revision: receipt.revision + (receipt.state === 'reserved' ? 1 : 0) };
  if (!result.durable || canonical(result.operation) !== canonical(expected)) return invalid();
  return result;
}
function endpoint(client: T3Client, environmentId: string): number {
  if (client.connection !== 'connected' || client.environmentId !== environmentId || !integer(client.generation))
    throw new ClientError('Connect to this pending task\'s environment before delivery.', 'outbox-stale');
  return client.generation;
}
export async function mobileOutboxDeliveryReserve(client: T3Client, handle: Native | null | undefined,
  capture: MobileOutboxCapture, input: MobileOutboxWireRequest, expectedRetiredRevision?: number): Promise<MobileOutboxDeliveryStatus> {
  const snapshot = mobileOutboxSnapshot(client), row = snapshot.rows.find(row => row.record.messageId === capture.messageId);
  if (!snapshot.complete || !snapshot.ownerEpoch || !row || row.status !== 'confirmed' || row.held || !integer(capture.nativeRevision, 1)
    || canonical(mobileOutboxCapture(client, capture.messageId)) !== canonical(capture)
    || snapshot.outcomes.some(item => item.messageId === capture.messageId && ['unknown', 'uncertain'].includes(item.status)))
    throw new ClientError('Read and confirm this pending task before delivery.', 'outbox-stale');
  const record = row.record, command = copy(input);
  if (canonical(command.owner) !== canonical({ origin: record.origin, environmentId: record.environmentId,
    threadId: record.threadId, messageId: record.messageId, commandId: record.commandId })) return invalid();
  if (expectedRetiredRevision !== undefined && !integer(expectedRetiredRevision, 1)) return invalid();
  const expected = mobileOutboxDeliveryDecode({ kind: 'outbox', operationId: command.payload.commandId,
    revision: (expectedRetiredRevision ?? 0) + 1, origin: record.origin, environmentId: record.environmentId,
    messageId: record.messageId, threadId: record.threadId, rowToken: capture.token, rowRevision: capture.nativeRevision,
    record, stage: command.stage, method: command.method, payload: command.payload,
    attachmentIDs: record.attachments.map(file => file.id), state: 'reserved',
    ...(expectedRetiredRevision === undefined ? {} : { retiredRevision: expectedRetiredRevision }) }, String(command.payload.commandId));
  const generation = endpoint(client, record.environmentId);
  const raw = await client.call(native(handle), { op: 'mobileOutboxDelivery', action: 'reserve',
    expectedOrigin: record.origin, expectedEnvironmentId: record.environmentId, ownerEpoch: snapshot.ownerEpoch,
    messageId: record.messageId, expectedToken: capture.token, expectedRevision: capture.nativeRevision,
    record: copy(record), stage: command.stage, method: command.method, payload: command.payload,
    ...(expectedRetiredRevision === undefined ? {} : { expectedRetiredRevision }) }, generation, true);
  return status(raw, expected.operationId, expected);
}
/** Bind the final command to the native journal's exact saved inline ACK. Native
 * validates the whole saved pair; JavaScript sends no replacement payload or record. */
export async function mobileOutboxDeliveryReserveInline(client: T3Client, handle: Native | null | undefined,
  input: MobileOutboxInlineReceipt, expectedRetiredRevision?: number): Promise<MobileOutboxDeliveryStatus> {
  const source = mobileOutboxInlineDecode(input, input.operationId);
  if (source.state !== 'acknowledged') throw new ClientError('Confirm the saved image result before reserving its command.', 'outbox-stale');
  if (expectedRetiredRevision !== undefined && !integer(expectedRetiredRevision, 1)) return invalid();
  const materialized = mobileOutboxMaterializeInline(source.template, source.result);
  if (materialized.status !== 'ready') return invalid();
  const command = materialized.value, record = source.record;
  const expected = mobileOutboxDeliveryDecode({ kind: 'outbox', operationId: record.commandId,
    revision: (expectedRetiredRevision ?? 0) + 1, origin: source.origin, environmentId: source.environmentId,
    messageId: source.messageId, threadId: source.threadId, rowToken: source.rowToken, rowRevision: source.rowRevision,
    record, stage: command.stage, method: command.method, payload: command.payload,
    attachmentIDs: source.attachmentIDs, state: 'reserved',
    inlineSource: { operationId: source.operationId, ackRevision: source.revision, payloadDigest: source.payloadDigest },
    ...(expectedRetiredRevision === undefined ? {} : { retiredRevision: expectedRetiredRevision }) }, record.commandId);
  const generation = endpoint(client, source.environmentId);
  const raw = await client.call(native(handle), { op: 'mobileOutboxDelivery', action: 'reserveInline',
    inlineOperationId: source.operationId, inlineRevision: source.revision, ownerEpoch: mobileOutboxSnapshot(client).ownerEpoch,
    expectedOrigin: source.origin, expectedEnvironmentId: source.environmentId,
    ...(expectedRetiredRevision === undefined ? {} : { expectedRetiredRevision }) }, generation, true);
  // Identity compares the original record/row, exact materialized payload and the immutable source pointer.
  const result = status(raw, record.commandId, expected);
  if (!result.durable) return invalid();
  return result;
}
/** Retry by native receipt identity only. Never rebuild a payload from the current row. */
export async function mobileOutboxDeliverySend(client: T3Client, handle: Native | null | undefined,
  input: MobileOutboxDeliveryReceipt, retryRejected = false): Promise<MobileOutboxDeliveryStatus> {
  const receipt = mobileOutboxDeliveryDecode(input, input.operationId), generation = endpoint(client, receipt.environmentId);
  if (receipt.state === 'retired' || receipt.state === 'rejected' && (receipt.stage !== 'settings-sync' || retryRejected !== true))
    throw new ClientError('This command needs an explicit supported resolution.', 'outbox-stale');
  const snapshot = mobileOutboxSnapshot(client);
  const raw = await client.call(native(handle), { op: 'mobileOutboxDelivery', action: 'send',
    expectedOrigin: receipt.origin, expectedEnvironmentId: receipt.environmentId,
    operationId: receipt.operationId, revision: receipt.revision, ownerEpoch: snapshot.ownerEpoch,
    ...(retryRejected ? { retryRejected: true } : {}) }, generation, true);
  const result = status(raw, receipt.operationId, receipt), saved = result.operation!;
  if (!result.durable) return invalid();
  if (receipt.state === 'acknowledged') {
    if (canonical(saved) !== canonical(receipt)) return invalid();
  } else {
    const previous = receipt.state === 'issued' ? 'uncertain' : receipt.state;
    const states = previous === 'uncertain' ? ['acknowledged', 'uncertain']
      : previous === 'rejected' ? ['acknowledged', 'uncertain', 'rejected'] : ['acknowledged', 'uncertain', 'rejected', 'reserved'];
    if (saved.attemptPreviousState !== previous || saved.attemptRevision !== receipt.revision + 1
      || saved.revision !== receipt.revision + 2 || !states.includes(saved.state)) return invalid();
  }
  return result;
}

/** Complete only the delivered capture. Edited rows stay available for the
 * source's separate recovery policy, with the ACK preventing another send. */
export async function mobileOutboxDeliveryComplete(client: T3Client, handle: Native | null | undefined,
  input: MobileOutboxDeliveryReceipt, retryCleanupRevision?: number): Promise<MobileOutboxDeliveryCompletion> {
  const receipt = mobileOutboxDeliveryDecode(input, input.operationId);
  if (receipt.state !== 'acknowledged' || retryCleanupRevision !== undefined && (retryCleanupRevision !== receipt.revision
    || !receipt.cleanup || !['edited', 'failed', 'not-started'].includes(receipt.cleanup.phase)))
    throw new ClientError('Resolve the saved delivery outcome before cleanup.', 'outbox-stale');
  return mobileOutboxCompleteDelivery(client, handle, { operationId: receipt.operationId, revision: receipt.revision,
    messageId: receipt.messageId, ...(retryCleanupRevision === undefined ? {} : { retryCleanupRevision }) }, (raw, admitted) => {
    const result = status(raw, receipt.operationId, receipt), saved = result.operation!, metadata = saved.cleanup;
    if (!object(raw) || !result.durable || saved.state !== 'acknowledged' || !metadata
      || canonical(saved.result) !== canonical(receipt.result) || saved.attemptRevision !== receipt.attemptRevision
      || saved.attemptPreviousState !== receipt.attemptPreviousState
      || raw.cleanup !== (metadata.phase === 'pending' ? 'uncertain' : metadata.phase)
      || canonical(raw.outcome) !== canonical(metadata.outcome)) return invalid();
    const previous = receipt.cleanup;
    if (previous && retryCleanupRevision === undefined) {
      for (const key of ['mutationId', 'ownerEpoch', 'ackRevision', 'intentRevision'] as const)
        if (metadata[key] !== previous[key]) return invalid();
      if (!['pending', 'uncertain'].includes(previous.phase) && canonical(saved) !== canonical(receipt)) return invalid();
    } else {
      if (metadata.intentRevision !== receipt.revision + 1) return invalid();
      if (receipt.stage === 'start-turn' && (metadata.mutationId !== admitted.mutationId || metadata.ownerEpoch !== admitted.ownerEpoch))
        return invalid();
    }
    return { operation: saved, durable: true, cleanup: raw.cleanup as MobileOutboxDeliveryCompletion['cleanup'], outcome: copy(metadata.outcome) };
  });
}
