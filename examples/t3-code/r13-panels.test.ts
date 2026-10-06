// Lane r13-panels: the floating device player's baseline (r12-threads-device.ts) and the panel tab
// strip (r12-threads-tabs.ts). Ported from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/previewMiniPlayerStore.test.ts and components/ChatView.logic.test.ts ("floating
// browser preview"), with device sources where the reference uses browser tabs (this client floats
// devices only); the baseline cases follow ChatView.tsx's device-session effect.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { autoShowDevices, EMPTY_DEVICE_STATE, resetLaunch, visibleMini } from './r12-threads-device';
import { closeMiniDevice, floatMiniDevice, miniDeviceOf, r6DeviceMini } from './r6-media-device';

const iphone = { hostId: 'local', id: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro', version: 'iOS 26.0' };
const pixel = { hostId: 'nucbox', id: 'emulator-5580', platform: 'android', name: 'Pixel', version: 'Android 16' };
const state = (sessions: Obj[], devices: Obj[] = [iphone, pixel]) => ({ hosts: [], devices, sessions });
const open = (threadId = 't1', device = iphone) => ({ threadId, hostId: device.hostId, deviceId: device.id });
const thread = (threadId = 't1', environmentId = 'env', generation = 1) =>
  ({ environmentId, threadId, generation, presentation: {} }) as unknown as T3Client;
const select = (client: T3Client, threadId: string) => { (client as unknown as { threadId: string }).threadId = threadId; };

describe('previewMiniPlayerStore (device sources)', () => {
  test('keeps floating previews scoped to their thread', () => {
    const client = thread('thread-A');
    floatMiniDevice(client, 'thread-A', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    floatMiniDevice(client, 'thread-B', { hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Pixel' });
    expect(miniDeviceOf(client, 'thread-A')).toMatchObject({ deviceId: 'IPHONE' });
    expect(miniDeviceOf(client, 'thread-B')).toMatchObject({ deviceId: 'emulator-5580' });
    expect(r6DeviceMini(client, state([]))).toMatchObject({ show: true, deviceId: 'IPHONE' });
    select(client, 'thread-B');
    expect(r6DeviceMini(client, state([]))).toMatchObject({ show: true, deviceId: 'emulator-5580' });
  });

  test('floats one source per thread, so a device replaces the browser tab', () => {
    const client = thread('thread-A');
    floatMiniDevice(client, 'thread-A', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    floatMiniDevice(client, 'thread-A', { hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Pixel' });
    const floating = miniDeviceOf(client, 'thread-A');
    expect(floating).toMatchObject({ deviceId: 'emulator-5580' });
    // The same device under a new label is still the same floating source.
    floatMiniDevice(client, 'thread-A', { hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Renamed' });
    expect(miniDeviceOf(client, 'thread-A')).toBe(floating!);
  });
});

describe('floating device preview (shouldRenderPreviewMiniPlayer)', () => {
  test('only hides the duplicate while the same device is rendered in the panel', () => {
    const client = thread();
    expect(visibleMini(r6DeviceMini(client, null), undefined).show).toBe(false);
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    const mini = r6DeviceMini(client, state([open()]));
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(false);
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'OTHER', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, { hostId: 'other', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, undefined).show).toBe(true);
  });
});

describe('a thread\'s device session floats (ChatView device-session effect)', () => {
  test('the thread shown before the first device state floats its open session when the state lands', () => {
    resetLaunch();
    const client = thread();
    autoShowDevices(client, null, false); // useDeviceState before the first chunk: loaded, EMPTY_DEVICE_STATE
    expect(r6DeviceMini(client, null).show).toBe(false);
    autoShowDevices(client, state([open()]), false);
    expect(r6DeviceMini(client, state([open()]))).toMatchObject({ show: true, name: 'iPhone 18 Pro', deviceId: 'IPHONE' });
    expect(EMPTY_DEVICE_STATE).toMatchObject({ sessions: [], devices: [], hostStatus: 'disabled', revision: 0 });
  });

  test('a thread first shown after the state arrived takes it as its baseline (navigating to it)', () => {
    resetLaunch();
    const client = thread('t2');
    autoShowDevices(client, null, false);
    autoShowDevices(client, state([open()]), false); // t1's session, while t2 is open
    select(client, 't1');
    autoShowDevices(client, state([open()]), false);
    expect(r6DeviceMini(client, state([open()])).show).toBe(false);
    // A session opened later on that thread floats.
    autoShowDevices(client, state([open(), open('t1', pixel)]), false);
    expect(r6DeviceMini(client, state([open(), open('t1', pixel)]))).toMatchObject({ show: true, deviceId: 'emulator-5580' });
  });

  test('a session whose device summary has not arrived stays out of the baseline', () => {
    resetLaunch();
    const client = thread();
    autoShowDevices(client, state([open()], []), false); // the session, but no device list yet
    expect(r6DeviceMini(client, null).show).toBe(false);
    autoShowDevices(client, state([open()]), false);
    expect(r6DeviceMini(client, state([open()]))).toMatchObject({ show: true, deviceId: 'IPHONE' });
  });

  test('a reconnect keeps the environment\'s last state: a closed player is not resurrected', () => {
    resetLaunch();
    const client = thread();
    autoShowDevices(client, null, false);
    autoShowDevices(client, state([open()]), false);
    expect(miniDeviceOf(client, 't1')).toMatchObject({ deviceId: 'IPHONE' });
    closeMiniDevice(client, 't1'); // the person closes the player
    autoShowDevices(client, null, false); // the connection came back; its stream has not answered yet
    autoShowDevices(client, state([open()]), false);
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    // Another environment's thread has no state of its own yet: an empty baseline there.
    const other = thread('t9', 'env-b');
    autoShowDevices(other, null, false);
    autoShowDevices(other, state([open('t9')]), false);
    expect(miniDeviceOf(other, 't9')).toMatchObject({ deviceId: 'IPHONE' });
  });

  test('a sheet layout floats nothing, and a closed session takes the player with it', () => {
    resetLaunch();
    const narrow = thread();
    autoShowDevices(narrow, null, true);
    autoShowDevices(narrow, state([open()]), true);
    expect(r6DeviceMini(narrow, state([open()])).show).toBe(false);
    const wide = thread();
    autoShowDevices(wide, null, false);
    autoShowDevices(wide, state([open()]), false);
    expect(r6DeviceMini(wide, state([open()])).show).toBe(true);
    autoShowDevices(wide, state([]), false);
    expect(r6DeviceMini(wide, state([])).show).toBe(false);
  });
});
