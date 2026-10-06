// Integrations › Devices across the settings scope (IntegrationsSettings.tsx DeviceIntegrationControls,
// ScopedSwitch): the two device switches read every selected environment, draw mixed when they
// disagree, and write each environment (device.configure, all awaited).
import { beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { applyShell, initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { decodeClientPrefs } from './settings-core';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { deviceScope, deviceScopedCommand } from './settings-integrations-scope';
import { toasts } from './toast';

const capabilities = { projectSettingsOverrides: true };
function focused(settings: Obj = {}) {
  const calls: Obj[] = [];
  const client = {
    local: { deviceSettings: {}, clientSettings: decodeClientPrefs({}), groupingMode: 'repository', groupingOverrides: {} },
    ready: true, environmentId: 'env-a', origin: 'http://127.0.0.1:16100', writable: true, calls,
    config: { environment: { environmentId: 'env-a', label: 'Lane A', capabilities }, settings: { projectSettingsOverrides: {}, ...settings } } as Obj,
    projectGroups: () => [{ key: 'github.com/acme/app', name: 'App', members: [{ id: 'pa', title: 'App', workspaceRoot: '/a/app' }] }],
    async settingsCoreRequest(_native: Native, method: string, payload: Obj) { return client.restAccess().request(method, payload, true); },
    restAccess() {
      return { request: async (method: string, payload: Obj = {}) => {
        calls.push({ method, payload });
        const current = client.config.settings as Obj;
        if (method === 'device.configure') {
          client.config.settings = { ...current, ...(payload.enabled !== undefined ? { enableDeviceSupport: payload.enabled } : {}),
            ...(payload.agentAccessEnabled !== undefined ? { enableAgentDeviceAccess: payload.agentAccessEnabled } : {}) };
          return {};
        }
        if (method === 'server.updateSettings') {
          const overrides = { ...(current.projectSettingsOverrides as Obj), ...((payload.patch as Obj).projectSettingsOverrides as Obj) };
          client.config.settings = { ...current, ...(payload.patch as Obj), projectSettingsOverrides: overrides };
        }
        return client.config.settings as Obj;
      } };
    },
  };
  return client;
}
const as = (client: ReturnType<typeof focused>) => client as unknown as T3Client;
function entry(key: string, environmentId: string, label: string, settings: Obj, phase: FleetEntry['phase'] = 'connected'): FleetEntry {
  const shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: `p-${environmentId}`, title: 'App', workspaceRoot: `/b/${environmentId}`,
    repositoryIdentity: { canonicalKey: 'github.com/acme/app' } }], threads: [] });
  return { key, origin: key.split('\n')[0]!, environmentId, phase, message: '', traceId: '', generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {},
    config: { environment: { environmentId, label, capabilities }, settings: { projectSettingsOverrides: {}, ...settings } }, shell, scopes: [], error: '', requested: true };
}
function nativeFor(calls: Obj[] = [], fail = new Set<string>()): Native {
  return { available: true, watch() {}, async later(request) {
    const call = request as Obj; calls.push(call);
    const target = fleet.entries.get(String(call.fleet))!;
    if (fail.has(target.key)) return { ok: false, generation: target.generation, error: { message: 'Disconnected during save' } };
    const payload = call.payload as Obj;
    if (call.method === 'device.configure') target.config = { ...target.config, settings: { ...(target.config.settings as Obj), enableDeviceSupport: payload.enabled ?? (target.config.settings as Obj).enableDeviceSupport } };
    if (call.method === 'server.updateSettings') target.config = { ...target.config, settings: { ...(target.config.settings as Obj), ...(payload.patch as Obj) } };
    return { ok: true, generation: target.generation, value: target.config.settings };
  } };
}
const B = 'http://127.0.0.1:16101\nenv-b';
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('Integrations device switches across environments', () => {
  test('disagreeing environments draw the device hub mixed; agreeing ones do not', () => {
    const client = focused({ enableDeviceSupport: true });
    fleet.entries.set(B, entry(B, 'env-b', 'Lane B', { enableDeviceSupport: false }));
    const all = deviceScope(as(client), '', '', '');
    expect([all.mixed.enableDeviceSupport, all.mixed.enableAgentDeviceAccess, all.anyHubEnabled, all.connectedCount]).toEqual([true, false, true, 2]);
    expect(deviceScope(as(client), 'env-b', '', '')).toMatchObject({ checked: { enableDeviceSupport: false }, mixed: { enableDeviceSupport: false } });
  });
  test('one press turns the hub on in every selected environment through its own transport', async () => {
    const client = focused({ enableDeviceSupport: true });
    fleet.entries.set(B, entry(B, 'env-b', 'Lane B', { enableDeviceSupport: false }));
    const calls: Obj[] = [];
    await deviceScopedCommand(as(client), nativeFor(calls), '||', 'enableDeviceSupport', 'true');
    expect(client.calls.filter(call => call.method === 'device.configure').map(call => call.payload)).toEqual([{ enabled: true, onboardingCompleted: true }]);
    expect(calls.filter(call => call.method === 'device.configure').map(call => [call.fleet, call.payload])).toEqual([[B, { enabled: true, onboardingCompleted: true }]]);
    expect(deviceScope(as(client), '', '', '').mixed.enableDeviceSupport).toBe(false);
  });
  test('a failed environment is named; a project scope writes the agent access override per member', async () => {
    const client = focused({ enableDeviceSupport: true });
    fleet.entries.set(B, entry(B, 'env-b', 'Lane B', {}));
    await deviceScopedCommand(as(client), nativeFor([], new Set([B])), '||', 'enableDeviceSupport', 'false');
    expect(toasts(as(client)).at(-1)).toMatchObject({ kind: 'error', title: 'Device settings not saved on all environments', description: 'Could not update Lane B.' });
    const calls: Obj[] = [];
    await deviceScopedCommand(as(client), nativeFor(calls), '|github.com/acme/app|', 'enableAgentDeviceAccess', 'true');
    expect(client.calls.at(-1)).toEqual({ method: 'server.updateSettings', payload: { patch: { projectSettingsOverrides: { pa: { enableAgentDeviceAccess: true } } } } });
    expect(calls.map(call => (call.payload as Obj).patch)).toEqual([{ projectSettingsOverrides: { 'p-env-b': { enableAgentDeviceAccess: true } } }]);
  });
});
