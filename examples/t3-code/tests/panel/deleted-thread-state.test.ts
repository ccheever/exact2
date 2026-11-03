// Deleting a thread drops what this device kept for it (T3 Code 5d3d78ab93, #28386: useThreadActions.ts
// clearDeletedThreadPanelState): its right panel, live and saved, its Changes selection and base, its floating player.
import { expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { clearDeletedThreadPanelState } from '../../src/panel/deleted-thread-state';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { floatMiniDevice, miniStoreOf } from '../../src/device/device-session';

test('a deleted thread leaves no panel, diff or player state; another thread keeps its own', () => {
  const client = new T3Client();
  const local = client.local as { rightPanels?: Record<string, unknown> };
  for (const id of ['gone', 'kept']) {
    const key = `env:${id}`;
    surfaceStore(client).panels.set(key, { surfaces: [], active: '', visible: true, userRevision: 1 });
    local.rightPanels = { ...local.rightPanels, [key]: { visible: true, active: 'files', surfaces: [] } };
    client.diffState.selections[key] = { kind: 'branch', runId: '', filePath: '', baseRef: 'main' };
    client.diffState.branchBaseRefs[key] = 'main';
    floatMiniDevice(client, id, { hostId: 'h', deviceId: 'd', platform: 'ios', name: 'iPhone' });
  }
  clearDeletedThreadPanelState(client, 'env', 'gone');
  expect([...surfaceStore(client).panels.keys()]).toEqual(['env:kept']);
  expect(Object.keys(local.rightPanels ?? {})).toEqual(['env:kept']);
  expect(Object.keys(client.diffState.selections)).toEqual(['env:kept']);
  expect(Object.keys(client.diffState.branchBaseRefs)).toEqual(['env:kept']);
  expect(Object.keys(miniStoreOf(client).byThreadKey)).toEqual(['kept']);
});
