// Keybinding dispatch for every reference command this client has a
// counterpart for. The server's resolved keybindings (keybindings.json plus
// defaults) decide each chord; `when` clauses evaluate against this window's
// context, and the last matching rule for a chord wins (resolveShortcutCommand).
// sidebar.toggle, modelPicker.toggle, composer.effort and composer.mode
// stay on their visible buttons (keyboardSettings, composer-presentation), so a
// chord has one button; the rest are dispatched from here.
import { settingsForward } from './r8-pointer-forward';
import { arr, obj, str, type Obj } from './domain';
import { modelCatalog } from './presentation';
import type { T3Client } from './client';
import { undoLive } from './sidebar-state';
import { addStripShortcuts } from './composer-controls-branch';
import { diffShown } from './r8-keys-chords'; // lane r8-keys: ⌘D
import { openFavoriteEnabled } from './remote-open'; // the open-favorite key works only where the Open picker shows
import { closeChordTarget } from './r9-input-panel'; // lane r9-input: ⌘W closes the active surface tab
import { legacyTraversal } from './legacy-sidebar-view'; // legacy-sidebar: ⌘1-9 and ⇧⌘[ / ⇧⌘] follow its visible order
import { resolveAdjacentThreadId } from './legacy-sidebar-model';
import { threadCommandRows } from './thread-keys'; // thread-commands-and-keys: queue and host keys
import { terminalRows, terminalOpen } from './terminal-drawer-view';
import { terminalFocused } from './terminal-focus'; // terminal-drawer: ⌘J

export type DispatchContext = { composerFocus: boolean; editableFocus: boolean; turnRunning: boolean; modelPickerOpen: boolean; draftThreadRoute: boolean; modalOpen: boolean; settingsOpen: boolean; diffOpen: boolean;
  terminalFocus?: boolean; terminalOpen?: boolean; paletteOpen?: boolean; paletteMode?: string; prNumber?: string; undoShown?: boolean; settingsRoute?: string; page?: string;
  /** The open picker's Settings row (settings-model-picker.ts); '' for the composer's. */
  modelTarget?: string };
type Rule = { command: string; chord: string; whenAst: unknown };

const NAMED: Record<string, string> = { ' ': 'Space', space: 'Space', escape: 'Escape', esc: 'Escape', enter: 'Enter', tab: 'Tab', arrowup: 'ArrowUp', arrowdown: 'ArrowDown', arrowleft: 'ArrowLeft', arrowright: 'ArrowRight', backspace: 'Backspace', delete: 'Delete', pageup: 'PageUp', pagedown: 'PageDown', home: 'Home', end: 'End', '+': 'Plus' };
/** A resolved shortcut as the host's aria-keyshortcuts grammar (Meta+Shift+K); '' if inexpressible. */
export function ariaChord(shortcut: Obj): string {
  const key = str(shortcut.key);
  const named = NAMED[key] || (/^f\d{1,2}$/.test(key) ? key.toUpperCase() : key.length === 1 ? key : '');
  if (!named) return '';
  return [...(shortcut.modKey || shortcut.metaKey ? ['Meta'] : []), ...(shortcut.ctrlKey ? ['Control'] : []), ...(shortcut.altKey ? ['Alt'] : []), ...(shortcut.shiftKey ? ['Shift'] : []), named].join('+');
}
function when(ast: unknown, context: Record<string, boolean>, depth = 0): boolean | null {
  const node = obj(ast);
  if (!node.type) return true;
  if (depth > 64) return null;
  if (node.type === 'identifier') return Object.prototype.hasOwnProperty.call(context, str(node.name)) ? context[str(node.name)] : null;
  const left = when(node.type === 'not' ? node.node : node.left, context, depth + 1);
  if (node.type === 'not') return left === null ? null : !left;
  const right = when(node.right, context, depth + 1);
  if (left === null || right === null) return null;
  return node.type === 'and' ? left && right : node.type === 'or' ? left || right : null;
}
/** Each chord's winning command in this context ('' when an unknown condition makes it undecidable). */
export function chordWinners(bindings: Obj[], context: DispatchContext): Map<string, string> {
  const values: Record<string, boolean> = { true: true, false: false, isDesktop: true, isWeb: false, terminalFocus: context.terminalFocus === true, terminalOpen: context.terminalOpen === true, previewFocus: false, previewOpen: false, usagePageOpen: false,
    composerFocus: context.composerFocus, editableFocus: context.editableFocus, turnRunning: context.turnRunning, modelPickerOpen: context.modelPickerOpen, draftThreadRoute: context.draftThreadRoute };
  const winners = new Map<string, string>();
  const rules: Rule[] = bindings.map(binding => ({ command: str(binding.command), chord: ariaChord(obj(binding.shortcut)), whenAst: binding.whenAst }));
  for (const rule of [...rules].reverse()) {
    if (!rule.chord || winners.has(rule.chord)) continue;
    const match = when(rule.whenAst, values);
    if (match === false) continue;
    winners.set(rule.chord, match === null ? '' : rule.command);
  }
  return winners;
}

// ── navigation.back / navigation.forward ─────────────────────────────────
// The main view's visited locations, newest last: a thread, a project's
// new-thread draft, or a page over them (Usage, Pull Requests; thread-commands-and-keys:
// the reference's history covers every route). Arriving at the entry beside the cursor moves the cursor
// (that is what Back and Forward do); anywhere else is a new visit.
type History = { entries: string[]; cursor: number };
const histories = new WeakMap<T3Client, History>();
export function visit(client: T3Client, page = ''): History {
  const history = histories.get(client) ?? { entries: [], cursor: -1 };
  histories.set(client, history);
  const here = page ? `page:${page}` : client.threadId ? `thread:${client.threadId}` : client.projectId ? `draft:${client.projectId}` : '';
  if (!here || history.entries[history.cursor] === here) return history;
  if (history.entries[history.cursor - 1] === here) history.cursor--;
  else if (history.entries[history.cursor + 1] === here) history.cursor++;
  else {
    history.entries = [...history.entries.slice(0, history.cursor + 1), here].slice(-50);
    history.cursor = history.entries.length - 1;
  }
  return history;
}
function liveEntry(client: T3Client, entry: string | undefined): boolean {
  if (!entry) return false;
  const [kind, id] = [entry.slice(0, entry.indexOf(':')), entry.slice(entry.indexOf(':') + 1)];
  if (kind === 'page') return true;
  return kind === 'thread' ? (client.shell?.threads ?? []).some(thread => thread.id === id && !thread.archivedAt) : (client.shell?.projects ?? []).some(project => project.id === id);
}

// ── editor.openFavorite ────────────────────────────────────────────────────
export const EDITOR_ORDER = ['cursor', 'trae', 'kiro', 'vscode', 'vscode-insiders', 'vscodium', 'zed', 'antigravity', 'idea', 'aqua', 'clion', 'datagrip', 'dataspell', 'goland', 'phpstorm', 'pycharm', 'rider', 'rubymine', 'rustrover', 'webstorm', 'file-manager'];
/** usePreferredEditor: the remembered editor while still available, else the first available editor in EDITORS order. */
export function favoriteEditor(config: Obj, last = ''): string {
  const available = (Array.isArray(config.availableEditors) ? config.availableEditors : []).filter((entry): entry is string => typeof entry === 'string');
  if (last && available.includes(last)) return last;
  return EDITOR_ORDER.find(editor => available.includes(editor)) ?? '';
}

export type Dispatch = { id: string; command: string; chord: string; kind: string; target: string; extra: string; label: string };
/** The hidden dispatch buttons: one per command with a native counterpart and a winning chord. */
export function keyboardDispatch(client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): Dispatch[] {
  context = { ...context, terminalFocus: terminalFocused(client), terminalOpen: terminalOpen(client) };
  const winners = chordWinners(arr(client.config.keybindings), context);
  const chords = (command: string) => [...winners].filter(([, winner]) => winner === command).map(([chord]) => chord).join(' ');
  const out: Dispatch[] = [];
  // One button per chord (lane r8-keys): the host files a button's first ⌘ chord as its menu key
  // equivalent, so chat.new's ⇧⌘O and ⌘N each need their own to work while no view holds the focus.
  const add = (command: string, kind: string, target: string, label: string, extra = '') => chords(command).split(' ').filter(Boolean).forEach((chord, index) =>
    out.push({ id: `${command}:${target}:${extra}${index ? `:${chord}` : ''}`, command, chord, kind, target, extra, label }));
  // The search overlay's three modes toggle from anywhere the palette can open,
  // including while it is open (re-pressing a mode's chord closes it).
  const overlays = () => {
    add('commandPalette.toggle', 'palette', 'command', 'Command Palette');
    add('filePicker.toggle', 'palette', 'files', 'Go to File');
    add('projectSearch.toggle', 'palette', 'content', 'Search Project Contents');
  };
  if (context.paletteOpen) { overlays(); return out; }
  if (context.modalOpen && !context.settingsOpen) return [];
  // Settings… (⌘,) opens Settings on General from anywhere in the main window.
  out.push({ id: 'settings.open', command: 'settings.open', chord: 'Meta+,', kind: 'settings', target: 'general', extra: '', label: 'Settings' });
  if (context.settingsOpen) {
    // The settings page is the router's other location; Back leaves it.
    add('navigation.back', 'back', '', 'Back');
    settingsForward(client, true, context.settingsRoute); // r8-pointer: ⌘] comes back here (r8-pointer-forward.ts)
    add('themeEditor.toggle', 'palette-run', 'theme-editor', 'Toggle Theme Editor');
    // thread-commands-and-keys: the palette provider's chords are app-wide (CommandPalette.tsx), so they work in Settings too.
    overlays(); add('theme.select', 'palette', 'command', 'Change Theme', 'theme'); add('usage.open', 'palette-run', 'usage', 'Open Usage'); appearanceRow(add, client, threads, browseProvider, modelQuery, context);
    modelPickerRows(add, client, threads, browseProvider, modelQuery, context); // a General model row's picker (ModelPickerContent's own keys)
    return out;
  }
  overlays();
  for (const row of MAIN_ROWS) row(add, client, threads, browseProvider, modelQuery, context);
  return out;
}

/** `add` files one button per winning chord of a command (keyboardDispatch). */
export type DispatchAdd = (command: string, kind: string, target: string, label: string, extra?: string) => void;
/** One main-window row: the buttons it adds, from the dispatch's own arguments. */
export type DispatchRow = (add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext) => void;
// The main window's commands after the search overlay's three, in button
// order (the host files a button's first ⌘ chord as its menu key equivalent,
// so order is kept). A feature adds a row function, here or in its own file,
// and one entry below; no row reads another's locals.
const MAIN_ROWS: DispatchRow[] = [paletteRows, appearanceRow, threadOrderRows, navigationRows, scratchRow, threadRows, panelRows, terminalRows, turnRows, modelPickerRows, threadCommandRows];
/** The palette, usage, theme editor and new-thread commands. */
function paletteRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // theme.select opens the palette on Change theme; usage.open and themeEditor.toggle run the palette's rows.
  add('theme.select', 'palette', 'command', 'Change Theme', 'theme');
  add('usage.open', 'palette-run', 'usage', 'Open Usage');
  add('themeEditor.toggle', 'palette-run', 'theme-editor', 'Toggle Theme Editor');
  add('chat.new', 'chat-new', '', 'New Thread'); // lane r8-keys: with or without the sidebar's button (routes/_chat.tsx)
  add('chat.newLocal', 'new-thread', '', 'New Local Thread');
}
/** appearance.cycle: System → Light → Dark. */
function appearanceRow(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  const appearanceMode = str(client.local.deviceSettings.appearanceMode, 'system');
  const next = appearanceMode === 'system' ? 'light' : appearanceMode === 'light' ? 'dark' : 'system';
  add('appearance.cycle', 'appearance', next, `Appearance: ${next === 'system' ? 'System' : next === 'light' ? 'Light' : 'Dark'}`);
}
/** Previous, next and the first nine threads, in sidebar order. */
function threadOrderRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // Thread order as the sidebar paints it: pinned, active, working, snoozed, settled.
  const order = ['pinned', 'active', 'working', 'snoozed', 'settled'];
  const legacy = legacyTraversal(client);
  const ordered = legacy?.ordered ?? order.flatMap(section => threads.filter(thread => str(thread.section) === section));
  if (legacy) { if (legacy.previous) add('thread.previous', 'thread', legacy.previous, 'Previous Thread'); if (legacy.next) add('thread.next', 'thread', legacy.next, 'Next Thread'); }
  else if (ordered.length && !context.modelPickerOpen) {
    // resolveAdjacentThreadId (thread-commands-and-keys): stops at the ends; no current thread starts from the last/first; an unlisted one goes nowhere.
    const ids = ordered.map(thread => str(thread.id)), selected = ordered.find(thread => thread.selected === true);
    const current = selected ? str(selected.id) : client.threadId ? undefined : null;
    const previous = current === undefined ? null : resolveAdjacentThreadId({ threadIds: ids, currentThreadId: current, direction: 'previous' });
    const following = current === undefined ? null : resolveAdjacentThreadId({ threadIds: ids, currentThreadId: current, direction: 'next' });
    if (previous) add('thread.previous', 'thread', previous, 'Previous Thread');
    if (following) add('thread.next', 'thread', following, 'Next Thread');
  }
  ordered.slice(0, 9).forEach((thread, index) => add(`thread.jump.${index + 1}`, 'thread', str(thread.id), `Thread ${index + 1}`));
}
/** Back and Forward. */
function navigationRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // Back and Forward between the visited threads and drafts.
  const history = visit(client, context.page ?? '');
  const back = history.entries[history.cursor - 1], forward = history.entries[history.cursor + 1];
  const route = (entry: string) => entry.startsWith('thread:') ? { kind: 'thread', target: entry.slice(7) } : entry.startsWith('page:') ? { kind: 'palette-run', target: entry.slice(5) } : { kind: 'new-thread-in', target: entry.slice(6) };
  if (back && liveEntry(client, back)) add('navigation.back', route(back).kind, route(back).target, 'Back');
  const settingsRoute = settingsForward(client, false); // r8-pointer: Settings left by Back is Forward's target
  if (settingsRoute) add('navigation.forward', 'palette-run', 'settings', 'Forward', settingsRoute);
  else if (forward && liveEntry(client, forward)) add('navigation.forward', route(forward).kind, route(forward).target, 'Forward');
}
/** chat.newWithoutProject, when the server has a scratch workspace. */
function scratchRow(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (str(obj(client.config).scratchWorkspaceRoot)) add('chat.newWithoutProject', 'flow', 'scratch', 'New Thread Without a Project');
}
/** The open thread: settle, pin, the PR number, Undo and Open in Editor. */
function threadRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  const thread = client.threadId ? (client.shell?.threads ?? []).find(candidate => candidate.id === client.threadId) : undefined;
  if (thread) {
    const settled = thread.settledOverride === 'settled' || (!!thread.settledAt && thread.settledOverride !== 'active');
    add('thread.settle', 'command', settled ? 'chat:unsettle' : 'chat:settle', settled ? 'Un-settle Thread' : 'Settle Thread', str(thread.id));
    // thread.pin: pin, or unpin through the sidebar's Unpin confirmation when that setting asks.
    if (obj(obj(obj(client.config).environment).capabilities).threadPinning === true) {
      const pinned = thread.pinnedAt != null;
      add('thread.pin', 'command', pinned ? 'sidebar:unpin' : 'sidebar:pin', pinned ? 'Unpin Thread' : 'Pin Thread', str(thread.id));
    }
  }
  // pullRequest.copyNumber: the open pull request detail panel copies its "#N" (PullRequestDetailPanel copyFromShortcut).
  if (context.prNumber) add('pullRequest.copyNumber', 'pr-copy', 'PR number', 'Copy PR Number', context.prNumber);
  // thread.undo: the sidebar's live Undo notice (sidebar-commands.ts undoLatest).
  if (context.undoShown ?? undoLive(client)) add('thread.undo', 'command', 'sidebar:undo', 'Undo');
  const project = (client.shell?.projects ?? []).find(candidate => candidate.id === client.projectId);
  if (project && favoriteEditor(client.config) && openFavoriteEnabled(client, str(project.title))) add('editor.openFavorite', 'flow', 'open-favorite', 'Open in Editor', str(thread?.worktreePath, str(project.workspaceRoot)));
}
/** The right panel: close, toggle, Diff, and Copy Thread ID. */
function panelRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // rightPanel.close: an open panel closes before ⌘W reaches the window, a draft's panel too (lane r8-keys).
  const closing = closeChordTarget(client, context.diffOpen, context.prNumber ?? ''); // lane r9-input: the active surface tab first
  if (closing) add('rightPanel.close', closing[0], closing[1], 'Close Right Panel');
  add('rightPanel.toggle', 'diff', '', 'Toggle Right Panel'); // thread-commands-and-keys: a draft's panel toggles too (ChatView toggleRightPanel)
  if (!context.draftThreadRoute) {
    add('diff.toggle', diffShown(client) ? 'diff' : 'diff-open', '', 'Toggle Diff');
    add('thread.copyReference', 'copy-thread', '', 'Copy Thread ID');
  }
}
/** Stop, and the composer's context strip. */
function turnRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.turnRunning) add('thread.stop', 'stop', '', 'Stop');
  addStripShortcuts(client, add); // ⇧⌘X, ⇧⌘G, ⇧⌘L: the composer's context strip (composer-controls-branch.ts).
}
/** The open model picker: provider rail and the first nine models. */
function modelPickerRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.modelPickerOpen) {
    const catalog = modelCatalog(client, browseProvider, modelQuery, context.modelTarget ?? '');
    const rail = ['favorites', ...catalog.providers.map(provider => provider.id)];
    const at = Math.max(0, catalog.railIndex);
    add('modelPicker.previousProvider', 'provider', rail[at <= 0 ? rail.length - 1 : at - 1], 'Previous Provider');
    add('modelPicker.nextProvider', 'provider', rail[at >= rail.length - 1 ? 0 : at + 1], 'Next Provider');
    // modelJumpCommandByKey: a disabled model (getModelDisabledReason) takes no jump number.
    catalog.models.filter(row => row.kind === 'model' && !row.reason).slice(0, 9).forEach((row, index) => add(`modelPicker.jump.${index + 1}`, 'model', row.id, `Model ${index + 1}`, row.providerId));
  }
}

/** The `keyboardDispatch` resource's positional arguments (app.contract). */
export function keyboardDispatchSource(client: T3Client, args: unknown[]): Dispatch[] {
  return keyboardDispatch(client, Array.isArray(args[0]) ? args[0] as Obj[] : [], String(args[1] || ''), String(args[2] || ''), { composerFocus: args[3] === true, editableFocus: args[3] === true || args[7] === true,
    turnRunning: args[4] === true, modelPickerOpen: args[5] === true, draftThreadRoute: args[6] === true, modalOpen: args[7] === true, settingsOpen: args[8] === true, diffOpen: args[9] === true,
    paletteOpen: args[11] === true, paletteMode: String(args[12] || ''), prNumber: String(args[13] || ''), undoShown: args.length > 14 ? args[14] === true : undefined, settingsRoute: String(args[15] || ''), page: String(args[16] || ''), modelTarget: String(args[17] || '') });
}
