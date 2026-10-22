// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/connection-routes-ops.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane environment-routes: the routes of a saved environment in Settings → Connections
// (T3 Code 1e2ecbd975 apps/web/src/components/settings/EnvironmentRoutesList.tsx,
// EnvironmentRow.tsx environmentTransportLabel, ConnectionsSettings.tsx "Add a route to";
// client-runtime registry.ts registerRoute / reorderRoutes / removeRoute and supervisor.ts
// learnRoutesFrom; MIT, see LICENSE-T3). The pure model is connection-routes.ts; the native
// store and the walk are T3SavedEnvironments and T3Routes.swift.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import {
  connectionRouteAddress, connectionRouteLabel, isLearned, mergeLearnedRoutes, registerRoute, reorderRoutes, routesAfterRemoving,
  savedRoutes, storedRoute, type ConnectionRoute, type RouteSshTarget,
} from './connection-routes';

const trim = (origin: string) => origin.trim().replace(/\/+$/, '');

async function call(native: Native, request: Obj) {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return reply;
}
export async function savedList(native: Native): Promise<Obj[]> { return arr(obj((await call(native, { op: 'environments' })).value).saved); }
export const savedEntry = (saved: Obj[], environmentId: string) => saved.find(entry => str(entry.environmentId) === environmentId);
export async function writeRoutes(native: Native, environmentId: string, routes: ConnectionRoute[]): Promise<Obj[]> {
  return arr(obj((await call(native, { op: 'setRoutes', environmentId, routes: routes.map(storedRoute) as unknown as Obj[] })).value).saved);
}

/**
 * A paired address the native store appended (pairing the same machine again): placed by
 * kind (registerRoute), an SSH tunnel marked as one. Nothing changes when it already sits there.
 */
export async function placeRoute(native: Native, environmentId: string, origin: string, ssh?: RouteSshTarget | null): Promise<void> {
  const entry = savedEntry(await savedList(native), environmentId);
  if (!entry) return;
  const routes = savedRoutes(entry), address = trim(origin);
  const added = routes.find(route => trim(route.origin) === address && !isLearned(route));
  if (!added) return;
  const route: ConnectionRoute = { id: added.id, origin: added.origin, kind: ssh ? 'ssh' : added.kind ?? '', ...(ssh ? { ssh } : added.ssh ? { ssh: added.ssh } : {}) };
  const next = registerRoute(routes.filter(other => other !== added), route);
  if (next.map(storedKey).join('\n') !== routes.map(storedKey).join('\n')) await writeRoutes(native, environmentId, next);
}
const storedKey = (route: ConnectionRoute) => `${route.id} ${route.kind ?? ''} ${route.ssh?.alias ?? ''}`;

type Learner = { entries: Map<string, { environmentId: string; phase: string; config: Obj; activeRouteId?: string }>; saved: Obj[] };
type FocusLike = { environmentId: string; connection?: string; config?: Obj };
/**
 * learnRoutesFrom: after each server config, the addresses it reports (directEndpoints) become
 * learned routes that borrow the credential of the route in use. Runs for the focused
 * environment and every connected background one; writes only when the routes change.
 */
export async function learnRoutes(native: Native, fleet: Learner, focused: FocusLike): Promise<void> {
  const connected: { environmentId: string; config: Obj; activeRouteId: string }[] = [];
  if (focused.environmentId && focused.connection === 'connected' && focused.config && Array.isArray(focused.config.directEndpoints)) {
    // The focused transport names the route it connected over in its status.
    const status = await bridgeReply(native, { op: 'status' });
    if (status.ok) connected.push({ environmentId: focused.environmentId, config: focused.config, activeRouteId: str(obj(status.value).activeRouteId) });
  }
  for (const entry of fleet.entries.values()) {
    if (entry.phase === 'connected' && Object.keys(entry.config).length) connected.push({ environmentId: entry.environmentId, config: entry.config, activeRouteId: str(entry.activeRouteId) });
  }
  for (const live of connected) {
    const reported = arr(live.config.directEndpoints).map(endpoint => ({ httpBaseUrl: str(endpoint.httpBaseUrl) })).filter(endpoint => endpoint.httpBaseUrl);
    if (!Array.isArray(live.config.directEndpoints)) continue;
    const saved = savedEntry(fleet.saved, live.environmentId);
    if (!saved) continue;
    const routes = savedRoutes(saved);
    const activeRoute = routes.find(route => route.id === live.activeRouteId);
    if (!activeRoute) continue;
    const next = mergeLearnedRoutes({ entry: { environmentId: live.environmentId, label: str(saved.label), routes }, activeRoute, reported, allowInsecure: true });
    if (next) fleet.saved = await writeRoutes(native, live.environmentId, next);
  }
}

// ── Projection (EnvironmentRoutesList, environmentTransportLabel) ──────────
export type RouteRow = { id: string; label: string; address: string; inUse: boolean; removable: boolean; position: number;
  first: boolean; last: boolean; learned: boolean; target: string };
export function routeRows(environmentId: string, routes: ConnectionRoute[], activeRouteId: string, connected: boolean): RouteRow[] {
  return routes.map((route, index) => {
    const address = connectionRouteAddress(route);
    return {
      id: route.id, label: connectionRouteLabel(route), position: index + 1, first: index === 0, last: index === routes.length - 1,
      address: address === null ? '' : `${address}${isLearned(route) ? ' · found automatically' : ''}`,
      inUse: connected && route.id === activeRouteId, learned: isLearned(route),
      // The last route goes with the machine ("Remove from this device"); a learned one would be learned again.
      removable: routes.length > 1 && !isLearned(route),
      target: `route:${environmentId}:${route.id}`,
    };
  });
}
/** environmentTransportLabel with several routes: "via <route>" for the one in use, else the first route's label. */
export function routesTransportLabel(routes: ConnectionRoute[], activeRouteId: string, connected: boolean): string | null {
  if (routes.length <= 1) return null;
  const active = connected ? routes.find(route => route.id === activeRouteId) : undefined;
  return active ? `via ${connectionRouteLabel(active)}` : connectionRouteLabel(routes[0]!);
}
export const routeCountLabel = (count: number) => count <= 1 ? 'Routes' : `${count} routes`;

// ── Writes ─────────────────────────────────────────────────────────────────
/** Reorder (the list's drop, by pointer or keyboard): `value` is `<route id>><id it lands before>` (empty: the end). */
export async function moveSavedRoute(native: Native, environmentId: string, value: string): Promise<void> {
  const mark = value.lastIndexOf('>'), id = value.slice(0, Math.max(0, mark)), before = value.slice(mark + 1);
  if (mark < 1) throw new ClientError('Choose a route to move.');
  const entry = savedEntry(await savedList(native), environmentId);
  if (!entry) throw new ClientError('That environment is no longer saved on this device.');
  const routes = savedRoutes(entry), ids = routes.map(route => route.id);
  const next = dropBefore(ids, id, before);
  if (next.join('\n') !== ids.join('\n')) await writeRoutes(native, environmentId, reorderRoutes(routes, next));
}
/** arrayMove for a drop: `id` lands before `before`, or at the end. */
export function dropBefore(ids: readonly string[], id: string, before: string): string[] {
  if (!ids.includes(id) || id === before) return [...ids];
  const rest = ids.filter(other => other !== id), at = before ? rest.indexOf(before) : -1;
  return at < 0 ? [...rest, id] : [...rest.slice(0, at), id, ...rest.slice(at)];
}
/** Remove one route (and the learned routes that borrow its credential). The last route is never removed here. */
export async function removeSavedRoute(native: Native, environmentId: string, routeId: string): Promise<void> {
  const entry = savedEntry(await savedList(native), environmentId);
  if (!entry) throw new ClientError('That environment is no longer saved on this device.');
  const routes = savedRoutes(entry), route = routes.find(candidate => candidate.id === routeId);
  if (!route) throw new ClientError('That route is no longer saved.');
  if (isLearned(route)) throw new ClientError('A route found automatically cannot be removed.');
  const next = routesAfterRemoving(routes, routeId);
  if (!next.length) throw new ClientError('The last route goes with the machine. Remove the environment instead.');
  await writeRoutes(native, environmentId, next);
}
