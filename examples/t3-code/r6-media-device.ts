// Lane r6-media: opening a simulator or emulator from the Device surface and the device
// workspace (MIT reference, see LICENSE-T3: components/device/DevicePanel.tsx selectDevice /
// closeActive / floatActive, DeviceLoadingView.tsx, DeviceWorkspace.tsx, DeviceControlsRail.tsx,
// useDeviceControls.ts, preview/ThreadPreviewMiniPlayer.tsx DeviceMiniPlayer,
// previewMiniPlayerStore.ts). The server's device RPCs are `device.open` (boots a stopped device
// unless `boot: false`), `device.close` (`shutdown` powers it off), `device.detail` and
// `device.action`; the picture and touches come from the hub through the server's
// `/api/device-hub` proxy, which the app module's `t3-media` hook reads (R6DeviceStream.swift:
// R7DeviceClient.swift: iOS's AVCC / MJPEG feed and helper socket, Android's H.264 socket). Lane
// r7-device adds the 3D phone, the Flat / 3D toggle, Android's Back / Recents / Rotate and the
// Tools drawer (r7-device-tools.ts).
import type { T3Client } from './client';
import { emptyR7Workspace, r7Workspace, r7DeviceCommand, r7DeviceLocal, type R7DevDeviceWorkspace } from './r7-device-tools';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { emptyR9Folding, r9Folding, r9FoldingValue, type R9DevFolding } from './r9-device-duo'; // lane r9-device
import { PreviewMiniPlayerStore, previewMiniPlayerSourceKey, type PreviewMiniPlayerSource } from './previewMiniPlayerStore'; // the floating player's per-thread state
import { letGo } from './let-go';

export type DeviceTarget = { hostId: string; deviceId: string; platform: string; name: string };
export type R6DeviceRail = {
  inputConnected: boolean; appearance: string; appearanceLabel: string; appearanceIcon: string; appearanceDisabled: boolean;
  textSize: string; textSizeDisabled: boolean; screenshotLabel: string; screenshotDisabled: boolean; menu: string; menuTop: number; menuRight: number;
};
export type R6DeviceStream = { status: string; message: string; error: boolean; inputNotice: string };
export type R6DeviceWorkspace = {
  key: string; name: string; description: string; platform: string; deviceId: string; hostId: string; screenLabel: string;
  alert: string; stream: R6DeviceStream; rail: R6DeviceRail; r7: R7DevDeviceWorkspace; r9: R9DevFolding;
};
export type R6DeviceView = {
  /** "list" (DevicePanel's picker), "opening" (DeviceLoadingView) or "workspace". */
  mode: string; pendingKey: string; openingName: string; openingDescription: string; openingMessage: string;
  operationError: string; booting: string; hostDetail: string; workspace: R6DeviceWorkspace;
};
/** The floating player's source and stream; its frame comes from the chat canvas (chat-canvas-view.ts). */
export type R6DeviceMini = { show: boolean; key: string; sourceKey: string; name: string; description: string; platform: string; deviceId: string; hostId: string; stream: R6DeviceStream };

type Store = {
  generation: number; pending: (DeviceTarget & { booted: boolean; version: string }) | null; operationError: string;
  targets: Map<string, DeviceTarget>; mini: PreviewMiniPlayerStore;
  detail: Obj | null; detailKey: string; detailError: string; acting: boolean; screenshotPending: boolean; screenshotError: string; menu: string;
};
const stores = new WeakMap<T3Client, Store>();
function storeOf(client: T3Client): Store {
  let store = stores.get(client);
  if (!store) {
    store = { generation: client.generation, pending: null, operationError: '', targets: new Map(), mini: new PreviewMiniPlayerStore(), detail: null, detailKey: '', detailError: '', acting: false, screenshotPending: false, screenshotError: '', menu: '' };
    stores.set(client, store);
  }
  if (store.generation !== client.generation) { store.generation = client.generation; store.detail = null; store.detailKey = ''; store.pending = null; }
  return store;
}

/** RightPanelTabs: a device tab takes its device's name and platform mark once one is open (lane r7-device). */
export function deviceTab(client: T3Client, panelKey: string): { title: string; icon: string } | null {
  const target = storeOf(client).targets.get(panelKey);
  return target ? { title: target.name || 'Device', icon: target.platform === 'android' ? 'android' : 'apple' } : null;
}

/** Lane r11-device: the device a thread's Device surface shows (rightPanelStore's surface `target`), kept across launches. */
export const deviceTargetOf = (client: T3Client, panelKey: string): DeviceTarget | undefined => storeOf(client).targets.get(panelKey);
export function restoreDeviceTarget(client: T3Client, panelKey: string, target: DeviceTarget): void {
  const store = storeOf(client);
  if (!store.targets.has(panelKey)) store.targets.set(panelKey, target);
}

export function selectDeviceTarget(client: T3Client, panelKey: string, target?: DeviceTarget): void {
  if (target) storeOf(client).targets.set(panelKey, target); else storeOf(client).targets.delete(panelKey);
}

/** DevicePanel deviceKey. */
export const deviceKey = (device: { hostId: string; id?: string; deviceId?: string }) => `${device.hostId}\u0000${device.id ?? device.deviceId ?? ''}`;
const message = (error: unknown, fallback: string) => (error instanceof Error && error.message ? error.message : fallback);
const hostLabel = (state: Obj, hostId: string) => str(arr(state.hosts).find(host => str(host.id) === hostId)?.label, 'Device host');
const platformOf = (value: string) => (value === 'android' ? 'android' : 'ios');

/** state.sessions for this thread and the surface's target: the device the workspace shows. */
export function activeDevice(state: Obj, threadId: string, target: DeviceTarget | undefined): Obj | null {
  if (!target || !threadId) return null;
  const session = arr(state.sessions).find(entry => str(entry.threadId) === threadId && str(entry.deviceId) === target.deviceId && str(entry.hostId) === target.hostId);
  if (!session) return null;
  return arr(state.devices).find(device => str(device.hostId) === str(session.hostId) && str(device.id) === str(session.deviceId)) ?? null;
}

/** The module's report for the stream it draws (`presentation.deviceStreams[key]`). */
export function streamView(client: T3Client, key: string, platform: string): R6DeviceStream {
  const report = obj(obj(client.presentation.deviceStreams)[key]);
  const status = str(report.status, 'connecting'), detail = str(report.detail);
  const inputConnected = report.inputConnected === true;
  return {
    status,
    message: status === 'error' ? (detail || 'Stream failed.') : status === 'streaming' ? '' : 'Connecting video…',
    error: status === 'error',
    // DeviceStreamView: "Input disconnected (detail), reconnecting…" over a live picture.
    inputNotice: status === 'streaming' && !inputConnected && platform !== '' ? `Input disconnected${str(report.inputDetail) ? ` (${str(report.inputDetail)})` : ''}, reconnecting…` : '',
  };
}

const EMPTY_STREAM: R6DeviceStream = { status: '', message: '', error: false, inputNotice: '' };
const EMPTY_RAIL: R6DeviceRail = { inputConnected: false, appearance: '', appearanceLabel: '', appearanceIcon: 'moon', appearanceDisabled: true, textSize: '', textSizeDisabled: true, screenshotLabel: 'Save screenshot', screenshotDisabled: true, menu: '', menuTop: 0, menuRight: 0 };
export const emptyWorkspace = (): R6DeviceWorkspace => ({ key: '', name: '', description: '', platform: '', deviceId: '', hostId: '', screenLabel: '', alert: '', stream: EMPTY_STREAM, rail: EMPTY_RAIL, r7: emptyR7Workspace(), r9: emptyR9Folding() });
export const emptyR6Device = (): R6DeviceView => ({ mode: 'list', pendingKey: '', openingName: '', openingDescription: '', openingMessage: '', operationError: '', booting: '', hostDetail: '', workspace: emptyWorkspace() });
export const emptyMini = (): R6DeviceMini => ({ show: false, key: '', sourceKey: '', name: '', description: '', platform: '', deviceId: '', hostId: '', stream: EMPTY_STREAM });

/** DevicePanel's body after onboarding, for the thread `panelKey` names. */
export async function r6DeviceView(client: T3Client, native: Native | null, state: Obj, loaded: boolean, panelKey: string): Promise<R6DeviceView> {
  const store = storeOf(client), threadId = client.threadId;
  const device = activeDevice(state, threadId, store.targets.get(panelKey));
  const statuses = Object.values(obj(state.hostStatuses)).map(status => str(obj(status).status));
  const hostReady = statuses.includes('ready');
  const hostBusy = !hostReady && statuses.some(status => status === 'installing' || status === 'starting');
  const booting = arr(state.bootingDevices).filter(entry => str(entry.threadId) === threadId).map(entry => str(entry.name));
  const base = { ...emptyR6Device(), operationError: store.operationError,
    booting: booting.length ? `Starting ${booting.join(', ')}… This can take a minute.` : '',
    hostDetail: hostReady && !device ? str(state.hostStatusDetail) : '' };
  if (device) {
    const key = deviceKey({ hostId: str(device.hostId), id: str(device.id) });
    if (native && store.detailKey !== key) await readDetail(client, native, store, key, { hostId: str(device.hostId), deviceId: str(device.id) });
    const platform = platformOf(str(device.platform)), stream = streamView(client, key, platform);
    const settings = obj(store.detail?.settings), appearance = str(settings.appearance), next = appearance === 'dark' ? 'light' : 'dark';
    const controlsDisabled = store.acting || !store.detail;
    const r7 = r7Workspace(client, { key, platform, name: str(device.name), detail: store.detail, acting: store.acting, error: store.detailError, hostDiagnostics: str(state.hostStatusDetail), controlsDisabled, inputConnected: obj(obj(client.presentation.deviceStreams)[key]).inputConnected === true });
    // The workspace alert shows the controls' error only while the drawer is closed.
    const alert = store.screenshotError || (r7.tools.open ? '' : store.detailError);
    return { ...base, mode: 'workspace', workspace: {
      key, name: str(device.name), description: `${hostLabel(state, str(device.hostId))} · ${str(device.version)}`, platform, deviceId: str(device.id), hostId: str(device.hostId),
      screenLabel: platform === 'ios' ? 'iOS Simulator screen' : 'Android Emulator screen',
      alert, stream: r7.retaining && r7.rail.phone ? { ...stream, status: 'streaming' } : stream, r7,
      r9: r9Folding(obj(obj(client.presentation.deviceStreams)[key]), platform, r7.rail.phone, obj(obj(client.presentation.deviceStreams)[key]).inputConnected === true),
      rail: { inputConnected: obj(obj(client.presentation.deviceStreams)[key]).inputConnected === true, appearance,
        appearanceLabel: `Switch device to ${next} mode`, appearanceIcon: appearance === 'dark' ? 'sun' : 'moon', appearanceDisabled: controlsDisabled || !appearance,
        textSize: str(settings.textSize), textSizeDisabled: controlsDisabled || !str(settings.textSize),
        screenshotLabel: store.screenshotPending ? 'Capturing screenshot' : 'Save screenshot', screenshotDisabled: stream.status !== 'streaming' || store.screenshotPending, menu: store.menu, ...menuPlace(client, store.menu) },
    } };
  }
  const pending = store.pending;
  if (pending || hostBusy || !loaded) {
    return { ...base, mode: 'opening', pendingKey: pending ? deviceKey(pending) : '',
      openingName: pending?.name ?? 'Devices', openingDescription: pending ? `${hostLabel(state, pending.hostId)} · ${pending.version}` : '',
      openingMessage: pending ? (pending.booted ? 'Opening device…' : 'Starting device…')
        : str(state.hostStatus) === 'installing' ? str(state.hostStatusDetail, 'Installing device support…') : 'Finding devices…' };
  }
  return base;
}

/** The floating player for this thread, if one was floated (previewMiniPlayerStore). */
export function r6DeviceMini(client: T3Client, state: Obj | null): R6DeviceMini {
  const target = miniDeviceOf(client, client.threadId);
  if (!target || !client.threadId) return emptyMini();
  const device = state ? arr(state.devices).find(entry => str(entry.hostId) === target.hostId && str(entry.id) === target.deviceId) : undefined;
  const key = deviceKey(target), platform = platformOf(target.platform);
  return { show: true, key, sourceKey: previewMiniPlayerSourceKey(miniSource(target)), name: str(device?.name, target.name), description: `${state ? hostLabel(state, target.hostId) : 'Device host'} · ${str(device?.version, platform)}`,
    platform, deviceId: target.deviceId, hostId: target.hostId, stream: streamView(client, key, platform) };
}

const miniSource = (target: DeviceTarget): PreviewMiniPlayerSource => ({ kind: 'device', hostId: target.hostId, deviceId: target.deviceId, platform: platformOf(target.platform), name: target.name });
/** The connection's previewMiniPlayerStore (chat-canvas-view.ts reads and moves it). Threads are its keys. */
export const miniStoreOf = (client: T3Client): PreviewMiniPlayerStore => storeOf(client).mini;
/** Lane r12-threads: previewMiniPlayerStore open / close / the thread's source, for ChatView's device effects. */
export function miniDeviceOf(client: T3Client, threadId: string): DeviceTarget | undefined {
  const source = storeOf(client).mini.get(threadId)?.source;
  return source?.kind === 'device' ? { hostId: source.hostId, deviceId: source.deviceId, platform: source.platform, name: source.name } : undefined;
}
/** previewMiniPlayerStore.open: a new source keeps the thread's position and width. */
export function floatMiniDevice(client: T3Client, threadId: string, target: DeviceTarget): void { storeOf(client).mini.open(threadId, miniSource(target)); }
export const closeMiniDevice = (client: T3Client, threadId: string): void => { storeOf(client).mini.close(threadId); };

/** Text size (side left, centred, min-w-40, four 28pt rows) and More (side left, aligned to the
 *  trigger's end, three rows and a separator) beside their rail triggers, measured by `t3-frame`. */
export const TEXT_MENU_HEIGHT = 4 * 28 + 8 + 2, MORE_MENU_HEIGHT = 3 * 28 + 9 + 8 + 2, ROTATE_MENU_HEIGHT = 2 * 28 + 8 + 2;
export function menuPlace(client: T3Client, menu: string): { menuTop: number; menuRight: number } {
  const frames = obj(client.presentation.frames), nums = (value: unknown) => (Array.isArray(value) ? value.map(Number) : []);
  const area = nums(frames['r6-device']), trigger = nums(frames[`r6-device-${menu}`]);
  if (!menu || area.length < 4 || trigger.length < 4) return { menuTop: 0, menuRight: 0 };
  const centred = menu === 'text' ? TEXT_MENU_HEIGHT : menu === 'rotate' ? ROTATE_MENU_HEIGHT : 0;
  const top = centred ? trigger[1]! + trigger[3]! / 2 - centred / 2 : trigger[1]! + trigger[3]! - MORE_MENU_HEIGHT;
  const height = centred || MORE_MENU_HEIGHT;
  return { menuTop: Math.max(4, Math.min(area[3]! - height - 4, top - area[1]!)), menuRight: Math.max(0, area[0]! + area[2]! - trigger[0]! + 4) };
}

async function readDetail(client: T3Client, native: Native, store: Store, key: string, target: Obj): Promise<void> {
  store.detailKey = key;
  try { store.detail = obj(await client.restAccess(native).request('device.detail', target)); store.detailError = ''; }
  catch (error) { if (letGo(error)) throw error; store.detail = null; store.detailError = message(error, 'Could not read device settings.'); }
}

/** DevicePanel selectDevice: `device.open` for this thread, then the surface follows the device. */
export async function openDevice(client: T3Client, native: Native, state: Obj, key: string, panelKey: string): Promise<void> {
  const store = storeOf(client);
  if (store.pending) return;
  const device = arr(state.devices).find(candidate => deviceKey({ hostId: str(candidate.hostId), id: str(candidate.id) }) === key);
  if (!device) throw new ClientError('That device is no longer available.');
  if (!client.threadId) throw new ClientError('Open a thread to use a device.');
  const platform = platformOf(str(device.platform));
  store.operationError = '';
  store.pending = { hostId: str(device.hostId), deviceId: str(device.id), platform, name: str(device.name), booted: device.booted === true, version: str(device.version) };
  try {
    const result = obj(await client.restAccess(native).request('device.open', { threadId: client.threadId, hostId: str(device.hostId), deviceId: str(device.id), platform }, true));
    store.targets.set(panelKey, { hostId: str(result.hostId, str(device.hostId)), deviceId: str(result.deviceId, str(device.id)), platform, name: str(device.name) });
    store.detailKey = ''; store.menu = '';
  } catch (error) { if (!letGo(error)) store.operationError = message(error, 'Could not open the device.'); }
  finally { store.pending = null; }
}

/** closeActive(true): `device.close` with `shutdown`, then the surface closes. Returns whether it closed. */
export async function powerOff(client: T3Client, native: Native, state: Obj, panelKey: string): Promise<boolean> {
  const store = storeOf(client), device = activeDevice(state, client.threadId, store.targets.get(panelKey));
  store.menu = '';
  if (!device) return false;
  store.operationError = '';
  try {
    await client.restAccess(native).request('device.close', { threadId: client.threadId, hostId: str(device.hostId), deviceId: str(device.id), shutdown: true }, true);
    store.targets.delete(panelKey);
    return true;
  } catch (error) { if (letGo(error)) throw error; store.operationError = message(error, 'Could not power off the device.'); return false; }
}

/** useDeviceControls.act: one serialized `device.action`; its result is the confirmed detail. */
export async function deviceAction(client: T3Client, native: Native, state: Obj, panelKey: string, op: string, value: string): Promise<void> {
  const store = storeOf(client);
  store.menu = '';
  const body = railAction(op, value, str(obj(store.detail?.settings).appearance));
  if (body) await actDevice(client, native, state, panelKey, body);
}

/** One serialized `device.action` with any body (the rail, the Tools drawer); false when it did not run. */
export async function actDevice(client: T3Client, native: Native, state: Obj, panelKey: string, body: Obj): Promise<boolean> {
  const store = storeOf(client), device = activeDevice(state, client.threadId, store.targets.get(panelKey));
  store.menu = '';
  if (!device || store.acting || !store.detail) return false;
  store.acting = true; store.detailError = '';
  try { store.detail = obj(await client.restAccess(native).request('device.action', { hostId: str(device.hostId), deviceId: str(device.id), ...body }, true)); }
  catch (error) { if (!letGo(error)) store.detailError = message(error, 'The device action failed.'); }
  finally { store.acting = false; }
  return true;
}

/** The action body a rail control sends (DeviceControlsRail). */
export function railAction(op: string, value: string, appearance: string): Obj | null {
  if (op === 'appearance') return appearance ? { type: 'setAppearance', value: appearance === 'dark' ? 'light' : 'dark' } : null;
  if (op === 'text-size') return ['small', 'default', 'large', 'extra-large'].includes(value) ? { type: 'setTextSize', value } : null;
  return null;
}

/** `shell:surface-r6dev-*`, the gated writes. Returns true when the device surface should close. */
export async function r6DeviceCommand(client: T3Client, native: Native, state: Obj | null, panelKey: string, op: string, id: string, value: string): Promise<boolean> {
  const current = state ?? {};
  if (op === 'open') { await openDevice(client, native, current, id, panelKey); return false; }
  if (op === 'poweroff') return powerOff(client, native, current, panelKey);
  if (op === 'screenshot') { await saveScreenshot(client, native, current, panelKey); return false; }
  if (op === 'appearance' || op === 'text-size') { await deviceAction(client, native, current, panelKey, op, value); return false; }
  if (op.startsWith('tools-')) {
    const store = storeOf(client), device = activeDevice(current, client.threadId, store.targets.get(panelKey));
    const key = device ? deviceKey({ hostId: str(device.hostId), id: str(device.id) }) : '';
    await r7DeviceCommand(client, op.slice(6), id, value, key, store.detail, body => actDevice(client, native, current, panelKey, body));
    return false;
  }
  throw new ClientError(`Unknown device action: ${op}`);
}

/** DeviceWorkspace saveScreenshot through the module (the hub's POST /api/screenshot). */
export async function saveScreenshot(client: T3Client, native: Native, state: Obj, panelKey: string): Promise<void> {
  const store = storeOf(client), device = activeDevice(state, client.threadId, store.targets.get(panelKey));
  store.menu = '';
  if (!device || store.screenshotPending) return;
  store.screenshotPending = true; store.screenshotError = '';
  try {
    const reply = obj(await client.restAccess(native).call({ op: 'r6DeviceScreenshot', platform: platformOf(str(device.platform)), deviceId: str(device.id), hostId: str(device.hostId), name: str(device.name) }));
    if (reply.ok !== true) store.screenshotError = str(reply.message, 'Screenshot capture failed. Try again.');
  } catch (error) { if (!letGo(error)) store.screenshotError = message(error, 'Screenshot capture failed. Try again.'); }
  finally { store.screenshotPending = false; }
}

/** The no-write controls: menus, the error dismissal, float / restore / close, input buttons and Reconnect. */
export async function r6DeviceLocal(client: T3Client, native: Native, state: Obj | null, panelKey: string, op: string, value: string): Promise<'close' | 'hide' | 'reopen' | ''> {
  const store = storeOf(client);
  if (op === 'dismiss-error') { store.operationError = ''; return ''; }
  if (op === 'menu') { store.menu = store.menu === value ? '' : value; return ''; }
  if (op === 'close-panel') { store.menu = ''; return 'close'; }
  if (op === 'float') {
    store.menu = '';
    const device = state ? activeDevice(state, client.threadId, store.targets.get(panelKey)) : null;
    if (!device) return '';
    floatMiniDevice(client, client.threadId, { hostId: str(device.hostId), deviceId: str(device.id), platform: platformOf(str(device.platform)), name: str(device.name) });
    return 'hide'; // floatActive: the panel closes
  }
  if (op === 'mini-close') { closeMiniDevice(client, client.threadId); return ''; }
  if (op === 'mini-restore') {
    const target = miniDeviceOf(client, client.threadId);
    closeMiniDevice(client, client.threadId);
    if (!target) return '';
    store.targets.set(panelKey, target);
    return 'reopen';
  }
  if (op === 'duo' || op === 'fold') { // lane r9-device: a stand or fold button
    const device = state ? activeDevice(state, client.threadId, store.targets.get(panelKey)) : null, next = r9FoldingValue(op, value);
    if (device && next) await client.restAccess(native).call({ op: 'r6DeviceInput', action: op, key: deviceKey({ hostId: str(device.hostId), id: str(device.id) }), value: next });
    return '';
  }
  if (op === 'home' || op === 'rotate' || op === 'reconnect' || op === 'back' || op === 'recents') {
    await client.restAccess(native).call({ op: 'r6DeviceInput', action: op, key: value });
    return '';
  }
  if (op.startsWith('tools-')) {
    const device = state ? activeDevice(state, client.threadId, store.targets.get(panelKey)) : null;
    store.menu = '';
    await r7DeviceLocal(client, op.slice(6), value, device ? deviceKey({ hostId: str(device.hostId), id: str(device.id) }) : '', {
      input: (action, key, extra) => client.restAccess(native).call({ op: 'r6DeviceInput', action, key, value: extra }).then(() => undefined),
      act: body => (state ? actDevice(client, native, state, panelKey, body) : Promise.resolve(false)), detail: store.detail,
    });
    return '';
  }
  throw new ClientError(`Unknown device action: ${op}`);
}
