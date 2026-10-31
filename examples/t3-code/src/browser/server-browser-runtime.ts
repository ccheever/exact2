// T3 Code 9c161462c9, ServerBrowserSurface.tsx and serverBrowserStream.ts (MIT).
// The active stream belongs to a connection generation and one server session.
import { appFiles } from '../media/save-file';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { obj, str } from '../client/domain';
import { composerNow } from '../composer/composer-controls';
import type { SocketEvent, SocketHandle } from '../transport/transport-session';
import { downloadStreamFile, uploadStreamFiles, type StreamPicked } from './server-browser-files';
import { ServerBrowserStream, emptyStreamFrame, type StreamFrame, type StreamInput } from './server-browser-stream';

type Runtime = { directory: string; retired: string[]; retryMark: string; threadId: string; tabId: string; request: number; pending: string; maxWidth: number; maxHeight: number; id: string; generation: number; url: string; stream: ServerBrowserStream; width: number; height: number; buttons: number; clickAt: number; clicks: number; clickX: number; clickY: number };
const runtimes = new WeakMap<T3Client, Map<string, Runtime>>();
function runtime(client: T3Client, id: string): Runtime {
  let entries = runtimes.get(client); if (!entries) { entries = new Map(); runtimes.set(client, entries); }
  let state = entries.get(id);
  if (!state) { state = { directory: '', retired: [], retryMark: '', threadId: '', tabId: '', request: 0, pending: '', maxWidth: 1280, maxHeight: 800, id: '', generation: -1, url: '', stream: new ServerBrowserStream(), width: 1280, height: 800, buttons: 0, clickAt: 0, clicks: 0, clickX: 0, clickY: 0 }; entries.set(id, state); }
  return state;
}
export const serverStreamUrl = (client: T3Client, id: string): string => { const state = runtime(client, id); return state.id === id ? state.url : ''; };
export const serverStreamFrame = (client: T3Client, id: string): StreamFrame => { const state = runtime(client, id); return state.id === id ? state.stream.frame : emptyStreamFrame(); };
/** `pip`: the picture-in-picture window's own stream (server-browser-pip.ts), which watches and never operates: the
 *  reference's `interactive: false` client at 1280 × 1280 (serverPictureInPicture.ts). */
export async function prepareServerStream(client: T3Client, native: Native, id: string, threadId: string, tabId: string, pip = false): Promise<void> {
  const state = runtime(client, id), now = composerNow(client);
  if (!id) { await retainServerStreams(client, []); return; }
  if (client.connection !== 'connected') { state.request++; state.pending = ''; state.stream.stop(); state.id = state.url = ''; return; }
  if (state.id === id && state.stream.retryAt && state.url) {
    const mark = `${state.url}:${state.stream.failures}:${state.stream.unauthorized}`;
    if (state.retryMark !== mark) { state.retryMark = mark; state.stream.retryAt = now + state.stream.retryDelay; }
  }
  if (state.id === id && state.generation === client.generation && state.url && (!state.stream.retryAt || now < state.stream.retryAt || state.stream.frame.refused || state.stream.frame.gone || state.stream.frame.setupNeed)) return;
  const generation = client.generation;
  if (state.id && state.generation !== generation) { state.stream.begin(''); state.buttons = 0; }
  const key = `${generation}:${id}`;
  if (state.pending === key) return;
  const request = ++state.request; state.pending = key;
  let access: Record<string, unknown>;
  try {
    if (!state.directory) {
      const directory = (await client.ids(native, 1))[0] ?? '';
      if (!/^[\w-]{1,128}$/.test(directory)) throw new Error('Could not allocate browser frame storage.');
      if (state.request !== request || client.generation !== generation) return;
      state.directory = `app:/tmp/browser-frames/${directory}`;
    }
    access = await client.call(native, { op: 'previewStreamAccess' });
  }
  finally { if (state.request === request) state.pending = ''; }
  if (client.generation !== generation || state.request !== request) return;
  const url = new URL(`${str(access.wsBase)}/ws`);
  const size = pip ? { maxWidth: '1280', maxHeight: '1280', interactive: 'false' } : { maxWidth: String(state.maxWidth * 2), maxHeight: String(state.maxHeight * 2) };
  for (const [key, value] of Object.entries({ threadId, tabId, ...size, wsTicket: str(access.ticket) })) url.searchParams.set(key, value);
  if (state.id !== id) { state.stream.stop(); state.buttons = 0; }
  state.stream.frame.runtimeId = id;
  state.id = id; state.threadId = threadId; state.tabId = tabId; state.generation = generation; state.url = url.href; state.stream.begin(state.url);
}
export function serverStreamSource(client: T3Client, id: string, url: string): unknown {
  const state = runtime(client, id);
  if (!url || id !== state.id || url !== state.url) return emptyStreamFrame();
  return fetch(url, { exactStream: (event: SocketEvent, handle?: SocketHandle) => state.generation === client.generation ? state.stream.event(url, event, handle, composerNow(client)) : emptyStreamFrame() } as RequestInit);
}
type FrameFiles = { rm(path: string, options?: { recursive?: boolean; force?: boolean }): Promise<unknown> };
const frameFiles = () => appFiles().fs as ReturnType<typeof appFiles>['fs'] & FrameFiles;
export async function serverStreamDecode(client: T3Client, id: string): Promise<StreamFrame> {
  const state = runtime(client, id), pending = state.stream.pendingJPEG();
  if (!pending || !state.directory || state.id !== id) return state.stream.frame;
  const generation = state.generation, directory = state.directory, path = `${directory}/${pending.sequence}.jpg`, files = frameFiles();
  try {
    await files.mkdir(directory, { recursive: true }); await files.writeFile(path, new Uint8Array(pending.bytes));
    if (state.id !== id || state.generation !== generation || client.generation !== generation || state.directory !== directory) { await files.rm(path, { force: true }); return state.stream.frame; }
    if (state.stream.frame.image) state.retired.push(state.stream.frame.image);
    return state.stream.decoded(pending.sequence, path);
  } catch (error) { if (letGo(error)) throw error; return state.stream.decodeFailed(pending.sequence, error instanceof Error ? error.message : 'The browser frame could not be displayed.'); }
}
export async function serverStreamPainted(client: T3Client, id: string, image: string): Promise<StreamFrame> {
  const state = runtime(client, id); if (state.id !== id) return emptyStreamFrame();
  const frame = state.stream.painted(image);
  for (const path of state.retired.splice(0)) await frameFiles().rm(path, { force: true }).catch(() => undefined);
  return frame;
}

const modifiers = (event: Record<string, unknown>): number => (event.altKey ? 1 : 0) | (event.ctrlKey ? 2 : 0) | (event.metaKey ? 4 : 0) | (event.shiftKey ? 8 : 0);
const buttonName = (bit: number): string => bit === 1 ? 'left' : bit === 2 ? 'right' : bit === 4 ? 'middle' : 'none';
export function streamPagePoint(x: number, y: number, boxWidth: number, boxHeight: number, pageWidth: number, pageHeight: number): { x: number; y: number } | null {
  if (!(boxWidth > 0 && boxHeight > 0 && pageWidth > 0 && pageHeight > 0)) return null;
  const scale = Math.min(boxWidth / pageWidth, boxHeight / pageHeight), left = (boxWidth - pageWidth * scale) / 2, top = (boxHeight - pageHeight * scale) / 2;
  const px = (x - left) / scale, py = (y - top) / scale;
  return px < 0 || py < 0 || px > pageWidth || py > pageHeight ? null : { x: Math.round(px), y: Math.round(py) };
}
/** A synchronous mutation sends input in its own turn; network callbacks never
 * depend on the next snapshot to acknowledge receipt or release a key. */
export function serverStreamInput(client: T3Client, id: string, kind: string, raw: unknown): StreamFrame {
  const state = runtime(client, id);
  if (state.id !== id || state.generation !== client.generation) return emptyStreamFrame();
  const value = obj(raw), stream = state.stream, frame = stream.frame;
  if (kind === 'retry') { stream.retry(); state.url = ''; stream.requestReconnect(); return stream.frame; }
  if (kind === 'resize') {
    const width = Math.max(1, Math.min(3840, Math.round(Number(value.width) || 1))), height = Math.max(1, Math.min(2160, Math.round(Number(value.height) || 1)));
    if (width !== state.width || height !== state.height) { state.width = width; state.height = height; stream.send({ type: 'resize', width, height }); if (width > state.maxWidth || height > state.maxHeight) { state.maxWidth = Math.max(width, state.maxWidth); state.maxHeight = Math.max(height, state.maxHeight); state.url = ''; stream.requestReconnect(); } }
    return stream.frame;
  }
  if (kind === 'mouse' || kind === 'wheel') {
    const point = streamPagePoint(Number(value.x), Number(value.y), Number(value.width), Number(value.height), frame.width, frame.height);
    if (!point) return frame;
    if (kind === 'wheel') {
      const factor = value.deltaMode === 1 ? 16 : value.deltaMode === 2 ? frame.height : 1;
      stream.send({ type: 'wheel', ...point, deltaX: (Number(value.deltaX) || 0) * factor, deltaY: (Number(value.deltaY) || 0) * factor, modifiers: modifiers(value) });
    } else {
      const buttons = Number(value.buttons) || 0, changed = state.buttons ^ buttons, phase = str(value.phase);
      if (phase === 'down') {
        const now = composerNow(client); state.clicks = now - state.clickAt <= 500 && Math.abs(point.x - state.clickX) <= 4 && Math.abs(point.y - state.clickY) <= 4 ? state.clicks % 3 + 1 : 1;
        state.clickAt = now; state.clickX = point.x; state.clickY = point.y;
      }
      for (const bit of [1, 2, 4]) if (changed & bit) stream.send({ type: 'mouse', action: buttons & bit ? 'down' : 'up', ...point, button: buttonName(bit), buttons, clickCount: state.clicks, modifiers: modifiers(value) });
      if (phase === 'move') stream.send({ type: 'mouse', action: 'move', ...point, button: 'none', buttons, clickCount: 0, modifiers: modifiers(value) });
      state.buttons = buttons;
    }
  } else if (kind === 'key') {
    const key = str(value.key), code = str(value.code), printable = [...key].length === 1 && !value.ctrlKey && !value.metaKey;
    stream.send({ type: 'key', action: value.phase === 'up' ? 'up' : 'down', key, code, keyCode: key.length === 1 ? key.toUpperCase().charCodeAt(0) : ({ Enter: 13, Backspace: 8, Tab: 9, Escape: 27, ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40, Delete: 46, Home: 36, End: 35, PageUp: 33, PageDown: 34 }[key] ?? 0),
      ...((printable || key === 'Enter') && value.phase !== 'up' && !value.ctrlKey && !value.metaKey ? { text: key === 'Enter' ? '\r' : key } : {}), modifiers: modifiers(value) });
  } else if (kind === 'text') { if (typeof value.text === 'string' && value.text) stream.send({ type: 'text', text: value.text }); }
  else if (kind === 'dialog') stream.send({ type: 'dialog', accept: value.accept === true, promptText: str(value.promptText) });
  else if (kind === 'takeControl' || kind === 'releaseControl') { if (stream.send({ type: kind }) && kind === 'takeControl') state.width = state.height = 0; }
  return stream.frame;
}
export function sendServerCommand(client: T3Client, id: string, input: StreamInput): boolean { const state = runtime(client, id); return state.id === id && state.stream.send(input); }

export async function drainServerNotices(client: T3Client, native: Native, id: string, popup: (tabId: string) => void): Promise<void> {
  const state = runtime(client, id), stream = state.stream, generation = state.generation;
  const current = () => state.id === id && client.generation === generation;
  for (const notice of stream.drainNotices()) {
    if (!current()) return;
    if (notice.type === 'popup') popup(notice.tabId);
    else if (notice.type === 'download') {
      try { await downloadStreamFile(client, native, state.threadId, state.tabId, notice, current); }
      catch (error) { if (letGo(error)) throw error; if (current()) stream.transferError(error instanceof Error ? error.message : 'The browser download failed.'); }
    }
  }
}
export async function serverStreamUpload(client: T3Client, native: Native, id: string, chooser: string, files: StreamPicked[]): Promise<StreamFrame> {
  const state = runtime(client, id), generation = client.generation;
  const current = () => state.id === id && state.generation === generation && state.stream.frame.chooserId === chooser && state.stream.frame.controlled;
  if (!current()) return state.stream.frame;
  try { await uploadStreamFiles(client, native, state.threadId, state.tabId, chooser, files, current); }
  catch (error) { if (letGo(error)) throw error; if (current()) state.stream.transferError(error instanceof Error ? error.message : 'The browser upload failed.'); }
  return state.stream.frame;
}

/** Keep only visible page/mini streams alive. Hidden tabs retain their server
 * session, while their sockets and decoded JPEGs are released. */
export async function retainServerStreams(client: T3Client, ids: string[]): Promise<void> {
  for (const [id, state] of runtimes.get(client) ?? []) if (!ids.includes(id) && (state.id || state.pending)) {
    state.request++; state.pending = ''; state.stream.stop(); state.id = state.url = '';
    const directory = state.directory; state.directory = ''; state.retired = [];
    if (directory) await frameFiles().rm(directory, { recursive: true, force: true }).catch(() => undefined);
  }
}
