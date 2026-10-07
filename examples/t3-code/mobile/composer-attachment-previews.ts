// @ref llp/1106.005-composer-and-transcript.decision.md#media-presentation
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileNative } from './client';
import { mobileComposerTarget, mobileComposerTargetCurrent } from './composer-target';
import { mobileMediaURL } from './media-preview';
interface PreviewItem { id: string; name: string; kind: string; mimeType: string; removeOperation: string }
interface Entry { url: string; expires: number; refreshAt: number; pending: boolean }
interface Cache { owner: string; entries: Map<string, Entry> }
const caches = new WeakMap<T3Client, Cache>();
const key = (item: PreviewItem) => JSON.stringify([item.id, item.name, item.mimeType, item.removeOperation]);
function cacheFor(client: T3Client) {
  const owner = mobileComposerTarget(client).owner;
  let cache = caches.get(client);
  if (!cache || cache.owner !== owner) { cache = { owner, entries: new Map() }; caches.set(client, cache); }
  return cache;
}
export function composerAttachmentPreview(client: T3Client, item: PreviewItem, now: number): string {
  const entry = cacheFor(client).entries.get(key(item));
  return entry && entry.expires > now ? entry.url : '';
}
export function composerAttachmentPreviewRequest(client: T3Client, items: PreviewItem[], now: number): string {
  const cache = cacheFor(client), live = new Set(items.filter(item => item.kind === 'image').map(key));
  for (const id of cache.entries.keys()) if (!live.has(id)) cache.entries.delete(id);
  const wanted = [...live].filter(id => { const entry = cache.entries.get(id); return !entry || !entry.pending && entry.refreshAt <= now; });
  return wanted.length ? JSON.stringify(wanted) : '';
}
/** Only scalar metadata survives an answer; drain every worker before returning. */
export async function prepareComposerAttachmentPreviews(nativeInput: Native | null | undefined, client: T3Client,
  items: () => PreviewItem[], now: number, expectedOwner = '', expectedRequest = '') {
  if (!nativeInput?.available || !Number.isFinite(now) || now < 0) return { revision: client.revision };
  const target = mobileComposerTarget(client), cache = cacheFor(client), base = letGoAware(mobileNative(nativeInput));
  if (expectedOwner && expectedOwner !== target.owner) throw new ClientError('The composer changed.', 'superseded');
  let requested: unknown = null;
  if (expectedRequest) {
    try { requested = JSON.parse(expectedRequest); } catch { throw new ClientError('The attachment request changed.', 'superseded'); }
    if (!Array.isArray(requested) || requested.some(value => typeof value !== 'string' || !items().some(item => key(item) === value)))
      throw new ClientError('The attachment request changed.', 'superseded');
  }
  const wanted = items().filter(item => item.kind === 'image' && (!Array.isArray(requested) || requested.includes(key(item)))), liveKeys = new Set(items().filter(item => item.kind === 'image').map(key));
  for (const id of cache.entries.keys()) if (!liveKeys.has(id)) cache.entries.delete(id);
  let cursor = 0;
  const workers = Array.from({ length: Math.min(4, wanted.length) }, async () => {
    while (cursor < wanted.length) {
      const item = wanted[cursor++]!, id = key(item), prior = cache.entries.get(id);
      if (prior?.pending || prior && prior.refreshAt > now) continue;
      const entry: Entry = { url: prior?.url ?? '', expires: prior?.expires ?? 0, refreshAt: 0, pending: true };
      cache.entries.set(id, entry);
      const current = () => mobileComposerTargetCurrent(client, target) && caches.get(client) === cache
        && cache.entries.get(id) === entry && items().some(value => key(value) === id);
      const check = () => { if (!current()) throw new ClientError('The composer attachment changed.', 'superseded'); };
      const native: Native = { available: true, watch: topic => base.watch(topic), later: async request => {
        check(); const reply = await base.later(request); check(); return reply;
      } };
      try {
        check();
        if (item.removeOperation === 'remove-retained') {
          const reply = await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: item.id,
            fileName: item.name, mimeType: item.mimeType, disposition: 'inline' } });
          check(); const expires = Number(reply.expiresAt);
          if (!Number.isFinite(expires) || expires <= now) throw new ClientError('The attachment URL expired.');
          entry.url = mobileMediaURL(client.origin, str(reply.relativeUrl)); entry.expires = expires;
          entry.refreshAt = Math.min(expires, now + 300_000);
        } else {
          const reply = await bridgeReply(native, { op: 'mobileAttachmentPreview', id: item.id, image: item.removeOperation === 'remove-snapshot' });
          check(); if (!reply.ok) throw new ClientError(reply.error!.message);
          entry.url = str(obj(reply.value).dataUrl); entry.expires = Number.MAX_SAFE_INTEGER; entry.refreshAt = Number.MAX_SAFE_INTEGER;
        }
      } catch (error) {
        if (letGo(error)) { if (current()) cache.entries.delete(id); throw error; }
        if (current()) { entry.url = ''; entry.expires = 0; entry.refreshAt = now + 300_000; }
      } finally { if (current()) entry.pending = false; }
    }
  });
  const settled = await Promise.allSettled(workers);
  const failure = settled.find((value): value is PromiseRejectedResult => value.status === 'rejected');
  if (failure) throw failure.reason;
  return { revision: client.revision };
}
