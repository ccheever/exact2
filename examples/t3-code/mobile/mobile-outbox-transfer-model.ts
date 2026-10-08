// App-owned durable capture, separate from the provider's queued command payload.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { MobileOutboxDraftPresentation } from './mobile-outbox-capture';
import { mobileOutboxCanonicalOrigin, mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';

export interface MobileOutboxTransferCapture { version: 1; draft: MobileOutboxDraftPresentation }
export interface MobileOutboxTransferClaim {
  transferId: string; draftKey: string; fingerprint: string;
  messageId: string; threadId: string; commandId: string; mutationId: string;
  state: 'prepared' | 'queued' | 'failed' | 'completed' | 'released';
  record: MobileOutboxRecord | null; capture: MobileOutboxTransferCapture | null;
}
type Plain = Record<string, unknown>;
const object = (value: unknown): value is Plain => value !== null && typeof value === 'object' && !Array.isArray(value);
const text = (value: unknown): value is string => typeof value === 'string';
const nonempty = (value: unknown): value is string => text(value) && value.length > 0 && value.trim() === value;
const integer = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const uuid = (value: unknown): value is string => text(value) && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
const key = (value: unknown): value is string => text(value) && /^new-task:[\w-]{1,128}$/.test(value);
const fields = (value: Plain, allowed: string[]) => Object.keys(value).every(field => allowed.includes(field));
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
export const mobileOutboxTransferCanonical = (value: unknown): string => JSON.stringify(value, (_key, item: unknown) =>
  object(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) : item);
function json(value: unknown, parents = new Set<object>()): boolean {
  if (value === null || text(value) || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (!object(value) && !Array.isArray(value) || parents.has(value as object)) return false;
  parents.add(value as object);
  const children = Array.isArray(value) ? [...value] : Object.values(value as object).filter(v => v !== undefined);
  const valid = children.every(v => json(v, parents));
  parents.delete(value as object); return valid;
}
function workspace(value: unknown, branchChoice = false): boolean {
  if (!object(value)) return false;
  return fields(value, ['envMode', 'branch', 'worktreePath', ...(branchChoice ? ['kind'] : [])]) &&
    ['local', 'worktree'].includes(String(value.envMode)) && text(value.branch) && text(value.worktreePath) &&
    (!branchChoice || ['automatic', 'explicit'].includes(String(value.kind)));
}
function choices(value: unknown): boolean {
  if (value === null) return true;
  if (!object(value) || !fields(value, ['providerId', 'modelId', 'modelOptions', 'runtimeMode', 'interactionMode'])) return false;
  if (['providerId', 'modelId', 'modelOptions'].some(k => k in value) &&
      (!text(value.providerId) || !text(value.modelId) || !Array.isArray(value.modelOptions) || !value.modelOptions.every(object))) return false;
  return ['runtimeMode', 'interactionMode'].every(k => value[k] === undefined || text(value[k]));
}
function descriptor(value: unknown): value is Plain & { id: string } {
  return object(value) && uuid(value.id) && nonempty(value.name) && nonempty(value.mimeType) && integer(value.sizeBytes);
}
export function mobileOutboxTransferDecodeCapture(raw: unknown, record?: MobileOutboxRecord): MobileOutboxTransferCapture {
  const bad = () => { throw new Error('The draft transfer capture is invalid.'); };
  if (!object(raw) || raw.version !== 1 || !fields(raw, ['version', 'draft']) || !object(raw.draft) || !json(raw)) return bad();
  const draft = raw.draft;
  if (!fields(draft, ['key', 'environmentId', 'projectId', 'origin', 'createdAt', 'revision', 'choices', 'branchChoice', 'attachmentIds', 'text', 'images', 'files', 'workspace']) ||
      !key(draft.key) || !nonempty(draft.environmentId) || !nonempty(draft.projectId) || !mobileOutboxCanonicalOrigin(draft.origin) ||
      !text(draft.createdAt) || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(draft.createdAt) || !Number.isFinite(Date.parse(draft.createdAt)) || new Date(draft.createdAt).toISOString() !== draft.createdAt || !integer(draft.revision) || !text(draft.text) || !choices(draft.choices) ||
      draft.workspace !== null && !workspace(draft.workspace) || draft.branchChoice !== undefined && !workspace(draft.branchChoice, true) ||
      !Array.isArray(draft.images) || !Array.isArray(draft.files) || !Array.isArray(draft.attachmentIds)) return bad();
  const images = draft.images, files = draft.files;
  if (!images.every(image => descriptor(image) && (image.uploadId === undefined || text(image.uploadId)) &&
        (image.source === undefined || object(image.source))) ||
      !files.every(file => descriptor(file) && file.draftKey === draft.key && file.environmentId === draft.environmentId &&
        nonempty(file.contextId) && file.source === 'attached' && text(file.attachmentId) && ['staged', 'ready'].includes(String(file.status)) &&
        ['videoWidth', 'videoHeight'].every(k => file[k] === undefined || typeof file[k] === 'number' && Number.isFinite(file[k]) && file[k] > 0))) return bad();
  const all = [...images, ...files] as Array<Plain & { id: string }>;
  if (!draft.attachmentIds.every(uuid) || new Set(all.map(item => item.id.toLowerCase())).size !== all.length ||
      draft.attachmentIds.length !== all.length || new Set(draft.attachmentIds).size !== all.length ||
      !draft.attachmentIds.every(id => all.some(item => item.id === id))) return bad();
  if (record) {
    if (draft.origin !== record.origin || draft.environmentId !== record.environmentId || draft.projectId !== record.creation?.projectId ||
        draft.text.trim() !== record.text || mobileOutboxTransferCanonical(draft.attachmentIds) !== mobileOutboxTransferCanonical(record.attachments.map(a => a.id))) return bad();
    for (const attachment of record.attachments) {
      const candidates = attachment.kind === 'image' ? images : files;
      const original = candidates.find(item => object(item) && item.id === attachment.id) as Plain | undefined;
      if (!original || !['name', 'mimeType', 'sizeBytes'].every(k => original[k] === (attachment as unknown as Plain)[k]) ||
          (attachment.kind === 'image' ? original.uploadId ?? '' : original.attachmentId) !== attachment.uploadId) return bad();
      for (const field of attachment.kind === 'image' ? ['source'] : ['contextId', 'source', 'videoWidth', 'videoHeight']) {
        if (mobileOutboxTransferCanonical(original[field]) !== mobileOutboxTransferCanonical((attachment as unknown as Plain)[field])) return bad();
      }
    }
  }
  return clone(raw) as unknown as MobileOutboxTransferCapture;
}
export function mobileOutboxTransferDecodeClaim(raw: unknown): MobileOutboxTransferClaim {
  if (!object(raw) || !nonempty(raw.transferId) || raw.transferId !== raw.messageId || !key(raw.draftKey) ||
      !text(raw.fingerprint) || !/^[0-9a-f]{64}$/.test(raw.fingerprint) || !nonempty(raw.threadId) || !nonempty(raw.commandId) || !nonempty(raw.mutationId) ||
      !['prepared', 'queued', 'failed', 'completed', 'released'].includes(String(raw.state))) throw new Error('The draft transfer claim is invalid.');
  const terminal = raw.state === 'completed' || raw.state === 'released';
  if (terminal) {
    if (raw.record !== null || raw.capture !== null) throw new Error('The retired draft transfer still has a capture.');
  } else {
    const decoded = mobileOutboxDecode(raw.record);
    if (!decoded.ok || decoded.record.messageId !== raw.messageId || decoded.record.threadId !== raw.threadId || decoded.record.commandId !== raw.commandId)
      throw new Error('The draft transfer record identity is invalid.');
    const capture = mobileOutboxTransferDecodeCapture(raw.capture, decoded.record);
    if (capture.draft.key !== raw.draftKey) throw new Error('The draft transfer belongs to another draft.');
  }
  return clone(raw) as unknown as MobileOutboxTransferClaim;
}
