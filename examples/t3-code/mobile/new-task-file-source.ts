// Reuse saved/focused/fleet transport ownership for source standalone NewTaskFile.
// @ref llp/1107.006-review-and-files.decision.md#draft-file-ownership
import type { T3Client } from './shared/client';
import { environmentSources } from './shared/connections';
import { savedList } from './shared/connection-routes-ops';
import { EnvironmentFleet, type FleetEntry } from './shared/settings-b-fleet';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { obj, str, type Obj } from './shared/domain';
import { mobileSessionGrants } from './environment-detail';

function backgroundEntry(environmentId: string, background: EnvironmentFleet): FleetEntry | undefined {
  return [...background.entries.values()].find(entry => entry.environmentId === environmentId);
}
/** Live transport identity is available synchronously, so completed reads do not
 * change the root preparation key. The saved catalog is refreshed before reads. */
export function newTaskFileEndpointIdentity(environmentId: string, client: T3Client, background: EnvironmentFleet): string {
  const saved = background.saved.find(entry => entry.environmentId === environmentId);
  if (client.environmentId === environmentId && client.connection !== 'disconnected')
    return JSON.stringify(['focused', environmentId, client.origin, client.generation, client.connection, saved?.enabled !== false]);
  const entry = backgroundEntry(environmentId, background);
  return JSON.stringify(['fleet', environmentId, entry?.key ?? '', entry?.origin ?? '', entry?.generation ?? -1,
    entry?.phase ?? '', entry?.activeRouteId ?? '', saved?.enabled !== false]);
}

/** Native is scoped to this awaited read only. Nothing retains its endpoint handle. */
export async function readStandaloneNewTaskFile(environmentId: string, cwd: string, path: string,
  native: Native, client: T3Client, background: EnvironmentFleet, check: () => void): Promise<Obj> {
  native.watch('t3.status'); native.watch('t3.fleet');
  const saved = await savedList(native); check();
  const source = environmentSources(client, saved, background.entries).find(source => source.environmentId === environmentId);
  if (!source || !source.enabled || source.phase !== 'connected')
    throw new ClientError('Connect to this workspace to view its files.');
  const entry = source.focused ? undefined : background.entries.get(source.key);
  if (!source.focused && (!entry || entry.environmentId !== environmentId)) throw new ClientError('This file connection changed.', 'superseded');
  const generation = source.focused ? client.generation : entry!.generation;
  const origin = source.focused ? client.origin : entry!.origin;
  const route = entry?.activeRouteId ?? '';
  const remote = source.focused ? native : EnvironmentFleet.native(native, source.key);
  const current = () => {
    check();
    const disabled = background.saved.find(item => item.environmentId === environmentId)?.enabled === false;
    const live = background.entries.get(source.key);
    if (disabled || (source.focused
      ? client.environmentId !== environmentId || client.origin !== origin || client.generation !== generation || client.connection !== 'connected'
      : client.environmentId === environmentId && client.connection !== 'disconnected'
        || !live || live.environmentId !== environmentId || live.origin !== origin || live.generation !== generation
        || live.phase !== 'connected' || (live.activeRouteId ?? '') !== route))
      throw new ClientError('This file connection changed.', 'superseded');
  };
  const call = async (input: Obj) => {
    current(); const response = await bridgeReply(remote, { ...input, generation }); current();
    if (response.generation !== generation) throw new ClientError('This file connection changed.', 'superseded');
    if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain);
    return obj(response.value);
  };
  const session = await call({ op: 'http', path: '/api/auth/session' });
  if (!mobileSessionGrants(session, 'filesystem:read')) throw new ClientError('This connection cannot read host files.');
  const result = await call({ op: 'request', method: 'projects.readFile', payload: { cwd, relativePath: path } });
  return { contents: str(result.contents), truncated: result.truncated === true };
}
