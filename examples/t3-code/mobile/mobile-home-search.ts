// Mobile Home search joins the focused connection with the synchronized fleet.
// The copied sidebar search stays pinned to its upstream source; this adapter
// supplies the mobile Home's environment-scoped message results.
import { arr, obj, str, type Obj } from './shared/domain';
import { bridgeReply, type Native } from './shared/protocol';
import type { T3Client } from './shared/client';
import { EnvironmentFleet, fleet, type EnvironmentFleet as EnvironmentFleetType } from './shared/settings-b-fleet';

interface Search { query: string; answers: (Obj[] | null)[] }
interface SearchState { current: Search | null; previous: { query: string; matches: Obj[] } | null }
const states = new WeakMap<T3Client, SearchState>();

function stateOf(client: T3Client): SearchState {
  const state = states.get(client);
  if (state) return state;
  const next: SearchState = { current: null, previous: null };
  states.set(client, next);
  return next;
}

function searchable(query: string): boolean { return query.length >= 2 && query.length <= 200 }

function relatedQuery(previous: string, current: string): boolean {
  const before = previous.toLocaleLowerCase(), now = current.toLocaleLowerCase();
  return !!before && !!now && (now.startsWith(before) || before.startsWith(now));
}

function visible(client: T3Client): { matches: Obj[]; pending: boolean } {
  const state = stateOf(client), needle = client.query.trim();
  if (!searchable(needle)) return { matches: [], pending: false };
  const current = state.current?.query === needle ? state.current : null;
  const matches = current?.answers.flatMap(answer => answer ?? []) ?? [];
  const answered = !!current && current.answers.every(answer => answer !== null);
  if (current && (answered || matches.length > 0)) {
    state.previous = { query: needle, matches };
    return { matches, pending: !answered };
  }
  const previous = state.previous;
  return { matches: previous && relatedQuery(previous.query, needle) ? previous.matches : [], pending: true };
}

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
  source: EnvironmentFleetType = fleet, landed: () => Promise<void> | void = () => undefined): Promise<void> {
  const needle = query.trim();
  const state = stateOf(client);
  if (!searchable(needle)) { state.current = null; state.previous = null; return; }
  const targets = searchTargets(client, source), search: Search = { query: needle, answers: targets.map(() => null) };
  state.current = search;
  const publish = async () => {
    if (state.current !== search) return;
    try { await landed(); } catch { /* a repaint failure never invalidates a completed search answer */ }
  };
  await Promise.all(targets.map(async (target, index) => {
    let matches: Obj[] = [];
    try {
      const value = target.fleetKey === ''
        ? await client.restAccess(native).request('orchestration.searchThreads', { query: needle, limit: 50 })
        : await (async () => {
          const reply = await bridgeReply(EnvironmentFleet.native(native, target.fleetKey), { op: 'request', method: 'orchestration.searchThreads',
            payload: { query: needle, limit: 50 }, generation: target.generation });
          if (!reply.ok) throw new Error(reply.error?.message ?? 'The environment search failed.');
          return reply.value;
        })();
      matches = arr(obj(value).matches).map(match => ({ ...match, environmentId: target.environmentId }));
    } catch { /* an unavailable environment contributes no message matches */ }
    if (state.current !== search) return;
    search.answers[index] = matches;
    await publish();
  }));
  if (state.current === search) state.previous = { query: needle, matches: search.answers.flatMap(answer => answer ?? []) };
}

export function startMobileHomeSearch(client: T3Client, native: Native, query: string,
  source: EnvironmentFleetType = fleet): void {
  void runMobileHomeSearch(client, native, query, source, async () => {
    if (query.trim().length < 2) return;
    try { await native.later({ op: 'sidebarNotify' }); } catch { /* the next event still repaints older modules */ }
  });
}

export function mobileHomeSearchPending(client: T3Client): boolean {
  return visible(client).pending;
}

/** Return matches by thread, using environment-scoped keys when Home is projected. */
export function mobileHomeSearchMatches(client: T3Client, scoped = false): Map<string, Obj> {
  const result = new Map<string, Obj>();
  for (const match of visible(client).matches) {
    const environmentId = str(match.environmentId, client.environmentId), threadId = str(match.threadId);
    const key = scoped ? `${environmentId}:${threadId}` : threadId;
    if (!result.has(key)) result.set(key, match);
  }
  return result;
}
