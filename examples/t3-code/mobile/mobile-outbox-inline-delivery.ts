// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Invocation-only bridge. Native owns bytes, immutable attempts and the existing journal.
import type { T3Client } from './shared/client';
import type { Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileOutboxCapture, mobileOutboxSnapshot, type MobileOutboxCapture } from './mobile-outbox';
import { mobileOutboxDecode, mobileOutboxCanonicalOrigin, type MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxMaterializeInline, type MobileOutboxInlineTemplate } from './mobile-outbox-inline';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

export interface MobileOutboxInlineReceipt {
  kind: 'outbox-inline'; operationId: string; revision: number; origin: string; environmentId: string;
  messageId: string; threadId: string; rowToken: string; rowRevision: number; record: MobileOutboxRecord;
  template: Omit<MobileOutboxInlineTemplate, 'inline'> & { inline: { index: number; localId: string; sha256: string }[] };
  attachmentIDs: string[]; state: 'reserved' | 'retired' | 'issued' | 'uncertain' | 'acknowledged' | 'rejected';
  payloadDigest?: string; attemptRevision?: number; attemptPreviousState?: 'reserved' | 'uncertain' | 'rejected';
  result?: { attachments: Obj[] }; error?: Obj;
}
export interface MobileOutboxInlineStatus { operation: MobileOutboxInlineReceipt | null; durable: boolean }
export interface MobileOutboxInlineLookup { operations: MobileOutboxInlineStatus[] }
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const object = (value: unknown): value is Obj => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = (value: unknown, minimum = 0): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum;
const uuid = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
const fields = (value: Obj, names: string[]): boolean => Object.keys(value).length === names.length && Object.keys(value).every(key => names.includes(key));
const digest = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const receiptKeys = ['kind', 'operationId', 'revision', 'origin', 'environmentId', 'messageId', 'threadId', 'rowToken',
  'rowRevision', 'record', 'template', 'attachmentIDs', 'state', 'payloadDigest', 'attemptRevision', 'attemptPreviousState', 'result', 'error'];
const invalid = (): never => { throw new ClientError('The native image reservation is invalid. Read its saved status.', 'protocol', true); };
function json(value: unknown, ancestors = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (!object(value) && !Array.isArray(value) || ancestors.has(value)) return false;
  ancestors.add(value);
  const valid = Object.values(value).every(item => json(item, ancestors));
  ancestors.delete(value); return valid;
}
function templateValid(raw: unknown, record: MobileOutboxRecord, captured: boolean): boolean {
  if (!object(raw) || !fields(raw, ['owner', 'commandTemplate', 'inline']) || !object(raw.commandTemplate)
    || !fields(raw.commandTemplate, ['owner', 'stage', 'method', 'payload']) || !object(raw.commandTemplate.payload)
    || !Array.isArray(raw.inline) || raw.inline.length < 1 || raw.inline.length > 100) return false;
  const owner = { origin: record.origin, environmentId: record.environmentId, threadId: record.threadId,
    messageId: record.messageId, commandId: record.commandId };
  const command = raw.commandTemplate, payload = command.payload as Obj;
  if (canonical(raw.owner) !== canonical(owner) || canonical(command.owner) !== canonical(owner)
    || command.stage !== 'start-turn' || payload.commandId !== record.commandId || payload.threadId !== record.threadId) return false;
  if (record.creation ? command.method !== 'orchestration.launchThread' || !object(payload.initialMessage)
    : command.method !== 'orchestration.dispatchCommand' || payload.type !== 'message.dispatch') return false;
  const message = record.creation ? payload.initialMessage as Obj : payload;
  if (message.messageId !== record.messageId || !Array.isArray(message.attachments)
    || message.attachments.length !== record.attachments.length) return false;
  const slots = new Set<number>(), ids = new Set<string>();
  let previous = -1;
  for (const binding of raw.inline) {
    if (!object(binding) || !fields(binding, captured ? ['index', 'localId', 'sha256'] : ['index', 'localId'])
      || !integer(binding.index) || binding.index <= previous || !(typeof binding.localId === 'string' && uuid(binding.localId.toLowerCase()))
      || ids.has(binding.localId) || captured && (typeof binding.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(binding.sha256))) return false;
    const local = record.attachments[binding.index], slot = message.attachments[binding.index];
    if (!local || local.id !== binding.localId || local.kind !== 'image' || !object(slot)
      || !fields(slot, ['type', 'name', 'mimeType', 'sizeBytes']) || slot.type !== 'image'
      || slot.name !== local.name || slot.mimeType !== local.mimeType || slot.sizeBytes !== local.sizeBytes) return false;
    previous = binding.index; slots.add(binding.index); ids.add(binding.localId);
  }
  return message.attachments.every((slot, index) => slots.has(index) || object(slot)
    && !('dataUrl' in slot) && typeof slot.id === 'string' && slot.id === record.attachments[index]!.uploadId
    && record.attachments[index]!.uploadEnvironmentId === record.environmentId);
}
/** Validate ownership and the native reservation lifecycle. Native validates the complete command schema. */
export function mobileOutboxInlineDecode(raw: unknown, operationId: string): MobileOutboxInlineReceipt {
  if (!object(raw) || !json(raw) || Object.keys(raw).some(key => !receiptKeys.includes(key))
    || !uuid(operationId) || raw.operationId !== operationId || raw.kind !== 'outbox-inline'
    || !integer(raw.rowRevision, 1) || typeof raw.rowToken !== 'string' || !raw.rowToken || raw.rowToken.trim() !== raw.rowToken
    || raw.rowToken.length > 4096 || !integer(raw.revision, 1)
    || !['reserved', 'retired', 'issued', 'uncertain', 'acknowledged', 'rejected'].includes(String(raw.state))) return invalid();
  const decoded = mobileOutboxDecode(raw.record);
  if (!decoded.ok) return invalid();
  const record = decoded.record;
  if (operationId === record.commandId || ['origin', 'environmentId', 'messageId', 'threadId'].some(key => raw[key] !== record[key as keyof MobileOutboxRecord])
    || canonical(raw.attachmentIDs) !== canonical(record.attachments.map(file => file.id)) || !templateValid(raw.template, record, true)) return invalid();
  if ('attemptRevision' in raw) {
    const attempt = raw.attemptRevision, previous = raw.attemptPreviousState;
    if (!integer(attempt, 2) || attempt > Number.MAX_SAFE_INTEGER - 2 || !digest(raw.payloadDigest)
      || !['reserved', 'uncertain', 'rejected'].includes(String(previous))
      || raw.revision !== attempt + (raw.state === 'issued' ? 0 : raw.state === 'retired' ? 2 : 1)
      || ['reserved', 'retired'].includes(String(raw.state)) && previous !== 'reserved'
      || raw.state === 'rejected' && previous === 'uncertain') return invalid();
    if (raw.state === 'issued' ? 'result' in raw || 'error' in raw
      : raw.state === 'acknowledged' ? !('result' in raw) || 'error' in raw : 'result' in raw || !object(raw.error)) return invalid();
    if (raw.state === 'acknowledged') {
      const result = raw.result;
      if (!object(result) || !fields(result, ['attachments']) || !Array.isArray(result.attachments)
        || !result.attachments.every(item => object(item) && fields(item, ['type', 'id', 'name', 'mimeType', 'sizeBytes'])))
        return invalid();
      if (mobileOutboxMaterializeInline(raw.template as unknown as MobileOutboxInlineTemplate, result).status !== 'ready') return invalid();
    }
  } else if (!['reserved', 'retired'].includes(String(raw.state)) || raw.revision !== (raw.state === 'reserved' ? 1 : 2)
    || ['payloadDigest', 'attemptPreviousState', 'result', 'error'].some(key => key in raw)) return invalid();
  return copy(raw) as unknown as MobileOutboxInlineReceipt;
}
function status(raw: unknown, id: string): MobileOutboxInlineStatus {
  if (!object(raw) || typeof raw.durable !== 'boolean') return invalid();
  if (raw.operation === null) return raw.durable ? invalid() : { operation: null, durable: false };
  return { operation: mobileOutboxInlineDecode(raw.operation, id), durable: raw.durable };
}
function native(handle: Native | null | undefined): Native {
  if (!handle?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to prepare queued images.');
  return letGoAware(handle);
}
async function local(handle: Native | null | undefined, request: Obj): Promise<unknown> {
  const reply = await bridgeReply(native(handle), { op: 'mobileOutboxInline', ...request });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  return reply.value;
}
/** Discover saved identities after a lost reply. An empty snapshot is not write admission. */
export async function mobileOutboxInlineLookup(handle: Native | null | undefined,
  input: MobileOutboxWireOwner): Promise<MobileOutboxInlineLookup> {
  const ownerKeys = ['origin', 'environmentId', 'threadId', 'messageId', 'commandId'];
  if (!object(input) || !fields(input, ownerKeys) || !mobileOutboxCanonicalOrigin(input.origin)
    || !ownerKeys.slice(1).every(key => typeof input[key] === 'string' && input[key].length > 0 && input[key].trim() === input[key])) return invalid();
  const owner = copy(input);
  const raw = await local(handle, { action: 'lookup', owner });
  if (!object(raw) || !fields(raw, ['operations']) || !Array.isArray(raw.operations)) return invalid();
  const ids = new Set<string>();
  let active = 0;
  const operations = raw.operations.map(value => {
    if (!object(value) || !fields(value, ['operation', 'durable']) || !object(value.operation)) return invalid();
    const result = status(value, String(value.operation.operationId)), operation = result.operation!;
    if (ids.has(operation.operationId) || canonical(operation.template.owner) !== canonical(owner)
      || operation.state !== 'retired' && ++active > 1) return invalid();
    ids.add(operation.operationId); return result;
  });
  return { operations };
}
export async function mobileOutboxInlineStatus(handle: Native | null | undefined, operationId: string): Promise<MobileOutboxInlineStatus> {
  if (!uuid(operationId)) return invalid();
  return status(await local(handle, { action: 'status', operationId }), operationId);
}
export async function mobileOutboxInlineRecover(handle: Native | null | undefined, input: MobileOutboxInlineReceipt): Promise<MobileOutboxInlineStatus> {
  const receipt = mobileOutboxInlineDecode(input, input.operationId);
  const result = status(await local(handle, { action: 'recover', operationId: receipt.operationId, revision: receipt.revision }), receipt.operationId);
  if (!result.durable || canonical(result.operation) !== canonical(receipt)) return invalid();
  return result;
}
export async function mobileOutboxInlineRetire(handle: Native | null | undefined, input: MobileOutboxInlineReceipt): Promise<MobileOutboxInlineStatus> {
  const receipt = mobileOutboxInlineDecode(input, input.operationId);
  if (!['reserved', 'retired'].includes(receipt.state))
    throw new ClientError('Resolve this image request before retiring its reservation.', 'outbox-stale');
  const result = status(await local(handle, { action: 'retire', operationId: receipt.operationId, revision: receipt.revision }), receipt.operationId);
  if (!result.durable || canonical(result.operation) !== canonical({ ...receipt, state: 'retired', revision: receipt.revision + (receipt.state === 'reserved' ? 1 : 0) })) return invalid();
  return result;
}
export async function mobileOutboxInlineReserve(client: T3Client, handle: Native | null | undefined,
  capture: MobileOutboxCapture, operationId: string, input: MobileOutboxInlineTemplate): Promise<MobileOutboxInlineStatus> {
  const snapshot = mobileOutboxSnapshot(client), row = snapshot.rows.find(row => row.record.messageId === capture.messageId);
  if (!snapshot.complete || !snapshot.ownerEpoch || !row || row.status !== 'confirmed' || row.held || !integer(capture.nativeRevision, 1)
    || canonical(mobileOutboxCapture(client, capture.messageId)) !== canonical(capture)
    || snapshot.outcomes.some(item => item.messageId === capture.messageId && ['unknown', 'uncertain'].includes(item.status)))
    throw new ClientError('Read and confirm this pending task before preparing its images.', 'outbox-stale');
  const record = copy(row.record);
  if (!uuid(operationId) || operationId === record.commandId || !json(input) || !templateValid(input, record, false)) return invalid();
  const template = copy(input), token = capture.token, revision = capture.nativeRevision;
  if (client.connection !== 'connected' || client.environmentId !== record.environmentId || !integer(client.generation))
    throw new ClientError('Connect to this pending task\'s environment before preparing its images.', 'outbox-stale');
  const raw = await client.call(native(handle), { op: 'mobileOutboxInline', action: 'reserve', operationId,
    expectedOrigin: record.origin, expectedEnvironmentId: record.environmentId, ownerEpoch: snapshot.ownerEpoch,
    messageId: record.messageId, expectedToken: token, expectedRevision: revision, record, template }, client.generation, true);
  const result = status(raw, operationId), saved = result.operation;
  if (!result.durable || !saved || saved.rowToken !== token || saved.rowRevision !== revision || canonical(saved.record) !== canonical(record)
    || canonical({ ...saved.template, inline: saved.template.inline.map(({ index, localId }) => ({ index, localId })) }) !== canonical(template)) return invalid();
  return result;
}

/** Retry only the saved native attempt. The request carries no bytes or replacement template. */
export async function mobileOutboxInlineSend(client: T3Client, handle: Native | null | undefined,
  input: MobileOutboxInlineReceipt, retryRejected = false): Promise<MobileOutboxInlineStatus> {
  const receipt = mobileOutboxInlineDecode(input, input.operationId);
  if (receipt.state === 'retired' || receipt.state === 'rejected' && retryRejected !== true)
    throw new ClientError('This image request needs an explicit supported resolution.', 'outbox-stale');
  if (client.connection !== 'connected' || client.environmentId !== receipt.environmentId || !integer(client.generation))
    throw new ClientError('Connect to this pending task\'s environment before preparing its images.', 'outbox-stale');
  const raw = await client.call(native(handle), { op: 'mobileOutboxInline', action: 'send',
    expectedOrigin: receipt.origin, expectedEnvironmentId: receipt.environmentId,
    operationId: receipt.operationId, revision: receipt.revision, ownerEpoch: mobileOutboxSnapshot(client).ownerEpoch,
    ...(retryRejected ? { retryRejected: true } : {}) }, client.generation, true);
  const result = status(raw, receipt.operationId), saved = result.operation;
  if (!result.durable || !saved) return invalid();
  const mutable = new Set(['revision', 'state', 'attemptRevision', 'attemptPreviousState', 'result', 'error']);
  // The first verified expansion adds the payload digest. Every later attempt must retain it.
  if (receipt.payloadDigest === undefined) mutable.add('payloadDigest');
  const identity = (value: MobileOutboxInlineReceipt) => canonical(Object.fromEntries(Object.entries(value).filter(([key]) => !mutable.has(key))));
  if (identity(saved) !== identity(receipt)) return invalid();
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
