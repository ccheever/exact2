// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r6-polish-refs.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r6-polish: "Loading more refs..." (BranchToolbarBranchSelector / BranchPicker
// isFetchingNextPage, T3 Code MIT; see LICENSE-T3). A scroll toward a ref list's
// end is answered twice (r5-composer-paging.ts morePages): first with the
// status, then with the page. The second answer comes from the root's shell
// clock, which runs while a list still wants its page (shell.ts `ticking`); the
// card's resource and the strip's both take that clock.
import type { T3Client } from './client';
import { pageWanted } from './r5-composer-paging';
import { cardPages } from './r4-git-branch';
import { stripPages } from './composer-controls-branch';

export function refsPageWanted(client: T3Client): boolean {
  if (pageWanted(cardPages(client), client.presentation, 'details-refs')) return true;
  return stripPages(client).some(entry => pageWanted({ nextCursor: entry.nextCursor ?? null, ends: entry.ends ?? 0 }, client.presentation, 'strip-refs'));
}
