// One environment, several routes: the route walk and the better-route check (T3 Code 7c3b3b3605ef,
// MIT, see LICENSE-T3: packages/client-runtime/src/connection/driver.ts connectOverRoutes,
// preflight and checkRoute, supervisor.ts checkBetterRoutes). The app
// (connection-routes.ts) owns the route model and writes each environment's ordered routes to the
// catalog; one session walks them, beside the reconnect ladder.
//
// An SSH route's origin is its tunnel's loopback address, so its kind comes from the stored
// `kind`. A learned plain-HTTP LAN or tailnet address is checked and walked like any direct route and
// reads the token of the route it was learned over (`credential`); the token goes out only after the
// address answered as this environment, and never over plain HTTP it was not paired over
// (connection-routes.ts sendsTokenInClear: such a route is left out of the walk and the checks). Checks run in the answer that connects (it may fetch); the
// 60 s cadence is the transport's tick. The walk's shared head start is a second check of each direct route with a
// 500 ms deadline, which the host keeps: natively a timer raced against the answer's own fetch runs only once the fetch
// lands (LLP 1027.000 D1, Fetches), so a 500 ms sleep would wait out a dead route's whole check.
import { TransportFailure, letGoAbort, origin as parseOrigin, path, trimmed } from './transport-wire';
import type { Catalog, Entry } from './transport-catalog';
import { sendsCredentialInClear, type ConnectionRoute } from '../connections/connection-routes';

export interface HttpReply { status: number; text: string }
/** `maxBytes`: the reply's ceiling (default 1 MiB); each request may overlap the others (exact2's independent HTTP). */
/** `bodyFrom`: an app file the host sends as the body (`exactBodyFrom`), whose bytes never enter JavaScript. */
export interface HttpInit { method?: string; headers?: Record<string, string>; body?: string | Uint8Array; bodyFrom?: string; timeoutMs: number; signal?: AbortSignal; maxBytes?: number }
/** A GET or POST the routes and sessions make; resolves with any status, rejects only when nothing came back. */
export type Http = (url: string, init: HttpInit) => Promise<HttpReply>;
/** A reply's JSON body; undefined when it is not JSON. An empty body is an empty object. */
export function json(reply: HttpReply): unknown {
  if (!reply.text) return {};
  try { return JSON.parse(reply.text) as unknown; } catch { return undefined; }
}

export interface WalkRoute { id: string; origin: string; kind: string; learned: boolean; credential: string; authorization?: 't3-connect'; network?: 'tailscale' }
const str = (value: unknown) => typeof value === 'string' ? value : '';
/** Direct routes have a cheap unauthenticated check; SSH and T3 Connect do not. */
export const direct = (route: WalkRoute) => route.kind !== 'ssh' && route.kind !== 'relay';

export function walkRoute(raw: Record<string, unknown>): WalkRoute | null {
  const id = str(raw.id), routeOrigin = str(raw.origin);
  if (!id || !routeOrigin) return null;
  return { id, origin: routeOrigin, kind: str(raw.kind), learned: raw.learned === true, credential: str(raw.credential) || routeOrigin,
    ...(raw.authorization === 't3-connect' ? { authorization: 't3-connect' as const } : {}), ...(raw.network === 'tailscale' ? { network: 'tailscale' as const } : {}) };
}
const modelRoute = (route: WalkRoute): ConnectionRoute => ({ id: route.id, origin: route.origin, kind: route.kind, learned: route.learned,
  ...(route.authorization ? { authorization: route.authorization } : {}), ...(route.network ? { network: route.network } : {}) });
/** usableRoutes: the routes a connect may use; none sends a paired token over plain HTTP it wasn't paired over. */
export function usable(routes: readonly WalkRoute[]): WalkRoute[] {
  const model = routes.map(modelRoute);
  return routes.filter((route, index) => !sendsCredentialInClear(model, model[index]!, route.credential));
}
export function homeRoute(origin: string): WalkRoute {
  const text = trimmed(origin);
  return { id: text, origin: text, kind: '', learned: false, credential: text };
}

/** How long a direct route has to answer before it counts as unreachable from here. */
export const CHECK_TIMEOUT_MS = 2_500;
/**
 * The head start the walk's direct checks share (ROUTE_CHECK_HEAD_START_MS): a route silent by then waits for the
 * second pass, so a dead first route costs half a second, not the full check.
 */
export const CHECK_HEAD_START_MS = 500;
/** A better route that answered but then failed to connect is not tried again for this long. */
export const BETTER_ROUTE_COOLDOWN_MS = 300_000;
/** While connected over a fallback route, how often to look for a better one (BETTER_ROUTE_CHECK_INTERVAL). */
export const BETTER_ROUTE_CHECK_MS = 60_000;
/** Failures that say this route will not work as configured; any other failure may pass. */
export const blocks = (failure: TransportFailure) => ['Authentication', 'Credential', 'Keychain', 'Address', 'Limit'].includes(failure.kind);

type Check = 'answered' | 'silent' | 'unchecked';
/** A route's checks in one walk: the head start's, the full one's and, once known, the full one's outcome. */
type Probe = { head: Promise<Check>; full: Promise<Check>; outcome: Check | null };
const settledProbe = (outcome: Check): Probe => ({ head: Promise.resolve(outcome), full: Promise.resolve(outcome), outcome });

export class RouteState {
  environmentId = '';
  label = '';
  home = '';
  routes: WalkRoute[] = [];
  walking = false;
  private walkEpoch = 0;
  private order: WalkRoute[] = [];
  private probes: Probe[] = [];
  private cursor = 0;
  private silent: number[] = [];
  private silentSorted = false;
  private silentCursor = 0;
  current: WalkRoute | null = null;
  /** The credential owner whose token the session holds. */
  tokenOwner = '';
  private transient: TransportFailure | null = null;
  private blocked: TransportFailure | null = null;
  activeId = '';
  private preferred: string | null = null;
  private switchingTo: string | null = null;
  private cooldowns = new Map<string, number>();
  private checking = false;
  /** When the next better-route check is due (ms on the transport's clock); 0 when none is scheduled. */
  nextCheckAt = 0;

  constructor(private readonly http: Http) {}

  /** The saved environment one of whose routes is `origin`; an unsaved origin (a pairing) is its own one route. */
  load(origin: string, catalog: Catalog): void {
    const entry = catalog.entryForOrigin(origin);
    if (entry) this.adopt(entry);
    else { this.environmentId = ''; this.label = ''; this.home = trimmed(origin); this.routes = [homeRoute(origin)]; }
    this.activeId = ''; this.preferred = null; this.switchingTo = null; this.current = null;
    this.walking = false; this.walkEpoch++; this.nextCheckAt = 0;
  }
  /** Re-reads the saved routes (learned routes and reorders arrive while connected). */
  reload(catalog: Catalog): void {
    if (!this.environmentId) return;
    const entry = catalog.entryForEnvironment(this.environmentId);
    if (entry) this.adopt(entry);
  }
  private adopt(entry: Entry): void {
    this.environmentId = str(entry.environmentId);
    let host = '';
    try { host = new URL(str(entry.origin)).hostname; } catch { host = ''; }
    this.label = str(entry.label) || host || 'The environment';
    this.home = trimmed(str(entry.origin));
    const parsed = (Array.isArray(entry.routes) ? entry.routes : []).map(route => walkRoute(route as Record<string, unknown>)).filter((route): route is WalkRoute => route !== null);
    this.routes = parsed.length ? parsed : [homeRoute(this.home)];
  }
  get activeIndex(): number { return this.routes.findIndex(route => route.id === this.activeId); }

  // MARK: The walk

  /** Starts a walk: every direct route is checked at once, twice (the head start's check and the full one). */
  beginWalk(catalog: Catalog): void {
    this.reload(catalog);
    this.walkEpoch++; this.walking = true; this.cursor = 0; this.silent = []; this.silentSorted = false; this.silentCursor = 0;
    this.current = null; this.transient = null; this.blocked = null;
    this.order = usable(this.routes);
    const wanted = this.preferred;
    const at = wanted ? this.order.findIndex(route => route.id === wanted) : -1;
    if (at > 0) this.order.unshift(...this.order.splice(at, 1));
    this.switchingTo = this.preferred; this.preferred = null;
    if (this.order.length <= 1) { this.probes = [settledProbe('unchecked')]; return; }
    // A check rejects only when its answer was let go; `next` passes that on, and the checks it never reaches are handled here.
    this.probes = this.order.map(route => {
      if (!direct(route)) return settledProbe('unchecked');
      const probe: Probe = { head: this.check(route, CHECK_HEAD_START_MS), full: this.check(route), outcome: null };
      probe.head.catch(() => undefined);
      probe.full.then(outcome => { probe.outcome = outcome; }, () => undefined);
      return probe;
    });
  }

  /**
   * The next route worth trying, in order. A route counts by its own check when that is in, else by the head start's;
   * one silent by then goes to a second pass, where routes that answered late come before those still silent, since a
   * check is not proof. null when none is left.
   */
  async next(): Promise<WalkRoute | null> {
    const epoch = this.walkEpoch;
    while (this.cursor < this.order.length) {
      const index = this.cursor, probe = this.probes[index]!;
      const outcome = probe.outcome ?? await Promise.race([probe.full, probe.head]);
      if (epoch !== this.walkEpoch) return null;
      this.cursor = Math.max(this.cursor, index + 1);
      if (outcome === 'silent') { this.silent.push(index); continue; }
      return this.current = this.order[index]!;
    }
    if (!this.silentSorted) {
      const late = await Promise.all(this.silent.map(index => this.probes[index]!.full));
      if (epoch !== this.walkEpoch) return null;
      this.silentSorted = true;
      this.silent = [...this.silent.filter((_, at) => late[at] !== 'silent'), ...this.silent.filter((_, at) => late[at] === 'silent')];
    }
    if (this.silentCursor < this.silent.length) return this.current = this.order[this.silent[this.silentCursor++]!]!;
    return this.current = null;
  }

  /** A route failed: a transient error is reported over a blocked one. */
  record(failure: TransportFailure): void {
    if (blocks(failure)) { if (!this.blocked) this.blocked = failure; }
    else if (!this.transient) this.transient = failure;
  }
  /** Whether this walk has more than its one route, so a failure moves on instead of ending it. */
  get walksSeveral(): boolean { return this.walking && this.order.length > 1; }
  /** What the walk reports when no route connected. */
  exhausted(): TransportFailure {
    this.walking = false;
    return this.transient ?? this.blocked ?? new TransportFailure('Network', `${this.label || 'The environment'} did not answer on any saved route.`);
  }
  /** The socket opened over `current`. A better route that did not land cools down. */
  landed(origin: string, environment: string, catalog: Catalog, now: number): void {
    this.walking = false;
    // A pairing's environment is saved only now; adopt its routes.
    const entry = (!this.environmentId || this.environmentId !== environment) ? catalog.entryForEnvironment(environment) : null;
    if (entry) this.adopt(entry); else this.reload(catalog);
    const address = trimmed(origin);
    this.activeId = this.current?.id ?? this.routes.find(route => route.origin === address && !route.learned)?.id
      ?? this.routes.find(route => route.origin === address)?.id ?? address;
    if (this.switchingTo && this.switchingTo !== this.activeId) this.cooldowns.set(this.switchingTo, now + BETTER_ROUTE_COOLDOWN_MS);
    this.switchingTo = null;
  }
  stop(): void { this.walking = false; this.walkEpoch++; this.activeId = ''; this.nextCheckAt = 0; }

  /** While connected over a fallback route, a check is due every interval. */
  schedule(now: number, interval = BETTER_ROUTE_CHECK_MS): void { this.nextCheckAt = this.activeIndex > 0 ? now + interval : 0; }

  // MARK: Checks

  /** checkRoute: the public descriptor answers as this environment within `timeoutMs`; no credential is sent. */
  async check(route: WalkRoute, timeoutMs = CHECK_TIMEOUT_MS): Promise<Check> {
    if (!direct(route)) return 'unchecked';
    let url: string;
    try { url = path('/.well-known/t3/environment', parseOrigin(route.origin)); } catch { return 'silent'; }
    const expected = this.environmentId;
    try {
      const reply = await this.http(url, { headers: { Accept: 'application/json' }, timeoutMs });
      const body = json(reply) as Record<string, unknown> | null | undefined;
      const ok = reply.status >= 200 && reply.status < 300 && typeof body?.environmentId === 'string' && (!expected || body.environmentId === expected);
      return ok ? 'answered' : 'silent';
    } catch (error) { if (letGoAbort(error)) throw error; return 'silent'; }
  }

  /** preflight: the route answers as this environment and accepts this client's token, without a socket. */
  async preflight(route: WalkRoute, catalog: Catalog): Promise<boolean> {
    if (await this.check(route) !== 'answered') return false;
    let url: string;
    try { url = path('/api/auth/session', parseOrigin(route.origin)); } catch { return false; }
    const token = catalog.token(route.credential, this.environmentId);
    if (!token) return false;
    try {
      const reply = await this.http(url, { headers: { Accept: 'application/json', Authorization: `Bearer ${token}` }, timeoutMs: CHECK_TIMEOUT_MS * 2 });
      return reply.status >= 200 && reply.status < 300 && (json(reply) as Record<string, unknown> | null | undefined)?.authenticated === true;
    } catch (error) { if (letGoAbort(error)) throw error; return false; }
  }

  /**
   * checkBetterRoutes: preflight the routes ranked above the one in use (not cooling down, direct only) and name the
   * best that would connect. One check at a time; a trigger during a check is dropped.
   */
  async checkBetter(catalog: Catalog, now: number): Promise<string | null> {
    this.reload(catalog);
    const current = this.activeIndex;
    if (this.checking || current <= 0) return null;
    const allowed = new Set(usable(this.routes));
    const better = this.routes.slice(0, current).filter(route => direct(route) && allowed.has(route) && (this.cooldowns.get(route.id) ?? 0) <= now);
    if (!better.length) return null;
    this.checking = true;
    const active = this.activeId;
    try {
      const passed = await Promise.all(better.map(route => this.preflight(route, catalog)));
      // The check may outlive the session it was asked about.
      const best = passed.indexOf(true);
      if (this.activeId !== active || best < 0) return null;
      this.preferred = better[best]!.id;
      return this.preferred;
    } finally { this.checking = false; }
  }
}
