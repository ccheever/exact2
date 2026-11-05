// Mobile Home search joins the focused connection with the synchronized fleet.
// The copied sidebar search stays pinned to its upstream source; this adapter
// supplies the mobile Home's environment-scoped message results.
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, type Native } from './shared/protocol';
import type { T3Client } from './shared/client';
import { EnvironmentFleet, fleet, type EnvironmentFleet as EnvironmentFleetType } from './shared/settings-b-fleet';

const searches = new WeakMap<T3Client, { query: string; matches: Obj[] }>();
const pendingSearches = new WeakMap<T3Client, string>();

interface SearchTarget { environmentId: string; fleetKey: string; generation: number }

function searchTargets(client: T3Client, source: EnvironmentFleetType): SearchTarget[] {
  const targets: SearchTarget[] = [];
  if (client.environmentId) targets.push({ environmentId: client.environmentId, fleetKey: '', generation: client.generation });
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || entry.environmentId === client.environmentId || entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    targets.push({ environmentId: entry.environmentId, fleetKey: entry.key, generation: entry.generation });
  }
  return targets;
}

/** Search every environment whose shell contributes rows to the mobile Home. */
export async function runMobileHomeSearch(client: T3Client, native: Native, query: string,
  source: EnvironmentFleetType = fleet): Promise<void> {
  const needle = query.trim();
  if (needle.length < 2 || needle.length > 200) { searches.delete(client); pendingSearches.delete(client); return; }
  pendingSearches.set(client, needle);
  const matches: Obj[] = [];
  await Promise.all(searchTargets(client, source).map(async target => {
    try {
      const value = target.fleetKey === ''
        ? await client.restAccess(native).request('orchestration.searchThreads', { query: needle, limit: 50 })
        : await (async () => {
          const reply = await bridgeReply(EnvironmentFleet.native(native, target.fleetKey), { op: 'request', method: 'orchestration.searchThreads',
            payload: { query: needle, limit: 50 }, generation: target.generation });
          if (!reply.ok) throw new Error(reply.error?.message ?? 'The environment search failed.');
          return reply.value;
        })();
      matches.push(...arr(obj(value).matches).map(match => ({ ...match, environmentId: target.environmentId })));
    } catch { /* an unavailable environment contributes no message matches */ }
  }));
  if (client.query.trim() === needle) searches.set(client, { query: needle, matches });
  if (pendingSearches.get(client) === needle) pendingSearches.delete(client);
}

export function startMobileHomeSearch(client: T3Client, native: Native, query: string,
  source: EnvironmentFleetType = fleet): void {
  void runMobileHomeSearch(client, native, query, source).then(async () => {
    if (query.trim().length < 2) return;
    try { await native.later({ op: 'sidebarNotify' }); } catch { /* the next event still repaints older modules */ }
  });
}

export function mobileHomeSearchPending(client: T3Client): boolean {
  const needle = client.query.trim();
  return needle.length >= 2 && pendingSearches.get(client) === needle;
}

/** Return matches by thread, using environment-scoped keys when Home is projected. */
export function mobileHomeSearchMatches(client: T3Client, scoped = false): Map<string, Obj> {
  const search = searches.get(client), result = new Map<string, Obj>();
  if (!search || search.query !== client.query.trim()) return result;
  for (const match of search.matches) {
    const environmentId = str(match.environmentId, client.environmentId), threadId = str(match.threadId);
    const key = scoped ? `${environmentId}:${threadId}` : threadId;
    if (!result.has(key)) result.set(key, match);
  }
  return result;
}
