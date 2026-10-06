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
// - One terminal per drawer until 20261005-terminal-layout: the toolbar has Close only; New appears in
//   the empty state. A draft (no server thread yet) has no terminal: the clone's drafts carry no thread id.
// - Command failures are kept as the view's `failure` (the agent's `state` shows it), as the reference's
//   atom commands only report them to the console; the attach stream's error shows in the terminal as
//   `[terminal] <message>`.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import type { DispatchAdd } from './keyboard-dispatch';
import { obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { subscriptionSerial } from './shell-vcs';
import { commandShortcut } from './shell';
import { terminalCloseConfirmMessage } from './terminal-close';
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

export const TERMINAL_METADATA_KEY = 'terminal-metadata';
const METADATA_RETRY_MS = 3000;

export type TerminalDrawerView = {
  available: boolean; open: boolean; threadKey: string; environmentId: string; threadId: string;
  terminalId: string; label: string; cwd: string; worktree: string; env: string; height: number;
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
function activeRef(client: T3Client): ScopedThreadRef | null {
  return client.environmentId && client.threadId ? { environmentId: client.environmentId, threadId: client.threadId } : null;
}
function threadOf(client: T3Client, threadId: string): Obj | undefined { return client.shell.threads.find(thread => thread.id === threadId); }
function projectOf(client: T3Client, thread: Obj | undefined): Obj | undefined {
  const projectId = str(thread?.projectId) || client.projectId;
  return client.shell.projects.find(project => project.id === projectId);
}
/** The launch location: the thread's worktree, else the project root, and the project script env. */
function launchFor(client: T3Client, threadId: string): { cwd: string; worktreePath: string | null; env: Record<string, string> } | null {
  const thread = threadOf(client, threadId), project = projectOf(client, thread);
  const root = str(project?.workspaceRoot);
  if (!project || !root) return null;
  const worktreePath = str(thread?.worktreePath) || null;
  return { cwd: projectScriptCwd({ project: { cwd: root }, worktreePath }), worktreePath, env: projectScriptRuntimeEnv({ project: { cwd: root }, worktreePath }) };
}

/** PanelLayoutControls `terminalAvailable` (an active project), for a server thread. */
export function terminalAvailable(client: T3Client): boolean {
  const ref = activeRef(client);
  return !!ref && !!launchFor(client, ref.threadId);
}
/** The active thread's drawer is open. */
export function terminalOpen(client: T3Client): boolean {
  const ref = activeRef(client);
  return !!ref && selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref).terminalOpen;
}

const sessionKey = (ref: ScopedThreadRef, terminalId: string) => JSON.stringify([ref.environmentId, ref.threadId, terminalId]);

/** useKnownTerminalSessions for one thread: its server sessions in terminal order (empty until the stream answers). */
function knownSessions(client: T3Client, ref: ScopedThreadRef) {
  return selectKnownTerminalSessions(drawerState(client).metadata.metadata, ref.environmentId, ref.threadId);
}
/** Every id a new terminal must avoid: the server's, the drawer's (the server list lags a fresh open). */
function allocatableIds(client: T3Client, ref: ScopedThreadRef): string[] {
  const ui = selectThreadTerminalUiState(terminalUiStore(client).getState().terminalUiStateByThreadKey, ref);
  return [...new Set([...knownSessions(client, ref).map(session => session.target.terminalId), ...ui.terminalIds])];
}

/** keyboard-dispatch MAIN_ROWS: terminal.toggle (⌘J by default) wherever the drawer can open. */
export function terminalRows(add: DispatchAdd, client: T3Client): void {
  if (terminalAvailable(client)) add('terminal.toggle', 'command', 'terminallocal:toggle', 'Toggle Terminal');
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
  } catch (error) { state.error = error instanceof Error ? error.message : 'Terminal metadata is unavailable.'; }
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
    const serverIds = knownSessions(client, ref).map(session => session.target.terminalId);
    if (!terminalIdListsEqual(serverIds, ui.terminalIds) && !serverTerminalIdsStrictSubsetOfClient(serverIds, ui.terminalIds)) {
      store.getState().reconcileTerminalIds(ref, serverIds);
      ui = selectThreadTerminalUiState(store.getState().terminalUiStateByThreadKey, ref);
    }
  }
  // ChatView's mounted-thread effect: the active open thread plus the ten most recent hidden open ones.
  const states = store.getState().terminalUiStateByThreadKey;
  const existing = new Set(client.shell.threads.map(thread => scopedThreadKey({ environmentId: client.environmentId, threadId: str(thread.id) })));
  const openKeys = Object.entries(states).filter(([key, state]) => state.terminalOpen && existing.has(key)).map(([key]) => key);
  drawer.mounted = reconcileMountedTerminalThreadIds({ currentThreadIds: drawer.mounted, openThreadIds: openKeys, activeThreadId: ref ? scopedThreadKey(ref) : null,
    activeThreadTerminalOpen: !!launch && ui.terminalOpen });
  const retained = drawer.mounted.flatMap(key => {
    const mountedRef = parseScopedThreadKey(key);
    return mountedRef ? (states[key]?.terminalIds ?? []).map(id => sessionKey(mountedRef, id)) : [];
  });
  if (native?.available && JSON.stringify(retained) !== drawer.retained) {
    drawer.retained = JSON.stringify(retained);
    await client.call(native, { op: 'terminalRetain', sessions: retained }).catch(() => { drawer.retained = ''; });
  }
  const terminalId = ui.terminalIds.includes(ui.activeTerminalId) ? ui.activeTerminalId : ui.terminalIds[0] ?? '';
  const summary = ref && terminalId ? knownSessions(client, ref).find(session => session.target.terminalId === terminalId)?.state.summary : null;
  const label = terminalId ? resolveTerminalSessionLabel(terminalId, summary) : '';
  const [closeTitle, closeBody] = terminalCloseConfirmMessage([label || getTerminalLabel(terminalId)]);
  const keyNew = commandShortcut(client.config, 'terminal.new'), keyClose = commandShortcut(client.config, 'terminal.close'), keyToggle = commandShortcut(client.config, 'terminal.toggle');
  return {
    available: !!launch, open: !!launch && ui.terminalOpen, threadKey: ref ? scopedThreadKey(ref) : '', environmentId: ref?.environmentId ?? '', threadId: ref?.threadId ?? '',
    terminalId, label, cwd: launch?.cwd ?? '', worktree: launch?.worktreePath ?? '', env: JSON.stringify(launch?.env ?? {}),
    height: clampDrawerHeight(ui.terminalHeight, drawer.viewportHeight || undefined), focusRequest: drawer.focusRequest,
    closeTitle, closeBody, closeLabel: ui.terminalOpen && keyClose ? `Close Terminal (${keyClose})` : 'Close Terminal',
    newLabel: ui.terminalOpen && keyNew ? `New Terminal (${keyNew})` : 'New Terminal',
    toggleLabel: launch ? `Toggle terminal drawer${keyToggle ? ` (${keyToggle})` : ''}` : 'Terminal drawer is unavailable',
    sessionKey: ref && terminalId ? sessionKey(ref, terminalId) : '', mounted: drawer.mounted.length,
    metadata: drawer.metadata.metadata === null ? (drawer.metadata.error || 'waiting') : `${drawer.metadata.metadata.length} sessions`,
    failure: drawer.failure,
  };
}

// ── Commands (client.command `terminallocal:*`) ──────────────────────────────────────────────────

function failed(client: T3Client, label: string, error: unknown): void {
  drawerState(client).failure = `${label}: ${error instanceof Error ? error.message : String(error)}`;
}

async function openTerminal(client: T3Client, native: Native, ref: ScopedThreadRef, terminalId: string): Promise<void> {
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
  try { await client.request(native, 'terminal.close', { threadId: ref.threadId, terminalId, deleteHistory: true }); }
  catch {
    try { await client.request(native, 'terminal.write', { threadId: ref.threadId, terminalId, data: 'exit\n' }); }
    catch (error) { failed(client, 'terminal write', error); }
  }
}

/** useThreadActions delete: close every session of the thread with its history, then forget its drawer. */
export async function closeThreadTerminals(client: T3Client, native: Native, threadId: string): Promise<void> {
  if (!client.environmentId || !threadId) return;
  try { await client.request(native, 'terminal.close', { threadId, deleteHistory: true }); }
  catch (error) { failed(client, 'terminal close', error); }
  terminalUiStore(client).getState().clearTerminalUiState({ environmentId: client.environmentId, threadId });
}

/** `terminallocal:` ops: toggle (⌘J and the layout buttons), new, close (confirmed), exited, height. */
export async function terminalOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  if (!op.startsWith('terminallocal:')) return false;
  void n; void storage;
  Object.assign(out, { message: '', id, value });
  const store = terminalUiStore(this), drawer = drawerState(this);
  const action = op.slice('terminallocal:'.length);
  if (action === 'toggle') {
    const ref = activeRef(this);
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
    return true;
  }
  const [threadKey, terminalId = ''] = id.includes('|') ? [id.slice(0, id.lastIndexOf('|')), id.slice(id.lastIndexOf('|') + 1)] : [id, ''];
  const ref = parseScopedThreadKey(threadKey);
  if (!ref) return true;
  if (action === 'new') {
    if (!launchFor(this, ref.threadId)) return true;
    const next = nextTerminalId(allocatableIds(this, ref));
    store.getState().newTerminal(ref, next);
    drawer.focusRequest++;
    await openTerminal(this, native, ref, next);
  } else if (action === 'close' && terminalId) await closeTerminal(this, native, ref, terminalId);
  else if (action === 'exited') {
    // onSessionExited: the tab closes without a confirmation.
    const exited = str(obj(safeJson(value)).terminalId);
    if (exited) await closeTerminal(this, native, ref, exited);
  } else if (action === 'height') {
    const height = Number(value);
    if (Number.isFinite(height) && height > 0) store.getState().setTerminalHeight(ref, clampDrawerHeight(height, drawer.viewportHeight || undefined));
  }
  return true;
}
const safeJson = (text: string): unknown => { try { return JSON.parse(text); } catch { return {}; } };
