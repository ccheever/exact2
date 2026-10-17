// Pinned AttachmentFileScreen's HTML-render branch, with its own tool-row owner.
// Ordinary attachment authorization remains in media-preview.ts.
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { EMPTY_ATTACHMENT_DOCUMENT, attachmentDocumentProjection, decodeAttachmentPrefix,
  type AttachmentDocumentCache, type AttachmentDocumentSnapshot } from './attachment-document';
import { mobileHtmlRenderDocumentNative, mobileHtmlRenderDocumentSource, mobileHtmlRenderRetireDocumentView,
  mobileHtmlRenderSelection, mobileHtmlRenderSnapshot, type HtmlRenderTheme } from './html-render-owner';

const caches = new WeakMap<T3Client, Map<string, AttachmentDocumentCache>>();

/** An opened tool page may read Source before a WKWebView ever mounts. */
export async function mobileHtmlRenderDocument(client: T3Client, context: string, key: string, attachmentId: string,
  rendered: boolean, theme: HtmlRenderTheme, iconColor: string, now: number, native: Native | null | undefined,
  nativeCovered = false): Promise<AttachmentDocumentSnapshot> {
  if (!native?.available) return { ...EMPTY_ATTACHMENT_DOCUMENT, error: 'Open T3 Code on your iPhone or iPad to preview files.' };
  const source = await mobileHtmlRenderDocumentSource(client, context, key, attachmentId, native);
  if (!source.ready) return { ...EMPTY_ATTACHMENT_DOCUMENT, error: source.error };
  if (!rendered || nativeCovered) mobileHtmlRenderRetireDocumentView(client, context, key);
  const guarded = mobileHtmlRenderDocumentNative(client, context, key, source.identifier, native);
  let cache = caches.get(client); if (!cache) { cache = new Map(); caches.set(client, cache); }
  let entry = cache.get(source.identifier);
  const uri = str(obj(JSON.parse(source.sourceJSON)).url);
  if (!entry) {
    entry = { name: source.name, kind: 'html', mimeType: 'text/html', sizeBytes: 0, content: null, contentError: '',
      sourceJSON: source.sourceJSON, uri, authorizedAt: now, dark: null, rows: [] };
    cache.set(source.identifier, entry);
    if (cache.size > 4) cache.delete(cache.keys().next().value!);
  } else if (entry.uri !== uri) {
    // Only the owner's actual render URL replaces the paired Source authorization.
    entry.uri = uri; entry.sourceJSON = source.sourceJSON; entry.authorizedAt = now; entry.contentError = '';
  }
  if (!rendered && entry.content === null && !entry.contentError) {
    try {
      let readSource = entry.sourceJSON;
      if (now - entry.authorizedAt > 5 * 60_000) {
        const fresh = await mobileHtmlRenderDocumentSource(client, context, key, attachmentId, guarded, true);
        if (!fresh.ready) throw new ClientError(fresh.error);
        readSource = fresh.sourceJSON;
        entry.sourceJSON = readSource; entry.authorizedAt = now;
      }
      const result = await bridgeReply(guarded, { op: 'mobileDocumentRead', sourceJSON: readSource, retry: false });
      mobileHtmlRenderDocumentNative(client, context, key, source.identifier, native);
      if (cache.get(source.identifier) !== entry) throw new ClientError('The file preview was replaced.', 'superseded');
      if (!result.ok) throw new ClientError(result.error!.message, result.error!.kind);
      if (obj(result.value).identifier !== source.identifier) throw new ClientError('The selected page changed.', 'superseded');
      entry.content = decodeAttachmentPrefix(result.value);
    } catch (error) {
      if (letGo(error)) throw error;
      entry.contentError = error instanceof Error ? error.message : 'Could not read this file.';
    }
  }
  mobileHtmlRenderDocumentNative(client, context, key, source.identifier, native);
  const snapshot = attachmentDocumentProjection(source.identifier, entry, 'html-render', rendered, theme.appearance === 'dark', true);
  return { ...snapshot, htmlRenderSource: rendered && !nativeCovered
    ? mobileHtmlRenderSnapshot(client, context, key, false, 0, theme, iconColor).sourceJSON : '' };
}

/** Source, share and native-viewer operations never enter the ordinary media ID path. */
export async function mobileHtmlRenderDocumentAction(client: T3Client, context: string, key: string, attachmentId: string,
  event: string, data: AttachmentDocumentSnapshot, native: Native | null | undefined) {
  const result = (message = '', operation = '', sourceJSON = '') => ({ identifier: data.identifier, message, operation, removed: false, sourceJSON });
  let value; try { value = obj(JSON.parse(event)); } catch { return result(); }
  if (!native?.available || !data.identifier || value.identifier !== data.identifier
    || mobileHtmlRenderSelection(client, key)?.reference.attachmentId !== attachmentId) return result();
  const guarded = mobileHtmlRenderDocumentNative(client, context, key, data.identifier, native);
  const operation = str(value.operation);
  try {
    if (['source', 'preview', 'word-wrap'].includes(operation)) return result('', operation);
    if (operation === 'copy' && data.hasContent) {
      const reply = await bridgeReply(guarded, { op: 'copyText', text: data.text });
      if (!reply.ok) throw new ClientError(reply.error!.message);
      return result(obj(reply.value).copied === false ? 'Copying is unavailable in this session.' : '');
    }
    if (operation === 'share' || operation === 'open-viewer') {
      const fresh = await mobileHtmlRenderDocumentSource(client, context, key, attachmentId, guarded, true);
      if (!fresh.ready) return result(fresh.error);
      if (operation === 'open-viewer') return result('', operation, fresh.sourceJSON);
      const reply = await bridgeReply(guarded, { op: 'mobileMediaShare', sourceJSON: fresh.sourceJSON });
      if (!reply.ok) throw new ClientError(reply.error!.message);
    }
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    return result(error instanceof Error ? error.message : 'The file action failed.');
  }
}
