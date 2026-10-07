import { mobileComposerTarget } from './composer-target';
// AttachmentFileScreen/useAttachmentDocument at365aa87982, over the existing media owner.
// @ref llp/1107.005-composer-and-transcript.decision.md#media-presentation
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileMediaPrepare, mobileMediaForget, mobileMediaOwned } from './media-preview';
import { filePreviewKind, decodeFilePreviewText, FILE_TEXT_PREVIEW_MAX_BYTES } from './attachment-document-kind';
import { filePreviewDelimiter, parseDelimitedPreview } from './attachment-document-table';
import { attachmentDocumentPresentation } from './attachment-document-presentation';
import { reviewRow, reviewLineTokens, type ReviewRow } from './review-model';

export interface AttachmentDocumentSnapshot {
  identifier: string; name: string; subtitle: string; kind: string; ready: boolean; error: string;
  text: string; hasContent: boolean; truncated: boolean; tableTruncated: boolean;
  renderedMode: string; activeMode: string; sourceJSON: string; uri: string; draft: boolean;
  rows: ReviewRow[]; table: { id: string; header: boolean; cells: { id: string; text: string }[] }[];
}
type Cached = { name: string; kind: ReturnType<typeof filePreviewKind>; mimeType: string; sizeBytes: number;
  content: ReturnType<typeof decodeFilePreviewText> | null; contentError: string; sourceJSON: string;
  uri: string; authorizedAt: number; dark: boolean | null; rows: ReviewRow[] };
const caches = new WeakMap<T3Client, Map<string, Cached>>();
export const EMPTY_ATTACHMENT_DOCUMENT: AttachmentDocumentSnapshot = { identifier: '', name: '', subtitle: '', kind: '',
  ready: false, error: '', text: '', hasContent: false, truncated: false, tableTruncated: false, renderedMode: '',
  activeMode: 'source', sourceJSON: '', uri: '', draft: false, rows: [], table: [] };
const owner = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.projectId, client.threadId, mobileComposerTarget(client).owner]);

/** Native returns a bounded prefix. Decode using the exact pinned fatal UTF-8 policy. */
export function decodeAttachmentPrefix(value: unknown) {
  const encoded = str(obj(value).base64);
  if (encoded.length > Math.ceil((FILE_TEXT_PREVIEW_MAX_BYTES + 1) / 3) * 4)
    throw new ClientError('The file preview exceeded its byte limit.');
  const binary = atob(encoded);
  if (binary.length > FILE_TEXT_PREVIEW_MAX_BYTES + 1) throw new ClientError('The file preview exceeded its byte limit.');
  return decodeFilePreviewText(Uint8Array.from(binary, character => character.charCodeAt(0)));
}
// Pinned client-runtime/state/attachments.ts formatAttachmentSize.
function formattedSize(bytes: number) {
  return bytes >= 1024 * 1024 ? `${(bytes / (1024 * 1024)).toFixed(1)} MB` : `${Math.max(1, Math.ceil(bytes / 1024))} KB`;
}
function projection(identifier: string, entry: Cached, scope: string, rendered: boolean, dark: boolean, hasEnvironment: boolean): AttachmentDocumentSnapshot {
  const delimiter = filePreviewDelimiter(entry), table = entry.content && delimiter ? parseDelimitedPreview(entry.content.text, delimiter) : null;
  const presentation = attachmentDocumentPresentation({ kind: entry.kind, hasTable: table !== null, hasEnvironment, rendered });
  const needsText = entry.kind === 'text' || entry.kind === 'markdown' || entry.kind === 'html' && !rendered;
  if (entry.content && entry.dark !== dark) {
    const lines = entry.content.text.replace(/\r\n?/g, '\n').split('\n');
    const tokens = reviewLineTokens(lines.map((content, index) => ({ content, change: 'context', oldLineNumber: null, newLineNumber: index + 1 })), entry.name, dark);
    entry.rows = lines.map((text, index) => ({ ...reviewRow(`attachment-line:${index}`, 'line'), text: text.replace(/\t/g, '    '),
      newNumber: String(index + 1), tokens: tokens[index] ?? [] }));
    entry.dark = dark;
  }
  return { identifier, name: entry.name, subtitle: `${scope === 'composer' ? 'Draft attachment' : 'Attachment'}${entry.sizeBytes > 0 ? ` · ${formattedSize(entry.sizeBytes)}` : ''}`,
    kind: entry.kind, ready: true, error: needsText ? entry.contentError : '', text: entry.content?.text ?? '', hasContent: entry.content !== null,
    truncated: entry.content?.truncated ?? false, tableTruncated: table?.truncated ?? false,
    renderedMode: presentation.renderedMode ?? '', activeMode: presentation.activeMode, sourceJSON: entry.sourceJSON,
    uri: entry.uri, draft: scope === 'composer', rows: entry.rows,
    table: table?.rows.map((row, index) => ({ id: String(index), header: index === 0,
      cells: row.map((text, column) => ({ id: String(column), text })) })) ?? [] };
}

/** Root keys reads by route/mode/owner; an action explicitly supplies retry. No saved native handle. */
export async function mobileAttachmentDocument(scope: string, id: string, routeKey: string, rendered: boolean, dark: boolean,
  now: number, retry: boolean, nativeInput?: Native | null, client: T3Client = mobileClient): Promise<AttachmentDocumentSnapshot> {
  const captured = owner(client);
  const assertOwner = () => { if (owner(client) !== captured || !mobileMediaOwned(scope, id, client)) throw new ClientError('The selected attachment changed.', 'superseded'); };
  const source = await mobileMediaPrepare(scope, id, routeKey, nativeInput, client);
  assertOwner();
  if (!source.ready || !nativeInput?.available) return { ...EMPTY_ATTACHMENT_DOCUMENT, error: source.error };
  let cache = caches.get(client); if (!cache) { cache = new Map(); caches.set(client, cache); }
  let entry = cache.get(source.identifier);
  if (!entry || retry) {
    const metadata = obj(JSON.parse(source.sourceJSON));
    entry = { name: source.name, kind: filePreviewKind({ name: source.name, mimeType: str(metadata.mimeType) }),
      mimeType: str(metadata.mimeType), sizeBytes: Number(metadata.sizeBytes) || 0, content: null, contentError: '',
      sourceJSON: source.sourceJSON, uri: str(metadata.url), authorizedAt: now, dark: null, rows: [] };
    cache.set(source.identifier, entry);
    if (cache.size > 4) cache.delete(cache.keys().next().value!);
  }
  const needsText = entry.kind === 'text' || entry.kind === 'markdown' || entry.kind === 'html' && !rendered;
  if (needsText && !entry.content && !entry.contentError) {
    const native = letGoAware(mobileNative(nativeInput));
    try {
      let readSource = entry.sourceJSON;
      if (retry || now - entry.authorizedAt > 5 * 60_000) {
        mobileMediaForget(source.identifier, client);
        const fresh = await mobileMediaPrepare(scope, id, routeKey, nativeInput, client);
        assertOwner();
        if (!fresh.ready) throw new ClientError(fresh.error);
        readSource = fresh.sourceJSON;
      }
      const result = await bridgeReply(native, { op: 'mobileDocumentRead', sourceJSON: readSource, retry });
      assertOwner();
      if (cache.get(source.identifier) !== entry) throw new ClientError('The file preview was replaced.', 'superseded');
      if (!result.ok) throw new ClientError(result.error!.message, result.error!.kind);
      if (obj(result.value).identifier !== source.identifier) throw new ClientError('The selected attachment changed.', 'superseded');
      entry.content = decodeAttachmentPrefix(result.value);
    } catch (error) { if (letGo(error)) throw error; entry.contentError = error instanceof Error ? error.message : 'Could not read this file.'; }
  }
  assertOwner();
  return projection(source.identifier, entry, scope, rendered, dark, !!client.environmentId);
}
