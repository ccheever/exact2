// Settings › Integrations reads (fix-settings-integrations-loop; STATUS "Found, not in scope", the #346 row): while the
// page was open the app asked server.getConfig, server.getSettings and device.list 26 times a second. Each answer sent
// `device.list { inspectOnly: true }`; the server's DeviceService.inspect publishes every host's summary to each
// subscribeDeviceState stream (T3 Code 1e2ecbd975 apps/server/src/device/DeviceService.ts:473-503), the app holds one
// (useDeviceState, r12-threads-device.ts), so the answer's own reply woke `data`, whose drain bumps data.revision, the
// key `integrations` is asked on (app.contract) — and the page asked again. The reference's page reads the scope's server
// config (useScopedSettings: server.getConfig at connect, then subscribeServerConfig) and useDeviceState
// (subscribeDeviceState); only "Check versions" sends inspectOnly (IntegrationsSettings.tsx:610-745).
// These tests go through the app's own answer(), as the Contract asks it, with a small runner of the two resources.
import { afterEach, describe, expect, test } from 'bun:test';
import { answer } from './app';
import { Backend, storage } from './client-fixture';
import { arr, obj, str, type Obj } from './domain';
import { DEVICE_STATE_KEY } from './r4-surfaces-device';
import { resetPrimary } from './local-primary-fixture';
import { resetHighlightSlicing } from './r12-render-highlight';

const READS = ['server.getConfig', 'server.getSettings', 'device.list'];

/** The lane server's device service behind the fake transport: inspect publishes to every device-state stream. */
class DeviceServer extends Backend {
  deviceRevision = 0;
  /** A watched topic changed: the transport appended to its inbox (T3Transport `changed("t3.events")`). */
  woke = false;
  hub: Obj = { requiredVersion: '0.4.0', installedVersions: ['0.3.0'] };
  deviceState(): Obj {
    return { hosts: [{ id: 'local', kind: 'local', label: 'This machine', platforms: [], tools: { hub: this.hub, agent: { requiredVersion: '1.2.0', installedVersions: ['1.2.0'] } } }],
      hostStatus: 'disabled', hostStatuses: {}, devices: [], sessions: [], onboardingCompleted: false, agentAccessEnabled: false,
      supportsToolInspection: true, supportsToolUpdate: true, revision: this.deviceRevision };
  }
  override emit(key: string, value: Obj, subscriptionId = this.subscriptions[key]) { super.emit(key, value, subscriptionId); this.woke = true; }
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'subscribe' && request.method === 'subscribeDeviceState') {
      this.calls.push(request);
      const id = `sub-${++this.serial}`;
      this.subscriptions[DEVICE_STATE_KEY] = id;
      this.emit(DEVICE_STATE_KEY, this.deviceState()); // DeviceService.stateStream: the current snapshot first
      return this.good({ id });
    }
    if (request.op === 'request' && request.method === 'device.list') {
      this.calls.push(request);
      // DeviceService.inspect: each host's fresh summary is published (revision + 1) to every stream.
      if (obj(request.payload).inspectOnly === true) { this.deviceRevision++; if (this.subscriptions[DEVICE_STATE_KEY]) this.emit(DEVICE_STATE_KEY, this.deviceState()); }
      return this.good(this.deviceState());
    }
    return super.later(input);
  }
  reads(from = 0) { return this.calls.slice(from).filter(call => call.op === 'request' && READS.includes(str(call.method))).map(call => str(call.method)); }
}

/**
 * The two resources as app.contract wires them: `data` (snapshot) is asked again when a topic it watches changes;
 * `integrations` is asked again when data.revision changes. Runs until the runner is idle or `limit` turns.
 */
async function run(native: DeviceServer, files: ReturnType<typeof storage>['files'], open: boolean, limit = 12) {
  const state = { asks: 0, page: {} as Obj };
  let revision = Number(obj(await answer('snapshot', [], null, files, native)).revision), asked = -1;
  for (let turn = 0; turn < limit; turn++) {
    native.woke = false;
    if (revision !== asked) {
      asked = revision; state.asks++;
      state.page = obj(await answer('integrationsPage', ['env1', '', open, 0, revision, '', '', ''], null, files, native));
    }
    if (!native.woke) return { ...state, idle: true };
    revision = Number(obj(await answer('snapshot', [], null, files, native)).revision);
  }
  return { ...state, idle: false };
}

/** A connected app whose shell has opened the device stream, as at launch (shellView → panelView → watchThreadDevices). */
let generation = 59; // above the other app tests' servers (providers-scope.test.ts: 41): the app's client bootstraps from each new one
async function launched() {
  const native = new DeviceServer(), files = storage().files;
  native.generation = ++generation; native.serial = generation * 1000; // the transport's subscription ids keep rising across connections
  native.config = { ...native.config, settings: { ...obj(native.config.settings), enableDeviceSupport: false, deviceHosts: [] } };
  for (let turn = 0; turn < 8; turn++) { native.woke = false; await answer('snapshot', [], null, files, native); if (!native.woke) break; }
  await answer('shellView', [0, Date.parse('2026-10-09T08:00:00.000Z'), '', false, false, true, 'visible'], null, files, native);
  for (let turn = 0; turn < 8; turn++) { native.woke = false; await answer('snapshot', [], null, files, native); if (!native.woke) break; }
  expect(native.calls.filter(call => call.op === 'subscribe' && call.method === 'subscribeDeviceState').length).toBe(1);
  return { native, files };
}

afterEach(() => { resetPrimary(); resetHighlightSlicing(); });

describe('Settings › Integrations reads its config, settings and device state as the reference does', () => {
  test('opening the page answers once with no server reads, and its answer never asks it again', async () => {
    const { native, files } = await launched();
    const before = native.calls.length;
    const opened = await run(native, files, true);
    // Before the fix: every answer sent getConfig, getSettings and device.list, the inspection woke the runner, and the
    // page was asked on every turn until the limit (12 asks, 36 reads, never idle).
    expect({ asks: opened.asks, idle: opened.idle, reads: native.reads(before) }).toEqual({ asks: 1, idle: true, reads: [] });
    expect(opened.page).toMatchObject({ available: true, error: '', hubStatus: 'v0.3.0', agentStatus: 'v1.2.0' });
    expect(native.calls.slice(before).some(call => call.op === 'subscribe')).toBe(false); // the shell's stream serves the page
    // Closing and opening it again: once more, still no reads.
    await run(native, files, false);
    const again = native.calls.length, reopened = await run(native, files, true);
    expect({ asks: reopened.asks, idle: reopened.idle, reads: native.reads(again) }).toEqual({ asks: 1, idle: true, reads: [] });
  });

  test('it changes on the events the reference listens to: settingsUpdated, a device state, and Check versions', async () => {
    const { native, files } = await launched();
    await run(native, files, true);
    const start = native.calls.length;
    // subscribeServerConfig's settingsUpdated (useScopedSettings): the page shows the new settings without reading them.
    native.config.settings = { ...obj(native.config.settings), deviceHosts: [{ id: 'mini', label: 'Mini', target: 'me@mini' }] };
    native.emit('config', { type: 'settingsUpdated', payload: { settings: native.config.settings } });
    let view = await run(native, files, true);
    expect({ asks: view.asks, idle: view.idle, hosts: view.page.hosts }).toEqual({ asks: 1, idle: true, hosts: 1 });
    // subscribeDeviceState (useDeviceState): a new tool version shows.
    native.hub = { requiredVersion: '0.4.0', installedVersions: ['0.4.0'] }; native.deviceRevision++;
    native.emit(DEVICE_STATE_KEY, native.deviceState());
    view = await run(native, files, true);
    expect({ asks: view.asks, idle: view.idle, hub: view.page.hubStatus }).toEqual({ asks: 1, idle: true, hub: 'v0.4.0' });
    expect(native.reads(start)).toEqual([]);
    // "Check versions" is the one inspection (versionActions): one device.list, then the page settles on the stream's state.
    native.hub = { requiredVersion: '0.4.0', installedVersions: ['0.4.0'], runningVersion: '0.4.1' };
    await answer('command', ['rest:device-tools', 'env1:', 'action=check', 0], null, files, native);
    view = await run(native, files, true);
    expect(native.reads(start)).toEqual(['device.list']);
    expect(arr(native.calls.slice(start).filter(call => call.method === 'device.list')).map(call => obj(call.payload))).toEqual([{ inspectOnly: true }]);
    expect({ idle: view.idle, hub: view.page.hubStatus }).toEqual({ idle: true, hub: 'v0.4.1' });
  });
});
