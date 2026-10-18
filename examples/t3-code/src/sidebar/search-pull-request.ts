// Thread search by pull request (T3 Code 9c161462c9, MIT; see LICENSE-T3): packages/shared/src/threadPullRequests.ts
// threadPullRequestSearchTerms, apps/web/src/components/Sidebar.logic.ts pullRequestMatchesSearchQuery and
// ThreadStatusIndicators.tsx ThreadSearchPullRequestNumber. A thread is found by its links, its legacy link and the
// pull request the server found for its branch; a search result names the number that matched.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { visiblePullRequests } from '../panel/shell-pr';
import { lifecycle, snapshotPr, statusTooltip } from '../panel/pr-surface';
import { branchPullRequest } from './sidebar-branch-pr';

const reference = (value: unknown): Obj | null => (value && typeof value === 'object' ? obj(value) : null);
const referenceTerms = (pr: Obj) => [`#${num(pr.number)}`, `${str(pr.repository)}#${num(pr.number)}`, str(pr.url)];

/** threadPullRequestSearchTerms: the branch's pull request, then the visible links (their titles too), else the legacy link. */
export function pullRequestSearchTerms(thread: Obj): string[] {
  const branch = reference(thread.branchPullRequest);
  const terms = branch ? referenceTerms(branch) : [];
  if (arr(thread.pullRequests).length > 0) {
    return terms.concat(visiblePullRequests(thread.pullRequests).flatMap(link => [...referenceTerms(link), str(reference(link.snapshot)?.title)]));
  }
  const legacy = reference(thread.linkedPullRequest);
  return legacy ? terms.concat(referenceTerms(legacy)) : terms;
}

/** lib/utils.ts normalizeSearchText. */
export const normalizeSearchText = (value: string): string => value.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/\s+/g, ' ').trim();

/** pullRequestMatchesSearchQuery: the terms contain every whitespace-separated token of the query. */
export function pullRequestMatchesSearchQuery(terms: string[], query: string): boolean {
  const normalized = normalizeSearchText(query);
  if (!normalized) return false;
  const haystack = normalizeSearchText(terms.join(' '));
  return normalized.split(' ').every(token => haystack.includes(token));
}

export interface SearchPullRequest { number: number; state: string; label: string }
/**
 * ThreadSearchPullRequestNumber: the first visible link the query matches, else the legacy link (with no links) or the
 * branch's pull request. Its state is the link's snapshot, or the summary the row's badge reads (sidebar-branch-pr.ts),
 * when that is the same pull request; with neither, the number stays muted.
 */
export function searchPullRequest(client: T3Client, thread: Obj, query: string): SearchPullRequest | null {
  const matched = visiblePullRequests(thread.pullRequests).find(link => pullRequestMatchesSearchQuery(pullRequestSearchTerms({ pullRequests: [link] }), query));
  const candidates = arr(thread.pullRequests).length === 0 ? [thread.linkedPullRequest, thread.branchPullRequest] : [thread.branchPullRequest];
  const legacy = matched ? undefined : candidates.map(reference).find((pr): pr is Obj => pr !== null && pullRequestMatchesSearchQuery(pullRequestSearchTerms({ linkedPullRequest: pr }), query));
  const found = matched ?? legacy;
  if (!found) return null;
  const summary = matched ? snapshotPr(matched) : branchPullRequest(client, thread);
  const pr = summary && summary.number === num(found.number) ? summary : null;
  return { number: num(found.number), state: pr ? lifecycle(pr.state, pr.isDraft) : '', label: pr ? statusTooltip(pr) : `Pull request #${num(found.number)}` };
}
