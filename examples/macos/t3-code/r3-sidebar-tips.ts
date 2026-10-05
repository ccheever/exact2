// Lane r3-sidebar: where a sidebar row's control tooltips land (Sidebar.tsx
// TooltipPopup side top, 4pt off, centered on the trigger; MIT, see
// LICENSE-T3). The bubbles are drawn by the window overlay, never inside the
// thread list's scroll view, so the top row's bubble is not clipped. Each
// control's place follows the row's layout (sidebar-row.contract) from the
// sidebar's edges: list inset 8, row inset 10, controls' fixed sizes and the
// measured system-font labels.

export interface SidebarRowTip { name: string; label: string; x: number; fromRight: boolean; top: number; start: boolean }
export interface RowFacts { card: boolean; draft: boolean; pinned: boolean; canPin: boolean; woke: boolean; canSnooze: boolean; canSettle: boolean; canWake: boolean; canUnsettle: boolean }

/** The Settle action's text and the Woke button's medium text as measured on the served reference: stand-ins until the probes report. */
export const SETTLE_TEXT = 33.5, WOKE_TEXT = 30;
const EDGE = 8 + 10;
/** Base UI's collision padding: a bubble that would cross the window's left edge shifts to start 5pt in. */
const COLLISION = 5;
/** r6-polish: a laid-out text's width (r6-polish-measure.ts), null before its probe reports. */
export type Measure = (text: string, size: number, weight: number) => number | null;
/** A 12pt system label's bubble width (8pt insets and 1pt borders), for the left-edge shift only: the measured label; before its probe reports, an average advance. */
export const bubbleWidth = (label: string, measure?: Measure) => (measure?.(label, 12, 400) ?? label.length * 6.2) + 18;

/** The control tooltips a row offers while hovered: name, label, center x (from the left or right edge) and the control's top. */
export function rowTips(row: RowFacts, measure?: Measure): SidebarRowTip[] {
  const tips: SidebarRowTip[] = [];
  const SETTLE = measure?.('Settle', 12, 400) ?? SETTLE_TEXT, WOKE = measure?.('Woke', 12, 500) ?? WOKE_TEXT;
  const tip = (name: string, label: string, x: number, fromRight: boolean, top: number) => {
    const shift = !fromRight && x - bubbleWidth(label, measure) / 2 < COLLISION;
    tips.push({ name, label, x: shift ? COLLISION : x, fromRight, top, start: shift });
  };
  if (row.card) {
    // Header row: 2pt list gap + 8pt card inset; the actions end 4pt past the inset (Settle's -mr-1).
    const settle = 6 + 14 + 4 + SETTLE + 6, woke = 16 + 4 + WOKE;
    const actions = (row.draft ? 26 : 0) + (row.canSnooze ? 24 : 0) + (row.canSettle ? settle - 4 : 0);
    let right = EDGE - (row.canSettle ? 4 : 0);
    if (row.canSettle) { tip('settle', 'Settle thread', right + settle / 2, true, 10); right += settle; }
    if (row.canSnooze) { tip('snooze', 'Snooze thread', right + 12, true, 10); right += 24; }
    if (row.draft) { tip('discard', 'Discard draft', right + 13, true, 10); right += 26; }
    if (row.woke) tip('woke', 'Dismiss Woke notification', EDGE + actions + woke / 2, true, 12);
    const slot = Math.max(32, (row.woke ? woke : 0) + actions);
    if (row.pinned && row.canPin) tip('unpin', 'Unpin thread', EDGE + slot + 6 + 6, true, 14);
    if (row.draft) tip('draft', 'Unsent draft', 8 + 10 + 6, false, 15);
    return tips;
  }
  // Slim rows: 36pt, items centered; the slot holds Woke, then Wake or Un-settle (-mr-1).
  const woke = 12 + 4 + WOKE, trailing = row.canWake ? 20 : row.canUnsettle ? 22 : 0;
  if (row.canUnsettle && !row.canWake) tip('unsettle', 'Un-settle thread', EDGE - 4 + 13, true, 6);
  if (row.woke) tip('woke', 'Dismiss Woke notification', EDGE + trailing + woke / 2, true, 10);
  const slot = Math.max(32, (row.woke ? woke : 0) + trailing);
  // A PR badge between the pin and the slot is not measured: the bubble then sits over the badge.
  if (row.pinned && row.canPin) tip('unpin', 'Unpin thread', EDGE + slot + 10 + 6, true, 12);
  if (row.draft) tip('draft', 'Unsent draft', 8 + 10 + 16 + 10 + 6, false, 12);
  return tips;
}
