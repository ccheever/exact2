// attachmentUploadQueue.ts and attachmentUploadState.ts at T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3): a draft image
// uploads as soon as it is attached, at most three at once, and its tile draws the upload's percentage while the person
// types. Each upload runs in one of the root's three upload resources (composerUpload0 to 2,
// composer-controls-members.contract), a stream whose fetch tells the bytes it has sent (`exactUploadProgress`, LLP
// 1016.000 D10), so no answer waits on it. The table here says which image each resource carries. An image keeps its
// slot until its upload ends, since new arguments let a running upload go; the server's attachment id then lands in
// the draft (`uploadId`), which the send names, and the slot is free again.
import type { T3Client } from '../client/client';
import { ClientError, type Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { draftUpload } from '../snapshot/snapshot-upload';
import { path as onOrigin } from '../transport/transport-wire';

/** MAX_UPLOADS_PER_ENVIRONMENT: the root's upload resources. */
export const UPLOAD_SLOTS = 3;
const UPLOAD_TIMEOUT_MS = 5 * 60_000;
/** The upload route's limit for an image (transport-session.ts uploadAttachment). */
const UPLOAD_LIMIT = 10 * 1024 * 1024;

/** One slot's answer: the image it carries and how its upload goes ('' while it carries none). */
export type UploadValue = { slot: number; id: string; attachmentId: string; status: '' | 'uploading' | 'ready' | 'failed'; progress: number; reason: string };
/** An image's upload for its tile: waiting for a slot, in one (whose answer has its percentage), or failed with the reason. */
export type ImageUpload = { id: string; attachmentId: string; status: 'queued' | 'slot' | 'failed'; progress: number; reason: string };
type Job = { id: string; environmentId: string; attachmentId: string; url: string; bodyFrom: string; contentType: string; step: number; value: UploadValue };
type Uploads = { slots: Array<Job | null>; claimed: Set<number>; minting: Set<string>; releases: Array<{ environmentId: string; attachmentId: string }> };
/** The progress, load and error events of one upload (uploadBytes' XMLHttpRequest listeners). */
export type UploadEvent = { type: 'progress'; loaded: number; total: number } | { type: 'load'; status: number } | { type: 'error'; kind?: string };

const state = new WeakMap<T3Client, Uploads>();
function uploads(client: T3Client): Uploads {
  let entry = state.get(client);
  if (!entry) { entry = { slots: Array.from({ length: UPLOAD_SLOTS }, () => null), claimed: new Set(), minting: new Set(), releases: [] }; state.set(client, entry); }
  return entry;
}

/** startAttachmentUpload's gate: the server stages uploads and this connection may operate its threads. */
export function uploadsSupported(client: T3Client): boolean {
  return client.writable && obj(obj(client.config.environment).capabilities).attachmentUploads === true;
}

/** A draft image by its id, in any draft: an upload outlives its image's move to another draft. */
function draftImage(client: T3Client, id: string): Obj | undefined {
  for (const images of Object.values(client.local.snapshotDrafts)) {
    const image = images.find(entry => str(entry.id) === id);
    if (image) return image;
  }
  return undefined;
}

/** Whether `id` is uploading: its upload URL is being minted, or a slot carries it. */
export function uploadRunning(client: T3Client, id: string): boolean {
  const entry = uploads(client);
  return entry.minting.has(id) || entry.slots.some(job => job?.id === id);
}

/** The composer's images still to upload: no attachment yet, none running, none failed (their Retry asks again). */
function waiting(client: T3Client): Obj[] {
  if (!uploadsSupported(client)) return [];
  return client.snapshotDrafts.filter(image => !str(image.uploadId) && !uploadRunning(client, str(image.id))
    && client.attachmentUploads[str(image.id)]?.status !== 'failed');
}

/** A slot whose upload ended, or whose image left every draft. */
const settles = (client: T3Client, job: Job | null): job is Job => !!job && (job.value.status !== 'uploading' || !draftImage(client, job.id));

/**
 * Each slot whose upload ended gives its slot back: a landed one's attachment id goes into its image's draft entry
 * (the store, which the command that runs this saves), a failed one keeps its reason for the tile's Retry and its
 * attachment is deleted. A slot whose image left every draft (its Remove) lets its upload go as cancelAttachmentUpload
 * does: the resource's next arguments abort the transfer, and the attachment it minted is deleted. Only a command
 * runs this, so a landed attachment is saved with the draft.
 */
export function settleUploads(client: T3Client): void {
  const entry = uploads(client);
  entry.slots.forEach((job, slot) => {
    if (!settles(client, job)) return;
    const image = draftImage(client, job.id);
    if (job.value.status === 'ready') {
      if (image) image.uploadId = job.attachmentId;
      client.attachmentUploads[job.id] = { status: 'ready', progress: 1, reason: '', attachmentId: job.attachmentId };
    } else {
      entry.releases.push({ environmentId: job.environmentId, attachmentId: job.attachmentId });
      if (!image) delete client.attachmentUploads[job.id];
    }
    entry.slots[slot] = null;
  });
}

/**
 * The snapshot's `composer.uploads`: what each upload resource carries, what waits for one, each image's upload, and
 * `settle`, the slots and attachments the root's next `cclocal:uploads` has to settle.
 */
export function uploadTable(client: T3Client) {
  const entry = uploads(client), next = waiting(client);
  const slot = (index: number) => ({ id: entry.slots[index]?.id ?? '', attachmentId: entry.slots[index]?.attachmentId ?? '' });
  const images = client.snapshotDrafts.flatMap((image): ImageUpload[] => {
    const id = str(image.id), job = entry.slots.find(each => each?.id === id), known = client.attachmentUploads[id];
    if (job?.value.status === 'ready') return [];
    if (job?.value.status === 'failed') return [{ id, attachmentId: '', status: 'failed', progress: 0, reason: job.value.reason }];
    if (job) return [{ id, attachmentId: job.attachmentId, status: 'slot', progress: job.value.progress, reason: '' }];
    if (known?.status === 'failed') return [{ id, attachmentId: '', status: 'failed', progress: 0, reason: known.reason }];
    if (entry.minting.has(id) || next.includes(image)) return [{ id, attachmentId: '', status: 'queued', progress: 0, reason: '' }];
    return [];
  });
  // ChatComposer's effect starts each image as it joins the draft; here the root asks while a slot is free.
  const free = entry.slots.some((job, index) => !job && !entry.claimed.has(index));
  const settle = entry.slots.filter(job => settles(client, job)).length + entry.releases.length;
  return { u0: slot(0), u1: slot(1), u2: slot(2), supported: uploadsSupported(client), pending: free ? next.map(image => str(image.id)).join(' ') : '', settle, images };
}

/**
 * attachmentUploadBlockReason: a send waits for the composer's images still uploading; a failed one's send retries it,
 * and a landed one's names its attachment.
 */
export function uploadBlock(client: T3Client): string {
  if (!uploadsSupported(client)) return '';
  const entry = uploads(client);
  const pending = uploadTable(client).images.filter(image => image.status === 'queued' || entry.slots.some(job => job?.id === image.id && job.value.status === 'uploading')).length;
  return pending === 0 ? '' : pending === 1 ? 'Attachment still uploading' : 'Attachments still uploading';
}

/**
 * The root's `cclocal:uploads`: uploads that ended give their slots back, the attachments of uploads let go are
 * deleted, and each free slot takes the next image, minting its upload URL (runAttachmentUploadCycle's
 * createUploadUrl). `failedSlots`: the slots whose resource failed with no answer (an upload cut off on its way).
 */
export async function runDraftUploads(client: T3Client, native: Native, failedSlots: number[] = []): Promise<void> {
  const entry = uploads(client);
  for (const slot of failedSlots) { const job = entry.slots[slot]; if (job && job.value.status === 'uploading') setFailed(client, job, 'Upload failed'); }
  settleUploads(client);
  for (const release of entry.releases.splice(0)) {
    if (release.environmentId === client.environmentId) await client.request(native, 'attachments.delete', { attachmentId: release.attachmentId }).catch(() => undefined);
  }
  for (const image of waiting(client)) {
    const slot = entry.slots.findIndex((job, index) => !job && !entry.claimed.has(index));
    if (slot < 0) break;
    entry.claimed.add(slot);
    try { await mint(client, native, image, slot); } finally { entry.claimed.delete(slot); }
  }
}

/** The send's own upload of `id` (client.ts uploadDraftImage): no upload resource takes the image until it ends. */
export function holdUpload(client: T3Client, id: string): () => void {
  const entry = uploads(client);
  entry.minting.add(id);
  return () => { entry.minting.delete(id); };
}

/** The tile's Retry (retryAttachmentUpload): a failed image goes back to the queue. */
export function retryDraftUpload(client: T3Client, id: string): void {
  if (client.attachmentUploads[id]?.status === 'failed') delete client.attachmentUploads[id];
}

async function mint(client: T3Client, native: Native, image: Obj, slot: number): Promise<void> {
  const entry = uploads(client), id = str(image.id), environmentId = client.environmentId, origin = client.origin;
  entry.minting.add(id);
  client.attachmentUploads[id] = { status: 'uploading', progress: 0, reason: '', attachmentId: '' };
  try {
    const body = await draftUpload(id).catch(() => { throw new ClientError('Original file is no longer available'); });
    if (!(body.sizeBytes > 0) || body.sizeBytes > UPLOAD_LIMIT) throw new ClientError('Unsupported image type');
    image.sizeBytes = body.sizeBytes;
    const contentType = str(image.mimeType, 'image/png');
    const minted = await client.request(native, 'attachments.createUploadUrl', { type: 'image', name: image.name, mimeType: contentType, sizeBytes: body.sizeBytes })
      .catch(() => { throw new ClientError('Upload could not start'); });
    const attachmentId = str(minted.attachmentId), relative = str(minted.relativeUrl);
    if (!attachmentId || !relative.startsWith('/api/attachments/upload/')) throw new ClientError('Upload could not start');
    // The image left the drafts, or the connection moved, while its URL was minted: that attachment is not used.
    if (!draftImage(client, id) || client.environmentId !== environmentId || client.origin !== origin) {
      entry.releases.push({ environmentId, attachmentId });
      delete client.attachmentUploads[id];
      return;
    }
    const value: UploadValue = { slot, id, attachmentId, status: 'uploading', progress: 0, reason: '' };
    entry.slots[slot] = { id, environmentId, attachmentId, url: onOrigin(relative, origin), bodyFrom: body.bodyFrom, contentType, step: 0, value };
  } catch (error) {
    client.attachmentUploads[id] = { status: 'failed', progress: 0, reason: error instanceof ClientError ? error.message : 'Upload could not start', attachmentId: '' };
  } finally { entry.minting.delete(id); }
}

function setFailed(client: T3Client, job: Job, reason: string): UploadValue {
  job.value = { ...job.value, status: 'failed', progress: 0, reason };
  client.attachmentUploads[job.id] = { status: 'failed', progress: 0, reason, attachmentId: '' };
  return job.value;
}

/**
 * The upload resource's source: slot `slot` carrying image `id` under the attachment its URL was minted for. The
 * fetch is the answer, made before any await, as a stream's must be on the web build.
 */
export function uploadSource(client: T3Client, slot: number, id: string, attachmentId: string): unknown {
  const job = uploads(client).slots[slot];
  if (!id || !job || job.id !== id || job.attachmentId !== attachmentId) return { slot, id: '', attachmentId: '', status: '', progress: 0, reason: '' } satisfies UploadValue;
  if (job.value.status !== 'uploading') return job.value;
  return fetch(job.url, {
    method: 'POST', headers: { 'x-t3-orchestration-protocol': '2', Accept: 'application/json', 'Content-Type': job.contentType },
    exactBodyFrom: job.bodyFrom, exactUploadProgress: true, exactTimeout: UPLOAD_TIMEOUT_MS,
    exactStream: (event: UploadEvent) => uploadEvent(client, job, event),
  } as RequestInit);
}

/** One event of an upload, as its slot's answer. Progress moves in twentieths, as uploadBytes' onProgress does (`lastStep`). */
function uploadEvent(client: T3Client, job: Job, event: UploadEvent): UploadValue {
  if (job.value.status !== 'uploading') return job.value;
  if (event.type === 'progress') {
    const progress = event.total > 0 ? Math.max(0, Math.min(1, event.loaded / event.total)) : 0, step = Math.floor(progress * 20);
    if (step === job.step) return job.value;
    job.step = step;
    job.value = { ...job.value, progress };
    client.attachmentUploads[job.id] = { status: 'uploading', progress, reason: '', attachmentId: '' };
    return job.value;
  }
  if (event.type === 'load' && event.status >= 200 && event.status < 300) {
    job.value = { ...job.value, status: 'ready', progress: 1 };
    client.attachmentUploads[job.id] = { status: 'ready', progress: 1, reason: '', attachmentId: job.attachmentId };
    return job.value;
  }
  if (event.type === 'load') return setFailed(client, job, `Upload rejected (${event.status})`);
  return setFailed(client, job, event.kind === 'Timeout' ? 'Upload timed out' : event.kind === 'Aborted' ? 'Upload cancelled' : 'Upload failed');
}
