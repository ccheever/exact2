// Integrations › Devices across the settings scope (MIT T3 Code 1e2ecbd975, LICENSE-T3:
// IntegrationsSettings.tsx DeviceIntegrationSettings / DeviceIntegrationControls, ScopedSwitch).
// The Device hub and Agent device access switches read the scope's representative target,
// draw mixed when the selected targets disagree, and write every selected environment:
// device.configure per environment at environment scope (all awaited; "Device settings not
// saved on all environments"), the project override through planScopedSettingsPatch at
// project scope.
import { obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { resolveScope, serverContext, settingPlan, type CoreScope } from './settings-core';
import { persistScopedSettingsPatch, scopedPlanNotice, scopedSettingsAreMixed } from './scoped-settings-plan';
import { environmentRequest, postScopedNotice, scopedWriter, type ScopeEnvironment } from './settings-scope-sources';
import { pushToast } from './toast';

const DEVICE_KEYS = ['enableDeviceSupport', 'enableAgentDeviceAccess'] as const;
type DeviceKey = typeof DEVICE_KEYS[number];

/** The scope's device facts: the representative's values, mixed per key, any hub on, and the key the switches send. */
export function deviceScope(client: T3Client, machine: string, projectKey: string, checkout: string) {
  const scope = resolveScope(client, machine, projectKey, checkout);
  const targets = serverContext(client, scope, new Map()).targets.map(target => ({ ...target, settings: { enableDeviceSupport: false, enableAgentDeviceAccess: false, ...target.settings } }));
  const connected = scope.selected.filter(environment => environment.connection.phase === 'connected' && environment.serverConfig);
  const first = targets[0]?.settings;
  return {
    scopeKey: `${machine}|${projectKey}|${checkout}`,
    project: scope.kind === 'project' || scope.kind === 'checkout',
    unavailable: scope.kind === 'unavailable',
    connectedCount: connected.length,
    checked: { enableDeviceSupport: first?.enableDeviceSupport === true, enableAgentDeviceAccess: first?.enableAgentDeviceAccess === true } as Record<DeviceKey, boolean>,
    mixed: { enableDeviceSupport: scopedSettingsAreMixed(targets, ['enableDeviceSupport']), enableAgentDeviceAccess: scopedSettingsAreMixed(targets, ['enableAgentDeviceAccess']) } as Record<DeviceKey, boolean>,
    anyHubEnabled: connected.some(environment => obj(environment.serverConfig?.settings).enableDeviceSupport === true),
  };
}

const parseKey = (key: string) => { const [machine = '', projectKey = '', checkout = ''] = key.split('|'); return { machine, projectKey, checkout }; };

/** One request through the environment's own transport, any method (the device ops are not settings reads). */
function deviceRequest(client: T3Client, native: Native, environment: ScopeEnvironment, method: string, payload: Obj): Promise<Obj> {
  const access = client.restAccess(native);
  return environmentRequest({ config: client.config, settingsCoreRequest: (_native, name, body, write) => access.request(name, body, write) }, native, environment, method, payload, true);
}

/** The Integrations device switches' write across the scope; returns '' (toasts carry partial failures). */
export async function deviceScopedCommand(client: T3Client, native: Native, scopeKey: string, key: string, value: string): Promise<string> {
  if (!(DEVICE_KEYS as readonly string[]).includes(key)) throw new ClientError('Unsupported device setting.');
  const { machine, projectKey, checkout } = parseKey(scopeKey);
  const scope: CoreScope = resolveScope(client, machine, projectKey, checkout);
  const checked = value === 'true';
  if (scope.kind === 'project' || scope.kind === 'checkout') {
    // Agent device access is project-scoped; the hub stays environment-wide (planScopedSettingsPatch refuses it).
    const plan = settingPlan(client, scope, key, checked, false);
    const refused = scopedPlanNotice(plan);
    if (refused) { postScopedNotice(client, refused); return ''; }
    const result = await persistScopedSettingsPatch(plan, scopedWriter(client as never, native, scope.environments), () => {});
    postScopedNotice(client, scopedPlanNotice(plan, result));
    return '';
  }
  if (scope.kind === 'unavailable') { postScopedNotice(client, { kind: 'warning', title: 'Setting not saved', description: scope.message }); return ''; }
  const facts = deviceScope(client, machine, projectKey, checkout);
  if (key === 'enableAgentDeviceAccess' && !facts.anyHubEnabled) throw new ClientError('Enable the device hub before changing agent device access.');
  const input = key === 'enableDeviceSupport' ? { enabled: checked, ...(checked ? { onboardingCompleted: true } : { agentAccessEnabled: false }) } : { agentAccessEnabled: checked };
  // DeviceIntegrationControls update(): every selected environment, all awaited; a disconnected one fails.
  const results = await Promise.allSettled(scope.selected.map(async environment => {
    if (environment.connection.phase !== 'connected' || !environment.serverConfig) throw new Error('Environment disconnected');
    await deviceRequest(client, native, environment, 'device.configure', input);
    const settings = await deviceRequest(client, native, environment, 'server.getSettings', {});
    if (!environment.fleetKey) client.config = { ...client.config, settings };
  }));
  const failed = scope.selected.filter((_, index) => results[index]?.status !== 'fulfilled');
  if (failed.length > 0) {
    pushToast(client, { kind: 'error', title: 'Device settings not saved on all environments', description: `Could not update ${failed.map(environment => str(environment.label)).join(', ')}.` });
  }
  return '';
}
