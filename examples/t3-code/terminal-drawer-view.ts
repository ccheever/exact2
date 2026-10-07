import { draftThreadId, ensureDraftThreadId } from './r7-handoff-thread';
// The thread terminal drawer's client side (task terminal-drawer): T3 Code 1e2ecbd975's
// PersistentThreadTerminalDrawer and the ChatView terminal actions (MIT reference, see LICENSE-T3:
// apps/web/src/components/ChatView.tsx toggleTerminalVisibility, createNewTerminal, closeTerminal, the
// mounted-thread effect and the known-session reconcile; useThreadActions.ts thread delete cleanup;
// state/terminalSessions.ts useKnownTerminalSessions over `subscribeTerminalMetadata`).
// Changes from the reference, for exact2:
// - The UI state is the ported store (terminal-ui-state.ts), one per client, saved in the preference file.
// - The drawer's resource (`terminalDrawer`) answers what the Contract drawer (terminal.contract) shows;
//   the attach stream, its output buffer, input and resize are native (T3TerminalSessions.swift), so
//   output never passes this client's event inbox. This side opens and closes sessions, follows the
//   metadata stream (labels, the known-session reconcile) and tells the native side which mounted
//   threads' terminals keep their streams without a view (`terminalRetain`).
// - Draft terminals reserve the same thread identity the first message launches; split groups and
//   right-panel surfaces share that identity and session allocation.
// - Command failures are kept as the view's `failure` (the agent's `state` shows it), as the reference's
//   atom commands only report them to the console; the attach stream's error shows in the terminal as
//   `[terminal] <message>`.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import type { DispatchAdd } from './keyboard-dispatch';
import { arr, obj, str, type Obj } from './domain';
import { activeRun, ClientError, type Files, type Native } from './protocol';
import { terminalLayout, terminalSplitLabel, terminalTabs, type TerminalPaneView, type TerminalTabView } from './terminal-layout';
import { focusedTerminal, recordTerminalFocus, clearTerminalFocus, terminalFocused } from './terminal-focus';
import { chordWinners } from './keyboard-dispatch';
import { terminalPanelIds, terminalPanelRetained } from './terminal-panel';
import { runProjectTerminalScript } from './r6-polish-scripts';
import { insertContext } from './composer-editor';
import { composerNow } from './composer-controls';
import { terminalLinkAction } from './terminal-integrations';
import { subscriptionSerial } from './shell-vcs';
import { setCloseThreadTerminals } from './worktree-cleanup';
import { commandShortcut } from './shell';
import { terminalCloseConfirmMessage } from './terminal-close';
import { surfaceStore } from './r4-surfaces-panel';
import {
  clampDrawerHeight,
  projectScriptCwd,
  projectScriptRuntimeEnv,
  reconcileMountedTerminalThreadIds,
  serverTerminalIdsStrictSubsetOfClient,
  terminalIdListsEqual,
} from './terminal-drawer';
import { getTerminalLabel, nextTerminalId, resolveTerminalSessionLabel } from './terminal-labels';
import { applyTerminalMetadataStreamEvent, type TerminalMetadataStreamEvent, type TerminalSummary } from './terminal-session';
import { selectKnownTerminalSessions } from './terminal-sessions';
import { parseScopedThreadKey, scopedThreadKey, selectThreadTerminalUiState, terminalUiStore, type ScopedThreadRef } from './terminal-ui-state';
import { letGo } from './let-go';

export const TERMINAL_METADATA_KEY = 'terminal-metadata';
const METADATA_RETRY_MS = 3000;

export type TerminalDrawerView = {
  available: boolean; open: boolean; threadKey: string; environmentId: string; threadId: string;
  terminalId: string; label: string; cwd: string; worktree: string; env: string; height: number;
  commandPrefix: string; surface: string; keybindings: string; panes: TerminalPaneView[]; tabs: TerminalTabView[]; direction: string; showTabs: boolean; splitDisabled: boolean; splitLabel: string; splitVerticalLabel: string;
  focusRequest: number; closeTitle: string; closeBody: string; closeLabel: string; newLabel: string;
  toggleLabel: string; sessionKey: string; mounted: number; metadata: string; failure: string;
};

type MetadataState = { generation: number; id: string; floor: number; maxSeen: number; metadata: TerminalSummary[] | null;
  error: string; refused: boolean; attemptAt: number };
type DrawerState = { mounted: string[]; retained: string; focusRequest: number; viewportHeight: number; failure: string; metadata: MetadataState };
const states = new WeakMap<T3Client, DrawerState>();
function drawerState(client: T3Client): DrawerState {
  let state = states.get(client);
  if (!state) {
    state = { mounted: [], retained: '', focusRequest: 0, viewportHeight: 0, failure: '',
      metadata: { generation: -1, id: '', floor: 0, maxSeen: 0, metadata: null, error: '', refused: false, attemptAt: -Infinity } };
    states.set(client, state);
  }
  return state;
}

/** The active server thread's scoped ref, or null (no connection, or a draft). */
export function activeRef(client: T3Client): ScopedThreadRef | null {
  if (!client.environmentId) return null;
  const threadId = client.threadId || (client.local.composerControls ? draftThreadId(client) : '');
  return client.environmentId && threadId ? { environmentId: client.environmentId, threadId } : null;
}
function threadOf(client: T3Client, threadId: string): Obj | undefined { return client.shell.threads.find(thread => thread.id === threadId); }
function projectOf(client: T3Client, thread: Obj | undefined): Obj | undefined {
  const projectId = str(thread?.projectId) || client.projectId;
  return client.shell.projects.find(project => project.id === projectId);
}
/** The launch location: the thread's worktree, else the project root, and the project script env. */
export function launchFor(client: T3Client, threadId: string): { cwd: string; worktreePath: string | null; env: Record<string, string> } | null {
  const thread = threadOf(client, threadId), project = projectOf(client, thread);
  const root = str(project?.workspaceRoot);
  if (!project || !root) return null;
  const worktreePath = str(thread?.worktreePath) || (!client.threadId ? str(client.local.composerControls?.contexts?.[client.draftKey]?.worktreePath) : '') || null;
  return { cwd: projectScriptCwd({ project: { cwd: root }, worktreePath }), worktreePath, env: projectScriptRuntimeEnv({ project: { cwd: root }, worktreePath }) };
}

/** PanelLayoutControls `terminalAvailable` (an active project), for a server thread. */
export function terminalAvailable(client: T3Client): boolean {
  return !!client.environmentId && !!launchFor(client, activeRef(client)?.threadId ?? '');
}
/** Reserve the draft's eventual server identity before opening any terminal. */
export async function ensureTerminalRef(client: T3Client, native: Native): Promise<ScopedThreadRef | null> {
  if (!terminalAvailable(client)) return null;
  if (!client.threadId && !activeRef(client)) await ensureDraftThreadId(client, native);
  return activeRef(client);
}
/** Worktree setup reveals its existing session; it never opens a new shell. */
export function revealTerminal(client: T3Client, ref: ScopedThreadRef, terminalId: string): void {
  terminalUiStore(client).getState().ensureTerminal(ref, terminalId, { open: true, active: true });
  drawerState(client).focusRequest++;
}

/** ChatView terminalShortcutLabelOptions: terminal action labels resolve with terminalFocus and the drawer's open state. */
export function terminalShortcut(client: T3Client, command: string): string {
  return commandShortcut(client.config, command, { terminalFocus: true, terminalOpen: terminalOpen(client) });
}

/** The active thread's drawer is open. */
export function terminalOpen(client: T3Client): boolean {
  const ref = activeRef(client);
  return !!ref && selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref).terminalOpen;
}

const sessionKey = (ref: ScopedThreadRef, terminalId: string) => JSON.stringify([ref.environmentId, ref.threadId, terminalId]);

/** useKnownTerminalSessions for one thread: its server sessions in terminal order (empty until the stream answers). */
export function knownSessions(client: T3Client, ref: ScopedThreadRef) {
  return selectKnownTerminalSessions(drawerState(client).metadata.metadata, ref.environmentId, ref.threadId);
}
/** Every id a new terminal must avoid: the server's, the drawer's (the server list lags a fresh open). */
export function allocatableIds(client: T3Client, ref: ScopedThreadRef): string[] {
  const ui = selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref);
  return [...new Set([...knownSessions(client, ref).map(session => session.target.terminalId), ...ui.terminalIds, ...terminalPanelIds(client, ref)])];
}

/** The same terminal-focus key context as the original window key handler. */
export function terminalKeybindings(client: T3Client): string {
  return JSON.stringify([...chordWinners(arr(client.config.keybindings), { composerFocus: false, editableFocus: false,
    terminalFocus: true, terminalOpen: terminalOpen(client), turnRunning: !!activeRun(obj(client.projection)), modelPickerOpen: false,
    draftThreadRoute: !client.threadId, modalOpen: false, settingsOpen: false, diffOpen: client.diffOpen })]
    .filter(([, command]) => command && !command.startsWith('preview.')).map(([chord, command]) => ({ chord, command })));
}

/** Resolve terminal keys from the emitting pane, before a focus refresh can replace shortcut buttons. */
export function terminalCommandAction(client: T3Client, message: Obj): { op: string; id: string } | null {
  const ref = activeRef(client), terminalId = str(message.terminalId), surface = str(message.surface, 'drawer');
  if (!ref || message.environmentId !== ref.environmentId || message.threadId !== ref.threadId || !terminalId) return null;
  const store = surfaceStore(client);
  const ids = surface === 'drawer'
    ? selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref).terminalIds
    : store.panels.get(scopedThreadKey(ref))?.surfaces.find(entry => entry.id === surface)?.terminal?.terminalIds ?? [];
  if (!ids.includes(terminalId)) return null;
  const prefix = surface === 'drawer' ? 'terminallocal' : 'terminalpanellocal', id = `${scopedThreadKey(ref)}|${terminalId}`;
  switch (message.command) {
    case 'terminal.toggle': return { op: 'terminallocal:toggle', id };
    case 'terminal.new': return { op: `${prefix}:new`, id };
    case 'terminal.split': return { op: `${prefix}:split`, id };
    case 'terminal.splitVertical': return { op: `${prefix}:split-vertical`, id };
    case 'terminal.close': {
      if (message.repeat === true) return null;
      const summary = knownSessions(client, ref).find(session => session.target.terminalId === terminalId)?.state.summary;
      const [title, body] = terminalCloseConfirmMessage([resolveTerminalSessionLabel(terminalId, summary)]);
      store.terminalClose = { serial: store.terminalClose.serial + 1, title, body, target: id, op: `${prefix}:close` };
      return null;
    }
    default: return null;
  }
}

export function emptyTerminalDrawerView(): TerminalDrawerView {
  return { available: false, open: false, threadKey: '', environmentId: '', threadId: '', terminalId: '', label: '', cwd: '', worktree: '', env: '{}',
    height: 280, focusRequest: 0, closeTitle: '', closeBody: '', closeLabel: 'Close Terminal', newLabel: 'New Terminal', toggleLabel: 'Terminal drawer is unavailable',
    sessionKey: '', mounted: 0, metadata: '', failure: '', commandPrefix: 'terminallocal', surface: 'drawer', keybindings: '[]', panes: [], tabs: [], direction: 'row',
    showTabs: false, splitDisabled: false, splitLabel: 'Split Terminal Horizontally', splitVerticalLabel: 'Split Terminal Vertically' };
}

/** keyboard-dispatch MAIN_ROWS: terminal.toggle (⌘J by default) wherever the drawer can open. */
export function terminalRows(add: DispatchAdd, client: T3Client): void {
  if (terminalAvailable(client)) add('terminal.toggle', 'command', 'terminallocal:toggle', 'Toggle Terminal');
  const owner = focusedTerminal(client);
  if (!owner) return;
  const prefix = owner.surface === 'drawer' ? 'terminallocal' : 'terminalpanellocal';
  const target = `${scopedThreadKey(owner)}|${owner.terminalId}`;
  add('terminal.split', 'command', `${prefix}:split`, 'Split Terminal Horizontally', target);
  add('terminal.splitVertical', 'command', `${prefix}:split-vertical`, 'Split Terminal Vertically', target);
  add('terminal.new', 'command', `${prefix}:new`, 'New Terminal', target);
  const summary = knownSessions(client, owner).find(session => session.target.terminalId === owner.terminalId)?.state.summary;
  const [title] = terminalCloseConfirmMessage([resolveTerminalSessionLabel(owner.terminalId, summary)]);
  add('terminal.close', 'terminal-close', `${prefix}:close`, title, target);
}

// ── The metadata stream (subscribeTerminalMetadata) ─────────────────────────────────────────────

async function watchMetadata(client: T3Client, native: Native, now: number): Promise<void> {
  const state = drawerState(client).metadata;
  if (state.generation !== client.generation) Object.assign(state, { generation: client.generation, id: '', metadata: null, error: '', refused: false, attemptAt: -Infinity });
  if (client.connection !== 'connected' || !client.ready || state.id || state.refused || now - state.attemptAt < METADATA_RETRY_MS) return;
  state.attemptAt = now; state.floor = state.maxSeen;
  try {
    const reply = await client.call(native, { op: 'subscribe', key: TERMINAL_METADATA_KEY, method: 'subscribeTerminalMetadata', payload: {} });
    const id = str(reply.id), serial = subscriptionSerial(id);
    state.maxSeen = Math.max(state.maxSeen, serial);
    if (serial > state.floor && (!state.id || serial > subscriptionSerial(state.id))) state.id = id;
  } catch (error) { if (!letGo(error)) state.error = error instanceof Error ? error.message : 'Terminal metadata is unavailable.'; }
}

/** One `terminal-metadata` inbox entry (client.ts drain): the newest stream since the latest subscribe. */
export function terminalMetadataEvent(client: T3Client, entry: Obj): void {
  const state = drawerState(client).metadata;
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  state.maxSeen = Math.max(state.maxSeen, serial);
  if (serial <= state.floor || (state.id && serial < subscriptionSerial(state.id))) return;
  state.id = id;
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded) { state.id = ''; state.attemptAt = -Infinity; return; }
  if (item._transportError) {
    const failure = obj(item._transportError);
    state.error = str(failure.message, 'Terminal metadata is unavailable.');
    // An authorization failure (a session without terminal:operate) waits for the next session.
    if (str(failure.kind) === 'EnvironmentAuthorizationError') state.refused = true;
    return;
  }
  if (!['snapshot', 'upsert', 'remove'].includes(str(item.type))) return;
  state.metadata = [...applyTerminalMetadataStreamEvent(state.metadata ?? [], item as unknown as TerminalMetadataStreamEvent)];
  state.error = '';
}

// ── The drawer's resource ─────────────────────────────────────────────────────────────────────

/** The `terminalDrawer` resource: the active thread's drawer, after the reconcile and mount effects. */
export async function terminalDrawerView(client: T3Client, native: Native | null | undefined, viewportHeight: number, now: number): Promise<TerminalDrawerView> {
  const drawer = drawerState(client);
  if (viewportHeight > 0) drawer.viewportHeight = viewportHeight;
  const store = terminalUiStore(client);
  const ref = activeRef(client);
  if (native?.available) await watchMetadata(client, native, now).catch(() => undefined);
  let ui = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
  const launch = ref ? launchFor(client, ref.threadId) : null;
  // PersistentThreadTerminalDrawer: follow the server's sessions unless its list lags a fresh open.
  if (ref && ui.terminalOpen && drawer.metadata.metadata !== null) {
    const panelIds = new Set(terminalPanelIds(client, ref));
    const serverIds = knownSessions(client, ref).map(session => session.target.terminalId).filter(id => !panelIds.has(id));
    if (!terminalIdListsEqual(serverIds, ui.terminalIds) && !serverTerminalIdsStrictSubsetOfClient(serverIds, ui.terminalIds)) {
      store.getState().reconcileTerminalIds(ref, serverIds);
      ui = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
    }
  }
  // ChatView's mounted-thread effect: the active open thread plus the ten most recent hidden open ones.
  const states = store.getState().terminalUiStateByThreadKey;
  const existing = new Set(client.shell.threads.map(thread => scopedThreadKey({ environmentId: client.environmentId, threadId: str(thread.id) })));
  if (ref) existing.add(scopedThreadKey(ref));
  const openKeys = Object.entries(states).filter(([key, state]) => state.terminalOpen && existing.has(key)).map(([key]) => key);
  drawer.mounted = reconcileMountedTerminalThreadIds({ currentThreadIds: drawer.mounted, openThreadIds: openKeys, activeThreadId: ref ? scopedThreadKey(ref) : null,
    activeThreadTerminalOpen: !!launch && ui.terminalOpen });
  const retained = [...drawer.mounted.flatMap(key => {
    const mountedRef = parseScopedThreadKey(key);
    return mountedRef ? (states[key]?.terminalIds ?? []).map(id => sessionKey(mountedRef, id)) : [];
  }), ...terminalPanelRetained(client)];
  if (native?.available && JSON.stringify(retained) !== drawer.retained) {
    drawer.retained = JSON.stringify(retained);
    await client.call(native, { op: 'terminalRetain', sessions: retained }).catch(() => { drawer.retained = ''; });
  }
  const terminalId = ui.terminalIds.includes(ui.activeTerminalId) ? ui.activeTerminalId : ui.terminalIds[0] ?? '';
  const summary = ref && terminalId ? knownSessions(client, ref).find(session => session.target.terminalId === terminalId)?.state.summary : null;
  const label = terminalId ? resolveTerminalSessionLabel(terminalId, summary) : '';
  const [closeTitle, closeBody] = terminalCloseConfirmMessage([label || getTerminalLabel(terminalId)]);
  const keyNew = terminalShortcut(client, 'terminal.new'), keyClose = terminalShortcut(client, 'terminal.close'), keyToggle = commandShortcut(client.config, 'terminal.toggle');
  const layout = terminalLayout(ui.terminalIds, ui.terminalGroups, terminalId, ui.activeTerminalGroupId);
  const threadKey = ref ? scopedThreadKey(ref) : '';
  const labelFor = (id: string) => resolveTerminalSessionLabel(id, ref ? knownSessions(client, ref).find(session => session.target.terminalId === id)?.state.summary : null);
  const panes = layout.visible.map(id => ({ terminalId: id, label: labelFor(id), sessionKey: ref ? sessionKey(ref, id) : '', active: id === terminalId,
    focusRequest: ui.terminalOpen && id === terminalId ? drawer.focusRequest : 0, target: `${threadKey}|${id}` }));
  const tabs = terminalTabs(ui.terminalGroups, terminalId, layout.showHeaders, threadKey, labelFor, ui.terminalOpen ? keyClose : '');
  return {
    available: terminalAvailable(client), open: !!launch && ui.terminalOpen, threadKey: ref ? scopedThreadKey(ref) : '', environmentId: ref?.environmentId ?? '', threadId: ref?.threadId ?? '',
    terminalId, label, cwd: launch?.cwd ?? '', worktree: launch?.worktreePath ?? '', env: JSON.stringify(launch?.env ?? {}),
    height: clampDrawerHeight(ui.terminalHeight, drawer.viewportHeight || undefined), focusRequest: drawer.focusRequest,
    closeTitle, closeBody, closeLabel: ui.terminalOpen && keyClose ? `Close Terminal (${keyClose})` : 'Close Terminal',
    newLabel: ui.terminalOpen && keyNew ? `New Terminal (${keyNew})` : 'New Terminal',
    toggleLabel: launch ? `Toggle terminal drawer${keyToggle ? ` (${keyToggle})` : ''}` : 'Terminal drawer is unavailable',
    sessionKey: ref && terminalId ? sessionKey(ref, terminalId) : '', mounted: drawer.mounted.length,
    metadata: drawer.metadata.metadata === null ? (drawer.metadata.error || 'waiting') : `${drawer.metadata.metadata.length} sessions`,
    failure: drawer.failure,
    commandPrefix: 'terminallocal', surface: 'drawer', keybindings: terminalKeybindings(client), panes, tabs,
    direction: layout.direction, showTabs: layout.showTabs, splitDisabled: layout.splitDisabled,
    splitLabel: terminalSplitLabel(false, layout.splitDisabled, terminalShortcut(client, 'terminal.split')),
    splitVerticalLabel: terminalSplitLabel(true, layout.splitDisabled, terminalShortcut(client, 'terminal.splitVertical')),
  };
}

// ── Commands (client.command `terminallocal:*`) ──────────────────────────────────────────────────

function failed(client: T3Client, label: string, error: unknown): void {
  if (letGo(error)) throw error; // let-go.ts: not a failure
  drawerState(client).failure = `${label}: ${error instanceof Error ? error.message : String(error)}`;
}

export async function openTerminal(client: T3Client, native: Native, ref: ScopedThreadRef, terminalId: string): Promise<void> {
  const launch = launchFor(client, ref.threadId);
  if (!launch) return;
  try {
    await client.request(native, 'terminal.open', { threadId: ref.threadId, terminalId, cwd: launch.cwd,
      ...(launch.worktreePath != null ? { worktreePath: launch.worktreePath } : {}), env: launch.env });
  } catch (error) { failed(client, 'terminal open', error); }
}

/** closeTerminal: delete the session with its history (on failure, type `exit`), drop the tab at once. */
async function closeTerminal(client: T3Client, native: Native, ref: ScopedThreadRef, terminalId: string): Promise<void> {
  terminalUiStore(client).getState().closeTerminal(ref, terminalId);
  drawerState(client).focusRequest++;
  await closeTerminalSession(client, native, ref, terminalId);
}

export async function closeTerminalSession(client: T3Client, native: Native, ref: ScopedThreadRef, terminalId: string): Promise<void> {
  try { await client.request(native, 'terminal.close', { threadId: ref.threadId, terminalId, deleteHistory: true }); }
  catch {
    try { await client.request(native, 'terminal.write', { threadId: ref.threadId, terminalId, data: 'exit\n' }); }
    catch (error) { failed(client, 'terminal write', error); }
  }
}

/** ChatView.runProjectScriptInTerminal: use the active idle shell, otherwise start a new group. */
export async function runTerminalCommand(client: T3Client, native: Native, storage: Files, command: string, scriptName = ''): Promise<void> {
  void storage;
  const ref = await ensureTerminalRef(client, native), launch = ref ? launchFor(client, ref.threadId) : null;
  if (!ref || !launch) throw new ClientError('A project is required to run a terminal command.');
  const store = terminalUiStore(client), state = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
  const active = state.activeTerminalId;
  const busy = knownSessions(client, ref).find(session => session.target.terminalId === active)?.state.summary?.hasRunningSubprocess === true;
  const terminalId = active && state.terminalIds.includes(active) && !busy ? active : nextTerminalId(allocatableIds(client, ref));
  await client.request(native, 'terminal.open', { threadId: ref.threadId, terminalId, cwd: launch.cwd,
    ...(launch.worktreePath ? { worktreePath: launch.worktreePath } : {}), env: launch.env, ...(terminalId !== active ? { cols: 120, rows: 30 } : {}) });
  store.getState().ensureTerminal(ref, terminalId, { open: true });
  drawerState(client).focusRequest++;
  try { await client.request(native, 'terminal.write', { threadId: ref.threadId, terminalId, data: `${command}\r` }); }
  catch (error) { if (letGo(error)) throw error; throw new ClientError(error instanceof Error ? error.message : scriptName ? `Failed to run script "${scriptName}".` : 'Failed to run command.'); }
}

export async function handleTerminalMessage(client: T3Client, native: Native, message: Obj): Promise<boolean> {
  if (message.type === 'link') { await terminalLinkAction(client, native, message); return true; }
  if (message.type !== 'selectionAction' || message.action !== 'add-to-chat') return false;
  const ref = activeRef(client);
  if (!ref || str(message.threadId) !== ref.threadId) return true;
  const selected = str(message.terminalId);
  const summary = knownSessions(client, ref).find(session => session.target.terminalId === selected)?.state.summary;
  await insertContext(client, native, 'terminal', JSON.stringify({ id: crypto.randomUUID(), threadId: ref.threadId, terminalId: selected,
    terminalLabel: resolveTerminalSessionLabel(selected, summary), text: str(message.text), lineStart: Number(message.lineStart) || 1, lineEnd: Number(message.lineEnd) || 1, createdAt: new Date(composerNow(client)).toISOString() }));
  return true;
}

/** useThreadActions delete: close every session of the thread with its history, then forget its drawer. */
export async function closeThreadTerminals(client: T3Client, native: Native, threadId: string): Promise<void> {
  if (!client.environmentId || !threadId) return;
  try { await client.request(native, 'terminal.close', { threadId, deleteHistory: true }); }
  catch (error) { failed(client, 'terminal close', error); }
  terminalUiStore(client).getState().clearTerminalUiState({ environmentId: client.environmentId, threadId });
}
// The sidebar delete (sidebar-commands.ts `remove`, thread-commands-and-keys) closes terminals through this hook.
setCloseThreadTerminals(closeThreadTerminals);

/** `terminallocal:` ops: toggle (⌘J and the layout buttons), new, close (confirmed), exited, height. */
export async function terminalOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  if (!op.startsWith('terminallocal:')) return false;
  void n; void storage;
  Object.assign(out, { message: '', id, value });
  const store = terminalUiStore(this), drawer = drawerState(this);
  const action = op.slice('terminallocal:'.length);
  if (action === 'message') {
    const message = obj(safeJson(value));
    if (message.type === 'command') {
      const command = terminalCommandAction(this, message);
      if (command) return terminalOps.call(this, command.op, command.id, '', n, native, storage, out);
      return true;
    }
  }
  if (action === 'run-script') { await runProjectTerminalScript(this, native, storage, id); return true; }
  if (action === 'run-command') { await runTerminalCommand(this, native, storage, value.trim()); return true; }
  if (action === 'toggle') {
    const ref = await ensureTerminalRef(this, native);
    if (!ref) return true;
    const ui = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
    const nextOpen = !ui.terminalOpen;
    if (nextOpen && ui.terminalIds.length === 0) {
      if (!launchFor(this, ref.threadId)) return true;
      const terminalId = nextTerminalId(allocatableIds(this, ref));
      store.getState().ensureTerminal(ref, terminalId, { open: true });
      drawer.focusRequest++;
      await openTerminal(this, native, ref, terminalId);
      return true;
    }
    store.getState().setTerminalOpen(ref, nextOpen);
    if (nextOpen) drawer.focusRequest++;
    else { clearTerminalFocus(this); out.message = 'terminal:focus-composer'; }
    return true;
  }
  const [threadKey, terminalId = ''] = id.includes('|') ? [id.slice(0, id.lastIndexOf('|')), id.slice(id.lastIndexOf('|') + 1)] : [id, ''];
  const ref = parseScopedThreadKey(threadKey) ?? await ensureTerminalRef(this, native);
  if (!ref) return true;
  const wasOpen = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref).terminalOpen;
  if (action === 'focus-in' || action === 'focus-out') {
    recordTerminalFocus(this, { ...ref, terminalId, surface: value || 'drawer', focused: action === 'focus-in' });
    if (action === 'focus-in' && terminalId) store.getState().setActiveTerminal(ref, terminalId);
    return true;
  }
  if (action === 'focus') { if (scopedThreadKey(ref) === (activeRef(this) ? scopedThreadKey(activeRef(this)!) : '')) drawer.focusRequest++; return true; }
  if (action === 'activate' && terminalId) { store.getState().setActiveTerminal(ref, terminalId); drawer.focusRequest++; }
  else if (action === 'new' || action === 'split' || action === 'split-vertical') {
    if (!launchFor(this, ref.threadId)) return true;
    if (terminalId && action !== 'new') store.getState().setActiveTerminal(ref, terminalId);
    const ui = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
    if (action !== 'new' && terminalLayout(ui.terminalIds, ui.terminalGroups, ui.activeTerminalId, ui.activeTerminalGroupId).splitDisabled) return true;
    const next = nextTerminalId(allocatableIds(this, ref));
    if (action === 'split') store.getState().splitTerminal(ref, next);
    else if (action === 'split-vertical') store.getState().splitTerminalVertical(ref, next);
    else store.getState().newTerminal(ref, next);
    drawer.focusRequest++;
    await openTerminal(this, native, ref, next);
  } else if (action === 'close' && terminalId) await closeTerminal(this, native, ref, terminalId);
  else if (action === 'exited' || action === 'message') {
    const message = obj(safeJson(value));
    if (message.type === 'focus') {
      recordTerminalFocus(this, message);
      if (message.focused === true && str(message.terminalId)) store.getState().setActiveTerminal(ref, str(message.terminalId));
      return true;
    }
    if (await handleTerminalMessage(this, native, message)) return true;
    if (action === 'message' && message.type !== 'exited') return true;
    // onSessionExited: the tab closes without a confirmation.
    const exited = str(message.terminalId);
    if (exited) await closeTerminal(this, native, ref, exited);
  } else if (action === 'height') {
    const height = Number(value);
    if (Number.isFinite(height) && height > 0) store.getState().setTerminalHeight(ref, clampDrawerHeight(height, drawer.viewportHeight || undefined));
  }
  const focusOwner = focusedTerminal(this);
  if (wasOpen && (!focusOwner || focusOwner.surface === 'drawer') && ['close', 'exited', 'message'].includes(action) && activeRef(this)?.threadId === ref.threadId && activeRef(this)?.environmentId === ref.environmentId && selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref).terminalIds.length === 0) {
    clearTerminalFocus(this); out.message = 'terminal:focus-composer';
  }
  return true;
}
const safeJson = (text: string): unknown => { try { return JSON.parse(text); } catch { return {}; } };
