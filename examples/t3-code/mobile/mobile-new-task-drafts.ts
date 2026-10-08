import { mobileDraftAttachmentIds, mobileDraftAttachmentForget } from './draft-attachment-order';
// App-owned foreground drafts; source365aa87982 use-composer-drafts/new-task-flow-provider.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
import { draftFiles, setDraftFiles, type DraftFile } from './shared/composer-editor-files';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { draftContext, type DraftContext } from './shared/composer-controls-branch';

export interface MobileNewTaskChoices {
  providerId?: string; modelId?: string; modelOptions?: Obj[]; runtimeMode?: string; interactionMode?: string;
}
export interface MobileNewTaskBranchChoice extends DraftContext { kind: 'automatic' | 'explicit' }
export interface MobileNewTaskDraft {
  key: string; environmentId: string; projectId: string; origin: string; createdAt: string;
  revision: number; choices: MobileNewTaskChoices | null; branchChoice?: MobileNewTaskBranchChoice;
}
export interface MobileNewTaskDraftStore {
  version: 1; records: Record<string, MobileNewTaskDraft>; receipts: Record<string, unknown>;
  claims: Record<string, string>; fileReleases: string[];
}
const bindings = new WeakMap<T3Client, { key: string; owner: string }>();
export const mobileNewTaskDraftIsKey = (key: string) => /^new-task:[\w-]{1,128}$/.test(key);
export const mobileNewTaskDraftClone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
export function mobileNewTaskDraftStore(client: T3Client): MobileNewTaskDraftStore {
  const local = client.local as typeof client.local & { mobileNewTaskDrafts?: MobileNewTaskDraftStore };
  return local.mobileNewTaskDrafts ??= { version: 1, records: {}, receipts: {}, claims: {}, fileReleases: [] };
}
function validStamp(value: { environmentId: string; projectId: string; origin: string }): boolean {
  return !!value.environmentId && !!value.projectId && /^https?:\/\//.test(value.origin);
}
export function mobileNewTaskDraftHydrate(client: T3Client, saved: Obj): void {
  const raw = obj(saved.mobileNewTaskDrafts), records: Record<string, MobileNewTaskDraft> = {};
  const dictionary = (value: unknown) => !!value && typeof value === 'object' && !Array.isArray(value);
  if (raw.version !== 1 || !dictionary(raw.records) || !dictionary(raw.receipts) || !dictionary(raw.claims) || !Array.isArray(raw.fileReleases)) {
    if (saved.mobileNewTaskDrafts !== undefined || Object.keys(obj(saved.drafts)).some(mobileNewTaskDraftIsKey)) {
      const held = mobileNewTaskDraftStore(client);
      for (const [environmentId, value] of Object.entries(obj(saved.pending))) {
        const pending = obj(value);
        if (pending.method === 'orchestration.launchThread') held.claims[JSON.stringify([environmentId, str(obj(pending.payload).commandId)])] = 'new-task:invalid';
      }
    }
    return;
  }
  for (const [key, entry] of Object.entries(obj(raw.records))) {
    const record = obj(entry);
    if (!mobileNewTaskDraftIsKey(key) || record.key !== key || !validStamp({ environmentId: str(record.environmentId), projectId: str(record.projectId), origin: str(record.origin) })
      || !Number.isSafeInteger(record.revision) || Number(record.revision) < 0 || !Number.isFinite(Date.parse(str(record.createdAt)))) continue;
    const choices = obj(record.choices), branchChoice = decodeBranchChoice(record.branchChoice);
    records[key] = { key, environmentId: str(record.environmentId), projectId: str(record.projectId), origin: str(record.origin),
      createdAt: str(record.createdAt), revision: Number(record.revision), choices: decodeChoices(choices),
      ...(branchChoice ? { branchChoice } : {}) };
  }
  // Keep invalid receipt values visible to admission. Silently dropping one could
  // route its pending launch through the ordinary project-slot cleanup.
  const store: MobileNewTaskDraftStore = { version: 1, records, receipts: mobileNewTaskDraftClone(obj(raw.receipts)),
    claims: Object.fromEntries(Object.entries(obj(raw.claims)).filter((entry): entry is [string, string] => typeof entry[1] === 'string')),
    fileReleases: Array.isArray(raw.fileReleases) ? raw.fileReleases.filter((id): id is string => typeof id === 'string' && !!id) : [] };
  Object.assign(client.local, { mobileNewTaskDrafts: store }); bindings.delete(client);
}
/** Persist only explicit picks. Never assign arbitrary decoded keys onto the client. */
function decodeChoices(value: Obj): MobileNewTaskChoices | null {
  const next: MobileNewTaskChoices = {};
  if ('providerId' in value || 'modelId' in value || 'modelOptions' in value) {
    if (typeof value.providerId !== 'string' || typeof value.modelId !== 'string' || !Array.isArray(value.modelOptions)) return null;
    next.providerId = value.providerId; next.modelId = value.modelId; next.modelOptions = mobileNewTaskDraftClone(value.modelOptions) as Obj[];
  }
  for (const key of ['runtimeMode', 'interactionMode'] as const) {
    if (!(key in value)) continue;
    if (typeof value[key] !== 'string') return null;
    next[key] = value[key];
  }
  return next;
}
function decodeBranchChoice(value: unknown): MobileNewTaskBranchChoice | null {
  const choice = obj(value);
  return (choice.kind === 'automatic' || choice.kind === 'explicit') &&
    (choice.envMode === 'local' || choice.envMode === 'worktree') && typeof choice.branch === 'string' &&
    typeof choice.worktreePath === 'string'
    ? { kind: choice.kind, envMode: choice.envMode, branch: choice.branch, worktreePath: choice.worktreePath } : null;
}
/** The same displayed branch may be an automatic checkout or an explicit pick.
 * Pinned projectThreadCreationValidation preserves only the latter when queued. */
export function mobileNewTaskDraftNoteBranch(client: T3Client, kind: MobileNewTaskBranchChoice['kind']): void {
  const current = mobileNewTaskDraftCurrent(client);
  if (!current) return;
  const record = mobileNewTaskDraftStore(client).records[current.key];
  const next: MobileNewTaskBranchChoice = { ...draftContext(client), kind };
  if (JSON.stringify(record.branchChoice) !== JSON.stringify(next)) {
    record.branchChoice = next; record.revision++; client.revision++;
  }
}
/** Undefined means the branch's origin is unknown. Never guess from its name. */
export function mobileNewTaskDraftSelectedBranch(draft: { branchChoice?: MobileNewTaskBranchChoice; workspace: DraftContext | null }): string | null | undefined {
  const context = draft.workspace, choice = draft.branchChoice;
  if (!context?.branch) return null;
  if (!choice || choice.branch !== context.branch || choice.envMode !== context.envMode || choice.worktreePath !== context.worktreePath) return undefined;
  return choice.kind === 'explicit' || choice.envMode === 'worktree' ? choice.branch : null;
}
export function mobileNewTaskDraftChoicesUpdate(client: T3Client, key: string, patch: MobileNewTaskChoices): boolean {
  const record = mobileNewTaskDraftStore(client).records[key];
  if (!record || mobileNewTaskDraftCurrent(client)?.key !== key) return false;
  const choices = decodeChoices({ ...record.choices, ...patch });
  if (!choices) throw new ClientError('The draft settings are invalid.');
  if (JSON.stringify(choices) !== JSON.stringify(record.choices)) { record.choices = choices; client.revision++; }
  return true;
}
export function mobileNewTaskDraftChoicesCapture(client: T3Client): MobileNewTaskChoices {
  return { providerId: client.providerId, modelId: client.modelId, modelOptions: mobileNewTaskDraftClone(client.modelOptions),
    runtimeMode: client.runtimeMode, interactionMode: client.interactionMode };
}
export function mobileNewTaskDraftChoicesRestore(client: T3Client, key: string): boolean {
  const record = mobileNewTaskDraftCurrent(client);
  if (!record || record.key !== key || !record.choices) return false;
  const choices = decodeChoices(record.choices as Obj);
  if (!choices) return false;
  Object.assign(client, choices); return true;
}
export function mobileNewTaskDraftLookup(client: T3Client, key: string): MobileNewTaskDraft | null {
  const record = mobileNewTaskDraftStore(client).records[key]; return record ? mobileNewTaskDraftClone(record) : null;
}
export const mobileNewTaskDraftExists = (client: T3Client, key: string) => !!mobileNewTaskDraftStore(client).records[key];
export function mobileNewTaskDraftHasContent(client: T3Client, key: string): boolean {
  return !!client.local.drafts[key]?.trim() || !!client.local.snapshotDrafts[key]?.length || draftFiles(client.local).some(file => file.draftKey === key);
}
export function mobileNewTaskDraftPresentation(client: T3Client, key: string) {
  const record = mobileNewTaskDraftLookup(client, key);
  return record ? { ...record, attachmentIds: mobileDraftAttachmentIds(client, key), text: client.local.drafts[key] ?? '', images: mobileNewTaskDraftClone(client.local.snapshotDrafts[key] ?? []),
    files: mobileNewTaskDraftClone(draftFiles(client.local).filter(file => file.draftKey === key)),
    workspace: mobileNewTaskDraftClone(client.local.composerControls.contexts[key] ?? null) } : null;
}
export function mobileNewTaskDraftList(client: T3Client): MobileNewTaskDraft[] {
  return Object.values(mobileNewTaskDraftStore(client).records).filter(record => mobileNewTaskDraftHasContent(client, record.key))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt) || a.key.localeCompare(b.key)).map(mobileNewTaskDraftClone);
}
export function mobileNewTaskDraftCreate(client: T3Client, input: { id: string; environmentId: string; projectId: string; origin: string; createdAt: string; choices?: MobileNewTaskChoices }): MobileNewTaskDraft {
  const key = `new-task:${input.id}`, store = mobileNewTaskDraftStore(client);
  if (!client.preferencesLoaded || !mobileNewTaskDraftIsKey(key) || !validStamp(input) || !Number.isFinite(Date.parse(input.createdAt))) throw new ClientError('The new draft identity is not ready.');
  if (store.records[key] || key in client.local.drafts || key in client.local.snapshotDrafts || draftFiles(client.local).some(file => file.draftKey === key)) throw new ClientError('That draft identity already exists.');
  const record: MobileNewTaskDraft = { key, environmentId: input.environmentId, projectId: input.projectId, origin: input.origin,
    createdAt: input.createdAt, revision: 0, choices: input.choices ? decodeChoices(input.choices as Obj) : null };
  store.records[key] = record; client.revision++; return mobileNewTaskDraftClone(record);
}
export function mobileNewTaskDraftBind(client: T3Client, key: string, owner: string): boolean {
  const record = mobileNewTaskDraftStore(client).records[key];
  if (!owner || !record || client.threadId || !matches(client, record)) return false;
  bindings.set(client, { key, owner }); client.revision++; return true;
}
export function mobileNewTaskDraftUnbind(client: T3Client, owner?: string): void {
  if (owner !== undefined && bindings.get(client)?.owner !== owner) return;
  if (bindings.delete(client)) client.revision++;
}
/** A stale binding still names its own key; it must never fall back to a project slot. */
export function mobileNewTaskDraftBoundKey(client: T3Client): string { return client.threadId ? '' : bindings.get(client)?.key ?? ''; }
function matches(client: T3Client, record: MobileNewTaskDraft) {
  return client.environmentId === record.environmentId && client.projectId === record.projectId && mobileQueuedEditOrigin(client) === record.origin;
}
export function mobileNewTaskDraftCurrent(client: T3Client): MobileNewTaskDraft | null {
  const record = mobileNewTaskDraftStore(client).records[mobileNewTaskDraftBoundKey(client)];
  return record && matches(client, record) ? mobileNewTaskDraftClone(record) : null;
}
export function mobileNewTaskDraftChanged(client: T3Client, key: string, captureChoices = false): boolean {
  const record = mobileNewTaskDraftStore(client).records[key]; if (!record) return false;
  record.revision++;
  if (captureChoices && mobileNewTaskDraftCurrent(client)?.key === key) record.choices = mobileNewTaskDraftChoicesCapture(client);
  client.revision++; return true;
}
function unlocked(client: T3Client, key: string) {
  const store = mobileNewTaskDraftStore(client);
  if (Object.values(store.claims).includes(key) || Object.values(store.receipts).some(raw => obj(raw).key === key)) throw new ClientError('Resolve the captured launch before changing this draft.');
}
export function mobileNewTaskDraftRetarget(client: T3Client, key: string, target: { environmentId: string; projectId: string; origin: string }): boolean {
  const record = mobileNewTaskDraftStore(client).records[key]; if (!record || !validStamp(target)) return false;
  unlocked(client, key);
  const crossing = record.environmentId !== target.environmentId || record.origin !== target.origin;
  if (record.environmentId === target.environmentId && record.projectId === target.projectId && record.origin === target.origin) return true;
  if (crossing && draftFiles(client.local).some(file => file.draftKey === key && file.source !== 'attached'))
    throw new ClientError('Save these local file bytes before moving the draft to another environment.');
  Object.assign(record, target); delete record.branchChoice; record.revision++;
  delete client.local.composerControls.contexts[key]; delete client.local.composerControls.draftThreads?.[key];
  if (crossing) {
    client.local.snapshotDrafts[key] = (client.local.snapshotDrafts[key] ?? []).map(image => { const next = { ...image }; delete next.uploadId; return next; });
    setDraftFiles(client.local, draftFiles(client.local).map(file => file.draftKey === key
      ? { ...file, environmentId: target.environmentId, attachmentId: '', status: 'staged' as const } : file));
  }
  client.revision++; return true;
}
export function mobileNewTaskDraftRemoveMetadata(client: T3Client, key: string): void {
  mobileDraftAttachmentForget(client, key);
  delete mobileNewTaskDraftStore(client).records[key]; delete client.local.composerControls.contexts[key];
  delete client.local.composerControls.draftThreads?.[key]; delete client.local.composerControls.staged[key];
  delete client.local.composerControls.balance?.[key];
}
export function mobileNewTaskDraftQueueFiles(client: T3Client, files: DraftFile[]): void {
  const releases = mobileNewTaskDraftStore(client).fileReleases;
  for (const file of files) if (file.source === 'attached' && !releases.includes(file.id)) releases.push(file.id);
}
export function mobileNewTaskDraftDiscard(client: T3Client, key: string): boolean {
  if (!mobileNewTaskDraftExists(client, key)) return false; unlocked(client, key);
  for (const image of client.local.snapshotDrafts[key] ?? []) if (str(image.id)) client.local.snapshotReleases.push(str(image.id));
  const files = draftFiles(client.local).filter(file => file.draftKey === key); mobileNewTaskDraftQueueFiles(client, files);
  setDraftFiles(client.local, draftFiles(client.local).filter(file => file.draftKey !== key));
  delete client.local.drafts[key]; delete client.local.snapshotDrafts[key]; mobileNewTaskDraftRemoveMetadata(client, key);
  client.revision++; return true;
}
/** Serialize only contentful drafts or receipt-owned drafts; leave empty live editors in memory. */
export function mobileNewTaskDraftPersisted(client: T3Client): MobileNewTaskDraftStore {
  const store = mobileNewTaskDraftStore(client), claimed = new Set(Object.values(store.claims));
  return { ...mobileNewTaskDraftClone(store), records: Object.fromEntries(Object.entries(store.records)
    .filter(([key]) => claimed.has(key) || mobileNewTaskDraftHasContent(client, key)).map(([key, record]) => [key, mobileNewTaskDraftClone(record)])) };
}
