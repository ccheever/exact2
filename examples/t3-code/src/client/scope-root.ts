// The window's client behind a thread scope (thread-scope.ts), apart so that stores keyed by the client can find it
// without importing the scope.
export const SCOPE_ROOT = Symbol('thread-scope-root');
/** The prefix a scope's own layout frames and anchors carry (side-chat.contract's ids). */
export const SCOPE_IDS = 'side-chat-';

/** The window's client behind a scope (the client itself otherwise). */
export function windowClient<T extends object>(client: T): T {
  return ((client as Record<symbol, unknown>)[SCOPE_ROOT] as T | undefined) ?? client;
}
export const isThreadScope = (client: object): boolean => windowClient(client) !== client;
