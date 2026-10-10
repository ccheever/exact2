// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r9-input-hover.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r9-input: a sidebar row's hover key names its place and how many times it has moved.
// Natively one node is hovered at a time and a node that unmounts under a still pointer sends
// no hover-out, so a press that moves its row (Settle, Pin, Snooze) left the window's hovered
// key behind; when ⌘Z (thread.undo) moved the row back, that stale key matched again and the
// "Settle thread" tooltip and the row's hover actions painted with the pointer elsewhere. Each
// move to another shelf gives the row a fresh key, so only a new hover can reveal it again.
import type { T3Client } from './client';

type Place = { section: string; moves: number };
const places = new WeakMap<T3Client, Map<string, Place>>();

/** Every thread's shelf this snapshot, painted or not (a collapsed Settled shelf paints none of its rows). */
export function notePlaces(client: T3Client, parts: object): void {
  for (const [section, threads] of Object.entries(parts) as [string, { id?: unknown }[]][]) for (const thread of threads) rowHoverKey(client, String(thread.id ?? ''), section);
}

/** `${id}~${section}` for a row that has stayed put, `${id}~${section}~${moves}` once it has moved. */
export function rowHoverKey(client: T3Client, id: string, section: string): string {
  let seen = places.get(client);
  if (!seen) { seen = new Map(); places.set(client, seen); }
  const place = seen.get(id);
  if (!place) { seen.set(id, { section, moves: 0 }); return `${id}~${section}`; }
  if (place.section !== section) { place.section = section; place.moves++; }
  return place.moves ? `${id}~${section}~${place.moves}` : `${id}~${section}`;
}
