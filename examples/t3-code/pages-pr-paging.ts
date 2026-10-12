// The Pull Requests page across servers and pages (pr-links-previews-and-routing, C12 and paging): every connected
// server that reads pull requests is asked, each repository by one of them (assignProjectsToEnvironments), and the
// answers fold into one list (mergePullRequestLists). A page is 99 rows; "Load more pull requests" carries on from
// each server's `nextCursors`, appending the slice under the rows already shown, and reads a server with more rows
// but no cursor again at a page one step larger (up to 500). Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/routes/_chat.pull-requests.tsx (environmentQueries, listTargets, page, loadMore, refreshList, ordered),
// components/pullRequest/pullRequestList.logic.ts.
import type { T3Client } from './client';
import { arr, str, type Obj } from './domain';
import { prEnvironments, readsPullRequests } from './pages-pr-environments';
import { assignProjectsToEnvironments, findScopedProject, mergePullRequestLists, PR_MAX_PAGE_SIZE, PR_PAGE_SIZE, pullRequestEntryKey, resolveQueryEnvironmentIds,
  type MergedPullRequestList } from './pages-pr-routing';

export type ListTarget = { environmentId: string; input: Obj };
/** The page the reader has grown to, for one question (`key`): its size, where it carries on from, the servers read again larger. */
type Page = { key: string; size: number; cursors: Record<string, Record<string, string>> | null; regrown: string[] };
type Paging = { page: Page; answered: { key: string; nextCursors: Record<string, Record<string, string>>; truncatedEnvironments: string[]; loaded: number } | null; showing: boolean };
const pagings = new WeakMap<object, Paging>();
function pagingOf(client: object): Paging {
  let paging = pagings.get(client);
  if (!paging) { paging = { page: { key: '', size: PR_PAGE_SIZE, cursors: null, regrown: [] }, answered: null, showing: false }; pagings.set(client, paging); }
  return paging;
}

/**
 * Which servers are asked and for which projects (environmentQueries): the connected servers that read pull requests,
 * the focused one first (where the page's actions land); scoping to a project asks only the servers holding that id;
 * a repository two servers hold is listed by one, and a server left with nothing of its own is not read.
 */
export function environmentQueries(client: T3Client, projectId: string): { environmentId: string; projectIds?: string[] }[] {
  const environments = prEnvironments(client).filter(environment => environment.connected && readsPullRequests(environment));
  const ids = environments.map(environment => environment.id);
  const projects = environments.flatMap(environment => environment.projects.map(project => ({ ...project, id: str(project.id), environmentId: environment.id })));
  const scoped = projectId ? findScopedProject(projects, client.environmentId, projectId) : undefined;
  const asked = resolveQueryEnvironmentIds(ids, projects, scoped, projectId || undefined, true);
  if (projectId) return asked.map(environmentId => ({ environmentId }));
  const assignment = assignProjectsToEnvironments(projects, asked, asked[0] ?? null);
  return asked.flatMap(environmentId => {
    const projectIds = assignment.get(environmentId);
    if (projectIds === undefined) return [];
    const total = projects.filter(project => project.environmentId === environmentId).length;
    return projectIds.length === total ? [{ environmentId }] : [{ environmentId, projectIds }];
  });
}

/** The question without its page (filterKey): a different one starts again at the first page. */
export const filterKeyOf = (queries: { environmentId: string; projectIds?: string[] }[], payload: Obj) => JSON.stringify([queries, payload]);

/** The listing each server is asked for this round (listTargets): a continuation asks only the servers that carry on, or grow. */
export function listTargets(client: T3Client, payload: Obj, queries: { environmentId: string; projectIds?: string[] }[]): { key: string; targets: ListTarget[]; cursors: boolean; size: number } {
  const key = filterKeyOf(queries, payload), paging = pagingOf(client);
  if (paging.page.key !== key) paging.page = { key, size: PR_PAGE_SIZE, cursors: null, regrown: [] };
  const { size, cursors, regrown } = paging.page;
  const targets = queries.flatMap(({ environmentId, projectIds }) => {
    const own = cursors?.[environmentId];
    if (cursors !== null && own === undefined && !regrown.includes(environmentId)) return [];
    return [{ environmentId, input: { ...payload, limit: size, ...(projectIds ? { projectIds } : {}), ...(own === undefined ? {} : { cursors: own }) } }];
  });
  return { key, targets, cursors: cursors !== null, size };
}

/**
 * The servers' answers folded into one, the focused server's rows keyed as a single-server listing keys them (its
 * viewers also by plain host), a background server's rows carrying the server they were listed on.
 */
export function foldAnswers(client: T3Client, answers: [string, Obj][]): MergedPullRequestList | null {
  const merged = mergePullRequestLists(answers);
  if (!merged) return null;
  for (const [key, login] of Object.entries(merged.viewers)) if (key.startsWith(`${client.environmentId} `)) merged.viewers[key.slice(client.environmentId.length + 1)] ??= login;
  merged.entries = merged.entries.map(entry => (entry.environmentId === client.environmentId ? (({ environmentId: _own, ...rest }) => rest)(entry) : entry));
  merged.errors = merged.errors.map(error => (error.environmentId === client.environmentId ? (({ environmentId: _own, ...rest }) => rest)(error) : error));
  return merged;
}

/**
 * The rows shown after an answer (`ordered`): a continuation's slice lands under the rows already held (ordered among
 * itself, newest first); a whole answer replaces them.
 */
export function orderAnswer(previous: Obj[] | null, answer: Obj[], continued: boolean): Obj[] {
  if (!continued || previous === null) return answer;
  const held = new Set(previous.map(pullRequestEntryKey));
  const arrived = answer.filter(entry => !held.has(pullRequestEntryKey(entry))).sort((left, right) => str(right.updatedAt).localeCompare(str(left.updatedAt)));
  return [...previous, ...arrived];
}

/** What an answer to this question said about carrying on, kept for "Load more pull requests". */
export function noteAnswer(client: object, key: string, merged: MergedPullRequestList | null, loaded: number): void {
  pagingOf(client).answered = merged ? { key, nextCursors: merged.nextCursors, truncatedEnvironments: merged.truncatedEnvironments, loaded } : null;
}
/** Whether the rows on screen answer an earlier question while this one travels (showingCarried). */
export function noteShowing(client: object, carried: boolean): void { pagingOf(client).showing = carried; }

/** The list footer's state (the page's `listData.truncated` block). */
export function pagingFooter(client: object, key: string, truncated: boolean, entries: number, loadingMore: boolean, carried: boolean) {
  const paging = pagingOf(client), answered = paging.answered?.key === key ? paging.answered : null;
  const canContinue = !carried && !!answered && Object.keys(answered.nextCursors).length > 0;
  const size = paging.page.key === key ? paging.page.size : PR_PAGE_SIZE;
  const sentCursors = paging.page.key === key && paging.page.cursors !== null;
  if (!truncated || entries === 0) return { footer: '', footerText: '', loadDisabled: false };
  if (loadingMore) return { footer: 'loading', footerText: sentCursors ? 'Loading more' : 'Updating pull requests', loadDisabled: true };
  if (canContinue || size < PR_MAX_PAGE_SIZE) return { footer: 'more', footerText: 'Load more pull requests', loadDisabled: carried };
  return { footer: 'narrow', footerText: 'Narrow your search to find more pull requests.', loadDisabled: false };
}

/** "Load more pull requests" (loadMore): carry on from the cursors, raising the page only for the servers that have none. */
export function loadMore(client: object): boolean {
  const paging = pagingOf(client), answered = paging.answered;
  if (!answered || answered.key !== paging.page.key) return false;
  const { size } = paging.page, nextCursors = answered.nextCursors;
  const regrown = answered.truncatedEnvironments.filter(environmentId => nextCursors[environmentId] === undefined);
  if (!paging.showing && Object.keys(nextCursors).length > 0) {
    paging.page = { key: paging.page.key, size: regrown.length === 0 ? size : Math.min(size + PR_PAGE_SIZE, PR_MAX_PAGE_SIZE), cursors: nextCursors, regrown };
    return true;
  }
  if (size >= PR_MAX_PAGE_SIZE) return false;
  paging.page = { key: paging.page.key, size: Math.min(size + PR_PAGE_SIZE, PR_MAX_PAGE_SIZE), cursors: null, regrown: [] };
  return true;
}

/**
 * A refresh means the whole visible list again (refreshList): a cursored page cannot answer that, so it goes back to
 * one page long enough to cover every row on screen.
 */
export function refreshPaging(client: object): void {
  const paging = pagingOf(client);
  if (paging.page.cursors === null) return;
  const loaded = paging.answered?.key === paging.page.key ? paging.answered.loaded : paging.page.size;
  paging.page = { key: paging.page.key, size: Math.min(Math.max(paging.page.size, Math.ceil(loaded / PR_PAGE_SIZE) * PR_PAGE_SIZE), PR_MAX_PAGE_SIZE), cursors: null, regrown: [] };
}

/** The servers' unmeasured rows, grouped by the server each was listed on, for `pullRequests.listStats`. */
export function statsBatches(entries: Obj[]): [string, Obj[]][] {
  const batches = new Map<string, Obj[]>();
  for (const entry of entries) { const key = str(entry.environmentId); batches.set(key, [...(batches.get(key) ?? []), entry]); }
  return [...batches];
}
export const asEntries = (value: unknown): Obj[] => arr(value);
