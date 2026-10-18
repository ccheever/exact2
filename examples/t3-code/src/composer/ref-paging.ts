// Ref pickers past their first page, adapted from T3 Code (MIT; see
// LICENSE-T3): state/queries.ts usePaginatedBranches (pages of VCS_REF_LIST_LIMIT
// refs by cursor, merged by name, the last page's cursor, the largest total) and
// BranchPicker.tsx maybeFetchNextBranchPage (a scroll toward the end within
// 96px loads the next page). The list's own `scroll` event tells it
// (ref-paging.contract RefPageScroll), and `ref-scroll-end` counts it here.
import { arr, num, str, type Obj } from '../client/domain';

export const REF_PAGE = 100;
// `loadingMore` is BranchPicker's isFetchingNextPage, shown for one answer before the page is read.
export type RefPages = { refs: Obj[]; total: number; nextCursor: number | null; ends: number; loadingMore?: boolean };

const reported = new WeakMap<object, Record<string, number>>();
/** Each ref list's scrolls toward its end so far, by `scroll:<name>`, for one client. */
export function refScrollEnds(client: object): Record<string, number> {
  let ends = reported.get(client);
  if (!ends) reported.set(client, ends = {});
  return ends;
}
/** `ref-scroll-end`: the list `name` was scrolled toward its end (RefPageScroll). */
export function noteRefScrollEnd(client: object, name: string): void {
  const ends = refScrollEnds(client);
  ends[`scroll:${name}`] = (ends[`scroll:${name}`] ?? 0) + 1;
}

/** How many qualifying scrolls the list `name` has reported. */
export function scrollEnds(ends: Obj, name: string): number {
  return num(ends[`scroll:${name}`], 0);
}

/** A fresh first page: earlier scrolls of the list are spent. */
export function firstPage(result: Obj, ends: number): RefPages {
  return { refs: arr(result.refs), total: num(result.totalCount, arr(result.refs).length), nextCursor: typeof result.nextCursor === 'number' ? result.nextCursor : null, ends };
}

/** usePaginatedBranches' merge: refs by name (a later page updates an earlier entry in place), the last cursor, the largest total. */
export function mergePage(pages: RefPages, result: Obj): RefPages {
  const byName = new Map<string, Obj>();
  for (const ref of pages.refs) byName.set(str(ref.name), ref);
  for (const ref of arr(result.refs)) byName.set(str(ref.name), ref);
  return { refs: [...byName.values()], total: Math.max(pages.total, num(result.totalCount, 0)),
    nextCursor: typeof result.nextCursor === 'number' ? result.nextCursor : null, ends: pages.ends };
}

/**
 * Loads the next page when the list was scrolled toward its end since the last
 * look and the server named a next cursor. Each scroll signal loads at most one page.
 * The first look only marks the page as loading (an answer shows its
 * status when it returns, so "Loading more refs..." needs an answer of its own);
 * the root's shell clock asks again while `pageWanted` holds, and that look reads it.
 */
export async function morePages(pages: RefPages, reports: Obj, name: string, fetch: (cursor: number) => Promise<Obj>): Promise<RefPages> {
  const ends = scrollEnds(reports, name);
  if (ends <= pages.ends) return pages;
  if (pages.nextCursor === null) return { ...pages, ends, loadingMore: false };
  if (!pages.loadingMore) return { ...pages, loadingMore: true };
  const result = await fetch(pages.nextCursor);
  return { ...mergePage(pages, result), ends, loadingMore: false };
}

/** A scroll toward the list's end has not been answered with its page yet (the shell clock keeps asking). */
export function pageWanted(pages: Pick<RefPages, 'nextCursor' | 'ends'> | null | undefined, reports: Obj, name: string): boolean {
  return !!pages && pages.nextCursor !== null && scrollEnds(reports, name) > pages.ends;
}

/** BranchPicker's status line ("Loading more refs..." is the moment the page is in flight). */
export function refsStatus(pages: RefPages | null, loading: boolean): string {
  if (loading) return 'Loading refs...';
  if (pages?.loadingMore) return 'Loading more refs...';
  return pages && pages.nextCursor !== null ? `Showing ${pages.refs.length} of ${pages.total} refs` : '';
}
