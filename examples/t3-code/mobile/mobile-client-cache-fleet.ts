// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
// @ref llp/1109.004-home-projection.decision.md#decision
import { str, type Obj, type Shell } from './shared/domain';
import type { Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { decodeMobileCatalogPayload, encodeMobileCatalogPayload, mobileCacheCatalogIdentity, mobileCacheCatalogCurrent, mobileCacheCatalogLive, mobileCacheCatalogPrepare } from './mobile-client-cache-catalog';
import { type EnvironmentFleet, type FleetEntry, type FocusedHost, trimOrigin } from './shared/settings-b-fleet';
import { mobileCacheReadDecoded, mobileCacheReadRevision, mobileCacheTicket, mobileCacheWrite, type MobileCacheKey } from './mobile-client-cache';
import { decodeMobileConfigCache, decodeMobileShellCache, encodeMobileConfigCache, encodeMobileShellCache } from './mobile-client-cache-codec';

/** Display facts only. Never install these objects into a FleetEntry: its
 * synchronized/config/shell fields participate in authoritative RPC ownership. */
export interface MobileFleetCacheDisplay {
  environmentId: string; origin: string; enabled: boolean; connected: false;
  shell: Shell; config: Obj;
}
interface RetainedDisplay { value: MobileFleetCacheDisplay; clearRevision: string }
const displays = new WeakMap<EnvironmentFleet, Map<string, RetainedDisplay>>();
const serials = new WeakMap<EnvironmentFleet, number>();
type Saved = () => readonly Obj[];
function records(fleet: EnvironmentFleet) {
  let retained = displays.get(fleet);
  if (!retained) { retained = new Map(); displays.set(fleet, retained); }
  return retained;
}
function catalogRows(rows: readonly Obj[]) {
  // Ambiguous identities must not join one environment's cached data to another
  // route. The native catalog normally guarantees uniqueness; fail closed here.
  const counts = new Map<string, number>();
  for (const row of rows) counts.set(str(row.environmentId), (counts.get(str(row.environmentId)) ?? 0) + 1);
  return rows.filter(row => str(row.environmentId) && str(row.origin) && counts.get(str(row.environmentId)) === 1);
}
const entryFor = (fleet: EnvironmentFleet, environmentId: string) =>
  [...fleet.entries.values()].find(entry => entry.environmentId === environmentId);
const connected = (entry: FleetEntry | undefined) => entry?.phase === 'connected';
const catalogIdentity = (row: Obj) => JSON.stringify([row.environmentId, trimOrigin(str(row.origin)), row.enabled !== false]);

/** Synchronous projection rechecks clear/catalog/live state on every read. A
 * disabled saved environment retains its disposable data; a forgotten one loses
 * visibility immediately. The caller chooses whether disabled rows are shown. */
export function mobileCacheFleetDisplays(fleet: EnvironmentFleet, focused: FocusedHost,
  saved: Saved = () => fleet.saved): MobileFleetCacheDisplay[] {
  mobileCacheCatalogCurrent(focused, saved(), focused.environmentId);
  const rows = new Map(catalogRows(saved()).map(row => [str(row.environmentId), row]));
  const result: MobileFleetCacheDisplay[] = [];
  for (const [environmentId, display] of records(fleet)) {
    const row = rows.get(environmentId);
    if (!row || !mobileCacheCatalogCurrent(focused, saved(), environmentId) || trimOrigin(str(row.origin)) !== display.value.origin
      || mobileCacheReadRevision(environmentId) !== display.clearRevision) {
      records(fleet).delete(environmentId); continue;
    }
    // Even a connected-but-denied or unsynchronized entry suppresses cache. A
    // cached read must not look like success for that currently failed request.
    if (environmentId === focused.environmentId || connected(entryFor(fleet, environmentId))) continue;
    result.push({ ...display.value, enabled: row.enabled !== false });
  }
  return result;
}

/** Prepared awaited consumer, to run after fleet.sync commits. It owns no
 * transport, native handle, Promise tail or timer beyond this calling answer. */
export async function mobileCacheFleetSync(fleet: EnvironmentFleet, native: Native | null | undefined,
  focused: FocusedHost, saved: Saved = () => fleet.saved): Promise<void> {
  const serial = (serials.get(fleet) ?? 0) + 1; serials.set(fleet, serial);
  mobileCacheFleetDisplays(fleet, focused, saved);
  await mobileCacheCatalogPrepare(focused, native, saved);
  if (!native?.available) return;
  const catalog = saved(), revision = fleet.revision;
  const focus = JSON.stringify([focused.environmentId, focused.origin, focused.connection]);
  const currentPass = () => serials.get(fleet) === serial && fleet.revision === revision && saved() === catalog
    && JSON.stringify([focused.environmentId, focused.origin, focused.connection]) === focus;
  for (const row of catalogRows(catalog)) {
    if (!currentPass()) return;
    const environmentId = str(row.environmentId), origin = trimOrigin(str(row.origin));
    if (environmentId === focused.environmentId || !mobileCacheCatalogCurrent(focused, saved(), environmentId)) continue;
    const entry = entryFor(fleet, environmentId), identity = catalogIdentity(row);
    const savedIdentity = mobileCacheCatalogIdentity(catalog, environmentId);
    const liveSnapshot = mobileCacheCatalogLive(focused, saved(), environmentId, 'fleet', entry?.generation,
      entry ? [entry.shell, entry.config] : []);
    const generation = entry?.generation, synchronized = entry?.synchronized, phase = entry?.phase;
    const shellOwner = entry?.shell, configOwner = entry?.config, entryOrigin = entry?.origin;
    const clearRevision = mobileCacheReadRevision(environmentId);
    const current = () => currentPass() && mobileCacheReadRevision(environmentId) === clearRevision
      && mobileCacheCatalogCurrent(focused, saved(), environmentId)
      && catalogRows(saved()).some(candidate => catalogIdentity(candidate) === identity)
      && entryFor(fleet, environmentId) === entry && entry?.generation === generation
      && entry?.synchronized === synchronized && entry?.phase === phase && entry?.origin === entryOrigin
      && entry?.shell === shellOwner && entry?.config === configOwner;
    const key = (kind: 'shell' | 'server-config'): MobileCacheKey => ({ environmentId, kind, key: kind === 'shell' ? 'snapshot' : 'config' });
    try {
      if (connected(entry)) {
        records(fleet).delete(environmentId);
        if (!entry || row.enabled === false || entry.synchronized !== entry.generation
          || trimOrigin(entry.origin) !== origin || entry.busy) continue;
        const persist = async (kind: 'shell' | 'server-config', encode: () => string) => {
          if (!current() || entry.busy || !liveSnapshot(kind === 'shell' ? entry.shell : entry.config)) return;
          const ticket = await mobileCacheTicket(native, key(kind));
          if (!current() || entry.busy || !liveSnapshot(kind === 'shell' ? entry.shell : entry.config)) return;
          const payload = encodeMobileCatalogPayload(savedIdentity, encode());
          await mobileCacheWrite(native, key(kind), ticket, payload);
        };
        await persist('shell', () => encodeMobileShellCache(environmentId, entry.shell));
        await persist('server-config', () => encodeMobileConfigCache(environmentId, entry.config));
        continue;
      }
      const prior = records(fleet).get(environmentId);
      if (prior?.clearRevision === clearRevision && prior.value.origin === origin) continue;
      const shell = await mobileCacheReadDecoded(native, key('shell'), payload => decodeMobileShellCache(decodeMobileCatalogPayload(payload, savedIdentity) ?? '', environmentId), current);
      if (!current() || !shell) continue;
      const config = await mobileCacheReadDecoded(native, key('server-config'), payload => decodeMobileConfigCache(decodeMobileCatalogPayload(payload, savedIdentity) ?? '', environmentId), current);
      if (!current()) continue;
      records(fleet).set(environmentId, { clearRevision,
        value: { environmentId, origin, enabled: row.enabled !== false, connected: false, shell, config: config ?? {} } });
    } catch (error) {
      if (letGo(error)) throw error;
      // Cache failure cannot fail another environment's successful live refresh.
      // Actual storage failures remain visible through Client Storage inspection.
    }
  }
}
