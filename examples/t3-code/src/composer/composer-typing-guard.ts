// An approval or question that arrives while the user types their thread
// draft waits until they stop: 1.5 s without typing, leaving the composer, or
// sending. Adapted from T3 Code (MIT); see LICENSE-T3. Sources at 9c161462c9:
// chat/composerTypingGuard.ts and chat/useComposerTypingGuard.ts. The window
// owns the typing clock (app.contract `composerTyping`: a draft edit in the
// focused composer, cleared after COMPOSER_TYPING_IDLE_MS, on blur and on send)
// and passes it with each snapshot; this module holds the requests it saw arrive.

export const COMPOSER_TYPING_IDLE_MS = 1500;

type Guard = { scope: string; seen: Set<string>; held: Set<string> };
const guards = new WeakMap<object, Guard>();
const typing = new WeakMap<object, boolean>();

/** The window's typing flag for this client, as the latest snapshot request carried it. */
export function noteComposerTyping(client: object, value: boolean): void {
  typing.set(client, value);
}

/**
 * The requests held back from taking over the composer. A request first seen
 * while the user types is held until typing stops; one seen otherwise shows at
 * once. A new scope (another thread or draft) starts over with every current
 * request seen and none held.
 */
export function heldRequestIds(client: object, scope: string, requestIds: readonly string[]): ReadonlySet<string> {
  const current = guards.get(client);
  if (!current || current.scope !== scope) {
    const fresh = { scope, seen: new Set(requestIds), held: new Set<string>() };
    guards.set(client, fresh);
    return fresh.held;
  }
  const active = typing.get(client) === true;
  const held = new Set(active ? [...current.held].filter(id => requestIds.includes(id)) : []);
  for (const id of requestIds) {
    if (!current.seen.has(id) && active) held.add(id);
    current.seen.add(id);
  }
  current.held = held;
  return held;
}

/** A send releases what it held, as blur and idle do. */
export function releaseHeldRequests(client: object): void {
  const current = guards.get(client);
  if (current) current.held = new Set();
  typing.set(client, false);
}
