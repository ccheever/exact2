// The document viewers' page as the app frames it (file-preview-host/src/entry.ts, an LLP 1020.001 bundled page built
// into assets/file-preview/; viewer-frame.contract). The page runs the reference's viewers (MIT reference, see
// LICENSE-T3: apps/web/src/components/files/; T3 Code 7c3b3b3605ef) and fetches the file at the signed URL its config
// names; the app keeps what surrounds them (AttachmentFilePreview's subheader, Copy and Save, its failures). A
// frame's state here holds:
// - the posts the page has not taken (a draft's bytes, a menu's pick, a Copy contents request), one at a time, dropped
//   when the page boots again, since a new document has none of the old one's state;
// - a Select's popup, a menu-shaped popover at the trigger the page names, which a newer one supersedes;
// - what the page reported that the app draws: an archive's opened entry, a notebook's cells, an output's height.
import { obj, str, type Obj } from '../client/domain';

export const VIEWER_PAGE = 'assets/file-preview/index.html';
/** A draft's bytes go to the page in posts of this many bytes, before base64. */
const CHUNK_BYTES = 1024 * 1024;

export type ViewerPost = { key: string; text: string };
export type ViewerMenuRow = { id: string; label: string; selected: boolean };
export type ViewerMenu = { serial: number; x: number; y: number; width: number; height: number; label: string; rows: ViewerMenuRow[] };
/** What a frame shows: the page, its config (JSON, posted at each boot and on each change), the posts and the menu. */
export type ViewerFrameView = { id: string; src: string; config: string; posts: ViewerPost[]; menu: ViewerMenu };

/** What the page shows and how (entry.ts `Config`, without its theme, which the frame adds). */
export type ViewerConfig = {
  view: 'pdf' | 'text' | 'office' | 'binary' | 'output';
  /** The viewer's identity: a new key mounts a new viewer, as the reference's React keys do. */
  key: string;
  /** A PDF's identity without its revision: a new key of the same document opens where the reader was
   *  (BrowserDocumentFrame's `documentId`). */
  documentId?: string;
  name: string;
  /** The signed URL, or '' for a draft: its bytes posted after it, or the file the page is given (`given`). */
  src: string;
  /** The page shows the file it was given in its own `src` (LLP 1020.001 D7): a draft past what a message carries. */
  given?: boolean;
  text: string | null;
  /** -1 when the app knows none (a Files read). */
  sizeBytes: number;
  kind: string;
  delimiter: ',' | '\t' | null;
  wrap: boolean;
  revealLine: number | null;
  revealRequestId: number;
  /** Settings › Appearance › Font smoothing: the reference's antialiased text while it is on. */
  smoothing: boolean;
};

/** An archive's opened entry, for the app to draw (BinaryViewer ArchiveFile). */
export type ViewerEntry = { path: string; view: 'text' | 'image' | 'failure'; text: string; truncated: boolean; data: string; message: string };
export type NotebookOutput = { kind: 'text' | 'image' | 'html'; text: string };
export type NotebookCell = { kind: 'markdown' | 'code' | 'raw'; source: string; outputs: NotebookOutput[] };
export type ViewerNotebook = { language: string; cells: NotebookCell[]; sizeBytes: number | null };

export type FrameState = {
  booted: boolean;
  posts: ViewerPost[];
  serial: number;
  menu: ViewerMenu;
  entry: ViewerEntry | null;
  notebook: ViewerNotebook | null;
  height: number;
  /** The config key whose draft bytes were queued since the page last booted. */
  sentBytes: string;
  /** The page's own number for the open menu, which its pick names. */
  pageSerial: number;
  /** The config key the frame last showed. */
  configKey: string;
  /**
   * The config key a Copy contents was asked for, until its page answers: only an answer for that key is the
   * clipboard's, and the request goes when the frame shows another key or another file (`dropCopy`).
   */
  copyKey: string | null;
};

const emptyMenu = (): ViewerMenu => ({ serial: 0, x: 0, y: 0, width: 0, height: 0, label: '', rows: [] });
export const newFrame = (): FrameState => ({ booted: false, posts: [], serial: 0, menu: emptyMenu(), entry: null, notebook: null, height: 0, sentBytes: '', pageSerial: 0, configKey: '', copyKey: null });
export const emptyViewerFrame = (): ViewerFrameView => ({ id: '', src: '', config: '', posts: [], menu: emptyMenu() });

/** A frame's id: a preview key, made safe for an element id and still one per key (`a.b` and `a_b` differ). */
export const viewerFrameId = (key: string) => `viewer-${key.replace(/[^A-Za-z0-9-]/g, (char) => `_${char.codePointAt(0)!.toString(16)}_`)}`;

/**
 * Whom the page may reach for `src` (entry.ts `narrowConnections`): its http(s) origin, or `local` for a draft's Blob,
 * a text the app read or none. The page narrows its policy once, so a frame keyed by this mounts again for another.
 */
export function connectionScope(src: string): string {
  try {
    const url = new URL(src);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.origin : 'local';
  } catch { return 'local'; }
}

/** The frame's view: the config as JSON and the page's next post, once it has booted. */
export function viewerFrameView(state: FrameState, id: string, config: ViewerConfig, given = ''): ViewerFrameView {
  if (state.configKey !== config.key) { state.configKey = config.key; state.copyKey = null; }
  // A file in app:/cache named in the page's own query, which the host puts a URL the page can fetch in place of.
  const src = given ? `${VIEWER_PAGE}?file=${encodeURIComponent(given)}` : VIEWER_PAGE;
  return { id, src, config: JSON.stringify({ type: 'config', ...config }), posts: state.booted ? state.posts.slice(0, 1) : [], menu: state.menu };
}

/** Queues `message` for the page; it goes once the page has booted, after the posts before it. */
export function postToViewer(state: FrameState, message: Obj): void {
  state.serial += 1;
  state.posts.push({ key: `post-${state.serial}`, text: JSON.stringify(message) });
}

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function base64(bytes: Uint8Array): string {
  const parts: string[] = [];
  for (let start = 0; start < bytes.length; start += 12_288) {
    const end = Math.min(bytes.length, start + 12_288);
    let out = '';
    for (let at = start; at < end; at += 3) {
      const word = (bytes[at]! << 16) | ((bytes[at + 1] ?? 0) << 8) | (bytes[at + 2] ?? 0);
      out += BASE64[(word >> 18) & 63]! + BASE64[(word >> 12) & 63]! + (at + 1 < end ? BASE64[(word >> 6) & 63]! : '=') + (at + 2 < end ? BASE64[word & 63]! : '=');
    }
    parts.push(out);
  }
  return parts.join('');
}

/** Copy contents of the file the frame shows: asked of its page now, or when that page boots, and answered once. */
export function requestCopy(state: FrameState): void {
  if (state.copyKey !== null && state.copyKey === state.configKey) return;
  state.copyKey = state.configKey;
  if (state.booted) postToViewer(state, { type: 'copy', key: state.copyKey });
}
/** The frame no longer shows its page (another body took the surface): a Copy asked of it is withdrawn. */
export function dropCopy(state: FrameState): void { state.copyKey = null; }

/**
 * The surface draws something else in the frame's place (a failure, a notebook's cells, a load), so its page is gone:
 * what it was sent goes with it, and nothing more is sent, a draft's bytes neither read nor encoded, until the next
 * page boots.
 */
export function frameGone(state: FrameState): void {
  if (state.booted) Object.assign(state, { booted: false, posts: [], sentBytes: '' });
}

/** A draft's bytes for the viewer `key`, in base64 posts, the last one `done`. */
export function postBytes(state: FrameState, key: string, bytes: Uint8Array): void {
  state.sentBytes = key;
  if (bytes.length === 0) { postToViewer(state, { type: 'bytes', key, data: '', done: true }); return; }
  for (let start = 0; start < bytes.length; start += CHUNK_BYTES) {
    postToViewer(state, { type: 'bytes', key, data: base64(bytes.subarray(start, start + CHUNK_BYTES)), done: start + CHUNK_BYTES >= bytes.length });
  }
}

/** What a page message asks of the frame's owner, beyond the frame state it updates here. */
export type ViewerEvent =
  | { type: 'boot' }
  | { type: 'copy'; text: string }
  | { type: 'copy-error'; message: string }
  | { type: 'retry' }
  | { type: 'link'; url: string }
  | { type: 'error'; message: string }
  | { type: 'none' };

/** A page message (`payload`, JSON): the frame state it changes, and what its owner must do. */
export function viewerMessage(state: FrameState, payload: string): ViewerEvent {
  let message: Obj;
  try { message = obj(JSON.parse(payload)); } catch { return { type: 'none' }; }
  switch (message.type) {
    case 'boot':
      // A new document: what the old one was told and reported is gone with it.
      Object.assign(state, { booted: true, posts: [], menu: { ...state.menu, serial: state.menu.serial + 1, rows: [] }, entry: null, sentBytes: '' });
      // A Copy asked before this page booted goes to it, if it still shows that file.
      if (state.copyKey !== null && state.copyKey === state.configKey) postToViewer(state, { type: 'copy', key: state.copyKey });
      return { type: 'boot' };
    case 'posted':
      state.posts = state.posts.filter(post => post.key !== str(message.key));
      return { type: 'none' };
    case 'menu': {
      const rows = Array.isArray(message.rows) ? message.rows.map(row => obj(row)).map(row => ({ id: str(row.id), label: str(row.label), selected: row.selected === true })) : [];
      state.menu = { serial: state.menu.serial + 1, x: Number(message.x) || 0, y: Number(message.y) || 0, width: Number(message.width) || 0, height: Number(message.height) || 0, label: str(message.label), rows };
      state.pageSerial = Number(message.serial) || 0;
      return { type: 'none' };
    }
    case 'entry':
      state.entry = { path: str(message.path), view: message.view === 'image' ? 'image' : message.view === 'text' ? 'text' : 'failure', text: str(message.text), truncated: message.truncated === true, data: str(message.data), message: str(message.message) };
      return { type: 'none' };
    case 'entry-closed':
      state.entry = null;
      return { type: 'none' };
    case 'notebook': {
      const cells = (Array.isArray(message.cells) ? message.cells : []).map(value => {
        const cell = obj(value);
        return {
          kind: cell.kind === 'markdown' ? 'markdown' as const : cell.kind === 'code' ? 'code' as const : 'raw' as const,
          source: str(cell.source),
          outputs: (Array.isArray(cell.outputs) ? cell.outputs : []).map(entry => obj(entry)).map(output => ({ kind: output.kind === 'image' ? 'image' as const : output.kind === 'html' ? 'html' as const : 'text' as const, text: str(output.text) })),
        };
      });
      state.notebook = { language: str(message.language), cells, sizeBytes: typeof message.sizeBytes === 'number' ? message.sizeBytes : null };
      return { type: 'none' };
    }
    case 'height':
      state.height = Math.max(0, Number(message.value) || 0);
      return { type: 'none' };
    // The clipboard takes text only for a Copy the app asked for, of the file it was asked for.
    case 'copy':
    case 'copy-error':
      if (state.copyKey === null || str(message.key) !== state.copyKey) return { type: 'none' };
      state.copyKey = null;
      return message.type === 'copy' ? { type: 'copy', text: str(message.text) } : { type: 'copy-error', message: str(message.message, 'Try again.') };
    case 'retry': return { type: 'retry' };
    case 'link': return { type: 'link', url: str(message.url) };
    case 'error': return { type: 'error', message: str(message.message) };
    default: return { type: 'none' };
  }
}

/** A menu row was chosen (`<serial>|<row>`): the page hears it; a row of a superseded menu does nothing. */
export function viewerPick(state: FrameState, value: string): void {
  const bar = value.indexOf('|');
  const serial = Number(value.slice(0, bar)), id = value.slice(bar + 1);
  if (serial !== state.menu.serial || !state.menu.rows.length) return;
  state.menu = { ...state.menu, rows: [] };
  postToViewer(state, { type: 'pick', serial: state.pageSerial, id });
}
