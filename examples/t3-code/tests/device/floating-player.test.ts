// The floating device player's baseline (floating-player.ts) and the panel tab
// strip (tab-strip.ts). Ported from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/previewMiniPlayerStore.test.ts and components/ChatView.logic.test.ts ("floating
// browser preview"), with device sources where the reference uses browser tabs (this client floats
// devices only), and agentOpenedPreviews.test.ts's device snapshots.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import { applyDeviceSnapshot, resetLaunch, visibleMini } from '../../src/device/floating-player';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { closeMiniDevice, floatMiniDevice, miniDeviceOf, miniStoreOf, deviceMini } from '../../src/device/device-session';

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
    expect(deviceMini(client, state([]))).toMatchObject({ show: true, deviceId: 'IPHONE' });
    select(client, 'thread-B');
    expect(deviceMini(client, state([]))).toMatchObject({ show: true, deviceId: 'emulator-5580' });
  });

  test('floats one source per thread, so a device replaces the browser tab', () => {
    const client = thread('thread-A');
    floatMiniDevice(client, 'thread-A', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    floatMiniDevice(client, 'thread-A', { hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Pixel' });
    const floating = miniStoreOf(client).get('thread-A');
    expect(miniDeviceOf(client, 'thread-A')).toMatchObject({ deviceId: 'emulator-5580' });
    // The same device under a new label is still the same floating source.
    floatMiniDevice(client, 'thread-A', { hostId: 'nucbox', deviceId: 'emulator-5580', platform: 'android', name: 'Renamed' });
    expect(miniStoreOf(client).get('thread-A')).toBe(floating);
    expect(miniDeviceOf(client, 'thread-A')).toMatchObject({ name: 'Pixel' });
  });
});

describe('floating device preview (shouldRenderPreviewMiniPlayer)', () => {
  test('only hides the duplicate while the same device is rendered in the panel', () => {
    const client = thread();
    expect(visibleMini(deviceMini(client, null), undefined).show).toBe(false);
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    const mini = deviceMini(client, state([open()]));
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(false);
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'OTHER', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, { hostId: 'other', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, undefined).show).toBe(true);
  });
});

// agentOpenedPreviews.test.ts at 7c3b3b3605ef "applyDeviceSnapshot", its 6 tests under their own names.
describe('applyDeviceSnapshot', () => {
  const opened = (threadId = 't1', device = iphone, openedAt = '2026-01-01T00:00:00.000Z') => ({ ...open(threadId, device), openedAt });
  const floating = { sheet: false, autoShowFloatingPreview: true };
  const apply = (client: T3Client, snapshot: Obj, options = floating) => applyDeviceSnapshot(client, 'env', snapshot, options, new EnvironmentFleet());

  test('treats the first snapshot as a baseline and floats sessions that start later', () => {
    resetLaunch();
    const client = thread();
    apply(client, state([opened()]));
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    apply(client, state([]));
    apply(client, state([opened()]));
    expect(miniDeviceOf(client, 't1')).toMatchObject({ deviceId: 'IPHONE' });
  });
  test('opens a session once its device summary arrives', () => {
    resetLaunch();
    const client = thread();
    apply(client, state([]));
    apply(client, state([opened()], []));
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    apply(client, state([opened()]));
    expect(miniDeviceOf(client, 't1')).toMatchObject({ name: 'iPhone 18 Pro' });
  });
  test('opens a panel tab once when floating previews are off, and nothing in sheet layouts', () => {
    resetLaunch();
    const client = { ...thread(), draftKey: 'env:t1', local: {} } as unknown as T3Client;
    const panel = { sheet: false, autoShowFloatingPreview: false };
    apply(client, state([]), { ...panel, sheet: true });
    apply(client, state([opened()]), { ...panel, sheet: true });
    expect(surfaceStore(client).panels.get('env:t1')).toBeUndefined();
    apply(client, state([]), panel);
    apply(client, state([opened()]), panel);
    apply(client, state([]), panel);
    apply(client, state([opened()]), panel);
    expect(surfaceStore(client).panels.get('env:t1')?.surfaces.filter(surface => surface.kind === 'device')).toHaveLength(1);
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
  });
  test('closes a floating device when its session ends, even before a baseline', () => {
    resetLaunch();
    const client = thread();
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    apply(client, state([opened()]));
    expect(miniDeviceOf(client, 't1')).toMatchObject({ deviceId: 'IPHONE' });
    apply(client, state([]));
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
  });
  test('opens a session that ended and reopened between two snapshots', () => {
    resetLaunch();
    const client = thread();
    apply(client, state([opened()]));
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    apply(client, state([opened('t1', iphone, '2026-01-01T00:05:00.000Z')]));
    expect(miniDeviceOf(client, 't1')).toMatchObject({ deviceId: 'IPHONE' });
  });
  test('opens a device for a thread that had no sessions when the baseline was taken', () => {
    resetLaunch();
    const client = thread();
    apply(client, state([opened()]));
    apply(client, state([opened(), opened('t2')]));
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    // A background thread's player: the person sees it on opening that thread.
    expect(miniDeviceOf(client, 't2')).toMatchObject({ deviceId: 'IPHONE' });
  });
});
