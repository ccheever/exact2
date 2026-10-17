// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
// App-only bridge over adopted shared connection/clone facts. No second refresh.
import { composerNow } from './shared/composer-controls';
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';
import { liveEnvironments } from './shared/live-streams';
import { mobileCacheCatalogCurrent, mobileCacheCatalogIdentity, mobileCacheCatalogPrepare } from './mobile-client-cache-catalog';
import { mobileFaviconCache } from './mobile-cache-controls';
import { mobileFaviconCatalogRevision, withMobileFaviconIO } from './mobile-favicon-io';
import { mobileFaviconResourceKey, type MobileFaviconTarget } from './mobile-favicon-cache';
import { mobileFaviconQueries, type MobileFaviconDemand, type MobileFaviconEndpoint, type MobileFaviconView } from './mobile-favicon-query';

const superseded = () => new ClientError('The project icon owner changed.', 'superseded');
const identity = (client: T3Client, background: EnvironmentFleet, environmentId: string) =>
  mobileCacheCatalogCurrent(client, background.saved, environmentId)
    ? mobileCacheCatalogIdentity(background.saved, environmentId) : '';
const enabled = (background: EnvironmentFleet, environmentId: string) => {
  const rows = background.saved.filter(row => row.environmentId === environmentId);
  return rows.length === 1 && rows[0]!.enabled !== false;
};

/** Synchronous projection uses today's saved identity/clear revision, even if a
 * root resource still holds the result of a previous native read. */
export function mobileFaviconDisplay(client: T3Client, target: MobileFaviconTarget, background: EnvironmentFleet = fleet): string | null {
  const catalogIdentity = identity(client, background, target.environmentId);
  return mobileFaviconQueries.display(target, { catalogIdentity,
    scopeRevision: mobileFaviconCatalogRevision(target.environmentId, catalogIdentity) }, mobileFaviconCache);
}

/** Actual focused rpc observation. This adapter neither sends another request
 * nor changes its result/error. Undemanded inherited scans do not load images. */
export async function mobileObserveFaviconRpc(client: T3Client, method: string, payload: Obj,
  send: () => Promise<Obj>, now: () => number = () => composerNow(client), background: EnvironmentFleet = fleet): Promise<Obj> {
  const resource = obj(payload.resource), environmentId = client.environmentId;
  if (method !== 'assets.createUrl' || resource._tag !== 'project-favicon' || typeof resource.cwd !== 'string'
    || client.connection !== 'connected' || !enabled(background, environmentId)) return send();
  const catalogIdentity = identity(client, background, environmentId);
  if (!catalogIdentity) return send();
  const origin = client.origin, generation = client.generation;
  const scopeRevision = mobileFaviconCatalogRevision(environmentId, catalogIdentity);
  const current = () => client.environmentId === environmentId && client.origin === origin && client.generation === generation
    && client.connection === 'connected' && enabled(background, environmentId)
    && identity(client, background, environmentId) === catalogIdentity
    && mobileFaviconCatalogRevision(environmentId, catalogIdentity) === scopeRevision;
  const token = mobileFaviconQueries.beginObservation({ environmentId, cwd: resource.cwd, faviconPath: str(resource.path) || null },
    { catalogIdentity, scopeRevision, origin, generation }, now());
  try {
    const result = await send();
    mobileFaviconQueries.observeReply(token, { ok: true, value: result }, now(), current);
    return result;
  } catch (error) {
    mobileFaviconQueries.observeReply(token, { ok: false, error }, now(), current);
    throw error;
  }
}

function endpoints(client: T3Client, background: EnvironmentFleet, native: Native,
  rootCurrent: () => boolean): MobileFaviconEndpoint[] {
  const adopted = liveEnvironments(client, null, background);
  return [...new Set(background.saved.map(row => str(row.environmentId)).filter(Boolean))].map(environmentId => {
    const focused = client.environmentId === environmentId;
    const entry = focused ? undefined : [...background.entries.values()].find(row => row.environmentId === environmentId);
    const saved = background.saved.find(row => row.environmentId === environmentId)!;
    const origin = focused ? client.origin : entry?.origin ?? str(saved.origin), generation = focused ? client.generation : entry?.generation ?? -1;
    const catalogIdentity = identity(client, background, environmentId), scopeRevision = mobileFaviconCatalogRevision(environmentId, catalogIdentity);
    const wasEnabled = enabled(background, environmentId);
    const phase = (): MobileFaviconEndpoint['phase'] => {
      if (!enabled(background, environmentId)) return 'unavailable';
      const value = focused ? client.connection : entry?.phase;
      if (value === 'connected') return (focused ? client.ready : entry?.synchronized === entry?.generation) ? 'connected' : 'waiting';
      return value === 'connecting' || value === 'reconnecting' ? 'waiting' : 'unavailable';
    };
    const capturedPhase = phase();
    const current = () => rootCurrent() && !!catalogIdentity && identity(client, background, environmentId) === catalogIdentity
      && mobileFaviconCatalogRevision(environmentId, catalogIdentity) === scopeRevision
      && enabled(background, environmentId) === wasEnabled && phase() === capturedPhase
      && (focused ? client.environmentId === environmentId && client.origin === origin && client.generation === generation
        : client.environmentId !== environmentId && [...background.entries.values()].find(row => row.environmentId === environmentId) === entry
          && (!entry || background.entries.get(entry.key) === entry && entry.origin === origin && entry.generation === generation));
    const guarded: Native = { available: native.available, watch(topic) { if (!current()) throw superseded(); native.watch(topic); }, async later(request) {
      if (!current()) throw superseded();
      const result = await bridgeReply(native, request);
      if (!current() || result.generation !== generation) throw superseded();
      return result;
    } };
    return { environmentId, catalogIdentity, scopeRevision, origin, generation, phase: capturedPhase,
      clones: adopted.find(row => row.environmentId === environmentId)?.clones.value ?? [], current,
      async request(resource) {
        if (!current() || capturedPhase !== 'connected') throw superseded();
        const environment = liveEnvironments(client, guarded, background).find(row => row.environmentId === environmentId);
        if (!environment?.connected) throw superseded();
        return environment.request('assets.createUrl', { resource });
      } };
  });
}

/** Root supplies raw projected demand, real clock and route/answer ownership.
 * Reconciliation precedes the first await. No native/Promise/timer is retained. */
export async function mobilePrepareFavicons(input: {
  client: T3Client; native: Native | null | undefined; demands: readonly MobileFaviconDemand[];
  now(): number; current(): boolean; background?: EnvironmentFleet; refresh?: readonly string[];
}): Promise<MobileFaviconView> {
  const { client, demands, now, current } = input, background = input.background ?? fleet;
  const empty = () => ({ revision: mobileFaviconQueries.version, nextDeadline: 0, items: [] });
  if (!current()) return empty();
  mobileFaviconQueries.reconcile(demands, now());
  if (!input.native?.available) return { ...empty(), items: [...new Map(demands.map(target => [mobileFaviconResourceKey(target), target])).entries()]
    .map(([key, target]) => ({ key, url: mobileFaviconDisplay(client, target, background) ?? '', state: 'failure', error: 'Native image storage is unavailable.' })) };
  const native = letGoAware(input.native);
  native.watch('t3.status'); native.watch('t3.fleet');
  await mobileCacheCatalogPrepare(client, native, () => background.saved);
  if (!current()) return empty();
  return withMobileFaviconIO(native, io => mobileFaviconQueries.prepare({ demands, endpoints: endpoints(client, background, native, current),
    now, current, cache: mobileFaviconCache, io, refresh: input.refresh }), {
    identity: environmentId => identity(client, background, environmentId), current,
  });
}
