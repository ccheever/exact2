// Lane r5-panels: a sent attachment opened beside the thread (MIT reference,
// see LICENSE-T3: ChatView.tsx openFileAttachment → rightPanelStore.openAttachment
// (an `attachment:<id>` file surface that replaces the standalone explorer tab),
// components/files/AttachmentFilePreview.tsx and @t3tools/shared filePreview /
// delimitedPreview). The captured bytes are read from a signed asset URL
// (`assets.createUrl`, re-minted after five minutes), never from a workspace
// file of the same name; the subheader reads "Attachment › name  size" with the
// rendered/source toggle, word wrap, Copy contents and Save file.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { assetUrl } from './settings-b-icons';
import { formatAttachmentSize } from './composer-editor-files';
import { markdownDocument, tableRows, type Document } from './r4-surfaces-render';
import { codeLines, type CodeLine } from './r4-surfaces-files';
import { fileIconToken } from './timeline-files';
import { mediaBody, mediaErrorMessage } from './r6-media-preview'; // lane r6-media: PDF, HTML, audio and video bodies
import { sourceGutter } from './r9-device-crumbs'; // lane r9-device: the reference's line-number column

export type AttachmentMeta = { id: string; name: string; mimeType: string; sizeBytes: number };
export type AttachmentView = {
  id: string; name: string; size: string; preview: string; error: string; canRender: boolean; rendered: boolean; renderLabel: string; renderIcon: string;
  showWrap: boolean; wrap: boolean; canCopy: boolean; copyLabel: string; copied: boolean; canSave: boolean; saving: boolean; saveLabel: string;
  truncatedNote: string; markdown: Document; table: { id: string; header: boolean; cells: { id: string; text: string; syntax: string }[] }[];
  lines: CodeLine[]; gutter: number; url: string; noPreview: string; code: never[];
};
type Content = { text: string; truncated: boolean };
type State = { url: string; urlAt: number; urlError: string; content: Content | null; contentError: string; rendered: boolean; saving: boolean; copiedAt: number; mediaError: string };
const states = new WeakMap<T3Client, Map<string, State>>();
const STALE_URL_MS = 5 * 60_000; // AttachmentFilePreview: signed URLs live an hour; re-mint after five minutes

/** filePreviewKind (shared/filePreview.ts): content classification from the MIME type, then the name. */
export function previewKind(file: { name: string; mimeType?: string }): string {
  const mime = (file.mimeType ?? '').split(';', 1)[0]!.trim().toLowerCase(), name = file.name.toLowerCase(), extension = name.slice(name.lastIndexOf('.'));
  const generic = !mime || mime === 'application/octet-stream' || mime === 'text/plain';
  if (mime === 'application/pdf') return 'pdf';
  if (mime === 'text/html') return 'html';
  if (mime === 'text/markdown' || mime === 'text/x-markdown') return 'markdown';
  if (mime.startsWith('image/')) return 'image';
  if (mime.startsWith('video/')) return 'video';
  if (mime.startsWith('audio/')) return 'audio';
  if (generic) {
    if (extension === '.pdf') return 'pdf';
    if (/^\.html?$/.test(extension)) return 'html';
    if (/^\.(md|markdown|mdown|mkd|mdx)$/.test(extension)) return 'markdown';
    if (/^\.(avif|gif|ico|jpeg|jpg|png|svg|webp)$/.test(extension)) return 'image';
    if (/^\.(mp4|m4v|mov|webm|ogv)$/.test(extension)) return 'video';
    if (/^\.(mp3|wav|ogg|oga|flac|aac|m4a|opus|aiff)$/.test(extension)) return 'audio';
    if (/^\.(txt|log|json|jsonc|jsonl|ndjson|yaml|yml|toml|ini|conf|config|env|csv|tsv|xml|css|scss|sass|less|js|jsx|mjs|cjs|ts|tsx|mts|cts|py|pyi|rb|go|rs|swift|kt|kts|java|c|h|cc|cpp|hpp|cs|php|sh|bash|zsh|fish|sql|graphql|gql|vue|svelte|r|lua|ex|exs|erl|hs|clj|dart|diff|patch|lock|properties|gradle)$/.test(extension)
      || /^(dockerfile|makefile|gemfile|rakefile|license|readme|\.gitignore|\.gitattributes|\.editorconfig|\.env)(\.|$)/i.test(name.split(/[\\/]/).pop() ?? '')) return 'text';
  }
  if (mime.startsWith('text/') || /^(application\/(json|.*\+json|xml|.*\+xml|javascript|x-javascript|yaml|x-yaml|toml|sql))$/.test(mime)) return 'text';
  return 'unsupported';
}
/** filePreviewDelimiter (shared/delimitedPreview.ts). */
export function previewDelimiter(file: { name: string; mimeType?: string }): ',' | '\t' | null {
  const mime = (file.mimeType ?? '').split(';', 1)[0]!.trim().toLowerCase();
  if (mime === 'text/csv') return ',';
  if (mime === 'text/tab-separated-values') return '\t';
  if (mime && mime !== 'text/plain' && mime !== 'application/octet-stream') return null;
  return /\.csv$/i.test(file.name) ? ',' : /\.tsv$/i.test(file.name) ? '\t' : null;
}

/** The sent file attachment with this id in the thread's loaded history (ChatFileAttachment). */
export function findAttachment(client: T3Client, id: string): AttachmentMeta | null {
  for (const row of arr(client.projection.visibleTurnItems)) {
    const item = obj(row.item);
    if (item.type !== 'user_message') continue;
    const found = arr(item.attachments).find(attachment => str(attachment.id) === id && attachment.type === 'file');
    if (found) return { id, name: str(found.name), mimeType: str(found.mimeType), sizeBytes: num(found.sizeBytes) };
  }
  return null;
}
export function requireAttachment(client: T3Client, id: string): AttachmentMeta {
  const meta = findAttachment(client, id);
  if (!meta) throw new ClientError('That attachment is no longer available.');
  return meta;
}

function stateOf(client: T3Client, id: string): State {
  let map = states.get(client);
  if (!map) { map = new Map(); states.set(client, map); }
  let state = map.get(id);
  // AttachmentFilePreview opens rendered.
  if (!state) { state = { url: '', urlAt: 0, urlError: '', content: null, contentError: '', rendered: true, saving: false, copiedAt: 0, mediaError: '' }; map.set(id, state); }
  return state;
}
async function mint(client: T3Client, native: Native, meta: AttachmentMeta, disposition: 'inline' | 'attachment'): Promise<string> {
  const result = obj(await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'attachment', attachmentId: meta.id,
    ...(meta.name ? { fileName: meta.name } : {}), ...(meta.mimeType ? { mimeType: meta.mimeType } : {}), disposition } }));
  const url = assetUrl(client.origin, str(result.relativeUrl));
  if (!url) throw new ClientError('Reconnect to the environment and try again.');
  return url;
}
const message = (error: unknown, fallback: string) => (error instanceof Error && error.message ? error.message : fallback);

/** The surface's projection: the URL (minted on first view), the text body for text kinds, and the header's controls. */
export async function attachmentView(client: T3Client, native: Native | null | undefined, meta: AttachmentMeta, now: number): Promise<AttachmentView> {
  const state = stateOf(client, meta.id), kind = previewKind(meta), delimiter = previewDelimiter(meta);
  const renderedMode = kind === 'markdown' ? 'markdown' : kind === 'html' ? 'html' : delimiter ? 'table' : '';
  const needsText = kind === 'text' || kind === 'markdown' || (kind === 'html' && !state.rendered);
  if (native?.available && client.ready) {
    if (!state.urlError && (!state.url || (needsText && !state.content && !state.contentError && now && now - state.urlAt > STALE_URL_MS))) {
      try { state.url = await mint(client, native, meta, 'inline'); state.urlAt = now; }
      catch (error) { state.urlError = message(error, 'The attachment is unavailable.'); }
    }
    if (needsText && state.url && !state.content && !state.contentError) {
      const reply = obj(await client.restAccess(native).call({ op: 'attachmentText', url: state.url }).catch(error => ({ ok: false, message: message(error, 'Could not load this file.') })));
      if (reply.ok === true) state.content = { text: str(reply.text), truncated: reply.truncated === true };
      else state.contentError = str(reply.message, 'Could not load this file.');
    }
  }
  const failure = state.urlError || (needsText ? state.contentError : state.mediaError);
  const content = state.content, rendered = state.rendered;
  const showsRawText = !failure && needsText && !!content && !(delimiter && rendered) && !(kind === 'markdown' && rendered);
  const preview = failure ? 'error' : !state.url || (needsText && !content) ? 'loading'
    : needsText && content ? (delimiter && rendered ? 'table' : kind === 'markdown' && rendered ? 'markdown' : 'code')
      : kind === 'image' ? 'image' : mediaBody(kind, rendered) || 'none';
  const text = content?.text ?? '';
  const table = preview === 'table' ? tableRows(meta.name, text) : null;
  const lines = preview === 'code' ? codeLines(meta.name, text) : [];
  const extension = meta.name.split('.').pop() || 'this format';
  // useCopyToClipboard: "Copied" for two seconds of the window's clock, stamped by the first view after the copy.
  if (state.copiedAt < 0 && now) state.copiedAt = now;
  const copied = state.copiedAt < 0 || (state.copiedAt > 0 && now - state.copiedAt < 2000);
  return {
    id: meta.id, name: meta.name, size: formatAttachmentSize(meta.sizeBytes), preview, error: failure,
    canRender: !!renderedMode, rendered,
    renderLabel: renderedMode === 'markdown' ? (rendered ? 'Show markdown source' : 'Show rendered markdown') : renderedMode === 'table' ? (rendered ? 'Show source' : 'Show table') : renderedMode === 'html' ? (rendered ? 'Show HTML source' : 'Show rendered page') : '',
    renderIcon: rendered ? 'code' : renderedMode === 'table' ? 'table' : 'eye',
    showWrap: showsRawText, wrap: client.local.clientSettings.wordWrap !== false,
    canCopy: !!content, copyLabel: copied ? 'Copied' : content?.truncated ? 'Copy preview' : 'Copy contents', copied,
    canSave: !!state.url, saving: state.saving, saveLabel: state.saving ? 'Preparing file…' : 'Save file',
    truncatedNote: content?.truncated ? `Preview limited to the first 1 MB of a ${meta.sizeBytes.toLocaleString('en-US')} byte file. Save the file to read it in full.` : '',
    markdown: preview === 'markdown' ? markdownDocument(`attachment:${meta.id}`, text) : { id: '', blocks: [] }, table: table?.rows ?? [],
    lines, gutter: sourceGutter(lines.length), url: preview === 'image' || mediaBody(preview, true) ? state.url : '',
    // The unsupported body.
    noPreview: preview === 'none' ? `Save it to open in an app that supports ${extension} files.` : '', code: [],
  };
}
export const emptyAttachment = (): AttachmentView => ({
  id: '', name: '', size: '', preview: '', error: '', canRender: false, rendered: false, renderLabel: '', renderIcon: '', showWrap: false, wrap: true, canCopy: false,
  copyLabel: 'Copy contents', copied: false, canSave: false, saving: false, saveLabel: 'Save file', truncatedNote: '', markdown: { id: '', blocks: [] }, table: [], lines: [], gutter: 0, url: '', noPreview: '', code: [],
});
export const attachmentToken = (name: string) => fileIconToken(name);

/** `surface-r5-att-*`: the header's controls. */
export async function attachmentLocal(client: T3Client, native: Native, op: string, id: string, now: number): Promise<string> {
  const meta = requireAttachment(client, id), state = stateOf(client, id);
  if (op === 'render') { state.rendered = !state.rendered; if (previewKind(meta) === 'html') state.contentError = ''; return ''; }
  if (op === 'wrap') { client.local.clientSettings.wordWrap = client.local.clientSettings.wordWrap === false; return ''; }
  if (op === 'retry') { state.url = ''; state.urlError = ''; state.content = null; state.contentError = ''; state.mediaError = ''; return ''; }
  // lane r6-media: the audio/video player's `error` event; a late one from a body already gone is ignored.
  if (op === 'media-error') { if (state.url && !state.mediaError) state.mediaError = mediaErrorMessage(previewKind(meta)); return ''; }
  if (op === 'copy') {
    if (!state.content) return '';
    try { await client.restAccess(native).call({ op: 'copyText', text: state.content.text }); state.copiedAt = now || -1; }
    catch (error) { pushToast(client, { kind: 'error', title: 'Could not copy', description: message(error, 'Please try again.') }); }
    return '';
  }
  if (op === 'save') {
    if (state.saving) return '';
    state.saving = true;
    try {
      const url = await mint(client, native, meta, 'attachment');
      const reply = obj(await client.restAccess(native).call({ op: 'attachmentSave', url, name: meta.name }));
      if (reply.ok !== true) throw new ClientError(str(reply.message, 'Please try again.'));
    } catch (error) {
      pushToast(client, { kind: 'error', title: 'Could not save file', description: message(error, 'Please try again.') });
    } finally { state.saving = false; }
    return '';
  }
  return '';
}
