import { describe, expect, test } from 'bun:test';
import { activeDevice, menuPlace, r6DeviceCommand, r6DeviceLocal, r6DeviceMini, r6DeviceView, railAction, streamView, MORE_MENU_HEIGHT, TEXT_MENU_HEIGHT } from './r6-media-device';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';
import { ClientError } from './protocol';

// DeviceServiceState as the HEAD server reports it (contracts/device.ts).
const iphone = { hostId: 'local', id: 'SIM-1', platform: 'ios', name: 'iPhone 17 Pro', version: 'iOS 26.0', booted: false, physical: false };
const pixel = { hostId: 'local', id: 'emulator-5554', platform: 'android', name: 'Pixel 9', version: 'Android 16', booted: true, physical: false };
function deviceState(extra: Obj = {}): Obj {
  return { hosts: [{ id: 'local', kind: 'local', label: 'This Mac', platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: true }], hubInstalled: true, agentDeviceInstalled: false }],
    hostStatus: 'ready', hostStatuses: { local: { status: 'ready' } }, devices: [iphone, pixel], sessions: [], onboardingCompleted: true, agentAccessEnabled: false, hubBasePath: '/api/device-hub', revision: 3, ...extra };
}
type Call = { method?: string; payload?: Obj; write?: boolean; op?: string; request?: Obj };
function client(calls: Call[], replies: Record<string, unknown> = {}) {
  return {
    threadId: 't1', generation: 1, presentation: {} as Obj,
    restAccess: () => ({
      request: async (method: string, payload: Obj = {}, write = false) => {
        calls.push({ method, payload, write });
        const reply = replies[method];
        if (reply instanceof Error) throw reply;
        return reply ?? {};
      },
      call: async (request: Obj) => { calls.push({ op: String(request.op), request }); return replies[String(request.op)] ?? { ok: true }; },
    }),
  } as unknown as T3Client;
}
const native = { available: true } as Native;
const KEY = 'local\u0000SIM-1';
const settings = { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', settings: { appearance: 'light', textSize: 'default' } };

describe('the Device surface after onboarding (lane r6-media)', () => {
  test('the list until a device is chosen; host detail, booting devices and a busy host as the reference words them', async () => {
    const c = client([]);
    expect((await r6DeviceView(c, native, deviceState(), true, 'p')).mode).toBe('list');
    const view = await r6DeviceView(c, native, deviceState({ hostStatusDetail: 'Simulator runtime 26.0 is installing', bootingDevices: [{ ...iphone, threadId: 't1' }, { ...pixel, threadId: 'other' }] }), true, 'p');
    expect([view.mode, view.hostDetail, view.booting]).toEqual(['list', 'Simulator runtime 26.0 is installing', 'Starting iPhone 17 Pro… This can take a minute.']);
    const busy = await r6DeviceView(c, native, deviceState({ hostStatus: 'installing', hostStatuses: { local: { status: 'installing' } }, hostStatusDetail: 'Installing device hub 0.11.0…' }), true, 'p');
    expect([busy.mode, busy.openingName, busy.openingMessage]).toEqual(['opening', 'Devices', 'Installing device hub 0.11.0…']);
    expect((await r6DeviceView(c, native, deviceState(), false, 'p')).openingMessage).toBe('Finding devices…');
  });

  test('on a draft a row opens its device for the draft\'s own thread id (ChatView activeThreadRef, PA-1)', async () => {
    const calls: Call[] = [];
    const c = Object.assign(client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' } }), { threadId: '', environmentId: 'env', draftKey: 'env:new:p1', local: { composerControls: { draftThreads: { 'env:new:p1': 'draft-7' } } } });
    await r6DeviceCommand(c, native, deviceState(), 'p', 'open', KEY, '');
    expect(calls[0]).toEqual({ method: 'device.open', payload: { threadId: 'draft-7', hostId: 'local', deviceId: 'SIM-1', platform: 'ios' }, write: true });
    const view = await r6DeviceView(c, native, deviceState({ sessions: [{ threadId: 'draft-7', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] }), true, 'p');
    expect(view.mode).toBe('workspace');
  });

  test('a row opens its device with device.open for this thread, then the workspace shows it with its settings', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings });
    let state = deviceState();
    await r6DeviceCommand(c, native, state, 'p', 'open', KEY, '');
    expect(calls[0]).toEqual({ method: 'device.open', payload: { threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios' }, write: true });
    // The session arrives with the next device state.
    state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '2026-10-04T12:00:00Z' }], devices: [{ ...iphone, booted: true }, pixel] });
    expect(activeDevice(state, 't1', { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', name: '' })?.name).toBe('iPhone 17 Pro');
    const view = await r6DeviceView(c, native, state, true, 'p');
    expect(view.mode).toBe('workspace');
    expect(calls.filter(call => call.method === 'device.detail').map(call => call.payload)).toEqual([{ hostId: 'local', deviceId: 'SIM-1' }]);
    const ws = view.workspace;
    expect([ws.name, ws.description, ws.screenLabel, ws.stream.status, ws.stream.message]).toEqual(['iPhone 17 Pro', 'This Mac · iOS 26.0', 'iOS Simulator screen', 'connecting', 'Connecting video…']);
    expect([ws.rail.appearanceLabel, ws.rail.appearanceIcon, ws.rail.appearanceDisabled, ws.rail.textSize, ws.rail.screenshotDisabled, ws.rail.inputConnected])
      .toEqual(['Switch device to dark mode', 'moon', false, 'default', true, false]);
    // Another thread does not see this thread's session.
    expect(activeDevice(state, 't2', { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', name: '' })).toBeNull();
  });

  test('a failed open is the dismissible device error; the list returns', async () => {
    const c = client([], { 'device.open': new ClientError('Device SIM-1 failed to boot: There is not enough free disk space on the environment server.') });
    await r6DeviceCommand(c, native, deviceState(), 'p', 'open', KEY, '');
    let view = await r6DeviceView(c, native, deviceState(), true, 'p');
    expect([view.mode, view.operationError]).toEqual(['list', 'Device SIM-1 failed to boot: There is not enough free disk space on the environment server.']);
    await r6DeviceLocal(c, native, deviceState(), 'p', 'dismiss-error', '');
    view = await r6DeviceView(c, native, deviceState(), true, 'p');
    expect(view.operationError).toBe('');
    await expect(r6DeviceCommand(c, native, deviceState(), 'p', 'open', 'local\u0000gone', '')).rejects.toThrow('That device is no longer available.');
  });

  test('rail actions go through device.action and take its confirmed detail; Power off closes with shutdown', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings,
      'device.action': { ...settings, settings: { appearance: 'dark', textSize: 'large' } } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await r6DeviceCommand(c, native, state, 'p', 'open', KEY, '');
    await r6DeviceView(c, native, state, true, 'p');
    await r6DeviceCommand(c, native, state, 'p', 'appearance', '', '');
    await r6DeviceCommand(c, native, state, 'p', 'text-size', '', 'large');
    expect(calls.filter(call => call.method === 'device.action').map(call => call.payload)).toEqual([
      { hostId: 'local', deviceId: 'SIM-1', type: 'setAppearance', value: 'dark' }, { hostId: 'local', deviceId: 'SIM-1', type: 'setTextSize', value: 'large' }]);
    const view = await r6DeviceView(c, native, state, true, 'p');
    expect([view.workspace.rail.appearanceLabel, view.workspace.rail.appearanceIcon, view.workspace.rail.textSize]).toEqual(['Switch device to light mode', 'sun', 'large']);
    expect([railAction('text-size', 'huge', 'dark'), railAction('appearance', '', '')]).toEqual([null, null]);
    expect(await r6DeviceCommand(c, native, state, 'p', 'poweroff', '', '')).toBe(true);
    expect(calls.find(call => call.method === 'device.close')?.payload).toEqual({ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', shutdown: true });
  });

  test('Float moves the device over the chat and hides the panel; Open in right panel and Close floating preview', async () => {
    const c = client([], { 'device.open': { hostId: 'local', deviceId: 'SIM-1' } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await r6DeviceCommand(c, native, state, 'p', 'open', KEY, '');
    expect(r6DeviceMini(c, state).show).toBe(false);
    expect(await r6DeviceLocal(c, native, state, 'p', 'float', '')).toBe('hide');
    const mini = r6DeviceMini(c, state);
    // The player's frame is the chat canvas's (chat-canvas-view.test.ts); this is its source and stream.
    expect([mini.show, mini.name, mini.description, mini.sourceKey]).toEqual([true, 'iPhone 17 Pro', 'This Mac · iOS 26.0', 'device:local:SIM-1']);
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'streaming', inputConnected: true, width: 1206, height: 2622 } };
    expect(r6DeviceMini(c, state).stream.status).toBe('streaming');
    expect(await r6DeviceLocal(c, native, state, 'p', 'mini-restore', '')).toBe('reopen');
    expect(r6DeviceMini(c, state).show).toBe(false);
    await r6DeviceLocal(c, native, state, 'p', 'float', '');
    await r6DeviceLocal(c, native, state, 'p', 'mini-close', '');
    expect(r6DeviceMini(c, state).show).toBe(false);
  });

  test('the stream report reads as DeviceStreamView words it; input buttons and Reconnect reach the module', async () => {
    const calls: Call[] = [], c = client(calls);
    expect(streamView(c, KEY, 'ios')).toEqual({ status: 'connecting', message: 'Connecting video…', error: false, inputNotice: '' });
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'error', detail: 'No video received from the device. Reconnect to try again.' } };
    expect(streamView(c, KEY, 'ios')).toEqual({ status: 'error', message: 'No video received from the device. Reconnect to try again.', error: true, inputNotice: '' });
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'streaming', inputConnected: false, inputDetail: 'closed 1006' } };
    expect(streamView(c, KEY, 'ios').inputNotice).toBe('Input disconnected (closed 1006), reconnecting…');
    for (const op of ['home', 'rotate', 'reconnect']) await r6DeviceLocal(c, native, null, 'p', op, KEY);
    expect(calls.map(call => [call.op, call.request?.action, call.request?.key])).toEqual([['r6DeviceInput', 'home', KEY], ['r6DeviceInput', 'rotate', KEY], ['r6DeviceInput', 'reconnect', KEY]]);
  });

  test('Save screenshot reports the hub failure in the workspace; menus open beside their triggers', async () => {
    const c = client([], { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings, r6DeviceScreenshot: { ok: false, message: 'Screenshot capture failed (502). Try again.' } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await r6DeviceCommand(c, native, state, 'p', 'open', KEY, '');
    await r6DeviceCommand(c, native, state, 'p', 'screenshot', '', '');
    expect((await r6DeviceView(c, native, state, true, 'p')).workspace.alert).toBe('Screenshot capture failed (502). Try again.');
    (c.presentation as Obj).frames = { 'r6-device': [800, 80, 480, 760], 'r6-device-text': [1238, 400, 28, 28], 'r6-device-more': [1238, 464, 28, 28] };
    expect(menuPlace(c, 'text')).toEqual({ menuTop: 400 + 14 - TEXT_MENU_HEIGHT / 2 - 80, menuRight: 1280 - 1238 + 4 });
    expect(menuPlace(c, 'more')).toEqual({ menuTop: 464 + 28 - MORE_MENU_HEIGHT - 80, menuRight: 46 });
    expect(menuPlace(c, '')).toEqual({ menuTop: 0, menuRight: 0 });
    await r6DeviceLocal(c, native, state, 'p', 'menu', 'more');
    expect((await r6DeviceView(c, native, state, true, 'p')).workspace.rail.menu).toBe('more');
    await r6DeviceLocal(c, native, state, 'p', 'menu', 'more');
    expect((await r6DeviceView(c, native, state, true, 'p')).workspace.rail.menu).toBe('');
  });
});
