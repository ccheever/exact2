// browser-surface part 2: where a Browser tab goes and what it can recommend (T3 Code 1e2ecbd975, MIT, see
// LICENSE-T3: apps/web/src/browser/browserTargetResolver.ts, apps/web/src/portDiscoveryState.ts
// `boundConfiguredLocalServerUrls`/`useDiscoveredPortsState`, apps/web/src/components/preview/
// useDiscoveredLocalServers.ts `mergeServers`, previewEmptyStateLogic.ts `getConfiguredPreviewUrls`;
// packages/contracts/src/preview.ts `DiscoveredLocalServer`, rpc.ts `subscribeDiscoveredLocalServers`).
//
// Target resolution: a URL loads as typed; an environment port (an agent's `localhost:5173`, a discovered
// loopback server) loads at the environment's own host when that host is on a private network (`localhost` for
// a loopback environment, so a dev server bound to ::1 or 127.0.0.1 alone is reached either way), and is refused
// for a public relay (the reference's planned authenticated gateway). The environment's address is the client's
// connected origin (the reference's prepared connection `httpBaseUrl`).
//
// Discovery: the server's port scanner (`lsof` on macOS, then an HTML probe) streams the live local servers;
// the empty state lists them, configured project preview URLs first. The stream lasts while a Browser tab of
// the focused thread shows its empty state (the reference's query lives with PreviewEmptyState), carried by the
// Swift transport like the client's other streams (X21, exact2 #126).
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { isLoopbackHost, normalizePreviewUrl } from './browser-url';
import { isLocalLoopbackHost, isPrivateNetworkHost } from './host-classification';
import { subscriptionSerial } from './shell-vcs';
import { letGo } from './let-go';

export { isLocalLoopbackHost, isPrivateNetworkHost, isPublicFaviconHost, normalizeHostname } from './host-classification';

export type BrowserNavigationTarget = { kind: 'url'; url: string } | { kind: 'environment-port'; port: number; protocol?: 'http' | 'https'; path?: string };
export type PreviewUrlResolution = { requestedUrl: string; resolvedUrl: string; resolutionKind: 'direct' | 'direct-private-network'; environmentId: string };
/** The environment's prepared connection (readPreparedConnection); null when it is not connected. */
export type PreparedConnection = { httpBaseUrl: string } | null;

const readEnvironmentUrl = (environmentId: string, connection: PreparedConnection): URL => {
  if (!connection) throw new Error(`Environment ${environmentId} is not connected.`);
  return new URL(connection.httpBaseUrl);
};

function resolveEnvironmentPortTarget(environmentId: string, target: Extract<BrowserNavigationTarget, { kind: 'environment-port' }>, environmentUrl: URL,
  requestedUrl?: string, sourceUrl?: URL): PreviewUrlResolution {
  if (!isPrivateNetworkHost(environmentUrl.hostname)) {
    throw new Error('This environment port needs the planned authenticated preview gateway; its server address is not directly private-network reachable.');
  }
  const protocol = target.protocol ?? 'http';
  const path = target.path?.startsWith('/') ? target.path : `/${target.path ?? ''}`;
  const normalizedEnvironmentHost = environmentUrl.hostname.replace(/^\[|\]$/g, '');
  // Local loopback environments advertise `localhost` so a dual-stack lookup reaches a server bound to ::1 or 127.0.0.1.
  const resolvedHost = isLocalLoopbackHost(normalizedEnvironmentHost) ? 'localhost' : normalizedEnvironmentHost.includes(':') ? `[${normalizedEnvironmentHost}]` : normalizedEnvironmentHost;
  const resolved = sourceUrl ? new URL(sourceUrl) : new URL(path, `${protocol}://${resolvedHost}:${target.port}`);
  if (sourceUrl) { resolved.hostname = resolvedHost; resolved.port = String(target.port); }
  return {
    requestedUrl: requestedUrl ?? `${protocol}://localhost:${target.port}${path}`, resolvedUrl: resolved.toString(),
    resolutionKind: isLocalLoopbackHost(normalizedEnvironmentHost) ? 'direct' : 'direct-private-network', environmentId,
  };
}

/** resolveBrowserNavigationTarget: a URL as given; an environment port at the environment's host. */
export function resolveBrowserNavigationTarget(environmentId: string, target: BrowserNavigationTarget, connection: PreparedConnection): PreviewUrlResolution {
  if (target.kind === 'url') return { requestedUrl: target.url, resolvedUrl: target.url, resolutionKind: 'direct', environmentId };
  return resolveEnvironmentPortTarget(environmentId, target, readEnvironmentUrl(environmentId, connection));
}

/** resolveDiscoveredServerUrl: a server-picker value, normalized; a loopback one at the environment's host. */
export function resolveDiscoveredServerUrl(environmentId: string, rawUrl: string, connection: PreparedConnection): string {
  try {
    const normalizedUrl = normalizePreviewUrl(rawUrl), parsed = new URL(normalizedUrl);
    if (!isLoopbackHost(parsed.hostname)) return normalizedUrl;
    return resolveEnvironmentPortTarget(environmentId, {
      kind: 'environment-port', port: Number(parsed.port || (parsed.protocol === 'https:' ? 443 : 80)), protocol: parsed.protocol === 'https:' ? 'https' : 'http',
      path: `${parsed.pathname}${parsed.search}${parsed.hash}`,
    }, readEnvironmentUrl(environmentId, connection), rawUrl, parsed).resolvedUrl;
  } catch { return rawUrl; }
}

/** The client's connection to its focused environment, as the resolver reads it. */
export const preparedConnection = (client: T3Client): PreparedConnection => client.environmentId && client.connection === 'connected' && client.origin ? { httpBaseUrl: client.origin } : null;
/** The environment host history treats as one site with loopback. */
export function environmentHostname(client: T3Client): string | null {
  try { return client.origin ? new URL(client.origin).hostname : null; } catch { return null; }
}

// ── Discovered local servers ────────────────────────────────────────────────────────────────────
export type DiscoveredLocalServer = { host: string; port: number; url: string; processName: string | null; pid: number | null; terminal: { threadId: string; terminalId: string } | null };
export type PreviewableServer = DiscoveredLocalServer & { source: 'scanner' | 'configured'; requestedUrl: string };

export const PREVIEW_URL_MAX_LENGTH = 2_048;
export const CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS = 32;

/** getConfiguredPreviewUrls: the project scripts' preview URLs. */
export const getConfiguredPreviewUrls = (scripts: ReadonlyArray<{ previewUrl?: unknown }> | undefined): ReadonlyArray<string> =>
  scripts?.flatMap(script => typeof script.previewUrl === 'string' && script.previewUrl ? [script.previewUrl] : []) ?? [];

/** boundConfiguredLocalServerUrls: loopback http(s) URLs only, deduped by resource, at most 32. */
export function boundConfiguredLocalServerUrls(urls: ReadonlyArray<string> | undefined): ReadonlyArray<string> {
  const bounded: string[] = [], seen = new Set<string>();
  for (const raw of urls ?? []) {
    if (raw.length === 0 || raw.length > PREVIEW_URL_MAX_LENGTH || raw.trim().length !== raw.length) continue;
    try {
      const url = new URL(raw);
      if (url.protocol !== 'http:' && url.protocol !== 'https:') continue;
      if (!isLoopbackHost(url.hostname) || url.href.length > PREVIEW_URL_MAX_LENGTH) continue;
      const resourceUrl = new URL(url.href);
      resourceUrl.hash = '';
      if (seen.has(resourceUrl.href)) continue;
      seen.add(resourceUrl.href);
      bounded.push(url.href);
      if (bounded.length >= CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS) break;
    } catch { /* Invalid and non-local project preview URLs are not discovery candidates. */ }
  }
  return bounded;
}

const canonicalKey = (host: string, port: number): string => { const normalizedHost = host.toLowerCase(); return `${isLoopbackHost(normalizedHost) ? 'loopback' : normalizedHost}:${port}`; };
function parseLocalUrl(raw: string): { host: string; port: number; url: string } | null {
  try {
    const parsed = new URL(raw);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null;
    if (!isLoopbackHost(parsed.hostname)) return null;
    const port = parsed.port ? Number.parseInt(parsed.port, 10) : parsed.protocol === 'http:' ? 80 : 443;
    if (!Number.isFinite(port) || port <= 0) return null;
    return { host: parsed.hostname, port, url: parsed.href };
  } catch { return null; }
}

/** mergeServers: live servers only, a configured one first (with its own URL where the server does not probe paths), then by port. */
export function mergeServers(input: { scanner: ReadonlyArray<DiscoveredLocalServer & { requestedUrl: string }>; configuredUrls: ReadonlyArray<string>; configuredUrlProbing?: boolean }): ReadonlyArray<PreviewableServer> {
  const configuredByServer = new Map<string, { host: string; port: number; url: string }>();
  for (const url of input.configuredUrls) {
    const parsed = parseLocalUrl(url);
    if (!parsed) continue;
    const key = canonicalKey(parsed.host, parsed.port);
    if (!configuredByServer.has(key)) configuredByServer.set(key, parsed);
  }
  const live: PreviewableServer[] = input.scanner.map(server => {
    const configured = configuredByServer.get(canonicalKey(server.host, server.port));
    return { ...server, requestedUrl: configured && input.configuredUrlProbing === false ? configured.url : server.requestedUrl, source: configured ? 'configured' : 'scanner' };
  });
  const order = { configured: 0, scanner: 1 } as const;
  return [...live].sort((a, b) => order[a.source] !== order[b.source] ? order[a.source] - order[b.source] : a.port - b.port);
}

/** A stream value read defensively (DiscoveredLocalServerList). */
export function readDiscoveredServers(value: unknown): { servers: DiscoveredLocalServer[]; configuredUrlProbing: boolean } {
  const list = obj(value);
  const servers = arr(list.servers).flatMap((raw): DiscoveredLocalServer[] => {
    const host = str(raw.host).trim(), port = Number(raw.port), url = str(raw.url), terminal = obj(raw.terminal);
    if (!host || !Number.isInteger(port) || port <= 0 || port >= 65536 || !url) return [];
    return [{ host, port, url, processName: str(raw.processName).trim() || null, pid: Number.isInteger(raw.pid) && Number(raw.pid) > 0 ? Number(raw.pid) : null,
      terminal: str(terminal.threadId) && str(terminal.terminalId) ? { threadId: str(terminal.threadId), terminalId: str(terminal.terminalId) } : null }];
  });
  return { servers, configuredUrlProbing: list.configuredUrlProbing === true };
}

/** useDiscoveredLocalServers: the live servers, each at its resolved URL, keeping the loopback URL it was found at. */
export function previewableServers(environmentId: string, servers: ReadonlyArray<DiscoveredLocalServer>, configuredUrls: ReadonlyArray<string>, configuredUrlProbing: boolean, connection: PreparedConnection): ReadonlyArray<PreviewableServer> {
  return mergeServers({ scanner: servers.map(server => ({ ...server, url: resolveDiscoveredServerUrl(environmentId, server.url, connection), requestedUrl: server.url })), configuredUrls, configuredUrlProbing });
}

// ── The stream (subscribeDiscoveredLocalServers on the focused connection) ─────────────────────
export const DISCOVERED_SERVERS_KEY = 'browser-discovered-servers';
type Discovery = { generation: number; id: string; floor: number; maxSeen: number; tried: boolean; signature: string; servers: DiscoveredLocalServer[]; configuredUrlProbing: boolean; subscribes: number };
const discoveries = new WeakMap<T3Client, Discovery>();
function discovery(client: T3Client): Discovery {
  let value = discoveries.get(client);
  if (!value || value.generation !== client.generation) {
    value = { generation: client.generation, id: '', floor: value?.maxSeen ?? 0, maxSeen: value?.maxSeen ?? 0, tried: false, signature: '', servers: [], configuredUrlProbing: false, subscribes: value?.subscribes ?? 0 };
    discoveries.set(client, value);
  }
  return value;
}

/** client.ts drain: the newest subscription's list wins; a failure keeps the last list until the next pass subscribes again. */
export function discoveredServersEvent(client: T3Client, entry: Obj): void {
  const value = discovery(client), id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  value.maxSeen = Math.max(value.maxSeen, serial);
  if (serial <= value.floor || (value.id && serial < subscriptionSerial(value.id))) return;
  value.id = id;
  const item = obj(entry.value);
  // As live-streams.ts: a retry the transport schedules (`_retryDue`) or an ended stream subscribes again on the next
  // pass; a transport error keeps the last list and waits for the next connection, so it never loops past the backoff.
  if (item._retryDue || item._streamEnded) { value.id = ''; value.tried = false; return; }
  if (item._transportError) return;
  const read = readDiscoveredServers(item);
  value.servers = read.servers; value.configuredUrlProbing = read.configuredUrlProbing;
}

/** Follows the empty state: subscribed (with the project's configured URLs) while it shows, released when it does not. */
export async function watchDiscoveredServers(client: T3Client, native: Native, wanted: boolean, configuredUrls: ReadonlyArray<string>): Promise<void> {
  const value = discovery(client), bounded = boundConfiguredLocalServerUrls(configuredUrls), signature = JSON.stringify(bounded);
  if (value.id && (!wanted || signature !== value.signature)) {
    value.id = ''; value.tried = false; value.floor = value.maxSeen;
    try { await client.restAccess(native).call({ op: 'unsubscribe', key: DISCOVERED_SERVERS_KEY }); } catch (error) { if (letGo(error)) throw error; }
    if (!wanted) { value.servers = []; value.configuredUrlProbing = false; }
  }
  if (!wanted || value.id || value.tried || client.connection !== 'connected') return;
  value.tried = true; value.floor = value.maxSeen; value.signature = signature;
  try {
    const reply = await client.restAccess(native).call({ op: 'subscribe', key: DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: bounded.length ? { configuredUrls: [...bounded] } : {} });
    value.subscribes++;
    const serial = subscriptionSerial(str(reply.id));
    value.maxSeen = Math.max(value.maxSeen, serial);
    if (serial > value.floor && (!value.id || serial > subscriptionSerial(value.id))) value.id = str(reply.id);
  } catch (error) { value.tried = false; if (letGo(error)) throw error; }
}

/** The focused connection's latest list (empty until the stream's first value). */
export const discoveredServers = (client: T3Client): { servers: ReadonlyArray<DiscoveredLocalServer>; configuredUrlProbing: boolean; subscribes: number; live: boolean } => {
  const value = discovery(client);
  return { servers: value.servers, configuredUrlProbing: value.configuredUrlProbing, subscribes: value.subscribes, live: !!value.id };
};
