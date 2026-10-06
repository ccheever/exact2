// One environment, several routes (T3 Code 1e2ecbd975 packages/client-runtime/src/connection/
// routes.ts and githubRoutingPermissions.ts gitHubRoutingConnectionKey, commits 979ca66ced and
// 745c225f92; MIT, see LICENSE-T3). A saved environment keeps an ordered list of routes: direct
// URLs (loopback, LAN, tailnet, public) and SSH tunnels. The client connects over the first route
// that answers as the same environment (T3Routes.swift walks them) and moves back to a better one
// when it becomes reachable again.
//
// Port changes: routes are plain records, the shape T3SavedEnvironments stores
// (`{ id, origin, kind, learned, credential, authorization?, ssh? }`), not Effect classes. A
// paired route's id is its origin, which is also the Keychain account its token is saved under;
// a learned route's id is `learned:<environment>:<origin>@<owner id>` and borrows the owner's
// token. An SSH route's origin is the tunnel's loopback address, so its kind comes from the stored
// `kind`, never from the address. `allowInsecure` is always true here (a native app is not an
// HTTPS page); the transport still refuses plain HTTP to a non-loopback host (T3Endpoint).
// The relay kind stays for test parity and is never created (this client has no T3 Connect).
import { isLocalLoopbackHost, isPrivateNetworkHost, isTailnetHost } from './host-classification';

export type ConnectionRouteKind = 'relay' | 'loopback' | 'lan' | 'tailnet' | 'public' | 'ssh';
export type RouteSshTarget = { alias: string; hostname: string; username: string | null; port: number | null };
export interface ConnectionRoute {
  /** The connection id: a paired route's origin, a learned route's `learned:…`, or "relay". */
  id: string;
  /** The http(s) base URL without a trailing slash; empty for T3 Connect. */
  origin: string;
  /** The stored kind: "ssh" for a tunnel, "relay" for T3 Connect, else empty (classified by address). */
  kind?: string;
  learned?: boolean;
  authorization?: 't3-connect';
  ssh?: RouteSshTarget;
}
export interface RouteEntry { environmentId: string; label: string; routes: ConnectionRoute[] }

/** An environment has at most one T3 Connect route, so it needs no per-route id. */
export const RELAY_ROUTE_ID = 'relay';
const trimSlash = (url: string) => url.replace(/\/+$/, '');

export function connectionRouteId(route: ConnectionRoute): string { return route.id; }

/** Every route of an entry, preferred first. */
export function connectionRoutes(entry: RouteEntry): ConnectionRoute[] { return entry.routes; }

/** Builds an entry whose preferred route is the first of `routes`, which must not be empty. */
export function entryWithRoutes(entry: RouteEntry, routes: readonly ConnectionRoute[]): RouteEntry {
  if (routes.length === 0) throw new Error('A saved environment needs at least one route.');
  return { ...entry, routes: [...routes] };
}

/** The entry a single route connects with. */
export function routeEntry(entry: RouteEntry, route: ConnectionRoute): RouteEntry { return entryWithRoutes(entry, [route]); }

/** The base URL of a direct route, or null for T3 Connect and SSH. */
export function routeHttpBaseUrl(route: ConnectionRoute): string | null {
  return route.kind === 'relay' || route.kind === 'ssh' || !route.origin ? null : `${trimSlash(route.origin)}/`;
}

function routeHostname(route: ConnectionRoute): string | null {
  const url = routeHttpBaseUrl(route);
  if (url === null) return null;
  try { return new URL(url).hostname; } catch { return null; }
}

export function connectionRouteKind(route: ConnectionRoute): ConnectionRouteKind {
  if (route.kind === 'relay') return 'relay';
  if (route.kind === 'ssh') return 'ssh';
  const hostname = routeHostname(route);
  if (hostname === null) return 'public';
  if (isLocalLoopbackHost(hostname)) return 'loopback';
  if (isTailnetHost(hostname)) return 'tailnet';
  return isPrivateNetworkHost(hostname) ? 'lan' : 'public';
}

const ROUTE_KIND_RANK: Record<ConnectionRouteKind, number> = { loopback: 0, lan: 1, tailnet: 2, public: 3, ssh: 4, relay: 5 };

/**
 * Where a newly added route goes: after every saved route of the same or a
 * faster kind, so LAN lands ahead of tailnet and both ahead of T3 Connect.
 * Users can reorder afterwards; this only picks a sensible starting point.
 */
export function insertRoute(routes: readonly ConnectionRoute[], route: ConnectionRoute): ConnectionRoute[] {
  const rank = ROUTE_KIND_RANK[connectionRouteKind(route)];
  const index = routes.findIndex(existing => ROUTE_KIND_RANK[connectionRouteKind(existing)] > rank);
  return index === -1 ? [...routes, route] : [...routes.slice(0, index), route, ...routes.slice(index)];
}

/** Replaces the route with the same id in place, or inserts it by kind. */
export function upsertRoute(routes: readonly ConnectionRoute[], route: ConnectionRoute): ConnectionRoute[] {
  return routes.some(existing => existing.id === route.id) ? routes.map(existing => existing.id === route.id ? route : existing) : insertRoute(routes, route);
}

/**
 * A saved route that reaches the same address as `route`: the same bearer
 * URL, or the same SSH target (alias, host, user, and port). Registering it
 * again replaces that route, even when it was saved under another id.
 */
export function findRouteToSameAddress(routes: readonly ConnectionRoute[], route: ConnectionRoute): ConnectionRoute | undefined {
  const key = routeAddressKey(route);
  return key === null ? undefined : routes.find(existing => routeAddressKey(existing) === key);
}
function routeAddressKey(route: ConnectionRoute): string | null {
  if (route.kind === 'ssh') return route.ssh ? `ssh:${sshTargetKey(route.ssh)}` : null;
  const url = routeHttpBaseUrl(route);
  return url === null ? null : `bearer:${trimSlash(url)}`;
}

/** Short user-facing route description: "LAN", "Tailscale", "T3 Connect", a URL, or an SSH host. */
export function connectionRouteLabel(route: ConnectionRoute): string {
  switch (connectionRouteKind(route)) {
    case 'relay': return 'T3 Connect';
    case 'loopback': return 'This device';
    case 'lan': return 'LAN';
    case 'tailnet': return 'Tailscale';
    case 'ssh': return route.ssh ? `SSH ${route.ssh.username ? `${route.ssh.username}@` : ''}${route.ssh.hostname}` : 'SSH';
    case 'public': return routeHostname(route) ?? 'Remote link';
  }
}

/** The address shown under a route, or null for T3 Connect. */
export function connectionRouteAddress(route: ConnectionRoute): string | null {
  if (route.kind === 'ssh') return route.ssh ? route.ssh.alias : null;
  return routeHttpBaseUrl(route);
}

/** Whether the environment can be reached through T3 Connect. */
export function hasRelayRoute(entry: Pick<RouteEntry, 'routes'>): boolean {
  return entry.routes.some(route => route.kind === 'relay');
}

/**
 * The routes after the server reports where it listens (server.getConfig
 * directEndpoints). Each newly reported address becomes a learned route that
 * authenticates the same way as the route in use: the paired token for a
 * bearer route, the T3 Connect credential for relay. A learned route the server
 * still reports keeps its place, so the user's order holds; one it no longer
 * reports is dropped, so a changed LAN address replaces the old one. Routes the
 * user saved are never touched, and an address already saved is not learned twice.
 */
export function mergeLearnedRoutes(input: { entry: RouteEntry; activeRoute: ConnectionRoute; reported: readonly { httpBaseUrl: string }[]; allowInsecure: boolean }): ConnectionRoute[] | null {
  const { entry } = input, active = input.activeRoute, saved = connectionRoutes(entry);
  // SSH routes have no credential a learned route could reuse.
  if (active.kind === 'ssh') return null;
  // A route learned over another learned route inherits what that one uses.
  const authorization = active.kind === 'relay' || active.authorization === 't3-connect' ? 't3-connect' as const : undefined;
  const sharedCredential = authorization === undefined ? credentialConnectionId(active.id) : undefined;
  const reported = new Map<string, URL>();
  for (const endpoint of input.reported) {
    let url: URL;
    try { url = new URL(endpoint.httpBaseUrl); } catch { continue; }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') continue;
    if (url.protocol === 'http:' && !input.allowInsecure) continue;
    // A loopback address names whichever device opens it, never the server.
    if (isLocalLoopbackHost(url.hostname)) continue;
    reported.set(url.origin, url);
  }
  const known = new Set(saved.flatMap(route => { const url = routeHttpBaseUrl(route); return url === null || isLearned(route) ? [] : [trimSlash(url)]; }));
  const kept = saved.filter(route => {
    if (!isLearned(route)) return true;
    const url = routeHttpBaseUrl(route);
    if (url === null || !reported.has(trimSlash(url)) || known.has(trimSlash(url))) return false;
    known.add(trimSlash(url));
    return true;
  });
  let next: ConnectionRoute[] = kept;
  for (const url of reported.values()) {
    if (known.has(url.origin)) continue;
    const id = sharedCredential === undefined ? `learned:${entry.environmentId}:${url.origin}` : `learned:${entry.environmentId}:${url.origin}@${sharedCredential}`;
    next = insertRoute(next, { id, origin: url.origin, kind: '', learned: true, ...(authorization === undefined ? {} : { authorization }) });
  }
  // Compare addresses too: a scheme or port change keeps no id stable.
  const signature = (routes: readonly ConnectionRoute[]) => routes.map(route => `${route.id} ${routeHttpBaseUrl(route) ?? ''}`).join('\n');
  return signature(saved) === signature(next) ? null : next;
}

/** The connection id whose stored credential a bearer route uses. */
export function credentialConnectionId(connectionId: string): string {
  // The borrowed id follows the first "@"; neither an environment id nor an origin contains one.
  const at = connectionId.indexOf('@');
  return connectionId.startsWith('learned:') && at !== -1 ? connectionId.slice(at + 1) : connectionId;
}

export function isLearned(route: ConnectionRoute): boolean { return route.learned === true; }

/**
 * Routes left after the user removes one. A learned route borrows the
 * credential of the route it was learned over, so it cannot outlive that
 * route: removing T3 Connect also removes routes learned through it, and
 * removing a paired address removes routes that borrow its token.
 */
export function routesAfterRemoving(routes: readonly ConnectionRoute[], removedId: string): ConnectionRoute[] {
  const removed = routes.find(route => route.id === removedId);
  if (removed === undefined) return [...routes];
  return routes.filter(route => {
    if (route === removed) return false;
    if (!isLearned(route)) return true;
    if (route.authorization === 't3-connect') return removed.kind !== 'relay';
    return credentialConnectionId(route.id) !== removedId;
  });
}

/** Whether removing T3 Connect leaves the environment no route, so signing out removes it entirely. */
export function removedWithRelay(entry: RouteEntry): boolean { return routesAfterRemoving(connectionRoutes(entry), RELAY_ROUTE_ID).length === 0; }

/** One SSH target, as desktop keys its tunnels: alias, host, user, and port. */
export function sshTargetKey(target: RouteSshTarget): string { return JSON.stringify([target.alias, target.hostname, target.username, target.port]); }

// ── GitHub sharing trust (githubRoutingPermissions.ts gitHubRoutingConnectionKey) ──
/** The clone's single-endpoint key (connections.ts routingKey), kept so existing trust survives. */
export const singleRouteKey = (origin: string, environmentId: string) => JSON.stringify([environmentId, `${trimSlash(origin.trim())}/`]);
/**
 * Trust belongs to the saved endpoints, never to an environment id advertised
 * by a server alone. With several routes the key covers all of them, sorted so
 * reordering keeps trust but adding or changing an address revokes it. Learned
 * routes come and go with the server's addresses, so they leave trust alone.
 */
export function gitHubRoutingConnectionKey(entry: RouteEntry): string | null {
  const routes = entry.routes.filter(route => !isLearned(route));
  if (routes.length === 1) {
    const only = routes[0]!;
    return only.kind === 'relay' ? routeConnectionKey(entry.environmentId, only) : only.origin ? singleRouteKey(only.origin, entry.environmentId) : null;
  }
  const keys = routes.map(route => routeConnectionKey(entry.environmentId, route));
  return keys.every(key => key !== null) ? JSON.stringify([...keys].sort()) : null;
}
function routeConnectionKey(environmentId: string, route: ConnectionRoute): string | null {
  if (route.kind === 'relay') return JSON.stringify(['RelayConnectionTarget', environmentId]);
  if (route.kind === 'ssh') {
    if (!route.ssh) return null;
    const { alias, hostname, username, port } = route.ssh;
    return JSON.stringify(['SshConnectionTarget', environmentId, alias, hostname, username, port]);
  }
  try {
    const url = new URL(route.origin);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return null;
    return JSON.stringify(['BearerConnectionTarget', environmentId, trimSlash(url.href)]);
  } catch { return null; }
}

// ── Registry route edits (registry.ts registerRoute, reorderRoutes, removeRoute) ──
/** A paired address joins the routes: the same address replaces its route in place, a new one goes by kind. */
export function registerRoute(routes: readonly ConnectionRoute[], route: ConnectionRoute): ConnectionRoute[] {
  const same = findRouteToSameAddress(routes, route);
  return same ? routes.map(existing => existing === same ? route : existing) : upsertRoute(routes, route);
}
/** A dropped order: it must name every saved route exactly once. */
export function reorderRoutes(routes: readonly ConnectionRoute[], ids: readonly string[]): ConnectionRoute[] {
  const byId = new Map(routes.map(route => [route.id, route]));
  if (ids.length !== routes.length || new Set(ids).size !== ids.length || ids.some(id => !byId.has(id))) {
    throw new Error('The new route order must name every saved route once.');
  }
  return ids.map(id => byId.get(id)!);
}
// ── Decoding the native store (T3SavedEnvironments) ──────────────────────
const asString = (value: unknown) => typeof value === 'string' ? value : '';
function decodeSsh(value: unknown): RouteSshTarget | undefined {
  if (!value || typeof value !== 'object') return undefined;
  const raw = value as Record<string, unknown>;
  const alias = asString(raw.alias);
  if (!alias) return undefined;
  return { alias, hostname: asString(raw.hostname) || alias, username: asString(raw.username) || null, port: typeof raw.port === 'number' ? raw.port : null };
}
/** A saved environment's routes as the native store keeps them; an entry from before routes has its origin as its one route. */
export function savedRoutes(entry: Record<string, unknown>): ConnectionRoute[] {
  const list = Array.isArray(entry.routes) ? entry.routes : [];
  const routes = list.flatMap((value): ConnectionRoute[] => {
    if (!value || typeof value !== 'object') return [];
    const raw = value as Record<string, unknown>, id = asString(raw.id), origin = asString(raw.origin);
    if (!id) return [];
    const ssh = decodeSsh(raw.ssh);
    return [{ id, origin, kind: asString(raw.kind), ...(raw.learned === true ? { learned: true } : {}),
      ...(raw.authorization === 't3-connect' ? { authorization: 't3-connect' as const } : {}), ...(ssh ? { ssh } : {}) }];
  });
  const origin = trimSlash(asString(entry.origin));
  return routes.length || !origin ? routes : [{ id: origin, origin, kind: '' }];
}
/** The record T3SavedEnvironments stores for a route: `credential` names the Keychain account's origin. */
export function storedRoute(route: ConnectionRoute): Record<string, unknown> {
  return { id: route.id, origin: route.origin, kind: route.kind ?? '', learned: route.learned === true, credential: credentialConnectionId(route.id),
    ...(route.authorization ? { authorization: route.authorization } : {}), ...(route.ssh ? { ssh: route.ssh } : {}) };
}
