// Tool output image previews from T3 Code 9c161462c9 V2ItemInspector.tsx (MIT, LICENSE-T3).
import { arr, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { ClientError, type Native } from '../client/protocol';
import { assetUrl } from '../settings/settings-b-icons';
import { encodeMediaSource } from '../media/media-actions';
import { turnItemDetailView, turnItemIsOpen } from './timeline-item-fetch';
import { turnItemOutputImages } from './tool-output-images';
export type ToolImage = { id: string; name: string; source: string; resource: Obj };
export function toolImages(item: Obj): ToolImage[] {
  return turnItemOutputImages(item).map((resource, index) => {
    const id = `tool-image:${JSON.stringify([resource.threadId, resource.itemId, index])}`, name = `Tool output image ${index + 1}`;
    return { id, name, resource, source: encodeMediaSource({ kind: 'image', name, src: null, asset: { resource } }) };
  });
}
interface Preview { owner: string; row: string; images: ToolImage[]; index: number }
const previews = new WeakMap<T3Client, Preview>();
const urls = new WeakMap<T3Client, Map<string, { url: string; at: number }>>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.origin, client.threadId]);
function active(client: T3Client): Preview | undefined {
  const value = previews.get(client);
  if (value?.owner === owner(client)) return value;
  previews.delete(client); return undefined;
}
function loadedImages(client: T3Client, rowId: string): ToolImage[] {
  const row = arr(client.projection.visibleTurnItems).find(row => JSON.stringify([row.sourceThreadId, row.sourceItemId]) === rowId);
  return row ? toolImages(turnItemDetailView(client, obj(row.item), undefined, rowId).item) : [];
}
export function toolImageAction(client: T3Client, op: string, id: string, value: string): boolean {
  if (op === 'image-close') { previews.delete(client); return false; }
  if (op === 'image-tool-open') {
    const images = loadedImages(client, value), index = images.findIndex(image => image.id === id);
    if (index < 0) throw new ClientError('That image is no longer available.');
    previews.set(client, { owner: owner(client), row: value, images, index }); return true;
  }
  const preview = active(client);
  if (op === 'image-step' && preview) { preview.index = (preview.index + (value === 'previous' ? -1 : 1) + preview.images.length) % preview.images.length; return true; }
  if (op === 'image-open' || op === 'image-md-open') previews.delete(client);
  return false;
}
export function toolImagePreview(client: T3Client) {
  const preview = active(client), image = preview?.images[preview.index];
  return image && preview ? { imagePreviewId: image.id, imagePreviewName: image.name, imagePreviewPosition: preview.images.length > 1 ? `(${preview.index + 1}/${preview.images.length})` : '',
    imagePreviewPrevious: preview.images.length > 1, imagePreviewNext: preview.images.length > 1, imagePreviewVideo: false,
    imagePreviewSource: image.source, imagePreviewTip: image.name, imagePreviewReturn: `tool-image-${image.id}` } : null;
}
export async function toolImageUrls(client: T3Client, native: Native, now: number) {
  const wanted = new Map<string, ToolImage>();
  for (const row of arr(client.projection.visibleTurnItems)) {
    const id = JSON.stringify([row.sourceThreadId, row.sourceItemId]);
    if (!turnItemIsOpen(client, id) && !turnItemIsOpen(client, str(obj(row.item).id))) continue;
    for (const image of loadedImages(client, id)) wanted.set(image.id, image);
  }
  for (const image of active(client)?.images ?? []) wanted.set(image.id, image);
  let cache = urls.get(client);
  if (!cache) { cache = new Map(); urls.set(client, cache); }
  const result: { id: string; url: string }[] = [], origin = client.origin;
  for (const image of wanted.values()) {
    const key = JSON.stringify([client.generation, client.environmentId, origin, image.id]), cached = cache.get(key);
    if (cached && now - cached.at < 5 * 60_000) { result.push({ id: cached.url ? image.id : `failed:${image.id}`, url: cached.url }); continue; }
    let url = '';
    try { const reply = await client.rpc(native, 'assets.createUrl', { resource: image.resource }); url = assetUrl(origin, str(reply.relativeUrl)); } catch { /* The preview exposes the unavailable state. */ }
    cache.set(key, { url, at: now });
    if (cache.size > 128) cache.delete(cache.keys().next().value!);
    result.push({ id: url ? image.id : `failed:${image.id}`, url });
  }
  return result;
}
