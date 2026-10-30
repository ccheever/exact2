// stage12-gate.mjs's iOS Dynamic Island rule (theme2's s15b2 ruling, 2026-10-29 15:32 (a): hardening, not a rerun),
// apart from the gate script for its line cap.
import { wholeLayout } from './stage12-named.mjs';

/** The gate simulator's sensor housing, [x, y, w, h] in points (an iOS shot is a pixel a point, 402 by 874): simctl's
 * capture draws the island in one shot and leaves it out of another, with the screen's top-left corner pixels, which
 * the same capture toggles with it. The system's, never the app's. */
export const ISLAND = [[138, 13, 126, 38], [0, 0, 2, 2]];
const REASON = 'the Dynamic Island: the simulator\'s capture draws its sensor housing (and the top-left corner pixels) in one shot and not another; over no library component';

const meets = (a, b) => a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3];
// A package's node, by the source map it carries: a file in the checkout's packages/ or a copy's node_modules/@exact/.
const library = n => /\/(?:packages|node_modules\/@exact)\/[\w-]+\/[\w-]+\.contract$/.test(n.sourceMap?.file ?? '');

/** The island's allowlist entries for an iOS drive's output: each rect only where no library component's box in the
 * drive's whole layout meets it; none on another host or without a layout. */
export function islandAllow(host, out) {
  if (host !== 'ios') return [];
  const layout = wholeLayout(out);
  if (!layout) return [];
  const boxes = layout.nodes.filter(n => library(n) && n.w > 0 && n.h > 0).map(n => [n.x, n.y, n.w, n.h]);
  return ISLAND.filter(rect => !boxes.some(b => meets(rect, b))).map(rect => ({ rect, reason: REASON }));
}
