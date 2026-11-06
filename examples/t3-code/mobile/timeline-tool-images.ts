// Tool output image previews from T3 Code's V2ItemInspector (MIT, see LICENSE-T3).
import { arr, obj, str, type Obj } from './shared/domain';
import type { T3Client } from './shared/client';
import type { Native } from './shared/protocol';
import { assetUrl } from './shared/settings-b-icons';
import { encodeMediaSource } from './shared/media-actions';
import { turnItemDetailView, turnItemIsOpen } from './shared/timeline-item-fetch';
import { turnItemOutputImages } from './timeline-tool-output-images';

export type ToolImage = { id: string; name: string; source: string; resource: Obj };

export function toolImages(item: Obj): ToolImage[] {
  return turnItemOutputImages(item).map((resource, index) => {
    const id = `tool-image:${JSON.stringify([resource.threadId, resource.itemId, index])}`;
    const name = `Tool output image ${index + 1}`;
    return { id, name, resource, source: encodeMediaSource({ kind: 'image', name, src: null, asset: { resource } }) };
  });
}

export function findToolImage(client: T3Client, id: string): ToolImage | undefined {
  for (const row of arr(client.projection.visibleTurnItems)) {
    const rowId = JSON.stringify([row.sourceThreadId, row.sourceItemId]);
    const image = toolImages(turnItemDetailView(client, obj(row.item), undefined, rowId).item).find(value => value.id === id);
    if (image) return image;
  }
  return undefined;
}

interface ToolImageCache { owner: string; url: string; at: number }
const urls = new WeakMap<T3Client, Map<string, ToolImageCache>>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.origin, client.threadId]);

export function cachedToolImageUrl(client: T3Client, id: string): string | null {
  const value = urls.get(client)?.get(`${owner(client)}\u0000${id}`);
  return value?.url || null;
}

/** Signs the image resources belonging to expanded tool rows and the active preview. */
export async function toolImageUrls(client: T3Client, native: Native, now: number): Promise<void> {
  const wanted = new Map<string, ToolImage>();
  for (const row of arr(client.projection.visibleTurnItems)) {
    const rowId = JSON.stringify([row.sourceThreadId, row.sourceItemId]);
    if (!turnItemIsOpen(client, rowId) && !turnItemIsOpen(client, str(obj(row.item).id))) continue;
    for (const image of toolImages(turnItemDetailView(client, obj(row.item), now, rowId).item)) wanted.set(image.id, image);
  }
  if (!wanted.size) return;
  let cache = urls.get(client);
  if (!cache) { cache = new Map(); urls.set(client, cache); }
  const scope = owner(client);
  for (const image of wanted.values()) {
    const key = `${scope}\u0000${image.id}`, previous = cache.get(key);
    if (previous && now - previous.at < 5 * 60_000) continue;
    let url = '';
    try {
      const reply = obj(await client.rpc(native, 'assets.createUrl', { resource: image.resource }));
      url = assetUrl(client.origin, str(reply.relativeUrl));
    } catch { /* The row keeps its unavailable state until the next refresh. */ }
    cache.set(key, { owner: scope, url, at: now });
    if (cache.size > 128) cache.delete(cache.keys().next().value!);
  }
}
