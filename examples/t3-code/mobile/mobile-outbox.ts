// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Native owns accepted FIFO storage work. This owner retains only plain projection data.
import type { T3Client } from './shared/client';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileOutboxCanonicalOrigin, mobileOutboxDecode, mobileOutboxEncode, mobileOutboxGroup, type MobileOutboxRecord } from './mobile-outbox-model';

import { mobileOutboxTransferDecodeCapture, mobileOutboxTransferDecodeClaim, mobileOutboxTransferCanonical,
  type MobileOutboxTransferCapture, type MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';

import { threadSendTransferDecodeCapture, threadSendTransferDecodeClaim,
  type ThreadSendTransferCapture, type ThreadSendTransferClaim } from './thread-send-transfer-model';
type TransferClaim = MobileOutboxTransferClaim | ThreadSendTransferClaim;
export interface MobileOutboxThreadTarget { origin:string; environmentId:string; threadId:string; draftKey:string }
const threadClaim = (claim:TransferClaim):claim is ThreadSendTransferClaim=>'kind' in claim&&claim.kind==='ordinary';
function decodeTransfer(raw:unknown):TransferClaim {
  if (object(raw)&&Object.hasOwn(raw,'kind')) {
    if(raw.kind!=='ordinary')throw new Error('Unknown draft transfer kind.');
    return threadSendTransferDecodeClaim(raw);
  }
  return mobileOutboxTransferDecodeClaim(raw);
}
type Client = Pick<T3Client, 'revision'>;
type Plain = Record<string, unknown>;
export interface MobileOutboxCapture { messageId: string; token: string; localRevision: number; nativeRevision: number | null }
export interface MobileOutboxRow {
  record: MobileOutboxRecord; token: string; localRevision: number; nativeRevision: number | null;
  status: 'optimistic' | 'confirmed' | 'uncertain'; held: boolean;
}
export interface MobileOutboxExpected { expectedToken?: string; expectedRevision?: number }
export interface MobileOutboxSavedUpdate {
  ownerEpoch: string; mutationId: string; messageId: string; operation: 'update';
  record: MobileOutboxRecord; expectedToken: string; expectedRevision: number; requireUnheld: false;
}
export interface MobileOutboxSavedRemoval extends Omit<MobileOutboxSavedUpdate, 'operation' | 'record'> { operation: 'remove' }
export interface MobileOutboxCurrent { record: MobileOutboxRecord | null; revision: number; token: string; pending: boolean }
export interface MobileOutboxOutcome {
  mutationId: string; messageId: string; status: 'committed' | 'stale' | 'failed' | 'uncertain' | 'unknown';
  revision?: number; record?: MobileOutboxRecord | null; removed?: MobileOutboxRecord | null; message: string;
  current?: MobileOutboxCurrent; ownerEpoch?: string; sequenceFloor?: number;
}
interface Intent {
  mutationId: string; messageId: string; ownerEpoch: string; operation: 'enqueue' | 'update' | 'remove';
  ordinal: number; record?: MobileOutboxRecord; expected: MobileOutboxExpected;
  status: 'submitted' | MobileOutboxOutcome['status'];
  threadTransfer?: { target:MobileOutboxThreadTarget; capture:ThreadSendTransferCapture };
}
interface Row extends MobileOutboxRow { ordinal: number; epoch: string }
interface State {
  ownerEpoch: string | null; sequence: number; ordinal: number; lastRead: number; revision: number; published: string;
  initialized: boolean; complete: boolean; errors: unknown[]; recovery: unknown[];
  rows: Record<string, Row>; localRevisions: Record<string, number>; nativeRevisions: Record<string, number>;
  transfers: Record<string, TransferClaim>; intents: Record<string, Intent>; outcomes: Record<string, MobileOutboxOutcome>;
}
const dictionary = <T>(): Record<string, T> => Object.create(null);
const states = new WeakMap<Client, State>();
function state(client: Client): State {
  let value = states.get(client);
  if (!value) { value = { ownerEpoch: null, sequence: 0, ordinal: 0, lastRead: 0, revision: 0, published: '', initialized: false,
    complete: false, errors: [], recovery: [], rows: dictionary(), localRevisions: dictionary(), nativeRevisions: dictionary(),
    transfers: dictionary(), intents: dictionary(), outcomes: dictionary() }; states.set(client, value); }
  return value;
}
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const canonical = (value: unknown): string => JSON.stringify(value, (_key, item: unknown) =>
  item && typeof item === 'object' && !Array.isArray(item) ?
    Object.fromEntries(Object.entries(item).sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)) : item);
const object = (value: unknown): value is Plain => value !== null && typeof value === 'object' && !Array.isArray(value);
const nonempty = (value: unknown): value is string => typeof value === 'string' && value.length > 0;
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
function projection(value: State) {
  const rows = Object.values(value.rows).map(({ ordinal: _ordinal, epoch: _epoch, ...row }) => row);
  return { initialized: value.initialized, ownerEpoch: value.ownerEpoch, complete: value.complete, errors: value.errors, rows,
    groups: mobileOutboxGroup(rows.map(row => row.record)), outcomes: Object.values(value.outcomes),
    transfers: Object.values(value.transfers).filter((claim):claim is MobileOutboxTransferClaim=>!threadClaim(claim)),
    threadTransfers:Object.values(value.transfers).filter(threadClaim), intents: Object.values(value.intents), recovery: value.recovery };
}
function change(client: Client, value: State): void {
  const published = canonical(projection(value));
  if (published === value.published) return;
  value.published = published; value.revision++; client.revision++;
}
function bump(value: State, messageId: string): number { return value.localRevisions[messageId] = (value.localRevisions[messageId] ?? 0) + 1; }
function decodeRecord(raw: unknown, messageId?: string): MobileOutboxRecord {
  const result = mobileOutboxDecode(raw);
  if (!result.ok || messageId !== undefined && result.record.messageId !== messageId) throw new Error('The outbox returned an invalid record owner.');
  return result.record;
}
function current(raw: unknown, messageId: string): MobileOutboxCurrent {
  if (!object(raw) || !integer(raw.revision) || typeof raw.token !== 'string' || typeof raw.pending !== 'boolean' ||
      raw.record !== null && !nonempty(raw.token)) throw new Error('The outbox returned an invalid current row.');
  return { record: raw.record === null ? null : decodeRecord(raw.record, messageId), revision: raw.revision, token: raw.token, pending: raw.pending };
}
function outcome(raw: unknown, messageId?: string, mutationId?: string): MobileOutboxOutcome {
  if (!object(raw) || !nonempty(raw.messageId) || !nonempty(raw.mutationId) || typeof raw.message !== 'string' ||
      messageId !== undefined && raw.messageId !== messageId || mutationId !== undefined && raw.mutationId !== mutationId ||
      !['committed', 'stale', 'failed', 'uncertain', 'unknown'].includes(String(raw.status))) throw new Error('The outbox returned an invalid mutation outcome.');
  const base: MobileOutboxOutcome = { messageId: raw.messageId, mutationId: raw.mutationId,
    status: raw.status as MobileOutboxOutcome['status'], message: raw.message };
  if (raw.status === 'unknown') return base;
  if (!integer(raw.revision) || !nonempty(raw.ownerEpoch) || !integer(raw.sequenceFloor)) throw new Error('The outbox outcome is missing its native revision.');
  return { ...base, revision: raw.revision, ownerEpoch: raw.ownerEpoch, sequenceFloor: raw.sequenceFloor,
    record: raw.record === null ? null : decodeRecord(raw.record, raw.messageId),
    removed: raw.removed === null ? null : decodeRecord(raw.removed, raw.messageId), current: current(raw.current, raw.messageId) };
}
/** Shared reply decoder; reading an outcome alone never adopts it. */
export function mobileOutboxDecodeOutcome(raw: unknown, messageId: string, mutationId: string): MobileOutboxOutcome {
  return clone(outcome(raw, messageId, mutationId));
}
function nativeHandle(native: Native | null | undefined): Native {
  if (!native?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to manage pending tasks.');
  return letGoAware(native);
}
async function invoke(native: Native, request: Plain): Promise<unknown> {
  const reply = await bridgeReply(native, { op: 'mobileOutbox', ...request });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  return reply.value;
}
function readyState(client: Client, native: Native | null | undefined): { value: State; native: Native; epoch: string } {
  const value = state(client), handle = nativeHandle(native);
  if (!value.initialized || !value.ownerEpoch) throw new ClientError('Read the pending tasks before changing them.');
  return { value, native: handle, epoch: value.ownerEpoch };
}
function tokenSequence(token: string, epoch: string): number | null {
  if (!token.startsWith(`${epoch}:`)) return null;
  const sequence = Number(token.slice(epoch.length + 1));
  return integer(sequence) && sequence > 0 ? sequence : null;
}
/** Apply native accepted state only if its admission watermark covers a local optimistic row. */
function adoptCurrent(value: State, messageId: string, next: MobileOutboxCurrent, epoch: string, floor: number,
  ordinal: number, held?: boolean): void {
  if (value.ownerEpoch !== epoch) return;
  const row = value.rows[messageId], sequence = row ? tokenSequence(row.token, epoch) : null;
  if (row && row.token !== next.token && row.status !== 'confirmed' && sequence !== null && sequence > floor) return;
  if (row && row.epoch !== epoch && row.ordinal > ordinal) return;
  if (next.revision < (value.nativeRevisions[messageId] ?? 0)) return;
  value.nativeRevisions[messageId] = next.revision;
  if (!next.record) {
    if (row) { delete value.rows[messageId]; bump(value, messageId); }
    return;
  }
  const logical = row?.token === next.token && canonical(row.record) === canonical(next.record) ? row.localRevision : bump(value, messageId);
  value.rows[messageId] = { record: clone(next.record), token: next.token, nativeRevision: next.revision,
    localRevision: logical, status: next.pending ? value.intents[next.token]?.status === 'submitted' ? 'optimistic' : 'uncertain' : 'confirmed', held: held ?? row?.held ?? false,
    ordinal: Math.max(ordinal, row?.ordinal ?? 0), epoch };
}
function adoptOutcome(value: State, result: MobileOutboxOutcome, ordinal: number): void {
  const old = value.outcomes[result.mutationId];
  if (old && !['unknown', 'uncertain'].includes(old.status) && ['unknown', 'uncertain'].includes(result.status)) return;
  value.outcomes[result.mutationId] = clone(result);
  const intent = value.intents[result.mutationId]; if (intent) intent.status = result.status;
  if (result.current && result.ownerEpoch && result.sequenceFloor !== undefined) {
    if (value.ownerEpoch === result.ownerEpoch) value.sequence = Math.max(value.sequence, result.sequenceFloor);
    adoptCurrent(value, result.messageId, result.current, result.ownerEpoch, result.sequenceFloor, ordinal);
  }
  if (result.status === 'uncertain' || result.status === 'unknown') {
    const row = value.rows[result.messageId];
    if (row?.token === result.mutationId) row.status = 'uncertain';
  }
}
export function mobileOutboxSnapshot(client: Client) {
  const value = state(client);
  return clone({ revision: value.revision, ...projection(value) });
}
export function mobileOutboxCapture(client: Client, messageId: string): MobileOutboxCapture | null {
  const row = state(client).rows[messageId];
  return row ? { messageId, token: row.token, localRevision: row.localRevision, nativeRevision: row.nativeRevision } : null;
}
export async function mobileOutboxRead(client: Client, native: Native | null | undefined): Promise<boolean> {
  const value = state(client), ordinal = ++value.ordinal;
  try {
    const raw = await invoke(nativeHandle(native), { action: 'read' });
    if (!object(raw) || !nonempty(raw.ownerEpoch) || !integer(raw.sequenceFloor) || typeof raw.complete !== 'boolean' ||
        !Array.isArray(raw.errors) || !Array.isArray(raw.records) || !Array.isArray(raw.outcomes) || !Array.isArray(raw.mutations) || !Array.isArray(raw.transfers) || !object(raw.revisions) || !object(raw.tokens))
      throw new Error('The outbox inventory is invalid.');
    if (ordinal < value.lastRead) return value.complete;
    const errors: unknown[] = clone(raw.errors), rows: Array<{ id: string; row: MobileOutboxCurrent; held: boolean }> = [];
    const results: MobileOutboxOutcome[] = [], seen = new Set<string>();
    for (const item of raw.records) try {
      if (!object(item) || typeof item.held !== 'boolean') throw new Error('Invalid held row.');
      const record = decodeRecord(item.record), decoded = current({ ...item, record }, record.messageId);
      if (seen.has(record.messageId)) throw new Error('Duplicate outbox record.');
      if (raw.revisions[record.messageId] !== decoded.revision || raw.tokens[record.messageId] !== decoded.token) throw new Error('Outbox row revision evidence does not match.');
      seen.add(record.messageId); rows.push({ id: record.messageId, row: decoded, held: item.held });
    } catch (error) { errors.push({ message: String(error), ownership: 'unknown', raw: item }); }
    for (const item of raw.outcomes) try { results.push(outcome(item)); }
    catch (error) { errors.push({ message: String(error), ownership: 'unknown', raw: item }); }
    const transferIds = new Set<string>();
    for (const item of raw.transfers) try {
      const claim = decodeTransfer(item);
      if (transferIds.has(claim.transferId)) throw new Error('Duplicate draft transfer identity.');
      transferIds.add(claim.transferId); adoptTransfer(value, claim);
    }
    catch (error) { errors.push({ message: String(error), ownership: 'unknown', raw: item }); }
    const epochChanged = value.ownerEpoch !== raw.ownerEpoch;
    value.ownerEpoch = raw.ownerEpoch; value.sequence = epochChanged ? raw.sequenceFloor : Math.max(value.sequence, raw.sequenceFloor);
    if (epochChanged) value.nativeRevisions = dictionary();
    value.initialized = true; value.lastRead = ordinal;
    for (const result of results) adoptOutcome(value, result, ordinal);
    for (const row of rows) adoptCurrent(value, row.id, row.row, raw.ownerEpoch, raw.sequenceFloor, ordinal, row.held);
    for (const [id, revision] of Object.entries(raw.revisions)) {
      if (!integer(revision) || typeof raw.tokens[id] !== 'string' || revision > 0 && !nonempty(raw.tokens[id])) {
        errors.push({ message: 'Invalid outbox tombstone revision/token.', ownership: 'unknown', messageId: id }); continue; }
      if (!seen.has(id) && errors.length === 0) adoptCurrent(value, id, { record: null, revision, token: raw.tokens[id] as string, pending: false }, raw.ownerEpoch, raw.sequenceFloor, ordinal);
    }
    for (const marker of raw.mutations) if (!object(marker) || !nonempty(marker.messageId) || !object(marker.mutation) || !nonempty(marker.mutation.mutationId))
      errors.push({ message: 'Invalid unresolved outbox mutation.', ownership: 'unknown', raw: marker });
    value.errors = errors; value.recovery = clone(raw.mutations); value.complete = raw.complete && errors.length === 0;
    change(client, value); return value.complete;
  } catch (error) {
    if (letGo(error)) throw error;
    if (ordinal >= value.lastRead) { value.lastRead = ordinal; value.complete = false; value.errors = [{ message: String(error), ownership: 'unknown' }]; change(client, value); }
    return false;
  }
}
function assertExpected(expected: MobileOutboxExpected): void {
  if (expected.expectedToken !== undefined && !nonempty(expected.expectedToken) ||
      expected.expectedRevision !== undefined && !integer(expected.expectedRevision)) throw new ClientError('The pending task capture is invalid.');

}
async function mutate(client: Client, native: Native | null | undefined, operation: Intent['operation'],
  messageId: string, input: MobileOutboxRecord | undefined, expected: MobileOutboxExpected, requireUnheld: boolean): Promise<MobileOutboxOutcome> {
  const { value, native: handle, epoch } = readyState(client, native);
  const record = input ? mobileOutboxEncode(input) : undefined;
  if (!nonempty(messageId)) throw new ClientError('Choose a pending task.');
  assertExpected(expected);
  const previous = value.rows[messageId];
  if (!Number.isSafeInteger(value.sequence + 1)) throw new ClientError('The pending task sequence is exhausted.');
  const mutationId = `${epoch}:${++value.sequence}`, ordinal = ++value.ordinal;
  const intent: Intent = { mutationId, messageId, ownerEpoch: epoch, operation, ordinal, expected: clone(expected), status: 'submitted', ...(record ? { record } : {}) };
  value.intents[mutationId] = intent;
  if (operation === 'enqueue' && record) value.rows[messageId] = { record: clone(record), token: mutationId,
    localRevision: bump(value, messageId), nativeRevision: null, status: 'optimistic', held: previous?.held ?? false, ordinal, epoch };
  change(client, value);
  // invoke enters native.later synchronously. There is no prior await or shared promise tail.
  try {
    const raw = await invoke(handle, { action: 'mutate', ownerEpoch: epoch, mutationId, messageId, operation,
      ...(record ? { record: clone(record) } : {}), ...expected, ...(operation === 'remove' || requireUnheld ? { requireUnheld } : {}) });
    const result = outcome(raw, messageId, mutationId); adoptOutcome(value, result, ordinal); change(client, value); return clone(result);
  } catch (error) {
    const result: MobileOutboxOutcome = { mutationId, messageId, status: 'unknown', message: String(error) };
    adoptOutcome(value, result, ordinal); change(client, value); return result;
  }
}
export const mobileOutboxEnqueue = (client: Client, native: Native | null | undefined, record: MobileOutboxRecord) =>
  mutate(client, native, 'enqueue', record.messageId, record, {}, false);
export const mobileOutboxUpdate = (client: Client, native: Native | null | undefined, record: MobileOutboxRecord, expected: MobileOutboxExpected = {}, requireUnheld = false) =>
  mutate(client, native, 'update', record.messageId, record, expected, requireUnheld);
/** Reserve an immutable update before root durably saves its editor marker. No native admission. */
export function mobileOutboxPrepareUpdate(client: Client, input: MobileOutboxRecord, expected: MobileOutboxExpected): MobileOutboxSavedUpdate {
  const value = state(client), record = mobileOutboxEncode(input);
  if (!value.initialized || !value.ownerEpoch || !value.complete) throw new ClientError('Read a complete pending-task inventory before saving edits.');
  assertExpected(expected);
  if (!nonempty(expected.expectedToken) || !integer(expected.expectedRevision)) throw new ClientError('Capture the exact pending task before saving edits.');
  if (!Number.isSafeInteger(value.sequence + 1)) throw new ClientError('The pending task sequence is exhausted.');
  const request: MobileOutboxSavedUpdate = { ownerEpoch: value.ownerEpoch, mutationId: `${value.ownerEpoch}:${++value.sequence}`,
    messageId: record.messageId, operation: 'update', record: clone(record), expectedToken: expected.expectedToken,
    expectedRevision: expected.expectedRevision, requireUnheld: false };
  const freeze = (item: unknown): void => { if (item && typeof item === 'object') { Object.values(item).forEach(freeze); Object.freeze(item); } };
  freeze(request); return request;
}
/** Root must persist this exact request before first invocation, and retain its editor hold. */
export async function mobileOutboxResumeUpdate(client: Client, native: Native | null | undefined,
  saved: MobileOutboxSavedUpdate, holdOwner: string): Promise<MobileOutboxOutcome> {
  return resumeSavedMutation(client, native, saved, holdOwner, 'update');
}
/** Reserve before persisting the recovery intent. This never removes the queue row. */
export function mobileOutboxPrepareRemoval(client: Client, messageId: string, expected: MobileOutboxExpected): MobileOutboxSavedRemoval {
  const value = state(client);
  if (!value.initialized || !value.ownerEpoch || !value.complete) throw new ClientError('Read a complete pending-task inventory before recovering a draft.');
  assertExpected(expected);
  if (!nonempty(messageId) || !nonempty(expected.expectedToken) || !integer(expected.expectedRevision))
    throw new ClientError('Capture the exact pending task before recovering its draft.');
  if (!Number.isSafeInteger(value.sequence + 1)) throw new ClientError('The pending task sequence is exhausted.');
  return Object.freeze({ ownerEpoch: value.ownerEpoch, mutationId: `${value.ownerEpoch}:${++value.sequence}`,
    messageId, operation: 'remove', expectedToken: expected.expectedToken, expectedRevision: expected.expectedRevision, requireUnheld: false });
}
/** Caller must durably publish the destination and save this exact request first.
 * Native requires the held row for first admission; terminal replay uses its receipt. */
export async function mobileOutboxResumeRemoval(client: Client, native: Native | null | undefined,
  saved: MobileOutboxSavedRemoval, holdOwner: string): Promise<MobileOutboxOutcome> {
  return resumeSavedMutation(client, native, saved, holdOwner, 'remove');
}
async function resumeSavedMutation(client: Client, native: Native | null | undefined,
  saved: MobileOutboxSavedUpdate | MobileOutboxSavedRemoval, holdOwner: string, operation: 'update' | 'remove'): Promise<MobileOutboxOutcome> {
  const { value, native: handle, epoch } = readyState(client, native);
  const keys = ['ownerEpoch', 'mutationId', 'messageId', 'operation', 'expectedToken', 'expectedRevision', 'requireUnheld',
    ...(operation === 'update' ? ['record'] : [])];
  if (!value.complete || !nonempty(holdOwner) || !object(saved) || !nonempty(saved.ownerEpoch)
    || !nonempty(saved.mutationId) || tokenSequence(saved.mutationId, saved.ownerEpoch) === null
    || saved.operation !== operation || saved.requireUnheld !== false || !nonempty(saved.expectedToken)
    || !integer(saved.expectedRevision) || !nonempty(saved.messageId) || Object.keys(saved).length !== keys.length
    || Object.keys(saved).some(key => !keys.includes(key))) throw new ClientError('The saved pending mutation is invalid.');
  const request = clone<MobileOutboxSavedUpdate | MobileOutboxSavedRemoval>(saved);
  if (request.operation === 'update') request.record = decodeRecord(request.record, request.messageId);
  value.sequence = Math.max(value.sequence, tokenSequence(request.mutationId, epoch) ?? 0);
  const ordinal = ++value.ordinal;
  value.intents[request.mutationId] = { mutationId: request.mutationId, messageId: request.messageId,
    ownerEpoch: request.ownerEpoch, operation, ordinal, ...(request.operation === 'update' ? { record: clone(request.record) } : {}),
    expected: { expectedToken: request.expectedToken, expectedRevision: request.expectedRevision }, status: 'submitted' };
  change(client, value);
  try {
    const result = outcome(await invoke(handle, { action: operation === 'update' ? 'resumeUpdate' : 'resumeRemoval', ownerEpoch: epoch, holdOwner, request }), request.messageId, request.mutationId);
    adoptOutcome(value, result, ordinal); change(client, value); return clone(result);
  } catch (error) {
    const result: MobileOutboxOutcome = { mutationId: request.mutationId, messageId: request.messageId, status: 'unknown', message: String(error) };
    adoptOutcome(value, result, ordinal); change(client, value); return result;
  }
}
export const mobileOutboxRemove = (client: Client, native: Native | null | undefined, messageId: string, expected: MobileOutboxExpected = {}, requireUnheld = true) =>
  mutate(client, native, 'remove', messageId, undefined, expected, requireUnheld);
export async function mobileOutboxStatus(client: Client, native: Native | null | undefined, messageId: string, mutationId: string): Promise<MobileOutboxOutcome> {
  const value = state(client), ordinal = ++value.ordinal;
  const result = outcome(await invoke(nativeHandle(native), { action: 'status', messageId, mutationId }), messageId, mutationId);
  adoptOutcome(value, result, ordinal); change(client, value); return clone(result);
}
function unresolved(value: State, messageId: string): boolean {
  return Object.values(value.outcomes).some(result => result.messageId === messageId && ['unknown', 'uncertain'].includes(result.status));
}
export async function mobileOutboxConfirmQueued(client: Client, native: Native | null | undefined, capture: MobileOutboxCapture): Promise<boolean> {
  const { value, native: handle, epoch } = readyState(client, native), row = value.rows[capture.messageId];
  if (!row || row.token !== capture.token || row.localRevision !== capture.localRevision || row.held || row.status === 'uncertain' || unresolved(value, capture.messageId)) return false;
  const raw = await invoke(handle, { action: 'confirmQueued', ownerEpoch: epoch, messageId: capture.messageId,
    token: capture.token, ...(capture.nativeRevision === null ? {} : { expectedRevision: capture.nativeRevision }) });
  if (!object(raw) || typeof raw.current !== 'boolean' || !integer(raw.revision)) throw new Error('The outbox returned an invalid confirmation.');
  const now = value.rows[capture.messageId];
  return raw.current && value.ownerEpoch === epoch && !!now && now.token === capture.token && now.localRevision === capture.localRevision && !now.held && now.status !== 'uncertain' && !unresolved(value, capture.messageId);
}
export async function mobileOutboxHold(client: Client, native: Native | null | undefined, capture: MobileOutboxCapture, owner: string): Promise<boolean> {
  const { value, native: handle, epoch } = readyState(client, native);
  const raw = await invoke(handle, { action: 'hold', ownerEpoch: epoch, messageId: capture.messageId, owner, expectedToken: capture.token });
  if (!object(raw) || typeof raw.held !== 'boolean') throw new Error('The outbox returned an invalid editor hold.');
  if (raw.held && value.rows[capture.messageId]?.token === capture.token) { value.rows[capture.messageId]!.held = true; change(client, value); }
  return raw.held;
}
export async function mobileOutboxReleaseHold(client: Client, native: Native | null | undefined, messageId: string, owner: string): Promise<boolean> {
  const { native: handle, epoch } = readyState(client, native);
  const raw = await invoke(handle, { action: 'releaseHold', ownerEpoch: epoch, messageId, owner });
  if (!object(raw) || typeof raw.released !== 'boolean') throw new Error('The outbox returned an invalid editor release.');
  // Another editor may still hold this row. Only a later native read clears held.
  return raw.released;
}
export async function mobileOutboxAcknowledge(client: Client, native: Native | null | undefined, messageId: string, mutationId: string): Promise<boolean> {
  const value = state(client), known = value.outcomes[mutationId];
  if (!known || known.messageId !== messageId || ['unknown', 'uncertain'].includes(known.status)) return false;
  const raw = await invoke(nativeHandle(native), { action: 'acknowledge', messageId, mutationId });
  if (!object(raw) || typeof raw.acknowledged !== 'boolean') throw new Error('The outbox returned an invalid acknowledgment.');
  if (raw.acknowledged) { delete value.outcomes[mutationId]; delete value.intents[mutationId]; change(client, value); }
  return raw.acknowledged;
}
export async function mobileOutboxRecover(client: Client, native: Native | null | undefined, messageId: string, mutationId: string,
  decision: 'commit' | 'rollback' | 'retry'): Promise<MobileOutboxOutcome> {
  const value = state(client), ordinal = ++value.ordinal;
  const result = outcome(await invoke(nativeHandle(native), { action: 'recover', messageId, mutationId, decision }), messageId, mutationId);
  adoptOutcome(value, result, ordinal); change(client, value); return clone(result);
}

/** Delivery uses this owner's mutation sequence and outcome projection. The caller
 * validates the entire native receipt before any returned row can be adopted. */
export async function mobileOutboxCompleteDelivery<T extends { outcome: unknown }>(client: Client, native: Native | null | undefined,
  request: { operationId: string; revision: number; messageId: string; retryCleanupRevision?: number },
  decode: (raw: unknown, admitted: { ownerEpoch: string; mutationId: string }) => T): Promise<T> {
  const { value, native: handle, epoch } = readyState(client, native);
  if (!Number.isSafeInteger(value.sequence + 1)) throw new ClientError('The pending task sequence is exhausted.');
  const mutationId = `${epoch}:${++value.sequence}`, ordinal = ++value.ordinal;
  const reply = await bridgeReply(handle, { op: 'mobileOutboxDelivery', action: 'complete',
    operationId: request.operationId, revision: request.revision, ownerEpoch: epoch, mutationId,
    ...(request.retryCleanupRevision === undefined ? {} : { retryCleanupRevision: request.retryCleanupRevision }) });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  const decoded = decode(reply.value, { ownerEpoch: epoch, mutationId });
  if (decoded.outcome !== null) {
    // Restart or a lost reply may return an earlier saved cleanup mutation. The
    // receipt decoder verifies its identity; normal epoch/watermark adoption applies.
    const result = outcome(decoded.outcome, request.messageId);
    adoptOutcome(value, result, ordinal); change(client, value);
  }
  return decoded;
}


export interface MobileOutboxTransferResult {
  disposition: 'created' | 'existing' | 'conflict' | 'unknown';
  claim: MobileOutboxTransferClaim | null; outcome: MobileOutboxOutcome | null;
}
function transferTransition(old:TransferClaim,claim:TransferClaim):boolean {
  if(threadClaim(old)!==threadClaim(claim)||threadClaim(old)&&threadClaim(claim)
    &&(old.origin!==claim.origin||old.environmentId!==claim.environmentId))throw new Error('The draft transfer scope changed.');
  for(const field of ['draftKey','fingerprint','messageId','threadId','commandId','mutationId'] as const)
    if(old[field]!==claim[field])throw new Error('The draft transfer identity changed.');
  if(old.record&&claim.record&&(canonical(old.record)!==canonical(claim.record)||canonical(old.capture)!==canonical(claim.capture)))
    throw new Error('The original draft transfer capture changed.');
  if(old.state==='queued'&&claim.state==='released'||old.state==='failed'&&claim.state==='completed'
    ||old.state==='completed'&&claim.state==='released'||old.state==='released'&&claim.state==='completed')
    throw new Error('The draft transfer retirement changed its outcome.');
  if(['completed','released'].includes(old.state)||old.state!=='prepared'&&claim.state==='prepared')return false;
  if(old.state!=='prepared'&&claim.state!==old.state&&!['completed','released'].includes(claim.state))throw new Error('The draft transfer outcome changed.');
  return true;
}
function adoptTransfer(value:State,claim:TransferClaim):void {
  const old=value.transfers[claim.transferId];
  if(old&&!transferTransition(old,claim))return;
  value.transfers[claim.transferId]=clone(claim);
}
function transferOutcome(raw: unknown, claim: TransferClaim): MobileOutboxOutcome | null {
  const result = raw === null ? null : outcome(raw, claim.messageId, claim.mutationId);
  if (claim.state === 'queued' && result?.status !== 'committed' || claim.state === 'failed' && !['failed', 'stale'].includes(result?.status ?? '') ||
      claim.state === 'prepared' && result && !['unknown', 'uncertain'].includes(result.status))
    throw new Error('The draft transfer and enqueue outcome disagree.');
  if (claim.state === 'queued' && canonical(result?.record) !== canonical(claim.record))
    throw new Error('The draft transfer lost its original enqueue record.');
  return result;
}
/** Native deduplicates the full captured draft before accepting the requested IDs. */
async function enqueueTransfer<C extends TransferClaim>(client: Client, native: Native | null | undefined,
  input: MobileOutboxRecord, captured:MobileOutboxTransferCapture|ThreadSendTransferCapture,
  decode:(raw:unknown)=>C, ordinary:boolean):Promise<{disposition:MobileOutboxTransferResult['disposition'];claim:C|null;outcome:MobileOutboxOutcome|null}> {
  const { value, native: handle, epoch } = readyState(client, native);
  if (!value.complete) throw new ClientError('Resolve the incomplete pending-task inventory before submitting a draft.');
  const record = mobileOutboxEncode(input), capture = ordinary?threadSendTransferDecodeCapture(captured,record):mobileOutboxTransferDecodeCapture(captured, record);
  const target=ordinary?threadTarget({origin:capture.draft.origin,environmentId:capture.draft.environmentId,threadId:record.threadId,draftKey:capture.draft.key}):null;
  if (!Number.isSafeInteger(value.sequence + 1)) throw new ClientError('The pending task sequence is exhausted.');
  const mutationId = `${epoch}:${++value.sequence}`, ordinal = ++value.ordinal, messageId = record.messageId;
  const previous = value.rows[messageId];
  value.intents[mutationId] = { mutationId, messageId, ownerEpoch: epoch, operation: 'enqueue', ordinal, record: clone(record), expected: {}, status: 'submitted',
    ...(target?{threadTransfer:{target:clone(target),capture:clone(capture as ThreadSendTransferCapture)}}:{}) };
  value.rows[messageId] = { record: clone(record), token: mutationId, localRevision: bump(value, messageId), nativeRevision: null,
    status: 'optimistic', held: value.rows[messageId]?.held ?? false, ordinal, epoch };
  change(client, value);
  try {
    const raw = await invoke(handle, { action: 'enqueueTransfer', ownerEpoch: epoch, mutationId, record, capture,...(target?{kind:'ordinary',target}:{}) });
    if (!object(raw) || ordinary&&typeof raw.disposition!=='string' || !['created', 'existing', 'conflict'].includes(String(raw.disposition))) throw new Error('Invalid draft transfer admission.');
    const claim = decode(raw.claim), result = transferOutcome(raw.outcome, claim);
    if(target&&!sameThreadTarget(claim,target))throw new Error('The ordinary transfer belongs to another scope.');
    if (claim.draftKey !== capture.draft.key || raw.disposition === 'created' &&
        (claim.mutationId !== mutationId || claim.messageId !== messageId || canonical(claim.record) !== canonical(record)) ||
        raw.disposition !== 'conflict' && claim.capture && mobileOutboxTransferCanonical(claim.capture) !== mobileOutboxTransferCanonical(capture))
      throw new Error('The draft transfer admission belongs to another capture.');
    adoptTransfer(value, claim);
    if (raw.disposition !== 'created') {
      // Only this rejected proposal is disposable. A later local replacement owns itself.
      if (value.rows[messageId]?.token === mutationId) {
        if (!result && previous) value.rows[messageId] = { ...previous, status: 'uncertain' };
        else { delete value.rows[messageId]; bump(value, messageId); }
      }
      delete value.intents[mutationId]; delete value.outcomes[mutationId];
    }
    if (result) adoptOutcome(value, result, ordinal);
    change(client, value);
    return { disposition: raw.disposition as 'created' | 'existing' | 'conflict',
      claim: ordinary?decode(value.transfers[claim.transferId]):clone(claim), outcome: result ? clone(result) : null };
  } catch (error) {
    const result: MobileOutboxOutcome = { mutationId, messageId, status: 'unknown', message: String(error) };
    adoptOutcome(value, result, ordinal); change(client, value);
    if(ordinary&&letGo(error))throw error;
    return { disposition: 'unknown', claim: null, outcome: result };
  }
}
export const mobileOutboxEnqueueTransfer = (client:Client,native:Native|null|undefined,input:MobileOutboxRecord,captured:MobileOutboxTransferCapture):Promise<MobileOutboxTransferResult> =>
  enqueueTransfer(client,native,input,captured,mobileOutboxTransferDecodeClaim,false);
export interface MobileOutboxThreadTransferResult {
  disposition:MobileOutboxTransferResult['disposition'];claim:ThreadSendTransferClaim|null;outcome:MobileOutboxOutcome|null;
}
export const mobileOutboxEnqueueThreadTransfer = (client:Client,native:Native|null|undefined,input:MobileOutboxRecord,captured:ThreadSendTransferCapture):Promise<MobileOutboxThreadTransferResult> =>
  enqueueTransfer(client,native,input,captured,threadSendTransferDecodeClaim,true);
export async function mobileOutboxTransferLookup(client: Client, native: Native | null | undefined, draftKey: string,
  captured?: MobileOutboxTransferCapture): Promise<{ complete: boolean; fingerprint: string | null; claims: MobileOutboxTransferClaim[] }> {
  if (!/^new-task:[\w-]{1,128}$/.test(draftKey)) throw new ClientError('Choose an independent draft.');
  const value = state(client), capture = captured ? mobileOutboxTransferDecodeCapture(captured) : undefined;
  if (capture && capture.draft.key !== draftKey) throw new ClientError('The captured draft has changed.');
  const raw = await invoke(nativeHandle(native), { action: 'transferLookup', draftKey, ...(capture ? { capture } : {}) });
  if (!object(raw) || typeof raw.complete !== 'boolean' || !Array.isArray(raw.claims) ||
      (capture ? typeof raw.fingerprint !== 'string' || !/^[0-9a-f]{64}$/.test(raw.fingerprint) : raw.fingerprint !== null))
    throw new Error('Invalid draft transfer lookup.');
  const claims = raw.claims.map(mobileOutboxTransferDecodeClaim);
  if (claims.some(claim => claim.draftKey !== draftKey) || new Set(claims.map(claim => claim.transferId)).size !== claims.length)
    throw new Error('The transfer lookup contains another draft or duplicate owner.');
  for (const claim of claims) adoptTransfer(value, claim);
  change(client, value); return clone({ complete: raw.complete, fingerprint: raw.fingerprint as string | null, claims });
}
export async function mobileOutboxTransferStatus(client: Client, native: Native | null | undefined, transferId: string) {
  const value = state(client), ordinal = ++value.ordinal;
  const raw = await invoke(nativeHandle(native), { action: 'transferStatus', transferId });
  if (!object(raw)) throw new Error('Invalid draft transfer status.');
  if (raw.claim === null && raw.outcome === null) return { claim: null, outcome: null };
  const claim = mobileOutboxTransferDecodeClaim(raw.claim);
  if (claim.transferId !== transferId) throw new Error('The returned transfer belongs to another task.');
  const result = transferOutcome(raw.outcome, claim); adoptTransfer(value, claim);
  if (result) adoptOutcome(value, result, ordinal);
  change(client, value); return clone({ claim, outcome: result });
}
async function retireTransfer(client: Client, native: Native | null | undefined, claim: MobileOutboxTransferClaim, failed: boolean) {
  const original = mobileOutboxTransferDecodeClaim(claim), value = state(client);
  const raw = await invoke(nativeHandle(native), { action: failed ? 'releaseFailedTransfer' : 'completeTransfer',
    transferId: original.transferId, fingerprint: original.fingerprint });
  if (!object(raw) || typeof raw[failed ? 'released' : 'completed'] !== 'boolean') throw new Error('Invalid transfer retirement result.');
  const next = mobileOutboxTransferDecodeClaim(raw.claim);
  if (next.transferId !== original.transferId || next.fingerprint !== original.fingerprint || next.draftKey !== original.draftKey ||
      raw[failed ? 'released' : 'completed'] && next.state !== (failed ? 'released' : 'completed')) throw new Error('The transfer retirement owner is invalid.');
  adoptTransfer(value, next); change(client, value); return raw[failed ? 'released' : 'completed'] as boolean;
}
/** Native checks the durable preference marker itself; JS cannot declare disk success. */
export const mobileOutboxCompleteTransfer = (client: Client, native: Native | null | undefined, claim: MobileOutboxTransferClaim) => retireTransfer(client, native, claim, false);
export const mobileOutboxReleaseFailedTransfer = (client: Client, native: Native | null | undefined, claim: MobileOutboxTransferClaim) => retireTransfer(client, native, claim, true);

/** Target fields are explicit even after native compacts capture/record from a terminal claim. */
function threadTarget(input:MobileOutboxThreadTarget):MobileOutboxThreadTarget {
  if(!object(input)||![Object.prototype,null].includes(Object.getPrototypeOf(input)))throw new ClientError('Choose an ordinary draft.');
  const keys=Reflect.ownKeys(input),allowed=['origin','environmentId','threadId','draftKey'];
  if(keys.length!==4||keys.some(key=>typeof key!=='string'||!allowed.includes(key)
    ||!Object.getOwnPropertyDescriptor(input,key)?.enumerable||!('value' in Object.getOwnPropertyDescriptor(input,key)!)))throw new ClientError('The ordinary draft scope is invalid.');
  if(!mobileOutboxCanonicalOrigin(input.origin)||![input.environmentId,input.threadId].every(v=>typeof v==='string'&&v.length>0&&v.length<=4096&&v.trim()===v)
    ||input.threadId.startsWith('new:')||input.draftKey!==`${input.environmentId}:${input.threadId}`||input.draftKey.includes('~queued-edit~'))throw new ClientError('The ordinary draft scope is invalid.');
  return clone(input);
}
function sameThreadTarget(claim:TransferClaim,target:MobileOutboxThreadTarget):claim is ThreadSendTransferClaim {
  return threadClaim(claim)&&claim.origin===target.origin&&claim.environmentId===target.environmentId&&claim.threadId===target.threadId&&claim.draftKey===target.draftKey;
}
const claimTarget=(claim:ThreadSendTransferClaim):MobileOutboxThreadTarget=>({origin:claim.origin,environmentId:claim.environmentId,threadId:claim.threadId,draftKey:claim.draftKey});
export async function mobileOutboxThreadTransferLookup(client:Client,native:Native|null|undefined,input:MobileOutboxThreadTarget,
  captured?:ThreadSendTransferCapture):Promise<{complete:boolean;fingerprint:string|null;claims:ThreadSendTransferClaim[]}> {
  const target=threadTarget(input),value=state(client),capture=captured?threadSendTransferDecodeCapture(captured):undefined;
  if(capture&&(capture.draft.origin!==target.origin||capture.draft.environmentId!==target.environmentId||capture.draft.threadId!==target.threadId||capture.draft.key!==target.draftKey))throw new ClientError('The captured ordinary draft has changed.');
  const raw=await invoke(nativeHandle(native),{action:'transferLookup',kind:'ordinary',target,...(capture?{capture}:{})});
  if(!object(raw)||typeof raw.complete!=='boolean'||!Array.isArray(raw.claims)
    ||(capture?typeof raw.fingerprint!=='string'||!/^[0-9a-f]{64}$/.test(raw.fingerprint):raw.fingerprint!==null))throw new Error('Invalid ordinary transfer lookup.');
  const claims=raw.claims.map(threadSendTransferDecodeClaim);
  if(claims.some(claim=>!sameThreadTarget(claim,target))||new Set(claims.map(claim=>claim.transferId)).size!==claims.length)throw new Error('The ordinary lookup contains another scope or duplicate owner.');
  for(const claim of claims){const old=value.transfers[claim.transferId];if(old)transferTransition(old,claim)}
  for(const claim of claims)adoptTransfer(value,claim);
  change(client,value);return clone({complete:raw.complete,fingerprint:raw.fingerprint as string|null,
    claims:claims.map(claim=>threadSendTransferDecodeClaim(value.transfers[claim.transferId]))});
}
export async function mobileOutboxThreadTransferStatus(client:Client,native:Native|null|undefined,input:MobileOutboxThreadTarget,transferId:string) {
  const target=threadTarget(input);if(!nonempty(transferId))throw new ClientError('Choose an ordinary transfer.');
  const value=state(client),ordinal=++value.ordinal;
  const raw=await invoke(nativeHandle(native),{action:'transferStatus',kind:'ordinary',target,transferId});
  if(!object(raw))throw new Error('Invalid ordinary transfer status.');
  if(raw.claim===null&&raw.outcome===null){
    const known=value.transfers[transferId];
    return {claim:known&&sameThreadTarget(known,target)&&['completed','released'].includes(known.state)?clone(known):null,outcome:null};
  }
  const claim=threadSendTransferDecodeClaim(raw.claim);
  if(claim.transferId!==transferId||!sameThreadTarget(claim,target))throw new Error('The returned ordinary transfer belongs to another scope.');
  const result=transferOutcome(raw.outcome,claim);adoptTransfer(value,claim);
  if(result)adoptOutcome(value,result,ordinal);
  change(client,value);return clone({claim:threadSendTransferDecodeClaim(value.transfers[claim.transferId]),outcome:result});
}
async function retireThreadTransfer(client:Client,native:Native|null|undefined,input:ThreadSendTransferClaim,failed:boolean) {
  const original=threadSendTransferDecodeClaim(input),target=claimTarget(original),value=state(client);
  const raw=await invoke(nativeHandle(native),{action:failed?'releaseFailedTransfer':'completeTransfer',kind:'ordinary',target,
    transferId:original.transferId,fingerprint:original.fingerprint});
  const flag=failed?'released':'completed';
  if(!object(raw)||typeof raw[flag]!=='boolean')throw new Error('Invalid ordinary transfer retirement result.');
  const next=threadSendTransferDecodeClaim(raw.claim);
  if(!sameThreadTarget(next,target)||['transferId','fingerprint','messageId','commandId','mutationId'].some(key=>next[key as keyof ThreadSendTransferClaim]!==original[key as keyof ThreadSendTransferClaim])
    ||raw[flag]&&next.state!==(failed?'released':'completed'))throw new Error('The ordinary transfer retirement owner is invalid.');
  // Validate against the supplied immutable receipt even if this runtime has not read it yet.
  const advances=transferTransition(original,next);
  adoptTransfer(value,advances?next:original);change(client,value);return raw[flag] as boolean;
}
/** Native verifies persisted completion evidence; no caller-provided saved flag grants retirement. */
export const mobileOutboxCompleteThreadTransfer=(client:Client,native:Native|null|undefined,claim:ThreadSendTransferClaim)=>retireThreadTransfer(client,native,claim,false);
export const mobileOutboxReleaseFailedThreadTransfer=(client:Client,native:Native|null|undefined,claim:ThreadSendTransferClaim)=>retireThreadTransfer(client,native,claim,true);
