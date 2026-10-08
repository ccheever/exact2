// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Native owns accepted FIFO storage work. This owner retains only plain projection data.
import type { T3Client } from './shared/client';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileOutboxDecode, mobileOutboxEncode, mobileOutboxGroup, type MobileOutboxRecord } from './mobile-outbox-model';

type Client = Pick<T3Client, 'revision'>;
type Plain = Record<string, unknown>;
export interface MobileOutboxCapture { messageId: string; token: string; localRevision: number; nativeRevision: number | null }
export interface MobileOutboxRow {
  record: MobileOutboxRecord; token: string; localRevision: number; nativeRevision: number | null;
  status: 'optimistic' | 'confirmed' | 'uncertain'; held: boolean;
}
export interface MobileOutboxExpected { expectedToken?: string; expectedRevision?: number }
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
}
interface Row extends MobileOutboxRow { ordinal: number; epoch: string }
interface State {
  ownerEpoch: string | null; sequence: number; ordinal: number; lastRead: number; revision: number; published: string;
  initialized: boolean; complete: boolean; errors: unknown[]; recovery: unknown[];
  rows: Record<string, Row>; localRevisions: Record<string, number>; nativeRevisions: Record<string, number>;
  intents: Record<string, Intent>; outcomes: Record<string, MobileOutboxOutcome>;
}
const dictionary = <T>(): Record<string, T> => Object.create(null);
const states = new WeakMap<Client, State>();
function state(client: Client): State {
  let value = states.get(client);
  if (!value) { value = { ownerEpoch: null, sequence: 0, ordinal: 0, lastRead: 0, revision: 0, published: '', initialized: false,
    complete: false, errors: [], recovery: [], rows: dictionary(), localRevisions: dictionary(), nativeRevisions: dictionary(),
    intents: dictionary(), outcomes: dictionary() }; states.set(client, value); }
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
    intents: Object.values(value.intents), recovery: value.recovery };
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
        !Array.isArray(raw.errors) || !Array.isArray(raw.records) || !Array.isArray(raw.outcomes) || !Array.isArray(raw.mutations) || !object(raw.revisions) || !object(raw.tokens))
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
      ...(record ? { record: clone(record) } : {}), ...expected, ...(operation === 'remove' ? { requireUnheld } : {}) });
    const result = outcome(raw, messageId, mutationId); adoptOutcome(value, result, ordinal); change(client, value); return clone(result);
  } catch (error) {
    const result: MobileOutboxOutcome = { mutationId, messageId, status: 'unknown', message: String(error) };
    adoptOutcome(value, result, ordinal); change(client, value); return result;
  }
}
export const mobileOutboxEnqueue = (client: Client, native: Native | null | undefined, record: MobileOutboxRecord) =>
  mutate(client, native, 'enqueue', record.messageId, record, {}, false);
export const mobileOutboxUpdate = (client: Client, native: Native | null | undefined, record: MobileOutboxRecord, expected: MobileOutboxExpected = {}) =>
  mutate(client, native, 'update', record.messageId, record, expected, false);
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
