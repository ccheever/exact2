// Lane r9-device: where the Files breadcrumbs settle (MIT reference, see LICENSE-T3:
// FilePreviewPanel.tsx). The reference scrolls the current crumb into view (`inline: "end"`, with
// the ScrollArea's 24pt scroll padding) once, when the file changes. Its word-wrap toggle appears
// only once the file's contents have loaded (`showsRawText` needs `file.data`), so for a file whose
// read was not yet cached the scroll ran while the crumbs' viewport was one 28pt toggle and its
// 8pt gap wider, and it stays there: the trail sits 36pt short of its end, its last crumb clipped on
// the right. A cached file (opened before in this session) scrolls fully to its end. Measured on
// the HEAD oracle at 840 × 620: scrollLeft 5 of 41 on a first open, 41 on a re-open.
import type { Obj } from './domain';
import { anchorFrame } from './r6-polish-measure';

export const LATE_TOGGLE = 28 + 8;

type Settle = { path: string; cold: boolean };
const settles = new WeakMap<object, Settle>();
const firstReads = new WeakMap<object, Set<string>>();

/** A file's first read started (nothing cached for it): the next time it becomes active it settles short. */
export function noteFirstRead(owner: object, path: string): void {
  let set = firstReads.get(owner);
  if (!set) { set = new Set(); firstReads.set(owner, set); }
  set.add(path);
}

/** Records the active file; whether it was uncached when it became active is fixed at that moment. */
export function settleCrumbs(owner: object, path: string): boolean {
  const current = settles.get(owner);
  if (current && current.path === path) return current.cold;
  const pending = firstReads.get(owner), cold = !!path && !!pending?.has(path);
  pending?.delete(path);
  settles.set(owner, { path, cold });
  return cold;
}

/** Lane r10-device: the preview mounted with `path` (r10-device-crumbs.ts): it settles at the end, read or not. */
export function settleMounted(owner: object, path: string): void {
  firstReads.get(owner)?.delete(path);
  settles.set(owner, { path, cold: false });
}

/** The trail's scroll offset when it settled short of its end (≥ 0), or -1 to end-justify it. */
export function crumbsOffset(presentation: Obj = {}, cold: boolean, rawText: boolean): number {
  if (!cold || !rawText) return -1;
  const trail = anchorFrame(presentation, 'crumbs'), clip = anchorFrame(presentation, 'crumbs-clip');
  if (!trail || !clip) return -1;
  const overflow = trail[1] - clip[1];
  if (overflow < 0.5) return -1;
  return Math.max(0, Math.round((overflow - LATE_TOGGLE) * 10) / 10);
}

/** ReadOnlySourcePreview's line-number column (@pierre/diffs `[data-column-number]`): content-box
 *  `${totalLines}`.length ch wide (SF Mono 13: 7.8267pt per ch), 2ch left and 1ch right padding and a
 *  2pt right border; measured on the HEAD oracle as 33.28 / 48.94 / 56.75pt for 1 / 3 / 4 digits. */
export const SOURCE_CH = 7.8267;
export const sourceGutter = (lineCount: number) => Math.round((String(Math.max(1, lineCount)).length * SOURCE_CH + 3 * SOURCE_CH + 2) * 100) / 100;
/** The number's inset from the column's right edge: 1ch padding and the 2pt border. */
export const SOURCE_NUMBER_INSET = Math.round((SOURCE_CH + 2) * 100) / 100;
