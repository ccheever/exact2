// The open row stays in view (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3): sidebar/routeReveal.ts
// pendingSidebarRouteReveal, completeSidebarRouteReveal and resolveSidebarRouteReveal. History (⌘[ ⌘]), the palette
// and links can open a thread the project scope hides or the list scrolled away, so each route change scrolls to the
// open row once, and only a route change may widen the scope. The window owns the record (the sidebar session), so a
// sidebar that remounts keeps the scope the user chose; its own once-per-mount scroll is the view's (sidebar.contract).
// A key of '' is not an observation: no thread open (a draft, a page) must not make the same thread look new.
export interface RouteReveal { observedKey: string; pendingKey: string }

/** Records the route; the reveal owed for it ('' for none). */
export function pendingRouteReveal(reveal: RouteReveal, routeKey: string): string {
  if (routeKey === '') return '';
  if (reveal.observedKey !== routeKey) { reveal.observedKey = routeKey; reveal.pendingKey = routeKey; }
  return reveal.pendingKey;
}
export function completeRouteReveal(reveal: RouteReveal, routeKey: string): void {
  if (reveal.pendingKey === routeKey) reveal.pendingKey = '';
}
/**
 * Records the route before the sidebar decides, so the first frame of a new route already widens the scope. Search
 * results replace the list, so the reveal waits for it to return. Widening needs a scope that hides the thread while
 * no scope shows it.
 */
export function resolveRouteReveal(input: { reveal: RouteReveal; routeKey: string; searching: boolean; scoped: boolean;
  hidden: (key: string) => boolean | undefined }): { pendingKey: string; widensScope: boolean } {
  const pendingKey = pendingRouteReveal(input.reveal, input.routeKey);
  const widensScope = pendingKey !== '' && !input.searching && input.scoped && input.hidden(pendingKey) === true;
  return { pendingKey, widensScope };
}
