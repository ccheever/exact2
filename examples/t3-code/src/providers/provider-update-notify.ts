// The environments a provider update is sent to (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3):
// the focused one, a background one (settings-b-fleet.ts) and the primary, the embedded server
// ("This machine"), whose providers the sidebar pill reads (`primaryServerProvidersAtom`). With no
// primary (the Local environment off, a remote-only or refused launch) there is none. Provider
// updates pop no toast at launch: Update all and the sidebar pill offer them.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native } from '../client/protocol';
import { fleet, EnvironmentFleet, type FleetEntry } from '../connections/settings-b-fleet';
import { isPrimaryEnvironment, primary } from '../connections/local-primary';

/** A server.updateProvider can run the provider's installer: wait up to 15 minutes, not the transport's 30 s. */
export const UPDATE_TIMEOUT_SECONDS = 900;
/** An environment that can be asked for an update: its providers, its label, and a request on its transport. */
export interface UpdateTarget { key: string; environmentId?: string; label: string; providers: Obj[]; request(method: string, payload: Obj): Promise<Obj> }
type Client = Pick<T3Client, 'ready' | 'environmentId' | 'config' | 'generation' | 'call'>;

/** The focused environment as an update target. */
export function focusedTarget(client: Client, native: Native | null): UpdateTarget {
  return { key: `focus:${client.environmentId}`, environmentId: client.environmentId, label: str(obj(client.config.environment).label, 'Environment'), providers: arr(client.config.providers),
    request: (method, payload) => {
      if (!native) throw new ClientError('Open this app on macOS to update providers.');
      return client.call(native, { op: 'request', method, payload, timeout: UPDATE_TIMEOUT_SECONDS }, client.generation, true);
    } };
}
/** A connected background environment (settings-b-fleet.ts) as an update target. */
export function fleetTarget(entry: FleetEntry, native: Native | null, label = ''): UpdateTarget {
  return { key: entry.key, environmentId: entry.environmentId, label: label || str(obj(entry.config.environment).label, 'Environment'), providers: arr(entry.config.providers),
    request: async (method, payload) => {
      if (!native) throw new ClientError('Open this app on macOS to update providers.');
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method, payload, generation: entry.generation, timeout: UPDATE_TIMEOUT_SECONDS });
      if (!reply.ok) throw new ClientError(reply.error?.message || 'Provider update failed.', reply.error?.kind ?? 'rpc', reply.error?.uncertain === true);
      if (reply.generation !== entry.generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale', true);
      return obj(reply.value);
    } };
}
/** usePrimaryEnvironment + primaryServerProvidersAtom: the embedded server, wherever it is connected; null without one. */
export function primaryTarget(client: Client, native: Native | null): UpdateTarget | null {
  if (!primary.target?.environmentId) return null;
  if (client.ready && isPrimaryEnvironment(client.environmentId)) return focusedTarget(client, native);
  const entry = [...fleet.entries.values()].find(candidate => candidate.primary === true && candidate.phase === 'connected' && Object.keys(candidate.config).length > 0);
  return entry ? fleetTarget(entry, native, primary.target?.label || '') : null;
}
