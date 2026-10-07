// Simulator support before the device hub is ready (T3 Code 1e2ecbd975, 5318d054a5;
// MIT, see LICENSE-T3): components/device/DeviceSetup.tsx `localPlatformsUnavailable`
// (wizard step 0 also shows "Check simulator support") and
// components/settings/IntegrationsSettings.tsx DeviceIntegrationControls (the
// "Simulator support" row with compact PlatformStatus and Refresh).
// exact2: the row's reveal flag is kept per client and environment here, as the
// reference keeps it in component state; it resets when the hub is off. The
// reference animates the reveal (AnimatedHeight); layout interpolation is
// unsupported, so the row shows at once.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { localPlatformsUnavailable, platformSetupStatus, type PlatformStatus } from './r4-surfaces-device';
export { localPlatformsUnavailable } from './r4-surfaces-device';
import { fleet } from './settings-b-fleet';

export type SimulatorSupportRow = { key: string; platforms: PlatformStatus[]; description: string; disabled: boolean };

const revealed = new WeakMap<T3Client, Map<string, boolean>>();
let checks = 0;

/**
 * The Integrations row: revealed when the hub is on and either the host is ready
 * or the local host has no available platform; it stays revealed while the hub
 * stays on (later agent setup and refresh phases), and resets when it turns off.
 * Returns [] while hidden, else one row (the key changes after each Refresh so
 * the row's "Checking…" state starts over).
 */
export function simulatorSupportRows(client: T3Client, environmentId: string, enabled: boolean, state: Obj | null, connected: number): SimulatorSupportRow[] {
  let map = revealed.get(client);
  if (!map) { map = new Map(); revealed.set(client, map); }
  const key = environmentId || client.environmentId;
  if (!enabled) { map.delete(key); return []; }
  const hostStatus = str(state?.hostStatus);
  if (!map.get(key) && (hostStatus === 'ready' || localPlatformsUnavailable(state))) map.set(key, true);
  if (!map.get(key) || !state) return [];
  const busy = hostStatus === 'installing' || hostStatus === 'starting';
  const label = str(obj(client.config.environment).label);
  return [{
    key: `${key}:${checks}`,
    platforms: (['ios', 'android'] as const).map(platform => ({ id: platform, platform: platform === 'ios' ? 'iOS' : 'Android', ...platformSetupStatus(state, platform) })),
    description: connected > 1 ? `Status for ${label}. Select an environment to inspect its simulator support.` : '',
    disabled: busy,
  }];
}

/** useSettingsScope connectedEnvironments: the focused connection plus every connected background environment. */
export function connectedEnvironmentCount(client: T3Client): number {
  let count = client.ready ? 1 : 0;
  for (const entry of fleet.entries.values()) if (entry.phase === 'connected') count++;
  return count;
}

/** rest:device-platforms — the row's Refresh: `device.list {}` (IntegrationsSettings.tsx). */
export async function devicePlatformsCommand(client: T3Client, native: Native): Promise<string> {
  try { await client.restAccess(native).request('device.list', {}, true); }
  catch { /* useAtomCommand({ reportFailure: false }): the row keeps its last status. */ }
  finally { checks++; }
  return '';
}
