// Shared pull request observations (T3 Code 9c161462c9, MIT; see LICENSE-T3):
// apps/web/src/state/pullRequests.ts. Host update time orders saved snapshots;
// receipt order breaks ties between reads, and merged state is final.
import type { T3Client } from '../client/client';
import { num, obj, str, type Obj } from '../client/domain';
import { visiblePullRequests } from '../panel/shell-pr';

const observations = new WeakMap<object, Map<string, Obj>>();
const keyOf = (environmentId: string, ref: Obj) => JSON.stringify([environmentId, str(ref.projectId), str(ref.host).toLowerCase(), str(ref.repository).toLowerCase(), num(ref.number)]);
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
export function observePullRequestSummary(client: object, environmentId: string, reference: Obj, summary: Obj): void {
  if (num(summary.number) <= 0 || !str(summary.updatedAt)) return;
  let map = observations.get(client);
  if (!map) observations.set(client, map = new Map());
  const key = keyOf(environmentId, reference), previous = map.get(key);
  const next = newestPullRequestSummary(previous ?? null, summary);
  if (next !== summary) return;
  const held = previous ?? {};
  const fields = ['provider', 'projectId', 'repository', 'number', 'title', 'url', 'state', 'isDraft', 'headBranch', 'baseBranch', 'updatedAt', 'observedAt', 'author', 'reviewDecision', 'checksState', 'mergeability'];
  const merged = Object.fromEntries(fields.filter(field => summary[field] !== undefined).map(field => [field, summary[field]]));
  for (const field of ['isDraft', 'mergeability', 'reviewDecision', 'checksState']) if (merged[field] === undefined && held[field] !== undefined) merged[field] = held[field];
  map.set(key, merged);
  if (map.size > 256) map.delete(map.keys().next().value!);
}
export function sharedPullRequestSummary(client: T3Client, reference: Obj, current: Obj | null, environmentId = client.environmentId): Obj | null {
  let newest = newestPullRequestSummary(current, observations.get(client)?.get(keyOf(environmentId, reference)) ?? null);
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
