// Thread search by pull request (T3 Code 74541bab17, MIT; see LICENSE-T3): packages/shared/src/threadPullRequests.ts
// threadPullRequestSearchTerms, apps/web/src/components/Sidebar.logic.ts pullRequestMatchesSearchQuery and
// ThreadStatusIndicators.tsx ThreadSearchPullRequestNumber. A thread is found by its links, its legacy link and the
// pull request the server found for its branch; a search result shows the badge its sidebar row shows.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { visiblePullRequests } from '../panel/shell-pr';
import { lifecycle, lifecycleIcon, sidebarPrBadge, snapshotPr, statusTooltip } from '../panel/pr-surface';
import { sharedPullRequestLinks } from '../pull-requests/pages-pr-shared';
import { parseFleetThreadId } from '../connections/settings-b-fleet';
import { branchPullRequest } from './sidebar-branch-pr';
import { resolveThreadCurrentPullRequestLink } from './thread-reference';

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

/** The row's links with the shared summaries' newer snapshots (Sidebar.tsx reads them through the same store). */
export function rowPullRequestLinks(client: T3Client, thread: Obj): Obj[] {
  return sharedPullRequestLinks(client, arr(thread.pullRequests), str(thread.projectId), parseFleetThreadId(str(thread.id))?.environmentId ?? client.environmentId);
}

const STATE_LABEL: Record<string, string> = { open: 'open', draft: 'draft', closed: 'closed', merged: 'merged' };
/** The badge a search result draws: its text, its glyph, its lifecycle state ('' until known) and its label. */
export interface SearchPullRequest { text: string; icon: string; state: string; label: string }
/**
 * ThreadSearchPullRequestNumber: once the query matches the thread's pull request terms, the sidebar row's badge
 * (resolveThreadPullRequestBadgePresentation): a stack of N, "+N" for several unrelated links, else the current link's
 * number in its state, or the branch's pull request. A stored reference's number shows before its details load.
 */
export function searchPullRequest(client: T3Client, thread: Obj, query: string): SearchPullRequest | null {
  if (!pullRequestMatchesSearchQuery(pullRequestSearchTerms(thread), query)) return null;
  const links = rowPullRequestLinks(client, thread), visible = visiblePullRequests(links);
  const badge = sidebarPrBadge({ ...thread, pullRequests: links });
  if (badge.stacked) return { text: badge.badge, icon: badge.badgeIcon, state: badge.badgeState, label: `Stack of ${badge.badge} pull requests, ${STATE_LABEL[badge.badgeState]}` };
  const current = resolveThreadCurrentPullRequestLink(links);
  const pr = current ? snapshotPr(current) : branchPullRequest(client, thread);
  const stored = current ?? [thread.linkedPullRequest, thread.branchPullRequest].map(reference)
    .find((candidate): candidate is Obj => candidate !== null && pullRequestMatchesSearchQuery(pullRequestSearchTerms({ linkedPullRequest: candidate }), query));
  const number = pr?.number ?? (stored ? num(stored.number) : 0), url = pr?.url || str(stored?.url);
  if (!number || !url) return null;
  const tooltip = pr ? statusTooltip(pr) : `PR #${number}, status pending`;
  if (visible.length > 1) {
    return { text: badge.badge, icon: badge.badgeIcon, state: badge.badgeState, label: `${tooltip}, and ${visible.length - 1} more linked; overall ${STATE_LABEL[badge.badgeState]}` };
  }
  const state = pr ? lifecycle(pr.state, pr.isDraft) : '';
  return { text: String(number), icon: pr ? lifecycleIcon(state) : 'git-pull-request-arrow', state, label: tooltip };
}
