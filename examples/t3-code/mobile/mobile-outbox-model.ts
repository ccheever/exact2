// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Pure policy port of pinned 365aa87982 apps/mobile/src/state/thread-outbox-model.ts.
import type { MobileQueuedEditAttachment } from './queued-edit-memory';
import type { DraftFile } from './shared/composer-editor-files';
import { MAX_FILE_BYTES } from './shared/composer-editor-files';
import type { Obj } from './shared/domain';

export type MobileOutboxAttachment = MobileQueuedEditAttachment & { uploadEnvironmentId?: string } & (
  | { kind: 'image'; source?: Obj }
  | ({ kind: 'file' } & Pick<DraftFile, 'contextId' | 'source' | 'videoWidth' | 'videoHeight'>)
);
export interface MobileOutboxModelSelection {
  instanceId: string; model: string; options?: { id: string; value: string | boolean }[];
}
export type MobileOutboxRuntimeMode = 'approval-required' | 'auto-accept-edits' | 'auto' | 'full-access';
export type MobileOutboxInteractionMode = 'default' | 'plan';
export interface MobileOutboxCreation {
  projectId: string; projectTitle?: string; projectCwd?: string;
  workspaceMode: 'local' | 'worktree'; branch: string | null; worktreePath: string | null;
  startFromOrigin?: boolean;
}
export interface MobileOutboxRecord {
  schemaVersion: 1; origin: string; environmentId: string; threadId: string; messageId: string; commandId: string;
  text: string; context?: Obj; attachments: MobileOutboxAttachment[];
  modelSelection?: MobileOutboxModelSelection; runtimeMode?: MobileOutboxRuntimeMode;
  interactionMode?: MobileOutboxInteractionMode; dispatchMode?: 'auto' | 'queue' | 'steer' | 'restart';
  creation?: MobileOutboxCreation; createdAt: string;
}
export type MobileOutboxDecode = { ok: true; record: MobileOutboxRecord } |
  { ok: false; ownership: 'unknown'; raw: unknown; error: string };
const object = (value: unknown): value is Obj => typeof value === 'object' && value !== null && !Array.isArray(value);
const text = (value: unknown): value is string => typeof value === 'string';
const nonempty = (value: unknown): value is string => text(value) && value.length > 0 && value.trim() === value;
const uuid = (value: unknown) => text(value) && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
const optional = (value: unknown, check: (value: unknown) => boolean) => value === undefined || check(value);
const oneOf = (value: unknown, values: readonly string[]) => text(value) && values.includes(value);
const runtimeModes = ['approval-required', 'auto-accept-edits', 'auto', 'full-access'];
const positive = (value: unknown) => typeof value === 'number' && Number.isFinite(value) && value > 0;
function json(value: unknown, parents = new Set<object>()): boolean {
  if (value === null || text(value) || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (!object(value) && !Array.isArray(value)) return false;
  if (parents.has(value)) return false;
  parents.add(value);
  const children = Array.isArray(value) ? [...value] : Object.values(value).filter(child => child !== undefined);
  const valid = children.every(child => json(child, parents));
  parents.delete(value); return valid;
}
export function mobileOutboxCanonicalOrigin(value: unknown): value is string {
  if (!text(value)) return false;
  try { const url = new URL(value); return ['http:', 'https:'].includes(url.protocol) && url.origin === value; }
  catch { return false; }
}
function attachment(value: unknown): value is MobileOutboxAttachment {
  if (!object(value) || !uuid(value.id) || !nonempty(value.name) || !nonempty(value.mimeType) ||
      typeof value.sizeBytes !== 'number' || !Number.isSafeInteger(value.sizeBytes) || value.sizeBytes < 0 ||
      !text(value.uploadId) || !optional(value.uploadEnvironmentId, nonempty) || !oneOf(value.status, ['staged', 'uploading', 'ready', 'error'])) return false;
  if (value.status === 'ready' && !nonempty(value.uploadId)) return false;
  if (value.kind === 'image') return optional(value.source, source => object(source) && json(source));
  return value.kind === 'file' && nonempty(value.contextId) && nonempty(value.source) &&
    optional(value.videoWidth, positive) && optional(value.videoHeight, positive);
}
function model(value: unknown): boolean {
  return object(value) && nonempty(value.instanceId) && nonempty(value.model) && optional(value.options, options =>
    Array.isArray(options) && options.every(option => object(option) && nonempty(option.id) &&
      (typeof option.value === 'boolean' || nonempty(option.value))));
}
function context(value: unknown): boolean {
  if (!object(value) || value.version !== 1 || !Array.isArray(value.records) || !json(value)) return false;
  const ids = new Set<string>();
  return value.records.every(record => {
    if (!object(record) || record.version !== 1 || !nonempty(record.contextId) ||
        !/^[a-z0-9_-]{1,128}$/i.test(record.contextId) || !text(record.label) || record.label.length > 200 ||
        !text(record.kind) || !/^[a-z][a-z0-9-]{0,39}$/.test(record.kind) || ids.has(record.contextId)) return false;
    ids.add(record.contextId); return true;
  });
}
function creation(value: unknown): boolean {
  return object(value) && nonempty(value.projectId) && optional(value.projectTitle, text) &&
    optional(value.projectCwd, text) && oneOf(value.workspaceMode, ['local', 'worktree']) &&
    (value.branch === null || text(value.branch)) && (value.worktreePath === null || text(value.worktreePath)) &&
    optional(value.startFromOrigin, flag => typeof flag === 'boolean');
}
/** This validates storage and ownership, not every provider's wire context union.
 * Unknown context kinds and their complete JSON payload survive a round trip.
 * Invalid/future records are retained by the caller and block byte collection. */
export function mobileOutboxDecode(value: unknown): MobileOutboxDecode {
  const invalid = (error: string): MobileOutboxDecode => ({ ok: false, ownership: 'unknown', raw: value, error });
  if (!object(value) || value.schemaVersion !== 1) return invalid('Unsupported or invalid outbox record.');
  if (!mobileOutboxCanonicalOrigin(value.origin) || !['environmentId', 'threadId', 'messageId', 'commandId'].every(key => nonempty(value[key])))
    return invalid('Invalid outbox owner or command identity.');
  if (!text(value.text) || !Array.isArray(value.attachments) || !value.attachments.every(attachment) ||
      new Set(value.attachments.map(file => object(file) && text(file.id) ? file.id.toLowerCase() : null)).size !== value.attachments.length ||
      value.attachments.some(file => !object(file) || file.uploadId !== '' && file.uploadEnvironmentId !== value.environmentId)) return invalid('Invalid outbox content or attachments.');
  if (!optional(value.context, context) || !optional(value.modelSelection, model) || !optional(value.creation, creation) ||
      !optional(value.runtimeMode, mode => oneOf(mode, runtimeModes)) ||
      !optional(value.interactionMode, mode => oneOf(mode, ['default', 'plan'])) ||
      !optional(value.dispatchMode, mode => oneOf(mode, ['auto', 'queue', 'steer', 'restart']))) return invalid('Invalid outbox settings or context.');
  if (!text(value.createdAt) || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(value.createdAt) ||
      !Number.isFinite(Date.parse(value.createdAt)) || new Date(value.createdAt).toISOString() !== value.createdAt)
    return invalid('Invalid outbox creation time.');
  // Undefined optional object properties are normal; JSON removes them at every depth.
  try {
    const clean = Object.fromEntries(Object.entries(value).filter(([, field]) => field !== undefined));
    if (!json(clean)) return invalid('Outbox record is not JSON data.');
    return { ok: true, record: JSON.parse(JSON.stringify(clean)) as MobileOutboxRecord };
  } catch { return invalid('Outbox record is not JSON data.'); }
}
export function mobileOutboxEncode(record: MobileOutboxRecord): MobileOutboxRecord {
  const decoded = mobileOutboxDecode(record);
  if (!decoded.ok) throw new Error(decoded.error);
  return decoded.record;
}

export function mobileOutboxGroup(messages: readonly MobileOutboxRecord[]): Record<string, MobileOutboxRecord[]> {
  const deduplicated = new Map<string, MobileOutboxRecord>();
  for (const message of messages) deduplicated.set(message.messageId, message);
  const grouped: Record<string, MobileOutboxRecord[]> = Object.create(null);
  for (const message of deduplicated.values()) (grouped[`${message.environmentId}:${message.threadId}`] ??= []).push(message);
  for (const queue of Object.values(grouped)) queue.sort((left, right) => left.createdAt.localeCompare(right.createdAt));
  return grouped;
}
export const mobileOutboxFlatten = (queues: Record<string, readonly MobileOutboxRecord[]>): readonly MobileOutboxRecord[] => Object.values(queues).flat();
export const mobileOutboxRetryDelay = (attempt: number): number => Math.min(1_000 * 2 ** Math.max(0, attempt - 1), 16_000);
export type MobileOutboxDeliveryAction = 'wait' | 'remove' | 'send';
export function mobileOutboxDeliveryAction(input: {
  isCreation: boolean; threadExists: boolean; shellStatus: string; environmentConnected: boolean; threadBusy: boolean;
}): MobileOutboxDeliveryAction {
  if (input.isCreation) return input.threadExists ? 'remove' : input.environmentConnected && input.shellStatus === 'live' ? 'send' : 'wait';
  if (!input.threadExists) return input.shellStatus === 'live' ? 'remove' : 'wait';
  return input.environmentConnected ? 'send' : 'wait';
}
export type MobileOutboxDispatchStep = { step: 'wait' | 'remove' | 'retry' | 'send' } | { step: 'restore'; reason: string };
export function mobileOutboxDispatchStep(input: {
  deliveryAction: MobileOutboxDeliveryAction; fileAttachments: readonly { name: string; sizeBytes: number }[];
  serverConfig: { maxFileUploadBytes: number | undefined } | null;
}): MobileOutboxDispatchStep {
  if (input.deliveryAction !== 'send') return { step: input.deliveryAction };
  if (input.serverConfig === null) return { step: 'retry' };
  if (input.fileAttachments.length === 0) return { step: 'send' };
  const max = input.serverConfig.maxFileUploadBytes;
  if (max === undefined) return { step: 'restore', reason: 'This server does not support file attachments.' };
  const effective = Math.min(max, MAX_FILE_BYTES), oversized = input.fileAttachments.find(file => file.sizeBytes > effective);
  const label = effective >= 1048576 && effective % 1048576 === 0 ? `${effective / 1048576} MB` :
    effective >= 1024 && effective % 1024 === 0 ? `${effective / 1024} KB` : `${effective} ${effective === 1 ? 'byte' : 'bytes'}`;
  return oversized ? { step: 'restore', reason: `'${oversized.name}' exceeds the ${label} attachment limit.` } : { step: 'send' };
}
export function mobileOutboxCreationSendable(message: MobileOutboxRecord): boolean {
  return !!message.creation && message.text.trim().length > 0 && message.modelSelection !== undefined &&
    (message.creation.workspaceMode !== 'worktree' || Boolean(message.creation.branch));
}
export interface MobileOutboxSettings {
  modelSelection: MobileOutboxModelSelection; runtimeMode: MobileOutboxRuntimeMode; interactionMode: MobileOutboxInteractionMode;
}
export function mobileOutboxResolveSettings(message: MobileOutboxRecord, thread: MobileOutboxSettings,
  providers: readonly { instanceId: string; showInteractionModeToggle?: boolean }[] = []): MobileOutboxSettings {
  const modelSelection = message.modelSelection ?? thread.modelSelection;
  return { modelSelection, runtimeMode: message.runtimeMode ?? thread.runtimeMode,
    interactionMode: providers.find(provider => provider.instanceId === modelSelection.instanceId)?.showInteractionModeToggle === false ?
      'default' : message.interactionMode ?? thread.interactionMode ?? 'default' };
}
export function mobileOutboxModelsEqual(left: MobileOutboxModelSelection, right: MobileOutboxModelSelection): boolean {
  return left.instanceId === right.instanceId && left.model === right.model && JSON.stringify(left.options ?? null) === JSON.stringify(right.options ?? null);
}
const networkHint = 'Your DNS or firewall may be blocking T3 Connect. Try another network, such as a phone hotspot.';
const transportPatterns = [/\bSocketCloseError\b/i, /\bSocketOpenError\b/i, /\bSocket is not connected\b/i,
  /Unable to connect to the T3 server WebSocket\./i,
  new RegExp(`\\b(?:is not connected|disconnected|stopped responding|could not establish a WebSocket connection)\\.(?: ${networkHint.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})?$`, 'i'),
  /\bClientProtocolError\b/i, /\bRpcClientError\b/i, /\bping timeout\b/i];
/** Source error classification only. Native outcome certainty is a separate receipt;
 * an unknown issued outcome must not be fabricated as a definitive source error. */
export function mobileOutboxShouldRetry(error: unknown): boolean {
  if (object(error)) {
    if (['OrchestrationDispatchCommandError', 'EnvironmentAuthorizationError'].includes(String(error._tag))) return false;
    if (['ConnectionTransientError', 'RpcClientError', 'EnvironmentRpcUnavailableError', 'EnvironmentNotRegisteredError'].includes(String(error._tag))) return true;
  }
  const message = error instanceof Error ? error.message : object(error) ? error.message : error;
  return text(message) && transportPatterns.some(pattern => pattern.test(message.trim()));
}
export function mobileOutboxFailureAction(input: { stage: 'settings-sync' | 'start-turn'; error: unknown; interrupted: boolean }): 'retry' | 'restore' {
  return input.stage === 'settings-sync' || input.interrupted || mobileOutboxShouldRetry(input.error) ? 'retry' : 'restore';
}
