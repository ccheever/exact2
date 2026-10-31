// The Files surface's viewer page (MIT reference, see LICENSE-T3: components/files/FilePreviewPanel.tsx
// WorkspaceBrowserPreview, DelimitedTablePreview and WorkspaceLargeTextPreview; T3 Code 7c3b3b3605ef): a PDF in PDF.js,
// keyed on the workspace revision as WorkspaceBrowserPreview's `documentKey` is; a rendered CSV or TSV table over the
// text Files read, every row; and a text file past 1 MB as "Full file", streamed from a `media-file` URL signed for
// download and again for each workspace change, with Copy contents and Save file. The page itself is viewer-frames.ts's.
import type { T3Client } from '../client/client';
import { obj, str } from '../client/domain';
import type { Native } from '../client/protocol';
import { assetUrl } from '../settings/settings-b-icons';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { saveAttachment } from './attachment-files';
import { revisedUrl } from './files-rendered-html';
import { connectionScope, dropCopy, emptyViewerFrame, newFrame, requestCopy, viewerFrameId, viewerFrameView, viewerMessage, viewerPick, type FrameState, type ViewerConfig, type ViewerFrameView } from './viewer-frames';
import { openExternal } from '../client/external-open';

export type FullFileView = { open: boolean; copyLabel: string; copied: boolean; saving: boolean; saveLabel: string };
export const closedFullFile = (): FullFileView => ({ open: false, copyLabel: 'Copy contents', copied: false, saving: false, saveLabel: 'Save file' });

type Signed = { key: string; url: string; at: number; error: string; signedFor: string; fileRevision: string };
/** `loaded`: the URL the viewer opened for a content revision, kept while that revision holds (WorkspaceLargeTextPreview). */
type State = { frame: FrameState; signed: Signed | null; loaded: { revision: string; url: string } | null; refresh: number; copiedAt: number; saving: boolean; name: string; path: string };
const states = new WeakMap<T3Client, State>();
const STALE_URL_MS = 5 * 60_000;

function stateOf(client: T3Client): State {
  let state = states.get(client);
  if (!state) { state = { frame: newFrame(), signed: null, loaded: null, refresh: 0, copiedAt: 0, saving: false, name: '', path: '' }; states.set(client, state); }
  return state;
}

/** WorkspaceLargeTextPreview's resource: the file as a download, which a text file of any size is served as. */
const fullResource = (client: T3Client, absolutePath: string) => ({ _tag: 'media-file', threadId: client.threadId, path: absolutePath, disposition: 'attachment' });

/** One signature of the full file. */
async function mintFull(client: T3Client, native: Native, absolutePath: string, revision: string, now: number): Promise<Signed> {
  const key = `${client.environmentId}|${client.threadId}|${absolutePath}`;
  try {
    const result = obj(await client.rpc(native, 'assets.createUrl', { resource: fullResource(client, absolutePath) }));
    const url = assetUrl(client.origin, str(result.relativeUrl));
    return { key, url, at: now, error: url ? '' : 'Unable to load file preview.', signedFor: revision, fileRevision: str(result.fileRevision) };
  } catch (error) {
    if (letGo(error)) throw error;
    return { key, url: '', at: now, error: 'Unable to load file preview.', signedFor: revision, fileRevision: '' };
  }
}

/**
 * useAssetUrlState for the full file, signed again for each workspace revision and after five minutes; a refusal stays
 * until Try again or the next revision.
 */
async function signFull(client: T3Client, native: Native, absolutePath: string, revision: string, now: number): Promise<Signed> {
  const state = stateOf(client), key = `${client.environmentId}|${client.threadId}|${absolutePath}`;
  const known = state.signed?.key === key ? state.signed : null;
  if (known && known.signedFor === revision && (known.error || !(now > 0 && known.at > 0 && now - known.at > STALE_URL_MS))) return known;
  state.signed = await mintFull(client, native, absolutePath, revision, now);
  return state.signed;
}

export type FilesViewerRequest = {
  kind: 'pdf' | 'table' | 'full';
  path: string; absolute: string; name: string;
  /** The workspace revision (ChatView's workspaceMutationId). */
  revision: string;
  /** A PDF's signed page URL (files-rendered-html.ts htmlPage). */
  url: string;
  /** A table's text, as Files read it. */
  text: string;
  delimiter: ',' | '\t' | null;
  revealLine: number | null; revealRequestId: number;
  /** The preview's word wrap: its own toggle's, else Settings' (files-surface.ts). */
  wrap: boolean;
};

/** The viewer's frame for what Files shows, the Full file subheader's controls, and why it cannot load. */
export async function filesViewer(client: T3Client, native: Native | null | undefined, request: FilesViewerRequest, now: number): Promise<{ viewer: ViewerFrameView; full: FullFileView; error: string }> {
  const state = stateOf(client);
  const settings = client.local.clientSettings as { fontSmoothing?: boolean };
  const base = { name: request.path, src: '', text: null, sizeBytes: -1, kind: '', delimiter: null, wrap: request.wrap, revealLine: null, revealRequestId: 0, smoothing: settings.fontSmoothing !== false };
  let config: ViewerConfig;
  let full = closedFullFile();
  if (request.kind === 'pdf') {
    config = { ...base, view: 'pdf', key: `pdf:${client.environmentId}:${client.threadId}:${request.absolute}:${request.revision}`, src: request.url };
  } else if (request.kind === 'table') {
    // LargeTextViewer parses its text again when the text changes: a new read of the file is a new viewer.
    config = { ...base, view: 'text', key: `table:${request.path}:${request.revision}:${request.text.length}:${state.refresh}`, text: request.text, delimiter: request.delimiter };
  } else {
    if (!native?.available) return { viewer: emptyViewerFrame(), full, error: '' };
    const signed = await signFull(client, native, request.absolute, request.revision, now);
    if (signed.error) return { viewer: emptyViewerFrame(), full, error: signed.error };
    // The content's revision: the file's own when the server reports it, else the workspace's. A renewed signature
    // keeps the URL the viewer opened with until it changes.
    const revision = JSON.stringify([client.environmentId, fullResource(client, request.absolute), signed.fileRevision || request.revision, state.refresh]);
    if (state.loaded?.revision !== revision) state.loaded = { revision, url: signed.url };
    const src = revisedUrl(state.loaded.url, revision, 'revision');
    // WorkspaceLargeTextPreview's key: the file, whether it shows as a table, and the panel's Refresh.
    config = { ...base, view: 'text', key: `${src}:${request.delimiter ? 'table' : 'source'}:${state.refresh}`, src, delimiter: request.delimiter, revealLine: request.revealLine, revealRequestId: request.revealRequestId };
    if (state.copiedAt < 0 && now) state.copiedAt = now;
    const copied = state.copiedAt < 0 || (state.copiedAt > 0 && now - state.copiedAt < 2000);
    full = { open: true, copyLabel: copied ? 'Copied' : 'Copy contents', copied, saving: state.saving, saveLabel: state.saving ? 'Preparing file…' : 'Save file' };
  }
  state.name = request.name;
  state.path = request.absolute;
  // The frame's id names the environment and whom its page may reach (`connectionScope`). A page narrows its
  // connections once, to its first file's origin or, for a table's text, to none; files-surface.contract mounts a new
  // frame for a new id.
  return { viewer: viewerFrameView(state.frame, viewerFrameId(`files-${client.environmentId}-${connectionScope(config.src)}`), config), full, error: '' };
}

/** `surface-files-viewer*` and the Full file subheader (`full-copy`, `full-save`). */
export async function filesViewerOp(client: T3Client, native: Native, op: string, now: number): Promise<void> {
  const state = stateOf(client);
  if (op.startsWith('viewer-posted:')) { state.frame.posts = state.frame.posts.filter(post => post.key !== op.slice(14)); return; }
  if (op.startsWith('viewer-pick:')) { viewerPick(state.frame, op.slice(12)); return; }
  if (op === 'full-copy') { requestCopy(state.frame); return; }
  // The signature's failure's Try again: signed afresh, the viewer mounting again.
  if (op === 'full-retry') { state.signed = null; state.refresh += 1; return; }
  if (op === 'full-save') {
    if (state.saving || !state.path) return;
    state.saving = true;
    try {
      const signed = await mintFull(client, native, state.path, '', now);
      if (!signed.url) throw new Error('Reconnect to the environment and try again.');
      await saveAttachment(client, signed.url, state.name);
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Could not read file', description: error instanceof Error && error.message ? error.message : 'Try again.' });
    } finally { state.saving = false; }
    return;
  }
  if (!op.startsWith('viewer:')) return;
  const event = viewerMessage(state.frame, op.slice(7));
  // WorkspaceLargeTextPreview's onRetry: the file is signed afresh and the viewer mounts again.
  if (event.type === 'retry') { state.signed = null; state.refresh += 1; return; }
  if (event.type === 'link') { openExternal(event.url); return; }
  if (event.type === 'copy') {
    try { await client.restAccess(native).call({ op: 'copyText', text: event.text }); state.copiedAt = now || -1; }
    catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'error', title: 'Could not read file', description: error instanceof Error && error.message ? error.message : 'Try again.' }); }
  }
  if (event.type === 'copy-error') pushToast(client, { kind: 'error', title: 'Could not read file', description: event.message });
}

/** Files shows another body: the frame unmounts, and a Copy asked of it is withdrawn. */
export function filesViewerHidden(client: T3Client): void { const state = states.get(client); if (state) dropCopy(state.frame); }

/** The panel's Refresh: the full file is signed afresh and its viewer mounts again (fileRefreshRevision). */
export function refreshFilesViewer(client: T3Client): void {
  const state = stateOf(client);
  state.signed = null;
  state.refresh += 1;
}
