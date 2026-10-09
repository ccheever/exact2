// @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
// Pinned365aa87982 state/assets.ts and state/runtime.ts. Root owns scheduling.
import { obj, str } from './shared/domain';
import { letGo } from './shared/let-go';
import { mobileFaviconMissing, mobileFaviconResourceKey, type MobileFaviconCache,
  type MobileFaviconIO, type MobileFaviconTarget } from './mobile-favicon-cache';

export const MOBILE_FAVICON_STALE_MS = 300_000;
export const MOBILE_FAVICON_REFRESH_MS = 1_800_000;
export const MOBILE_FAVICON_IDLE_MS = 3_600_000;
export interface MobileFaviconDemand extends MobileFaviconTarget { mountId: string }
/** Plain capture. scopeRevision must include favicon clear revision AND catalog identity. */
export interface MobileFaviconOwner {
  catalogIdentity: string; scopeRevision: string; origin: string; generation: number;
}
/** Never retained. request uses this environment's already-owned transport. */
export interface MobileFaviconEndpoint extends MobileFaviconOwner {
  environmentId: string; phase: 'connected' | 'waiting' | 'unavailable';
  clones: readonly { destinationPath?: unknown; phase?: unknown }[];
  current(): boolean;
  request(resource: { _tag: 'project-favicon'; cwd: string; path?: string }): Promise<unknown>;
}
export interface MobileFaviconObservation {
  key: string; serial: number; owner: MobileFaviconOwner;
}
export interface MobileFaviconView {
  revision: number; nextDeadline: number;
  items: { key: string; url: string; state: string; error: string }[];
}
type QueryState = 'initial' | 'waiting' | 'success' | 'failure';
interface Query {
  target: MobileFaviconTarget; mounts: Set<string>; owner: MobileFaviconOwner | null;
  url: string | null; state: QueryState; error: string; successAt: number | null;
  phase: string; clonePhase: string | null; cloneObserved: boolean;
  evaluated: boolean; idleAt: number; refreshAt: number; pending: boolean;
  serial: number; active: number; imageSerial: number;
}
const copyOwner = (owner: MobileFaviconOwner): MobileFaviconOwner => ({ catalogIdentity: owner.catalogIdentity,
  scopeRevision: owner.scopeRevision, origin: owner.origin, generation: owner.generation });
const sameOwner = (a: MobileFaviconOwner | null, b: MobileFaviconOwner) => !!a
  && a.catalogIdentity === b.catalogIdentity && a.scopeRevision === b.scopeRevision
  && a.origin === b.origin && a.generation === b.generation;
const fresh = (query: Query, now: number) => query.successAt !== null && now - query.successAt < MOBILE_FAVICON_STALE_MS;
const failure = (error: unknown) => error instanceof Error ? error.message : 'Could not load the project icon.';

/** URL query state only. The single application image cache is supplied by the
 * caller. Every transport, clock callback, signal and IO object is call-local.
 * No timers are created here; root services nextDeadline with its stated cadence.
 * Idle records retain their evaluated deadline until explicit TTL disposal.
 * This models retained query lifetime, not native virtual-cell mount events. */
export class MobileFaviconQueries {
  private queries = new Map<string, Query>();
  private revision = 0;
  private serial = 0;

  get version(): number { return this.revision; }
  private ensure(target: MobileFaviconTarget, now: number): Query {
    const key = mobileFaviconResourceKey(target);
    let query = this.queries.get(key);
    if (!query) {
      query = { target: { environmentId: target.environmentId, cwd: target.cwd, faviconPath: target.faviconPath || null },
        mounts: new Set(), owner: null, url: null, state: 'initial', error: '', successAt: null,
        phase: '', clonePhase: null, cloneObserved: false, evaluated: false,
        idleAt: now + MOBILE_FAVICON_IDLE_MS, refreshAt: 0, pending: false, serial: ++this.serial, active: 0, imageSerial: 0 };
      this.queries.set(key, query);
    }
    return query;
  }
  private adoptOwner(query: Query, owner: MobileFaviconOwner): void {
    if (sameOwner(query.owner, owner)) return;
    if (query.owner && (query.owner.catalogIdentity !== owner.catalogIdentity || query.owner.scopeRevision !== owner.scopeRevision)) {
      query.url = null; query.successAt = null; query.state = 'initial'; query.error = '';
      query.cloneObserved = false;
    }
    query.owner = copyOwner(owner); query.serial = ++this.serial; query.active = 0;
    query.pending = true; this.revision++;
  }
  private expire(now: number): void {
    for (const [key, query] of this.queries) if (!query.mounts.size && now >= query.idleAt) {
      this.queries.delete(key); this.revision++;
    }
  }
  /** Complete current demand set; stable mount IDs distinguish simultaneous
   * Home/sidebar/etc. Overrides and unusable roots must be filtered by caller. */
  reconcile(demands: readonly MobileFaviconDemand[], now: number): void {
    this.expire(now);
    const mounts = new Map<string, Set<string>>();
    for (const demand of demands) {
      if (!demand.environmentId || !demand.cwd || !demand.mountId) continue;
      const key = mobileFaviconResourceKey(demand), query = this.ensure(demand, now);
      const set = mounts.get(key) ?? new Set<string>(); set.add(demand.mountId); mounts.set(key, set);
      if (!query.mounts.size) {
        query.evaluated = true; query.idleAt = 0;
        if (!query.refreshAt) query.refreshAt = now + MOBILE_FAVICON_REFRESH_MS;
        if (query.state === 'initial' || query.state !== 'waiting' && !fresh(query, now)) query.pending = true;
      }
    }
    for (const [key, query] of this.queries) {
      const next = mounts.get(key) ?? new Set<string>();
      if (query.mounts.size && !next.size) {
        query.idleAt = now + MOBILE_FAVICON_IDLE_MS;
        // Departing answers cannot publish after their last UI owner left.
        query.serial = ++this.serial; query.active = 0;
        if (query.state === 'waiting' && query.phase === 'connected') {
          query.state = query.successAt === null ? 'initial' : 'success'; query.pending = true;
        }
      }
      query.mounts = next;
    }
  }
  /** Capture before the inherited real rpc. Its observer does not mount demand
   * or start timers for the inherited eager all-project scan. */
  beginObservation(target: MobileFaviconTarget, owner: MobileFaviconOwner, now: number): MobileFaviconObservation {
    const query = this.ensure(target, now); this.adoptOwner(query, owner);
    query.serial = ++this.serial; query.active = query.serial; query.state = 'waiting'; query.pending = false;
    this.revision++;
    return { key: mobileFaviconResourceKey(target), serial: query.serial, owner: copyOwner(owner) };
  }
  private observed(token: MobileFaviconObservation): Query | undefined {
    const query = this.queries.get(token.key);
    return query?.serial === token.serial && sameOwner(query.owner, token.owner) ? query : undefined;
  }
  /** current must recheck captured endpoint/catalog/clear ownership at receipt.
   * The real result and failure are returned/rethrown unchanged by the caller. */
  observeReply(token: MobileFaviconObservation, result: { ok: true; value: unknown } | { ok: false; error: unknown },
    now: number, current: () => boolean): boolean {
    const query = this.observed(token);
    if (!query || !current()) { this.abandonObservation(token); return false; }
    if (!result.ok && letGo(result.error)) { this.abandonObservation(token); return false; }
    try {
      if (!result.ok) throw result.error;
      const relativeUrl = obj(result.value).relativeUrl;
      if (typeof relativeUrl !== 'string' || !relativeUrl) throw new Error('The project icon URL is invalid.');
      query.url = new URL(relativeUrl, token.owner.origin).href;
      query.successAt = now; query.state = 'success'; query.error = '';
    } catch (error) { query.state = 'failure'; query.error = failure(error); }
    // Completion is a new image evaluation. A concurrent reader may already
    // have resolved the previous URL while this real query was waiting.
    query.serial = ++this.serial; query.active = 0; query.pending = false;
    if (query.evaluated) query.refreshAt = now + MOBILE_FAVICON_REFRESH_MS;
    this.revision++; return true;
  }
  abandonObservation(token: MobileFaviconObservation): void {
    const query = this.observed(token);
    if (!query) return;
    query.serial = ++this.serial; query.active = 0; query.state = query.successAt === null ? 'initial' : 'success'; query.pending = true;
    this.revision++;
  }
  /** Pure projection over current provenance; an authoritative missing URL wins
   * immediately even while native deletion is still awaiting its reply. */
  display(target: MobileFaviconTarget, context: Pick<MobileFaviconOwner, 'catalogIdentity' | 'scopeRevision'>,
    cache: MobileFaviconCache): string | null {
    if (!context.catalogIdentity) return null;
    const query = this.queries.get(mobileFaviconResourceKey(target));
    const matching = query?.owner?.catalogIdentity === context.catalogIdentity && query.owner.scopeRevision === context.scopeRevision;
    if (matching && query.url && mobileFaviconMissing(query.url)) return null;
    return cache.peek(target, { scopeRevision: () => context.scopeRevision }) ?? (matching ? query.url : null) ?? null;
  }
  private view(now: number, endpoints: readonly MobileFaviconEndpoint[], cache: MobileFaviconCache, active: boolean): MobileFaviconView {
    if (!active) return { revision: this.revision, nextDeadline: 0, items: [] };
    let nextDeadline = 0;
    const items: MobileFaviconView['items'] = [];
    for (const [key, query] of this.queries) {
      for (const deadline of [query.evaluated ? query.refreshAt : 0, query.mounts.size ? 0 : query.idleAt])
        if (deadline && (!nextDeadline || deadline < nextDeadline)) nextDeadline = deadline;
      const endpoint = endpoints.find(row => row.environmentId === query.target.environmentId);
      if (query.mounts.size) items.push({ key, url: endpoint?.current() ? this.display(query.target, endpoint, cache) ?? '' : '',
        state: query.state, error: query.error });
    }
    return { revision: this.revision, nextDeadline: nextDeadline ? Math.max(now, nextDeadline) : 0, items };
  }
  async prepare(input: {
    demands: readonly MobileFaviconDemand[]; endpoints: readonly MobileFaviconEndpoint[];
    now(): number; current(): boolean; cache: MobileFaviconCache; io: MobileFaviconIO;
    /** Explicit refresh signals, delivered once by their root event owner. */
    refresh?: readonly string[];
  }): Promise<MobileFaviconView> {
    const { endpoints, cache, io, current, now } = input;
    if (!current()) return this.view(now(), endpoints, cache, current());
    this.reconcile(input.demands, now());
    const hydrated = current() && await cache.hydrate(io, current);
    if (!current()) return this.view(now(), endpoints, cache, current());
    for (const [key, query] of this.queries) {
      if (!current()) break;
      const endpoint = endpoints.find(row => row.environmentId === query.target.environmentId);
      if (!endpoint?.catalogIdentity || !endpoint.current()) continue;
      const previousOwner = query.owner, previousPhase = query.phase;
      this.adoptOwner(query, endpoint);
      const phase = str(endpoint.clones.find(row => row.destinationPath === query.target.cwd)?.phase) || null;
      if (query.cloneObserved ? phase !== query.clonePhase : phase !== null) query.pending = true;
      query.clonePhase = phase; query.cloneObserved = true; query.phase = endpoint.phase;
      if (endpoint.phase !== 'connected') {
        if (previousPhase !== endpoint.phase || !previousOwner) {
          query.serial = ++this.serial; query.active = 0;
          query.state = endpoint.phase === 'waiting' ? 'waiting' : 'failure';
          query.error = endpoint.phase === 'waiting' ? '' : 'The environment is unavailable.';
          if (query.evaluated) query.refreshAt = now() + MOBILE_FAVICON_REFRESH_MS;
          this.revision++;
        } else if (query.refreshAt && now() >= query.refreshAt) query.refreshAt = now() + MOBILE_FAVICON_REFRESH_MS;
      } else {
        if (previousPhase && previousPhase !== 'connected') query.pending = true;
        const due = query.evaluated && query.refreshAt > 0 && now() >= query.refreshAt;
        const demanded = query.mounts.size > 0 && (query.pending || due || input.refresh?.includes(key));
        const retainedDeadline = !query.mounts.size && query.evaluated && now() < query.idleAt && due;
        if ((demanded || retainedDeadline) && !query.active) {
          const token = this.beginObservation(query.target, endpoint, now());
          if (query.evaluated) query.refreshAt = now() + MOBILE_FAVICON_REFRESH_MS;
          const owned = () => current() && endpoint.current() && this.observed(token) === query
            && io.scopeRevision?.(query.target.environmentId) === endpoint.scopeRevision;
          try {
            if (!owned()) { this.abandonObservation(token); continue; }
            const value = await endpoint.request({ _tag: 'project-favicon', cwd: query.target.cwd,
              ...(query.target.faviconPath ? { path: query.target.faviconPath } : {}) });
            this.observeReply(token, { ok: true, value }, now(), owned);
          } catch (error) {
            this.observeReply(token, { ok: false, error }, now(), owned);
            if (letGo(error)) throw error;
          }
        }
      }
      if (!query.evaluated || query.imageSerial === query.serial || !hydrated || !current()
        || !endpoint.current() || !sameOwner(query.owner, endpoint)) continue;
      const serial = query.serial, owner = copyOwner(endpoint), controller = new AbortController();
      const owned = () => current() && endpoint.current() && this.queries.get(key) === query && query.serial === serial
        && sameOwner(query.owner, owner) && io.scopeRevision?.(query.target.environmentId) === owner.scopeRevision;
      // A pending query retains the last resolved URL/image. Missing hides it
      // synchronously in display; resolve owns the durable key invalidation.
      await cache.resolve(io, query.target, query.url, { current: owned, signal: controller.signal });
      if (owned()) query.imageSerial = serial;
    }
    this.expire(now());
    return this.view(now(), endpoints, cache, current());
  }
}

export const mobileFaviconQueries = new MobileFaviconQueries();
