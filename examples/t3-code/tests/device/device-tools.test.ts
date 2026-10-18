import { describe, expect, test } from 'bun:test';
import { deviceSessionCommand, deviceSessionLocal, deviceSessionView } from '../../src/device/device-session';
import { foregroundOf, hasKeyboard, locationValid, toolBody, workspaceOf } from '../../src/device/device-tools';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

// DeviceStreamView's 3D / Flat, DeviceControlsRail's Android controls and
// DeviceToolsPanel, over HEAD's DeviceServiceState / DeviceDetail shapes (contracts/device.ts).
const iphone = { hostId: 'local', id: 'SIM-1', platform: 'ios', name: 'iPad Pro 13-inch (M5)', version: 'iOS 26.0', booted: true, physical: false };
const pixel = { hostId: 'local', id: 'emulator-5554', platform: 'android', name: 'Pixel 9', version: 'Android 16', booted: true, physical: false };
const IOS = 'local\u0000SIM-1', ANDROID = 'local\u0000emulator-5554';
function deviceState(device: Obj): Obj {
  return { hosts: [{ id: 'local', kind: 'local', label: 'This Mac', platforms: [], hubInstalled: true, agentDeviceInstalled: false }],
    hostStatus: 'ready', hostStatusDetail: 'Simulator runtime 26.0 ready', hostStatuses: { local: { status: 'ready' } }, devices: [iphone, pixel],
    sessions: [{ threadId: 't1', hostId: 'local', deviceId: device.id, platform: device.platform, openedAt: '' }], onboardingCompleted: true, agentAccessEnabled: false, hubBasePath: '/api/device-hub', revision: 1 };
}
type Call = { method?: string; payload?: Obj; op?: string; request?: Obj };
const detail = (deviceId: string, settings: Obj, foregroundApp: Obj | null = null) => ({ hostId: 'local', deviceId, settings, foregroundApp, readAt: '2026-10-05T00:00:00Z' });
function client(calls: Call[], replies: Record<string, unknown>) {
  return {
    threadId: 't1', generation: 1, presentation: {} as Obj,
    restAccess: () => ({
      request: async (method: string, payload: Obj = {}) => { calls.push({ method, payload }); const reply = replies[method]; if (reply instanceof Error) throw reply; return typeof reply === 'function' ? reply(payload) : reply ?? {}; },
      call: async (request: Obj) => { calls.push({ op: String(request.op), request }); return { ok: true }; },
    }),
  } as unknown as T3Client;
}
const native = { available: true } as Native;
const iosSettings = { appearance: 'dark', textSize: 'large', reduceMotion: false, increaseContrast: true, reduceTransparency: false, showBorders: false, voiceOver: false, liquidGlass: 'clear', colorFilter: 'grayscale' };
async function open(c: T3Client, device: Obj) {
  const state = deviceState(device);
  await deviceSessionCommand(c, native, state, 'p', 'open', `local\u0000${device.id}`, '');
  return state;
}

describe('device workspace completeness', () => {
  test('the rail: 3D by default once streaming H.264, Flat on request, the reason 3D is unavailable, the Magic Keyboard', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': detail('SIM-1', iosSettings) });
    const state = await open(c, iphone);
    let ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect(ws.controls.rail).toMatchObject({ view: 'phone', ax: '0', phone: false, streaming: false, phoneDisabled: true, toolsOpen: false, keyboard: false });
    (c.presentation as Obj).deviceStreams = { [IOS]: { status: 'streaming', inputConnected: true, phone: true, phoneUnavailable: '', orientation: 'portrait' } };
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect(ws.controls.rail).toMatchObject({ phone: true, streaming: true, phoneDisabled: false, keyboard: true, keyboardAttached: false, rotateDisabled: false });
    // Attaching the keyboard turns the iPad to landscape first and disables Rotate.
    await deviceSessionLocal(c, native, state, 'p', 'tools-keyboard', '');
    expect(calls.filter(call => call.op === 'deviceInput').map(call => call.request)).toEqual([{ op: 'deviceInput', action: 'orientation', key: IOS, value: 'landscape_right' }]);
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.controls.rail.view, ws.controls.rail.keyboardAttached, ws.controls.rail.rotateDisabled]).toEqual(['phone-keyboard', true, true]);
    await deviceSessionLocal(c, native, state, 'p', 'tools-reset', '');
    expect(calls.at(-1)?.request).toEqual({ op: 'deviceInput', action: 'reset', key: IOS, value: 'keyboard' });
    await deviceSessionLocal(c, native, state, 'p', 'tools-view', 'flat');
    (c.presentation as Obj).deviceStreams = { [IOS]: { status: 'streaming', inputConnected: true, phone: false, phoneUnavailable: '' } };
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.controls.rail.view, ws.controls.rail.phone, ws.controls.rail.keyboard]).toEqual(['flat', false, false]);
    // The module's reason disables the 3D button (MJPEG fallback, frames overlay).
    (c.presentation as Obj).deviceStreams = { [IOS]: { status: 'streaming', inputConnected: true, phone: false, phoneUnavailable: '3D requires the H.264 stream', mjpeg: true } };
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.controls.rail.phoneDisabled, ws.controls.rail.phoneReason]).toEqual([true, '3D requires the H.264 stream']);
    expect(hasKeyboard('ios', 'iPad Pro 13-inch (M5)')).toBe(true);
    expect(hasKeyboard('ios', 'iPad Air 11-inch (M3)')).toBe(false);
  });

  test('Android: Back and Recents reach the device; Rotate is a menu of device.action orientations; input notices name the socket', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'emulator-5554' }, 'device.detail': detail('emulator-5554', { appearance: 'light', textSize: 'default', reduceMotion: false, networkEnabled: true }),
      'device.action': (payload: Obj) => detail('emulator-5554', { appearance: 'light', textSize: 'default', reduceMotion: false, networkEnabled: payload.type === 'setToggle' ? payload.value : true }) });
    const state = await open(c, pixel);
    await deviceSessionView(c, native, state, true, 'p');
    await deviceSessionLocal(c, native, state, 'p', 'back', ANDROID);
    await deviceSessionLocal(c, native, state, 'p', 'recents', ANDROID);
    expect(calls.filter(call => call.op === 'deviceInput').map(call => call.request?.action)).toEqual(['back', 'recents']);
    let ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect(ws.controls.rail.rotateDisabled).toBe(false);
    await deviceSessionLocal(c, native, state, 'p', 'tools-pick-orientation', 'landscape_left');
    expect(calls.filter(call => call.method === 'device.action').map(call => call.payload)).toEqual([{ hostId: 'local', deviceId: 'emulator-5554', type: 'setOrientation', value: 'landscape_left' }]);
    (c.presentation as Obj).deviceStreams = { [ANDROID]: { status: 'streaming', inputConnected: false, inputDetail: 'closed 1001' } };
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect(ws.stream.inputNotice).toBe('Input disconnected (closed 1001), reconnecting…');
    // The encoder restarting after a fold: the last frame stays in 3D, then "Waiting for device video…".
    (c.presentation as Obj).deviceStreams = { [ANDROID]: { status: 'connecting', inputConnected: true, phone: true, retaining: true, restartNotice: true } };
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.stream.status, ws.controls.rail.waiting]).toEqual(['streaming', 'Waiting for device video…']);
    // The drawer's Android rows: Orientation and Network instead of the iOS-only controls.
    await deviceSessionLocal(c, native, state, 'p', 'tools-toggle', '');
    const tools = (await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools;
    expect([tools.ios, tools.platformTitle, tools.log, tools.launchPlaceholder]).toEqual([false, 'Emulator', false, 'Package name to launch']);
    expect(tools.switches.map(item => [item.id, item.label, item.checked])).toEqual([['reduceMotion', 'Reduce Motion', false], ['networkEnabled', 'Network', true]]);
    expect(tools.permissions.map(item => item.label)).toEqual(['Camera', 'Microphone', 'Photos', 'Contacts', 'Calendar', 'Location', 'Notifications', 'Physical activity']);
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'toggle', 'networkEnabled=0');
    expect(calls.filter(call => call.method === 'device.action').at(-1)?.payload).toEqual({ hostId: 'local', deviceId: 'emulator-5554', type: 'setToggle', setting: 'networkEnabled', value: false });
    expect((await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools.switches[1]!.checked).toBe(false);
  });

  test('the Tools drawer: settings read from the device, one device.action per control, confirmed values only', async () => {
    let current: Obj = { ...iosSettings };
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': detail('SIM-1', current, { id: 'com.example.app', name: 'Example' }),
      'device.action': (payload: Obj) => {
        if (payload.type === 'setColorFilter') current = { ...current, colorFilter: payload.value };
        if (payload.type === 'setLiquidGlass') current = { ...current, liquidGlass: payload.value };
        return detail('SIM-1', current, { id: 'com.example.app' });
      } });
    const state = await open(c, iphone);
    await deviceSessionLocal(c, native, state, 'p', 'tools-toggle', '');
    let tools = (await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools;
    expect(tools).toMatchObject({ open: true, overlay: true, ios: true, platformTitle: 'Simulator', foreground: 'com.example.app', hasForeground: true, disabled: false, loading: false,
      appearance: 'dark', textSizeLabel: 'Large', textSizeKnown: true, liquidGlass: 'clear', liquidGlassDisabled: false, colorFilterLabel: 'Grayscale', diagnostics: 'Simulator runtime 26.0 ready',
      permPlaceholder: 'com.example.app', permDisabled: false, permissionLabel: 'Camera', pushDisabled: false, pushSendDisabled: true, log: true, logOpen: false, eventsEmpty: true });
    expect(tools.switches.map(item => [item.label, item.checked, item.disabled])).toEqual([['Reduce Motion', false, false], ['Increase Contrast', true, false], ['Reduce Transparency', false, false], ['Show Borders', false, false], ['VoiceOver', false, false]]);
    expect(tools.colorFilters.map(item => item.label)).toEqual(['None', 'Grayscale', 'Red / green (protanopia)', 'Green / red (deuteranopia)', 'Blue / yellow (tritanopia)']);
    expect(tools.presets.map(item => item.label)).toEqual(['San Francisco', 'New York', 'London', 'Stockholm', 'Tokyo']);
    // A 700pt-wide area keeps the drawer beside the screen.
    (c.presentation as Obj).frames = { 'device-area': [0, 0, 760, 700] };
    expect((await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools.overlay).toBe(false);
    // Drafts, submits (a submitted field clears), selects, toggles, permissions, push, location.
    await deviceSessionLocal(c, native, state, 'p', 'tools-draft-url', '  myapp://home ');
    tools = (await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools;
    expect([tools.url, tools.urlDisabled]).toEqual(['  myapp://home ', false]);
    await deviceSessionLocal(c, native, state, 'p', 'tools-submit-url', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-pick-color-filter', 'blue-yellow');
    await deviceSessionLocal(c, native, state, 'p', 'tools-pick-liquid-glass', 'tinted');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'terminate', '');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'relaunch', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-permission', 'faceid');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'grant', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-draft-permApp', 'com.other');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'reset', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-draft-push', 'Hello');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'push', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-pick-preset', 'Tokyo');
    await deviceSessionLocal(c, native, state, 'p', 'tools-draft-lat', '95');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'location', '');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'location-clear', '');
    expect(calls.filter(call => call.method === 'device.action').map(call => { const { hostId: _h, deviceId: _d, ...body } = call.payload!; return body; })).toEqual([
      { type: 'openUrl', url: 'myapp://home' },
      { type: 'setColorFilter', value: 'blue-yellow' },
      { type: 'setLiquidGlass', value: 'tinted' },
      { type: 'terminateApp', appId: 'com.example.app' },
      { type: 'launchApp', appId: 'com.example.app' },
      { type: 'setPermission', appId: 'com.example.app', permission: 'faceid', decision: 'grant' },
      { type: 'setPermission', appId: 'com.other', permission: 'faceid', decision: 'reset' },
      { type: 'sendPush', appId: 'com.example.app', payload: 'Hello' },
      { type: 'setLocation', latitude: 35.6762, longitude: 139.6503 },
      { type: 'clearLocation' },
    ]);
    tools = (await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools;
    expect([tools.url, tools.push, tools.lat, tools.lng, tools.colorFilterLabel, tools.liquidGlass]).toEqual(['', '', '', '', 'Blue / yellow (tritanopia)', 'tinted']);
    // The overlay and event log are local; the log's entries come from the module's feed.
    await deviceSessionLocal(c, native, state, 'p', 'tools-ax', '');
    await deviceSessionLocal(c, native, state, 'p', 'tools-log', '');
    (c.presentation as Obj).deviceTools = { [IOS]: { foreground: 'com.apple.Preferences', eventsOpen: true, events: [{ id: 3, time: '12:35:09', summary: 'Swipe' }] } };
    const ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.controls.rail.ax, ws.controls.tools.axOn, ws.controls.tools.logOpen, ws.controls.tools.logDevice, ws.controls.tools.logHost, ws.controls.tools.foreground]).toEqual(['1', true, true, 'SIM-1', 'local', 'com.apple.Preferences']);
    expect(ws.controls.tools.events).toEqual([{ id: '3', time: '12:35:09', summary: 'Swipe' }]);
    await deviceSessionLocal(c, native, state, 'p', 'tools-close', '');
    expect((await deviceSessionView(c, native, state, true, 'p')).workspace.controls.tools.open).toBe(false);
  });

  test('a failed action shows in the drawer while it is open, in the workspace otherwise; nothing runs while one is pending', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': detail('SIM-1', iosSettings), 'device.action': new Error('simctl failed') });
    const state = await open(c, iphone);
    await deviceSessionView(c, native, state, true, 'p');
    await deviceSessionLocal(c, native, state, 'p', 'tools-toggle', '');
    await deviceSessionCommand(c, native, state, 'p', 'tools-act', 'toggle', 'voiceOver=1');
    let ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect([ws.alert, ws.controls.tools.error]).toEqual(['', 'simctl failed']);
    await deviceSessionLocal(c, native, state, 'p', 'tools-close', '');
    ws = (await deviceSessionView(c, native, state, true, 'p')).workspace;
    expect(ws.alert).toBe('simctl failed');
  });

  test('bodies, foreground and location rules', () => {
    const ws = workspaceOf({} as T3Client, 'k');
    expect(toolBody(ws, 'toggle', 'shake=1', '')).toBeNull();
    expect(toolBody(ws, 'appearance', 'sepia', '')).toBeNull();
    expect(toolBody(ws, 'terminate', '', '')).toBeNull();
    expect(toolBody(ws, 'grant', '', '')).toBeNull();
    expect(toolBody(ws, 'push', '', 'com.example.app')).toBeNull();
    expect(toolBody(ws, 'orientation', 'portrait_upside_down', '')).toEqual({ type: 'setOrientation', value: 'portrait_upside_down' });
    expect(foregroundOf({}, { foregroundApp: { id: 'a' } })).toBe('a');
    expect(foregroundOf({ foreground: null }, { foregroundApp: { id: 'a' } })).toBe('');
    expect(foregroundOf({ foreground: 'b' }, null)).toBe('b');
    expect([locationValid('', '1'), locationValid('90', '-180'), locationValid('90.1', '0'), locationValid('1', 'x')]).toEqual([false, true, false, false]);
  });
});
