// Filters › Author and Labels search the host (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// apps/web/src/components/pullRequest/PullRequestListFilters.tsx useHostedFilterOptions, PullRequestAuthorChoices and
// PullRequestLabelChoices; pullRequestList.logic.ts pullRequestAuthorChoices, selectedPullRequestAuthor and
// pullRequestLabelChoices; state/pullRequests.ts usePullRequestFilterOptions over client-runtime's `filterOptions` query).
// The open submenu's search, sent 250 ms after the last keystroke (pull-requests-members.contract), asks each server the
// list reads for `pullRequests.filterOptions` in the list's own scope: its projects and host. The first answer to a
// search is shown pending (the loaded matches and a spinner), then read; an answer is kept five minutes (the query's
// staleTimeMs) per search and scope. A server that fails the read leaves the others' options and marks the answer failed.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';

/** A loaded author: the actor its rows carry, how many rows in the state, how many merged. */
export type AuthorFacet = { actor: Obj; count: number; mergedCount: number };
/** A loaded label, its colour as the dot draws it ('' for none). */
export type LabelFacet = { name: string; color: string; count: number };

const normalize = (value: unknown): string | null => { const trimmed = str(value).trim().toLowerCase(); return trimmed.length > 0 ? trimmed : null; };
/** How many loaded matches come before the host's answers. */
const HOSTED_CHOICE_ROOM = 15;

/**
 * What the Author picker offers for `query`: the loaded authors that match keep their counts and lead, then whatever the
 * host found that no loaded row carries, so a name nobody has loaded yet is still one search away.
 */
export function pullRequestAuthorChoices(facets: readonly AuthorFacet[], hosted: readonly Obj[], query: string): AuthorFacet[] {
  const needle = normalize(query);
  const loaded = facets.filter(facet => needle === null || str(facet.actor.login).toLowerCase().includes(needle) || str(facet.actor.name).toLowerCase().includes(needle));
  const known = new Map(facets.map(facet => [normalize(facet.actor.login), facet]));
  const seen = new Set(loaded.map(facet => normalize(facet.actor.login)));
  const added: AuthorFacet[] = [];
  for (const actor of hosted) {
    const key = normalize(actor.login);
    if (seen.has(key)) continue;
    seen.add(key);
    added.push(known.get(key) ?? { actor, count: 0, mergedCount: 0 });
  }
  // The picker shows a short list, and loaded matches alone could fill it.
  return [...loaded.slice(0, HOSTED_CHOICE_ROOM), ...added, ...loaded.slice(HOSTED_CHOICE_ROOM)];
}

/** The author a filter names, as a choice: one picked from the host's answer is on no loaded row, so it stays shown after the search that found it clears. */
export function selectedPullRequestAuthor(facets: readonly AuthorFacet[], hosted: readonly Obj[], value: string | undefined): AuthorFacet | undefined {
  if (value === undefined) return undefined;
  const key = normalize(value);
  return facets.find(facet => normalize(facet.actor.login) === key)
    ?? { actor: hosted.find(actor => normalize(actor.login) === key) ?? { login: value, name: null, avatarUrl: null }, count: 0, mergedCount: 0 };
}

/** What the Labels picker offers for `query`: the loaded labels that match, then the host's that no loaded row carries. */
export function pullRequestLabelChoices(facets: readonly LabelFacet[], hosted: readonly { name: string; color: string }[], query: string): LabelFacet[] {
  const needle = normalize(query);
  const loaded = facets.filter(facet => needle === null || facet.name.toLowerCase().includes(needle));
  const known = new Map(facets.map(facet => [normalize(facet.name), facet]));
  const seen = new Set(loaded.map(facet => normalize(facet.name)));
  const added: LabelFacet[] = [];
  for (const label of hosted) {
    const key = normalize(label.name);
    if (seen.has(key)) continue;
    seen.add(key);
    added.push(known.get(key) ?? { ...label, count: 0 });
  }
  return [...loaded, ...added];
}

export type FilterOptionKind = 'authors' | 'labels';
/** One server's question: the list's scope (its projects and host), the kind and the search, which is sent only when typed. */
export type FilterOptionTarget = { environmentId: string; input: Obj };
export function filterOptionTargets(queries: readonly { environmentId: string; projectIds?: string[] }[], scope: { projectId: string; host: string }, kind: FilterOptionKind, query: string): FilterOptionTarget[] {
  return queries.map(({ environmentId, projectIds }) => ({ environmentId, input: { kind, ...(scope.projectId ? { projectId: scope.projectId } : {}), ...(projectIds ? { projectIds } : {}),
    ...(scope.host ? { host: scope.host } : {}), ...(query ? { query } : {}) } }));
}

/** The host's answers to a search: every server's authors and labels, `busy` while it is first asked, `failed` when a server could not answer. */
export type HostedOptions = { authors: Obj[]; labels: Obj[]; busy: boolean; failed: boolean };
type Answer = { authors: Obj[]; labels: Obj[]; failed: boolean; at: number };
const STALE_MS = 5 * 60_000;
const MAX_ANSWERS = 64;
const answers = new WeakMap<object, Map<string, Answer>>();
const pendingShown = new WeakMap<object, Set<string>>();
const ofClient = <T>(map: WeakMap<object, T>, client: object, make: () => T): T => { let value = map.get(client); if (!value) { value = make(); map.set(client, value); } return value; };

/**
 * usePullRequestFilterOptions: the servers' answers to `pullRequests.filterOptions`, read on the presenting resource's
 * next run after it showed the search pending (it wakes `t3.pr`, the page's topic), then kept five minutes.
 */
export async function hostedFilterOptions(client: T3Client, native: Native, kind: FilterOptionKind, query: string, targets: readonly FilterOptionTarget[], now: number): Promise<HostedOptions> {
  const key = JSON.stringify([kind, query, targets]), held = ofClient(answers, client, () => new Map<string, Answer>()), answer = held.get(key);
  if (answer && (now <= 0 || now - answer.at < STALE_MS)) return { authors: answer.authors, labels: answer.labels, busy: false, failed: answer.failed };
  if (!answer) {
    const shown = ofClient(pendingShown, client, () => new Set<string>());
    if (!shown.has(key)) {
      shown.add(key);
      if (shown.size > MAX_ANSWERS) shown.delete(shown.values().next().value!);
      try { await native.later({ op: 'wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; }
      return { authors: [], labels: [], busy: true, failed: false };
    }
    shown.delete(key);
  }
  const next: Answer = { authors: [], labels: [], failed: false, at: now };
  for (const target of targets) {
    try {
      const options = obj(await client.rpc(native, 'pullRequests.filterOptions', target.environmentId === client.environmentId ? target.input : { ...target.input, environmentId: target.environmentId }));
      next.authors.push(...arr(options.authors));
      next.labels.push(...arr(options.labels));
    } catch (error) { if (letGo(error)) throw error; next.failed = true; }
  }
  held.delete(key); held.set(key, next);
  if (held.size > MAX_ANSWERS) held.delete(held.keys().next().value!);
  return { authors: next.authors, labels: next.labels, busy: false, failed: next.failed };
}
