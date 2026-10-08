// Pinned365aa87982 flush-before-remove recovery, over the existing app journal.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxSavedRemoval } from './mobile-outbox';
import { mobileOutboxDeliveryDecode, type MobileOutboxDeliveryReceipt } from './mobile-outbox-delivery';
import { mobileOutboxRecoveryKey } from './mobile-outbox-recovery-content';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

export interface MobileOutboxDraftHandoff {
  version: 1; operationId: string; receiptRevision: number; record: MobileOutboxRecord;
  request: MobileOutboxSavedRemoval; draftKey: string; draft: Obj;
}
const field = 'mobileOutboxDraftHandoffs';
const copy = <T>(value: T): T => value === undefined ? value : JSON.parse(JSON.stringify(value));
const object = (value: unknown): value is Obj => !!value && typeof value === 'object' && !Array.isArray(value);
const integer = (value: unknown, minimum = 0): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum;
const text = (value: unknown): value is string => typeof value === 'string' && !!value && value.trim() === value;
const fields = (value: Obj, names: string[]) => Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
const invalid = (): never => { throw new ClientError('The saved draft handoff is invalid. Keep the draft and original receipt.', 'protocol', true); };
function json(value: unknown, parents = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (!object(value) && !Array.isArray(value) || parents.has(value)) return false;
  parents.add(value); const valid = Object.values(value).every(item => json(item, parents)); parents.delete(value); return valid;
}

/** Raw invalid data remains visible to admission; dropping it could replay a consumed draft. */
export function mobileOutboxDraftHandoffsHydrate(client: T3Client, document: Obj): void {
  if (Object.hasOwn(document, field)) Object.assign(client.local, { [field]: copy(document[field]) });
}
export function mobileOutboxDraftHandoffsPersisted(client: T3Client): unknown { return copy(obj(client.local)[field]); }

/** Exact stored projection, shared with native evidence. Does not normalize or prune content. */
export function mobileOutboxDraftHandoffProjection(document: Obj, key: string): Obj {
  const controls = obj(document.composerControls), metadata = obj(obj(document.mobileNewTaskDrafts).records);
  return copy({ text: obj(document.drafts)[key] ?? '', images: obj(document.snapshotDrafts)[key] ?? [],
    files: Array.isArray(document.composerFiles) ? document.composerFiles.filter(raw => obj(raw).draftKey === key) : [],
    metadata: metadata[key] ?? null, staged: obj(controls.staged)[key] ?? null, workspace: obj(controls.contexts)[key] ?? null,
    order: obj(document.mobileAttachmentOrder)[key] ?? [],
    recovered: Object.fromEntries(Object.entries(obj(document.mobileRecoveredDrafts)).filter(([, raw]) => obj(raw).key === key)) });
}
export function mobileOutboxDraftHandoffDecode(raw: unknown, terminal: MobileOutboxDeliveryReceipt): MobileOutboxDraftHandoff {
  const receipt = mobileOutboxDeliveryDecode(terminal, terminal.operationId);
  if (!object(raw) || !json(raw) || !fields(raw, ['version', 'operationId', 'receiptRevision', 'record', 'request', 'draftKey', 'draft'])
    || raw.version !== 1 || raw.operationId !== receipt.operationId || raw.receiptRevision !== receipt.revision
    || receipt.stage !== 'start-turn') return invalid();
  const decoded = mobileOutboxDecode(raw.record);
  if (!decoded.ok) return invalid();
  const record = decoded.record, rejected = receipt.state === 'rejected';
  if (!rejected && !(receipt.state === 'acknowledged' && receipt.record.creation && receipt.cleanup?.phase === 'edited')
    || ['origin', 'environmentId', 'threadId', 'messageId', 'commandId', 'createdAt'].some(key => record[key as keyof MobileOutboxRecord] !== receipt.record[key as keyof MobileOutboxRecord])
    || raw.draftKey !== mobileOutboxRecoveryKey(record, rejected ? 'rejected' : 'accepted-edits')) return invalid();
  const request = raw.request, draft = raw.draft;
  if (!object(request) || !fields(request, ['ownerEpoch', 'mutationId', 'messageId', 'operation', 'expectedToken', 'expectedRevision', 'requireUnheld'])
    || !text(request.ownerEpoch) || !text(request.mutationId) || request.operation !== 'remove' || request.requireUnheld !== false
    || request.messageId !== record.messageId || !text(request.expectedToken) || !integer(request.expectedRevision, 1)) return invalid();
  const prefix = `${request.ownerEpoch}:`, suffix = request.mutationId.slice(prefix.length);
  if (!request.mutationId.startsWith(prefix) || !/^[1-9][0-9]*$/.test(suffix) || !integer(Number(suffix), 1)
    || !object(draft) || !fields(draft, ['text', 'images', 'files', 'metadata', 'staged', 'workspace', 'order', 'recovered'])
    || typeof draft.text !== 'string' || !Array.isArray(draft.images) || !draft.images.every(object)
    || !Array.isArray(draft.files) || !draft.files.every(file => object(file) && file.draftKey === raw.draftKey)
    || !Array.isArray(draft.order) || !draft.order.every(id => typeof id === 'string') || !object(draft.recovered)
    || !Object.values(draft.recovered).every(object) || ['metadata', 'staged', 'workspace'].some(key => draft[key] !== null && !object(draft[key]))) return invalid();
  if (rejected && record.creation) {
    const metadata = obj(draft.metadata);
    if (metadata.key !== raw.draftKey || metadata.projectId !== record.creation.projectId
      || ['origin', 'environmentId', 'createdAt'].some(key => metadata[key] !== record[key as keyof MobileOutboxRecord])) return invalid();
  }
  for (const file of record.attachments) {
    const candidates = (file.kind === 'image' ? draft.images : draft.files) as Obj[];
    if (!candidates.some(item => typeof item.id === 'string' && item.id.toLowerCase() === file.id.toLowerCase())) return invalid();
  }
  try {
    // Reject values JSON would silently erase or alter before a durability comparison.
    const clean = copy(raw);
    if (canonical(clean) !== canonical(raw)) return invalid();
    return clean as unknown as MobileOutboxDraftHandoff;
  } catch { return invalid(); }
}
export interface MobileOutboxDraftHandoffStatus { handoff: MobileOutboxDraftHandoff | null; durable: boolean }
async function invoke(native: Native, receipt: MobileOutboxDeliveryReceipt, input?: MobileOutboxDraftHandoff): Promise<MobileOutboxDraftHandoffStatus> {
  if (!native.available) throw new ClientError('Open T3 Code on iPhone or iPad to recover this draft.');
  receipt = mobileOutboxDeliveryDecode(receipt, receipt.operationId);
  const saved = input && mobileOutboxDraftHandoffDecode(input, receipt);
  const reply = await bridgeReply(letGoAware(native), { op: 'mobileOutboxDelivery',
    action: saved ? 'draftHandoffComplete' : 'draftHandoffStatus', operationId: receipt.operationId, ...(saved ? { handoff: saved } : {}) });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  const raw = reply.value;
  if (!object(raw) || typeof raw.durable !== 'boolean') return invalid();
  if (raw.handoff === null) { if (raw.durable || saved) return invalid(); return { handoff: null, durable: false }; }
  const handoff = mobileOutboxDraftHandoffDecode(raw.handoff, receipt);
  if (saved && (canonical(handoff) !== canonical(saved) || !raw.durable)) return invalid();
  return { handoff, durable: raw.durable };
}
export const mobileOutboxDraftHandoffStatus = (native: Native, receipt: MobileOutboxDeliveryReceipt) => invoke(native, receipt);
export const mobileOutboxDraftHandoffComplete = (native: Native, receipt: MobileOutboxDeliveryReceipt, handoff: MobileOutboxDraftHandoff) => invoke(native, receipt, handoff);
