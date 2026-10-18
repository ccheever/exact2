// "Loading more refs..." (BranchToolbarBranchSelector / BranchPicker
// isFetchingNextPage, T3 Code MIT; see LICENSE-T3). A scroll toward a ref list's
// end is answered twice (ref-paging.ts morePages): first with the
// status, then with the page. The second answer comes from the root's shell
// clock, which runs while a list still wants its page (shell.ts `ticking`); the
// card's resource and the strip's both take that clock.
import type { T3Client } from '../client/client';
import { pageWanted, refScrollEnds } from './ref-paging';
import { cardPages } from '../details/git-branch';
import { stripPages } from './composer-controls-branch';
import { scheduledRefsWanted } from '../settings/scheduled-view'; // the scheduled task's base-branch picker

export function refsPageWanted(client: T3Client): boolean {
  if (pageWanted(cardPages(client), refScrollEnds(client), 'details-refs') || scheduledRefsWanted(client)) return true;
  return stripPages(client).some(entry => pageWanted({ nextCursor: entry.nextCursor ?? null, ends: entry.ends ?? 0 }, refScrollEnds(client), 'strip-refs'));
}
