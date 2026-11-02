// The Files surface's rendered HTML (MIT reference, see LICENSE-T3:
// components/files/FilePreviewPanel.tsx RENDER_BROWSER_FILE_STORAGE_KEY "t3code.renderBrowserFile"
// (default true), renderedToggleLabel, WorkspaceBrowserPreview and WorkspaceAudioPreview; filePreviewMode.ts
// workspaceAssetResource; browser/openFileInPreview.ts isBrowserPreviewFile; components/files/BrowserDocumentFrame.tsx;
// T3 Code 2359643cff). An `.html` / `.htm` file opens as its page; the eye toggle ("Show HTML source" / "Show
// rendered page") switches to the numbered source and back, and the choice is a preference, not a property of one
// file. The page is read from a signed asset URL (`assets.createUrl`: `workspace-file` for a thread, `media-file` for a
// file outside the workspace, `draft-workspace-file` for a draft's workspace), suffixed with the workspace revision
// (ChatView's workspaceMutationId), so a workspace change reloads it. A renewed signature keeps the URL the page opened
// with until that revision changes, so a page's form input and scroll survive it (BrowserDocumentFrame's
// `documentKey`). The page is shown by the app module's sandboxed WebKit body (T3MediaPreview.swift, as the attachment's
// HTML preview; siblings load only from the token directory), never in the app's own session. A PDF and an audio file
// are `media-file` URLs signed again for each workspace change (the revisioned asset URL of WorkspaceBrowserPreview and
// WorkspaceAudioPreview); each keeps its URL while the server reports the same `fileRevision`, so a command that leaves
// the file alone does not reload it. A PDF opens in the viewer page's PDF.js (files-viewer.ts), mounted again for each
// revision its URL carries, as the reference's BrowserDocumentFrame keys it.
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { assetUrl } from '../settings/settings-b-icons';
import { letGo } from '../client/let-go';
import { mediaFileReference } from '../media/media-reference';

/** browser/openFileInPreview.ts isBrowserPreviewFile: a page or a PDF, which the integrated browser opens. */
export const isBrowserPreviewFile = (path: string) => /\.(?:html?|pdf)$/i.test(path.split(/[?#]/, 1)[0] ?? '');

/** BrowserDocumentFrame isPdfPreviewFile: a PDF renders as its document, never as text (FilePreviewPanel's `isPdf`). */
export const isPdfPath = (path: string) => /\.pdf$/i.test(path.split(/[?#]/, 1)[0] ?? '');

/** isBrowserPreviewFile without PDFs (FilePreviewPanel's `isHtml`). */
export const isHtmlPath = (path: string) => /\.html?$/i.test(path.split(/[?#]/, 1)[0] ?? '');

/** renderedToggleLabel(“html”, rendered). */
export const htmlToggleLabel = (rendered: boolean) => (rendered ? 'Show HTML source' : 'Show rendered page');

/** workspaceAssetResource: the asset for a file the Files surface shows. A draft has no thread on the server yet, so it
 *  names its workspace root instead of a thread to resolve one from. The server signs `workspace-file` only below the
 *  workspace root, so a file outside it (an absolute link in the thread) is the host `media-file` the media viewer uses. */
export function workspaceAssetResource(kind: 'workspace-file' | 'media-file', threadId: string, cwd: string, absolutePath: string) {
  if (!threadId) return { _tag: 'draft-workspace-file', cwd, path: absolutePath };
  return { _tag: mediaFileReference(absolutePath, cwd).relativePath !== undefined ? kind : 'media-file', threadId, path: absolutePath };
}
/** A page: its `workspace-file`, or the `media-file` of one outside the workspace. */
export const htmlResource = (threadId: string, cwd: string, absolutePath: string) => workspaceAssetResource('workspace-file', threadId, cwd, absolutePath);

/** The page URL with its revision (WorkspaceBrowserPreview's `revisionSuffix`; an audio file's may be `file-revision`). */
export const revisedUrl = (url: string, revision: string, name = 'workspace-revision') => (url && revision ? `${url}${url.includes('?') ? '&' : '?'}${name}=${encodeURIComponent(revision)}` : url);

/**
 * One asset's signature: `signedFor` the workspace revision it was signed for, `fileRevision` the file's own revision
 * the server reported with it, and `shown` the URL the preview shows for a revision.
 */
type Minted = { key: string; url: string; at: number; error: string; signedFor: string; fileRevision: string; shown?: { revision: string; url: string } };
const minted = new WeakMap<T3Client, Map<string, Minted>>();
const STALE_URL_MS = 5 * 60_000; // signed URLs live an hour; re-mint after five minutes, as the attachment does

/** `revision`: the one the URL carries, `file:<the file's own>` when the server reports it, else the workspace's. */
export type HtmlPage = { url: string; error: string; revision?: string };

/**
 * useAssetUrlState for one Files asset: '' while it is minted, `failure` when the server refuses it. `perRevision`
 * signs it again when the revision changes (useAssetUrlState's `revision`), not only after five minutes.
 */
async function filesAsset(client: T3Client, native: Native | null | undefined, kind: 'workspace-file' | 'media-file', cwd: string, absolutePath: string, revision: string, now: number, failure: string, perRevision = false): Promise<HtmlPage> {
  let map = minted.get(client);
  if (!map) { map = new Map(); minted.set(client, map); }
  const key = assetKey(client, kind, cwd, absolutePath);
  let entry = map.get(key);
  const previous = entry;
  const stale = !!entry && !entry.error && now > 0 && entry.at > 0 && now - entry.at > STALE_URL_MS;
  const resign = perRevision && !!entry && !entry.error && entry.signedFor !== revision;
  if ((!entry || stale || resign) && native?.available && client.ready) {
    try {
      const result = obj(await client.rpc(native, 'assets.createUrl', { resource: workspaceAssetResource(kind, client.threadId, cwd, absolutePath) }));
      const url = assetUrl(client.origin, str(result.relativeUrl));
      entry = { key, url, at: now, error: url ? '' : failure, signedFor: revision, fileRevision: perRevision ? str(result.fileRevision) : '' };
    } catch (error) {
      if (letGo(error)) throw error;
      entry = { key, url: '', at: now, error: failure, signedFor: revision, fileRevision: '' };
    }
    if (previous?.shown) entry.shown = previous.shown;
    map.set(key, entry);
  }
  if (!entry || entry.error) return { url: '', error: entry?.error ?? '' };
  // WorkspaceAudioPreview: `fileRevision ?? workspaceMutationId`, and the URL names which one it carries.
  const shownRevision = entry.fileRevision ? `file:${entry.fileRevision}` : revision;
  if (entry.shown?.revision !== shownRevision) entry.shown = { revision: shownRevision, url: entry.fileRevision ? revisedUrl(entry.url, entry.fileRevision, 'file-revision') : revisedUrl(entry.url, revision) };
  return { url: entry.shown.url, error: '', revision: shownRevision };
}
const assetKey = (client: T3Client, kind: string, cwd: string, absolutePath: string) => `${client.environmentId}|${client.threadId}|${cwd}|${absolutePath}|${kind}`;

/**
 * One HTML file or PDF's signed URL. A PDF is a `media-file` signed again for each workspace revision, whose URL changes
 * only with the file's own revision when the server reports one, so an agent's reply that leaves it alone keeps its page.
 */
export function htmlPage(client: T3Client, native: Native | null | undefined, cwd: string, absolutePath: string, revision: string, now = 0): Promise<HtmlPage> {
  const pdf = isPdfPath(absolutePath);
  return filesAsset(client, native, pdf ? 'media-file' : 'workspace-file', cwd, absolutePath, revision, now, 'Unable to load file preview.', pdf);
}
/**
 * WorkspaceAudioPreview's `media-file` URL for workspace revision `revision`, signed again when that revision changes;
 * the URL changes only with the file's revision when the server reports one. A refused one is "Unable to load audio.".
 */
export function audioPage(client: T3Client, native: Native | null | undefined, cwd: string, absolutePath: string, revision: string, now = 0): Promise<HtmlPage> {
  return filesAsset(client, native, 'media-file', cwd, absolutePath, revision, now, 'Unable to load audio.', true);
}
/** Try again on an audio file: its URL is signed afresh (useAssetUrlRefresh). */
export function forgetAudioPage(client: T3Client, cwd: string, absolutePath: string): void { minted.get(client)?.delete(assetKey(client, 'media-file', cwd, absolutePath)); }

/** Forget a refused mint so a later view tries again (the panel's refresh). */
export function forgetHtmlPages(client: T3Client): void { minted.delete(client); }
