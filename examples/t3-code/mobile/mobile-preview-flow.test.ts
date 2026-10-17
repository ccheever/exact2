import { expect, test } from 'bun:test';
import { mobileClient, mobileNative } from './client';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserRead, mobileBrowserSnapshot, BROWSER_EVENTS } from './browser-mobile-data';
import { mobilePreviewMenus, mobilePreviewMenuAction } from './mobile-preview-flow';
import { mobileDevicesRead, mobileDevicesSnapshot } from './devices-mobile-data';
import { T3Client } from './shared/client';
import { deviceStateEvent, DEVICE_STATE_KEY } from './shared/r4-surfaces-device';
import { obj } from './shared/domain';
import type { Native } from './shared/protocol';

test('native preview menus preserve immutable owner/target and explicit operation arguments', () => {
  const menus = mobilePreviewMenus('browser', { owner: 'owner', source: 'target', tabs: [{ id: 'b', title: 'Tab B', selected: true }], menu: [{ id: 'clear:cache', title: 'Clear cache', destructive: true, group: 'Site data' }], pipSupported: true });
  const choices = JSON.parse(menus.selection), options = JSON.parse(menus.options);
  expect(choices).toMatchObject({ owner: 'owner', target: 'target', items: [{ id: 'b', operation: 'select', value: 'b', selected: true }] });
  expect(options.items[0]).toMatchObject({ operation: 'clear:cache', destructive: true, group: 'Site data' });
  expect(options.items[1].operation).toBe('pip');
});
test('an obsolete native menu cannot dispatch into a different preview', async () => {
  const calls: unknown[] = [];
  const native: Native = { available: true, watch() {}, async later(input) { calls.push(input); return {}; } };
  await mobilePreviewMenuAction('browser', JSON.stringify({ owner: 'old', target: 'old', operation: 'clear:cache' }), 'new', native);
  expect(calls).toHaveLength(0);
});
test.each([1, 2])('device options show a picker only for multiple open previews: %i', async count => {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://device-menu.test', environmentId: 'env', threadId: 'thread', generation: 4,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  const calls: { op: string; target: string }[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push({ op: String(request.op), target: String(request.path ?? request.method) });
    return { ok: true, generation: 4, value: request.op === 'http'
      ? { authenticated: true, permissions: ['orchestration:read'] } : { id: '4-1' } };
  } };
  const owner = mobileStreamOwner(client); await mobileDevicesRead(owner, native, client);
  deviceStateEvent(client, { key: DEVICE_STATE_KEY, subscriptionId: '4-1', value: {
    hosts: [{ id: 'host', label: 'Host' }], devices: [], supportsToolInspection: true,
    sessions: [
      { hostId: 'host', deviceId: 'first', threadId: 'thread', platform: 'ios' },
      ...(count === 2 ? [{ hostId: 'host', deviceId: 'second', threadId: 'thread', platform: 'android' }] : []),
      { hostId: 'host', deviceId: 'unrelated', threadId: 'another-thread', platform: 'ios' },
    ],
  } });
  const snapshot = mobileDevicesSnapshot(owner, client), menus = mobilePreviewMenus('devices', snapshot);
  const selection = JSON.parse(menus.selection), options = JSON.parse(menus.options);
  expect(snapshot.count).toBe(count);
  expect(snapshot.devices.map(item => item.id)).toEqual(count === 2 ? ['["host","first"]', '["host","second"]'] : ['["host","first"]']);
  expect(selection).toMatchObject({ owner, target: snapshot.source });
  expect(options).toMatchObject({ owner, target: snapshot.source });
  expect(selection.items).toHaveLength(count);
  expect(selection.items.map((item: { operation: string; value: string; selected: boolean }) => [item.operation, item.value, item.selected]))
    .toEqual(snapshot.devices.map(item => ['select', item.id, item.selected]));
  const controls = options.items.filter((item: { group: string }) => item.group !== 'Devices');
  expect(controls.map((item: { id: string; disabled: boolean; operation: string; value: string }) => [item.id, item.disabled, item.operation, item.value]))
    .toEqual([['inspect', false, 'inspect', ''], ['tools', false, 'tools', ''], ['reload', false, 'reload', ''],
      ['appSwitcher', true, 'appSwitcher', ''], ['rotate', true, 'rotate', ''], ['shutdown', true, 'shutdown', '']]);
  expect(calls).toEqual([{ op: 'http', target: '/api/auth/session' }, { op: 'subscribe', target: 'subscribeDeviceState' }]);
  expect(options.items.filter((item: { group: string }) => item.group === 'Devices')).toEqual(count === 2
    ? selection.items.map((item: object) => ({ ...item, group: 'Devices' })) : []);
});
test('mobile event hook reassembles large native batches before shared acknowledgment', async () => {
  const saved = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, generation: mobileClient.generation,
    connection: mobileClient.connection, configLive: mobileClient.configLive, shellLive: mobileClient.shellLive, threadLive: mobileClient.threadLive };
  const calls: string[] = [];
  Object.assign(mobileClient, { origin: 'https://menu-fixture.test', environmentId: 'event-env', threadId: 'event-thread', generation: 4,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  try {
    const owner = mobileStreamOwner(mobileClient);
    const batch = { events: [{ seq: 1, generation: 4, key: BROWSER_EVENTS, subscriptionId: '4-7', value: { type: 'changed', serverEpoch: 'a', revision: 2, threadId: 'event-thread', tabId: 'new', snapshot: { tabId: 'new', runtime: 'server', navStatus: { _tag: 'Loaded', title: 'New tab' }, updatedAt: '2026-01-01' } } }] };
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input); calls.push(String(request.op));
      const value = request.op === 'http' ? { authenticated: true, permissions: ['orchestration:read', 'preview:operate'] }
        : request.op === 'subscribe' ? { id: '4-7' }
        : request.method === 'preview.list' ? { serverEpoch: 'a', revision: 1, sessions: [] }
        : request.op === 'events' ? { _nativeTransfer: { id: 'large', parts: 1 } }
        : request.op === 'readChunk' ? { text: JSON.stringify(batch) } : {};
      return { ok: true, generation: 4, value };
    } };
    await mobileBrowserRead(owner, native);
    const response = obj(await mobileNative(native).later({ op: 'events', after: 0 }));
    expect(response.value).toEqual(batch);
    expect(calls.slice(-3)).toEqual(['events', 'readChunk', 'releaseChunk']);
    expect(mobileBrowserSnapshot(owner).title).toBe('New tab');
    expect(mobileBrowserSnapshot(owner).count).toBe(1);
  } finally { Object.assign(mobileClient, saved); }
});
