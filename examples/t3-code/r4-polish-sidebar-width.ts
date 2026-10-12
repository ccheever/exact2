// Lane r4-polish: the thread sidebar's width as the reference keeps it
// (threadSidebarWidth.ts, AppSidebarLayout.tsx and ui/sidebar.tsx SidebarRail;
// MIT, see LICENSE-T3). The rail stores a width (localStorage
// `chat_thread_sidebar_width`) only when a drag ends, and a double-click reset
// forgets it. With no stored width the sidebar is sized once, when the layout
// mounts, to min(16rem, max(13rem, viewport - 40rem)) and keeps that size when
// the window later resizes. With a stored width the rail's layout effect
// re-clamps it to [13rem, max(13rem, viewport - 40rem)] on every layout pass, so
// it follows the window. The stored flag lives in the client's preference file
// (app:/data/t3-code.json) beside `sidebarWidth`.
import type { Obj } from './domain';
import { clampSidebarWidth, sidebarMinimumWidth } from './r12-sidebar-width';

export const SIDEBAR_DEFAULT_WIDTH = 256, SIDEBAR_MIN_WIDTH = 208, MAIN_CONTENT_MIN_WIDTH = 640;
type Holder = { local: object };
type Prefs = { sidebarWidth?: number; sidebarWidthStored?: boolean };

/** resolveThreadSidebarMaximumWidth. */
export function sidebarMaximumWidth(viewportWidth: number): number {
  return Math.max(SIDEBAR_MIN_WIDTH, Math.floor(viewportWidth) - MAIN_CONTENT_MIN_WIDTH);
}
/** resolveInitialThreadSidebarWidth: the stored width (at least 13rem) or 16rem, capped by the room left for 40rem of main content. */
export function initialSidebarWidth(stored: number | null, viewportWidth: number): number {
  const preferred = stored === null ? SIDEBAR_DEFAULT_WIDTH : Math.max(SIDEBAR_MIN_WIDTH, stored);
  return Math.min(preferred, sidebarMaximumWidth(viewportWidth));
}

/** Whether a dragged width is stored (the reference's localStorage key exists). */
export function sidebarWidthStored(owner: Holder): boolean {
  return (owner.local as Prefs).sidebarWidthStored === true;
}
/**
 * load(): carry the stored flag. Files written before the flag existed saved a
 * width on every sidebar toggle; only a width other than the default can have
 * come from a drag, so only that one counts as stored.
 */
export function adoptSidebarWidth(next: object, saved: Obj): void {
  const width = saved.sidebarWidth;
  (next as Prefs).sidebarWidthStored = typeof saved.sidebarWidthStored === 'boolean' ? saved.sidebarWidthStored
    : typeof width === 'number' && Number.isFinite(width) && width !== SIDEBAR_DEFAULT_WIDTH;
}
/** A drag ended (SidebarRail finish): the width is now stored. */
export function storeSidebarWidth(owner: Holder): void {
  (owner.local as Prefs).sidebarWidthStored = true;
  launched.delete(owner);
}
/** The rail's reset: forget the stored width and size the sidebar again from the current window. */
export function resetSidebarWidth(owner: Holder): void {
  const local = owner.local as Prefs;
  local.sidebarWidth = SIDEBAR_DEFAULT_WIDTH;
  local.sidebarWidthStored = false;
  launched.delete(owner);
}

const launched = new WeakMap<object, number>();
/**
 * The `sidebarLaunch` source: while no width is stored, the width computed the
 * first time this window's viewport is seen after the preferences load (and
 * again after a reset); 0 while a width is stored or before the first real
 * answer, when the window clamps `data.sidebarWidth` to the live viewport.
 */
export function sidebarLaunchWidth(owner: Holder & { preferencesLoaded: boolean }, viewportWidth: number, live: boolean): number {
  if (!live || !owner.preferencesLoaded || !(viewportWidth > 0)) return 0;
  if (sidebarWidthStored(owner)) { launched.delete(owner); return 0; }
  let width = launched.get(owner);
  if (width === undefined) { width = initialSidebarWidth(null, viewportWidth); launched.set(owner, width); }
  // r12-sidebar: the layout clamps it to the live minimum (the brand at the Interface font size).
  return clampSidebarWidth(width, viewportWidth, sidebarMinimumWidth((owner.local as { clientSettings?: { fontSizeInterface?: number } }).clientSettings?.fontSizeInterface));
}
