// @ref llp/1106.011-responsive-workspace.decision.md#source-helpers
// Source: T3 Code 365aa87982a4d81cc8e0c085e8d1a40ca7daecdc apps/mobile/src/lib/adaptive-navigation.ts
// Original SHA256: e19f8ca82b61dc92758e665c6a6574466d6e83cce5eb60b95d3d614939764be4
// Body unchanged except local imports and platform-only type references.
import type { NavigationState, ThreadParams } from "./adaptive-navigation-types";

export type AdaptiveNavigationAction = "push" | "replace" | "set-params";

const BASE_THREAD_ROUTE_PATTERN = /^\/threads\/[^/]+\/[^/]+\/?$/;

export function isBaseThreadRoute(pathname: string): boolean {
  return BASE_THREAD_ROUTE_PATTERN.test(pathname);
}

/**
 * A persistent sidebar selects a peer destination in place. A compact list
 * drills into a new destination so the native back stack remains available.
 * From Home the selection pushes (never replaces) so Home stays beneath the
 * thread — collapsing back to a compact width keeps a sane back stack.
 */
export function resolveThreadSelectionNavigationAction(input: {
  readonly usesSplitView: boolean;
  readonly pathname: string;
}): AdaptiveNavigationAction {
  if (!input.usesSplitView || input.pathname === "/") {
    return "push";
  }

  return isBaseThreadRoute(input.pathname) ? "set-params" : "replace";
}

/** Dismiss sheets and select their underlying workspace destination in one stack update. */
export function resolveThreadSelectionOverlayState(input: {
  readonly state: NavigationState | undefined;
  readonly workspaceRouteKey: string | undefined;
  readonly action: AdaptiveNavigationAction;
  readonly params: ThreadParams;
}) {
  if (input.state === undefined) return null;
  const workspaceIndex = input.state.routes.findIndex(
    (route) => route.key === input.workspaceRouteKey,
  );
  if (workspaceIndex < 0 || workspaceIndex >= input.state.index) return null;

  const workspaceRoute = input.state.routes[workspaceIndex];
  const routes = input.state.routes.slice(0, workspaceIndex + (input.action === "push" ? 1 : 0));
  return {
    ...input.state,
    index: routes.length,
    routes: [
      ...routes,
      input.action === "set-params" && workspaceRoute?.name === "Thread"
        ? { ...workspaceRoute, params: { ...workspaceRoute.params, ...input.params } }
        : { name: "Thread", params: input.params },
    ],
  };
}

/**
 * On regular-width layouts, the file browser and preview occupy one workspace
 * destination. Replacing the browser route keeps a single back step to chat.
 * Compact layouts retain the browser as the previous stack screen.
 */
export function resolveFileSelectionNavigationAction(input: {
  readonly hasPersistentFileInspector: boolean;
}): AdaptiveNavigationAction {
  return input.hasPersistentFileInspector ? "replace" : "push";
}
