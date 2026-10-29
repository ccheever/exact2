// A sent attachment opened beside the thread (MIT reference,
// see LICENSE-T3: ChatView.tsx openFileAttachment → rightPanelStore.openAttachment
// (an `attachment:<id>` file surface that replaces the standalone explorer tab),
// components/files/AttachmentFilePreview.tsx and @t3tools/shared filePreview /
// delimitedPreview; T3 Code 7c3b3b3605ef). The captured bytes are read from a signed asset URL
// (`assets.createUrl`, re-minted after five minutes), never from a workspace
// file of the same name; the subheader reads "Attachment › name  size" with the
// rendered/source toggle, word wrap, Copy contents and Save file. Its rendered
// Markdown is ChatMarkdown with no `cwd`, a Mermaid file is its diagram, an image has the size and zoom bar
// (preview-image.contract) and a font its specimen (font-preview.ts). The reference's lazy-loaded document viewers
// (Office, OpenDocument, RTF, EPUB, notebooks, archives, SQLite) are not here yet: those files say "No preview for this
// file", and a notebook shows its JSON source. Files shows a document, or a file it cannot read as text, in the same
// body under "File" (files-surface.ts).
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { assetUrl } from '../settings/settings-b-icons';
import { formatAttachmentSize } from '../composer/composer-editor-files';
import { tableRows } from './rendered-previews';
import { codeLines, type CodeLine, type MarkdownSource } from './files-surface';
import { markdownEnv, messageChips, type MarkdownEnv } from '../timeline/message-chips';
import { withFileLinkMenus } from '../timeline/file-link-menu';
import { messageCodeBlocks } from '../highlight/timeline-highlight';
import { filesMermaid, type MermaidDiagramView } from '../timeline/timeline-mermaid';
import { timelineView } from '../timeline/timeline-presentation';
import { fileIconToken } from '../shared/timeline-files';
import { mediaBody, mediaErrorMessage } from '../media/attachment-media'; // PDF, HTML, audio and video bodies
import { mediaFrameSource, stagedCopyName } from '../media/media-frame'; // a draft PDF's or a rendered page's frame source
import { sourceGutter } from './files-crumbs-settle'; // the reference's line-number column
import { letGo } from '../client/let-go';
import { attachmentText, saveAttachment } from './attachment-files'; // the preview's text and Save file
import { filePreviewDelimiter, filePreviewKind, maskMarkdownFrontmatter, type FilePreviewKind } from './file-preview-kind';
import { mermaidFile } from './mermaid-file'; // a Mermaid file's diagram
import { FONT_REFUSED, fontPage, fontPageKept, forgetFontPage } from './font-preview'; // a font's specimen
import { openInlineDocumentImage } from '../timeline/timeline-attachments'; // the expanded image dialog

export type AttachmentMeta = { id: string; name: string; mimeType: string; sizeBytes: number };
type Content = { text: string; truncated: boolean };
/**
 * Where a preview's bytes come from. A sent attachment (and a draft file the send already uploaded) is the server's,
 * read at a signed asset URL; a draft's staged file or folded paste is this device's (AttachmentFilePreview's `file`
 * Blob): `local.url` names it to the media bodies, `text` reads its first megabyte and `save` saves it. A file Files
 * shows in this body (FilePreviewPanel's AttachmentFilePreview, origin "File") is signed as its `resource` instead of an
 * attachment: a `media-file`, or a draft's `draft-workspace-file`. `id` names the preview to the window and `key` keeps
 * its state; `origin` is the first crumb, and a draft's preview in its dialog has Remove from draft and Close.
 */
export type PreviewSource = {
  id: string; key: string; origin: string; dialog: boolean;
  local: { url: string; text: () => Promise<Obj>; save: () => Promise<void> | void } | null;
  resource?: Obj;
};
const sentSource = (meta: AttachmentMeta): PreviewSource => ({ id: meta.id, key: meta.id, origin: 'Attachment', dialog: false, local: null });
export type AttachmentView = {
  id: string; origin: string; canRemove: boolean; canClose: boolean; name: string; size: string; preview: string; error: string; canRender: boolean; rendered: boolean; renderLabel: string; renderIcon: string;
  showWrap: boolean; wrap: boolean; canCopy: boolean; copyLabel: string; copied: boolean; canSave: boolean; saving: boolean; saveLabel: string;
  truncatedNote: string; table: { id: string; header: boolean; cells: { id: string; text: string; syntax: string }[] }[];
  lines: CodeLine[]; gutter: number; url: string; noPreview: string;
  // The rendered Markdown as Files' (files-surface.ts FilesView): renderFileMarkdown's one
  // message (kind "attachment"), its chips and settings, code colours, Mermaid fences, host-path media and Copy code's check.
  markdownSource: MarkdownSource[]; md: MarkdownEnv; code: ReturnType<typeof messageCodeBlocks>; diagrams: MermaidDiagramView[];
  mdUrls: { id: string; url: string; fill: string; hover: string; border: string; ink: string }[]; codeCopied: string; codeCopyNonce: number;
  /** A Mermaid file's refusal (MermaidViewer), shown instead of its diagram; `diagrams` is then empty. */
  mermaidNote: string;
  /** "No preview for this file" reports a failure (FontViewer's), not only a format. */
  noPreviewAlert: boolean;
};
type State = { url: string; urlAt: number; urlError: string; content: Content | null; contentError: string; rendered: boolean; saving: boolean; copiedAt: number; mediaError: string;
  /** Why the font has no specimen: its read failed, or its frame's FontFace did not load it; kept while it is shown. */
  fontError: string };
const states = new WeakMap<T3Client, Map<string, State>>();
const STALE_URL_MS = 5 * 60_000; // AttachmentFilePreview: signed URLs live an hour; re-mint after five minutes

/** filePreviewKind, as composer-chip-file.ts reads it. */
export const previewKind = (file: { name: string; mimeType?: string }): FilePreviewKind => filePreviewKind(file);
/**
 * The body a kind draws here. The reference's document viewers are not here yet: a notebook shows its JSON source,
 * which it did before it had a viewer, and the other documents say "No preview for this file".
 */
export function previewBody(kind: FilePreviewKind): FilePreviewKind {
  if (kind === 'ipynb') return 'text';
  return ['archive', 'sqlite', 'docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'rtf', 'epub'].includes(kind) ? 'unsupported' : kind;
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
  if (!state) { state = { url: '', urlAt: 0, urlError: '', content: null, contentError: '', rendered: true, saving: false, copiedAt: 0, mediaError: '', fontError: '' }; map.set(id, state); }
  return state;
}
/** AttachmentFilePreview's `resource`, or its `downloadResource` for Save file: a `media-file` saved as an attachment. */
function resourceOf(meta: AttachmentMeta, source: PreviewSource, disposition: 'inline' | 'attachment'): Obj {
  if (source.resource) return disposition === 'attachment' && source.resource._tag === 'media-file' ? { ...source.resource, disposition } : source.resource;
  return { _tag: 'attachment', attachmentId: meta.id, ...(meta.name ? { fileName: meta.name } : {}), ...(meta.mimeType ? { mimeType: meta.mimeType } : {}), disposition };
}
async function mint(client: T3Client, native: Native, meta: AttachmentMeta, source: PreviewSource, disposition: 'inline' | 'attachment'): Promise<string> {
  const result = obj(await client.rpc(native, 'assets.createUrl', { resource: resourceOf(meta, source, disposition) }));
  const url = assetUrl(client.origin, str(result.relativeUrl));
  if (!url) throw new ClientError('Reconnect to the environment and try again.');
  return url;
}
const message = (error: unknown, fallback: string) => (error instanceof Error && error.message ? error.message : fallback);
const NO_MARKDOWN = () => ({ markdownSource: [], md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [], threadGlyphs: [], runCommands: [] }, code: [], diagrams: [] as MermaidDiagramView[], mdUrls: [], codeCopied: '', codeCopyNonce: 0 });

/**
 * AttachmentFilePreview's rendered Markdown: `<ChatMarkdown text cwd={undefined} />`, the chat renderer Files' rendered
 * file uses (files-surface.ts renderedMarkdown), with no folder: relative links resolve against nothing
 * (data/src/markdown_media.rs links_without_cwd), code blocks keep their colours and Copy code, tables their menu,
 * ```mermaid fences draw (filesMermaid: Files and an attachment are never the active surface together), and the task
 * checkboxes stay disabled. Its media have no folder and no thread: a
 * web URL loads as written, and a host path, relative or absolute, is "Image unavailable · <alt>" (MarkdownImage's
 * ChatMarkdownImageFallback), so nothing is signed.
 */
async function renderedAttachment(client: T3Client, native: Native | null | undefined, id: string, source: string) {
  // A leading YAML frontmatter block is blanked (maskMarkdownFrontmatter), not drawn as a rule and a heading.
  const text = maskMarkdownFrontmatter(source);
  const copy = timelineView(client).codeCopy, copied = copy.threadId === client.threadId;
  const diagrams = filesMermaid(client, text);
  return {
    markdownSource: [{ id: `attachment:${id}`, kind: 'attachment', title: '', body: text }],
    md: { ...markdownEnv(client), chips: withFileLinkMenus(client, messageChips({ text }, '', [], `attachment:${id}`, { thread: false }, client)) }, // the owner names the document
    code: messageCodeBlocks(text, 'shell', `attachment:${id}`), diagrams, mdUrls: [],
    codeCopied: copied ? copy.text : '', codeCopyNonce: copied ? copy.nonce : 0,
  };
}

/** renderedToggleLabel: the rendered/source toggle's label for the mode it switches. */
function renderedToggleLabel(mode: string, rendered: boolean): string {
  if (mode === 'mermaid') return rendered ? 'Show Mermaid source' : 'Show rendered diagram';
  if (mode === 'markdown') return rendered ? 'Show markdown source' : 'Show rendered markdown';
  if (mode === 'table') return rendered ? 'Show source' : 'Show table';
  return mode === 'html' ? (rendered ? 'Show HTML source' : 'Show rendered page') : '';
}

/** The surface's projection: the URL (minted on first view), the text body for text kinds, and the header's controls. */
export async function attachmentView(client: T3Client, native: Native | null | undefined, meta: AttachmentMeta, now: number, source = sentSource(meta)): Promise<AttachmentView> {
  const state = stateOf(client, source.key), kind = previewBody(filePreviewKind(meta)), delimiter = filePreviewDelimiter(meta);
  const renderedMode = kind === 'mermaid' ? 'mermaid' : kind === 'markdown' ? 'markdown' : kind === 'html' ? 'html' : delimiter ? 'table' : '';
  const needsText = kind === 'text' || kind === 'markdown' || kind === 'mermaid' || (kind === 'html' && !state.rendered);
  if (source.local) {
    state.url = source.local.url;
    if (needsText && !state.content && !state.contentError) {
      const reply = obj(await source.local.text().catch(error => { if (letGo(error)) throw error; return { ok: false, message: message(error, 'Could not load this file.') }; }));
      if (reply.ok === true) state.content = { text: str(reply.text), truncated: reply.truncated === true };
      else state.contentError = str(reply.message, 'Could not load this file.');
    }
  } else if (native?.available && client.ready) {
    // A font whose specimen must be written again reads the font from a URL signed afresh, as its text does.
    const rereads = (needsText && !state.content && !state.contentError) || (kind === 'font' && !state.fontError && !fontPageKept(source.key));
    if (!state.urlError && (!state.url || (rereads && now && now - state.urlAt > STALE_URL_MS))) {
      try { state.url = await mint(client, native, meta, source, 'inline'); state.urlAt = now; }
      catch (error) { if (letGo(error)) throw error; state.urlError = message(error, 'The attachment is unavailable.'); }
    }
    if (needsText && state.url && !state.content && !state.contentError) {
      const reply = obj(await attachmentText(state.url));
      if (reply.ok === true) state.content = { text: str(reply.text), truncated: reply.truncated === true };
      else state.contentError = str(reply.message, 'Could not load this file.');
    }
  }
  // A PDF or a rendered page is a frame. A PDF the server serves inline is framed at its URL, which WebKit's PDF view
  // reads, as Files' is. One it serves as a download, a sent file stored as `-bin` (no usable extension; AssetAccess's
  // `download` claim), and a draft's staged PDF are copied under a `.pdf` name (media-frame.ts) and shown from the copy,
  // as PDF.js reads the bytes whatever the server's disposition; the copy can fail.
  const framed = !needsText && (kind === 'pdf' || kind === 'html') && state.url && !state.urlError && !state.mediaError;
  const atUrl = kind === 'pdf' && !stagedCopyName(state.url) && (!!source.resource || /-pdf$/i.test(meta.id));
  const frame = !framed ? null : atUrl ? { src: state.url, error: '' } : await mediaFrameSource(kind, state.url);
  if (frame?.error) state.mediaError = frame.error;
  // FontViewer: the specimen once its bytes are read; its frame says whether the face loaded.
  const font = kind === 'font' && state.url && !state.urlError && !state.fontError ? await fontPage(source.key, state.url, meta.sizeBytes) : null;
  if (font?.error) state.fontError = font.error;
  const fontError = kind === 'font' ? state.fontError : '';
  const failure = state.urlError || (needsText ? state.contentError : state.mediaError);
  const content = state.content, rendered = state.rendered;
  const showsRawText = !failure && needsText && !!content && !(delimiter && rendered) && !((kind === 'markdown' || kind === 'mermaid') && rendered);
  const preview = failure ? 'error' : !state.url || (needsText && !content) ? 'loading'
    : needsText && content ? (delimiter && rendered ? 'table' : kind === 'mermaid' && rendered ? 'mermaid' : kind === 'markdown' && rendered ? 'markdown' : 'code')
      : kind === 'font' ? (fontError ? 'none' : font?.page ? 'font' : 'loading')
        : kind === 'image' ? 'image' : mediaBody(kind, rendered) || 'none';
  const text = content?.text ?? '';
  const table = preview === 'table' ? tableRows(meta.name, text) : null;
  const lines = preview === 'code' ? codeLines(meta.name, text, 'shell') : [];
  const extension = meta.name.split('.').pop() || 'this format';
  // useCopyToClipboard: "Copied" for two seconds of the window's clock, stamped by the first view after the copy.
  if (state.copiedAt < 0 && now) state.copiedAt = now;
  const copied = state.copiedAt < 0 || (state.copiedAt > 0 && now - state.copiedAt < 2000);
  const rich = preview === 'markdown' ? await renderedAttachment(client, native, source.id, text) : NO_MARKDOWN();
  const diagram = preview === 'mermaid' ? mermaidFile(client, text) : null;
  return {
    id: source.id, origin: source.origin, canRemove: source.dialog, canClose: source.dialog, name: meta.name, size: meta.sizeBytes > 0 ? formatAttachmentSize(meta.sizeBytes) : '', preview, error: failure,
    canRender: !!renderedMode, rendered, renderLabel: renderedToggleLabel(renderedMode, rendered),
    renderIcon: rendered ? 'code' : renderedMode === 'table' ? 'table' : 'eye',
    showWrap: showsRawText, wrap: client.local.clientSettings.wordWrap !== false,
    canCopy: !!content, copyLabel: copied ? 'Copied' : content?.truncated ? 'Copy preview' : 'Copy contents', copied,
    canSave: !!state.url, saving: state.saving, saveLabel: state.saving ? 'Preparing file…' : 'Save file',
    truncatedNote: content?.truncated ? `Preview limited to the first 1 MB${meta.sizeBytes > 0 ? ` of a ${meta.sizeBytes.toLocaleString('en-US')} byte file` : ''}. Save the file to read it in full.` : '',
    ...rich, ...(diagram ? { diagrams: diagram.diagrams } : {}), table: table?.rows ?? [],
    lines, gutter: sourceGutter(lines.length),
    url: preview === 'pdf' || preview === 'html' ? frame?.src ?? '' : preview === 'font' ? font?.page ?? '' : preview === 'image' || mediaBody(preview, true) ? state.url : '',
    // The unsupported body, or why a font has no specimen.
    noPreview: preview !== 'none' ? '' : fontError ? `${fontError} Save it to open in another app.` : `Save it to open in an app that supports ${extension} files.`,
    mermaidNote: diagram?.note ?? '', noPreviewAlert: !!fontError,
  };
}
export const emptyAttachment = (): AttachmentView => ({
  id: '', origin: 'Attachment', canRemove: false, canClose: false, name: '', size: '', preview: '', error: '', canRender: false, rendered: false, renderLabel: '', renderIcon: '', showWrap: false, wrap: true, canCopy: false,
  copyLabel: 'Copy contents', copied: false, canSave: false, saving: false, saveLabel: 'Save file', truncatedNote: '', table: [], lines: [], gutter: 0, url: '', noPreview: '', ...NO_MARKDOWN(),
  mermaidNote: '', noPreviewAlert: false,
});
export const attachmentToken = (name: string) => fileIconToken(name);

/** Drops a preview's state (a draft's preview opens afresh, as its dialog mounts a new AttachmentFilePreview). */
export function forgetPreview(client: T3Client, key: string): void { states.get(client)?.delete(key); forgetFontPage(key); }
/**
 * A sent file's preview shown again after another surface: the reference mounts only the active surface, which signs
 * its URL afresh (a signature lives an hour), so the next view signs it again, and its failures (a refused URL, a read,
 * the media's or the font's) end with the preview that had them.
 */
export function previewShownAgain(client: T3Client, key: string): void {
  const state = states.get(client)?.get(key);
  if (state) { state.url = ''; state.urlAt = 0; state.urlError = ''; state.contentError = ''; state.mediaError = ''; state.fontError = ''; }
}

/** `surface-linked-att-*`: the header's controls. */
export async function attachmentLocal(client: T3Client, native: Native, op: string, id: string, now: number): Promise<string> {
  const meta = requireAttachment(client, id);
  return attachmentOp(client, native, meta, sentSource(meta), op, now);
}

/** A preview's header controls (and its media body's error), on the bytes `source` names. */
export async function attachmentOp(client: T3Client, native: Native, meta: AttachmentMeta, source: PreviewSource, op: string, now: number): Promise<string> {
  const state = stateOf(client, source.key);
  if (op === 'render') { state.rendered = !state.rendered; if (filePreviewKind(meta) === 'html') state.contentError = ''; return ''; }
  if (op === 'wrap') { client.local.clientSettings.wordWrap = client.local.clientSettings.wordWrap === false; return ''; }
  if (op === 'retry') {
    forgetFontPage(source.key);
    state.url = source.local?.url ?? ''; state.urlError = ''; state.content = null; state.contentError = ''; state.mediaError = ''; state.fontError = '';
    return '';
  }
  // The audio/video player's or the picture's `error` event; a late one from a body already gone is ignored.
  if (op === 'media-error') { if (state.url && !state.mediaError) state.mediaError = mediaErrorMessage(previewBody(filePreviewKind(meta))); return ''; }
  // The specimen's FontFace did not load the bytes (font-preview.ts).
  if (op === 'font-failed') { if (fontPageKept(source.key)) state.fontError = FONT_REFUSED; return ''; }
  // PreviewImageContent's press: the expanded image dialog over this one image, the focus going back to the picture.
  if (op === 'image-open') {
    if (/^https?:/i.test(state.url)) openInlineDocumentImage(client, state.url, meta.name, source.dialog ? 'chip-file-preview-image-zoom' : 'attachment-image-zoom');
    return '';
  }
  if (op === 'copy') {
    if (!state.content) return '';
    try { await client.restAccess(native).call({ op: 'copyText', text: state.content.text }); state.copiedAt = now || -1; }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not copy', description: message(error, 'Please try again.') }); }
    return '';
  }
  if (op === 'save') {
    if (state.saving) return '';
    state.saving = true;
    try {
      if (source.local) await source.local.save();
      else await saveAttachment(client, await mint(client, native, meta, source, 'attachment'), meta.name);
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Could not save file', description: message(error, 'Please try again.') });
    } finally { state.saving = false; }
    return '';
  }
  return '';
}
