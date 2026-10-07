import { mobileComposerTarget } from './composer-target';
import { mobileQueuedEditCurrent } from './queued-edit-state';
// Pinned365aa87982 FilePreviewModal/VideoPreviewModal.ios resolve once per presentation.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { referencedFiles } from './shared/composer-editor-files';
import { videoMimeType } from './shared/r4-composer-video';
import { assetUrl } from './shared/settings-b-icons';

export interface MediaPreviewSnapshot { identifier: string; name: string; kind: string; sourceJSON: string; ready: boolean; error: string; }
interface MediaSource { identifier: string; name: string; kind: string; source: string; id: string; url: string; mimeType: string; sizeBytes: number; }
const resolved = new WeakMap<T3Client, Map<string, MediaPreviewSnapshot>>();
function selected(client: T3Client, scope: string, id: string): { attachment: Obj; source: string } | undefined {
  if (scope === 'composer') {
    const edit = mobileQueuedEditCurrent(client);
    if (edit) {
      const kept = edit.existingAttachments.find(item => item.id === id);
      if (kept) return { attachment: kept, source: 'remote' };
      const added = edit.attachments.find(item => item.id === id);
      return added ? { attachment: { ...added }, source: added.kind === 'image' ? 'draft-image' : 'draft-file' } : undefined;
    }
    const image = client.snapshotDrafts.find(item => item.id === id);
    if (image) return { attachment: image, source: 'draft-image' };
    const file = referencedFiles(client.local, client.draftKey, client.draft).find(item => item.id === id);
    if (file) return { attachment: { ...file }, source: 'draft-file' };
  } else if (scope === 'transcript') {
    for (const row of arr(client.projection.visibleTurnItems)) {
      const attachment = arr(obj(row.item).attachments).find(item => item.id === id);
      if (attachment && !str(attachment.name).startsWith('preview-annotation-')) return { attachment, source: 'remote' };
    }
  }
  return undefined;
}
function owner(client: T3Client, scope: string) {
  return JSON.stringify([client.generation, client.environmentId, client.threadId, scope === 'composer' ? mobileComposerTarget(client).owner : '']);
}
/** Recheck after asynchronous file work, including removal without a route change. */
export function mobileMediaOwned(scope: string, id: string, client: T3Client = mobileClient): boolean {
  return selected(client, scope, id) !== undefined;
}

/** Only server-issued http(s) URLs enter the native downloader; local paths stay below the seam. */
export function mobileMediaURL(origin: string, relative: string): string {
  if (!relative) throw new ClientError('The environment returned an invalid media URL.');
  const value = assetUrl(origin, relative), parsed = new URL(value);
  if (!['http:', 'https:'].includes(parsed.protocol) || !parsed.hostname || parsed.username || parsed.password)
    throw new ClientError('The environment returned an invalid media URL.');
  return parsed.href;
}

/** Root keys this answer by route identity and current environment/thread, not every client revision.
 * It renders a presenter only while that route is current. Its native view owns dismissal/cancellation.
 */
export async function mobileMediaPrepare(scope: string, id: string, routeKey: string, nativeInput: Native | null | undefined,
  client: T3Client = mobileClient, expectedEnvironment = client.environmentId, expectedThread = client.threadId, expectedContentOwner = ''): Promise<MediaPreviewSnapshot> {
  const scopeOwner = owner(client, scope), identifier = JSON.stringify([scopeOwner, scope, id, routeKey]);
  const failure = (error: string): MediaPreviewSnapshot => ({ identifier, name: '', kind: '', sourceJSON: '', ready: false, error });
  if (!routeKey || !id || !['composer', 'transcript'].includes(scope)) return failure('That preview is no longer available.');
  if (scope === 'composer' && expectedContentOwner && expectedContentOwner !== mobileComposerTarget(client).owner) return failure('The composer changed. Open this attachment again.');
  if (client.environmentId !== expectedEnvironment || client.threadId !== expectedThread) return failure('The selected conversation changed.');
  const found = selected(client, scope, id);
  if (!found) return failure(scope === 'composer' ? 'This attachment is no longer available. Attach the file again.' : 'That attachment is no longer in this conversation.');
  if (!nativeInput?.available) return failure('Open T3 Code on your iPhone or iPad to preview files.');
  const { attachment, source } = found, name = str(attachment.name, 'Preview'), mimeType = str(attachment.mimeType);
  const kind = videoMimeType({ name, mimeType }) ? 'video' : source === 'draft-image' || mimeType.startsWith('image/') || attachment.type === 'image' ? 'image' : 'file';
  let cache = resolved.get(client); if (!cache) { cache = new Map(); resolved.set(client, cache); }
  const existing = cache.get(identifier); if (existing) return existing;
  const native = letGoAware(mobileNative(nativeInput));
  try {
    let url = '';
    if (source === 'remote') {
      if (client.connection !== 'connected') throw new ClientError('Reconnect to this environment and try again.');
      // Re-mint at open: a thumbnail's URL may have expired while the app was suspended.
      const reply = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: id,
        fileName: name, ...(mimeType ? { mimeType } : {}), disposition: 'inline' } }));
      if (scopeOwner !== owner(client, scope) || !selected(client, scope, id)) throw new ClientError('The selected conversation changed.');
      url = mobileMediaURL(client.origin, str(reply.relativeUrl));
    }
    const value: MediaSource = { identifier, name, kind, source, id, url, mimeType, sizeBytes: Number(attachment.sizeBytes) || 0 };
    const snapshot = { identifier, name, kind, sourceJSON: JSON.stringify(value), ready: true, error: '' };
    cache.set(identifier, snapshot); if (cache.size > 16) cache.delete(cache.keys().next().value!);
    return snapshot;
  } catch (error) { if (letGo(error)) throw error; return failure(error instanceof Error ? error.message : 'The file could not be loaded. Please try again.'); }
}

/** Share original bytes using the same ownership checks; native keeps an independent copy until dismissed. */
export async function mobileMediaShare(scope: string, id: string, routeKey: string, nativeInput: Native | null | undefined,
  client: T3Client = mobileClient) {
  const scopeOwner = owner(client, scope);
  const source = await mobileMediaPrepare(scope, id, routeKey, nativeInput, client);
  if (!source.ready || !nativeInput?.available) return { identifier: source.identifier, message: source.error };
  if (scopeOwner !== owner(client, scope) || !selected(client, scope, id)) return { identifier: source.identifier, message: 'The selected conversation changed.' };
  const native = letGoAware(mobileNative(nativeInput));
  try {
    const reply = await bridgeReply(native, { op: 'mobileMediaShare', sourceJSON: source.sourceJSON });
    if (!reply.ok) throw new ClientError(reply.error!.message);
    return { identifier: source.identifier, message: '' };
  } catch (error) { if (letGo(error)) throw error; return { identifier: source.identifier, message: error instanceof Error ? error.message : 'Could not share this file.' }; }
}

/** On native decode/playback failure, a subsequent explicit retry gets a newly signed URL. */
export function mobileMediaForget(identifier: string, client: T3Client = mobileClient): void { resolved.get(client)?.delete(identifier); }
