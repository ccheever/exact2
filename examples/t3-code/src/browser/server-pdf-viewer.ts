// A server Browser tab's PDF (ServerBrowserSurface `pdfUrl`, T3 Code 74541bab17; browser-url.ts previewPdfAssetUrl)
// in the viewer page (viewer-frames.ts, file-preview-host/), drawn as Chromium's PDF viewer draws the reference's
// `<embed type="application/pdf">` (pdf/browser-pdf-viewer.ts). Its PDF.js reads the bytes the URL answers, so an
// answer that is no PDF draws nothing and runs nothing, where an embed would run an HTML answer as a page; and the page
// reaches only that URL's origin.
import type { T3Client } from '../client/client';
import { openExternal } from '../client/external-open';
import { saveDownload, saveName } from '../media/save-file';
import {
  connectionScope, emptyViewerFrame, newFrame, viewerFrameId, viewerFrameView, viewerMessage, viewerPick,
  type FrameState, type ViewerConfig, type ViewerFrameView,
} from '../panel/viewer-frames';

/** The panel's Browser tab, or the floating player. */
export type PdfPlace = 'tab' | 'mini';
type Viewer = { frame: FrameState; revision: number; url: string };

const viewers = new WeakMap<T3Client, Map<string, Viewer>>();
function viewersOf(client: T3Client): Map<string, Viewer> {
  let map = viewers.get(client);
  if (!map) viewers.set(client, map = new Map());
  return map;
}

/** The file's name in the asset URL's last segment, as the viewer's title. */
function pdfName(url: string): string {
  try { return decodeURIComponent(new URL(url).pathname.split('/').pop() ?? '') || 'PDF document'; } catch { return 'PDF document'; }
}

/** The viewer frame that shows the PDF at `url` for `place`'s server tab, or none when it shows none. */
export function serverPdfViewer(client: T3Client, place: PdfPlace, runtimeId: string, url: string): ViewerFrameView {
  const map = viewersOf(client), key = `${place}|${runtimeId}|${connectionScope(url)}`;
  // Only the frame `place` shows now keeps its state; another runtime or origin is another frame.
  for (const other of map.keys()) if (other.startsWith(`${place}|`) && (other !== key || !url)) map.delete(other);
  if (!url || !runtimeId) return emptyViewerFrame();
  let viewer = map.get(key);
  if (!viewer) map.set(key, viewer = { frame: newFrame(), revision: 0, url });
  viewer.url = url;
  const settings = client.local.clientSettings as { wordWrap?: boolean; fontSmoothing?: boolean };
  const config: ViewerConfig = { view: 'browser-pdf', key: `server-pdf:${key}:${url}:${viewer.revision}`, name: pdfName(url), src: url, text: null, sizeBytes: -1,
    kind: 'pdf', delimiter: null, wrap: settings.wordWrap !== false, revealLine: null, revealRequestId: 0, smoothing: settings.fontSmoothing !== false };
  // The id names whom the page may reach: it narrows its connections once, to its first file's origin.
  return viewerFrameView(viewer.frame, viewerFrameId(`server-pdf-${key}`), config);
}

/**
 * The page's messages for `place`'s server tab (`viewer:<payload>`, `viewer-posted:<key>`, `viewer-pick:<serial>|<row>`):
 * a link opens in the default browser, Try again reads the URL afresh, and Download saves the PDF where the person
 * chooses, as Electron's Save dialog does for the viewer's download. The frame itself never navigates.
 */
export async function serverPdfLocal(client: T3Client, op: string, runtimeId: string, place: string): Promise<string> {
  const prefix = `${place === 'mini' ? 'mini' : 'tab'}|${runtimeId}|`;
  const viewer = [...viewersOf(client)].find(([key]) => key.startsWith(prefix))?.[1];
  if (!viewer) return '';
  if (op.startsWith('viewer-posted:')) { viewer.frame.posts = viewer.frame.posts.filter(post => post.key !== op.slice(14)); return ''; }
  if (op.startsWith('viewer-pick:')) { viewerPick(viewer.frame, op.slice(12)); return ''; }
  if (!op.startsWith('viewer:')) return '';
  const event = viewerMessage(viewer.frame, op.slice(7));
  if (event.type === 'link') openExternal(event.url);
  if (event.type === 'retry') viewer.revision += 1;
  if (event.type === 'download') {
    try { await saveDownload(client, viewer.url, saveName(pdfName(viewer.url), 'document.pdf')); } catch { throw new Error('The file could not be loaded. Try again.'); }
  }
  return '';
}
