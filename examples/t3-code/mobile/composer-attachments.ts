import { mobileNewTaskDraftLookup, mobileNewTaskDraftChanged, mobileNewTaskDraftPersisted, mobileNewTaskDraftQueueFiles } from './mobile-new-task-drafts';
import { mobileQueuedEditPresentation } from './queued-edit';
import { composerAttachmentPreview, composerAttachmentPreviewRequest, prepareComposerAttachmentPreviews } from './composer-attachment-previews';
import { mobileComposerTarget, mobileComposerTargetRequire, mobileComposerTargetCurrent } from './composer-target';
import { mobileQueuedEditCurrent } from './queued-edit-state';
import { mobileQueuedEditAttachmentAction } from './queued-edit-attachments';
// Photo Library / Choose Files at upstream365aa87982, over shared draft/file/upload ownership.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
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
export interface ComposerAttachmentsSnapshot { previewRequest: string; contentOwner: string; items: ComposerAttachment[]; canPick: boolean; supportsFiles: boolean; remaining: number; error: string; }
const errors = new WeakMap<T3Client, string>();
const picking = new WeakSet<T3Client>();
const imageTypes = new Set(['image/png', 'image/jpeg', 'image/gif', 'image/webp']);
/** Plain files retain inline shared references; media entries can feed the source's attachment strip. */
export function mobileComposerAttachments(client: T3Client = mobileClient, now = 0): ComposerAttachmentsSnapshot {
  const target = mobileComposerTarget(client), edit = mobileQueuedEditCurrent(client);
  if (edit) {
    const disabled = !mobileQueuedEditPresentation(client).canCancel || client.busy || !!client.pending || picking.has(client);
    const remaining = Math.max(0, MAX_ATTACHMENTS - edit.existingAttachments.length - edit.attachments.length);
    const items = [...edit.existingAttachments.map(file => ({ id: str(file.id), name: str(file.name),
      kind: str(file.mimeType).startsWith('image/') ? 'image' : str(file.mimeType).startsWith('video/') ? 'video' : 'file',
      mimeType: str(file.mimeType), size: formatAttachmentSize(Number(file.sizeBytes) || 0), preview: '',
      removeOperation: 'remove-retained', disabled })), ...edit.attachments.map(file => ({ id: file.id, name: file.name,
      kind: file.mimeType.startsWith('video/') ? 'video' : file.kind, mimeType: file.mimeType, size: formatAttachmentSize(file.sizeBytes),
      preview: '', removeOperation: file.kind === 'image' ? 'remove-snapshot' : 'editorlocal:r4c-video-remove', disabled }))];
    return { previewRequest: composerAttachmentPreviewRequest(client, items, now), contentOwner: target.owner, items: items.map(item => ({ ...item, preview: composerAttachmentPreview(client, item, now) })), canPick: !disabled && remaining > 0, supportsFiles: attachStagingLimit(client) > 0,
      remaining, error: errors.get(client) ?? '' };
  }
  const disabled = !!client.pending || client.busy || picking.has(client), remaining = Math.max(0, MAX_ATTACHMENTS - reservedAttachments(client));
  const images = client.snapshotDrafts.map(image => ({ id: str(image.id), name: str(image.name), kind: 'image',
    mimeType: str(image.mimeType), size: formatAttachmentSize(Number(image.sizeBytes) || 0), preview: '', removeOperation: 'remove-snapshot', disabled }));
  const files = referencedFiles(client.local, client.draftKey, client.draft).map(file => ({ id: file.id, name: file.name,
    kind: file.mimeType.startsWith('video/') ? 'video' : file.mimeType.startsWith('image/') ? 'image' : 'file', mimeType: file.mimeType,
    size: formatAttachmentSize(file.sizeBytes), preview: '', removeOperation: 'editorlocal:r4c-video-remove', disabled }));
  return { previewRequest: composerAttachmentPreviewRequest(client, [...images, ...files], now), contentOwner: target.owner, items: [...images, ...files].map(item => ({ ...item, preview: composerAttachmentPreview(client, item, now) })), canPick: !!client.projectId && !disabled && remaining > 0 && !activeInput(client),
    supportsFiles: attachStagingLimit(client) > 0, remaining, error: errors.get(client) ?? '' };
}

/** An actual native picker request; shared attachFiles owns acceptance, count limits and draft chip insertion. */
export async function mobileComposerAttachmentAction(source: string, id: string, nativeInput: Native | null | undefined, suppliedStorage: Files,
  client: T3Client = mobileClient, expectedOwner = '') {
  const result = (message = '') => ({ revision: client.revision, message });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to attach files.');
  const target = mobileComposerTargetRequire(client, expectedOwner);
  if (target.kind === 'queued-edit') return mobileQueuedEditAttachmentAction(source, id, target.editOwner, nativeInput, client);
  if (picking.has(client) || client.pending || client.busy) return result('Wait for the current submission before changing attachments.');
  const native = letGoAware(mobileNative(nativeInput)), storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
  const key = client.draftKey, environmentId = client.environmentId, generation = client.generation;
  const current = () => mobileComposerTargetCurrent(client, target) && key === client.draftKey && environmentId === client.environmentId && generation === client.generation;
  const independent = !!mobileNewTaskDraftLookup(client, key);
  const content = () => JSON.stringify([client.local.drafts[key], client.local.snapshotDrafts[key], draftFiles(client.local).filter(file => file.draftKey === key)]);
  let observed = content();
  const observe = () => {
    const next = content();
    if (independent && next !== observed) mobileNewTaskDraftChanged(client, key);
    observed = next;
  };
  const durable: Files = independent ? { fs: { ...storage.fs, atomicWriteFile: async (path, bytes) => {
    observe();
    const document = obj(JSON.parse(new TextDecoder().decode(bytes)));
    document.mobileNewTaskDrafts = mobileNewTaskDraftPersisted(client) as unknown as Obj;
    await storage.fs.atomicWriteFile(path, new TextEncoder().encode(JSON.stringify(document)));
  } } } : storage;
  let picked: Obj[] = [];
  const cleanup = async () => { for (const file of picked) if (str(file.id)) await bridgeReply(native,
    { op: file.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: file.id }).catch(() => undefined); };
  picking.add(client); errors.delete(client);
  try {
    if (source === 'remove-image' || source === 'remove-file') {
      const operation = source === 'remove-image' ? 'remove-snapshot' : 'editorlocal:r4c-video-remove';
      const assertCurrent = () => { if (!current()) throw new ClientError('The composer changed before the attachment could be removed.', 'superseded'); };
      const removedFile = draftFiles(client.local).find(file => file.draftKey === key && file.id === id);
      let deferredRelease = false;
      const guarded: Native = { available: native.available, watch: topic => native.watch(topic), later: async request => {
        observe();
        if (independent && !deferredRelease && removedFile && obj(request).op === 'composerAttachRemove' && obj(request).id === removedFile.id) {
          deferredRelease = true; mobileNewTaskDraftQueueFiles(client, [removedFile]);
          return { ok: true, generation: client.generation, value: {} };
        }
        assertCurrent(); const reply = await native.later(request); observe(); assertCurrent(); return reply;
      } };
      assertCurrent();
      const guardedStorage: Files = { fs: { ...durable.fs, atomicWriteFile: async (path, bytes) => { assertCurrent(); await durable.fs.atomicWriteFile(path, bytes); assertCurrent(); } } };
      const response = !independent && client === mobileClient ? await mobileCommand([operation, id, ''], guarded, suppliedStorage)
        : await client.command(operation, id, '', 0, guarded, guardedStorage);
      if (response.message) errors.set(client, response.message);
      return result(response.message);
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
      observe();
      if (request.op !== 'composerAttachPick') {
        if (!independent || request.op !== 'editorInsert') return native.later(input);
        // Shared image-only fallback reads the current key after this await.
        // Finish that insertion into the captured draft even if another opens.
        let reply: Obj | null = null;
        if (current()) { try { reply = obj(await native.later(input)); } catch { /* Keep accepted bytes and their reference. */ } }
        if (!reply?.ok || obj(reply.value).applied !== true) {
          const prompt = client.local.drafts[key] ?? '', text = str(request.text);
          if (mobileNewTaskDraftLookup(client, key)) client.local.drafts[key] = `${prompt}${prompt && !/\s$/.test(prompt) ? ' ' : ''}${text} `;
        }
        observe();
        return { ok: true, generation, value: { applied: true } };
      }
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
    observe();
    // Capabilities can change while a system picker is open. Release only rejected staged
    // bytes; accepted files remain owned even when persistence fails and the user retries.
    const owned = new Set([...(client.local.snapshotDrafts[key] ?? []).map(image => str(image.id)),
      ...draftFiles(client.local).map(file => file.id)]);
    for (const file of picked) if (str(file.id) && !owned.has(str(file.id))) {
      await bridgeReply(native, { op: file.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: file.id });
    }
    await client.persist(durable); // Bytes stay owned until durable shared draft state says otherwise.
    const message = pickError || client.error;
    if (message) errors.set(client, message);
    return result(message);
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not attach files.';
    errors.set(client, message); return result(message);
  } finally { observe(); picking.delete(client); client.revision++; }
}

/** Root refreshes previews only for actual owned draft IDs; payload is a bounded native thumbnail. */
export function mobileComposerAttachmentPreviews(nativeInput: Native | null | undefined, client: T3Client = mobileClient, now = 0, owner = '', request = '') {
  return prepareComposerAttachmentPreviews(nativeInput, client, () => mobileComposerAttachments(client, now).items, now, owner, request);
}
