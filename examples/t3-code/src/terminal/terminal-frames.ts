// The terminal pages the app shows (terminal-host/src/entry.ts, an LLP 1020.001 bundled page in an `iframe`,
// terminal-frame.contract): what each page is told beyond its config, and what it reports. T3 Code 74541bab17
// (MIT reference, see LICENSE-T3: apps/web/src/components/ThreadTerminalDrawer.tsx) keeps the terminal and its
// attach stream in the renderer; here the page holds them and opens its own socket, with a ticket minted here for
// the terminal's environment (`ticket` → `socket`). This side keeps:
// - the posts a page has not taken yet (`posts`, acknowledged one at a time by the frame), dropped when the page
//   boots again, since a new document has none of the old one's state. A taken post leaves the view at once, so a
//   socket URL's ticket stays in the view only until the frame has posted it;
// - the selection popup and context menu (ThreadTerminalDrawer.tsx terminalSelectionMenuItems,
//   terminalContextMenuItems), a menu-shaped popover at the point the page names; a newer gesture or a dismissal
//   supersedes an older menu before its row can act;
// - the page's last `stats` report, for the agent's `state`.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { letGo } from '../client/let-go';

/** A socket URL with a fresh ticket for `environmentId`'s server ('' when it is not connected). */
export async function mintTerminalSocket(client: T3Client, native: Native, environmentId: string): Promise<string> {
  try { return str((await client.call(native, { op: 'terminalSocket', environmentId })).url); }
  catch (error) { if (letGo(error)) throw error; return ''; }
}

/** The clipboard write of a menu's Copy (`copyText`); false when it failed. */
export async function copyTerminalText(client: T3Client, native: Native, text: string): Promise<boolean> {
  try { return (await client.call(native, { op: 'copyText', text })).ok !== false; }
  catch (error) { if (letGo(error)) throw error; return false; }
}

export type TerminalFramePost = { key: string; text: string };
export type TerminalMenuRow = { id: string; label: string; disabled: boolean };
export type TerminalFrameMenu = { serial: number; x: number; y: number; rows: TerminalMenuRow[] };
/** What a terminal frame shows besides its config: the page's URL, its session (JSON), posts, menu and report. */
export type TerminalFrameView = { id: string; src: string; session: string; posts: TerminalFramePost[]; menu: TerminalFrameMenu; stats: string };

export const TERMINAL_PAGE = 'assets/terminal/index.html';
const MAX_FRAMES = 64;

type Selection = { text: string; clipboard: string; lineStart: number; lineEnd: number };
type FrameState = { serial: number; posts: TerminalFramePost[]; menu: TerminalFrameMenu; selection: Selection | null; stats: string; booted: boolean; identity: Obj;
  transcript: string };
const registries = new WeakMap<object, Map<string, FrameState>>();

function frames(owner: object): Map<string, FrameState> {
  let map = registries.get(owner);
  if (!map) { map = new Map(); registries.set(owner, map); }
  return map;
}
const emptyMenu = (): TerminalFrameMenu => ({ serial: 0, x: 0, y: 0, rows: [] });
/** A frame's state, the most recently used last: past MAX_FRAMES the least recently used goes (every view uses the frames it shows). */
function frame(owner: object, id: string): FrameState {
  const map = frames(owner);
  const state = map.get(id) ?? { serial: 0, posts: [], menu: emptyMenu(), selection: null, stats: '', booted: false, identity: {}, transcript: '' };
  map.delete(id);
  map.set(id, state);
  if (map.size > MAX_FRAMES) map.delete(map.keys().next().value!);
  return state;
}

/** A frame's id: one per session it shows, so a page replaced for another session never takes the old one's posts. */
export function terminalFrameId(surface: string, environmentId: string, threadId: string, terminalId: string): string {
  return `terminal-${surface}-${environmentId}-${threadId}-${terminalId}`.replace(/[^A-Za-z0-9_-]/g, '_');
}

/**
 * The view of a frame for `session` (the page's session config). A sign-in transcript (`output` up to `offset`) goes
 * to a booted page whenever it moved on (the page writes what is new, terminalTranscriptUpdate).
 */
export function terminalFrameView(owner: object, id: string, session: Obj, transcript?: { flow: string; output: string; offset: number }): TerminalFrameView {
  const state = frame(owner, id);
  const key = transcript ? `${transcript.flow}|${transcript.offset}|${transcript.output.length}` : '';
  if (transcript && state.booted && state.transcript !== key) {
    state.transcript = key;
    postToTerminalFrame(owner, id, { type: 'transcript', ...transcript });
  }
  return { id, src: TERMINAL_PAGE, session: JSON.stringify(session), posts: state.booted ? state.posts.slice(0, 1) : [], menu: state.menu, stats: state.stats };
}

/** Queues `message` for the page; it goes once the page has booted, after the posts before it. */
export function postToTerminalFrame(owner: object, id: string, message: Obj): void {
  const state = frame(owner, id);
  state.serial += 1;
  state.posts.push({ key: `${state.serial}`, text: JSON.stringify(message) });
}

/** The frame took the post `key`. */
export function terminalFramePosted(owner: object, id: string, key: string): void {
  const state = frames(owner).get(id);
  if (state) state.posts = state.posts.filter(post => post.key !== key);
}

/** Every frame showing `terminalId` of a thread: a system message for it (`[terminal] <message>`). */
export function terminalFrameSystemMessage(owner: object, match: (id: string) => boolean, message: string): boolean {
  let written = false;
  // A snapshot of the ids: a post moves its frame to the end of the map (frame, by use).
  for (const id of [...frames(owner).keys()]) if (match(id)) { postToTerminalFrame(owner, id, { type: 'system', message }); written = true; }
  return written;
}

const selectionOf = (message: Obj): Selection | null => {
  const text = str(message.text).replace(/\r\n/g, '\n').replace(/^\n+|\n+$/g, '');
  const position = obj(message.position), start = obj(position.start), end = obj(position.end);
  if (!text || typeof start.y !== 'number' || typeof end.y !== 'number') return null;
  const lineStart = start.y + 1;
  return { text, clipboard: str(message.text), lineStart, lineEnd: Math.max(lineStart, end.y + 1) };
};

/**
 * The page's own messages, for any frame: its boot, its socket ticket, the selection popup and context menu, and
 * its report. True when consumed; the rest (focus, link, command, exited, …) are the surface's owner's.
 */
export async function terminalFrameMessage(owner: object, id: string, message: Obj,
  options: { mint: () => Promise<string>; canAddToChat?: boolean }): Promise<boolean> {
  const state = frame(owner, id);
  if (str(message.threadId)) state.identity = { environmentId: str(message.environmentId), threadId: str(message.threadId), terminalId: str(message.terminalId), surface: str(message.surface) };
  switch (message.type) {
    case 'boot':
      Object.assign(state, { booted: true, posts: [], menu: emptyMenu(), selection: null, transcript: '' });
      return true;
    case 'ticket': {
      // The page waits for an answer: a mint let go (the app moved on) still answers, with no URL, so it asks again.
      let url = '';
      try { url = await options.mint(); }
      finally { postToTerminalFrame(owner, id, { type: 'socket', url, serial: Number(message.serial) || 0 }); }
      return true;
    }
    case 'posted':
      terminalFramePosted(owner, id, str(message.key));
      return true;
    case 'stats':
      state.stats = JSON.stringify(message);
      return true;
    case 'selection-ready': case 'contextmenu': {
      const context = message.type === 'contextmenu', selection = selectionOf(message);
      if (!context && !selection) { state.menu = { ...state.menu, rows: [] }; state.selection = null; return true; }
      const rows: TerminalMenuRow[] = [
        ...(options.canAddToChat ? [{ id: 'add-to-chat', label: 'Add to chat', disabled: context && !selection }] : []),
        { id: 'copy', label: 'Copy', disabled: context && !selection },
        ...(context ? [{ id: 'paste', label: 'Paste', disabled: message.readOnly === true }, { id: 'select-all', label: 'Select all', disabled: false },
          { id: 'scroll-to-bottom', label: 'Jump to latest', disabled: false }] : []),
      ];
      state.selection = selection;
      state.menu = { serial: state.menu.serial + 1, x: Math.max(0, Number(message.x) || 0), y: Math.max(0, Number(message.y) || 0), rows };
      return true;
    }
    case 'selection-dismiss':
      // A newer gesture: the open menu's rows no longer act.
      if (message.reason === 'interaction') { state.selection = null; state.menu = { ...state.menu, serial: state.menu.serial + 1, rows: [] }; }
      return true;
    default:
      return false;
  }
}

/**
 * A menu row was chosen (`<serial>|<row>`): copy, paste, select all and jump act on the page; Add to chat returns the
 * selection for the owner to insert. A row of a superseded menu does nothing.
 */
export async function terminalFrameMenuPick(owner: object, id: string, value: string, copy: (text: string) => Promise<boolean>): Promise<(Selection & { identity: Obj }) | null> {
  const state = frames(owner).get(id);
  const at = value.indexOf('|'), serial = Number(value.slice(0, at)), row = value.slice(at + 1);
  if (!state || serial !== state.menu.serial) return null;
  const selection = state.selection;
  state.menu = { ...state.menu, rows: [] };
  switch (row) {
    case 'add-to-chat':
      if (!selection) return null;
      postToTerminalFrame(owner, id, { type: 'clearSelection' });
      postToTerminalFrame(owner, id, { type: 'focus' });
      return { ...selection, identity: state.identity };
    case 'copy': {
      if (!selection) return null;
      if (!await copy(selection.clipboard)) postToTerminalFrame(owner, id, { type: 'system', message: 'Unable to copy terminal selection' });
      postToTerminalFrame(owner, id, { type: 'focus' });
      return null;
    }
    case 'paste': postToTerminalFrame(owner, id, { type: 'pasteClipboard' }); return null;
    case 'select-all': postToTerminalFrame(owner, id, { type: 'selectAll' }); return null;
    case 'scroll-to-bottom': postToTerminalFrame(owner, id, { type: 'scrollToBottom' }); return null;
    default: return null;
  }
}
