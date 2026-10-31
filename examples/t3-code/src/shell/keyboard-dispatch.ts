import { focusedDiffFind } from '../diff/diff-find';
// Keybinding dispatch for every reference command this client has a
// counterpart for. The server's resolved keybindings (keybindings.json plus
// defaults) decide each chord; `when` clauses evaluate against this window's
// context, and the last matching rule for a chord wins (resolveShortcutCommand).
// sidebar.toggle, modelPicker.toggle, composer.effort and composer.mode
// stay on their visible buttons (keyboardSettings, composer-presentation), so a
// chord has one button; the rest are dispatched from here, and sidebar.toggle
// too while Settings or Usage leaves its button inert (sidebarRow).
import { settingsForward } from './navigation-forward';
import { arr, obj, str, type Obj } from '../client/domain';
import { modelCatalog } from '../client/presentation';
import { adjacentPickerProvider } from '../composer/model-catalog';
import type { T3Client } from '../client/client';
import { undoLive } from '../sidebar/sidebar-state';
import { addStripShortcuts } from '../composer/composer-controls-branch';
import { diffShown } from './button-chords'; // ⌘D
import { openFavoriteEnabled, remoteOpenFor } from '../connections/remote-open'; // the open-favorite key works only where the Open picker shows
import { closeChordTarget } from '../panel/close-surface'; // ⌘W closes the active surface tab
import { legacyTraversal } from '../sidebar/legacy-sidebar-view'; // the legacy sidebar: ⌘1-9 and ⇧⌘[ / ⇧⌘] follow its visible order
import { resolveAdjacentThreadId } from '../sidebar/legacy-sidebar-model';
import { threadCommandRows } from './thread-keys'; // queue and host keys
import { terminalRows, terminalOpen } from '../terminal/terminal-drawer-view';
import { terminalFocused } from '../terminal/terminal-focus'; // ⌘J
import { previewToggleRow } from '../browser/browser-navigation'; // ⇧⌘J

export type DispatchContext = { composerFocus: boolean; editableFocus: boolean; turnRunning: boolean; modelPickerOpen: boolean; draftThreadRoute: boolean; modalOpen: boolean; settingsOpen: boolean; diffOpen: boolean;
  terminalFocus?: boolean; terminalOpen?: boolean; paletteOpen?: boolean; paletteMode?: string; prNumber?: string; undoShown?: boolean; settingsRoute?: string; page?: string;
  /** A new undo notice owns its chord even in a field until another key or an edit (showThreadUndoNotice's claim), and
   *  the sidebar that shows the notice is open. */
  undoClaimed?: boolean;
  /** The open picker's Settings row (settings-model-picker.ts); '' for the composer's. */
  modelTarget?: string;
  /** The preview's chords, which the module answers while the preview has the focus. */
  previewFocus?: boolean;
  /** A terminal close confirm is open (the window's confirmOp is a terminal close). */
  terminalClosePending?: boolean; findPanelKind?: string; prCodeOpen?: boolean;
  /** The right panel shows a file (not an attachment): its Open in picker's path, '' while that picker cannot open it. */
  fileSurface?: boolean; fileOpenPath?: string };
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
  const values: Record<string, boolean> = { true: true, false: false, isDesktop: true, isWeb: false, terminalFocus: context.terminalFocus === true, terminalOpen: context.terminalOpen === true, previewFocus: context.previewFocus === true, previewOpen: false, usagePageOpen: context.page === 'usage',
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
// new-thread draft, or a page over them (Usage, Pull Requests;
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
  if (context.terminalClosePending) return refuseTerminalClose(client, keyboardDispatch(client, threads, browseProvider, modelQuery, { ...context, terminalClosePending: false }));
  context = { ...context, terminalFocus: terminalFocused(client), terminalOpen: terminalOpen(client) };
  const winners = chordWinners(arr(client.config.keybindings), context);
  const chords = (command: string) => [...winners].filter(([, winner]) => winner === command).map(([chord]) => chord).join(' ');
  const out: Dispatch[] = [];
  // One button per chord: the host files a button's first ⌘ chord as its menu key
  // equivalent, so chat.new's ⇧⌘O and ⌘N each need their own to work while no view holds the focus.
  const add: DispatchAdd = Object.assign((command: string, kind: string, target: string, label: string, extra = '') => chords(command).split(' ').filter(Boolean).forEach((chord, index) =>
    out.push({ id: `${command}:${target}:${extra}${index ? `:${chord}` : ''}`, command, chord, kind, target, extra, label })),
  { chord: (command: string, chord: string, kind: string, target: string, label: string, extra = '') => out.push({ id: `${command}:${target}:${chord}`, command, chord, kind, target, extra, label }) });
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
  add('view.reopenClosed', 'panel-reopen', '', 'Reopen Closed Tab');
  if (context.settingsOpen) {
    // The settings page is the router's other location; Back leaves it.
    add('navigation.back', 'back', '', 'Back');
    settingsForward(client, true, context.settingsRoute); // ⌘] comes back here (navigation-forward.ts)
    add('themeEditor.toggle', 'palette-run', 'theme-editor', 'Toggle Theme Editor');
    // The palette provider's chords are app-wide (CommandPalette.tsx), so they work in Settings too.
    overlays(); add('theme.select', 'palette', 'command', 'Change Theme', 'theme'); add('usage.open', 'palette-run', 'usage', 'Open Usage'); appearanceRow(add, client, threads, browseProvider, modelQuery, context);
    modelPickerRows(add, client, threads, browseProvider, modelQuery, context); // a General model row's picker (ModelPickerContent's own keys)
    sidebarRow(add, client, threads, browseProvider, modelQuery, context);
    workspaceRow(add); // Settings' own areas: its sections and its page
    return out;
  }
  overlays();
  const focusedFind = !context.composerFocus && context.diffOpen ? focusedDiffFind(client) : null;
  const find = focusedFind && (focusedFind.slot === 'panel' ? context.findPanelKind === undefined || context.findPanelKind === 'diff' : context.prCodeOpen !== false && (context.findPanelKind === undefined || context.findPanelKind === 'pull-request' || !!context.prNumber)) ? focusedFind : null;
  if (find) add('chat.find', 'command', find.slot === 'pr' ? 'chatlocal:pr-code-find-open' : 'chatlocal:diff-find-open', 'Find in Diff', find.reference);
  else if (client.threadId) add('chat.find', 'command', 'chatlocal:find-open', 'Find in Thread');
  for (const row of MAIN_ROWS) row(add, client, threads, browseProvider, modelQuery, context);
  // A claimed undo chord is the notice's alone, as the reference's capture listener stops it before anything else.
  const undo = new Set(out.filter(row => row.command === 'thread.undo').map(row => row.chord));
  return out.filter(row => row.command === 'thread.undo' || !undo.has(row.chord));
}

/**
 * thread.undo's chords (AppSidebarLayout ThreadUndoShortcut): with the focus at a field, only while the notice claims
 * them, resolved as if no field had the focus, so a field's own undo comes back once the claim is released.
 */
function undoChords(client: T3Client, context: DispatchContext): string[] {
  const claimed = context.editableFocus && context.undoClaimed === true;
  const winners = chordWinners(arr(client.config.keybindings), claimed ? { ...context, editableFocus: false } : context);
  return [...winners].filter(([, command]) => command === 'thread.undo').map(([chord]) => chord);
}
/**
 * A chord's character key, lowercased, which a held chord's repeats carry as `KeyboardEvent.key` with ⌘ held, whatever
 * the layout puts under it; '' for a named key.
 */
export function chordKey(chord: string): string {
  const key = chord.slice(chord.lastIndexOf('+', chord.length - 2) + 1);
  return key.length === 1 ? key.toLowerCase() : '';
}

/** The confirm ops of a terminal close (the drawer's and a panel terminal's, and a Terminal surface's tab). */
export const TERMINAL_CLOSE_CONFIRMS = ['terminallocal:close', 'terminalpanellocal:close', 'shelllocal:surface-close-confirmed'];
/** ChatView's window key handler while a terminal close confirm is pending (`isTerminalCloseConfirmPending() &&
 *  preventTerminalCloseShortcut`): a chord `terminal.close` wins with a terminal focused runs nothing, whatever it would
 *  run now (`rightPanel.close`, or the same close asked again). It stays a button ("refuse"), so the chord never falls
 *  through to the window's Close. */
function refuseTerminalClose(client: T3Client, rows: Dispatch[]): Dispatch[] {
  const winners = chordWinners(arr(client.config.keybindings), { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false,
    modalOpen: false, settingsOpen: false, diffOpen: false, terminalFocus: true, terminalOpen: true });
  const closing = [...winners].filter(([, command]) => command === 'terminal.close').map(([chord]) => chord);
  return [...rows.filter(row => !closing.includes(row.chord)),
    ...closing.map(chord => ({ id: `terminal.close:refused:${chord}`, command: 'terminal.close', chord, kind: 'refuse', target: '', extra: '', label: 'Close Terminal' }))];
}

/** `add` files one button per winning chord of a command (keyboardDispatch); `add.chord` files one for a chord it names. */
export type DispatchAdd = ((command: string, kind: string, target: string, label: string, extra?: string) => void) &
  { chord(command: string, chord: string, kind: string, target: string, label: string, extra?: string): void };
/** One main-window row: the buttons it adds, from the dispatch's own arguments. */
export type DispatchRow = (add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext) => void;
// The main window's commands after the search overlay's three, in button
// order (the host files a button's first ⌘ chord as its menu key equivalent,
// so order is kept). A feature adds a row function, here or in its own file,
// and one entry below; no row reads another's locals.
const MAIN_ROWS: DispatchRow[] = [workspaceRow, paletteRows, appearanceRow, threadOrderRows, navigationRows, scratchRow, threadRows, panelRows, (add, client) => previewToggleRow(add, client), terminalRows, turnRows, effortRows, modelPickerRows, threadCommandRows, sidebarRow];
/** workspace.focusNext / focusPrevious: F6 and ⇧F6 (⌘F6 and ⇧⌘F6 too) move between the shown workspace areas. */
function workspaceRow(add: DispatchAdd): void {
  add('workspace.focusNext', 'workspace-focus', 'next', 'Focus Next Workspace Area');
  add('workspace.focusPrevious', 'workspace-focus', 'previous', 'Focus Previous Workspace Area');
}
/** The palette, usage, theme editor and new-thread commands. */
function paletteRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // theme.select opens the palette on Change theme; usage.open and themeEditor.toggle run the palette's rows.
  add('theme.select', 'palette', 'command', 'Change Theme', 'theme');
  add('usage.open', 'palette-run', 'usage', 'Open Usage');
  add('themeEditor.toggle', 'palette-run', 'theme-editor', 'Toggle Theme Editor');
  add('chat.new', 'chat-new', '', 'New Thread'); // with or without the sidebar's button (routes/_chat.tsx)
  add('chat.newLocal', 'new-thread', '', 'New Local Thread');
}
/**
 * sidebar.toggle while Settings or Usage covers the window: SidebarControl's capture listener has no dialog check, so
 * the sidebar behind the modal toggles. The sidebar's own button is inert under the modal, so its chord is here.
 */
function sidebarRow(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.settingsOpen || context.page === 'usage') add('sidebar.toggle', 'command', 'sidebar', 'Toggle Sidebar');
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
    // resolveAdjacentThreadId: stops at the ends; no current thread starts from the last/first; an unlisted one goes nowhere.
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
  const settingsRoute = settingsForward(client, false); // Settings left by Back is Forward's target
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
    // ChatView activeThreadSettled: only an explicit settle makes ⇧⌘S un-settle; an automatic one is settled explicitly
    // (a thread whose settledAt outlived its un-settle took Un-settle, a no-op).
    const settled = thread.settledOverride === 'settled';
    add('thread.settle', 'command', settled ? 'chat:unsettle' : 'chat:settle-advance', settled ? 'Un-settle Thread' : 'Settle Thread', str(thread.id));
    // thread.pin: pin, or unpin through the sidebar's Unpin confirmation when that setting asks.
    if (obj(obj(obj(client.config).environment).capabilities).threadPinning === true) {
      const pinned = thread.pinnedAt != null;
      add('thread.pin', 'command', pinned ? 'sidebar:unpin' : 'sidebar:pin', pinned ? 'Unpin Thread' : 'Pin Thread', str(thread.id));
    }
  }
  // pullRequest.copyNumber: the open pull request detail panel copies its "#N" (PullRequestDetailPanel copyFromShortcut).
  if (context.prNumber) add('pullRequest.copyNumber', 'pr-copy', 'PR number', 'Copy PR Number', context.prNumber);
  // thread.undo: the sidebar's live Undo notice (sidebar-commands.ts undoLatest), never under the model picker.
  if ((context.undoShown ?? undoLive(client)) && !context.modelPickerOpen) for (const chord of undoChords(client, context)) add.chord('thread.undo', chord, 'command', 'sidebar:undo', 'Undo', chordKey(chord));
  // editor.openFavorite: a shown file's picker takes the chord and opens the file (FilePreviewPanel's OpenInPicker,
  // `openInShortcutEnabled={visible}`), and the thread details picker stands down for it; without one, the workspace.
  // With the file shown and its picker unable to open it, only the window's fallback runs, on this machine alone
  // (useOpenFavoriteEditorShortcut: `remote.mode === "local-exec"`).
  const project = (client.shell?.projects ?? []).find(candidate => candidate.id === client.projectId);
  if (context.fileSurface && context.fileOpenPath) add('editor.openFavorite', 'command', 'chatlocal:surface-files-open-editor', 'Open in Editor', context.fileOpenPath);
  else if (project && favoriteEditor(client.config) && openFavoriteEnabled(client, str(project.title)) && (!context.fileSurface || remoteOpenFor(client).state.mode === 'local-exec')) add('editor.openFavorite', 'flow', 'open-favorite', 'Open in Editor', str(thread?.worktreePath, str(project.workspaceRoot)));
}
/** The right panel: close, toggle, thread details, Diff, and Copy Link or Thread. */
function panelRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  // rightPanel.close: an open panel closes before ⌘W reaches the window, a draft's panel too.
  const closing = closeChordTarget(client, context.diffOpen, context.prNumber ?? ''); // the active surface tab first
  if (closing) add('rightPanel.close', closing[0], closing[1], 'Close Right Panel');
  add('rightPanel.new', 'panel-new', '', 'New Right Panel Tab');
  add('rightPanel.toggle', 'diff', '', 'Toggle Right Panel'); // a draft's panel toggles too (ChatView toggleRightPanel)
  // threadPanel.toggle: ChatView's toggleThreadPanel, for the open thread or draft (no ChatView under a utility page).
  if (!context.page && (client.threadId || client.projectId)) add('threadPanel.toggle', 'details', '', 'Toggle Thread Details');
  if (!context.draftThreadRoute) {
    add('diff.toggle', diffShown(client) ? 'diff' : 'diff-open', '', 'Toggle Diff');
    add('thread.copyReference', 'copy-thread', '', 'Copy Link or Thread');
  }
}
/** Stop, and the composer's context strip. */
function turnRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.turnRunning) add('thread.stop', 'stop', '', 'Stop');
  addStripShortcuts(client, add); // ⇧⌘X, ⇧⌘G, ⇧⌘L: the composer's context strip (composer-controls-branch.ts).
}
/** composer.increaseEffort / composer.decreaseEffort (unbound by default): the chat view's composer steps its effort (composer-effort-step.ts). */
function effortRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.page) return;
  add('composer.increaseEffort', 'command', 'effort-step', 'Increase Effort', '1');
  add('composer.decreaseEffort', 'command', 'effort-step', 'Decrease Effort', '-1');
}
/** The open model picker: provider rail and the first nine models. */
function modelPickerRows(add: DispatchAdd, client: T3Client, threads: Obj[], browseProvider: string, modelQuery: string, context: DispatchContext): void {
  if (context.modelPickerOpen) {
    const catalog = modelCatalog(client, browseProvider, modelQuery, context.modelTarget ?? '');
    // adjacentModelPickerProvider: a rail button that cannot be chosen is skipped.
    add('modelPicker.previousProvider', 'provider', adjacentPickerProvider(catalog, -1), 'Previous Provider');
    add('modelPicker.nextProvider', 'provider', adjacentPickerProvider(catalog, 1), 'Next Provider');
    // modelJumpCommandByKey: a disabled model (getModelDisabledReason) takes no jump number.
    catalog.models.filter(row => row.kind === 'model' && !row.reason).slice(0, 9).forEach((row, index) => add(`modelPicker.jump.${index + 1}`, 'model', row.id, `Model ${index + 1}`, row.providerId));
  }
}

/** The `keyboardDispatch` resource's positional arguments (app.contract). */
export function keyboardDispatchSource(client: T3Client, args: unknown[]): Dispatch[] {
  return keyboardDispatch(client, Array.isArray(args[0]) ? args[0] as Obj[] : [], String(args[1] || ''), String(args[2] || ''), { composerFocus: args[3] === true, editableFocus: args[3] === true || args[7] === true,
    turnRunning: args[4] === true, modelPickerOpen: args[5] === true, draftThreadRoute: args[6] === true, modalOpen: args[7] === true, settingsOpen: args[8] === true, diffOpen: args[9] === true,
    paletteOpen: args[11] === true, paletteMode: String(args[12] || ''), prNumber: String(args[13] || ''), undoShown: args.length > 14 ? args[14] === true : undefined, settingsRoute: String(args[15] || ''), page: String(args[16] || ''), modelTarget: String(args[17] || ''),
    terminalClosePending: TERMINAL_CLOSE_CONFIRMS.includes(String(args[18] || '')), findPanelKind: args.length > 19 ? String(args[19] || '') : undefined, prCodeOpen: args.length > 20 ? args[20] === true : undefined, undoClaimed: args[21] === true,
    fileSurface: args[22] === true, fileOpenPath: String(args[23] || '') });
}
