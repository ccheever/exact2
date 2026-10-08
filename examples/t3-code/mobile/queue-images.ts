// Pinned365aa87982 state/assets: metadata-keyed signed URLs; no retained native answers.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client } from './shared/client';
export interface QueueImage { id: string; name: string; mimeType: string }
export interface QueueImageEntry { url: string; expires: number; refreshAt: number; pending: boolean }
export interface QueueImageCache { owner: string; visit: string; entries: Map<string, QueueImageEntry> }
const caches = new WeakMap<T3Client, QueueImageCache>();
export const queueImageKey = (image: QueueImage) => JSON.stringify([image.id, image.name, image.mimeType, 'inline']);
export function queueImageCache(client: T3Client, owner: string, visit: string): QueueImageCache {
  let cache = caches.get(client);
  if (!cache || cache.owner !== owner || cache.visit !== visit) {
    cache = { owner, visit, entries: new Map() }; caches.set(client, cache);
  }
  return cache;
}
export const queueImageCacheCurrent = (client: T3Client, cache: QueueImageCache) => caches.get(client) === cache;
export function queueImageSnapshot(client: T3Client, owner: string, visit: string, active: boolean, images: QueueImage[], now: number) {
  const cache = queueImageCache(client, owner, visit), visible = new Set(images.map(queueImageKey));
  // Retain only current displayed queue metadata. Pending entries are never promises.
  for (const key of cache.entries.keys()) if (!active || !visible.has(key)) cache.entries.delete(key);
  const read = active && client.ready && Number.isFinite(now) && now >= 0 ? [...visible].filter(key => {
    const entry = cache.entries.get(key); return !entry || !entry.pending && entry.refreshAt <= now;
  }) : [];
  return { request: read.length ? JSON.stringify(read) : '', url: (image: QueueImage) => {
    const entry = cache.entries.get(queueImageKey(image));
    return active && client.ready && entry && entry.expires > now ? entry.url : '';
  } };
}
