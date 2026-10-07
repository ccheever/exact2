// Pinned365aa87982 QueueAttachmentThumbnails/useAssetUrl; shared signed-asset RPC.
// @ref llp/1106.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { queueState } from './shared/composer-controls-queue';
import { arr, str } from './shared/domain';
import { mobileMediaURL } from './media-preview';
import { mobileQueueCurrent } from './queue';
import { queueImageCache, queueImageCacheCurrent, queueImageKey, type QueueImageEntry } from './queue-images';
const STALE_MS = 5 * 60_000;
/** Explicit mutation. Pending metadata coalesces repeated requests without sharing
 * answer promises. All four workers drain before the native answer is released. */
export async function mobileQueuePrepare(owner: string, visit: string, request: string, now: number,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient): Promise<{ revision: number; message: string }> {
  const result = () => ({ revision: client.revision, message: '' });
  const assertCurrent = () => {
    if (!mobileQueueCurrent(owner, visit, client) || !client.ready) throw new ClientError('The queue changed.', 'superseded');
  };
  assertCurrent();
  if (!nativeInput?.available || !request || !Number.isFinite(now) || now < 0) return result();
  let keys: unknown;
  try { keys = JSON.parse(request); } catch { throw new ClientError('That thumbnail request changed.', 'superseded'); }
  if (!Array.isArray(keys) || keys.some(key => typeof key !== 'string')) throw new ClientError('That thumbnail request changed.', 'superseded');
  const images = () => queueState(client.projection).queued.flatMap(entry => {
    const message = arr(client.projection.messages).find(message => message.id === entry.run.userMessageId);
    return arr(message?.attachments).filter(image => str(image.mimeType).startsWith('image/')).slice(0, 3)
      .map(image => ({ id: str(image.id), name: str(image.name), mimeType: str(image.mimeType) }));
  });
  const live = (key: string) => images().find(image => queueImageKey(image) === key);
  const cache = queueImageCache(client, owner, visit), wanted = [...new Set(keys as string[])];
  if (wanted.some(key => !live(key))) throw new ClientError('The queue attachments changed.', 'superseded');
  const base = letGoAware(mobileNative(nativeInput));
  let cursor = 0;
  const workers = Array.from({ length: Math.min(4, wanted.length) }, async () => {
    while (cursor < wanted.length) {
      assertCurrent();
      const key = wanted[cursor++]!, image = live(key), prior = cache.entries.get(key);
      if (!image || prior?.pending || prior && prior.refreshAt > now) continue;
      const entry: QueueImageEntry = { url: prior?.url ?? '', expires: prior?.expires ?? 0, refreshAt: 0, pending: true };
      cache.entries.set(key, entry);
      const current = () => queueImageCacheCurrent(client, cache) && cache.entries.get(key) === entry && !!live(key);
      const assertImage = () => { assertCurrent(); if (!current()) throw new ClientError('The queue attachment changed.', 'superseded'); };
      const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
        assertImage(); const reply = await base.later(input); assertImage(); return reply;
      } };
      try {
        const reply = await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: image.id,
          fileName: image.name, mimeType: image.mimeType, disposition: 'inline' } });
        assertImage(); const expires = Number(reply.expiresAt);
        if (!Number.isFinite(expires) || expires <= now) throw new ClientError('The queued attachment URL expired.');
        entry.url = mobileMediaURL(client.origin, str(reply.relativeUrl)); entry.expires = expires;
        entry.refreshAt = Math.min(expires, now + STALE_MS);
      } catch (error) {
        if (letGo(error)) { if (current()) cache.entries.delete(key); throw error; }
        if (current()) { entry.url = ''; entry.expires = 0; entry.refreshAt = now + STALE_MS; }
      } finally { if (current()) entry.pending = false; }
    }
  });
  const settled = await Promise.allSettled(workers);
  client.revision++;
  const failed = settled.find((entry): entry is PromiseRejectedResult => entry.status === 'rejected');
  if (failed) throw failed.reason;
  assertCurrent(); return result();
}
