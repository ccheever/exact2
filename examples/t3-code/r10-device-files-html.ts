// Lane r10-device: the Files surface's rendered HTML (MIT reference, see LICENSE-T3:
// components/files/FilePreviewPanel.tsx RENDER_BROWSER_FILE_STORAGE_KEY "t3code.renderBrowserFile"
// (default true), renderedToggleLabel, WorkspaceBrowserPreview; browser/openFileInPreview.ts
// isBrowserPreviewFile; components/files/BrowserDocumentFrame.tsx). An `.html` / `.htm` file opens
// as its page; the eye toggle ("Show HTML source" / "Show rendered page") switches to the
// numbered source and back, and the choice is a preference, not a property of one file. The page
// is read from a signed asset URL (`assets.createUrl`: `workspace-file` for a thread,
// `draft-workspace-file` for a draft's workspace), suffixed with the workspace revision so an edit
// reloads it, and shown by the app module's sandboxed WebKit body (R6MediaPreview.swift,
// as the attachment's HTML preview; siblings load only from the token directory, lane r11-misc), never in the app's own session.
import type { T3Client } from './client';
import { obj, str } from './domain';
import type { Native } from './protocol';
import { assetUrl } from './settings-b-icons';

/** isBrowserPreviewFile without PDFs (FilePreviewPanel's `isHtml`). */
export const isHtmlPath = (path: string) => /\.html?$/i.test(path.split(/[?#]/, 1)[0] ?? '');

/** renderedToggleLabel(“html”, rendered). */
export const htmlToggleLabel = (rendered: boolean) => (rendered ? 'Show HTML source' : 'Show rendered page');

/** The asset resource for a file the Files surface shows (a thread's workspace, or a draft's). */
export function htmlResource(threadId: string, cwd: string, absolutePath: string, relativePath: string) {
  return threadId ? { _tag: 'workspace-file', threadId, path: absolutePath } : { _tag: 'draft-workspace-file', cwd, path: relativePath };
}

/** A short revision of the contents last read (FNV-1a), the `workspace-revision` the page reloads on. */
export function contentRevision(text: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index++) { hash ^= text.charCodeAt(index); hash = Math.imul(hash, 0x01000193) >>> 0; }
  return hash.toString(36);
}

/** The page URL with its revision (WorkspaceBrowserPreview's `revisionSuffix`). */
export const revisedUrl = (url: string, revision: string) => (url && revision ? `${url}${url.includes('?') ? '&' : '?'}workspace-revision=${encodeURIComponent(revision)}` : url);

type Minted = { key: string; url: string; at: number; error: string };
const minted = new WeakMap<T3Client, Map<string, Minted>>();
const STALE_URL_MS = 5 * 60_000; // signed URLs live an hour; re-mint after five minutes, as the attachment does

export type HtmlPage = { url: string; error: string };

/** useAssetUrlState for one HTML file: '' while it is minted, an error when the server refuses it. */
export async function htmlPage(client: T3Client, native: Native | null | undefined, cwd: string, absolutePath: string, relativePath: string, revision: string, now = 0): Promise<HtmlPage> {
  let map = minted.get(client);
  if (!map) { map = new Map(); minted.set(client, map); }
  const key = `${client.environmentId}|${client.threadId}|${cwd}|${absolutePath}`;
  let entry = map.get(key);
  const stale = !!entry && !entry.error && now > 0 && entry.at > 0 && now - entry.at > STALE_URL_MS;
  if ((!entry || stale) && native?.available && client.ready) {
    try {
      const result = obj(await client.rpc(native, 'assets.createUrl', { resource: htmlResource(client.threadId, cwd, absolutePath, relativePath) }));
      const url = assetUrl(client.origin, str(result.relativeUrl));
      entry = { key, url, at: now, error: url ? '' : 'Unable to load file preview.' };
    } catch {
      entry = { key, url: '', at: now, error: 'Unable to load file preview.' };
    }
    map.set(key, entry);
  }
  if (!entry) return { url: '', error: '' };
  return { url: revisedUrl(entry.url, revision), error: entry.error };
}

/** Forget a refused mint so a later view tries again (the panel's refresh). */
export function forgetHtmlPages(client: T3Client): void { minted.delete(client); }
