// Lane r7-polish: the Files breadcrumbs as the reference scrolls them (MIT reference, see LICENSE-T3:
// FilePreviewPanel.tsx — a `ScrollArea scrollFade` around `w-max min-w-full` crumbs, scrolled with
// `scrollIntoView({ inline: "end" })` to the current file whenever the file changes; ui/scroll-area.tsx
// masks each overflowing edge over `min(1.5rem, overflow)`). The crumb row here is end-justified inside
// a clipping row (r4-surfaces-files.contract), so an overflowing trail shows its end, as the scrolled
// reference does; both rows are hooked (`t3-anchor`: `crumbs` is the trail's [x, width] in the
// clipping row, `crumbs-clip` the clip's [x, width] in the subheader), which gives the left fade and
// the shift the folder menu's anchor needs.
import type { Obj } from './domain';
import { anchorFrame } from './r6-polish-measure';

const FADE = 24; // --fade-size: 1.5rem

/** How far the trail sits left of its clip (0 while it fits or before layout). */
export function crumbsShift(presentation: Obj = {}): number {
  const trail = anchorFrame(presentation, 'crumbs');
  return trail && trail[0] < 0 ? trail[0] : 0;
}

/** The clip's mask: transparent at its left edge, opaque after min(24, overflow) points; "none" while the trail fits. */
export function crumbsMask(presentation: Obj = {}): string {
  const clip = anchorFrame(presentation, 'crumbs-clip'), hidden = -crumbsShift(presentation);
  // Lane r9-device: a trail that settled short of its end (r9-device-crumbs.ts) also fades its right edge.
  const trail = anchorFrame(presentation, 'crumbs'), after = clip && trail ? trail[0] + trail[1] - clip[1] : 0;
  if (!clip || (hidden < 0.5 && after < 0.5)) return 'none';
  const percent = Math.round(Math.min(FADE, hidden) / clip[1] * 1000) / 10;
  if (after < 0.5) return `linear-gradient(to right, #00000000 0%, #000000 ${percent}%)`;
  const end = Math.round((1 - Math.min(FADE, after) / clip[1]) * 1000) / 10;
  return `linear-gradient(to right, #${hidden < 0.5 ? '000000' : '00000000'} 0%, #000000 ${percent}%, #000000 ${end}%, #00000000 100%)`;
}
