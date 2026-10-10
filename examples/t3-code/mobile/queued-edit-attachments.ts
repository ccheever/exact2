// Pinned365aa87982 appendComposerDraftAttachments: ordered dedicated content,
// captured insertion, actual native Photo Library/Choose Files bytes.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fileStagingLimit } from './shared/composer-editor-files';
import { contextLabel, contextLink } from './shared/composer-editor-menu';
import { mobileQueuedEditCurrent, mobileQueuedEditLookup, mobileQueuedEditPersist, mobileQueuedEditWriteContent,
  queuedEditNative, queuedEditReplaceAttachments, queuedEditSessionNoticeOwner, queuedEditSetNotice, type MobileQueuedEditAttachment } from './queued-edit-state';
import { mobileComposerStageSelection } from './voice-data';
import { mobileComposerTarget } from './composer-target';
import { queuedEditImageMime } from './queued-edit-upload';
const picking = new WeakSet<T3Client>();
export const mobileQueuedEditAttachmentPicking=(client:T3Client):boolean=>picking.has(client);
const uuid = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i;
async function invoke(native: Native, input: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, input);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  return obj(reply.value);
}
/** Retained attachments remove only their server reference. New local bytes
 * become release obligations in native CAS after durable ownership changes. */
export async function mobileQueuedEditAttachmentAction(source: string, id: string, owner: string,
  nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const result = (message = '') => ({ revision: client.revision, message });
  const edit = mobileQueuedEditLookup(owner, client);
  if (!edit || mobileQueuedEditCurrent(client)?.owner !== owner) return result('That queued edit has ended.');
  if (edit.saving || picking.has(client)) return result('Wait for the queued edit to finish.');
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to attach files.');
  const target = mobileComposerTarget(client);
  const native = letGoAware(mobileNative(nativeInput)), generation = client.generation, environmentId = client.environmentId;
  const current = () => {
    const live = mobileQueuedEditLookup(owner, client);
    if (!live || live.saving || client.generation !== generation || client.environmentId !== environmentId) throw new ClientError('The queued edit changed.', 'superseded');
    return live;
  };
  let picked: Obj[] = [];
  picking.add(client);
  try {
    if (source === 'remove-retained') {
      queuedEditReplaceAttachments(owner, edit.attachments, edit.existingAttachments.filter(file => file.id !== id), client);
    } else if (source === 'remove-image' || source === 'remove-file') {
      const live = current(), removedContextIDs = arr(live.context?.records).filter(record => record.attachmentId === id).map(record => str(record.contextId));
      let text = live.text;
      for (const contextID of removedContextIDs) {
        const escaped = contextID.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        text = text.replace(new RegExp(`!?\\[[^\\]\\n]*\\]\\(t3-context://v1/(?:file|image)/${escaped}\\) ?`, 'g'), '');
      }
      const records = arr(live.context?.records).filter(record => record.attachmentId !== id);
      mobileQueuedEditWriteContent(owner, { text, context: records.length ? { version: 1, records } : undefined }, client);
      queuedEditReplaceAttachments(owner, current().attachments.filter(file => file.id !== id), current().existingAttachments, client);
    } else {
      const config = obj(obj(client.config.environment).capabilities), fileLimit = fileStagingLimit(config);
      const selection = await invoke(native, { op: 'mobileVoice', action: 'selection', owner: edit.owner, text: edit.text, optional: true }).catch(error => {
        if (letGo(error)) throw error; return { start: edit.text.length, end: edit.text.length };
      });
      current();
      if (!['photos', 'files'].includes(source)) throw new ClientError('Choose Photo Library or Choose Files.');
      if (source === 'files' && !fileLimit) throw new ClientError('This server does not support file attachments.');
      const remaining = Math.max(0, 100 - current().attachments.length - current().existingAttachments.length);
      if (!remaining) throw new ClientError('You can attach up to 100 attachments per message.');
      const response = await invoke(native, { op: 'composerAttachPick', source, remaining, fileLimit, generation });
      picked = arr(response.files); let live = current(), problems = str(response.error), accepted: MobileQueuedEditAttachment[] = [];
      const available = Math.max(0, Math.min(100 - live.attachments.length - live.existingAttachments.length, 200 - arr(live.context?.records).length));
      for (const raw of picked) {
        const name = str(raw.name, 'file'), id = str(raw.id), sizeBytes = Number(raw.sizeBytes), kind = raw.kind;
        if (!uuid.test(id) || !['image', 'file'].includes(str(kind)) || !Number.isFinite(sizeBytes) || sizeBytes <= 0) { problems ||= `'${name}' could not be attached.`; continue; }
        if (accepted.length >= available) { problems ||= 'You can attach up to 100 attachments per message.'; continue; }
        const imageMime = queuedEditImageMime({ name, mimeType: str(raw.mimeType) });
        const maxBytes = kind === 'image' ? 10 * 1024 * 1024 : fileStagingLimit(obj(obj(client.config.environment).capabilities));
        if (!maxBytes || sizeBytes > maxBytes || kind === 'image' && !imageMime) { problems ||= `'${name}' exceeds the attachment limit or is unsupported.`; continue; }
        accepted.push({ id, name, mimeType: imageMime || str(raw.mimeType, 'application/octet-stream'), sizeBytes,
          kind: kind as 'image' | 'file', uploadId: '', status: 'staged' });
      }
      if (accepted.length) {
        const records: Obj[] = accepted.map(file => ({ version: 1, contextId: file.id, kind: queuedEditImageMime(file) ? 'image' : 'file',
          label: contextLabel(file.name, file.kind), attachmentId: file.id, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes }));
        const insert = records.map(record => contextLink(str(record.kind), str(record.contextId), str(record.label))).join(' ');
        const start = live.text === edit.text ? Math.min(live.text.length, Math.max(0, Number(selection.start) || 0)) : live.text.length;
        const end = live.text === edit.text ? Math.min(live.text.length, Math.max(start, Number(selection.end) || start)) : start;
        const before = live.text.slice(0, start), after = live.text.slice(end), inserted = `${before && !/\s$/.test(before) ? ' ' : ''}${insert}${after && !/^\s/.test(after) ? ' ' : ''}`;
        mobileQueuedEditWriteContent(owner, { text: before + inserted + after, context: { version: 1, records: [...arr(live.context?.records), ...records] } }, client);
        live = current(); queuedEditReplaceAttachments(owner, [...live.attachments, ...accepted], live.existingAttachments, client);
        await mobileQueuedEditPersist(owner, native, client); current();
        // Session epoch is checked above; native matching text prevents a stale
        // selection token from moving another editor's caret.
        await mobileComposerStageSelection(client, target, current().text, before.length + inserted.length, before.length + inserted.length, native);
      }
      const owned = new Set(current().attachments.map(file => file.id));
      for (const raw of picked) if (uuid.test(str(raw.id)) && !owned.has(str(raw.id))) await invoke(native, {
        op: raw.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: raw.id, generation });
      return result(problems);
    }
    await mobileQueuedEditPersist(owner, native, client); await queuedEditNative(native, { action: 'release' }); return result();
  } catch (error) {
    const owned = new Set(mobileQueuedEditLookup(owner, client)?.attachments.map(file => file.id));
    for (const raw of picked) if (uuid.test(str(raw.id)) && !owned.has(str(raw.id))) {
      try { await invoke(native, { op: raw.kind === 'image' ? 'snapshotDraftRemove' : 'composerAttachRemove', id: raw.id, generation }); }
      catch { /* A revoked answer cannot issue further cleanup; native owns its byte cache. */ }
    }
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not attach files.';
    queuedEditSetNotice(client, message, queuedEditSessionNoticeOwner(edit)); return result(message);
  } finally { picking.delete(client); client.revision++; }
}
