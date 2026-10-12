// Lane r7-device: the device workspace's 3D / Flat presentation, Android's Back / Recents / Rotate
// and the Tools drawer (MIT reference, see LICENSE-T3: components/device/DeviceWorkspace.tsx,
// DeviceStreamView.tsx, DeviceControlsRail.tsx, DeviceToolsPanel.tsx, useDeviceControls.ts,
// deviceHubApi.ts). The picture, the 3D phone, accessibility frames and the hub's foreground /
// event-log feeds are the app module's (R6DeviceStream.swift, R7Device*.swift), reported as
// `presentation.deviceStreams[key]` and `presentation.deviceTools[key]`; every device change is one
// serialized `device.action` (r6-media-device.ts actDevice) whose result is the confirmed detail.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';

export type R7DevOption = { value: string; label: string; selected: boolean };
export type R7DevSwitch = { id: string; label: string; checked: boolean; disabled: boolean };
export type R7DevEvent = { id: string; time: string; summary: string };
export type R7DevDeviceRail = {
  /** data-device-view: "phone", "phone-keyboard" or "flat"; data-device-ax: "1" or "0". */
  view: string; ax: string; phone: boolean; streaming: boolean; phoneDisabled: boolean; phoneReason: string;
  keyboard: boolean; keyboardAttached: boolean; toolsOpen: boolean; waiting: string; rotateDisabled: boolean; controlsDisabled: boolean;
};
export type R7DevDeviceTools = {
  open: boolean; overlay: boolean; pending: boolean; error: string; loading: boolean; diagnostics: string; ios: boolean; platformTitle: string;
  foreground: string; hasForeground: boolean; disabled: boolean;
  appearance: string; textSizeLabel: string; textSizeKnown: boolean; textSizes: R7DevOption[];
  liquidGlass: string; liquidGlassDisabled: boolean; colorFilterLabel: string; colorFilterKnown: boolean; colorFilters: R7DevOption[]; orientations: R7DevOption[];
  switches: R7DevSwitch[]; axOn: boolean;
  url: string; urlDisabled: boolean; launch: string; launchPlaceholder: string; launchDisabled: boolean;
  lat: string; lng: string; locationValid: boolean; presets: R7DevOption[];
  permApp: string; permPlaceholder: string; permDisabled: boolean; permissionLabel: string; permissions: R7DevOption[];
  push: string; pushDisabled: boolean; pushSendDisabled: boolean;
  log: boolean; logOpen: boolean; logDevice: string; logHost: string; eventsEmpty: boolean; events: R7DevEvent[];
};
export type R7DevDeviceWorkspace = { rail: R7DevDeviceRail; tools: R7DevDeviceTools; retaining: boolean };

export const TEXT_SIZES = [['small', 'Small'], ['default', 'Default'], ['large', 'Large'], ['extra-large', 'Extra large']] as const;
export const COLOR_FILTERS = [['none', 'None'], ['grayscale', 'Grayscale'], ['red-green', 'Red / green (protanopia)'], ['green-red', 'Green / red (deuteranopia)'], ['blue-yellow', 'Blue / yellow (tritanopia)']] as const;
export const ORIENTATIONS = [['portrait', 'Portrait'], ['landscape_left', 'Landscape left'], ['portrait_upside_down', 'Upside down'], ['landscape_right', 'Landscape right']] as const;
export const IOS_PERMISSIONS = [['camera', 'Camera'], ['microphone', 'Microphone'], ['photos', 'Photos'], ['contacts', 'Contacts'], ['calendar', 'Calendar'], ['reminders', 'Reminders'], ['location', 'Location'], ['notifications', 'Notifications'], ['motion', 'Motion'], ['media-library', 'Media library'], ['faceid', 'Face ID']] as const;
export const ANDROID_PERMISSIONS = [['camera', 'Camera'], ['microphone', 'Microphone'], ['photos', 'Photos'], ['contacts', 'Contacts'], ['calendar', 'Calendar'], ['location', 'Location'], ['notifications', 'Notifications'], ['motion', 'Physical activity']] as const;
export const LOCATION_PRESETS = [['San Francisco', 37.7749, -122.4194], ['New York', 40.7128, -74.006], ['London', 51.5074, -0.1278], ['Stockholm', 59.3293, 18.0686], ['Tokyo', 35.6762, 139.6503]] as const;
const IOS_SWITCHES = [['reduceMotion', 'Reduce Motion'], ['increaseContrast', 'Increase Contrast'], ['reduceTransparency', 'Reduce Transparency'], ['showBorders', 'Show Borders'], ['voiceOver', 'VoiceOver']] as const;
const ANDROID_SWITCHES = [['reduceMotion', 'Reduce Motion'], ['networkEnabled', 'Network']] as const;
const TOGGLES: readonly string[] = ['reduceMotion', 'increaseContrast', 'reduceTransparency', 'showBorders', 'voiceOver', 'networkEnabled'];

/** DeviceWorkspace's and DeviceStreamView's local state, per device (the reference keys the workspace by device). */
type Workspace = {
  presentation: 'phone' | 'flat'; keyboard: boolean; tools: boolean; ax: boolean; log: boolean;
  url: string; launch: string; push: string; lat: string; lng: string; permApp: string; permission: string;
};
const stores = new WeakMap<T3Client, Map<string, Workspace>>();
export function workspaceOf(client: T3Client, key: string): Workspace {
  let map = stores.get(client);
  if (!map) { map = new Map(); stores.set(client, map); }
  let entry = map.get(key);
  if (!entry) {
    entry = { presentation: 'phone', keyboard: false, tools: false, ax: false, log: false, url: '', launch: '', push: '', lat: '', lng: '', permApp: '', permission: 'camera' };
    map.set(key, entry);
  }
  return entry;
}

const options = (list: readonly (readonly [string, string])[], value: string): R7DevOption[] => list.map(([v, label]) => ({ value: v, label, selected: v === value }));
const EMPTY_RAIL: R7DevDeviceRail = { view: 'flat', ax: '0', phone: false, streaming: false, phoneDisabled: true, phoneReason: '', keyboard: false, keyboardAttached: false, toolsOpen: false, waiting: '', rotateDisabled: true, controlsDisabled: true };
export const emptyR7Tools = (): R7DevDeviceTools => ({
  open: false, overlay: true, pending: false, error: '', loading: false, diagnostics: '', ios: true, platformTitle: 'Simulator', foreground: '—', hasForeground: false, disabled: true,
  appearance: '', textSizeLabel: 'Unknown', textSizeKnown: false, textSizes: [], liquidGlass: '', liquidGlassDisabled: true, colorFilterLabel: 'Unknown', colorFilterKnown: false, colorFilters: [], orientations: [],
  switches: [], axOn: false, url: '', urlDisabled: true, launch: '', launchPlaceholder: 'Bundle ID to launch', launchDisabled: true, lat: '', lng: '', locationValid: false, presets: [],
  permApp: '', permPlaceholder: 'App ID', permDisabled: true, permissionLabel: 'Camera', permissions: [], push: '', pushDisabled: true, pushSendDisabled: true, log: false, logOpen: false, logDevice: '', logHost: '', eventsEmpty: true, events: [],
});
export const emptyR7Workspace = (): R7DevDeviceWorkspace => ({ rail: EMPTY_RAIL, tools: emptyR7Tools(), retaining: false });

/** deviceModels.ts deviceKeyboard: only the iPad Pro 13-inch (M5) has a Magic Keyboard model. */
export const hasKeyboard = (platform: string, name: string) => platform === 'ios' && /^iPad Pro 13-inch \(M5\)$/i.test(name);

/** useDeviceControls foregroundApp: the hub's feed once it reported, else the detail's. */
export function foregroundOf(feed: Obj, detail: Obj | null): string {
  if ('foreground' in feed) return feed.foreground === null ? '' : str(feed.foreground);
  return str(obj(detail?.foregroundApp).id);
}

/** LocationSection's `valid`: both fields filled and inside latitude / longitude bounds. */
export function locationValid(lat: string, lng: string): boolean {
  if (lat.trim() === '' || lng.trim() === '') return false;
  const latitude = Number(lat), longitude = Number(lng);
  return Math.abs(latitude) <= 90 && Math.abs(longitude) <= 180;
}

export type R7DevInput = { key: string; platform: string; name: string; detail: Obj | null; acting: boolean; error: string; hostDiagnostics: string; controlsDisabled: boolean; inputConnected: boolean };

export function r7Workspace(client: T3Client, input: R7DevInput): R7DevDeviceWorkspace {
  const ws = workspaceOf(client, input.key);
  const report = obj(obj(client.presentation.deviceStreams)[input.key]);
  const feed = obj(obj(client.presentation.deviceTools)[input.key]);
  const status = str(report.status), streaming = status === 'streaming';
  const phone = report.phone === true, reason = str(report.phoneUnavailable);
  const keyboard = phone && hasKeyboard(input.platform, input.name);
  const attached = ws.keyboard && hasKeyboard(input.platform, input.name);
  const ios = input.platform === 'ios';
  const rail: R7DevDeviceRail = {
    view: ws.presentation === 'phone' ? (attached ? 'phone-keyboard' : 'phone') : 'flat', ax: ws.ax ? '1' : '0',
    phone, streaming, phoneDisabled: !streaming || reason !== '', phoneReason: reason, keyboard, keyboardAttached: attached, toolsOpen: ws.tools,
    waiting: report.restartNotice === true ? 'Waiting for device video…' : '',
    rotateDisabled: ios ? !input.inputConnected || (keyboard && attached) : input.controlsDisabled, controlsDisabled: input.controlsDisabled,
  };
  const settings = obj(input.detail?.settings);
  const disabled = input.acting || !input.detail;
  const foreground = foregroundOf(feed, input.detail);
  const textSize = str(settings.textSize), colorFilter = str(settings.colorFilter);
  const textLabel = TEXT_SIZES.find(([value]) => value === textSize)?.[1], filterLabel = COLOR_FILTERS.find(([value]) => value === colorFilter)?.[1];
  const permissionList = ios ? IOS_PERMISSIONS : ANDROID_PERMISSIONS;
  const resolvedApp = ws.permApp.trim() || foreground;
  const frame = obj(client.presentation.frames)['r7-device-area'], area = Array.isArray(frame) ? frame.map(Number) : [];
  const tools: R7DevDeviceTools = {
    open: ws.tools, overlay: !(area.length >= 4 && area[2]! >= 700), pending: input.acting, error: input.error, loading: !input.detail && !input.error,
    diagnostics: input.hostDiagnostics, ios, platformTitle: ios ? 'Simulator' : 'Emulator',
    foreground: foreground || '—', hasForeground: foreground !== '', disabled,
    appearance: str(settings.appearance), textSizeLabel: textLabel ?? 'Unknown', textSizeKnown: !!textLabel, textSizes: options(TEXT_SIZES, textSize),
    liquidGlass: str(settings.liquidGlass), liquidGlassDisabled: disabled || settings.liquidGlass === undefined,
    colorFilterLabel: filterLabel ?? 'Unknown', colorFilterKnown: !!filterLabel, colorFilters: options(COLOR_FILTERS, colorFilter), orientations: options(ORIENTATIONS, ''),
    switches: (ios ? IOS_SWITCHES : ANDROID_SWITCHES).map(([id, label]) => ({ id, label, checked: settings[id] === true, disabled: disabled || typeof settings[id] !== 'boolean' })),
    axOn: ws.ax,
    url: ws.url, urlDisabled: disabled || ws.url.trim() === '', launch: ws.launch, launchPlaceholder: ios ? 'Bundle ID to launch' : 'Package name to launch', launchDisabled: disabled || ws.launch.trim() === '',
    lat: ws.lat, lng: ws.lng, locationValid: locationValid(ws.lat, ws.lng), presets: LOCATION_PRESETS.map(([label]) => ({ value: label, label, selected: false })),
    permApp: ws.permApp, permPlaceholder: foreground || 'App ID', permDisabled: disabled || resolvedApp === '',
    permissionLabel: permissionList.find(([value]) => value === ws.permission)?.[1] ?? 'Camera', permissions: options(permissionList, ws.permission),
    push: ws.push, pushDisabled: disabled || foreground === '', pushSendDisabled: disabled || foreground === '' || ws.push.trim() === '',
    log: ios, logOpen: ws.log && ios, logHost: input.key.split('\u0000')[0] ?? '', logDevice: input.key.split('\u0000')[1] ?? '',
    eventsEmpty: arr(feed.events).length === 0,
    events: arr(feed.events).map(entry => ({ id: String(entry.id ?? ''), time: str(entry.time), summary: str(entry.summary) })),
  };
  return { rail, tools, retaining: report.retaining === true };
}

type Act = (body: Obj) => Promise<boolean>;

/** The action body for one drawer control (DeviceToolsPanel), or null when it should not run. */
export function toolBody(ws: Workspace, kind: string, value: string, foreground: string): Obj | null {
  switch (kind) {
    case 'appearance': return value === 'light' || value === 'dark' ? { type: 'setAppearance', value } : null;
    case 'text-size': return TEXT_SIZES.some(([v]) => v === value) ? { type: 'setTextSize', value } : null;
    case 'liquid-glass': return value === 'clear' || value === 'tinted' ? { type: 'setLiquidGlass', value } : null;
    case 'color-filter': return COLOR_FILTERS.some(([v]) => v === value) ? { type: 'setColorFilter', value } : null;
    case 'orientation': return ORIENTATIONS.some(([v]) => v === value) ? { type: 'setOrientation', value } : null;
    case 'toggle': {
      const [setting, on] = value.split('=');
      return setting && TOGGLES.includes(setting) ? { type: 'setToggle', setting, value: on === '1' } : null;
    }
    case 'terminate': return foreground ? { type: 'terminateApp', appId: foreground } : null;
    case 'relaunch': return foreground ? { type: 'launchApp', appId: foreground } : null;
    case 'url': return ws.url.trim() ? { type: 'openUrl', url: ws.url.trim() } : null;
    case 'launch': return ws.launch.trim() ? { type: 'launchApp', appId: ws.launch.trim() } : null;
    case 'push': return foreground && ws.push.trim() ? { type: 'sendPush', appId: foreground, payload: ws.push.trim() } : null;
    case 'location': return locationValid(ws.lat, ws.lng) ? { type: 'setLocation', latitude: Number(ws.lat), longitude: Number(ws.lng) } : null;
    case 'location-clear': return { type: 'clearLocation' };
    case 'preset': {
      const preset = LOCATION_PRESETS.find(([label]) => label === value);
      return preset ? { type: 'setLocation', latitude: preset[1], longitude: preset[2] } : null;
    }
    case 'grant': case 'revoke': case 'reset': {
      const appId = ws.permApp.trim() || foreground;
      return appId ? { type: 'setPermission', appId, permission: ws.permission, decision: kind } : null;
    }
    default: return null;
  }
}

/** Runs one drawer control; a submitted field clears (SubmitRow), a preset fills the fields. */
async function runTool(client: T3Client, key: string, kind: string, value: string, foreground: string, act: Act): Promise<void> {
  const ws = workspaceOf(client, key);
  const body = toolBody(ws, kind, value, foreground);
  if (!body) return;
  if (kind === 'preset') { ws.lat = String(body.latitude); ws.lng = String(body.longitude); }
  if (kind === 'location-clear') { ws.lat = ''; ws.lng = ''; }
  await act(body);
  if (kind === 'url') ws.url = '';
  if (kind === 'launch') ws.launch = '';
  if (kind === 'push') ws.push = '';
}

/** `shell:surface-r6dev-tools-act`: id names the control, value its choice. */
export async function r7DeviceCommand(client: T3Client, op: string, id: string, value: string, key: string, detail: Obj | null, act: Act): Promise<void> {
  if (op !== 'act' || !key) return;
  await runTool(client, key, id, value, foregroundOf(obj(obj(client.presentation.deviceTools)[key]), detail), act);
}

export type R7DevLocalDeps = { input: (action: string, key: string, value: string) => Promise<void>; act: Act; detail: Obj | null };

/** The drawer's and rail's local state, and the input-only controls. */
export async function r7DeviceLocal(client: T3Client, op: string, value: string, key: string, deps: R7DevLocalDeps): Promise<void> {
  if (!key) return;
  const ws = workspaceOf(client, key);
  const report = obj(obj(client.presentation.deviceStreams)[key]);
  if (op === 'toggle') { ws.tools = !ws.tools; return; }
  if (op === 'close') { ws.tools = false; return; }
  if (op === 'ax') { ws.ax = !ws.ax; return; }
  if (op === 'log') { ws.log = !ws.log; return; }
  if (op === 'view') { if (value === 'phone' || value === 'flat') ws.presentation = value; return; }
  if (op === 'keyboard') {
    // The keyboard docks the iPad in landscape.
    if (!ws.keyboard && str(report.orientation) !== 'landscape_right') await deps.input('orientation', key, 'landscape_right');
    ws.keyboard = !ws.keyboard;
    return;
  }
  if (op === 'reset') { await deps.input('reset', key, ws.keyboard ? 'keyboard' : ''); return; }
  if (op === 'permission') { if ([...IOS_PERMISSIONS, ...ANDROID_PERMISSIONS].some(([v]) => v === value)) ws.permission = value; return; }
  if (op.startsWith('draft-')) {
    const field = op.slice(6) as 'url' | 'launch' | 'push' | 'lat' | 'lng' | 'permApp';
    if (['url', 'launch', 'push', 'lat', 'lng', 'permApp'].includes(field)) ws[field] = value;
    return;
  }
  if (op.startsWith('pick-')) {
    await runTool(client, key, op.slice(5), value, foregroundOf(obj(obj(client.presentation.deviceTools)[key]), deps.detail), deps.act);
    return;
  }
  if (op.startsWith('submit-')) {
    await runTool(client, key, op.slice(7), value, foregroundOf(obj(obj(client.presentation.deviceTools)[key]), deps.detail), deps.act);
    return;
  }
}
