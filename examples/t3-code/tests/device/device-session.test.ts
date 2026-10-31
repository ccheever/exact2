import { describe, expect, test } from 'bun:test';
import { activeDevice, deviceSessionCommand, deviceSessionLocal, deviceMini, deviceSessionView, railAction, refreshDeviceHub, streamView } from '../../src/device/device-session';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { ClientError } from '../../src/client/protocol';

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
      call: async (request: Obj) => {
        calls.push({ op: String(request.op), request });
        const reply = replies[String(request.op)];
        return typeof reply === 'function' ? reply(request) : reply ?? { ok: true };
      },
    }),
  } as unknown as T3Client;
}
const native = { available: true } as Native;
const KEY = 'local\u0000SIM-1';
const settings = { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', settings: { appearance: 'light', textSize: 'default' } };

describe('the Device surface after onboarding', () => {
  test('the list until a device is chosen; host detail, booting devices and a busy host as the reference words them', async () => {
    const c = client([]);
    expect((await deviceSessionView(c, native, deviceState(), true, 'p')).mode).toBe('list');
    const view = await deviceSessionView(c, native, deviceState({ hostStatusDetail: 'Simulator runtime 26.0 is installing', bootingDevices: [{ ...iphone, threadId: 't1' }, { ...pixel, threadId: 'other' }] }), true, 'p');
    expect([view.mode, view.hostDetail, view.booting]).toEqual(['list', 'Simulator runtime 26.0 is installing', 'Starting iPhone 17 Pro… This can take a minute.']);
    const busy = await deviceSessionView(c, native, deviceState({ hostStatus: 'installing', hostStatuses: { local: { status: 'installing' } }, hostStatusDetail: 'Installing device hub 0.11.0…' }), true, 'p');
    expect([busy.mode, busy.openingName, busy.openingMessage]).toEqual(['opening', 'Devices', 'Installing device hub 0.11.0…']);
    expect((await deviceSessionView(c, native, deviceState(), false, 'p')).openingMessage).toBe('Finding devices…');
  });

  test('on a draft a row opens its device for the draft\'s own thread id (ChatView activeThreadRef)', async () => {
    const calls: Call[] = [];
    const c = Object.assign(client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' } }), { threadId: '', environmentId: 'env', draftKey: 'env:new:p1', local: { composerControls: { draftThreads: { 'env:new:p1': 'draft-7' } } } });
    await deviceSessionCommand(c, native, deviceState(), 'p', 'open', KEY, '');
    expect(calls[0]).toEqual({ method: 'device.open', payload: { threadId: 'draft-7', hostId: 'local', deviceId: 'SIM-1', platform: 'ios' }, write: true });
    const view = await deviceSessionView(c, native, deviceState({ sessions: [{ threadId: 'draft-7', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] }), true, 'p');
    expect(view.mode).toBe('workspace');
  });

  test('a row opens its device with device.open for this thread, then the workspace shows it with its settings', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings });
    let state = deviceState();
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    expect(calls[0]).toEqual({ method: 'device.open', payload: { threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios' }, write: true });
    // The session arrives with the next device state.
    state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '2026-10-04T12:00:00Z' }], devices: [{ ...iphone, booted: true }, pixel] });
    expect(activeDevice(state, 't1', { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', name: '' })?.name).toBe('iPhone 17 Pro');
    const view = await deviceSessionView(c, native, state, true, 'p');
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
    await deviceSessionCommand(c, native, deviceState(), 'p', 'open', KEY, '');
    let view = await deviceSessionView(c, native, deviceState(), true, 'p');
    expect([view.mode, view.operationError]).toEqual(['list', 'Device SIM-1 failed to boot: There is not enough free disk space on the environment server.']);
    await deviceSessionLocal(c, native, deviceState(), 'p', 'dismiss-error', '');
    view = await deviceSessionView(c, native, deviceState(), true, 'p');
    expect(view.operationError).toBe('');
    await expect(deviceSessionCommand(c, native, deviceState(), 'p', 'open', 'local\u0000gone', '')).rejects.toThrow('That device is no longer available.');
  });

  test('rail actions go through device.action and take its confirmed detail; Power off closes with shutdown', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings,
      'device.action': { ...settings, settings: { appearance: 'dark', textSize: 'large' } } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    await deviceSessionView(c, native, state, true, 'p');
    await deviceSessionCommand(c, native, state, 'p', 'appearance', '', '');
    await deviceSessionCommand(c, native, state, 'p', 'text-size', '', 'large');
    expect(calls.filter(call => call.method === 'device.action').map(call => call.payload)).toEqual([
      { hostId: 'local', deviceId: 'SIM-1', type: 'setAppearance', value: 'dark' }, { hostId: 'local', deviceId: 'SIM-1', type: 'setTextSize', value: 'large' }]);
    const view = await deviceSessionView(c, native, state, true, 'p');
    expect([view.workspace.rail.appearanceLabel, view.workspace.rail.appearanceIcon, view.workspace.rail.textSize]).toEqual(['Switch device to light mode', 'sun', 'large']);
    expect([railAction('text-size', 'huge', 'dark'), railAction('appearance', '', '')]).toEqual([null, null]);
    expect(await deviceSessionCommand(c, native, state, 'p', 'poweroff', '', '')).toBe(true);
    expect(calls.find(call => call.method === 'device.close')?.payload).toEqual({ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', shutdown: true });
  });

  test('Float moves the device over the chat and hides the panel; Open in right panel and Close floating preview', async () => {
    const c = client([], { 'device.open': { hostId: 'local', deviceId: 'SIM-1' } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    expect(deviceMini(c, state).show).toBe(false);
    expect(await deviceSessionLocal(c, native, state, 'p', 'float', '')).toBe('hide');
    const mini = deviceMini(c, state);
    // The player's frame is the chat canvas's (chat-canvas-view.test.ts); this is its source and stream.
    expect([mini.show, mini.name, mini.description, mini.sourceKey]).toEqual([true, 'iPhone 17 Pro', 'This Mac · iOS 26.0', 'device:local:SIM-1']);
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'streaming', inputConnected: true, width: 1206, height: 2622 } };
    expect(deviceMini(c, state).stream.status).toBe('streaming');
    expect(await deviceSessionLocal(c, native, state, 'p', 'mini-restore', '')).toBe('reopen');
    expect(deviceMini(c, state).show).toBe(false);
    await deviceSessionLocal(c, native, state, 'p', 'float', '');
    await deviceSessionLocal(c, native, state, 'p', 'mini-close', '');
    expect(deviceMini(c, state).show).toBe(false);
  });

  test('the stream report reads as DeviceStreamView words it; input buttons and Reconnect reach the module', async () => {
    const calls: Call[] = [], c = client(calls);
    expect(streamView(c, KEY, 'ios')).toEqual({ status: 'connecting', message: 'Connecting video…', error: false, inputNotice: '' });
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'error', detail: 'No video received from the device. Reconnect to try again.' } };
    expect(streamView(c, KEY, 'ios')).toEqual({ status: 'error', message: 'No video received from the device. Reconnect to try again.', error: true, inputNotice: '' });
    (c.presentation as Obj).deviceStreams = { [KEY]: { status: 'streaming', inputConnected: false, inputDetail: 'closed 1006' } };
    expect(streamView(c, KEY, 'ios').inputNotice).toBe('Input disconnected (closed 1006), reconnecting…');
    for (const op of ['home', 'rotate', 'reconnect']) await deviceSessionLocal(c, native, null, 'p', op, KEY);
    expect(calls.map(call => [call.op, call.request?.action, call.request?.key])).toEqual([['deviceInput', 'home', KEY], ['deviceInput', 'rotate', KEY], ['deviceInput', 'reconnect', KEY]]);
  });

  test('a ticket the module asks for is minted once per ask and carried by the workspace\'s stream node; a failed mint carries none', async () => {
    const calls: Call[] = [];
    let serial = 0, refuse = false;
    const c = client(calls, { 'device.detail': settings, deviceHubAccess: () => {
      if (refuse) throw new ClientError('The server is not connected.', 'Disconnected');
      return { origin: 'http://127.0.0.1:4001', ticket: `ticket-${++serial}` };
    } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    const hub = async () => { await refreshDeviceHub(c, native); return (await deviceSessionView(c, native, state, true, 'p')).workspace.hub; };
    expect(await hub()).toEqual({ origin: '', ticket: '', ask: '' }); // nothing asked yet
    (c.presentation as Obj).deviceHub = { want: 1, module: 'm1' };
    expect(await hub()).toEqual({ origin: 'http://127.0.0.1:4001', ticket: 'ticket-1', ask: 'm1:1' });
    expect(await hub()).toEqual({ origin: 'http://127.0.0.1:4001', ticket: 'ticket-1', ask: 'm1:1' }); // the same ask is answered once
    (c.presentation as Obj).deviceHub = { want: 2, module: 'm1' };
    expect(await hub()).toEqual({ origin: 'http://127.0.0.1:4001', ticket: 'ticket-2', ask: 'm1:2' });
    expect(deviceMini(c, state).hub).toEqual({ origin: '', ticket: '', ask: '' }); // no floating player
    // A new module instance counts from 1 again: its asks are its own.
    (c.presentation as Obj).deviceHub = { want: 2, module: 'm2' };
    expect(await hub()).toEqual({ origin: 'http://127.0.0.1:4001', ticket: 'ticket-3', ask: 'm2:2' });
    // Once the module reports the ask answered, the node no longer carries the ticket.
    (c.presentation as Obj).deviceHub = { want: 2, module: 'm2', answered: 2 };
    expect(await hub()).toEqual({ origin: '', ticket: '', ask: 'm2:2' });
    refuse = true; (c.presentation as Obj).deviceHub = { want: 3, module: 'm2', answered: 2 };
    expect(await hub()).toEqual({ origin: '', ticket: '', ask: 'm2:3' });
    expect(calls.filter(entry => entry.op === 'deviceHubAccess')).toHaveLength(4);
  });

  test('Save screenshot sends a fresh hub ticket along', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.detail': settings, deviceHubAccess: { origin: 'http://127.0.0.1:4001', ticket: 'ticket-s' }, deviceScreenshot: { ok: true, saved: true } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    await deviceSessionCommand(c, native, state, 'p', 'screenshot', '', '');
    expect(calls.filter(entry => entry.op === 'deviceScreenshot').map(entry => entry.request)).toEqual([
      { op: 'deviceScreenshot', platform: 'ios', deviceId: 'SIM-1', hostId: 'local', name: 'iPhone 17 Pro', origin: 'http://127.0.0.1:4001', ticket: 'ticket-s' }]);
  });

  test('Save screenshot without a ticket says to reconnect and asks the module nothing', async () => {
    const calls: Call[] = [], c = client(calls, { 'device.detail': settings, deviceHubAccess: () => { throw new ClientError('The server is not connected.', 'Disconnected'); } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    await deviceSessionCommand(c, native, state, 'p', 'screenshot', '', '');
    expect(calls.some(entry => entry.op === 'deviceScreenshot')).toBe(false);
    expect((await deviceSessionView(c, native, state, true, 'p')).workspace.alert).toBe('Reconnect to the environment and try again.');
  });

  test('Save screenshot reports the hub failure in the workspace', async () => {
    const c = client([], { 'device.open': { hostId: 'local', deviceId: 'SIM-1' }, 'device.detail': settings, deviceScreenshot: { ok: false, message: 'Screenshot capture failed (502). Try again.' } });
    const state = deviceState({ sessions: [{ threadId: 't1', hostId: 'local', deviceId: 'SIM-1', platform: 'ios', openedAt: '' }] });
    await deviceSessionCommand(c, native, state, 'p', 'open', KEY, '');
    await deviceSessionCommand(c, native, state, 'p', 'screenshot', '', '');
    expect((await deviceSessionView(c, native, state, true, 'p')).workspace.alert).toBe('Screenshot capture failed (502). Try again.');
  });
});
