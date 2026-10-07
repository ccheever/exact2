import { expect, test } from 'bun:test';
import { mobileClient, mobileNative } from './client';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserRead, mobileBrowserSnapshot, BROWSER_EVENTS } from './browser-mobile-data';
import { mobilePreviewMenus, mobilePreviewMenuAction } from './mobile-preview-flow';
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
