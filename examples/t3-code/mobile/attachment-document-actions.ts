import { mobileComposerTarget } from './composer-target';
// AttachmentFileScreen365aa87982 menu actions over the document's captured owner.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileMediaPrepare, mobileMediaForget, mobileMediaShare, mobileMediaOwned } from './media-preview';
import { mobileComposerAttachmentAction } from './composer-attachments';
import type { AttachmentDocumentSnapshot } from './attachment-document';

export function mobileAttachmentMenu(data: AttachmentDocumentSnapshot, wrap: boolean) {
  const actions: { id: string; title: string; icon: string; selected?: boolean }[] = [];
  if (data.renderedMode) actions.push({ id: 'preview', title: data.renderedMode === 'table' ? 'Table' : 'Preview', icon: data.renderedMode === 'table' ? 'tablecells' : 'eye', selected: data.activeMode !== 'source' },
    { id: 'source', title: 'Source', icon: 'doc.text', selected: data.activeMode === 'source' });
  if (data.hasContent && data.activeMode === 'source') actions.push({ id: 'word-wrap', title: wrap ? 'Disable word wrap' : 'Enable word wrap', icon: 'text.alignleft' });
  if (data.hasContent) actions.push({ id: 'copy', title: data.truncated ? 'Copy preview' : 'Copy contents', icon: 'doc.on.doc' });
  if (data.sourceJSON) actions.push({ id: 'share', title: 'Save or share', icon: 'square.and.arrow.up' },
    { id: 'open-viewer', title: 'Open in file viewer', icon: 'arrow.up.left.and.arrow.down.right' });
  if (data.draft) actions.push({ id: 'remove', title: 'Remove from draft', icon: 'trash' });
  return { configuration: JSON.stringify({ identifier: data.identifier, actions }) };
}
export async function mobileAttachmentDocumentAction(event: string, scope: string, id: string, routeKey: string,
  data: AttachmentDocumentSnapshot, nativeInput: Native | null | undefined, storage: Files, client: T3Client = mobileClient) {
  let value; try { value = obj(JSON.parse(event)); } catch { value = {}; }
  const result = (message = '', operation = '', removed = false, sourceJSON = '') => ({ identifier: data.identifier, message, operation, removed, sourceJSON });
  if (!data.identifier || str(value.identifier) !== data.identifier || !nativeInput?.available) return result();
  const contentOwner = mobileComposerTarget(client).owner;
  const captured = JSON.stringify([client.generation, client.environmentId, client.threadId, mobileComposerTarget(client).owner]);
  const current = () => captured === JSON.stringify([client.generation, client.environmentId, client.threadId, mobileComposerTarget(client).owner]) && mobileMediaOwned(scope, id, client);
  const source = await mobileMediaPrepare(scope, id, routeKey, nativeInput, client);
  if (!source.ready || source.identifier !== data.identifier || !current()) return result();
  const operation = str(value.operation), native = letGoAware(mobileNative(nativeInput));
  try {
    if (['source', 'preview', 'word-wrap'].includes(operation)) return result('', operation);
    if (operation === 'open-viewer') {
      mobileMediaForget(source.identifier, client);
      const fresh = await mobileMediaPrepare(scope, id, routeKey, nativeInput, client);
      if (!current()) return result();
      return fresh.ready ? result('', operation, false, fresh.sourceJSON) : result(fresh.error);
    }
    if (operation === 'copy' && data.hasContent) {
      const reply = await bridgeReply(native, { op: 'copyText', text: data.text });
      if (!reply.ok) throw new ClientError(reply.error!.message);
      return result(obj(reply.value).copied === false ? 'Copying is unavailable in this session.' : '');
    }
    if (operation === 'share') {
      // Sharing always reauthorizes instead of depending on the preview's long-lived URL.
      mobileMediaForget(source.identifier, client);
      const reply = await mobileMediaShare(scope, id, routeKey, nativeInput, client);
      return result(reply.message);
    }
    if (operation === 'remove' && scope === 'composer') {
      const sourceKind = obj(JSON.parse(source.sourceJSON)).source;
      const reply = await mobileComposerAttachmentAction(sourceKind === 'remote' ? 'remove-retained' : sourceKind === 'draft-image' ? 'remove-image' : 'remove-file', id, nativeInput, storage, client, contentOwner);
      return result(reply.message, '', !reply.message && !mobileMediaOwned(scope, id, client));
    }
    return result();
  } catch (error) { if (letGo(error)) throw error; return result(error instanceof Error ? error.message : 'The file action failed.'); }
}
