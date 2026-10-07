// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
// Shared transport/snapshot ownership; pinned mobile auth.ts sessionGrantsScope.
import { mobileClient, mobileNative } from './client';
import { environmentSources } from './shared/connections';
import { savedList } from './shared/connection-routes-ops';
import { obj, str, type Obj } from './shared/domain';
import { letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';

export type MobileSettingsSource = ReturnType<typeof environmentSources>[number];
export type MobileSettingsEndpoint = { source: MobileSettingsSource; remote: Native; generation: number };
export const settingsNative = (native: Native) => letGoAware(mobileNative(native));
export function settingsGrants(session: Obj, permission: 'settings:write' | 'providers:manage'): boolean {
  if (session.authenticated !== true) return false;
  if (session.permissions !== undefined) return Array.isArray(session.permissions) && session.permissions.includes(permission);
  const scopes = Array.isArray(session.scopes) ? session.scopes : [];
  return scopes.includes(permission) || (obj(session.auth).serverUpdateScope === undefined && scopes.includes('orchestration:operate'));
}
export async function settingsSources(native: Native): Promise<MobileSettingsSource[]> {
  native.watch('t3.status'); native.watch('t3.fleet');
  const saved = await savedList(native);
  return environmentSources(mobileClient, saved, fleet.entries).map(source => ({ ...source,
    label: str(saved.find(item => item.environmentId === source.environmentId)?.mobileLabel) || source.label }));
}
export function settingsEndpoint(source: MobileSettingsSource, native: Native): MobileSettingsEndpoint {
  return { source, remote: source.focused ? native : EnvironmentFleet.native(native, source.key),
    generation: source.focused ? mobileClient.generation : fleet.entries.get(source.key)?.generation ?? -1 };
}
export function settingsEndpointCurrent(endpoint: MobileSettingsEndpoint): boolean {
  const { source, generation } = endpoint;
  return source.enabled && source.phase === 'connected' && (source.focused
    ? mobileClient.environmentId === source.environmentId && mobileClient.generation === generation && mobileClient.connection === 'connected'
    : fleet.entries.get(source.key)?.generation === generation && fleet.entries.get(source.key)?.phase === 'connected');
}
export async function settingsCall(endpoint: MobileSettingsEndpoint, request: Obj): Promise<Obj> {
  if (!settingsEndpointCurrent(endpoint)) throw new ClientError('The connection changed. Reopen this settings page.', 'stale');
  const reply = await bridgeReply(endpoint.remote, { ...request, generation: endpoint.generation });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
  if (reply.generation !== endpoint.generation || !settingsEndpointCurrent(endpoint)) throw new ClientError('The connection changed. Reopen this settings page.', 'stale');
  return obj(reply.value);
}
export function settingsAdoptConfig(endpoint: MobileSettingsEndpoint, config: Obj): void {
  if (!settingsEndpointCurrent(endpoint)) return;
  if (endpoint.source.focused) mobileClient.config = config;
  else { const entry = fleet.entries.get(endpoint.source.key); if (entry) { entry.config = config; fleet.revision++; } }
  endpoint.source.config = config;
}
