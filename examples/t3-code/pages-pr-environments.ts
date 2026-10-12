// The servers a pull request can be read on (pr-links-previews-and-routing): the focused connection
// (T3Client) and every connected background environment (settings-b-fleet.ts), each with its config,
// its shell and a way to send it a request. This is the clone's side of T3 Code's EnvironmentRegistry
// as the pull request code uses it (MIT, see LICENSE-T3: packages/client-runtime/src/connection/
// {registry,githubRoutingPermissions}.ts, state/pullRequestRouting.ts): the router asks it which
// servers are connected, local and trusted, and sends each routed request through it.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { EnvironmentFleet, fleet, type FleetEntry } from './settings-b-fleet';
import { githubSharing } from './connections';
import { balancePrefs } from './auto-balance';
import { pullRequestRouter, type PullRequestRouterHost, type RoutedEnvironment, type RoutingPermission } from './pages-pr-routing';

export type PrEnvironment = {
  id: string; label: string; machine: string; focused: boolean; local: boolean; enabled: boolean; connected: boolean;
  permission: RoutingPermission; config: Obj; projects: Obj[]; threads: Obj[];
  /** The background transport's key (`origin\nid`), '' for the focused connection. */
  key: string;
};

const MACHINES = ['server', 'cloud', 'linux', 'desktop', 'laptop', 'mac-mini', 'mac-studio'];
/** EnvironmentMachineKind: the chosen icon, else the platform's machine, else a server. */
function machineOf(config: Obj): string {
  const icon = str(obj(config.settings).environmentIcon), machine = str(obj(obj(config.environment).platform).machine);
  return MACHINES.includes(icon) ? icon : MACHINES.includes(machine) ? machine : 'server';
}
const permissionOf = (value: string | undefined): RoutingPermission => value === 'read' || value === 'read-write' ? value : 'off';
const ready = (entry: FleetEntry) => entry.phase === 'connected' && entry.synchronized === entry.generation;

/** Every server this window reads, the focused one first; a background one once it is connected and synchronized. */
export function prEnvironments(client: T3Client, source: EnvironmentFleet = fleet): PrEnvironment[] {
  const sharing = githubSharing(client, source.saved, source.entries, balancePrefs(client));
  const list: PrEnvironment[] = [];
  if (client.environmentId) {
    const own = sharing.get(client.environmentId);
    list.push({ id: client.environmentId, label: str(obj(client.config.environment).label, 'This environment'), machine: machineOf(client.config), focused: true,
      local: own?.local ?? false, enabled: own?.enabled ?? true, connected: client.ready, permission: permissionOf(own?.permission),
      config: client.config, projects: client.shell.projects, threads: client.shell.threads, key: '' });
  }
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || entry.environmentId === client.environmentId || list.some(item => item.id === entry.environmentId)) continue;
    const facts = sharing.get(entry.environmentId);
    list.push({ id: entry.environmentId, label: str(obj(entry.config.environment).label, 'Environment'), machine: machineOf(entry.config), focused: false,
      local: entry.primary === true || (facts?.local ?? false), enabled: facts?.enabled ?? true, connected: ready(entry), permission: permissionOf(facts?.permission),
      config: entry.config, projects: entry.shell.projects, threads: entry.shell.threads, key: entry.key });
  }
  return list;
}
export function prEnvironment(client: T3Client, environmentId: string, source: EnvironmentFleet = fleet): PrEnvironment | null {
  return prEnvironments(client, source).find(environment => environment.id === (environmentId || client.environmentId)) ?? null;
}
/** The pull request capability a server advertises (`pullRequests`). */
export const readsPullRequests = (environment: PrEnvironment) => obj(obj(environment.config.environment).capabilities).pullRequests === true;

/**
 * One request on one server: the focused connection's own (its generation and write rules), or a background
 * transport's (`fleet: <key>` at its generation). `timeoutSeconds` is the transport's per-request deadline
 * (T3Transport `timeout`), how the reference's 2 s probes and 1 s invalidations are bounded here (X19: no timer
 * in the data module).
 */
export async function environmentRequest(client: T3Client, native: Native, environmentId: string, method: string, payload: Obj,
  options: { write?: boolean; timeoutSeconds?: number; share?: boolean } = {}, source: EnvironmentFleet = fleet): Promise<Obj> {
  const timeout = options.timeoutSeconds === undefined ? {} : { timeout: options.timeoutSeconds };
  // `share`: a read that joins an identical one still pending on that transport (T3Transport `share`).
  const share = options.share && options.write !== true ? { share: true } : {};
  if ((!environmentId || environmentId === client.environmentId) && !options.timeoutSeconds && !share.share) {
    return client.request(native, method, payload, client.generation, options.write === true);
  }
  return environmentCall(client, native, environmentId, { op: 'request', method, payload, ...timeout, ...share }, options.write === true, source);
}
/**
 * Any transport op (`request`, the diff's `prDiff`) on one server: the focused connection's, or a background
 * transport's at its generation (realinput-1010e-followups RE-5: a pull request's own server, not the focus).
 */
export async function environmentCall(client: T3Client, native: Native, environmentId: string, request: Obj, write = false, source: EnvironmentFleet = fleet): Promise<Obj> {
  if (!environmentId || environmentId === client.environmentId) return client.call(native, request, client.generation, write);
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry || !ready(entry)) throw new ClientError('The environment was removed.', 'EnvironmentRpcUnavailableError');
  const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { ...request, generation: entry.generation });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain, { reason: reply.error!.reason, detail: reply.error!.detail });
  if (reply.generation !== entry.generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
  return obj(reply.value);
}
/** A background server's transport (its fleet key and generation) when `environmentId` names a connected one; null for the focus. */
export function environmentTarget(client: T3Client, environmentId: string, source: EnvironmentFleet = fleet): { key: string; generation: number } | null {
  if (!environmentId || environmentId === client.environmentId) return null;
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  return entry && ready(entry) ? { key: entry.key, generation: entry.generation } : null;
}

/** The router's view of the servers (PullRequestRouterHost), read afresh at each decision. */
export function routerHost(client: T3Client, native: Native, originId: string, source: EnvironmentFleet = fleet, share = false): PullRequestRouterHost {
  return {
    originId: originId || client.environmentId,
    environments: (): RoutedEnvironment[] => prEnvironments(client, source).map(environment => ({ id: environment.id, enabled: environment.enabled, connected: environment.connected,
      local: environment.local, permission: environment.permission, pullRequestChecks: obj(obj(environment.config.environment).capabilities).pullRequestChecks === true })),
    request: (environmentId, method, payload, options) => environmentRequest(client, native, environmentId, method, payload, { ...options, share }, source),
  };
}

/**
 * Every pull request request goes through here (T3Client.rpc): `environmentId` in the payload names the server the
 * pull request was listed on (a background one's row), which is stripped before it is sent; the router then reads
 * or writes through whichever server the reader shares GitHub with (pullRequestRouting.ts).
 */
export function routedPullRequestRequest(client: T3Client, native: Native, method: string, payload: Obj, write: boolean, share = false): Promise<Obj> {
  const { environmentId, ...rest } = payload;
  // An invalidation names its pull request inside `reference`; the marker rides there too.
  const reference = obj(rest.reference), nested = str(reference.environmentId);
  if (nested) { const { environmentId: _nested, ...plain } = reference; rest.reference = plain; }
  return pullRequestRouter.request(routerHost(client, native, pullRequestServer(client, str(environmentId) || nested, rest), fleet, share), method, rest, write);
}
/** The server a pull request request belongs to: the one it was listed on, else the one its project lives on. */
export const pullRequestServer = (client: T3Client, environmentId: string, payload: Obj, source: EnvironmentFleet = fleet): string =>
  environmentId || projectServer(client, payload, source);
/**
 * The server a request's project lives on when the payload does not say (a write built from the panel's reference, a row's
 * quick action): project ids are per server, so a project only a background server holds names that server; the focused
 * connection's own projects, or none, keep the focus.
 */
export function projectServer(client: T3Client, payload: Obj, source: EnvironmentFleet = fleet): string {
  const projectId = str(payload.projectId) || str(obj(payload.reference).projectId) || str(obj(arr(payload.refs)[0]).projectId);
  if (!projectId || client.shell.projects.some(project => str(project.id) === projectId)) return '';
  for (const entry of source.entries.values()) if (ready(entry) && entry.shell.projects.some(project => str(project.id) === projectId)) return entry.environmentId;
  return '';
}
