// The window shell's projection (MIT reference: apps/web/src/components/ui/
// toast.tsx, chat/ChatHeader.tsx, chat/PanelLayoutControls.tsx,
// RightPanelTabs.tsx RightPanelEmptyState, threadActionMenu.logic.ts):
// the toast stack with its timers, the header's panel-control labels, the
// right panel's surface chooser and the thread title's action menu.
import { noteAutomationWindow } from './browser-automation';
import type { T3Client } from './client';
import { terminalAvailable, terminalOpen } from './terminal-drawer-view'; // terminal-drawer
import type { DispatchContext } from './keyboard-dispatch'; // terminal-layout: terminal labels resolve with terminalFocus
import { highlightPending } from './r12-render-highlight';
import { toasts, dismissToast, type Toast, type ToastAction, type ToastKind } from './toast';
import { arr, obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { shortcutInput } from './keybinding-settings';
import { shortcutLabel } from './keybinding-view';
import { snoozePresets } from './sidebar-presentation';
import { canSnooze, effectiveSnoozed } from './sidebar-model';
import { diffNotGit } from './diff';
import { threadNotifications, providerUpdates, nativeNotifyStatus, reportWindowFacts, type NotifyStatus } from './shell-notify';
import { slowRequests, tracking } from './shell-slow';
import { settleLiveTraces } from './r3-protocol-reader'; // r13-slow: answered requests whose answer was let go end on the next shell read
import { nightlyMobileBetaNotice } from './shell-nightly';
import { pullRequestPanelTarget } from './shell-pr';
import { availability, panelView, surfaceStore } from './r4-surfaces-panel'; // r4-surfaces: the tabbed Files / Linked pull requests / Device surfaces
import { syncRightPanels } from './r10-device-panels'; // lane r10-device: the right panels persist across launches
import { revealWaiting } from './r10-device-crumbs'; // lane r10-device: a Files reveal still missing a folder retries
import { gitTicking } from './r4-git-actions'; // lane r4-git: a running action's clock
import { refsPageWanted } from './r6-polish-refs'; // r6-polish: a ref list's next page
import { effectiveShortcut } from './r4-polish-shortcuts'; // r4-polish: header shortcut labels
import { checkoutView } from './r9-connect-checkout';
import { cloneToasts } from './project-clones-live';
import { activationOpen } from './desktop-activation'; // app-activation: a `t3 app` request opens like a clicked notification

export type ShellToastView = {
  id: number; kind: string; title: string; description: string; icon: string; iconColor: string; provider: string;
  actionLabel: string; actionOp: string; actionId: string; actionValue: string; actionOutline: boolean;
  secondaryLabel: string; secondaryOp: string; secondaryId: string; secondaryValue: string; secondaryGhost: boolean;
  stacked: boolean; copyText: string; copied: boolean; index: number;
  /** browser-surface part 3: the additional action, and each button's "Copied!" state (2 s after its copy). */
  extraLabel: string; extraOp: string; extraId: string; extraValue: string; actionDone: boolean; secondaryDone: boolean; extraDone: boolean;
  details: { id: string; title: string; subtitle: string; last: boolean }[]; expandLabel: string; collapseLabel: string;
};
export type ShellMenuItem = { id: string; label: string; icon: string; destructive: boolean; disabled: boolean; separated: boolean;
  submenu: string; checked: boolean; op: string; target: string; value: string; confirmTitle: string; confirmBody: string; offset: number };
export type ShellSurface = { id: string; label: string; icon: string; shortcut: string; available: boolean; reason: string };

/** Base UI's default toast limit: later toasts wait behind the third. */
export const TOAST_LIMIT = 3;
const KIND_ICON: Record<ToastKind, [string, string]> = {
  success: ['circle-check', '#00bc7d'], error: ['circle-alert', 'light-dark(#fb2c36, #fb414a)'],
  warning: ['triangle-alert', '#fe9a00'], info: ['info', '#2b7fff'], loading: ['loader', 'light-dark(#27272a, #f5f5f5)'],
};
/** Leading glyphs callers name (ThreadToastData.leadingIcon): `<glyph>:<tone>`. */
const TONES: Record<string, string> = {
  'success-foreground': 'light-dark(#007a55, #00d492)', 'warning-foreground': 'light-dark(#bb4d00, #ffb900)',
  'destructive-foreground': 'light-dark(#c10007, #ff6467)', 'info-foreground': 'light-dark(#1447e6, #51a2ff)',
  success: '#00bc7d', warning: '#fe9a00', info: '#2b7fff', destructive: 'light-dark(#fb2c36, #fb414a)',
  foreground: 'light-dark(#27272a, #f5f5f5)',
};

type Clock = { remaining: number; last: number; timeout?: number };
type ShellState = { clocks: Map<number, Clock>; handled: Set<string>; status: NotifyStatus; now: number; copied: Map<number, number>; copiedActions: Map<string, number> };
const states = new WeakMap<T3Client, ShellState>();
export function shellState(client: T3Client): ShellState {
  let state = states.get(client);
  if (!state) { state = { clocks: new Map(), handled: new Set(), status: { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' }, now: 0, copied: new Map(), copiedActions: new Map() }; states.set(client, state); }
  return state;
}

/**
 * The contract records each dismissal as `x<id>` (the corner orb, which runs
 * the toast's onClose) or `a<id>` (an action, which only closes it) in one
 * growing string, so a dismissal is never lost to a superseded request.
 */
export function applyDismissals(client: T3Client, dismissed: string): boolean {
  const state = shellState(client);
  let closed = false;
  for (const token of dismissed.split(/\s+/)) {
    const match = /^([xa])(\d+)$/.exec(token);
    if (!match || state.handled.has(token)) continue;
    state.handled.add(token);
    const id = Number(match[2]);
    const toast = toasts(client).find(entry => entry.id === id);
    if (!toast) continue;
    if ((match[1] === 'x' || toast.closeOnAction) && toast.onClose) { closed = true; try { toast.onClose(); } catch { /* a dismissal never fails the shell */ } }
    dismissToast(client, id);
  }
  return closed;
}

/** CopyErrorButton's useCopyToClipboard: "Copied error" with a check for 2 s after a copy, on the shell's clock. */
export const COPIED_MS = 2000;
export function markCopied(client: T3Client, id: number): void {
  const state = shellState(client);
  state.copied.set(id, state.now);
}
/** A toast button's own copy (Copy image, Copy path): "Copied!" and disabled for 2 s (PreviewView's `pathCopied`). */
export function markActionCopied(client: T3Client, id: number, op: string): void {
  const state = shellState(client);
  state.copiedActions.set(`${id}:${op}`, state.now);
}
export function copiedActions(client: T3Client): Set<string> {
  const state = shellState(client);
  for (const [key, at] of [...state.copiedActions]) if (state.now - at >= COPIED_MS) state.copiedActions.delete(key);
  return new Set(state.copiedActions.keys());
}
export function copiedToasts(client: T3Client, now: number): Set<number> {
  const state = shellState(client);
  if (Number.isFinite(now) && now > 0) state.now = now;
  for (const [id, at] of [...state.copied]) if (state.now - at >= COPIED_MS) state.copied.delete(id);
  return new Set(state.copied.keys());
}

/**
 * Base UI timers: each toast counts down its own timeout while the region is
 * not hovered (`paused`), from the first frame it is shown. A zero timeout
 * (loading, provider updates) stays until dismissed.
 */
export function advanceToasts(client: T3Client, now: number, paused: boolean): Toast[] {
  const state = shellState(client);
  const live = toasts(client);
  if (!Number.isFinite(now) || now <= 0) return live;
  for (const toast of live) {
    let clock = state.clocks.get(toast.id);
    // A toast updated to another timeout (a clone's running → done) starts that timer afresh.
    if (!clock || (clock.timeout ?? toast.timeoutMs) !== toast.timeoutMs) { clock = { remaining: toast.timeoutMs, last: now, timeout: toast.timeoutMs }; state.clocks.set(toast.id, clock); }
    // Only the front three are visible; hidden toasts wait their turn.
    const visible = live.slice(-TOAST_LIMIT).includes(toast);
    // The window ticks every 500 ms only while toasts exist, so a longer gap
    // is a clock that was idle, never elapsed time on screen.
    if (!paused && visible && toast.timeoutMs > 0) clock.remaining -= Math.min(1000, Math.max(0, now - clock.last));
    clock.last = now;
    if (toast.timeoutMs > 0 && clock.remaining <= 0) dismissToast(client, toast.id);
  }
  const ids = new Set(toasts(client).map(toast => toast.id));
  for (const id of [...state.clocks.keys()]) if (!ids.has(id)) state.clocks.delete(id);
  return toasts(client);
}

/** Newest first, as Base UI stacks them; `index` is the visible depth. */
export function toastViews(queue: Toast[], copied: ReadonlySet<number> = new Set(), copiedOps: ReadonlySet<string> = new Set()): ShellToastView[] {
  return [...queue].reverse().map((toast, index) => {
    const leading = str(toast.leading);
    const provider = leading.startsWith('provider:') ? leading.slice(9) : '';
    const [glyph, tone] = leading && !provider && leading !== 'none' ? leading.split(':') : ['', ''];
    const [kindGlyph, kindColor] = KIND_ICON[toast.kind] ?? KIND_ICON.info;
    const action = toast.action, secondary = toast.secondary ?? null, extra = toast.extra ?? null;
    const done = (entry: ToastAction | null) => !!entry && copiedOps.has(`${toast.id}:${entry.op}`);
    const label = (entry: ToastAction | null) => (done(entry) ? 'Copied!' : entry?.label ?? '');
    return {
      id: toast.id, kind: toast.kind, title: toast.title, description: toast.description,
      icon: provider || leading === 'none' ? '' : glyph || kindGlyph, iconColor: provider || leading === 'none' ? '' : glyph ? TONES[tone ?? ''] ?? kindColor : kindColor, provider,
      actionLabel: label(action), actionOp: action?.op ?? '', actionId: action?.id ?? '', actionValue: action?.value ?? '',
      actionOutline: toast.actionVariant === 'outline',
      secondaryLabel: label(secondary), secondaryOp: secondary?.op ?? '', secondaryId: secondary?.id ?? '', secondaryValue: secondary?.value ?? '', secondaryGhost: toast.secondaryVariant === 'ghost',
      stacked: toast.stacked === true && !!action, copyText: toast.kind === 'error' && !toast.hideCopy ? toast.description : '', copied: copied.has(toast.id), index,
      extraLabel: label(extra), extraOp: extra?.op ?? '', extraId: extra?.id ?? '', extraValue: extra?.value ?? '', actionDone: done(action), secondaryDone: done(secondary), extraDone: done(extra),
      details: (toast.details ?? []).map((detail, at, all) => ({ ...detail, last: at === all.length - 1 })), expandLabel: toast.expandLabels?.expand ?? 'Show details', collapseLabel: toast.expandLabels?.collapse ?? 'Hide details',
    };
  });
}

/** The last binding for a command, as formatShortcutLabel prints it (⌥⌘B). */
export function commandShortcut(config: Obj, command: string, context: Partial<DispatchContext> = {}): string {
  const shortcut = effectiveShortcut(config, command, context); // r4-polish-shortcuts.ts: findEffectiveShortcutForCommand
  return shortcut ? shortcutLabel(shortcutInput(shortcut)) : '';
}

/** RightPanelEmptyState: every surface stays listed; unavailable ones say why. */
export function surfaces(client: T3Client): ShellSurface[] {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  // threadPullRequestPanelTarget (shell-pr.ts, upstream f90b77d809): the current link, the legacy link or the branch's PR.
  const target = thread ? pullRequestPanelTarget(thread) : null, can = availability(client);
  const row = (id: string, label: string, icon: string, shortcut: string, available: boolean, reason: string): ShellSurface =>
    ({ id, label, icon, shortcut, available, reason: available ? '' : reason });
  return [
    row('browser', 'Browser', 'earth', 'B', client.available === true, 'Only available in the desktop app.'), // browser-surface: the module's WKWebView (isPreviewSupportedInRuntime)
    row('terminal', 'Terminal', 'square-terminal', 'T', terminalAvailable(client), 'Available when a project is open.'),
    row('files', 'Files', 'files', 'F', can.files, 'Available when a project is open.'),
    row('diff', 'Diff', 'file-diff', 'D', !!client.threadId && client.ready && !diffNotGit(client), 'Available for Git repositories.'),
    // r5-panels: ChatView pullRequestSurfaceAvailable (supportsPullRequests and a panel target); the detail panel opens beside the thread.
    row('pull-request', 'Pull request', 'git-pull-request-arrow', 'P', can.pullRequest && !!target, 'No pull request on this branch yet.'),
    row('pull-requests', 'Linked pull requests', 'link-2', 'L', can.pullRequests, 'No linked pull requests available.'),
    row('device', 'Device', 'smartphone', 'M', can.device, 'Available from a thread.'),
  ];
}

/**
 * buildThreadActionMenuItems for the chat header (no project filter there).
 * Submenu children carry their parent's id in `submenu`; ops prefixed `ui:`
 * are answered by the window, the rest are client commands.
 */
export function titleMenu(client: T3Client, now: number): ShellMenuItem[] {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  if (!thread) return [{ ...item('project-settings', 'Project settings', 'settings', 'ui:project-settings', client.projectId) }];
  const caps = obj(obj(client.config.environment).capabilities);
  const id = str(thread.id), branch = str(thread.branch);
  // useThreadActionMenu: only an explicit settle offers Un-settle; a snooze that has woken or raised its hand reads as awake.
  const settled = thread.settledOverride === 'settled';
  const snoozed = effectiveSnoozed(thread, now);
  const running = !!thread.activeRunId || ['preparing', 'starting', 'running', 'waiting'].includes(str(thread.status));
  const regenerating = thread.titleRegeneration != null;
  const autoSettle = thread.autoSettleDisabledAt == null;
  const items: ShellMenuItem[] = [];
  if (branch) items.push(item('new-thread-on-branch', `New thread on ${branch}`, 'message-square-plus', 'ui:new-thread', client.projectId));
  const prefs = client.local.clientSettings, title = str(thread.title);
  if (caps.threadPinning === true) items.push(thread.pinnedAt ? confirmable(item('unpin', 'Unpin thread', 'pin-off', 'shell:unpin', id), prefs.confirmThreadUnpin === true, `Unpin thread "${title}"?`, 'This will move the thread out of your pinned section.') : item('pin', 'Pin thread', 'pin', 'shell:pin', id));
  if (caps.threadSettlement === true) items.push(settled ? item('unsettle', 'Un-settle thread', 'circle-check', 'chat:unsettle', id) : item('settle', 'Settle thread', 'circle-check', 'chat:settle', id));
  if (caps.threadSnooze === true) {
    if (snoozed) items.push(item('unsnooze', 'Wake thread', 'clock', 'chat:unsnooze', id));
    else {
      const presets = snoozePresets(now, client.local.deviceSettings.timestampFormat);
      items.push({ ...item('snooze', 'Snooze', 'clock', 'ui:submenu', 'snooze'), disabled: !canSnooze(thread, now) });
      for (const preset of presets) items.push({ ...item(`snooze:${preset.id}`, `${preset.label} (${preset.wakeLabel})`, '', 'chat:snooze', id, preset.until), submenu: 'snooze' });
      items.push({ ...item('snooze:custom', 'Custom…', '', 'ui:custom-snooze', id), submenu: 'snooze', separated: true });
    }
  }
  items.push({ ...item('rename', 'Rename thread', 'pencil', 'ui:rename', id), separated: true });
  if (caps.threadTitleRegeneration === true) items.push({ ...item('regenerate-title', regenerating ? 'Regenerating…' : 'Regenerate title', 'refresh', 'shell:regenerate-title', id), disabled: regenerating });
  items.push(item('mark-unread', 'Mark unread', 'mail-open', 'shell:mark-unread', id));
  if (caps.threadAutoSettleOptOut === true) {
    items.push(item('auto-settle', 'Auto-settle behavior', 'timer', 'ui:submenu', 'auto-settle'));
    items.push({ ...item('auto-settle:enabled', 'Enabled', '', 'shell:auto-settle', id, 'true'), submenu: 'auto-settle', checked: autoSettle });
    items.push({ ...item('auto-settle:disabled', 'Disabled', '', 'shell:auto-settle', id, 'false'), submenu: 'auto-settle', checked: !autoSettle });
  }
  items.push({ ...item('copy', 'Copy', 'copy', 'ui:submenu', 'copy'), separated: true });
  items.push({ ...item('copy-path', 'Path', 'folder', 'shelllocal:copy-path', id), submenu: 'copy' });
  if (branch) items.push({ ...item('copy-branch', 'Branch', 'git-branch', 'shelllocal:copy-branch', id), submenu: 'copy' });
  items.push({ ...item('copy-thread-id', 'Thread ID', 'hash', 'shelllocal:copy-thread-id', id), submenu: 'copy' });
  items.push(item('project-settings', 'Project settings', 'settings', 'ui:project-settings', str(thread.projectId)));
  items.push({ ...confirmable(item('archive', 'Archive thread', 'archive', 'shell:archive', id), prefs.confirmThreadArchive === true, `Archive thread "${title}"?`, ''), disabled: running, separated: true });
  items.push({ ...confirmable(item('delete', 'Delete', 'trash', 'shell:delete', id), prefs.confirmThreadDelete === true, `Delete thread "${title}"?`, 'This permanently clears conversation history for this thread.'), destructive: true });
  return withOffsets(items);
}
/** Each top-level row's top inside its menu card (1pt border, 4pt inset, 28pt rows, 9pt separators): where its submenu card starts. */
export function withOffsets(items: ShellMenuItem[]): ShellMenuItem[] {
  let top = 5;
  return items.map(entry => {
    if (entry.submenu !== '') return entry;
    if (entry.separated) top += 9;
    const placed = { ...entry, offset: top };
    top += 28;
    return placed;
  });
}
function item(id: string, label: string, icon: string, op: string, target: string, value = ''): ShellMenuItem {
  return { id, label, icon, destructive: false, disabled: false, separated: false, submenu: '', checked: false, op, target, value, confirmTitle: '', confirmBody: '', offset: 0 };
}
/** api.dialogs.confirm before the command when its Confirmations setting is on: the window asks, then runs `value`. */
function confirmable(entry: ShellMenuItem, ask: boolean, title: string, body: string): ShellMenuItem {
  return ask ? { ...entry, op: 'ui:confirm', value: entry.op, confirmTitle: title, confirmBody: body } : entry;
}

/** Lane r10-device: once the preferences were read, a changed right panel is written with them. */
async function panelSaved<T>(client: T3Client, storage: Files, view: T): Promise<T> {
  if (client.preferencesLoaded && syncRightPanels(client, surfaceStore(client).panels)) await client.savePreferences(storage);
  return view;
}

/**
 * One shell answer: toasts after dismissals and timers, notifications, header and menus.
 * `page` is the window's `exactPage()` focus and visibility (exact2 #219).
 */
export async function shellView(client: T3Client, native: Native | null | undefined, storage: Files, now: number, dismissed: string, paused: boolean, sheet = false,
  page: { focused: boolean; visible: boolean } = { focused: true, visible: true }) {
  // A closed toast's onClose may change saved preferences (shell-prefs.ts): write them now.
  if (applyDismissals(client, dismissed)) await client.savePreferences(storage);
  const state = shellState(client);
  nightlyMobileBetaNotice(client, client.preferencesLoaded); // shell-nightly.ts
  if (native?.available) {
    native.watch('t3.notify');
    state.status = await nativeNotifyStatus(native, state.status, page.focused);
    await reportWindowFacts(client, native, page.visible, page.focused); // client-activity-reporting's visible and focused
    noteAutomationWindow(client, page.focused && page.visible); // browser-automation.ts: previewAutomation.focusHost's `focused`
    await providerUpdates(client, storage);
    await cloneToasts(client, native); // project-clones-live.ts: a toast per tracked clone, every environment
    await threadNotifications(client, native, state.status);
    if (tracking(client)) await settleLiveTraces(client, native);
  }
  slowRequests(client, now);
  const queue = advanceToasts(client, now, paused);
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  const menu = titleMenu(client, now);
  return {
    toasts: toastViews(queue, copiedToasts(client, now), copiedActions(client)), toastCount: queue.length, ticking: tracking(client) || gitTicking(client) || refsPageWanted(client) || revealWaiting(client),
    ...(await activationOpen(client, native, state.status)), // openRequest, openThreadId: a clicked notification's thread or a `t3 app` request (desktop-activation.ts)
    keyRightPanel: commandShortcut(client.config, 'rightPanel.toggle'), keyThreadPanel: commandShortcut(client.config, 'threadPanel.toggle'),
    keyTerminal: commandShortcut(client.config, 'terminal.toggle'), keyNewThread: commandShortcut(client.config, 'chat.new'),
    terminalAvailable: terminalAvailable(client), terminalOpen: terminalOpen(client), // terminal-drawer: the layout controls' toggle
    keyNewThreadLocal: commandShortcut(client.config, 'chat.newLocal'), keySidebar: commandShortcut(client.config, 'sidebar.toggle'),
    serverThread: !!thread, projectPath: str(thread?.worktreePath) || str(project?.workspaceRoot),
    surfaces: surfaces(client), titleMenu: menu.filter(entry => entry.submenu === ''), titleSubmenu: menu.filter(entry => entry.submenu !== ''),
    panel: await panelSaved(client, storage, await panelView(client, native, now, sheet)), // r12-threads: `sheet` (window ≤ 980)
    prCheckout: checkoutView(client), // lane r9-connect
    notifications: `${state.status.authorization}${state.status.agent ? ' (agent)' : ''}${state.status.active ? '' : ' (window inactive)'}`,
    highlightPending: highlightPending(), // shiki-residuals: after the panel's code texts asked for their tokens
  };
}
