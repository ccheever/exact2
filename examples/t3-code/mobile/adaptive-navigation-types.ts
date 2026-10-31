// @ref llp/1038-router.rfc.md#d3--the-router-is-a-value-in-a-state-slot-the-compiler-declares-its-shapes
// Structural inputs for pinned adaptive-navigation.ts; no native navigator dependency.
export interface ThreadParams { readonly environmentId: string; readonly threadId: string }
export interface NavigationRoute {
  readonly key: string;
  readonly name: string;
  readonly params?: Readonly<Record<string, unknown>>;
}
export interface NavigationState {
  readonly index: number;
  readonly routes: readonly NavigationRoute[];
  readonly [key: string]: unknown;
}
