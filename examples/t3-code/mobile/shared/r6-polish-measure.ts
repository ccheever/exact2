// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r6-polish-measure.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// r6-polish: laid-out widths read back from the t3-anchor hook (T3Composer.swift
// reports each hooked box as [x, width] in its parent) instead of estimated
// advance tables: the sidebar's row tooltips (r3-sidebar-tips.ts, probes drawn by
// r6-polish.contract) and the Files breadcrumbs' folder menu (r4-surfaces-files.ts,
// each crumb hooked as `crumb:<path>`). Null until the box has been laid out.
import { obj, type Obj } from './domain';
import { probe, probeKey, type Probe } from './r5-composer-menus';

/** A hooked box's [x, width] in its parent, or null before it is laid out. */
export function anchorFrame(presentation: Obj, key: string): [number, number] | null {
  const value = obj(presentation.anchors)[key];
  if (!Array.isArray(value)) return null;
  const x = Number(value[0]), width = Number(value[1]);
  return Number.isFinite(x) && Number.isFinite(width) && width > 0 ? [x, width] : null;
}

/** A probed text's width (r6-polish.contract R6TextProbes), or null before its probe reports. */
export type TextMeasure = (text: string, size: number, weight: number) => number | null;
export const textMeasure = (presentation: Obj): TextMeasure => (text, size, weight) => anchorFrame(presentation, probeKey(text, size, weight))?.[1] ?? null;

/** The sidebar row texts r3-sidebar-tips.ts places tooltips by: the Settle and Woke labels and each left-anchored tooltip's text. */
export const SIDEBAR_PROBES: Probe[] = [probe('Settle', 12, 400), probe('Woke', 12, 500), probe('Unsent draft', 12, 400)];

/**
 * The AlertStack's clip (chat.contract): its full-width wrapper (`pointer-events-none
 * absolute inset-x-0` in the reference) would take presses in its side gutters,
 * since a box hit-tests natively whatever its pointer-events; a clip path also
 * bounds hit-testing, so the wrapper is clipped to its banners' horizontal span.
 * "none" until the wrapper and a banner have been laid out.
 */
export function alertClip(presentation: Obj): string {
  const wrapper = anchorFrame(presentation, 'alerts');
  const banners = ['alert:provider', 'alert:error', 'alert:uncertain'].map(key => anchorFrame(presentation, key)).filter((frame): frame is [number, number] => !!frame);
  if (!wrapper || !banners.length) return 'none';
  const left = Math.min(...banners.map(([x]) => x)), right = Math.max(...banners.map(([x, width]) => x + width));
  if (left < 0 || right > wrapper[1] + 0.5 || right <= left) return 'none';
  return `path('M ${left} 0 H ${right} V 4000 H ${left} Z')`;
}
