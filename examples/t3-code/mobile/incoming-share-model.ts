// Pinned365aa87982 features/sharing/incoming-share-model.ts, over native owned byte IDs.
// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import { ClientError } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
import { fileStagingLimit, MAX_ATTACHMENTS, MAX_FILE_BYTES } from './shared/composer-editor-files';
import { fileTooLargeMessage } from './shared/composer-editor-attach';

export interface IncomingShareAttachment { id: string; kind: 'image' | 'file'; name: string; mimeType: string; sizeBytes: number }
export interface IncomingShareEntry {
  schemaVersion: 1; id: string; instanceId: string; createdAt: string; text: string; attachments: IncomingShareAttachment[]; warnings: string[];
}
export interface IncomingShareDestination { draftKey: string; environmentId: string; projectId: string; origin: string }
export interface IncomingShareImport {
  version: 1; shareId: string; instanceId: string; adoptionId: string; createdAt: string; destination: IncomingShareDestination; attachmentIds: string[];
}
export const incomingShareID = (value: unknown): value is string => typeof value === 'string' && /^share-[0-9a-f]{64}$/.test(value);
export const incomingShareUUID = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
export const incomingShareObject = (value: unknown): value is Obj => !!value && typeof value === 'object' && !Array.isArray(value);
const invalid = (): never => { throw new ClientError('The saved shared content is invalid. Keep the inbox item until it can be recovered.', 'protocol'); };
const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(item => typeof item === 'string');
export function incomingShareDestination(raw: unknown): IncomingShareDestination {
  if (!incomingShareObject(raw) || typeof raw.draftKey !== 'string' || !/^new-task:[\w-]{1,128}$/.test(raw.draftKey)
    || raw.draftKey.startsWith('new-task:pending-') || typeof raw.environmentId !== 'string' || !raw.environmentId
    || typeof raw.projectId !== 'string' || !raw.projectId || typeof raw.origin !== 'string' || !/^https?:\/\//.test(raw.origin)) return invalid();
  return { draftKey: raw.draftKey, environmentId: raw.environmentId, projectId: raw.projectId, origin: raw.origin };
}
export function incomingShareAttachment(raw: unknown): IncomingShareAttachment {
  if (!incomingShareObject(raw) || !incomingShareUUID(raw.id) || !['image', 'file'].includes(String(raw.kind))
    || typeof raw.name !== 'string' || !raw.name.trim() || typeof raw.mimeType !== 'string' || !raw.mimeType
    || typeof raw.sizeBytes !== 'number' || !Number.isSafeInteger(raw.sizeBytes) || raw.sizeBytes < 1
    || raw.sizeBytes > (raw.kind === 'image' ? 10 * 1024 * 1024 : MAX_FILE_BYTES)
    || raw.kind === 'image' && !['image/png', 'image/jpeg', 'image/gif', 'image/webp'].includes(raw.mimeType)) return invalid();
  return { id: raw.id, kind: raw.kind as 'image' | 'file', name: raw.name, mimeType: raw.mimeType, sizeBytes: raw.sizeBytes };
}
export function incomingShareEntry(raw: unknown): IncomingShareEntry {
  if (!incomingShareObject(raw) || raw.schemaVersion !== 1 || !incomingShareID(raw.id) || !incomingShareUUID(raw.instanceId)
    || typeof raw.createdAt !== 'string' || !Number.isFinite(Date.parse(raw.createdAt)) || typeof raw.text !== 'string'
    || !strings(raw.warnings) || !Array.isArray(raw.attachments) || raw.attachments.length > MAX_ATTACHMENTS) return invalid();
  const attachments = raw.attachments.map(incomingShareAttachment);
  if (new Set(attachments.map(file => file.id)).size !== attachments.length || !raw.text.trim() && !attachments.length) return invalid();
  return { schemaVersion: 1, id: raw.id, instanceId: raw.instanceId, createdAt: raw.createdAt, text: raw.text, attachments, warnings: [...raw.warnings] };
}
export function incomingShareImport(raw: unknown): IncomingShareImport {
  if (!incomingShareObject(raw) || raw.version !== 1 || !incomingShareID(raw.shareId) || !incomingShareUUID(raw.adoptionId) || !incomingShareUUID(raw.instanceId)
    || typeof raw.createdAt !== 'string' || !Number.isFinite(Date.parse(raw.createdAt)) || !strings(raw.attachmentIds)
    || raw.attachmentIds.length > MAX_ATTACHMENTS || !raw.attachmentIds.every(incomingShareUUID)
    || new Set(raw.attachmentIds).size !== raw.attachmentIds.length) return invalid();
  return { version: 1, shareId: raw.shareId, instanceId: raw.instanceId, adoptionId: raw.adoptionId, createdAt: raw.createdAt,
    destination: incomingShareDestination(raw.destination), attachmentIds: [...raw.attachmentIds] };
}
/** Generic files wait for a real config; unknown capabilities never mean support.
 * Images remain eligible without a file capability, matching the pinned source. */
export function incomingShareSelection(entry: IncomingShareEntry, config: Obj | null, existingIds: string[]) {
  if (entry.attachments.some(file => file.kind === 'file') && config === null) return { status: 'pending' as const };
  const limit = fileStagingLimit(obj(obj(config?.environment).capabilities));
  const ids = new Set(existingIds), attachments: IncomingShareAttachment[] = [], warnings = [...entry.warnings];
  let skipped = 0;
  for (const attachment of entry.attachments) {
    if (attachment.kind === 'file' && !limit) { warnings.push(`'${attachment.name}' was skipped because this server does not support files.`); continue; }
    if (attachment.kind === 'file' && attachment.sizeBytes > limit) { warnings.push(fileTooLargeMessage(attachment.name, limit)); continue; }
    if (ids.has(attachment.id)) continue;
    if (ids.size >= MAX_ATTACHMENTS) { skipped++; continue; }
    ids.add(attachment.id); attachments.push(attachment);
  }
  if (skipped) warnings.push(`${skipped} shared file${skipped === 1 ? ' was' : 's were'} skipped because this draft reached the attachment limit.`);
  return { status: 'ready' as const, attachments, warnings };
}
export function incomingShareMergedText(existing: string, incoming: string): string {
  return !incoming || existing === incoming || existing.endsWith(`\n\n${incoming}`) ? existing : existing ? `${existing}\n\n${incoming}` : incoming;
}
