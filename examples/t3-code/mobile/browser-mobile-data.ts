// @ref llp/1109.010-mobile-browser-devices.decision.md#connection-and-command-ownership
// Pinned365aa87982 BrowserPreviewRouteScreen / BrowserTabMenu / state/preview.
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, num, type Obj } from './shared/domain';
import { ClientError, bridgeReply, type Native } from './shared/protocol';
import { subscriptionSerial } from './shared/shell-vcs';
import { letGo } from './shared/let-go';
import { mobileStreamOwner, mobileStreamNative, mobileStreamPermission, mobileStreamDescriptor, assertMobileStreamOwner } from './browser-mobile-owner';
import { applyBrowserEvent, applyBrowserList, emptyBrowserTabs, latestBrowserTab, browserTabTitle, browserTabUrl, browserZoomLevels, normalizeMobilePreviewUrl, type BrowserTabs } from './browser-mobile-model';

export interface MobilePreviewMenu { id: string; title: string; subtitle: string; symbol: string; selected: boolean; disabled: boolean; destructive: boolean; group: string }
export interface MobileBrowserSnapshot { owner: string; revision: number; title: string; url: string; tabs: MobilePreviewMenu[]; menu: MobilePreviewMenu[];
  count: number; loaded: boolean; loading: boolean; error: string; close: boolean; source: string; status: string; detail: string;
  canBack: boolean; canForward: boolean; canNavigate: boolean; canAdjust: boolean; controlLabel: string; controlAction: string;
  canControl: boolean; pipSupported: boolean; pipActive: boolean; hostSetupCommand: string; canReconnect: boolean; retryLabel: string; needsRefresh: boolean }
interface State { owner: string; tabs: BrowserTabs; events: Obj[]; listRevision: number; listEpoch: string; subscription: string;
  selected: string; error: string; loading: boolean; allowed: boolean; operate: boolean; refresh: boolean; native: Obj; serial: number; lastSeq: number; invalidation: number }
const states = new WeakMap<T3Client, State>();
export const BROWSER_EVENTS = 'mobile-browser-tabs';
function stateOf(client: T3Client): State {
  const owner = mobileStreamOwner(client); let state = states.get(client);
  if (!state || state.owner !== owner) { state = { owner, tabs: emptyBrowserTabs(), events: [], listRevision: 0, listEpoch: '', subscription: '', selected: '',
    error: '', loading: false, allowed: false, operate: false, refresh: false, native: {}, serial: 0, lastSeq: 0, invalidation: 0 }; states.set(client, state); }
  return state;
}
/** Parent calls this before the shared client's events batch is acknowledged. */
export function mobileBrowserEvents(entries: unknown, client: T3Client = mobileClient) {
  const state = states.get(client); if (!state || state.owner !== mobileStreamOwner(client)) return;
  for (const entry of arr(entries)) {
    if (num(entry.generation, -1) !== client.generation || entry.key !== BROWSER_EVENTS || (!state.subscription && !state.loading) || (state.subscription && subscriptionSerial(str(entry.subscriptionId)) < subscriptionSerial(state.subscription))) continue;
    const seq = num(entry.seq); if (seq > 0 && seq <= state.lastSeq) continue;
    state.lastSeq = Math.max(state.lastSeq, seq);
    const event = obj(entry.value);
    state.subscription = str(entry.subscriptionId);
    if (event._retryDue) { state.subscription = ''; state.refresh = true; state.invalidation++; continue; }
    if (event._streamEnded || event._transportError) { state.error = str(obj(event._transportError).message); continue; }
    if (event.threadId !== client.threadId) continue;
    if (state.listEpoch === event.serverEpoch && num(event.revision) <= state.listRevision) continue;
    state.events = [...state.events.slice(-199), event];
    if (state.tabs.serverEpoch !== null && event.serverEpoch !== state.tabs.serverEpoch) { state.refresh = true; state.invalidation++; }
    else state.tabs = applyBrowserEvent(state.tabs, event);
  }
}
export const previewMenuItem = (id: string, title: string, symbol = '', selected = false, disabled = false, group = ''): MobilePreviewMenu =>
  ({ id, title, subtitle: '', symbol, selected, disabled, destructive: false, group });
function menus(tab: Obj, disabled: boolean): MobilePreviewMenu[] {
  const zoom = num(tab.zoomFactor, 1), viewport = obj(tab.viewport);
  return [previewMenuItem('hard-reload', 'Hard reload', 'arrow.clockwise.circle', false, disabled),
    ...['system', 'light', 'dark'].map(value => previewMenuItem(`appearance:${value}`, value[0]!.toUpperCase() + value.slice(1), '', (tab.colorScheme || 'system') === value, disabled, 'Appearance')),
    ...[['in', 'Zoom in', 'plus.magnifyingglass'], ['out', 'Zoom out', 'minus.magnifyingglass'], ['reset', 'Actual size', '1.magnifyingglass']].map(([id, title, symbol]) => previewMenuItem(`zoom:${id}`, title!, symbol, false, disabled, `Zoom (${Math.round(zoom * 100)}%)`)),
    ...VIEWPORTS.map(row => previewMenuItem(`viewport:${row.id}`, row.title, '', (viewport._tag === 'preset' ? `preset:${viewport.presetId}` : viewport._tag === 'freeform' ? 'custom' : 'fill') === row.id, disabled, 'Viewport')),
    ...['cookies', 'cache'].map(value => ({ ...previewMenuItem(`clear:${value}`, `Clear ${value}`, 'trash', false, disabled, 'Site data'), destructive: true }))];
}
// First source preset in each Phone/Tablet/Desktop category (previewViewport.ts).
const VIEWPORTS = [{ id: 'fill', title: 'Fit to screen', setting: { _tag: 'fill' } },
  { id: 'preset:iphone-se', title: 'Phone (iPhone SE)', setting: { _tag: 'preset', presetId: 'iphone-se', width: 375, height: 667 } },
  { id: 'preset:ipad-mini', title: 'Tablet (iPad Mini)', setting: { _tag: 'preset', presetId: 'ipad-mini', width: 768, height: 1024 } }];
export function mobileBrowserSnapshot(owner = mobileStreamOwner(mobileClient), client: T3Client = mobileClient): MobileBrowserSnapshot {
  const state = stateOf(client), valid = owner === state.owner && !!owner, tabs = valid && state.allowed ? state.tabs.sessions.filter(tab => tab.runtime === 'server') : [];
  const tab = tabs.find(tab => tab.tabId === state.selected) || latestBrowserTab(tabs);
  if (tab && state.selected !== str(tab.tabId)) { state.selected = str(tab.tabId); state.native = {}; } // Source pins the fallback once, so later activity cannot jump selection.
  const native = valid ? state.native : {}, control = obj(native.control), streaming = native.status === 'streaming';
  const controller = str(control.controller), canNavigate = streaming && controller === 'you' && state.operate;
  const controlLabel = !streaming ? 'Connecting...' : control.canOperate !== true ? 'Read-only' : controller === 'you' ? 'You have control' : controller === 'agent' ? 'Agent has control' : controller === 'another-viewer' ? 'Another viewer has control' : 'Watching';
  return { owner, revision: client.revision, title: tab ? browserTabTitle(tab) : 'Browser', url: tab ? browserTabUrl(tab) : '',
    tabs: tabs.map(tab => ({ ...previewMenuItem(str(tab.tabId), browserTabTitle(tab), '', tab.tabId === state.selected), subtitle: browserTabUrl(tab) })),
    menu: tab ? menus(tab, !streaming || !state.operate) : [], count: tabs.length, loaded: valid && state.tabs.listed,
    loading: valid && state.loading, error: valid ? state.error : '', close: valid && state.tabs.listed && tabs.length === 0,
    source: tab ? JSON.stringify({ ...mobileStreamDescriptor(client, owner), tabId: tab.tabId, operate: state.operate }) : '',
    status: str(native.status, 'connecting'), detail: str(native.detail), canBack: canNavigate && tab?.canGoBack === true,
    canForward: canNavigate && tab?.canGoForward === true, canNavigate, canAdjust: streaming && state.operate, controlLabel,
    controlAction: controller === 'you' ? 'Release control' : 'Take control', canControl: streaming && state.operate && control.canOperate === true && controller !== 'another-viewer',
    hostSetupCommand: str(native.hostSetupCommand), canReconnect: native.gone !== true, retryLabel: native.hostSetupCommand ? 'Try again' : 'Reconnect',
    pipSupported: native.pipSupported === true, pipActive: native.pipActive === true, needsRefresh: state.refresh };
}
export async function mobileBrowserRead(owner: string, nativeInput: Native, client: T3Client = mobileClient) {
  const state = stateOf(client), serial = ++state.serial, invalidation = state.invalidation; state.loading = true;
  try {
    const native = mobileStreamNative(client, owner, nativeInput, () => state.serial === serial);
    state.operate = await mobileStreamPermission(client, native, 'preview:operate'); state.allowed = true;
    if (!state.subscription) {
      const reply = await client.restAccess(native).call({ op: 'subscribe', key: BROWSER_EVENTS, method: 'subscribePreviewEvents', payload: {} });
      state.subscription = str(reply.id);
    }
    const result = obj(await client.restAccess(native).request('preview.list', { threadId: client.threadId }));
    if (serial !== state.serial) return mobileBrowserSnapshot(owner, client);
    if (state.listEpoch !== result.serverEpoch || num(result.revision) >= state.listRevision) {
      state.tabs = applyBrowserList(result, state.events); state.listEpoch = str(result.serverEpoch); state.listRevision = num(result.revision);
      state.events = state.events.filter(event => event.serverEpoch !== state.listEpoch || num(event.revision) > state.listRevision);
    }
    state.error = ''; state.refresh = state.invalidation !== invalidation;
  } catch (error) { if (letGo(error)) throw error; if (error instanceof ClientError && error.kind === 'permission') state.allowed = false; if (states.get(client) === state) state.error = error instanceof Error ? error.message : 'Browser unavailable'; }
  finally { if (serial === state.serial) { state.loading = false; state.refresh = state.invalidation !== invalidation; } client.revision++; }
  return mobileBrowserSnapshot(owner, client);
}
export async function mobileBrowserStatus(owner: string, nativeInput: Native, client: T3Client = mobileClient) {
  const state = stateOf(client); mobileBrowserSnapshot(owner, client); const selected = state.selected;
  const native = mobileStreamNative(client, owner, nativeInput); native.watch('t3.mobile-browser');
  const response = await bridgeReply(native, { op: 'mobileBrowser', action: 'status', owner, generation: client.generation });
  if (response.ok && states.get(client) === state && state.selected === selected && str(obj(response.value).tabId) === selected) state.native = obj(response.value);
  return mobileBrowserSnapshot(owner, client);
}
export async function mobileBrowserAction(owner: string, op: string, arg: string, nativeInput: Native, client: T3Client = mobileClient) {
  let message = '';
  try {
    const native = mobileStreamNative(client, owner, nativeInput), state = stateOf(client);
    assertMobileStreamOwner(client, owner);
    const snapshot = mobileBrowserSnapshot(owner, client), tab = state.tabs.sessions.find(tab => tab.tabId === state.selected);
    if (op === 'select') { if (!snapshot.tabs.some(tab => tab.id === arg)) throw new ClientError('This tab was closed.'); state.selected = arg; state.native = {}; }
    else if (op === 'refresh') return { message: '', data: await mobileBrowserRead(owner, nativeInput, client) };
    else if (['back', 'forward', 'reload', 'navigate', 'control', 'pip', 'reconnect', 'copy-setup'].includes(op)) {
      if (['back', 'forward', 'reload', 'navigate'].includes(op) && !snapshot.canNavigate || op === 'control' && !snapshot.canControl) throw new ClientError('This browser is not under your control.');
      const input = op === 'back' || op === 'forward' ? { type: 'history', delta: op === 'back' ? -1 : 1 } : op === 'navigate' ? { type: 'navigate', url: normalizeMobilePreviewUrl(arg) } : op === 'control' ? { type: state.native.control && obj(state.native.control).controller === 'you' ? 'releaseControl' : 'takeControl' } : { type: op };
      const result = await bridgeReply(native, { op: 'mobileBrowser', action: ['pip', 'reconnect', 'copy-setup'].includes(op) ? op : 'command', owner, input, generation: client.generation });
      if (!result.ok) throw new ClientError(result.error!.message);
    } else {
      if (!tab || !snapshot.canAdjust || !snapshot.menu.some(item => item.id === op)) throw new ClientError('This tab option is unavailable.');
      if (!await mobileStreamPermission(client, native, 'preview:operate')) throw new ClientError('This connection cannot operate browser previews.');
      const target = { threadId: client.threadId, tabId: tab.tabId }, change: Obj = {};
      if (op.startsWith('viewport:')) { const row = VIEWPORTS.find(row => `viewport:${row.id}` === op)!; await client.restAccess(native).request('preview.resize', { ...target, viewport: row.setting }, true); }
      else {
        if (op === 'hard-reload') change.hardReload = true;
        else if (op.startsWith('appearance:')) change.colorScheme = op.slice(11);
        else if (op.startsWith('clear:')) change.clear = op.slice(6);
        else if (op.startsWith('zoom:')) { const index = browserZoomLevels.indexOf(num(tab.zoomFactor, 1)); change.zoomFactor = op === 'zoom:reset' ? 1 : browserZoomLevels[Math.min(Math.max((index < 0 ? 7 : index) + (op === 'zoom:in' ? 1 : -1), 0), browserZoomLevels.length - 1)]; }
        await client.restAccess(native).request('preview.adjust', { ...target, ...change }, true);
      }
    }
  } catch (error) { if (letGo(error)) throw error; message = error instanceof Error ? error.message : 'Could not change this browser tab'; }
  client.revision++; return { message, data: mobileBrowserSnapshot(owner, client) };
}
export async function mobileBrowserRelease(owner: string, native: Native, client: T3Client = mobileClient) {
  const state = states.get(client); if (!state || state.owner !== owner || owner !== mobileStreamOwner(client)) return;
  state.serial++; state.subscription = ''; state.allowed = false;
  await client.restAccess(mobileStreamNative(client, owner, native)).call({ op: 'unsubscribe', key: BROWSER_EVENTS });
}
