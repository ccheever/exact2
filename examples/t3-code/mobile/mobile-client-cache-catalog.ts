// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
import { obj, str, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import type { Native } from './shared/protocol';
import { mobileCacheClear } from './mobile-client-cache';

interface LiveBarrier { revision: number; generation: number; blocked: WeakSet<object> }
interface Catalog {
  known: Map<string, string>; pending: Map<string, number>; revisions: Map<string, number>;
  clearing: Set<string>; live: Map<string, LiveBarrier>; serial: number;
}
const catalogs = new WeakMap<object, Catalog>();
/** Saved identity, independent of whichever learned transport route is active. */
export function mobileCacheCatalogIdentity(rows: readonly Obj[], environmentId: string): string {
  const matches = rows.filter(row => row.environmentId === environmentId);
  const origin = matches.length === 1 ? str(matches[0]!.origin).trim().replace(/\/+$/, '') : '';
  return environmentId && origin ? JSON.stringify([environmentId, origin]) : '';
}
/** App-owned disk provenance. Unwrapped prepared payloads are disposable; they
 * cannot establish which saved identity wrote an environment-keyed record. */
function catalogEnvironment(catalogIdentity: string): string {
  const identity = JSON.parse(catalogIdentity);
  if (!Array.isArray(identity) || identity.length !== 2 || typeof identity[0] !== 'string'
    || !identity[0] || identity[0].trim() !== identity[0] || typeof identity[1] !== 'string'
    || !identity[1] || identity[1].trim().replace(/\/+$/, '') !== identity[1]
    || JSON.stringify(identity) !== catalogIdentity) throw new Error('Invalid mobile cache catalog identity.');
  return identity[0];
}
export function encodeMobileCatalogPayload(catalogIdentity: string, payload: string): string {
  const environmentId = catalogEnvironment(catalogIdentity);
  if (obj(JSON.parse(payload)).environmentId !== environmentId) throw new Error('Mobile cache payload environment changed.');
  // The native store validates this top-level field before accepting any bytes.
  return JSON.stringify({ format: 1, environmentId, catalogIdentity, payload });
}
export function decodeMobileCatalogPayload(payload: string, catalogIdentity: string): string | null {
  try {
    const environmentId = catalogEnvironment(catalogIdentity), record = obj(JSON.parse(payload));
    return record.format === 1 && record.environmentId === environmentId && record.catalogIdentity === catalogIdentity
      && typeof record.payload === 'string' && obj(JSON.parse(record.payload)).environmentId === environmentId ? record.payload : null;
  } catch { return null; }
}
function observe(owner: object, rows: readonly Obj[]): Catalog {
  let state = catalogs.get(owner);
  if (!state) { state = { known: new Map(), pending: new Map(), revisions: new Map(), clearing: new Set(), live: new Map(), serial: 0 }; catalogs.set(owner, state); }
  const identities = new Map<string, string>();
  for (const row of rows) {
    const id = str(row.environmentId);
    if (id) identities.set(id, mobileCacheCatalogIdentity(rows, id));
  }
  for (const [id, previous] of state.known) {
    if (identities.get(id) !== previous) {
      const revision = ++state.serial; state.pending.set(id, revision); state.revisions.set(id, revision);
    }
  }
  state.known = identities;
  return state;
}
/** Projection and pending reads both refuse a replaced/forgotten identity until
 * its durable bytes and native write tickets have been invalidated. */
export function mobileCacheCatalogCurrent(owner: object, rows: readonly Obj[], environmentId: string): boolean {
  const state = observe(owner, rows);
  return !!mobileCacheCatalogIdentity(rows, environmentId) && !state.pending.has(environmentId);
}
/** A catalog clear must not immediately re-save the previous identity's live
 * objects. A freshly adopted snapshot or a new live producer generation can
 * persist again. Weak membership never keeps an old transcript alive. */
export function mobileCacheCatalogLive(owner: object, rows: readonly Obj[], environmentId: string,
  producer: 'focused' | 'fleet', generation: number | undefined, snapshots: readonly object[]): (snapshot: object) => boolean {
  const state = observe(owner, rows), revision = state.revisions.get(environmentId) ?? 0;
  const key = JSON.stringify([environmentId, producer]);
  let barrier = state.live.get(key);
  if (!barrier || barrier.revision !== revision) {
    barrier = { revision, generation: generation ?? -1, blocked: new WeakSet(revision ? snapshots : []) };
    state.live.set(key, barrier);
  }
  const captured = barrier;
  return snapshot => generation !== captured.generation || !captured.blocked.has(snapshot);
}
/** Focused and fleet adapters share the focused client as owner. Only plain
 * identity/serial state survives; every clear belongs to this awaited answer.
 * Failures keep that environment blocked and retry on the next owned refresh. */
export async function mobileCacheCatalogPrepare(owner: object, native: Native | null | undefined,
  saved: () => readonly Obj[]): Promise<void> {
  const state = observe(owner, saved());
  if (!native?.available) return;
  for (const [environmentId, serial] of [...state.pending]) {
    if (state.clearing.has(environmentId)) continue;
    state.clearing.add(environmentId);
    try {
      await mobileCacheClear(native, { environmentId });
      observe(owner, saved());
      if (state.pending.get(environmentId) === serial) state.pending.delete(environmentId);
    } catch (error) { if (letGo(error)) throw error; }
    finally { state.clearing.delete(environmentId); }
  }
}
