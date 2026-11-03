// Pinned365aa87982 composerAttachmentUploadQueue/useComposerAttachmentUploadState.
// Upload bytes are adopted into the draft before Send, so the strip can report
// the real session state and Send never races an attachment upload.
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { mobileComposerAttachmentWireKindAndMime } from './mobile-attachment-policy';
import { draftFiles, type DraftFile } from './shared/composer-editor-files';

export type ComposerAttachmentUploadState = 'uploading' | 'ready' | 'failed';
export interface ComposerAttachmentUploadSnapshot {
  states: Record<string, ComposerAttachmentUploadState>;
  progress: Record<string, number>;
  errors: Record<string, string>;
  owners: Record<string, { origin: string; environmentId: string }>;
}

interface UploadEntry {
  id: string;
  state: ComposerAttachmentUploadState | 'idle';
  progress: number;
  error: string;
  attempt: number;
}
interface UploadRuntime {
  owner: string;
  draftKey: string;
  origin: string;
  environmentId: string;
  generation: number;
  entries: Map<string, UploadEntry>;
  pending: string[];
  active: number;
  native: Native;
  storage: Files;
}

const runtimes = new WeakMap<T3Client, Map<string, UploadRuntime>>();
const MAX_CONCURRENT = 3;
const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
const MAX_FILE_BYTES = 50 * 1024 * 1024;
const validUploadId = (value: string) => /^[a-z0-9_-]{1,128}$/i.test(value);
const owner = (client: T3Client, draftKey = client.draftKey) => JSON.stringify([
  client.origin, client.environmentId, client.generation, draftKey,
]);
const current = (client: T3Client, runtime: UploadRuntime) => runtimes.get(client)?.get(runtime.draftKey) === runtime
  && client.origin === runtime.origin && client.environmentId === runtime.environmentId
  && client.generation === runtime.generation && client.connection === 'connected';

function wake(client: T3Client, native: Native, generation: number): void {
  client.revision++;
  void native.later({ op: 'r10Wake', topic: 't3.notify', generation }).catch(() => undefined);
}

function entry(runtime: UploadRuntime, id: string): UploadEntry {
  let value = runtime.entries.get(id);
  if (!value) {
    value = { id, state: 'idle', progress: 0, error: '', attempt: 0 };
    runtime.entries.set(id, value);
  }
  return value;
}

function localAttachment(client: T3Client, runtime: UploadRuntime, id: string):
  { kind: 'image'; image: Obj } | { kind: 'file'; file: DraftFile } | null {
  const image = (client.local.snapshotDrafts[runtime.draftKey] ?? []).find(row => str(row.id) === id);
  if (image) return { kind: 'image', image };
  const file = draftFiles(client.local).find(row => row.draftKey === runtime.draftKey && row.id === id);
  return file ? { kind: 'file', file } : null;
}

function inventory(client: T3Client, runtime: UploadRuntime): Array<{ id: string; kind: 'image' | 'file' }> {
  const images = (client.local.snapshotDrafts[runtime.draftKey] ?? []).flatMap(image => {
    const id = str(image.id); return id ? [{ id, kind: 'image' as const }] : [];
  });
  const files = draftFiles(client.local).filter(file => file.draftKey === runtime.draftKey)
    .map(file => ({ id: file.id, kind: 'file' as const }));
  const byId = new Map([...images, ...files].map(value => [value.id, value]));
  const order = arr(obj(obj(client.local).mobileAttachmentOrder)[runtime.draftKey]);
  return [...new Set([...order.map(value => str(value)), ...byId.keys()])].flatMap(id => {
    const value = byId.get(id); return value ? [value] : [];
  });
}

function uploadId(value: { kind: 'image'; image: Obj } | { kind: 'file'; file: DraftFile }): string {
  return value.kind === 'image' ? str(value.image.uploadId) : str(value.file.attachmentId);
}
function ownedUpload(value: { kind: 'image'; image: Obj } | { kind: 'file'; file: DraftFile }, environmentId: string): boolean {
  return !!uploadId(value) && (value.kind === 'image' ? str(value.image.uploadEnvironmentId) : value.file.uploadEnvironmentId) === environmentId;
}

function setState(client: T3Client, runtime: UploadRuntime, value: UploadEntry): void {
  if (!current(client, runtime)) return;
  wake(client, runtime.native, runtime.generation);
}

async function readBytes(runtime: UploadRuntime, attachment: { kind: 'image'; image: Obj } | { kind: 'file'; file: DraftFile }): Promise<string> {
  const id = attachment.kind === 'image' ? str(attachment.image.id) : attachment.file.id;
  const reply = await bridgeReply(runtime.native, { op: attachment.kind === 'image' ? 'snapshotDraftRead' : 'composerAttachRead', id });
  if (!reply.ok) throw new ClientError(reply.error?.message ?? 'The attachment bytes could not be read.', reply.error?.kind);
  const value = obj(reply.value), base64 = str(value.base64), sizeBytes = Number(value.sizeBytes);
  const expected = attachment.kind === 'image' ? Number(attachment.image.sizeBytes) : attachment.file.sizeBytes;
  if (!base64 || !Number.isSafeInteger(sizeBytes) || sizeBytes !== expected) throw new ClientError('The attachment bytes changed. Attach it again.');
  return base64;
}

async function upload(client: T3Client, runtime: UploadRuntime, value: { kind: 'image'; image: Obj } | { kind: 'file'; file: DraftFile }, state: UploadEntry): Promise<void> {
  if (!current(client, runtime)) throw new ClientError('The attachment connection changed.', 'superseded');
  const row = value.kind === 'image' ? value.image : value.file;
  const wire = mobileComposerAttachmentWireKindAndMime({ kind: value.kind, name: str(row.name), mimeType: str(row.mimeType) });
  const sizeBytes = Number(row.sizeBytes);
  const max = wire.type === 'image' ? MAX_IMAGE_BYTES : MAX_FILE_BYTES;
  if (!Number.isSafeInteger(sizeBytes) || sizeBytes < 1 || sizeBytes > max) throw new ClientError(`'${str(row.name)}' is too large to upload.`);
  state.progress = 0.05; setState(client, runtime, state);
  const base64 = await readBytes(runtime, value);
  if (!current(client, runtime)) throw new ClientError('The attachment connection changed.', 'superseded');
  state.progress = 0.2; setState(client, runtime, state);
  let attachmentId = '', adopted = false;
  try {
    const minted = await client.rpc(runtime.native, 'attachments.createUploadUrl', {
      name: str(row.name), mimeType: wire.mimeType, sizeBytes, ...(wire.type === 'file' ? { type: 'file' } : {}),
    }, true);
    attachmentId = str(minted.attachmentId);
    const path = str(minted.relativeUrl);
    if (!validUploadId(attachmentId) || !path.startsWith('/api/attachments/upload/'))
      throw new ClientError('The server returned an invalid attachment upload.');
    state.progress = 0.3; setState(client, runtime, state);
    await client.call(runtime.native, { op: 'uploadAttachment', path, base64, contentType: wire.mimeType,
      expectedOrigin: runtime.origin, expectedEnvironmentId: runtime.environmentId }, runtime.generation, true);
    if (!current(client, runtime)) throw new ClientError('The attachment connection changed.', 'superseded');
    state.progress = 0.9; setState(client, runtime, state);
    const live = localAttachment(client, runtime, state.id);
    if (!live) { runtime.entries.delete(state.id); setState(client, runtime, state); return; }
    if (live.kind !== value.kind) throw new ClientError('The attachment changed while it was uploading.', 'superseded');
    if (uploadId(live)) {
      if (!ownedUpload(live, runtime.environmentId)) throw new ClientError('The saved attachment does not belong to this environment.');
      await client.persist(runtime.storage);
      state.progress = 1; state.error = ''; state.state = 'ready'; setState(client, runtime, state); return;
    }
    if (live.kind === 'image') {
      live.image.uploadId = attachmentId; live.image.uploadEnvironmentId = runtime.environmentId;
    } else {
      live.file.attachmentId = attachmentId; live.file.uploadEnvironmentId = runtime.environmentId; live.file.status = 'ready';
    }
    adopted = true;
    await client.persist(runtime.storage);
    state.progress = 1; state.error = ''; state.state = 'ready'; setState(client, runtime, state);
  } catch (error) {
    // A failed POST leaves a newly minted server asset otherwise orphaned. It
    // is safe to delete only while this exact environment session is current.
    if (validUploadId(attachmentId) && !adopted && current(client, runtime))
      await client.rpc(runtime.native, 'attachments.delete', { attachmentId }, true).catch(() => undefined);
    throw error;
  }
}

async function run(client: T3Client, runtime: UploadRuntime, id: string): Promise<void> {
  const state = entry(runtime, id); state.state = 'uploading'; state.error = ''; state.progress = 0; state.attempt++;
  setState(client, runtime, state);
  try {
    const attachment = localAttachment(client, runtime, id);
    if (!attachment) throw new ClientError('That attachment was removed.');
    if (ownedUpload(attachment, runtime.environmentId)) { state.progress = 1; state.state = 'ready'; setState(client, runtime, state); return; }
    if (uploadId(attachment)) throw new ClientError('The saved attachment does not belong to this environment.');
    await upload(client, runtime, attachment, state);
  } catch (error) {
    if (error instanceof ClientError && error.kind === 'superseded') return;
    state.state = 'failed'; state.progress = 0; state.error = error instanceof Error ? error.message : 'The attachment could not be uploaded.';
    setState(client, runtime, state);
  }
}

function pump(client: T3Client, runtime: UploadRuntime): void {
  while (current(client, runtime) && runtime.active < MAX_CONCURRENT && runtime.pending.length) {
    const id = runtime.pending.shift()!; runtime.active++;
    void run(client, runtime, id).finally(() => { runtime.active--; pump(client, runtime); });
  }
}

function runtimeFor(client: T3Client, native: Native, storage: Files, draftKey = client.draftKey): UploadRuntime {
  const key = owner(client, draftKey);
  let all = runtimes.get(client);
  if (!all) { all = new Map(); runtimes.set(client, all); }
  let runtime = all.get(draftKey);
  if (!runtime || runtime.owner !== key) {
    runtime = { owner: key, draftKey, origin: client.origin, environmentId: client.environmentId,
      generation: client.generation, entries: new Map(), pending: [], active: 0, native, storage };
    all.set(draftKey, runtime);
    native.watch('t3.notify');
  } else { runtime.native = native; runtime.storage = storage; }
  return runtime;
}

/** Starts missing uploads for one draft. The queue is session-lived and may
 * continue while the user navigates to another route in the same environment. */
export function mobileComposerAttachmentUploadsStart(client: T3Client, native: Native | null | undefined,
  storage: Files, draftKey = client.draftKey, onlyIds: readonly string[] = []): void {
  if (!native?.available || client.connection !== 'connected' || !client.scopes.includes('orchestration:operate')
    || obj(obj(client.config.environment).capabilities).attachmentUploads !== true) return;
  const runtime = runtimeFor(client, native, storage, draftKey), wanted = onlyIds.length ? new Set(onlyIds) : null;
  for (const item of inventory(client, runtime)) {
    if (wanted && !wanted.has(item.id)) continue;
    const attachment = localAttachment(client, runtime, item.id), state = entry(runtime, item.id);
    if (!attachment) continue;
    if (ownedUpload(attachment, runtime.environmentId)) { state.state = 'ready'; state.progress = 1; state.error = ''; continue; }
    // A legacy or foreign remote ID has no proof that this session owns it.
    // Keep it idle so the send admission fails closed instead of inventing provenance.
    if (uploadId(attachment)) continue;
    if (state.state === 'uploading' || state.state === 'ready') continue;
    if (state.state === 'failed' && !wanted) continue;
    if (!runtime.pending.includes(item.id)) runtime.pending.push(item.id);
  }
  pump(client, runtime);
  if (runtime.pending.length || runtime.active) wake(client, native, runtime.generation);
}

/** Explicit Retry action for a failed tile. */
export function mobileComposerAttachmentUploadRetry(client: T3Client, native: Native | null | undefined,
  storage: Files, id: string): boolean {
  // The public attachment action performs the captured target check before
  // reaching this queue operation. Keep this module free of composer-target's
  // route import so the upload queue cannot form a target/attachment cycle.
  const runtime = runtimes.get(client)?.get(client.draftKey);
  if (!runtime || runtime.draftKey !== client.draftKey || runtime.owner !== owner(client, runtime.draftKey)) return false;
  const state = runtime.entries.get(id);
  if (!state || state.state !== 'failed') return false;
  runtime.native = native ?? runtime.native; runtime.storage = storage; state.error = ''; state.progress = 0;
  if (!runtime.pending.includes(id)) runtime.pending.push(id);
  pump(client, runtime); wake(client, runtime.native, runtime.generation); return true;
}

/** Read the current draft's upload states for strip rendering and Send admission. */
export function mobileComposerAttachmentUploadSnapshot(client: T3Client, draftKey = client.draftKey): ComposerAttachmentUploadSnapshot {
  const runtime = runtimes.get(client)?.get(draftKey), result: ComposerAttachmentUploadSnapshot = { states: {}, progress: {}, errors: {}, owners: {} };
  if (runtime && runtime.draftKey === draftKey && runtime.owner === owner(client, draftKey)) {
    for (const value of runtime.entries.values()) {
      if (value.state !== 'idle') {
        result.states[value.id] = value.state; result.progress[value.id] = value.progress; if (value.error) result.errors[value.id] = value.error;
      }
    }
  }
  const images = client.local.snapshotDrafts[draftKey] ?? [], files = draftFiles(client.local).filter(file => file.draftKey === draftKey);
  for (const image of images) if (str(image.uploadId) && str(image.uploadEnvironmentId) === client.environmentId) {
    result.states[str(image.id)] = 'ready'; result.progress[str(image.id)] = 1;
    result.owners[str(image.id)] = { origin: client.origin, environmentId: client.environmentId };
  }
  for (const file of files) if (file.attachmentId && file.uploadEnvironmentId === client.environmentId) {
    result.states[file.id] = 'ready'; result.progress[file.id] = 1;
    result.owners[file.id] = { origin: client.origin, environmentId: client.environmentId };
  }
  return result;
}
