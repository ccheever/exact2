// A sent attachment opened beside the thread (MIT reference,
// see LICENSE-T3: ChatView.tsx openFileAttachment → rightPanelStore.openAttachment
// (an `attachment:<id>` file surface that replaces the standalone explorer tab),
// components/files/AttachmentFilePreview.tsx and @t3tools/shared filePreview /
// delimitedPreview; T3 Code 7c3b3b3605ef). The captured bytes are read from a signed asset URL
// (`assets.createUrl`, re-minted after five minutes), never from a workspace
// file of the same name; the subheader reads "Attachment › name  size" with the
// rendered/source toggle, word wrap, Copy contents and Save file. Its rendered
// Markdown is ChatMarkdown with no `cwd`, a Mermaid file is its diagram, an image has the size and zoom bar
// (preview-image.contract) and a font its specimen (font-preview.ts). The reference's lazy-loaded viewers run in the
// viewer page (viewer-frames.ts): a PDF, text past 1 MB, a CSV or TSV table, Office, OpenDocument, RTF, EPUB and
// notebook files, archives and databases; the page fetches the signed URL, or takes a draft's bytes by message. A
// notebook's cells and an archive's opened entry come back from it and are drawn here (notebook-preview.ts). Files
// shows a document, or a file it cannot read as text, in the same body under "File" (files-surface.ts).
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { assetUrl } from '../settings/settings-b-icons';
import { formatAttachmentSize } from '../composer/composer-editor-files';
import { codeLines, type CodeLine, type MarkdownSource } from './files-surface';
import { markdownEnv, messageChips, type MarkdownEnv } from '../timeline/message-chips';
import { withFileLinkMenus } from '../timeline/file-link-menu';
import { messageCodeBlocks } from '../highlight/timeline-highlight';
import { filesMermaid, type MermaidDiagramView } from '../timeline/timeline-mermaid';
import { timelineView } from '../timeline/timeline-presentation';
import { fileIconToken } from '../shared/timeline-files';
import { mediaBody, mediaErrorMessage } from '../media/attachment-media'; // PDF, HTML, audio and video bodies
import { cachedStagedCopy, forgetStagedCopies, mediaFrameSource, stagedCopyName } from '../media/media-frame'; // a rendered page's frame source; a draft's staged copy
import { sourceGutter } from './files-crumbs-settle'; // the reference's line-number column
import { letGo } from '../client/let-go';
import { attachmentText, saveAttachment } from './attachment-files'; // the preview's text and Save file
import { FILE_TEXT_PREVIEW_MAX_BYTES, filePreviewDelimiter, filePreviewKind, maskMarkdownFrontmatter, officePreviewKind, type FilePreviewKind } from './file-preview-kind';
import { connectionScope, emptyViewerFrame, frameGone, newFrame, postBytes, requestCopy, viewerFrameId, viewerFrameView, viewerMessage, viewerPick, type FrameState, type ViewerConfig, type ViewerFrameView } from './viewer-frames'; // the viewer page
import { emptyEntry, emptyNotebook, entryPicture, entryView, forgetNotebook, notebookOutputMessage, notebookView, showMoreCells, type EntryView, type NotebookView } from './notebook-preview'; // what the page hands back
import { appFiles } from '../media/save-file';
import { stagedPath } from './attachment-files';
import { foldedText } from '../composer/composer-editor-files';
import { openExternal } from '../client/external-open';
import { mermaidFile } from './mermaid-file'; // a Mermaid file's diagram
import { FONT_REFUSED, fontPage, fontPageKept, forgetFontPage } from './font-preview'; // a font's specimen
import { openCachedImage, openInlineDocumentImage } from '../timeline/timeline-attachments'; // the expanded image dialog

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
  truncatedNote: string;
  lines: CodeLine[]; gutter: number; url: string; noPreview: string;
  // The rendered Markdown as Files' (files-surface.ts FilesView): renderFileMarkdown's one
  // message (kind "attachment"), its chips and settings, code colours, Mermaid fences, host-path media and Copy code's check.
  markdownSource: MarkdownSource[]; md: MarkdownEnv; code: ReturnType<typeof messageCodeBlocks>; diagrams: MermaidDiagramView[];
  mdUrls: { id: string; url: string; fill: string; hover: string; border: string; ink: string }[]; codeCopied: string; codeCopyNonce: number;
  /** A Mermaid file's refusal (MermaidViewer), shown instead of its diagram; `diagrams` is then empty. */
  mermaidNote: string;
  /** "No preview for this file" reports a failure (FontViewer's), not only a format. */
  noPreviewAlert: boolean;
  /** The viewer page (`preview == "viewer"`), its archive entry and its notebook (`preview == "notebook"`). */
  viewer: ViewerFrameView; entry: EntryView; notebook: NotebookView;
};
type State = { url: string; urlAt: number; urlError: string; content: Content | null; contentError: string; rendered: boolean; saving: boolean; copiedAt: number; mediaError: string;
  /** Why the font has no specimen: its read failed, or its frame's FontFace did not load it; kept while it is shown. */
  fontError: string;
  /** The viewer page's frame, and the Try again count that mounts its viewer afresh (the reference's `revision`). */
  frame: FrameState; revision: number;
  /** Why a draft's bytes cannot reach the page. */
  bytesError: string;
  /** This preview's own word wrap (AttachmentFilePreview's wordWrapOverride, 6d450a4284); null follows Settings' word wrap. */
  wrap: boolean | null };
const states = new WeakMap<T3Client, Map<string, State>>();
const STALE_URL_MS = 5 * 60_000; // AttachmentFilePreview: signed URLs live an hour; re-mint after five minutes

/** filePreviewKind, as composer-chip-file.ts reads it. */
export const previewKind = (file: { name: string; mimeType?: string }): FilePreviewKind => filePreviewKind(file);
/** The kinds the viewer page draws: PDF.js, the Office and document viewers, and the archive and database viewer. */
const PAGE_KINDS = new Set<FilePreviewKind>(['pdf', 'archive', 'sqlite', 'docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'rtf', 'epub', 'ipynb']);
/** The body a kind draws here: the viewer page's kinds are `viewer`, as text past 1 MB and a CSV or TSV table are. */
export function previewBody(kind: FilePreviewKind): FilePreviewKind | 'viewer' {
  return PAGE_KINDS.has(kind) ? 'viewer' : kind;
}
/** The draft files the page takes by message: `readFile`'s bound. A larger one is a file the page is given (givenDraft). */
const DRAFT_PAGE_BYTES = 16 * 1024 * 1024;

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

const wrapOf = (client: T3Client, state: State): boolean => state.wrap ?? client.local.clientSettings.wordWrap !== false;
function stateOf(client: T3Client, id: string): State {
  let map = states.get(client);
  if (!map) { map = new Map(); states.set(client, map); }
  let state = map.get(id);
  // AttachmentFilePreview opens rendered.
  if (!state) { state = { url: '', urlAt: 0, urlError: '', content: null, contentError: '', rendered: true, saving: false, copiedAt: 0, mediaError: '', fontError: '', frame: newFrame(), revision: 0, bytesError: '', wrap: null }; map.set(id, state); }
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

/**
 * A draft's staged copy past what a message carries, copied into app:/cache so its page is given it in its frame's
 * `src` (LLP 1020.001 D7) and reads it from there, as it reads a sent file from its URL; '' for a smaller copy, whose bytes
 * go by message, and for a folded paste.
 */
async function givenDraft(source: PreviewSource, name: string): Promise<string> {
  const staged = stagedCopyName(source.local?.url ?? '');
  if (!staged) return '';
  const files = appFiles().fs as unknown as { stat?(path: string): Promise<{ size?: number }> };
  const size = Number((await files.stat?.(stagedPath(staged)).catch(() => null))?.size ?? 0);
  return size > DRAFT_PAGE_BYTES ? cachedStagedCopy(staged, name) : '';
}

/** A draft's bytes for the viewer page: its staged copy, or a folded paste's text (composer-chip-file.ts draftSource). */
async function draftBytes(client: T3Client, source: PreviewSource): Promise<Uint8Array> {
  const staged = stagedCopyName(source.local?.url ?? '');
  if (staged) {
    const bytes = await appFiles().fs.readFile(stagedPath(staged)).catch(() => null);
    if (bytes) return new Uint8Array(bytes);
  }
  const text = foldedText(client, source.id);
  if (text === undefined) throw new ClientError('Could not load this file.');
  return new TextEncoder().encode(text);
}

/** The surface's projection: the URL (minted on first view), the text body for text kinds, and the header's controls. */
export async function attachmentView(client: T3Client, native: Native | null | undefined, meta: AttachmentMeta, now: number, source = sentSource(meta)): Promise<AttachmentView> {
  const state = stateOf(client, source.key), fileKind = filePreviewKind(meta), kind = previewBody(fileKind), delimiter = filePreviewDelimiter(meta);
  const office = officePreviewKind(meta);
  const renderedMode = kind === 'mermaid' ? 'mermaid' : kind === 'markdown' ? 'markdown' : kind === 'html' ? 'html' : delimiter ? 'table' : '';
  // AttachmentFilePreview's streamsText: text past 1 MB, or a sent file of unknown size, streams in the large-text viewer.
  const streamsText = fileKind === 'text' && (meta.sizeBytes > FILE_TEXT_PREVIEW_MAX_BYTES || (meta.sizeBytes === 0 && (!source.resource || source.resource._tag === 'attachment')));
  const needsText = kind === 'text' || kind === 'markdown' || kind === 'mermaid' || (kind === 'html' && !state.rendered);
  const readsText = needsText && !streamsText;
  if (source.local) {
    state.url = source.local.url;
    if (readsText && !state.content && !state.contentError) {
      const reply = obj(await source.local.text().catch(error => { if (letGo(error)) throw error; return { ok: false, message: message(error, 'Could not load this file.') }; }));
      if (reply.ok === true) state.content = { text: str(reply.text), truncated: reply.truncated === true };
      else state.contentError = str(reply.message, 'Could not load this file.');
    }
  } else if (native?.available && client.ready) {
    // A font whose specimen must be written again reads the font from a URL signed afresh, as its text does.
    const rereads = (readsText && !state.content && !state.contentError) || (kind === 'font' && !state.fontError && !fontPageKept(source.key));
    if (!state.urlError && (!state.url || (rereads && now && now - state.urlAt > STALE_URL_MS))) {
      try { state.url = await mint(client, native, meta, source, 'inline'); state.urlAt = now; }
      catch (error) { if (letGo(error)) throw error; state.urlError = message(error, 'The attachment is unavailable.'); }
    }
    if (readsText && state.url && !state.content && !state.contentError) {
      const reply = obj(await attachmentText(state.url));
      if (reply.ok === true) state.content = { text: str(reply.text), truncated: reply.truncated === true };
      else state.contentError = str(reply.message, 'Could not load this file.');
    }
  }
  // A rendered page is a frame: the server's inline page at its URL, or a copy under a `.html` name (media-frame.ts).
  const framed = !needsText && kind === 'html' && state.url && !state.urlError && !state.mediaError;
  const frame = !framed ? null : await mediaFrameSource(state.url);
  if (frame?.error) state.mediaError = frame.error;
  // FontViewer: the specimen once its bytes are read; its frame says whether the face loaded.
  const font = kind === 'font' && state.url && !state.urlError && !state.fontError ? await fontPage(source.key, state.url, meta.sizeBytes) : null;
  if (font?.error) state.fontError = font.error;
  const fontError = kind === 'font' ? state.fontError : '';
  const failure = state.urlError || state.bytesError || (readsText ? state.contentError : state.mediaError);
  const content = state.content, rendered = state.rendered;
  // The viewer page: the large-text viewer for a stream or a rendered table, else the kind's own viewer.
  const table = !failure && readsText && !!content && !!delimiter && rendered;
  const pageView: ViewerConfig['view'] | null = failure || !state.url ? null
    : streamsText || table ? 'text' : kind === 'viewer' ? (fileKind === 'pdf' ? 'pdf' : office ? 'office' : 'binary') : null;
  const viewer = pageView ? viewerConfig(client, meta, state, pageView, { streams: streamsText, table, delimiter, office, kind: fileKind, local: !!source.local }) : null;
  // A copy that could not be made says why itself: its draft is past what the bytes' read below takes.
  let given = '', uncopied = false;
  if (viewer && source.local && !viewer.text) {
    try { given = await givenDraft(source, meta.name); }
    catch (error) { if (letGo(error)) throw error; uncopied = true; state.bytesError = message(error, 'Could not load this file.'); }
    if (given) viewer.given = true;
  }
  if (viewer && source.local && !viewer.text && !given && !uncopied && state.frame.booted && state.frame.sentBytes !== viewer.key) {
    try { postBytes(state.frame, viewer.key, await draftBytes(client, source)); }
    catch (error) { if (letGo(error)) throw error; state.bytesError = message(error, 'Could not load this file.'); }
  }
  const notebook = pageView === 'office' && state.frame.notebook ? state.frame.notebook : null;
  const showsRawText = !failure && needsText && (!!content || streamsText) && !(delimiter && rendered) && !((kind === 'markdown' || kind === 'mermaid') && rendered);
  const preview = failure ? 'error' : notebook ? 'notebook' : pageView ? 'viewer' : !state.url || (readsText && !content) ? 'loading'
    : readsText && content ? (kind === 'mermaid' && rendered ? 'mermaid' : kind === 'markdown' && rendered ? 'markdown' : 'code')
      : kind === 'font' ? (fontError ? 'none' : font?.page ? 'font' : 'loading')
        : kind === 'image' ? 'image' : mediaBody(kind === 'viewer' ? 'unsupported' : kind, rendered) || 'none';
  if (preview !== 'viewer') frameGone(state.frame);
  const text = content?.text ?? '';
  const lines = preview === 'code' ? codeLines(meta.name, text, 'shell') : [];
  const extension = meta.name.split('.').pop() || 'this format';
  // useCopyToClipboard: "Copied" for two seconds of the window's clock, stamped by the first view after the copy.
  if (state.copiedAt < 0 && now) state.copiedAt = now;
  const copied = state.copiedAt < 0 || (state.copiedAt > 0 && now - state.copiedAt < 2000);
  // A notebook's markdown cells are documents of the same parse (app.contract filesMarkdown).
  const cells = notebook ? await notebookView(client, source.key, notebook) : null;
  const rich = preview === 'markdown' ? await renderedAttachment(client, native, source.id, text) : cells ? { ...NO_MARKDOWN(), ...cells.markdown } : NO_MARKDOWN();
  const diagram = preview === 'mermaid' ? mermaidFile(client, text) : null;
  // One frame per file and whom its page may reach (`connectionScope`): a page narrows its connections once.
  const frameId = viewerFrameId(`${source.key}-${connectionScope(viewer?.src ?? '')}`);
  return {
    id: source.id, origin: source.origin, canRemove: source.dialog, canClose: source.dialog, name: meta.name, size: meta.sizeBytes > 0 ? formatAttachmentSize(meta.sizeBytes) : '', preview, error: failure,
    canRender: !!renderedMode, rendered, renderLabel: renderedToggleLabel(renderedMode, rendered),
    renderIcon: rendered ? 'code' : renderedMode === 'table' ? 'table' : 'eye',
    showWrap: showsRawText, wrap: wrapOf(client, state),
    canCopy: !!content || streamsText, copyLabel: copied ? 'Copied' : content?.truncated ? 'Copy preview' : 'Copy contents', copied,
    canSave: !!state.url, saving: state.saving, saveLabel: state.saving ? 'Preparing file…' : 'Save file',
    truncatedNote: content?.truncated ? `Preview limited to the first 1 MB${meta.sizeBytes > 0 ? ` of a ${meta.sizeBytes.toLocaleString('en-US')} byte file` : ''}. Save the file to read it in full.` : '',
    ...rich, ...(diagram ? { diagrams: diagram.diagrams } : {}),
    lines, gutter: sourceGutter(lines.length),
    url: preview === 'html' ? frame?.src ?? '' : preview === 'font' ? font?.page ?? '' : preview === 'image' || mediaBody(preview, true) ? state.url : '',
    // The unsupported body, or why a font has no specimen.
    noPreview: preview !== 'none' ? '' : fontError ? `${fontError} Save it to open in another app.` : `Save it to open in an app that supports ${extension} files.`,
    mermaidNote: diagram?.note ?? '', noPreviewAlert: !!fontError,
    viewer: viewer && preview === 'viewer' ? viewerFrameView(state.frame, frameId, viewer, given) : emptyViewerFrame(),
    entry: preview === 'viewer' && state.frame.entry ? await entryView(client, source.key, state.frame.entry) : emptyEntry(),
    notebook: cells?.view ?? emptyNotebook(),
  };
}

/** What the viewer page shows for this preview, keyed as the reference keys the viewer it mounts. */
function viewerConfig(client: T3Client, meta: AttachmentMeta, state: State, view: ViewerConfig['view'],
  how: { streams: boolean; table: boolean; delimiter: ',' | '\t' | null; office: string | null; kind: FilePreviewKind; local: boolean }): ViewerConfig {
  const url = state.url, revision = state.revision, settings = client.local.clientSettings as { wordWrap?: boolean; fontSmoothing?: boolean };
  const base = { name: meta.name, src: how.local ? '' : url, text: null, sizeBytes: meta.sizeBytes, kind: '', delimiter: null, wrap: wrapOf(client, state), revealLine: null, revealRequestId: 0, smoothing: settings.fontSmoothing !== false };
  // LargeTextViewer `${url}:${rendered}:${revision}`; DelimitedTablePreview by the name, with the text already read.
  if (how.streams) return { ...base, view, key: `text:${url}:${state.rendered}:${revision}`, delimiter: state.rendered ? how.delimiter : null };
  if (how.table) return { ...base, view, key: `table:${meta.name}:${revision}`, src: '', text: state.content?.text ?? '', sizeBytes: -1, delimiter: how.delimiter };
  if (view === 'pdf') return { ...base, view, key: `pdf:${url}:${revision}` };
  if (view === 'office') return { ...base, view, key: JSON.stringify([url, how.office, meta.sizeBytes, revision]), kind: how.office ?? '' };
  return { ...base, view, key: `binary:${revision}:${url}`, kind: how.kind === 'sqlite' ? 'sqlite' : 'archive' };
}
export const emptyAttachment = (): AttachmentView => ({
  id: '', origin: 'Attachment', canRemove: false, canClose: false, name: '', size: '', preview: '', error: '', canRender: false, rendered: false, renderLabel: '', renderIcon: '', showWrap: false, wrap: true, canCopy: false,
  copyLabel: 'Copy contents', copied: false, canSave: false, saving: false, saveLabel: 'Save file', truncatedNote: '', lines: [], gutter: 0, url: '', noPreview: '', ...NO_MARKDOWN(),
  mermaidNote: '', noPreviewAlert: false, viewer: emptyViewerFrame(), entry: emptyEntry(), notebook: emptyNotebook(),
});
export const attachmentToken = (name: string) => fileIconToken(name);

/** Drops a preview's state (a draft's preview opens afresh, as its dialog mounts a new AttachmentFilePreview). */
export function forgetPreview(client: T3Client, key: string): void { states.get(client)?.delete(key); forgetFontPage(key); forgetNotebook(key); }
/**
 * A sent file's preview another surface replaced: the reference mounts only the active surface, so the preview goes with
 * what it signed (a signature lives an hour), read and parsed (a notebook's cells and its Show more count), and its
 * failures. The font specimen pages stay among the four kept (font-preview.ts).
 */
export function previewHidden(client: T3Client, key: string): void { states.get(client)?.delete(key); forgetNotebook(key); }

/** `surface-linked-att-*`: the header's controls. */
export async function attachmentLocal(client: T3Client, native: Native, op: string, id: string, now: number): Promise<string> {
  const meta = requireAttachment(client, id);
  return attachmentOp(client, native, meta, sentSource(meta), op, now);
}

/**
 * The viewer page's messages, carried in the op so every owner's route passes them as they are (`viewer:<payload>`,
 * `viewer-posted:<key>`, `viewer-pick:<serial>|<row>`): a boot, a post taken, a menu and its pick, Copy contents' text,
 * Try again, a link, an archive's entry and a notebook's cells.
 */
async function viewerOp(client: T3Client, native: Native, meta: AttachmentMeta, source: PreviewSource, op: string, now: number): Promise<string> {
  const state = stateOf(client, source.key), frame: FrameState = state.frame;
  if (op.startsWith('viewer-posted:')) { frame.posts = frame.posts.filter(post => post.key !== op.slice(14)); return ''; }
  if (op.startsWith('viewer-pick:')) { viewerPick(frame, op.slice(12)); return ''; }
  if (op === 'viewer-more') { showMoreCells(source.key); return ''; }
  if (op.startsWith('viewer-output:')) {
    const rest = op.slice(14), bar = rest.indexOf('|');
    notebookOutputMessage(source.key, rest.slice(0, bar), rest.slice(bar + 1));
    return '';
  }
  const event = viewerMessage(frame, op.slice(7));
  if (event.type === 'retry') return attachmentOp(client, native, meta, source, 'retry', now);
  if (event.type === 'link') { openExternal(event.url); return ''; }
  if (event.type === 'copy') {
    try { await client.restAccess(native).call({ op: 'copyText', text: event.text }); state.copiedAt = now || -1; }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not copy file', description: message(error, 'Try again.') }); }
  }
  if (event.type === 'copy-error') pushToast(client, { kind: 'error', title: 'Could not copy file', description: event.message });
  return '';
}

/** A preview's header controls (and its media body's error), on the bytes `source` names. */
export async function attachmentOp(client: T3Client, native: Native, meta: AttachmentMeta, source: PreviewSource, op: string, now: number): Promise<string> {
  const state = stateOf(client, source.key);
  if (op.startsWith('viewer')) return viewerOp(client, native, meta, source, op, now);
  if (op === 'render') { state.rendered = !state.rendered; if (filePreviewKind(meta) === 'html') state.contentError = ''; return ''; }
  // The wrap button changes this preview only, not Settings' word wrap.
  if (op === 'wrap') { state.wrap = !wrapOf(client, state); return ''; }
  if (op === 'retry') {
    forgetFontPage(source.key);
    state.url = source.local?.url ?? ''; state.urlError = ''; state.content = null; state.contentError = ''; state.mediaError = ''; state.fontError = '';
    // AttachmentFilePreview's `revision`: the config's key changes, so the page mounts its viewer afresh in the frame
    // it has, and a draft's bytes go to it again.
    state.revision += 1; state.bytesError = ''; state.frame.notebook = null; state.frame.entry = null;
    forgetNotebook(source.key); // NotebookPreview remounts: twenty cells again
    // A draft's copies are made afresh: one can be gone from the cache while its path is kept.
    const staged = stagedCopyName(source.local?.url ?? '');
    if (staged) forgetStagedCopies(staged);
    return '';
  }
  // The audio/video player's or the picture's `error` event; a late one from a body already gone is ignored.
  if (op === 'media-error') { if (state.url && !state.mediaError) state.mediaError = mediaErrorMessage(previewBody(filePreviewKind(meta))); return ''; }
  // The specimen's FontFace did not load the bytes (font-preview.ts).
  if (op === 'font-failed') { if (fontPageKept(source.key)) state.fontError = FONT_REFUSED; return ''; }
  // The same press on an archive entry's picture, which the app wrote to its media cache.
  if (op === 'entry-image-open') {
    const path = state.frame.entry?.view === 'image' ? await entryPicture(source.key) : '';
    if (path) openCachedImage(client, path, state.frame.entry!.path, 'attachment-entry-image-zoom');
    return '';
  }
  // PreviewImageContent's press: the expanded image dialog over this one image, the focus going back to the picture.
  if (op === 'image-open') {
    if (/^https?:/i.test(state.url)) openInlineDocumentImage(client, state.url, meta.name, source.dialog ? 'chip-file-preview-image-zoom' : 'attachment-image-zoom');
    return '';
  }
  if (op === 'copy') {
    // A streamed text's Copy contents is the large-text viewer's: the page reads the whole file and answers `copy`. A
    // failure body mounts no viewer to ask.
    if (!state.content) {
      if (state.urlError || state.bytesError) pushToast(client, { kind: 'error', title: 'Could not copy file', description: 'Wait for the file to finish loading before copying.' });
      else requestCopy(state.frame);
      return '';
    }
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
