import { requireRpcPermissions } from '../connections/command-permissions';
// Settings › Projects over every environment (T3 Code 1e2ecbd975, MIT, see LICENSE-T3): ProjectSettingsPanel reads
// useSettingsProjectGroups, the project groups of every environment, and writes each member through its own environment
// (projectEnvironment.update / .delete with `member.environmentId`, the scoped settings per environment). Project
// settings on a project only a background environment has opens this page without changing the focused environment.
//
// A member of the focused environment keeps its project id; a background environment's member is named
// `fleet:<environment>:<project>` (settings-b-fleet.ts fleetThreadId), as the sidebar names a background environment's rows.
import { obj, str, type Obj } from '../client/domain';
import { bridgeReply, ClientError, type Native, type Files } from '../client/protocol';
import type { T3Client } from '../client/client';
import { fleet, fleetThreadId, parseFleetThreadId, EnvironmentFleet, type FleetEntry } from '../connections/settings-b-fleet';
import { scopeProjectGroups } from './settings-scope-sources';

export type ProjectsGroup = { key: string; name: string; members: Obj[] };
type Grouping = Pick<T3Client, 'environmentId' | 'config' | 'ready' | 'projectGroups'> & Partial<Pick<T3Client, 'origin' | 'local'>>;

/** useSettingsProjectGroups: the focused environment's groups joined by each synchronized background environment's projects. */
export function settingsGroups(client: Grouping, source: EnvironmentFleet = fleet): ProjectsGroup[] {
  return scopeProjectGroups(client, source).map(group => ({ key: group.projectKey, name: group.displayName,
    members: group.memberProjects.map(member => (member.environmentId === client.environmentId ? { ...member } : { ...member, id: fleetThreadId(member.environmentId, member.id) }) as Obj) }));
}
/** The group holding a member (its project id, or `fleet:<environment>:<project>`). */
export function groupOfMember(client: Grouping, id: string, source: EnvironmentFleet = fleet): ProjectsGroup | undefined {
  return id ? settingsGroups(client, source).find(group => group.members.some(member => member.id === id)) : undefined;
}
/** A member's environment and its own project id. */
export function memberRef(client: Pick<T3Client, 'environmentId'>, id: string): { environmentId: string; projectId: string } {
  const ref = parseFleetThreadId(id);
  return ref ? { environmentId: ref.environmentId, projectId: ref.threadId } : { environmentId: client.environmentId, projectId: id };
}
/** Members by environment, each with its own project ids, in their first appearance's order. */
export function byEnvironment(client: Pick<T3Client, 'environmentId'>, ids: string[]): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const id of ids) {
    const ref = memberRef(client, id);
    out.set(ref.environmentId, [...(out.get(ref.environmentId) ?? []), ref.projectId]);
  }
  return out;
}

const synchronizedEntry = (source: EnvironmentFleet, environmentId: string): FleetEntry | undefined =>
  [...source.entries.values()].find(entry => entry.environmentId === environmentId && entry.phase === 'connected' && entry.synchronized === entry.generation);

/**
 * The client a write or read of one environment goes through: T3Client for the focused environment; for a background
 * one, a view of T3Client whose connection, config, shell and requests are that environment's fleet transport's.
 * `native` may be absent for a projection that only reads config and shell.
 */
export function environmentClient(client: T3Client, environmentId: string, native?: Native | null, source: EnvironmentFleet = fleet): T3Client {
  if (!environmentId || environmentId === client.environmentId) return client;
  const entry = synchronizedEntry(source, environmentId);
  if (!entry) throw new ClientError('That environment is no longer connected.');
  const call = async (request: Obj): Promise<Obj> => {
    if (!native?.available) throw new ClientError('That environment is no longer connected.');
    requireRpcPermissions(entry.scopes, request);
    const generation = entry.generation;
    const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { ...request, generation });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    if (reply.generation !== generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    return obj(reply.value);
  };
  const request = (method: string, payload: Obj = {}) => call({ op: 'request', method, payload });
  const view = Object.create(client) as T3Client;
  const capabilities = () => obj(obj(entry.config.environment).capabilities);
  Object.defineProperties(view, {
    environmentId: { value: environmentId }, origin: { value: entry.origin }, generation: { value: entry.generation }, connection: { value: 'connected' },
    config: { get: () => entry.config, set: (value: Obj) => { entry.config = value; source.revision++; } },
    shell: { get: () => entry.shell, set: (value: FleetEntry['shell']) => { entry.shell = value; source.revision++; } },
    ready: { get: () => true },
    scopes: { get: () => entry.scopes },
    writable: { get: () => entry.scopes.includes('orchestration:operate') && capabilities().serverResolvedCommandContext === true },
    // T3Client's own reads and writes (`call`, `request`, `http`) go through the environment's transport too.
    call: { value: (_native: Native, value: Obj) => call(value) },
    request: { value: (_native: Native, method: string, payload: Obj) => request(method, payload) },
    http: { value: (_native: Native, path: string) => call({ op: 'http', path }) },
    rpc: { value: (_native: Native, method: string, payload: Obj) => request(method, payload) },
    settingsCoreRequest: { value: (_native: Native, method: string, payload: Obj) => request(method, payload) },
    restAccess: { value: () => ({
      request, read: request, http: (path: string) => call({ op: 'http', path }), call,
      ids: (count: number) => native ? client.ids(native, count) : Promise.reject(new ClientError('That environment is no longer connected.')),
      dispatch: async (_storage: Files, payload: Obj) => { const [commandId] = await client.ids(native!, 1); await request('orchestration.dispatchCommand', { ...payload, commandId }); },
      // A background environment's write is its request; the focused connection's outbox keeps only its own.
      write: async (_storage: Files, pending: { method: string; payload: Obj }) => { await request(pending.method, pending.payload); },
    }) },
  });
  return view;
}

/** The environment's label as Settings names it (EnvironmentRow), for a member's line and the removal's wording. */
export function environmentLabel(client: T3Client, environmentId: string, source: EnvironmentFleet = fleet): string {
  const config = environmentId === client.environmentId ? client.config : synchronizedEntry(source, environmentId)?.config ?? {};
  return str(obj(config.environment).label, 'Environment');
}
/** A member's threads in its own environment's shell. */
export function memberThreads(client: T3Client, id: string, source: EnvironmentFleet = fleet): Obj[] {
  const ref = memberRef(client, id);
  const shell = ref.environmentId === client.environmentId ? client.shell : synchronizedEntry(source, ref.environmentId)?.shell;
  return (shell?.threads ?? []).filter(thread => str(thread.projectId) === ref.projectId);
}

/** The settings pages keyed by environment and project (`args[0]`, `args[1]`): a background environment's project reads that environment. */
export const PROJECT_SCOPED_PAGES = new Set(['archivedSettings', 'storageSettings', 'scheduledSettings', 'sourceControlPage', 'integrationsPage']);
/** The client such a page reads through, and its arguments: a `fleet:` project's become that environment's and its own project id. */
export function settingsPageClient(client: T3Client, native: Native | null | undefined, args: unknown[], source: EnvironmentFleet = fleet): { client: T3Client; args: unknown[] } {
  const ref = parseFleetThreadId(String(args[1] || ''));
  if (!ref || !synchronizedEntry(source, ref.environmentId)) return { client, args };
  return { client: environmentClient(client, ref.environmentId, native, source), args: [ref.environmentId, ref.threadId, ...args.slice(2)] };
}
