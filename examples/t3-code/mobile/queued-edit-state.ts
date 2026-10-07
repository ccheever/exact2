// Pinned365aa87982 queued-run-edit/use-composer-drafts: dedicated mobile content owner.
// @ref llp/1107.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { arr } from './shared/domain';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
export { mobileQueuedEditOrigin, queuedEditRefreshOrigin } from './queued-edit-origin';
import { contextReferences } from './shared/composer-editor-menu';

export interface MobileQueuedEditAttachment {
  id: string; name: string; mimeType: string; sizeBytes: number;
  kind: 'image' | 'file'; uploadId: string; status: 'staged' | 'uploading' | 'ready' | 'error';
}
export interface MobileQueuedEditSession {
  owner: string; draftKey: string; origin: string; environmentId: string; threadId: string; projectId: string;
  generation: number; session: string; revision: number; runId: string; messageId: string; text: string;
  recoveryKey?: string; context?: Obj; attachments: MobileQueuedEditAttachment[]; existingAttachments: Obj[]; saving: boolean;
}
export interface MobileQueuedEditOperation {
  operationId: string; owner: string; revision: number; state: string; environmentId: string; origin: string;
  method: string; payload: Obj; editorRevision: number; error?: unknown;
}
export interface QueuedEditState {
  sessions: Map<string, MobileQueuedEditSession>; active: Map<string, string>;
  recoveries: Map<string, MobileQueuedEditSession>; operationEpoch: number; readSerial: number;
  operations: Map<string, MobileQueuedEditOperation>; notice: string; hydrated: boolean;
}
export const queuedEditClone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const states = new WeakMap<T3Client, QueuedEditState>();
export const queuedEditThreadKey = (environmentId: string, threadId: string) => `${environmentId}:${threadId}`;
export function queuedEditState(client: T3Client): QueuedEditState {
  let state = states.get(client);
  if (!state) { state = { sessions: new Map(), active: new Map(), recoveries: new Map(), operationEpoch: 0, readSerial: 0, operations: new Map(), notice: '', hydrated: false }; states.set(client, state); }
  return state;
}
export function mobileQueuedEditLookup(owner: string, client: T3Client = mobileClient): MobileQueuedEditSession | null {
  return queuedEditState(client).sessions.get(owner) ?? null;
}
export function mobileQueuedEditCurrent(client: T3Client = mobileClient): MobileQueuedEditSession | null {
  const state = queuedEditState(client), owner = state.active.get(queuedEditThreadKey(client.environmentId, client.threadId));
  const session = owner ? state.sessions.get(owner) : undefined;
  return session?.origin === mobileQueuedEditOrigin(client) ? session : null;
}
export const mobileQueuedEditRecovery = (key: string, client: T3Client = mobileClient) => queuedEditState(client).recoveries.get(key) ?? null;
export const mobileQueuedEditNotice = (client: T3Client = mobileClient) => queuedEditState(client).notice;
export function mobileQueuedEditWriteContent(owner: string, content: { text: string; context?: Obj }, client: T3Client = mobileClient): boolean {
  const prior = mobileQueuedEditLookup(owner, client);
  if (!prior || prior.saving || [...queuedEditState(client).operations.values()].some(op => op.owner === owner && ['reserved', 'issued', 'uncertain'].includes(op.state))) return false;
  if (content.text.length > 1_000_000) throw new ClientError('Keep a draft under 1,000,000 characters.');
  const referenced = new Set(contextReferences(content.text).map(ref => ref.id)), records = arr(content.context?.records);
  for (const record of records) if (referenced.has(str(record.contextId)) && record.kind === 'preview-annotation' && str(record.screenshotContextId)) referenced.add(str(record.screenshotContextId));
  const kept = records.filter(record => referenced.has(str(record.contextId)));
  const oldFileIds = new Set(arr(prior.context?.records).filter(record => record.kind === 'file').map(record => str(record.attachmentId)));
  const keptIds = new Set(kept.map(record => str(record.attachmentId)));
  queuedEditState(client).sessions.set(owner, { ...prior, text: content.text,
    attachments: prior.attachments.filter(file => file.kind !== 'file' || !oldFileIds.has(file.id) || keptIds.has(file.id)),
    context: kept.length ? queuedEditClone({ version: 1, records: kept }) : undefined, revision: prior.revision + 1 });
  client.revision++; return true;
}
export function mobileQueuedEditWriteText(owner: string, text: string, client: T3Client = mobileClient): boolean {
  const prior = mobileQueuedEditLookup(owner, client);
  return !!prior && mobileQueuedEditWriteContent(owner, { text, context: prior.context }, client);
}
export function mobileQueuedEditWriteContext(owner: string, context: Obj | undefined, client: T3Client = mobileClient): boolean {
  const prior = mobileQueuedEditLookup(owner, client);
  return !!prior && mobileQueuedEditWriteContent(owner, { text: prior.text, context }, client);
}
export function queuedEditReplaceAttachments(owner: string, attachments: MobileQueuedEditAttachment[], existingAttachments: Obj[], client: T3Client): boolean {
  const prior = mobileQueuedEditLookup(owner, client); if (!prior || prior.saving || [...queuedEditState(client).operations.values()].some(op => op.owner === owner && ['reserved', 'issued', 'uncertain'].includes(op.state))) return false;
  queuedEditState(client).sessions.set(owner, { ...prior, attachments: queuedEditClone(attachments),
    existingAttachments: queuedEditClone(existingAttachments), revision: prior.revision + 1 });
  client.revision++; return true;
}
/** No promise or native handle is retained. Native accepts monotonic content
 * revisions, so overlapping keystrokes cannot let an older save replace a new one. */
export async function mobileQueuedEditPersist(owner: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient): Promise<void> {
  const session = mobileQueuedEditLookup(owner, client);
  if (!session) throw new ClientError('That queued edit has ended.', 'superseded');
  const record = queuedEditRecord(session), revision = session.revision;
  await queuedEditNative(nativeInput, { action: 'cas', owner, revision, record });
}
export function queuedEditRecord(session: MobileQueuedEditSession): Obj {
  const { saving: _saving, ...record } = session;
  return obj(queuedEditClone({ ...record, context: record.context ?? null }));
}
export async function queuedEditNative(nativeInput: Native | null | undefined, input: Obj): Promise<Obj> {
  if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to edit queued messages.');
  const reply = await bridgeReply(letGoAware(mobileNative(nativeInput)), { ...input, op: 'mobileQueuedEdit' });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  return obj(reply.value);
}
export function queuedEditEndMemory(owner: string, client: T3Client) {
  const state = queuedEditState(client), session = state.sessions.get(owner); if (!session) return;
  state.sessions.delete(owner);
  const key = queuedEditThreadKey(session.environmentId, session.threadId);
  if (state.active.get(key) === owner) state.active.delete(key);
  client.revision++;
}
export function queuedEditOperation(raw: unknown): MobileQueuedEditOperation | null {
  const value = obj(raw);
  if (!str(value.operationId) || !str(value.owner) || !Number.isSafeInteger(value.revision) || typeof value.state !== 'string') return null;
  return { operationId: str(value.operationId), owner: str(value.owner), revision: Number(value.revision), state: str(value.state),
    environmentId: str(value.environmentId), origin: str(value.origin), method: str(value.method), payload: obj(value.payload),
    editorRevision: Number(value.editorRevision), error: value.error };
}
