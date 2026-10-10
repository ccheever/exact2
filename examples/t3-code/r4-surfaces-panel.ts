import { parseScopedThreadKey } from './terminal-ui-state';
import { addTerminalSurface, focusPanelTerminal, terminalPanelLocal, terminalPanelView, panelTerminalLabel, terminalSurfaceCloseCopy, installTerminalPanelCleanup, type PanelTerminal } from './terminal-panel';
import { activeRef, terminalAvailable, terminalDrawerView, emptyTerminalDrawerView, type TerminalDrawerView } from './terminal-drawer-view';
// The right panel's tabbed surfaces (lane r4-surfaces; MIT reference, see LICENSE-T3:
// apps/web/src/rightPanelStore.ts, components/RightPanelTabs.tsx, ChatView.tsx
// addFilesSurface / addDeviceSurface / openFileSurface): an ordered set of surface
// tabs per thread with one active, opened from the launcher, the "+" menu, the
// palette and file links. The Diff stays the client's own panel (client.diffOpen);
// this model lists it as a tab and keeps it in step. Every op here is a
// `shelllocal:surface-*` (or `shell:surface-*` for writes) routed from
// shell-commands.ts, so the window's root state only learns `shell.panel.open`.
import { closeSurface, closePanelSurfaces, openDeviceSurface, editTabName, tabRename, copyTabPath, showTabMenu, tabMenuRows, type TabMenuRow } from './right-panel-tabs';
import { selectDeviceTarget } from './r6-media-device';
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { diffRequest, adoptDiff, diffNotGit } from './diff';
import { fileIconToken } from './timeline-files';
import { visiblePullRequests } from './shell-pr';
import { filesView, filesLocal, ensureFile, ensureTree, reconcileFiles, emptyFiles, pendingPaths, forgetFilesSearchFocus, type FilesView } from './r4-surfaces-files';
import { prsView, prsLocal, prsCommand, emptyPrs, type PrsView } from './r4-surfaces-prs';
import { deviceView, deviceLocal, deviceCommand, deviceReady, watchDevice, emptyDevice, deviceStateOf, type DeviceView } from './r4-surfaces-device';
// lane r6-media: opening a device, its workspace and the floating player (r6-media-device.ts).
import { r6DeviceView, r6DeviceMini, r6DeviceLocal, r6DeviceCommand, emptyMini, deviceTab, floatMiniDevice, type R6DeviceMini } from './r6-media-device';
import { threadDevices, visibleMini } from './r12-threads-device'; // lane r12-threads
import { activeSerial, tabStrip, NO_TAB_STRIP, type TabStrip } from './r12-threads-tabs'; // lane r12-threads: the tab strip scrolls
// lane r5-panels: the Pull request (P) surface and sent attachments (r5-panels-surfaces.ts).
import { openThreadPullRequest, r5Local, r5Command, r5Tab, r5Views, r5Prefetch, emptyPrSurface, threadPrTarget, type PrSurfaceView } from './r5-panels-surfaces';
import { emptyAttachment, type AttachmentView, type AttachmentMeta } from './r5-panels-attach';
import { crumbsHidden } from './r10-device-crumbs'; // lane r10-device: the Files preview unmounted
import { restoreRightPanel } from './r10-device-panels'; // lane r10-device: the panel as the last launch left it
import { restoredEffects } from './r11-device-panels'; // lane r11-device: with its Diff and device
import { requestDiff } from './r11-device-diff';
import { deviceTargetOf, deviceThreadId, miniStoreOf, restoreDeviceTarget, type DeviceTarget } from './r6-media-device';
import { ensureDraftThreadId } from './r7-handoff-thread';
import type { PrTarget } from './r5-panels-pr';
import { letGo } from './let-go';
// browser-surface part 1: Browser tabs over the module's WKWebView (browser-surface.ts).
import { annotationSendSerial, browserMiniView, emptyBrowserMini, floatingTabOf, type BrowserMiniView } from './browser-capture'; // browser-surface part 3: the floating player's browser source, an annotation's send
import { browserMiniPlayerSource } from './previewMiniPlayerStore';
import { addBrowserSurface, browserLocal, browserMiniSessions, browserPrepare, browserTab, browserView, emptyBrowserView, installBrowserCleanup, type BrowserView } from './browser-surface';
import { browserTabAudio, browserTabMute } from './browser-automation-tabs'; // browser-surface part 5: Mute / Unmute and the audible indicator

export type SurfaceKind = 'terminal' | 'diff' | 'files' | 'file' | 'pull-requests' | 'device' | 'pull-request' | 'attachment' | 'browser';
export type Surface = { id: string; kind: SurfaceKind; path: string; line: number; reveal: number; pr?: PrTarget; attachment?: AttachmentMeta; device?: DeviceTarget; title?: string; terminal?: PanelTerminal; browser?: { tabId: string; threadKey: string } };
export type PanelState = { surfaces: Surface[]; active: string; visible: boolean; userRevision: number };
export type PanelTab = { id: string; kind: string; title: string; icon: string; tone: string; fileToken: string; active: boolean; pending: boolean; renaming: boolean; renameValue: string; closeTitle: string; closeBody: string; closeTarget: string; menu: TabMenuRow[]; favicon: string; faviconFallback: string; audio: string };
export type PanelView = {
  /** The thread (or draft) this panel belongs to (panelKey), and whether it is open on the launcher alone (rightPanelStore
   *  isOpen with no surface: RightPanelEmptyState). The window keys its own open state by `key` (app.contract rightPanelAt). */
  key: string; launcher: boolean;
  open: boolean; kind: string; active: string; count: number; tabs: PanelTab[]; terminal: TerminalDrawerView; terminalClose: { serial: number; title: string; body: string; target: string; op: string };
  files: FilesView; prs: PrsView; device: DeviceView; deviceSetup: boolean; pr: PrSurfaceView; attachment: AttachmentView; deviceMini: R6DeviceMini; tabStrip: TabStrip; browser: BrowserView;
  /** Part 3: the floating player's browser tab (its frame: chat-canvas-view.ts). */
  browserMini: BrowserMiniView;
  /** Part 3: bumps when an annotation's ⌘Return should send, whatever the panel shows (app.contract annotationSend). */
  annotationSend: number;
};

type Store = { panels: Map<string, PanelState>; deviceSetup: string; terminalClose: { serial: number; title: string; body: string; target: string; op: string } };
const stores = new WeakMap<T3Client, Store>();
export function surfaceStore(client: T3Client): Store {
  let store = stores.get(client);
  if (!store) { store = { panels: new Map(), deviceSetup: '', terminalClose: { serial: 0, title: '', body: '', target: '', op: '' } }; stores.set(client, store); }
  return store;
}
/** The thread (or draft) the panel belongs to: rightPanelStore keys by thread. */
export const panelKey = (client: T3Client): string => { const ref = activeRef(client); return ref ? `${ref.environmentId}:${ref.threadId}` : client.draftKey; };
export function panelState(client: T3Client): PanelState {
  const store = surfaceStore(client), key = panelKey(client);
  let state = store.panels.get(key);
  if (!state && key !== client.draftKey) {
    state = store.panels.get(client.draftKey);
    if (state) { store.panels.delete(client.draftKey); store.panels.set(key, state); }
  }
  if (!state) { state = { surfaces: [], active: '', visible: false, userRevision: 0 }; store.panels.set(key, state); }
  return state;
}

const singleton = (kind: SurfaceKind): Surface => ({ id: kind, kind, path: '', line: 0, reveal: 0 });
const SINGLETON: Record<string, SurfaceKind> = { terminal: 'terminal', t: 'terminal', files: 'files', f: 'files', device: 'device', m: 'device', 'pull-requests': 'pull-requests', l: 'pull-requests', diff: 'diff', d: 'diff' };
export const surfaceKindOf = (value: string): SurfaceKind | null => SINGLETON[value.toLowerCase()] ?? null;

function upsert(state: PanelState, surface: Surface, activate = true): void {
  if (!state.surfaces.some(entry => entry.id === surface.id)) state.surfaces.push(surface);
  if (activate) state.active = surface.id;
  state.visible = true;
}
/** rightPanelStore.closeSurface: the neighbour at the closed index becomes active; no tabs closes the panel. */
export function closeSurfaceIn(state: PanelState, id: string): void {
  closeSurface(state, id);
}
/** rightPanelStore.openFile: "." is the explorer; a file replaces the standalone explorer tab and bumps its reveal. */
export function openFileIn(state: PanelState, requested: string, line: number): Surface {
  if (requested === '.' || requested === '') { const files = singleton('files'); upsert(state, files); return files; }
  const path = /^[A-Za-z]:\/+$/.test(requested) ? requested : requested.replace(/\/+$/, '') || requested;
  const id = `file:${path}`;
  const existing = state.surfaces.find(entry => entry.id === id);
  const surface: Surface = { id, kind: 'file', path, line: line > 0 ? Math.trunc(line) : 0, reveal: (existing?.reveal ?? 0) + 1 };
  const without = state.surfaces.filter(entry => entry.kind !== 'files');
  state.surfaces = existing ? without.map(entry => entry.id === id ? surface : entry) : [...without, surface];
  state.active = id; state.visible = true;
  return surface;
}

/** Keeps the Diff tab in step with the client's diff panel, which other paths open and close. */
export function syncDiff(client: T3Client, state = panelState(client)): void {
  if (client.diffOpen) {
    if (!state.surfaces.some(entry => entry.kind === 'diff')) state.surfaces.push(singleton('diff'));
    state.active = 'diff'; state.visible = true;
  } else if (state.active === 'diff' && state.visible) state.visible = false;
}

export function workspaceOf(client: T3Client): { cwd: string; projectName: string } {
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  return { cwd: str(thread?.worktreePath) || str(project?.workspaceRoot), projectName: str(project?.title) };
}
export function capabilities(client: T3Client): Obj { return obj(obj(obj(client.config).environment).capabilities); }
/** ChatView's availability flags for the launcher and the "+" menu. */
export function availability(client: T3Client) {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  return {
    terminal: terminalAvailable(client),
    files: !!project && !!workspaceOf(client).cwd,
    // ChatView `deviceAvailable={activeThreadRef !== null}`: a draft has its thread ref too (its id is allocated on open).
    device: !!client.environmentId && (!!client.threadId || !!client.projectId),
    pullRequests: !!thread && capabilities(client).threadPullRequests === true && visiblePullRequests(thread.pullRequests).length > 0,
    pullRequest: !!threadPrTarget(client), // r5-panels: supportsPullRequests && threadPullRequestPanelTarget
    diff: !!client.threadId && client.ready && !diffNotGit(client), // ChatView: isServerThread && isGitRepo
  };
}

/** Reopens the Diff from its tab: the client's own request for the remembered scope (client.ts diff()). */
async function showDiff(client: T3Client, native: Native): Promise<void> {
  client.diffOpen = true; client.diffError = '';
  let request;
  try { request = diffRequest(client); } catch (error) { client.diffText = ''; client.diffError = error instanceof Error ? error.message : String(error); return; }
  if (request.scope !== client.diffState.scopeKey) client.diffText = '';
  const threadId = client.threadId;
  client.diffLoading = true;
  try {
    const result = await requestDiff(client.config, request, (method, payload) => client.restAccess(native).request(method, payload)); // r11-device-diff.ts
    if (threadId === client.threadId && client.diffOpen) client.diffText = adoptDiff(client, request, result);
  } catch (error) { if (threadId === client.threadId && !letGo(error)) client.diffError = error instanceof Error ? error.message : String(error); }
  finally { if (threadId === client.threadId) client.diffLoading = false; }
}

/** Opens one surface kind as the reference's onAdd* handlers do; unavailable kinds are ignored. */
export async function openSurface(client: T3Client, native: Native, value: string): Promise<string> {
  const kind = surfaceKindOf(value), state = panelState(client), can = availability(client);
  if (openThreadPullRequest(client, state, value) !== null) return ''; // r5-panels: Pull request (P)
  if (value === 'browser' || value === 'b' || value === 'B') return addBrowserSurface(client, native, state); // browser-surface: Browser (B), the default profile
  if (!kind || kind === 'file') return '';
  if (kind === 'terminal') { await addTerminalSurface(client, native); return ''; }
  if (kind === 'diff') { if (!can.diff) return ''; upsert(state, singleton('diff')); await showDiff(client, native); return ''; }
  if (kind === 'files') { if (!can.files) return ''; client.diffOpen = false; upsert(state, singleton('files')); await ensureTree(client, native); return ''; }
  if (kind === 'pull-requests') { if (!can.pullRequests) return ''; client.diffOpen = false; upsert(state, singleton('pull-requests')); return ''; }
  if (!can.device) return '';
  // addDeviceSurface on a draft: the reference's draft has its thread id from the start; here it is allocated now (as Browser does).
  if (!client.threadId && !activeRef(client)) await ensureDraftThreadId(client, native);
  await watchDevice(client, native);
  // addDeviceSurface: before onboarding (or with the hub off) the setup wizard opens instead of a tab.
  if (!deviceReady(client)) { surfaceStore(client).deviceSetup = panelKey(client); return ''; }
  client.diffOpen = false; upsert(state, singleton('device'));
  return '';
}
/** A chat link's target (resolvePathLinkTarget's `/root/a.ts:3`) as the workspace-relative path openFile takes, and its line. */
export function workspacePath(target: string, cwd: string, line = 0): { path: string; line: number } {
  let path = target.trim(), at = line;
  const suffix = /^(.*?):(\d+)(?::\d+)?$/.exec(path);
  if (suffix && !/^[A-Za-z]$/.test(suffix[1]!)) { path = suffix[1]!; at = at || Number(suffix[2]); }
  const root = cwd.replace(/\/+$/, '');
  if (root && (path === root || path.startsWith(`${root}/`))) path = path.slice(root.length + 1) || '.';
  return { path, line: at };
}
/** openFileSurface (palette file picker, content search, file links): needs a project workspace. */
export async function openFileSurface(client: T3Client, native: Native, requested: string, requestedLine: number, fromTree = false): Promise<void> {
  if (!availability(client).files) throw new ClientError('Open a project to open its files.');
  const { path, line } = workspacePath(requested, workspaceOf(client).cwd, requestedLine);
  client.diffOpen = false;
  const surface = openFileIn(panelState(client), path, line);
  if (surface.kind === 'file') await ensureFile(client, native, surface.path, true, fromTree);
  else await ensureTree(client, native);
}

/** `shelllocal:surface-*`: tab and panel state, the explorer and every read. */
export async function surfaceLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const targetSplit = op === 'close-confirmed' ? id.lastIndexOf('|') : -1;
  const targetKey = targetSplit >= 0 ? id.slice(0, targetSplit) : panelKey(client);
  if (targetSplit >= 0) id = id.slice(targetSplit + 1);
  const state = targetKey === panelKey(client) ? panelState(client) : surfaceStore(client).panels.get(targetKey);
  if (!state) return '';
  if (targetKey === panelKey(client)) syncDiff(client, state);
  state.userRevision++;
  installTerminalPanelCleanup(client, native, state, parseScopedThreadKey(targetKey)); installBrowserCleanup(client, native);
  if (op.startsWith('terminal-')) return terminalPanelLocal(client, native, op.slice(9), id, value);
  if (op === 'open') return openSurface(client, native, id || value);
  if (op === 'file') { await openFileSurface(client, native, id, Number(value) || 0, value === 'tree'); return ''; }
  if (op === 'toggle-mute') return browserLocal(client, native, state, 'toggle-mute', id.replace(/^browser:/, ''), ''); // browser-surface part 5
  if (op === 'menu') { const action = await showTabMenu(client, native, state, id, value === 'key', surface => browserTabMute(client, surface)); return action && panelState(client) === state ? surfaceLocal(client, native, action, id, '') : ''; }
  if (op.startsWith('rename')) { editTabName(state, op, id, value); return ''; }
  if (op === 'copy-path') { const surface = state.surfaces.find(entry => entry.id === id); if (surface) await copyTabPath(client, native, surface); return ''; }
  if (op === 'activate') {
    const surface = state.surfaces.find(entry => entry.id === id);
    if (!surface) return '';
    state.active = id; state.visible = true;
    if (surface.kind === 'terminal') focusPanelTerminal(client);
    if (surface.kind === 'device') selectDeviceTarget(client, panelKey(client), surface.device);
    if (surface.kind === 'diff') await showDiff(client, native);
    else { client.diffOpen = false; if (surface.kind === 'file') await ensureFile(client, native, surface.path, false); if (surface.kind === 'files') await ensureTree(client, native); }
    return '';
  }
  if (op === 'close') {
    const surface = state.surfaces.find(entry => entry.id === id && entry.kind === 'terminal');
    if (surface) {
      const copy = terminalSurfaceCloseCopy(client, surface), store = surfaceStore(client);
      store.terminalClose = { serial: store.terminalClose.serial + 1, title: copy.closeTitle, body: copy.closeBody, op: 'shelllocal:surface-close-confirmed', target: `${panelKey(client)}|${id}` };
      return '';
    }
  }
  if (op === 'close-confirmed') op = 'close';
  if (['close', 'close-others', 'close-to-right', 'close-all'].includes(op)) {
    const hadTerminal = state.surfaces.some(entry => entry.kind === 'terminal');
    await closePanelSurfaces(client, state, op, id);
    if (panelState(client) !== state) return '';
    if (!state.surfaces.some(entry => entry.kind === 'diff')) client.diffOpen = false;
    const next = state.surfaces.find(entry => entry.id === state.active);
    if (next?.kind === 'diff' && state.visible && !client.diffOpen) await showDiff(client, native);
    else if (next && next.kind !== 'diff') client.diffOpen = false;
    if (next?.kind === 'device') selectDeviceTarget(client, panelKey(client), next.device);
    if (next?.kind === 'terminal') focusPanelTerminal(client);
    return hadTerminal && !state.surfaces.length ? 'terminal:focus-composer' : '';
  }
  if (op === 'hide') {
    // closePreviewPanel (ChatView.tsx): closing the whole panel on a live device floats it instead of dropping it.
    const active = state.visible ? state.surfaces.find(entry => entry.id === state.active) : undefined, target = active?.kind === 'device' ? deviceTargetOf(client, panelKey(client)) : undefined;
    const thread = deviceThreadId(client);
    if (target && thread) floatMiniDevice(client, thread, target);
    // Part 3: a live Browser tab floats too (closePreviewPanel's browser half).
    if (active?.kind === 'browser' && active.browser && thread && floatingTabOf(client) !== active.browser.tabId) miniStoreOf(client).open(thread, browserMiniPlayerSource(active.browser.tabId));
    forgetFilesSearchFocus(client); // right-panel-escape
    state.visible = false; client.diffOpen = false; client.diffLoading = false; return '';
  }
  if (op === 'show') {
    // rightPanelStore.show: the thread's panel opens, on its launcher when it has no surface (audit-wave-followups FU-2).
    forgetFilesSearchFocus(client);
    state.visible = true;
    const active = state.surfaces.find(entry => entry.id === state.active);
    if (state.visible && active?.kind === 'diff') await showDiff(client, native);
    return '';
  }
  if (op === 'setup-close') { surfaceStore(client).deviceSetup = ''; return ''; }
  if (op.startsWith('r5-')) return r5Local(client, native, state, op.slice(3), id, value); // r5-panels-surfaces.ts
  if (op.startsWith('browser-')) return browserLocal(client, native, state, op.slice(8), id, value); // browser-surface.ts
  // right-panel-escape: a sheet's Files search Escape clears the search, then the dialog closes (app.contract chatLocal).
  if (op === 'files-search-key' && id === 'sheet' && value === 'Escape') { await filesLocal(client, native, 'search-key', id, value); return surfaceLocal(client, native, 'hide', '', ''); }
  if (op.startsWith('files-')) return filesLocal(client, native, op.slice(6), id, value);
  if (op.startsWith('pr-')) return prsLocal(client, native, op.slice(3), id, value);
  if (op.startsWith('device-')) return deviceLocal(client, native, op.slice(7), id, value);
  if (op.startsWith('r6dev-')) {
    const key = panelKey(client), closingId = state.active;
    const effect = await r6DeviceLocal(client, native, deviceStateOf(client), key, op.slice(6), value);
    if (effect === 'close') closeSurfaceIn(state, closingId);
    if (panelState(client) !== state) return '';
    if (effect === 'hide') { state.visible = false; client.diffOpen = false; }
    if (effect === 'reopen') { client.diffOpen = false; const target = deviceTargetOf(client, key); if (target) openDeviceSurface(state, target); }
    return '';
  }
  throw new ClientError(`Unknown surface action: ${op}`);
}
/** `shell:surface-*`: the writes (file saves, pull-request watch/unlink, device hub settings). */
export async function surfaceCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op.startsWith('pr-')) return prsCommand(client, native, storage, op.slice(3), id, value);
  if (op.startsWith('r5-')) return r5Command(client, native, storage, op.slice(3), id); // r5-panels-surfaces.ts
  if (op.startsWith('r6dev-')) {
    const state = panelState(client), key = panelKey(client), closingId = state.active;
    if (await r6DeviceCommand(client, native, deviceStateOf(client), key, op.slice(6), id, value)) closeSurfaceIn(state, closingId);
    if (op === 'r6dev-open') { const target = deviceTargetOf(client, key); if (target) openDeviceSurface(state, target); }
    return '';
  }
  if (op.startsWith('device-')) {
    const result = await deviceCommand(client, native, op.slice(7), id, value);
    // DeviceSetup onComplete: open the Device surface once onboarding is saved.
    if (op === 'device-complete' && deviceReady(client)) { surfaceStore(client).deviceSetup = ''; client.diffOpen = false; upsert(panelState(client), singleton('device')); }
    return result;
  }
  throw new ClientError(`Unknown surface action: ${op}`);
}

function tabOf(client: T3Client, surface: Surface, active: string, pending: ReadonlySet<string>): PanelTab {
  const state = panelState(client), editor = tabRename(state);
  const rename = { closeTarget: `${panelKey(client)}|${surface.id}`, ...terminalSurfaceCloseCopy(client, surface), renaming: editor.id === surface.id, renameValue: editor.id === surface.id ? editor.value : '', menu: tabMenuRows(surface, state.surfaces, entry => browserTabMute(client, entry)), audio: '' };
  const r5 = r5Tab(client, surface);
  if (r5) return { id: surface.id, kind: surface.kind, ...r5, ...rename, active: surface.id === active, pending: false, favicon: '', faviconFallback: '' };
  if (surface.kind === 'browser') { const tab = browserTab(client, surface); return { id: surface.id, kind: surface.kind, ...rename, title: tab.title, icon: 'earth', tone: '', fileToken: '', active: surface.id === active, pending: false, favicon: tab.favicon, faviconFallback: tab.faviconFallback, audio: browserTabAudio(client, surface) }; }
  const name = surface.path.slice(Math.max(surface.path.lastIndexOf('/'), surface.path.lastIndexOf('\\')) + 1);
  const device = surface.kind === 'device' ? deviceTab(client, panelKey(client)) : null; // lane r7-device: the open device's name and mark
  const title = surface.kind === 'terminal' ? panelTerminalLabel(client, surface.terminal?.activeTerminalId ?? '') : surface.kind === 'diff' ? 'Diff' : surface.kind === 'files' ? 'Files' : surface.kind === 'file' ? name : surface.kind === 'pull-requests' ? 'Pull requests' : surface.title || surface.device?.name || device?.title || 'Device';
  const icon = surface.kind === 'terminal' ? 'square-terminal' : surface.kind === 'diff' ? 'file-diff' : surface.kind === 'files' ? 'files' : surface.kind === 'pull-requests' ? 'link-2' : surface.kind === 'device' ? (surface.device ? surface.device.platform === 'android' ? 'android' : 'apple' : 'smartphone') : '';
  return { id: surface.id, kind: surface.kind, ...rename, title, icon, tone: '', fileToken: surface.kind === 'file' ? fileIconToken(surface.path) : '', active: surface.id === active, pending: pending.has(surface.path), favicon: '', faviconFallback: '' };
}

/** The panel's projection for ShellView: tabs, the active surface's body and the device wizard. */
export async function panelView(client: T3Client, native: Native | null | undefined, now: number, sheet = false): Promise<PanelView> {
  const state = panelState(client), store = surfaceStore(client);
  await threadDevices(client, native, sheet); // lane r12-threads: a thread's device session floats (ChatView autoShowFloatingPreview)
  if (restoreRightPanel(client, panelKey(client), state)) { // lane r11-device: the device it showed and its open Diff
    const effects = restoredEffects(state.surfaces, state.active, state.visible);
    if (effects.device) restoreDeviceTarget(client, panelKey(client), effects.device);
    if (effects.diff && native?.available) await showDiff(client, native);
  }
  const deviceSurface = state.surfaces.find(entry => entry.id === state.active && entry.kind === 'device');
  if (deviceSurface) selectDeviceTarget(client, panelKey(client), deviceSurface.device);
  syncDiff(client, state);
  if (!availability(client).files) reconcileFiles(state);
  await browserPrepare(client, native, state); // browser-surface: the thread's sessions, the native web views, status reports
  const active = state.surfaces.find(entry => entry.id === state.active) ?? null;
  const open = state.visible && !!active && active.kind !== 'diff';
  const live = native?.available ? native : null;
  const files = live && open && (active.kind === 'files' || active.kind === 'file') ? await filesView(client, live, active, now) : (crumbsHidden(client), emptyFiles());
  const deviceSetup = store.deviceSetup !== '' && store.deviceSetup === panelKey(client);
  const device = live && (deviceSetup || (open && active.kind === 'device')) ? await deviceView(client, live) : emptyDevice();
  if (live && open && active.kind === 'device' && !deviceSetup) device.r6 = await r6DeviceView(client, live, deviceStateOf(client) ?? {}, device.loaded, panelKey(client));
  const prs = open && active.kind === 'pull-requests' ? prsView(client, now) : emptyPrs();
  const r5 = await r5Views(client, live, active, open, now);
  await r5Prefetch(client, live, state.surfaces, now); // lane r6-pr: tab icons from loaded detail
  return {
    terminalClose: store.terminalClose,
    terminal: active?.kind === 'terminal' ? terminalPanelView(client, await terminalDrawerView(client, native, 0, now), active) : emptyTerminalDrawerView(),
    key: panelKey(client), launcher: state.visible && !active, open, kind: active?.kind ?? '', active: active?.id ?? '', count: state.surfaces.length,
    tabs: state.surfaces.map(surface => tabOf(client, surface, state.active, pendingPaths(client))), files, prs, device, deviceSetup, ...r5,
    deviceMini: visibleMini(r6DeviceMini(client, deviceStateOf(client)), shownDevice(client)), // r12-threads: shouldRenderPreviewMiniPlayer (its frame: chat-canvas-view.ts)
    tabStrip: tabStrip(obj(client.presentation), state.surfaces.map(surface => surface.id), active?.id ?? '', activeSerial(client, panelKey(client), active?.id ?? '')),
    browser: open && active.kind === 'browser' ? browserView(client, active, now) : emptyBrowserView(client), // part 4: the client's profiles for the "+" menu and the launcher
    browserMini: browserMiniView(client, ref => browserMiniSessions(client, ref), shownBrowserTab(client)), // part 3: shouldRenderPreviewMiniPlayer's browser half
    annotationSend: annotationSendSerial(client), // part 3: after browserPrepare applied the picks
  };
}
/** The Browser tab the rendered right panel shows, if any (shouldRenderPreviewMiniPlayer). */
export function shownBrowserTab(client: T3Client): string | null {
  const state = panelState(client), active = state.surfaces.find(entry => entry.id === state.active);
  return state.visible && active?.kind === 'browser' && active.browser ? active.browser.tabId : null;
}
/** The device the rendered right panel shows (shouldRenderPreviewMiniPlayer's renderedRightPanelSurface), if any. */
export function shownDevice(client: T3Client): DeviceTarget | undefined {
  const state = panelState(client), active = state.surfaces.find(entry => entry.id === state.active);
  return state.visible && active?.kind === 'device' ? deviceTargetOf(client, panelKey(client)) : undefined;
}
export const closedPanel = (): PanelView => ({ key: '', launcher: false, terminalClose: { serial: 0, title: '', body: '', target: '', op: '' }, terminal: emptyTerminalDrawerView(), open: false, kind: '', active: '', count: 0, tabs: [], files: emptyFiles(), prs: emptyPrs(), device: emptyDevice(), deviceSetup: false, pr: emptyPrSurface(), attachment: emptyAttachment(), deviceMini: emptyMini(), tabStrip: NO_TAB_STRIP, browser: emptyBrowserView(), browserMini: emptyBrowserMini(), annotationSend: 0 });
