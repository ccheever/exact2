// Shared pull request observations (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3):
// apps/web/src/state/pullRequests.ts. Host update time orders saved snapshots;
// receipt order breaks ties between reads, and merged state is final. Checks are read far more often than the
// rest of a pull request, so they keep their own order: a checks read does not carry old metadata forward, and a
// list read does not take back newer checks.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { checksRollup } from '../details/pr-row-logic';
import { visiblePullRequests } from '../panel/shell-pr';

/** ObservedPullRequestChecks: the rollup, the server's read time where it reports one, and the order it arrived in. */
type ObservedChecks = { checksState: string | null; readAt: number | undefined; arrival: number };
/** PullRequestObservations: the newest metadata and, apart from it, the newest checks. */
type Observation = { metadata: Obj | null; checks: ObservedChecks | null };
const observations = new WeakMap<object, Map<string, Observation>>();
/** Client arrival order, the only ordering older servers leave for reads of the same moment. */
let arrivals = 0;
const keyOf = (environmentId: string, ref: Obj) => JSON.stringify([environmentId, str(ref.projectId), str(ref.host).toLowerCase(), str(ref.repository).toLowerCase(), num(ref.number)]);
function observationOf(client: object, environmentId: string, reference: Obj): Observation {
  let map = observations.get(client);
  if (!map) observations.set(client, map = new Map());
  const key = keyOf(environmentId, reference);
  let entry = map.get(key);
  if (!entry) {
    map.set(key, entry = { metadata: null, checks: null });
    if (map.size > 256) map.delete(map.keys().next().value!);
  }
  return entry;
}
const readTime = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) ? value : undefined);
export function newestPullRequestSummary(current: Obj | null, incoming: Obj | null): Obj | null {
  if (!current) return incoming;
  if (!incoming) return current;
  const merged = Number(incoming.state === 'merged') - Number(current.state === 'merged');
  if (merged) return merged > 0 ? incoming : current;
  const updated = Date.parse(str(incoming.updatedAt)) - Date.parse(str(current.updatedAt));
  if (Number.isFinite(updated) && updated) return updated > 0 ? incoming : current;
  const observed = num(incoming.observedAt, -Infinity) - num(current.observedAt, -Infinity);
  return observed < 0 ? current : incoming;
}
/** observePullRequestChecks: newer by the server's read time where both report one, else by arrival. */
function observeChecks(entry: Observation, incoming: ObservedChecks): void {
  const previous = entry.checks;
  const newer = previous === null || (previous.readAt !== undefined && incoming.readAt !== undefined ? incoming.readAt >= previous.readAt : incoming.arrival >= previous.arrival);
  if (newer) entry.checks = incoming;
}
/**
 * observePullRequestMetadata: a detail, summary or list row as it lands. Its checks, where it carries them (a row's
 * `checksState`, a detail's `checks`), are observed in their own right.
 */
export function observePullRequestSummary(client: object, environmentId: string, reference: Obj, summary: Obj): void {
  if (num(summary.number) <= 0 || !str(summary.updatedAt)) return;
  const entry = observationOf(client, environmentId, reference), arrival = ++arrivals;
  const previous = entry.metadata;
  if (newestPullRequestSummary(previous, summary) === summary) {
    const held = previous ?? {};
    const fields = ['provider', 'projectId', 'repository', 'number', 'title', 'url', 'state', 'isDraft', 'headBranch', 'baseBranch', 'updatedAt', 'observedAt', 'author', 'reviewDecision', 'checksState', 'mergeability'];
    const merged = Object.fromEntries(fields.filter(field => summary[field] !== undefined).map(field => [field, summary[field]]));
    for (const field of ['isDraft', 'mergeability', 'reviewDecision', 'checksState']) if (merged[field] === undefined && held[field] !== undefined) merged[field] = held[field];
    entry.metadata = merged;
  }
  const checksState = summary.checksState !== undefined ? summary.checksState : Array.isArray(summary.checks) ? checksRollup(arr(summary.checks)) || null : undefined;
  if (checksState !== undefined) observeChecks(entry, { checksState: checksState === null ? null : str(checksState) || null, readAt: readTime(summary.observedAt), arrival });
}
/** A `pullRequests.checks` answer: the pull request's checks alone, ordered by their own read. */
export function observePullRequestChecks(client: object, environmentId: string, reference: Obj, answer: Obj | null): void {
  // A host that reports no checks answers null.
  if (!answer || !Array.isArray(answer.checks)) return;
  observeChecks(observationOf(client, environmentId, reference), { checksState: checksRollup(arr(answer.checks)) || null, readAt: readTime(answer.observedAt), arrival: ++arrivals });
}
/** observedPullRequestSummary: the metadata, wearing the newest checks. */
function observedSummary(client: object, environmentId: string, reference: Obj): Obj | null {
  const entry = observations.get(client)?.get(keyOf(environmentId, reference));
  if (!entry?.metadata) return null;
  return entry.checks ? { ...entry.metadata, checksState: entry.checks.checksState } : entry.metadata;
}
export function sharedPullRequestSummary(client: T3Client, reference: Obj, current: Obj | null, environmentId = client.environmentId): Obj | null {
  let newest = newestPullRequestSummary(current, observedSummary(client, environmentId, reference));
  // Saved snapshots require host update time and a qualified host to join a live read.
  if (!str(reference.host) || environmentId !== client.environmentId) return newest;
  for (const thread of client.shell.threads ?? []) for (const link of visiblePullRequests(thread.pullRequests)) {
    if (str(link.host).toLowerCase() !== str(reference.host).toLowerCase() || str(link.repository).toLowerCase() !== str(reference.repository).toLowerCase() || num(link.number) !== num(reference.number)) continue;
    const snapshot = obj(link.snapshot);
    if (!str(snapshot.updatedAt)) continue;
    newest = newestPullRequestSummary(newest, { ...snapshot, number: num(link.number), url: str(link.url), repository: str(link.repository), projectId: str(thread.projectId) });
  }
  return newest;
}
export function sharedPullRequestLinks(client: T3Client, links: Obj[], projectId: string, environmentId = client.environmentId): Obj[] {
  return links.map(link => {
    const reference = { projectId, host: str(link.host), repository: str(link.repository), number: num(link.number) };
    const snapshot = sharedPullRequestSummary(client, reference, link.snapshot ? obj(link.snapshot) : null, environmentId);
    // Sync time belongs to this thread's saved link, not a newer host summary; so do the line counts, which a linked row
    // reads from the link's own snapshot (ThreadPullRequestsPanel LinkRow) and a summary may leave out.
    const saved = obj(link.snapshot), counts = Object.fromEntries(['additions', 'deletions', 'changedFiles'].filter(field => saved[field] !== undefined).map(field => [field, saved[field]]));
    return snapshot ? { ...link, snapshot: { ...snapshot, ...counts, syncedAt: saved.syncedAt } } : link;
  });
}
