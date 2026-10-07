// Photo Library / Choose Files at upstream365aa87982, over shared draft/file/upload ownership.
// @ref llp/1106.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileClient, mobileCommand, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, nativeFiles, type Native, type Files, ClientError } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { attachFiles, MAX_ATTACHMENTS } from './shared/composer-controls-attach';
import { attachStagingLimit, reservedAttachments } from './shared/composer-editor-attach';
import { draftFiles, referencedFiles, formatAttachmentSize } from './shared/composer-editor-files';
import { activeInput } from './shared/requests';

export interface ComposerAttachment { id: string; name: string; kind: string; size: string; mimeType: string; preview: string; removeOperation: string; disabled: boolean }
export interface ComposerAttachmentsSnapshot { items: ComposerAttachment[]; canPick: boolean; supportsFiles: boolean; remaining: number; error: string; }
const previews = new WeakMap<T3Client, Map<string, string>>();
const errors = new WeakMap<T3Client, string>();
const picking = new WeakSet<T3Client>();
const imageTypes = new Set(['image/png', 'image/jpeg', 'image/gif', 'image/webp']);
/** Plain files retain inline shared references; media entries can feed the source's attachment strip. */
export function mobileComposerAttachments(client: T3Client = mobileClient): ComposerAttachmentsSnapshot {
  const disabled = !!client.pending || client.busy || picking.has(client), remaining = Math.max(0, MAX_ATTACHMENTS - reservedAttachments(client));
  const cache = previews.get(client), images = client.snapshotDrafts.map(image => ({ id: str(image.id), name: str(image.name), kind: 'image',
    mimeType: str(image.mimeType), size: formatAttachmentSize(Number(image.sizeBytes) || 0), preview: cache?.get(str(image.id)) ?? '', removeOperation: 'remove-snapshot', disabled }));
  const files = referencedFiles(client.local, client.draftKey, client.draft).map(file => ({ id: file.id, name: file.name,
    kind: file.mimeType.startsWith('video/') ? 'video' : file.mimeType.startsWith('image/') ? 'image' : 'file', mimeType: file.mimeType,
    size: formatAttachmentSize(file.sizeBytes), preview: cache?.get(file.id) ?? '', removeOperation: 'editorlocal:r4c-video-remove', disabled }));
  return { items: [...images, ...files], canPick: !!client.projectId && !disabled && remaining > 0 && !activeInput(client),
    supportsFiles: attachStagingLimit(client) > 0, remaining, error: errors.get(client) ?? '' };
}

/** An actual native picker request; shared attachFiles owns acceptance, count limits and draft chip insertion. */
export async function mobileComposerAttachmentAction(source: string, id: string, nativeInput: Native | null | undefined, suppliedStorage: Files,
  client: T3Client = mobileClient) {
  const result = (message = '') => ({ revision: client.revision, message });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to attach files.');
  if (picking.has(client) || client.pending || client.busy) return result('Wait for the current submission before changing attachments.');
  const native = letGoAware(mobileNative(nativeInput)), storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
  const key = client.draftKey, environmentId = client.environmentId, generation = client.generation;
  const current = () => key === client.draftKey && environmentId === client.environmentId && generation === client.generation;
  let picked: Obj[] = [];
  const cleanup = async () => { for (const file of picked) if (str(file.id)) await bridgeReply(native,
    { op: file.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: file.id }).catch(() => undefined); };
  picking.add(client); errors.delete(client);
  try {
    if (source === 'remove-image' || source === 'remove-file') {
      const operation = source === 'remove-image' ? 'remove-snapshot' : 'editorlocal:r4c-video-remove';
      const response = client === mobileClient ? await mobileCommand([operation, id, ''], nativeInput, suppliedStorage)
        : await client.command(operation, id, '', 0, native, storage);
      if (response.message) errors.set(client, response.message);
      previews.get(client)?.delete(id); return result(response.message);
    }
    if (!client.projectId) throw new ClientError('Choose a project first.');
    if (activeInput(client)) throw new ClientError('Answer the pending question before attaching files here.');
    const remaining = Math.max(0, MAX_ATTACHMENTS - reservedAttachments(client));
    if (!remaining) throw new ClientError('You can attach up to 100 attachments per message.');
    if (source === 'menu') {
      const response = await bridgeReply(native, { op: 'mobileAttachmentSource', supportsFiles: attachStagingLimit(client) > 0 });
      if (!response.ok) throw new ClientError(response.error!.message);
      const value = obj(response.value); if (str(value.error)) throw new ClientError(str(value.error));
      source = str(value.source); if (!source) return result();
    }
    if (!['photos', 'files'].includes(source)) throw new ClientError('Choose Photo Library or Choose Files.');
    if (!current()) throw new ClientError('The draft changed while files were being chosen.');
    let pickError = '';
    const picker: Native = { available: true, watch: topic => native.watch(topic), later: async input => {
      const request = obj(input);
      if (request.op !== 'composerAttachPick') return native.later(input);
      const response = await bridgeReply(native, { ...request, source, remaining });
      picked = arr(obj(response.value).files); pickError = str(obj(response.value).error);
      // Preserve actual byte MIME/name after shared acceptance (desktop assumes PNG images).
      for (const file of picked) if (file.kind === 'image' && !imageTypes.has(str(file.mimeType))) {
        if (str(file.id)) await bridgeReply(native, { op: 'snapshotDraftRemove', id: file.id });
        file.kind = 'unsupported-image'; delete file.id;
      }
      if (!current()) { await cleanup(); throw new ClientError('The draft changed while files were being chosen.'); }
      return { ...response, value: { ...obj(response.value), files: picked } };
    } };
    await attachFiles(client, picker);
    for (const image of client.local.snapshotDrafts[key] ?? []) {
      const actual = picked.find(file => file.kind === 'image' && file.id === image.id);
      if (actual) { image.mimeType = str(actual.mimeType); image.name = str(actual.name); }
    }
    // Capabilities can change while a system picker is open. Release only rejected staged
    // bytes; accepted files remain owned even when persistence fails and the user retries.
    const owned = new Set([...(client.local.snapshotDrafts[key] ?? []).map(image => str(image.id)),
      ...draftFiles(client.local).map(file => file.id)]);
    for (const file of picked) if (str(file.id) && !owned.has(str(file.id))) {
      await bridgeReply(native, { op: file.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: file.id });
    }
    await client.persist(storage); // Bytes stay owned until durable shared draft state says otherwise.
    const message = pickError || client.error;
    if (message) errors.set(client, message);
    return result(message);
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not attach files.';
    errors.set(client, message); return result(message);
  } finally { picking.delete(client); client.revision++; }
}

/** Root refreshes previews only for actual owned draft IDs; payload is a bounded native thumbnail. */
export async function mobileComposerAttachmentPreviews(nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  if (!nativeInput?.available) return { revision: client.revision };
  const native = letGoAware(mobileNative(nativeInput)), key = client.draftKey;
  let cache = previews.get(client); if (!cache) { cache = new Map(); previews.set(client, cache); }
  const items = mobileComposerAttachments(client).items;
  for (const item of items.filter(item => item.kind === 'image' && !cache!.has(item.id))) {
    const response = await bridgeReply(native, { op: 'mobileAttachmentPreview', id: item.id, image: item.removeOperation === 'remove-snapshot' });
    if (key !== client.draftKey) break;
    if (response.ok) cache.set(item.id, str(obj(response.value).dataUrl));
  }
  const live = new Set(items.map(item => item.id)); for (const id of cache.keys()) if (!live.has(id)) cache.delete(id);
  return { revision: client.revision };
}
