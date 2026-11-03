// The ref lists' freshness: client-runtime state/vcs.ts (makeCachedVcsRefsChanges' serverRefsRevisionAtom),
// vcsRefInvalidation.ts and the commands that invalidate through it (state/vcs.ts, git.ts, sourceControl.ts) at
// T3 Code 5d3d78ab93 (MIT; see LICENSE-T3). A command that can change a checkout's refs invalidates them once it
// settles, failed or not; the next vcs.listRefs for that checkout then asks `refresh: true`, so the server reads the
// refs again rather than answering from its snapshot, which misses branches changed outside T3 Code. Every picker's
// read (the composer's, the workspace card's, Settings' base branch) goes through T3Client.call, which applies this.
import { obj, str, type Obj } from '../client/domain';

const INVALIDATING = new Set(['vcs.pull', 'vcs.refreshStatus', 'vcs.createWorktree', 'vcs.removeWorktree', 'vcs.createRef', 'vcs.switchRef',
  'vcs.init', 'git.preparePullRequestThread', 'sourceControl.publishRepository']);

/** Per client and checkout: how often its refs were invalidated, and the count the last bypassing read answered. */
const revisions = new WeakMap<object, Map<string, { invalidated: number; served: number }>>();
const revision = (client: object, key: string) => {
  let byKey = revisions.get(client);
  if (!byKey) revisions.set(client, byKey = new Map());
  let entry = byKey.get(key);
  if (!entry) byKey.set(key, entry = { invalidated: 0, served: 0 });
  return entry;
};

/**
 * The request as sent, `refresh: true` added to a vcs.listRefs whose checkout changed since the last bypassing read,
 * and what to note once it settles.
 */
export function vcsRefsFreshness(client: { environmentId: string }, request: Obj): { request: Obj; done: (ok: boolean) => void } {
  const method = str(request.method), payload = obj(request.payload), cwd = str(payload.cwd);
  if (request.op !== 'request' || !cwd || !client.environmentId) return { request, done: () => undefined };
  const entry = revision(client, `${client.environmentId}\0${cwd}`);
  if (INVALIDATING.has(method)) return { request, done: () => { entry.invalidated++; } };
  if (method !== 'vcs.listRefs' || entry.invalidated <= entry.served || payload.refresh === true) return { request, done: () => undefined };
  const expected = entry.invalidated;
  return { request: { ...request, payload: { ...payload, refresh: true } }, done: ok => { if (ok) entry.served = Math.max(entry.served, expected); } };
}
