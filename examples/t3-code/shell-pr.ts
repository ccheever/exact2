// The right panel's "Pull request" entry target (MIT reference, see LICENSE-T3:
// apps/web/src/components/pullRequest/pullRequestDetail.logic.ts
// threadPullRequestPanelTarget, packages/shared/src/threadPullRequests.ts
// resolveThreadCurrentPullRequest; upstream f90b77d809): the thread's current
// link from `pullRequests` (open work first, then the newest), reusing the
// legacy `linkedPullRequest` only when it names the same PR, else the legacy
// link or the branch's PR.
import { arr, num, obj, str, type Obj } from './domain';

export type PullRequestTarget = { projectId: string; host: string; repository: string; number: number; url: string };

const hostOf = (url: string) => { try { return new URL(url).hostname.toLowerCase(); } catch { return 'unknown'; } };
const keyOf = (host: string, repository: string, number: number) => `${host.trim().toLowerCase()}/${repository.trim().toLowerCase()}#${number}`;
const isOpen = (link: Obj) => link.snapshot == null || obj(link.snapshot).state === 'open';
const updatedAt = (link: Obj) => { const at = Date.parse(str(obj(link.snapshot).updatedAt) || str(link.linkedAt)); return Number.isNaN(at) ? 0 : at; };

/** visibleThreadPullRequests: tombstoned stack members stay stored but are never shown. */
export const visiblePullRequests = (links: unknown): Obj[] => arr(links).filter(link => link.source !== 'stack-dismissed');

/** resolveThreadCurrentPullRequestLink, without stack layering: one open link, else the newest open, else the newest updated. */
export function currentPullRequestLink(links: unknown): Obj | null {
  const visible = visiblePullRequests(links);
  if (!visible.length) return null;
  const open = visible.filter(isOpen);
  if (open.length === 1) return open[0]!;
  const pool = open.length > 1 ? open : visible;
  return [...pool].sort((left, right) => (open.length > 1 ? Date.parse(str(right.linkedAt)) - Date.parse(str(left.linkedAt)) : updatedAt(right) - updatedAt(left)))[0]!;
}

/** threadPullRequestPanelTarget. */
export function pullRequestPanelTarget(thread: Obj): PullRequestTarget | Obj | null {
  const current = currentPullRequestLink(thread.pullRequests);
  const legacy = thread.linkedPullRequest && typeof thread.linkedPullRequest === 'object' ? obj(thread.linkedPullRequest) : null;
  if (current && legacy && keyOf(str(current.host), str(current.repository), num(current.number))
    === keyOf(hostOf(str(legacy.url)), str(legacy.repository), num(legacy.number))) return legacy;
  if (current) return { projectId: str(thread.projectId), host: str(current.host), repository: str(current.repository), number: num(current.number), url: str(current.url) };
  const branch = thread.branchPullRequest && typeof thread.branchPullRequest === 'object' ? obj(thread.branchPullRequest) : null;
  return legacy ?? branch ?? null;
}
