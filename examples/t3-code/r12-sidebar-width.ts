// Lane r12-sidebar: the thread sidebar's minimum width as the reference derives it
// (threadSidebarWidth.ts resolveThreadSidebarMinimumWidth, SidebarChrome.tsx
// SidebarBrandWidthProbe, AppSidebarLayout.tsx; f870c419fc, MIT, see LICENSE-T3):
// max(13 × 16 px, ceil(the brand probe's width)), so "T3 Code" never clips. The
// probe is the brand mark after the title bar's content inset with `pr-3` and a
// 1 px border: on the macOS desktop, 90 px of traffic lights + 1.75rem control +
// 0.75rem gap, the mark, 0.75rem, 1 px. Its rems follow the root font size, which
// is the Interface font size setting (appearanceFonts.ts applyAppearanceFontVariables).
// The mark's widths are the reference's, measured per size in Chrome on macOS
// (lanes/r12-sidebar/tools/refbrand.mjs); 16 px gives 196.78, under the floor.
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

export function clampInterfaceFontSize(size: unknown): number {
  const n = typeof size === 'number' && Number.isFinite(size) ? Math.round(size) : 16;
  return Math.min(20, Math.max(12, n));
}
/** The brand probe's border-box width on the macOS desktop at an Interface font size (windowed or full screen). */
export function brandProbeWidth(fontSize: unknown, fullScreen: unknown = false): number {
  const rem = clampInterfaceFontSize(fontSize);
  return workspaceControlsLeft(rem, fullScreen) + 2.5 * rem + BRAND_MARK[rem]! + 0.75 * rem + 1;
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
