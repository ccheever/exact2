// A held import's brand offers (*1.18*, theme2's brand-fill ruling) appear only on the import's first proposal. Its
// ink was measured on the design and the accent as that import proposed them, so any re-hold (a partial accept, an
// edit, a recompute after the files moved) drops every brand offer together with all its operations, fill and ink,
// and the review says to import again for them (theme2's 06:31 ruling, the scope cut).

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/** Every brand offer's items in `review`: what a re-hold leaves out. */
export const brandItems = review => new Set((review?.brands ?? []).flatMap(b => b.items ?? []));

/** The review's note when a re-hold drops brand offers. */
export const droppedNote = 'its brand fills were offered on the import\'s first proposal only, measured on what it proposed: import again to get brand offers';

/** A brand offer whose ink doesn't reach 4.5:1 in a scheme (or has no one ratio there): a bare accept leaves it out. */
export const belowContrast = b => [b.contrast?.light, b.contrast?.dark].some(r => typeof r !== 'number' || r < 4.5);

/** The items a partial accept of `chosen` (indices) keeps of a proposal held with `review`: no brand offer's, but for
 * those it names whole when it keeps every one of the import's own token operations and `unchanged()` (the files'
 * content digests are still those it was held on: theme2's 06:44 exception), since the ink was then measured on
 * exactly what will be written. `{kept, brands}`: the items, and whether brand offers are among them. */
export function partialKept(review, chosen, unchanged) {
  const all = brandItems(review);
  const named = new Set((review?.brands ?? []).filter(b => (b.items ?? []).length && b.items.every(i => chosen.includes(i))).flatMap(b => b.items));
  const tokens = Number.isInteger(review?.tokens) && Array.from({ length: review.tokens }, (_, i) => i).every(i => chosen.includes(i));
  const brands = named.size > 0 && tokens && unchanged();
  return { kept: chosen.filter(i => !all.has(i) || (brands && named.has(i))), brands };
}

/** Whether the files (path → content digest, from `appDir`) still hold that content: by digest, never by time. */
export const unchangedSince = (appDir, files) => Object.entries(files ?? {}).every(([path, d]) => { try { return createHash('sha256').update(readFileSync(resolve(appDir, path))).digest('hex') === d; } catch { return false; } });
