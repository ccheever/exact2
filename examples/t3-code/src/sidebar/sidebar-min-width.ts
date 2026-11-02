// The thread sidebar's minimum width as the reference derives it
// (threadSidebarWidth.ts resolveThreadSidebarMinimumWidth, SidebarChrome.tsx
// SidebarBrandWidthProbe, AppSidebarLayout.tsx; 2359643cff, MIT, see LICENSE-T3):
// max(13 × 16 px, ceil(the brand probe's width)), so "T3 Code" and the Go back and
// Go forward buttons after it never clip. The probe is the brand mark after the
// title bar's content inset, then `gap-1` and two 1.75rem controls, with `pr-3`
// and a 1 px border: on the macOS desktop, 90 px of traffic lights + 1.75rem
// control + 0.75rem gap, the mark, 0.25rem, 3.5rem, 0.75rem, 1 px. Its rems follow the root font size, which
// is the Interface font size setting (appearanceFonts.ts applyAppearanceFontVariables).
// The mark's widths are the reference's, measured per size in Chrome on macOS
// with a script over the reference page; 16 px gives 256.78.
import { clampInterfaceFontSize } from '../appearance/appearance-fonts';
export const SIDEBAR_FLOOR_WIDTH = 13 * 16;
export const MACOS_TRAFFIC_LIGHTS_INSET = 90;
/**
 * --workspace-controls-left (AppSidebarLayout.tsx, index.css): the traffic lights' 90 px on the
 * macOS desktop, or the plain 0.75rem once the window is in full screen (no traffic lights).
 */
export function workspaceControlsLeft(fontSize: unknown, fullScreen: unknown = false): number {
  return fullScreen === true ? 0.75 * clampInterfaceFontSize(fontSize) : MACOS_TRAFFIC_LIGHTS_INSET;
}
/** SidebarBrandMark's width (T3 wordmark at 1cap, gap-1, "Code" in text-sm) per Interface font size, 12–20 px. */
const BRAND_MARK: Record<number, number> = { 12: 41.15625, 13: 44.328125, 14: 47.515625, 15: 50.625, 16: 53.78125, 17: 56.875, 18: 60.015625, 19: 63.03125, 20: 66.1875 };

/** The brand mark's width at an Interface font size: where the sidebar header's Go back button starts, 0.25rem after it. */
export const brandMarkWidth = (fontSize: unknown): number => BRAND_MARK[clampInterfaceFontSize(fontSize)]!;
/** The brand probe's border-box width on the macOS desktop at an Interface font size (windowed or full screen). */
export function brandProbeWidth(fontSize: unknown, fullScreen: unknown = false): number {
  const rem = clampInterfaceFontSize(fontSize);
  return workspaceControlsLeft(rem, fullScreen) + 2.5 * rem + brandMarkWidth(rem) + 0.25 * rem + 3.5 * rem + 0.75 * rem + 1;
}
/** resolveThreadSidebarMinimumWidth(brandWidth). */
export function sidebarMinimumWidth(fontSize: unknown, fullScreen: unknown = false): number {
  return Math.max(SIDEBAR_FLOOR_WIDTH, Math.ceil(brandProbeWidth(fontSize, fullScreen)));
}
/** clampThreadSidebarWidth(width, minimum, resolveThreadSidebarMaximumWidth(viewport, minimum)). */
export function clampSidebarWidth(width: number, viewportWidth: number, minimum: number): number {
  const maximum = Math.max(minimum, Math.floor(viewportWidth) - 40 * 16);
  return Math.min(maximum, Math.max(minimum, width));
}
