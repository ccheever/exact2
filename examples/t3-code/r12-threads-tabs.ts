// Lane r12-threads: the right panel's tabs scroll when they overflow (T3 Code, MIT, see
// LICENSE-T3: RightPanelTabs.tsx's ScrollArea with scrollFade, updateTabScrollState's
// hasOverflow / canScrollLeft / canScrollRight with TAB_SCROLL_EDGE_TOLERANCE 1, scrollTabs'
// max(120, 75% of the viewport) step, the "Scroll tabs left/right" buttons, and the active tab
// scrolled into view, inline "nearest", when it changes). The 840 sheet's four restored tabs
// overflow its strip; without this the Diff tab was clipped and could not be reached.
// Widths come from the t3-anchor hook ([x within the parent, width]): the strip's viewport,
// its content row and each tab chip.
import { obj, type Obj } from './domain';

export type TabStrip = { overflow: boolean; view: number; content: number; max: number; activeLeft: number; activeRight: number; serial: number };
export const NO_TAB_STRIP: TabStrip = { overflow: false, view: 0, content: 0, max: 0, activeLeft: 0, activeRight: 0, serial: 0 };
/** RightPanelTabs: gap-1 between items, then the 24pt Add panel surface button. */
const GAP = 4, ADD = 24;
const EDGE = 1;

const anchor = (anchors: Obj, name: string): [number, number] | null => {
  const value = anchors[name];
  if (!Array.isArray(value) || value.length < 2) return null;
  const x = Number(value[0]), width = Number(value[1]);
  return Number.isFinite(x) && Number.isFinite(width) && width > 0 ? [x, width] : null;
};

/**
 * Active-tab changes: `serial` counts them, so the strip scrolls the new active tab into view
 * once (R4HeaderBar compares it with the serial it last scrolled for). Another panel (a launch,
 * a thread switch) is not a change: the reference's restored sheet opens at the strip's start.
 */
const seen = new WeakMap<object, { panel: string; active: string; serial: number }>();
export function activeSerial(owner: object, panel: string, active: string): number {
  const last = seen.get(owner);
  if (!last) { seen.set(owner, { panel, active, serial: 0 }); return 0; }
  if (last.panel !== panel) { last.panel = panel; last.active = active; }
  else if (last.active !== active) { if (last.active && active) last.serial++; last.active = active; }
  return last.serial;
}

/**
 * The strip's scroll geometry for the contract (R4HeaderBar keeps the scroll offset itself). The
 * content row is as wide as its chips and the "+" (`content`, which the row takes as its minimum
 * width so the strip has something to scroll); its own anchor is only the fallback.
 */
export function tabStrip(presentation: Obj, tabs: readonly string[], active: string, serial = 0): TabStrip {
  const anchors = obj(presentation.anchors);
  const view = anchor(anchors, 'r12-tabs-view');
  if (!view) return { ...NO_TAB_STRIP, serial };
  const chips = tabs.map(id => anchor(anchors, `r12-tab:${id}`)).filter((value): value is [number, number] => value !== null);
  const right = chips.reduce((edge, [x, width]) => Math.max(edge, x + width), 0);
  const content = chips.length ? Math.ceil(right + GAP + ADD) : (anchor(anchors, 'r12-tabs-content')?.[1] ?? 0);
  const max = Math.max(0, content - view[1]);
  const tab = active ? anchor(anchors, `r12-tab:${active}`) : null;
  return { overflow: max > EDGE, view: view[1], content, max, activeLeft: tab ? tab[0] : 0, activeRight: tab ? tab[0] + tab[1] : 0, serial };
}
