// What the viewer page hands back for the app to draw (MIT reference, see LICENSE-T3: apps/web/src/components/files/
// NotebookPreview.tsx and BinaryViewer.tsx ArchiveFile; T3 Code 7c3b3b3605ef). A notebook is parsed in the page's
// Office worker (extendedDocumentPreview.ts openNotebook) and drawn here: "<n> cells · <language> · <size>", "Saved
// outputs only. Code is never executed.", twenty cells at a time with Show more, each markdown cell as inert
// ChatMarkdown, each source cell's numbered lines up to 360 pt tall, its text and image outputs, and each HTML output in
// a frame of the page that reports its height. An archive's opened entry is its source view or its image.
import type { T3Client } from '../client/client';
import { codeLines, type CodeLine, type MarkdownSource } from './files-surface';
import { markdownEnv, messageChips, type MarkdownEnv } from '../timeline/message-chips';
import { withFileLinkMenus } from '../timeline/file-link-menu';
import { messageCodeBlocks } from '../highlight/timeline-highlight';
import { filesMermaid, type MermaidDiagramView } from '../timeline/timeline-mermaid';
import { sourceGutter } from './files-crumbs-settle';
import { formatAttachmentSize } from '../composer/composer-editor-files';
import { appFiles } from '../media/save-file';
import { MEDIA_CACHE } from '../media/media-frame';
import { emptyViewerFrame, newFrame, viewerFrameId, viewerFrameView, type FrameState, type ViewerEntry, type ViewerFrameView, type ViewerNotebook } from './viewer-frames';

export type EntryView = { open: boolean; path: string; preview: string; lines: CodeLine[]; gutter: number; url: string; message: string; truncatedNote: string };
export const emptyEntry = (): EntryView => ({ open: false, path: '', preview: '', lines: [], gutter: 0, url: '', message: '', truncatedNote: '' });

export type NotebookOutputView = { id: string; kind: string; text: string; url: string; frame: ViewerFrameView; height: number };
export type NotebookCellView = { id: string; label: string; aria: string; document: string; lines: CodeLine[]; gutter: number; sourceHeight: number; outputs: NotebookOutputView[] };
export type NotebookView = { open: boolean; status: string; cells: NotebookCellView[]; empty: boolean; more: string };
/** The markdown cells' documents and what their parse needs, which the preview's own Markdown fields carry. */
export type NotebookMarkdown = { markdownSource: MarkdownSource[]; md: MarkdownEnv; code: ReturnType<typeof messageCodeBlocks>; diagrams: MermaidDiagramView[] };
export const emptyNotebook = (): NotebookView => ({ open: false, status: '', cells: [], empty: false, more: '' });

const IMAGE_TYPES: Record<string, string> = { png: 'png', jpg: 'jpg', jpeg: 'jpg', gif: 'gif', webp: 'webp', bmp: 'bmp', svg: 'svg', avif: 'avif', heic: 'heic', heif: 'heif', tif: 'tiff', tiff: 'tiff', ico: 'ico' };
/** A `data:` URL an `image` shows directly; a larger picture goes through a file (contract-for-agents.md, image sources). */
const DATA_URL_BYTES = 1024 * 1024;

type Kept = { key: string; page: Promise<string> };
const pictures = new Map<string, Kept>();
let serial = 0;

function bytesOf(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

/** A picture the page sent as base64, written once to app:/cache/media-preview for an `image` to show. */
function picture(owner: string, key: string, base64: string, extension: string): Promise<string> {
  const known = pictures.get(owner);
  if (known?.key === key) return known.page;
  const path = `${MEDIA_CACHE}/entry-${++serial}.${extension}`;
  const files = appFiles().fs as unknown as { mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>; writeFile(path: string, bytes: Uint8Array): Promise<unknown>; rm?(path: string): Promise<unknown> };
  const page = (async () => {
    await files.mkdir(MEDIA_CACHE, { recursive: true }).catch(() => undefined);
    await files.writeFile(path, bytesOf(base64));
    return path;
  })().catch(() => '');
  if (known) void known.page.then(old => (old ? files.rm?.(old) : undefined)).catch(() => undefined);
  pictures.set(owner, { key, page });
  return page;
}

/** ArchiveFile: the entry's source, its picture, or why it cannot be opened inside an archive. */
export async function entryView(_client: T3Client, owner: string, entry: ViewerEntry): Promise<EntryView> {
  const base = { ...emptyEntry(), open: true, path: entry.path };
  if (entry.view === 'failure') return { ...base, preview: 'error', message: entry.message };
  if (entry.view === 'image') {
    const extension = IMAGE_TYPES[entry.path.split('.').pop()?.toLowerCase() ?? ''] ?? 'png';
    const url = await picture(`entry:${owner}`, `${entry.path}:${entry.data.length}`, entry.data, extension);
    return { ...base, preview: url ? 'image' : 'error', url, message: url ? '' : 'Unable to load image.' };
  }
  const lines = codeLines(entry.path, entry.text);
  return { ...base, preview: 'code', lines, gutter: sourceGutter(lines.length), truncatedNote: entry.truncated ? 'Preview limited to the first 1 MB.' : '' };
}

type Outputs = Map<string, FrameState>;
const notebookState = new Map<string, { visible: number; outputs: Outputs }>();
const NOTEBOOK_SOURCE_EXTENSIONS: Record<string, string> = { python: 'py', javascript: 'js', typescript: 'ts', julia: 'jl' };
const VISIBLE_STEP = 20;

/** A notebook preview's own state: the cells shown and its HTML outputs' frames. */
function notebookOf(owner: string) {
  let state = notebookState.get(owner);
  if (!state) {
    state = { visible: VISIBLE_STEP, outputs: new Map() };
    notebookState.set(owner, state);
    if (notebookState.size > 8) notebookState.delete(notebookState.keys().next().value!);
  }
  return state;
}
export function forgetNotebook(owner: string): void { notebookState.delete(owner); pictures.delete(`entry:${owner}`); }
/** The media cache file of the archive entry's picture `owner` shows, or ''. */
export function entryPicture(owner: string): Promise<string> { return pictures.get(`entry:${owner}`)?.page ?? Promise.resolve(''); }

/** NotebookPreview's projection: the shown cells, their Markdown documents and code, and the outputs' frames. */
export async function notebookView(client: T3Client, owner: string, notebook: ViewerNotebook): Promise<{ view: NotebookView; markdown: NotebookMarkdown }> {
  const state = notebookOf(owner);
  const shown = notebook.cells.slice(0, state.visible);
  const markdownSource: MarkdownSource[] = [];
  const wanted = new Set<string>();
  const cells: NotebookCellView[] = [];
  for (const [index, cell] of shown.entries()) {
    const id = `${index}`;
    const document = cell.kind === 'markdown' ? `notebook:${owner}:${index}` : '';
    // `notebook`: the renderer makes the cell inert (data/src/markdown.rs `inert_blocks`), as upstream's tree pass does.
    if (document) markdownSource.push({ id: document, kind: 'notebook', title: '', body: cell.source });
    const source = cell.kind !== 'markdown' && cell.source.trim() ? cell.source : '';
    const extension = cell.kind === 'code' ? (NOTEBOOK_SOURCE_EXTENSIONS[notebook.language] ?? notebook.language) : 'txt';
    const lines = source ? codeLines(`cell.${extension}`, source) : [];
    const outputs: NotebookOutputView[] = [];
    for (const [at, output] of cell.outputs.entries()) {
      const outputId = `${index}-${at}`;
      if (output.kind !== 'html') {
        outputs.push({ id: outputId, kind: output.kind, text: output.kind === 'text' ? output.text : '', url: output.kind === 'image' ? await imageSource(owner, outputId, output.text) : '', frame: emptyViewerFrame(), height: 0 });
        continue;
      }
      wanted.add(outputId);
      let frame = state.outputs.get(outputId);
      if (!frame) { frame = newFrame(); state.outputs.set(outputId, frame); }
      const config = { view: 'output' as const, key: `output:${owner}:${outputId}`, name: `Cell ${index + 1} HTML output`, src: '', text: output.text, sizeBytes: -1, kind: '', delimiter: null, wrap: true, revealLine: null, revealRequestId: 0, smoothing: true };
      outputs.push({ id: outputId, kind: 'html', text: '', url: '', frame: viewerFrameView(frame, viewerFrameId(`${owner}-output-${outputId}`), config), height: frame.height || 80 });
    }
    cells.push({
      id, label: `${cell.kind === 'code' ? notebook.language : cell.kind} · ${index + 1}`, aria: `Cell ${index + 1}`, document,
      lines, gutter: sourceGutter(lines.length), sourceHeight: Math.min(360, lines.length * 20 + 16), outputs,
    });
  }
  for (const key of [...state.outputs.keys()]) if (!wanted.has(key)) state.outputs.delete(key);
  const markdown = markdownSource.map((entry) => entry.body).join('\n\n');
  const size = notebook.sizeBytes === null ? '' : ` · ${notebook.sizeBytes === 0 ? '0 bytes' : formatAttachmentSize(notebook.sizeBytes)}`;
  return {
    view: {
      open: true, status: `${notebook.cells.length.toLocaleString('en-US')} cells · ${notebook.language}${size}`, cells, empty: notebook.cells.length === 0,
      more: state.visible < notebook.cells.length ? `Show more cells (${notebook.cells.length - state.visible} remaining)` : '',
    },
    markdown: {
      markdownSource, md: { ...markdownEnv(client), chips: withFileLinkMenus(client, messageChips({ text: markdown }, '', [], `notebook:${owner}`, { thread: false }, client)) },
      code: messageCodeBlocks(markdown), diagrams: filesMermaid(client, markdown),
    },
  };
}

/**
 * An image output's source: its `data:` URL, or past 1 MiB a file the image reads. Only a picture's `data:` URL: what
 * the page sends is shown with no press, so any other source (a remote URL) shows nothing.
 */
async function imageSource(owner: string, outputId: string, dataUrl: string): Promise<string> {
  const match = /^data:image\/(png|jpeg|gif|webp);base64,([A-Za-z0-9+/=]*)$/.exec(dataUrl);
  if (!match) return '';
  if (dataUrl.length <= DATA_URL_BYTES) return dataUrl;
  return picture(`output:${owner}:${outputId}`, String(dataUrl.length), match[2]!, match[1] === 'jpeg' ? 'jpg' : match[1]!);
}

/** Show more cells: twenty more. */
export function showMoreCells(owner: string): void { notebookOf(owner).visible += VISIBLE_STEP; }

/** An HTML output frame's message: its boot and its measured height. */
export function notebookOutputMessage(owner: string, outputId: string, payload: string): void {
  const frame = notebookOf(owner).outputs.get(outputId);
  if (!frame) return;
  try {
    const message = JSON.parse(payload) as { type?: string; value?: number };
    if (message.type === 'boot') frame.booted = true;
    if (message.type === 'height') frame.height = Math.max(0, Number(message.value) || 0);
  } catch { /* not the page's */ }
}
