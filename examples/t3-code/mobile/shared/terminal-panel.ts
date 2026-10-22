// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/terminal-panel.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
import { terminalLayout, terminalSplitLabel, terminalTabs } from './terminal-layout';
import { recordTerminalFocus, clearTerminalFocus } from './terminal-focus';
// T3 Code 1e2ecbd975 rightPanelStore and ChatView terminal surface actions.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import type { Files, Native } from './protocol';
import { obj, str } from './domain';
import { panelState, surfaceStore, type PanelState, type Surface } from './r4-surfaces-panel';
import { closeSurface, registerSurfaceClose } from './right-panel-tabs';
import { activeRef, ensureTerminalRef, launchFor, knownSessions, allocatableIds, openTerminal, closeTerminalSession, handleTerminalMessage, terminalKeybindings, terminalShortcut, terminalCommandAction, terminalOps, type TerminalDrawerView } from './terminal-drawer-view';
import { nextTerminalId, resolveTerminalSessionLabel } from './terminal-labels';
import { terminalCloseConfirmMessage } from './terminal-close';
import { parseScopedThreadKey, scopedThreadKey, terminalUiStore, type ScopedThreadRef } from './terminal-ui-state';

export type PanelTerminal = { terminalIds: string[]; activeTerminalId: string; splitDirection: 'horizontal' | 'vertical' };
const focus = new WeakMap<T3Client, number>();
export const focusPanelTerminal = (client: T3Client) => focus.set(client, (focus.get(client) ?? 0) + 1);
export function terminalPanelIds(client: T3Client, ref: ScopedThreadRef): string[] {
  if (client.environmentId !== ref.environmentId) return [];
  // Panels and the drawer share the scoped server-thread key.
  const state = surfaceStore(client).panels.get(scopedThreadKey(ref));
  return state?.surfaces.flatMap(surface => surface.terminal?.terminalIds ?? []) ?? [];
}
export function terminalPanelRetained(client: T3Client): string[] {
  return [...surfaceStore(client).panels].flatMap(([key, state]) => {
    const ref = parseScopedThreadKey(key);
    return ref ? state.surfaces.flatMap(surface => (surface.terminal?.terminalIds ?? []).map(id => JSON.stringify([ref.environmentId, ref.threadId, id]))) : [];
  });
}
export function panelTerminalLabel(client: T3Client, terminalId: string): string {
  const ref = activeRef(client);
  const summary = ref ? knownSessions(client, ref).find(session => session.target.terminalId === terminalId)?.state.summary : null;
  return resolveTerminalSessionLabel(terminalId, summary);
}
export function terminalSurfaceCloseCopy(client: T3Client, surface: Surface): { closeTitle: string; closeBody: string } {
  const [first, ...rest] = surface.terminal?.terminalIds.map(id => panelTerminalLabel(client, id)) ?? [];
  if (!first) return { closeTitle: '', closeBody: '' };
  const [closeTitle, closeBody] = terminalCloseConfirmMessage([first, ...rest]);
  return { closeTitle, closeBody };
}
export async function addTerminalSurface(client: T3Client, native: Native): Promise<void> {
  const ref = await ensureTerminalRef(client, native);
  if (!ref || !launchFor(client, ref.threadId)) return;
  const terminalId = nextTerminalId(allocatableIds(client, ref));
  openTerminalSurface(panelState(client), terminalId); client.diffOpen = false; focusPanelTerminal(client);
  await openTerminal(client, native, ref, terminalId);
}
// rightPanelStore.ts:715-790 openTerminal / splitTerminal / activateTerminal / closeTerminal over the clone's
// PanelState (`active`, `visible`); closeTerminal is removePanelTerminal.
/** openTerminal: one surface per terminal session, upserted and made active in an open panel. */
export function openTerminalSurface(state: PanelState, terminalId: string): Surface {
  const id = `terminal:${terminalId}`;
  let surface = state.surfaces.find(entry => entry.id === id);
  if (!surface) {
    surface = { id, kind: 'terminal', path: '', line: 0, reveal: 0, terminal: { terminalIds: [terminalId], activeTerminalId: terminalId, splitDirection: 'horizontal' } };
    state.surfaces.push(surface);
  }
  state.active = id; state.visible = true;
  return surface;
}
/** splitTerminal: the new pane goes last and becomes active; the split's direction becomes the surface's. */
export function splitTerminalSurface(state: PanelState, surfaceId: string, terminalId: string, direction: 'horizontal' | 'vertical' = 'horizontal'): void {
  const terminal = state.surfaces.find(entry => entry.id === surfaceId && entry.kind === 'terminal')?.terminal;
  state.visible = true; state.active = surfaceId;
  if (!terminal) return;
  if (!terminal.terminalIds.includes(terminalId)) terminal.terminalIds.push(terminalId);
  terminal.activeTerminalId = terminalId; terminal.splitDirection = direction;
}
/** activateTerminal: selects the surface and, when it holds the pane, the pane. */
export function activateTerminalPane(state: PanelState, surfaceId: string, terminalId: string): void {
  state.active = surfaceId;
  const terminal = state.surfaces.find(entry => entry.id === surfaceId && entry.kind === 'terminal')?.terminal;
  if (terminal?.terminalIds.includes(terminalId)) terminal.activeTerminalId = terminalId;
}
export function removePanelTerminal(state: PanelState, surface: Surface, terminalId: string): void {
  const terminal = surface.terminal;
  if (!terminal || !terminal.terminalIds.includes(terminalId)) return;
  terminal.terminalIds = terminal.terminalIds.filter(id => id !== terminalId);
  if (!terminal.terminalIds.length) closeSurface(state, surface.id);
  else if (terminal.activeTerminalId === terminalId) terminal.activeTerminalId = terminal.terminalIds.at(-1) ?? '';
}
export function installTerminalPanelCleanup(client: T3Client, native: Native, state: PanelState, ref = activeRef(client)): void {
  void state;
  if (!ref) return;
  registerSurfaceClose(client, 'terminal', { cleanup: async surface => {
    for (const id of surface.terminal?.terminalIds ?? []) {
      terminalUiStore(client).getState().closeTerminal(ref, id);
      await closeTerminalSession(client, native, ref, id);
    }
  } });
}
export async function terminalPanelLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const ref = activeRef(client), state = panelState(client);
  if (!ref) return '';
  if (op === 'focus-in' || op === 'focus-out') {
    const terminalId = id.slice(id.lastIndexOf('|') + 1);
    const owner = parseScopedThreadKey(id.slice(0, id.lastIndexOf('|')));
    if (!owner) return '';
    recordTerminalFocus(client, { ...owner, terminalId, surface: value, focused: op === 'focus-in' });
    if (op === 'focus-in') {
      const surface = state.surfaces.find(entry => entry.id === value);
      if (surface?.terminal?.terminalIds.includes(terminalId)) surface.terminal.activeTerminalId = terminalId;
    }
    return '';
  }
  if (op === 'message') {
    let message; try { message = obj(JSON.parse(value)); } catch { return ''; }
    if (message.type === 'focus') {
      recordTerminalFocus(client, message);
      if (message.focused === true) {
        const surface = state.surfaces.find(entry => entry.id === message.surface);
        if (surface?.terminal?.terminalIds.includes(str(message.terminalId))) surface.terminal.activeTerminalId = str(message.terminalId);
      }
      return '';
    }
    if (await handleTerminalMessage(client, native, message)) return '';
    if (message.type !== 'exited') return '';
    op = 'exited'; id = `${scopedThreadKey(ref)}|${str(message.terminalId)}`;
  }
  if (op === 'new') { await addTerminalSurface(client, native); return ''; }
  const terminalId = id.slice(id.lastIndexOf('|') + 1);
  const targetRef = parseScopedThreadKey(id.includes('|') ? id.slice(0, id.lastIndexOf('|')) : id);
  if (targetRef && scopedThreadKey(targetRef) !== scopedThreadKey(ref)) return '';
  const surface = state.surfaces.find(entry => entry.kind === 'terminal' && (terminalId ? entry.terminal?.terminalIds.includes(terminalId) : entry.id === state.active));
  const terminal = surface?.terminal;
  if (!surface || !terminal) return '';
  if (op === 'split' || op === 'split-vertical') {
    if (terminal.terminalIds.length >= 4 || !launchFor(client, ref.threadId)) return '';
    const next = nextTerminalId(allocatableIds(client, ref));
    splitTerminalSurface(state, surface.id, next, op === 'split-vertical' ? 'vertical' : 'horizontal'); focusPanelTerminal(client);
    await openTerminal(client, native, ref, next);
  } else if (op === 'activate' || op === 'focus') {
    if (terminal.terminalIds.includes(terminalId)) { activateTerminalPane(state, surface.id, terminalId); focusPanelTerminal(client); }
  } else if (op === 'close' || op === 'exited') {
    let target = terminalId;
    if (op === 'exited') { try { target = str(obj(JSON.parse(value)).terminalId); } catch { return ''; } }
    if (!terminal.terminalIds.includes(target)) return '';
    removePanelTerminal(state, surface, target); clearTerminalFocus(client);
    terminalUiStore(client).getState().closeTerminal(ref, target); focusPanelTerminal(client);
    await closeTerminalSession(client, native, ref, target);
  }
  return '';
}
export function terminalPanelView(client: T3Client, base: TerminalDrawerView, surface: Surface | null): TerminalDrawerView {
  const terminal = surface?.terminal, ref = activeRef(client), launch = ref ? launchFor(client, ref.threadId) : null;
  if (!terminal || !surface || !ref) return { ...base, open: false, panes: [], tabs: [] };
  const threadKey = scopedThreadKey(ref), terminalId = terminal.activeTerminalId, label = panelTerminalLabel(client, terminalId);
  const [closeTitle, closeBody] = terminalCloseConfirmMessage([label]);
  const panes = terminal.terminalIds.map(id => ({ terminalId: id, label: panelTerminalLabel(client, id), sessionKey: JSON.stringify([ref.environmentId, ref.threadId, id]),
    active: id === terminalId, focusRequest: id === terminalId ? focus.get(client) ?? 0 : 0, target: `${threadKey}|${id}` }));
  // RightPanelTerminalSurface: the surface is the drawer component's one group, so 2+ panes show the tab list.
  const groups = [{ id: surface.id, terminalIds: terminal.terminalIds, splitDirection: terminal.splitDirection }];
  const layout = terminalLayout(terminal.terminalIds, groups, terminalId, surface.id);
  const keyClose = terminalShortcut(client, 'terminal.close'), keyNew = terminalShortcut(client, 'terminal.new');
  const tabs = layout.showTabs ? terminalTabs(groups, terminalId, layout.showHeaders, threadKey, id => panelTerminalLabel(client, id), keyClose) : [];
  return { ...base, open: !!launch, available: !!launch, threadKey, environmentId: ref.environmentId, threadId: ref.threadId,
    terminalId, label, cwd: launch?.cwd ?? '', worktree: launch?.worktreePath ?? '', env: JSON.stringify(launch?.env ?? {}),
    closeTitle, closeBody, sessionKey: JSON.stringify([ref.environmentId, ref.threadId, terminalId]), focusRequest: focus.get(client) ?? 0,
    closeLabel: keyClose ? `Close Terminal (${keyClose})` : 'Close Terminal', newLabel: keyNew ? `New Terminal (${keyNew})` : 'New Terminal',
    keybindings: terminalKeybindings(client), commandPrefix: 'terminalpanellocal', surface: surface.id, panes, tabs, direction: layout.direction,
    showTabs: layout.showTabs, splitDisabled: layout.splitDisabled,
    splitLabel: terminalSplitLabel(false, layout.splitDisabled, terminalShortcut(client, 'terminal.split')),
    splitVerticalLabel: terminalSplitLabel(true, layout.splitDisabled, terminalShortcut(client, 'terminal.splitVertical')) };
}

export async function terminalPanelOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  if (!op.startsWith('terminalpanellocal:')) return false;
  void n; void storage;
  Object.assign(out, { message: '', id, value });
  if (op === 'terminalpanellocal:message') {
    let message; try { message = obj(JSON.parse(value)); } catch { return true; }
    if (message.type === 'command') {
      const command = terminalCommandAction(this, message);
      if (!command) return true;
      if (command.op.startsWith('terminallocal:')) return terminalOps.call(this, command.op, command.id, '', n, native, storage, out);
      op = command.op; id = command.id; value = '';
    }
  }
  panelState(this).userRevision++;
  const state = panelState(this), wasOpen = state.visible;
  await terminalPanelLocal(this, native, op.slice('terminalpanellocal:'.length), id, value);
  if (wasOpen && !state.surfaces.length && state === panelState(this)) out.message = 'terminal:focus-composer';
  return true;
}
