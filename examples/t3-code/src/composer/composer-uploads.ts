// attachmentUploadQueue.ts and attachmentUploadState.ts at T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3): a draft image
// uploads as soon as it is attached, at most three at once on each machine, and its tile draws the upload's percentage
// while the person types. Each upload runs in one of the root's six upload resources (composerUpload0 to 5,
// composer-controls-members.contract), a stream whose fetch tells the bytes it has sent (`exactUploadProgress`, LLP
// 1016.000 D10), so no answer waits on it. The table here says which image each resource carries. An image keeps its
// resource until its upload ends, since new arguments let a running upload go; the server's attachment id then lands in
// the image's own draft entry (`uploadId`), which the send names. An upload belongs to its draft entry, not to the image
// id: a chip pasted into another draft is that draft's image, and uploads again for it. The uploads go on whichever draft
// is shown; one waiting on another machine waits until that machine is focused again, since the requests go to the
// focused one (upstream's queue runs each environment's at once). The ids last the session: the server deletes an
// attachment that was never sent after a day, so a relaunch uploads again, as upstream's in-memory queue does.
//
// A send waits for its images on no timer of its own: an answer let go meanwhile would never resume (LLP 1027.000 D1).
// A press whose images have not all landed is kept with what it sends (useComposerSend's snapshot at the press), its
// failed images go back to the queue, and the root sends it once they land (`sendResume`), whichever thread is shown.
import type { T3Client } from '../client/client';
import { windowClient } from '../client/scope-root';
import { ClientError, type Files, type Native } from '../client/protocol';
import { arr, obj, str, type Obj } from '../client/domain';
import { draftUpload } from '../snapshot/snapshot-upload';
import { path as onOrigin } from '../transport/transport-wire';
import { editStashImages } from './composer-controls-queue';

/** The root's upload resources: MAX_UPLOADS_PER_ENVIRONMENT for the focused machine, and as many for one focused before. */
export const UPLOAD_SLOTS = 6;
/** MAX_UPLOADS_PER_ENVIRONMENT. */
export const UPLOADS_PER_MACHINE = 3;
const UPLOAD_TIMEOUT_MS = 5 * 60_000;
/** PROVIDER_SEND_TURN_SUPPORTED_IMAGE_MIME_TYPES. */
const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

/** One slot's answer: the image it carries and how its upload goes ('' while it carries none). */
export type UploadValue = { slot: number; id: string; attachmentId: string; status: '' | 'uploading' | 'ready' | 'failed'; progress: number; reason: string };
/** An image's upload for its tile: waiting for a slot, in one (whose answer has its percentage), or failed with the reason. */
export type ImageUpload = { id: string; attachmentId: string; status: 'queued' | 'slot' | 'failed'; progress: number; reason: string };
type Entry = { draftKey: string; id: string; environmentId: string };
type Job = Entry & { attachmentId: string; url: string; bodyFrom: string; contentType: string; step: number; value: UploadValue };
type Failure = { reason: string; generation: number };
type Uploads = {
  slots: Array<Job | null>; claimed: Set<number>; minting: Set<string>;
  /** Images started (startAttachmentUpload) and waiting for a slot, in order, whatever draft is shown. */
  queue: Entry[];
  failed: Map<string, Failure>;
  releases: Array<{ environmentId: string; attachmentId: string }>;
};
/** The progress, load and error events of one upload (uploadBytes' XMLHttpRequest listeners). */
export type UploadEvent = { type: 'progress'; loaded: number; total: number } | { type: 'load'; status: number } | { type: 'error'; kind?: string };

const state = new WeakMap<T3Client, Uploads>();
// The window's: a side chat's draft images (thread-scope.ts) share its six upload resources and queue.
function uploads(client: T3Client): Uploads {
  client = windowClient(client);
  let entry = state.get(client);
  if (!entry) { entry = { slots: Array.from({ length: UPLOAD_SLOTS }, () => null), claimed: new Set(), minting: new Set(), queue: [], failed: new Map(), releases: [] }; state.set(client, entry); }
  return entry;
}
/** An image's upload is its draft entry's: the draft and the image id. */
const keyOf = (draftKey: string, id: string) => `${draftKey}\u0000${id}`;

/** startAttachmentUpload's gate: the server stages uploads and this connection may operate its threads. */
export function uploadsSupported(client: T3Client): boolean {
  return client.writable && obj(obj(client.config.environment).capabilities).attachmentUploads === true;
}

/** The draft entry an upload is for: in its draft, or in that draft's stash while a queued message is edited. */
function draftImage(client: T3Client, draftKey: string, id: string): Obj | undefined {
  return (client.local.snapshotDrafts[draftKey] ?? []).find(image => str(image.id) === id)
    ?? editStashImages(client, draftKey).find(image => str(image.id) === id);
}

/** Whether the draft entry's upload is in flight: its URL being minted, or in a slot. */
function inFlight(client: T3Client, draftKey: string, id: string): boolean {
  const entry = uploads(client), key = keyOf(draftKey, id);
  return entry.minting.has(key) || entry.slots.some(job => job !== null && keyOf(job.draftKey, job.id) === key && job.value.status === 'uploading');
}
/** Whether the draft entry's upload is under way: in flight, or waiting for a slot. */
function running(client: T3Client, draftKey: string, id: string): boolean {
  return inFlight(client, draftKey, id) || uploads(client).queue.some(queued => queued.draftKey === draftKey && queued.id === id);
}

/**
 * A failure is kept for the tile's Retry, until the connection that failed it is replaced: then it is asked again, as
 * upstream's queue retries a failed upload when its environment reconnects.
 */
function failure(client: T3Client, draftKey: string, id: string): Failure | undefined {
  const entry = uploads(client), key = keyOf(draftKey, id), failed = entry.failed.get(key);
  if (failed && client.connection === 'connected' && client.generation > failed.generation) { entry.failed.delete(key); return undefined; }
  return failed;
}

/** ChatComposer's effect: each image of the shown draft without an attachment starts uploading (joins the queue). */
function start(client: T3Client): void {
  if (!uploadsSupported(client)) return;
  const entry = uploads(client), draftKey = client.attachmentKey;
  for (const image of client.snapshotDrafts) {
    const id = str(image.id);
    if (str(image.uploadId) || running(client, draftKey, id) || failure(client, draftKey, id)) continue;
    entry.queue.push({ draftKey, id, environmentId: client.environmentId });
  }
}

/** The queue's next image this connection can mint, dropping the images that left their drafts. */
function nextQueued(client: T3Client): Entry | undefined {
  const entry = uploads(client);
  entry.queue = entry.queue.filter(queued => {
    const image = draftImage(client, queued.draftKey, queued.id);
    return !!image && !str(image.uploadId) && !entry.minting.has(keyOf(queued.draftKey, queued.id));
  });
  return entry.queue.find(queued => queued.environmentId === client.environmentId);
}

/** A slot whose upload ended, or whose image left its draft. */
const settles = (client: T3Client, job: Job | null): job is Job => !!job && (job.value.status !== 'uploading' || !draftImage(client, job.draftKey, job.id));

/**
 * Each slot whose upload ended gives its slot back. A landed one's attachment id goes into its own draft entry (the
 * command that runs this saves it); a failed one keeps its reason for the tile's Retry and its attachment is deleted. A
 * slot whose image left its draft (its Remove, landed or not) lets its upload go, as cancelAttachmentUpload and
 * releaseAttachmentUpload do: the resource's next arguments abort the transfer, and the attachment is deleted. A sent
 * image never reaches that: the send settles its draft's uploads before it names them.
 */
export function settleUploads(client: T3Client): void {
  const entry = uploads(client);
  entry.slots.forEach((job, slot) => {
    if (!settles(client, job)) return;
    const image = draftImage(client, job.draftKey, job.id), key = keyOf(job.draftKey, job.id);
    if (job.value.status === 'ready' && image) image.uploadId = job.attachmentId;
    else {
      entry.releases.push({ environmentId: job.environmentId, attachmentId: job.attachmentId });
      if (image) entry.failed.set(key, { reason: job.value.reason, generation: client.generation });
    }
    entry.slots[slot] = null;
  });
}

/**
 * A resource free for the focused machine's next upload, or -1: MAX_UPLOADS_PER_ENVIRONMENT counts that machine's
 * uploads (the ones being minted are its own), whatever another machine still sends.
 */
function freeSlot(client: T3Client): number {
  const entry = uploads(client);
  const busy = entry.slots.filter(job => job !== null && job.environmentId === client.environmentId).length + entry.claimed.size;
  return busy >= UPLOADS_PER_MACHINE ? -1 : entry.slots.findIndex((job, index) => !job && !entry.claimed.has(index));
}

/**
 * The snapshot's `composer.uploads`: what each upload resource carries, the images waiting while one is free (the root
 * then starts them), the slots and attachments to settle, and the shown draft's images' uploads.
 */
export function uploadTable(client: T3Client) {
  const entry = uploads(client), draftKey = client.attachmentKey, supported = uploadsSupported(client);
  const slot = (index: number) => ({ id: entry.slots[index]?.id ?? '', attachmentId: entry.slots[index]?.attachmentId ?? '' });
  const unstarted = supported ? client.snapshotDrafts.filter(image => !str(image.uploadId) && !running(client, draftKey, str(image.id)) && !failure(client, draftKey, str(image.id))) : [];
  const images = client.snapshotDrafts.flatMap((image): ImageUpload[] => {
    const id = str(image.id), key = keyOf(draftKey, id), job = entry.slots.find(each => each !== null && keyOf(each.draftKey, each.id) === key);
    if (job?.value.status === 'ready' || str(image.uploadId)) return [];
    if (job?.value.status === 'failed') return [{ id, attachmentId: '', status: 'failed', progress: 0, reason: job.value.reason }];
    if (job) return [{ id, attachmentId: job.attachmentId, status: 'slot', progress: job.value.progress, reason: '' }];
    const failed = failure(client, draftKey, id);
    if (failed) return [{ id, attachmentId: '', status: 'failed', progress: 0, reason: failed.reason }];
    if (running(client, draftKey, id) || unstarted.includes(image)) return [{ id, attachmentId: '', status: 'queued', progress: 0, reason: '' }];
    return [];
  });
  const free = freeSlot(client) >= 0;
  const waiting = [...unstarted.map(image => str(image.id)), ...entry.queue.filter(queued => queued.environmentId === client.environmentId).map(queued => queued.id)];
  const settle = entry.slots.filter(job => settles(client, job)).length + entry.releases.filter(release => release.environmentId === client.environmentId).length;
  return { u0: slot(0), u1: slot(1), u2: slot(2), u3: slot(3), u4: slot(4), u5: slot(5), supported, operate: client.scopes.includes('orchestration:operate'),
    // ChatComposer's effect starts each image as it joins the draft; here the root asks while a slot is free.
    pending: free ? [...new Set(waiting)].join(' ') : '', settle, images };
}

/**
 * attachmentUploadBlockReason: Send and Return wait for the shown draft's images still uploading. A failed one does not
 * hold them, since a send retries it, nor does a landed one, which the send names.
 */
export function uploadBlock(client: T3Client): string {
  if (!uploadsSupported(client)) return '';
  const pending = uploadTable(client).images.filter(image => image.status !== 'failed').length;
  return pending === 0 ? '' : pending === 1 ? 'Attachment still uploading' : 'Attachments still uploading';
}

/**
 * The root's `cclocal:uploads`: the shown draft's new images join the queue, uploads that ended give their slots back,
 * the attachments of uploads let go are deleted, and each free slot takes the queue's next image, minting its upload
 * URL (runAttachmentUploadCycle's createUploadUrl). `failedSlots`: the slots whose resource failed with no answer (an
 * upload cut off on its way).
 */
export async function runDraftUploads(client: T3Client, native: Native, failedSlots: number[] = []): Promise<void> {
  const entry = uploads(client);
  for (const slot of failedSlots) { const job = entry.slots[slot]; if (job && job.value.status === 'uploading') setFailed(job, 'Upload failed'); }
  settleUploads(client);
  start(client);
  // Another machine's attachments wait until it is focused again: the requests go to the focused one.
  const releases = entry.releases.filter(release => release.environmentId === client.environmentId);
  entry.releases = entry.releases.filter(release => !releases.includes(release));
  for (const release of releases) await client.request(native, 'attachments.delete', { attachmentId: release.attachmentId }).catch(() => undefined);
  for (let queued = nextQueued(client); queued; queued = nextQueued(client)) {
    const slot = freeSlot(client);
    if (slot < 0) break;
    entry.queue.splice(entry.queue.indexOf(queued), 1);
    entry.claimed.add(slot);
    try { await mint(client, native, queued, slot); } finally { entry.claimed.delete(slot); }
  }
}

/** RETRY_FAILED_UPLOADS: what a kept send says when an image it names failed again or left the draft. */
export const RETRY_FAILED_UPLOADS = 'Retry or remove failed uploads before sending.';

/**
 * A press kept until its images land, as useComposerSend snapshots it at the press: `resume` sends what it named, from
 * whichever thread is shown then.
 */
export type WaitingSend = { token: string; draftKey: string; environmentId: string; imageIds: string[]; resume: (native: Native, storage: Files) => Promise<string | void> };
const kept = new WeakMap<T3Client, WaitingSend[]>();
let sends = 0;
const keptSends = (client: T3Client) => kept.get(client) ?? [];

/** The press the shown draft keeps, if any: a second press there sends nothing, as while upstream's send is in flight. */
export function waitingSend(client: T3Client): WaitingSend | undefined { return keptSends(client).find(press => press.draftKey === client.attachmentKey); }

/**
 * The images a send names (client.ts uploadSnapshots), where the server stages uploads: all of them once each has its
 * attachment. Until then the press is kept (`deferred`): its failed images go back to the queue, as useComposerSend
 * retries them. A kept press sent again (`resume`) asks instead to retry or remove one that failed again or left the draft.
 */
export function stagedImages(client: T3Client, draftKey: string, images: Obj[], named: number, resume: boolean): Obj[] {
  settleUploads(client);
  const missing = images.filter(image => !str(image.uploadId));
  if (resume && (missing.length || images.length !== named)) throw new ClientError(RETRY_FAILED_UPLOADS);
  if (!missing.length) return images;
  for (const image of missing) uploads(client).failed.delete(keyOf(draftKey, str(image.id)));
  throw new ClientError('Attachments still uploading', 'deferred');
}

/** Keeps a press whose images are still uploading, to be sent once they land (the root's `sendResume`). */
export function deferSend(client: T3Client, press: Omit<WaitingSend, 'token'>): void {
  kept.set(client, [...keptSends(client).filter(each => each.draftKey !== press.draftKey), { token: `send-${++sends}`, ...press }]);
}

/**
 * The first kept press on the focused machine that can be sent: each image it names has its attachment, failed, or left
 * the draft, so none is still on its way (the send then says what failed). '' when none can, and while the connection
 * may not write, so the root sends it once one can.
 */
export function readySend(client: T3Client): string {
  if (!client.writable) return '';
  const entry = uploads(client);
  return keptSends(client).find(press => press.environmentId === client.environmentId && !press.imageIds.some(id => {
    const image = draftImage(client, press.draftKey, id), key = keyOf(press.draftKey, id);
    if (!image || str(image.uploadId)) return false;
    const job = entry.slots.find(each => each !== null && keyOf(each.draftKey, each.id) === key);
    return job ? job.value.status === 'uploading' : !failure(client, press.draftKey, id);
  }))?.token ?? '';
}

/** The kept press `token`, taken, or undefined when none is kept under it. */
export function takeWaitingSend(client: T3Client, token: string): WaitingSend | undefined {
  const press = keptSends(client).find(each => each.token === token);
  if (press) kept.set(client, keptSends(client).filter(each => each !== press));
  return press;
}

/** The tile's Retry (retryAttachmentUpload): a failed image goes back to the queue. */
export function retryDraftUpload(client: T3Client, id: string): void {
  uploads(client).failed.delete(keyOf(client.attachmentKey, id));
}

/**
 * A send the server refused for an attachment it no longer has (AttachmentClaims: "Attachment '<name>' cannot be sent:
 * attachment not found", one deleted unsent after a day): this machine's draft images of that name forget their ids,
 * so they upload again. True when that was the refusal.
 */
export function forgetMissingAttachments(client: T3Client, error: unknown): boolean {
  const refused = error instanceof Error ? /Attachment '(.*)' cannot be sent: attachment not found/.exec(error.message) : null;
  if (!refused) return false;
  for (const [key, images] of Object.entries(client.local.snapshotDrafts)) {
    if (key.startsWith(`${client.environmentId}:`)) for (const image of images) if (str(image.name) === refused[1]) delete image.uploadId;
  }
  return true;
}

/**
 * At launch, draft images keep no attachment id from an earlier session (the server may have deleted it unsent), but
 * one a submission still in doubt names, which settles it when its outcome comes back.
 */
export function forgetUnsentUploads(local: { snapshotDrafts: Record<string, Obj[]>; pending: Record<string, { payload: Obj }>; launches?: Record<string, { payload: Obj }> }): void {
  const named = new Set<string>();
  for (const pending of [...Object.values(local.pending), ...Object.values(local.launches ?? {})]) {
    for (const attachment of [...arr(pending.payload.attachments), ...arr(obj(pending.payload.initialMessage).attachments)]) named.add(str(attachment.id));
  }
  for (const images of Object.values(local.snapshotDrafts)) for (const image of images) if (!named.has(str(image.uploadId))) delete image.uploadId;
}

async function mint(client: T3Client, native: Native, queued: Entry, slot: number): Promise<void> {
  const entry = uploads(client), { draftKey, id } = queued, key = keyOf(draftKey, id), environmentId = client.environmentId, origin = client.origin;
  const image = draftImage(client, draftKey, id);
  if (!image) return;
  entry.minting.add(key);
  try {
    const contentType = str(image.mimeType, 'image/png').toLowerCase();
    if (!IMAGE_TYPES.includes(contentType)) throw new ClientError('Unsupported image type');
    const body = await draftUpload(id).catch(() => { throw new ClientError('Original file is no longer available'); });
    if (!(body.sizeBytes > 0)) throw new ClientError('Original file is no longer available');
    image.sizeBytes = body.sizeBytes;
    const minted = await client.request(native, 'attachments.createUploadUrl', { type: 'image', name: image.name, mimeType: contentType, sizeBytes: body.sizeBytes })
      .catch(() => { throw new ClientError('Upload could not start'); });
    const attachmentId = str(minted.attachmentId), relative = str(minted.relativeUrl);
    if (!attachmentId || !relative.startsWith('/api/attachments/upload/')) throw new ClientError('Upload could not start');
    // The image left its draft, or the connection moved, while its URL was minted: that attachment is not used.
    if (!draftImage(client, draftKey, id) || client.environmentId !== environmentId || client.origin !== origin) {
      entry.releases.push({ environmentId, attachmentId });
      return;
    }
    // `step` -1: the first progress shows, as uploadBytes' `lastStep` starts below any step.
    const value: UploadValue = { slot, id, attachmentId, status: 'uploading', progress: 0, reason: '' };
    entry.slots[slot] = { draftKey, id, environmentId, attachmentId, url: onOrigin(relative, origin), bodyFrom: body.bodyFrom, contentType, step: -1, value };
  } catch (error) {
    entry.failed.set(key, { reason: error instanceof ClientError ? error.message : 'Upload could not start', generation: client.generation });
  } finally { entry.minting.delete(key); }
}

function setFailed(job: Job, reason: string): UploadValue {
  job.value = { ...job.value, status: 'failed', progress: 0, reason };
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
    exactStream: (event: UploadEvent) => uploadEvent(job, event),
  } as RequestInit);
}

/** One event of an upload, as its slot's answer. Progress moves in twentieths, as uploadBytes' onProgress does. */
function uploadEvent(job: Job, event: UploadEvent): UploadValue {
  if (job.value.status !== 'uploading') return job.value;
  if (event.type === 'progress') {
    const progress = event.total > 0 ? Math.max(0, Math.min(1, event.loaded / event.total)) : 0, step = Math.floor(progress * 20);
    if (step === job.step) return job.value;
    job.step = step;
    job.value = { ...job.value, progress };
    return job.value;
  }
  if (event.type === 'load' && event.status >= 200 && event.status < 300) {
    job.value = { ...job.value, status: 'ready', progress: 1 };
    return job.value;
  }
  if (event.type === 'load') return setFailed(job, `Upload rejected (${event.status})`);
  return setFailed(job, event.kind === 'Timeout' ? 'Upload timed out' : event.kind === 'Aborted' ? 'Upload cancelled' : 'Upload failed');
}
