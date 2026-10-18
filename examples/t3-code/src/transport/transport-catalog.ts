// The connection catalog: environments this client has paired with, their bearer tokens, the
// Connections preferences beside them, and the focus a launch restores. It is one secret
// (`secret.keep t3-catalog`, the Keychain on Apple), as the reference keeps one catalog
// document (T3 Code 1e2ecbd975 connection/catalog.ts: Electron safeStorage on the desktop,
// DesktopConnectionCatalogStore.ts; expo-secure-store's `t3code.connection-catalog.v1` on mobile).
//
// An environment is one entry per environment id, in the order it was added (rows never jump
// when one reconnects): its identity, display facts, whether this device keeps it switched on,
// and its ordered routes (catalog.ts alternateRoutes). A route is `{ id, origin, kind, learned,
// credential, authorization?, network?, ssh? }`; `credential` names the origin whose token the route
// uses, so a learned route borrows its source route's token; `network: "tailscale"` marks an
// address the server found on its Tailscale interface. The entry's `origin` is its home:
// the first address it was saved under, kept while that route exists.
import { TransportFailure, trimmed } from './transport-wire';

export interface SecretStore { get(name: string): string | null; set(name: string, value: string): void; forget(name: string): void }
export type Entry = Record<string, unknown>;
export type Route = Record<string, unknown>;

export const CATALOG_SECRET = 't3-catalog';
const str = (value: unknown) => typeof value === 'string' ? value : '';
const list = (value: unknown): Entry[] => Array.isArray(value) ? value.filter((item): item is Entry => typeof item === 'object' && item !== null && !Array.isArray(item)) : [];

interface Document {
  version: 1;
  saved: Entry[];
  tokens: Record<string, string>;
  preferences: string;
  /** The saved environment the focused connection last opened (its home), and `primary` when that was this machine. */
  origin: string;
  focus: string;
  /** backgroundActivityReporter.ts's client id (the reference keeps it in localStorage). */
  activityClientId: string;
}
const empty = (): Document => ({ version: 1, saved: [], tokens: {}, preferences: '{}', origin: '', focus: '', activityClientId: '' });

/** A paired route: its id and its credential are its own origin. */
export function pairedRoute(origin: string): Route {
  const text = trimmed(origin);
  return { id: text, origin: text, kind: '', learned: false, credential: text };
}

/** One entry per environment id, each with at least one route; duplicates join the first one's routes. */
export function migrated(raw: Entry[]): Entry[] {
  const out: Entry[] = [], index = new Map<string, number>();
  for (const original of raw) {
    const entry = { ...original };
    const environment = str(entry.environmentId);
    let routes = list(entry.routes).filter(route => str(route.id));
    if (!routes.length && str(entry.origin)) routes = [pairedRoute(str(entry.origin))];
    const at = environment ? index.get(environment) : undefined;
    if (at !== undefined) {
      const merged = list(out[at]!.routes);
      for (const route of routes) if (!merged.some(other => other.id === route.id)) merged.push(route);
      out[at]!.routes = merged;
      continue;
    }
    entry.routes = routes;
    if (environment) index.set(environment, out.length);
    out.push(entry);
  }
  return out;
}

function hasOrigin(entry: Entry, origin: string): boolean {
  const wanted = trimmed(origin);
  return trimmed(str(entry.origin)) === wanted || list(entry.routes).some(route => trimmed(str(route.origin)) === wanted);
}

/** The origins whose tokens an entry's routes own (learned routes borrow, never own). */
export function credentialOrigins(entry: Entry | null | undefined): string[] {
  if (!entry) return [];
  const origins = [trimmed(str(entry.origin))];
  for (const route of list(entry.routes)) if (route.learned !== true) origins.push(trimmed(str(route.credential) || str(route.origin)));
  return [...new Set(origins.filter(Boolean))];
}

export const account = (origin: string, environment: string) => `${origin}\n${environment}`;

export class Catalog {
  private document: Document | null = null;
  constructor(private readonly store: () => SecretStore | null) {}

  private read(): Document {
    if (this.document) return this.document;
    let raw: string | null = null;
    try { raw = this.store()?.get(CATALOG_SECRET) ?? null; } catch { raw = null; }
    let parsed: Partial<Document> = {};
    try { parsed = raw ? JSON.parse(raw) as Partial<Document> : {}; } catch { parsed = {}; }
    const document = empty();
    document.saved = migrated(list(parsed.saved));
    if (parsed.tokens && typeof parsed.tokens === 'object') for (const [key, value] of Object.entries(parsed.tokens)) if (typeof value === 'string') document.tokens[key] = value;
    document.preferences = str(parsed.preferences) || '{}';
    document.origin = str(parsed.origin); document.focus = str(parsed.focus);
    document.activityClientId = str(parsed.activityClientId);
    // Only a document read from the store is kept: before the module has its store, nothing is cached.
    if (this.store()) this.document = document;
    return document;
  }
  private write(change: (document: Document) => void): void {
    const document = this.read();
    change(document);
    const store = this.store();
    if (!store) return;
    try { store.set(CATALOG_SECRET, JSON.stringify(document)); }
    catch { throw new TransportFailure('Keychain', 'The server credential could not be saved in Keychain.'); }
  }

  get all(): Entry[] { return this.read().saved.map(entry => ({ ...entry, routes: list(entry.routes).map(route => ({ ...route })) })); }
  entryForEnvironment(environment: string): Entry | null { return this.all.find(entry => entry.environmentId === environment) ?? null; }
  /** The environment one of whose routes (or whose home) is this origin. */
  entryForOrigin(origin: string): Entry | null {
    const all = this.all;
    return all.find(entry => trimmed(str(entry.origin)) === trimmed(origin)) ?? all.find(entry => hasOrigin(entry, origin)) ?? null;
  }

  /**
   * Refreshes an entry's display facts in place, or appends a new one switched on. A reconnect never reorders the
   * list or switches it back on. An address this environment was not saved under joins its routes, last.
   */
  remember(origin: string, descriptor: Record<string, unknown>, pairedAt: string): void {
    const environment = str(descriptor.environmentId);
    if (!environment) return;
    const platform = (descriptor.platform && typeof descriptor.platform === 'object' ? descriptor.platform : {}) as Record<string, unknown>;
    this.write(document => {
      const entry: Entry = { origin: trimmed(origin), environmentId: environment, label: str(descriptor.label), machine: str(platform.machine), os: str(platform.os),
        serverVersion: str(descriptor.serverVersion), enabled: true, pairedAt, routes: [pairedRoute(origin)] };
      const index = document.saved.findIndex(other => other.environmentId === environment);
      if (index >= 0) {
        const previous = document.saved[index]!;
        entry.origin = previous.origin ?? entry.origin;
        entry.enabled = typeof previous.enabled === 'boolean' ? previous.enabled : true;
        entry.pairedAt = previous.pairedAt ?? entry.pairedAt;
        const routes = list(previous.routes);
        if (!hasOrigin(previous, origin)) routes.push(pairedRoute(origin));
        entry.routes = routes;
        document.saved[index] = entry;
      } else document.saved.push(entry);
      document.saved = document.saved.slice(-32);
    });
  }

  /** The app's route edits; returns the routes that left the list, so their tokens can be forgotten. */
  setRoutes(environment: string, routes: Route[]): Route[] {
    if (!routes.length || routes.length > 32) throw new TransportFailure('Arguments', 'A saved environment needs one to 32 routes.');
    const ids = new Set<string>(), cleaned: Route[] = [];
    for (const route of routes) {
      const id = str(route.id), routeOrigin = str(route.origin), owner = str(route.credential);
      let scheme = '', host = '';
      try { const url = new URL(routeOrigin); scheme = url.protocol; host = url.hostname; } catch { /* refused below */ }
      if (!id || id.length > 2048 || ids.has(id) || !['http:', 'https:'].includes(scheme) || !host || !owner) {
        throw new TransportFailure('Arguments', 'Each route needs a unique id, an http(s) origin and a credential owner.');
      }
      ids.add(id);
      const kind = str(route.kind);
      if (kind !== '' && kind !== 'ssh') throw new TransportFailure('Arguments', 'Unknown route kind.');
      const stored: Route = { id, origin: trimmed(routeOrigin), kind, learned: route.learned === true, credential: trimmed(owner) };
      if (route.ssh && typeof route.ssh === 'object') stored.ssh = route.ssh;
      if (route.authorization === 't3-connect') stored.authorization = 't3-connect';
      if (route.network === 'tailscale') stored.network = 'tailscale';
      cleaned.push(stored);
    }
    let removed: Route[] = [];
    const document = this.read();
    const index = document.saved.findIndex(entry => entry.environmentId === environment);
    if (index < 0) throw new TransportFailure('Missing', 'That environment is no longer saved on this device.');
    this.write(doc => {
      const entry = doc.saved[index]!;
      removed = list(entry.routes).filter(old => !ids.has(str(old.id)));
      entry.routes = cleaned;
      // The home stays while its route does; otherwise the preferred route's address becomes the home.
      if (!cleaned.some(route => route.origin === trimmed(str(entry.origin)) && route.learned !== true)) {
        entry.origin = (cleaned.find(route => route.learned !== true) ?? cleaned[0]!).origin;
      }
    });
    return removed;
  }

  /** The Environments switch: whether this device keeps a connection open. */
  setEnabled(environment: string, enabled: boolean): boolean {
    const index = this.read().saved.findIndex(entry => entry.environmentId === environment);
    if (index < 0) return false;
    this.write(document => { document.saved[index]!.enabled = enabled; });
    return true;
  }

  /** Forgets the environment with every route; returns the forgotten entry. */
  forget(environment: string): Entry | null {
    const gone = this.read().saved.find(entry => entry.environmentId === environment) ?? null;
    this.write(document => { document.saved = document.saved.filter(entry => entry.environmentId !== environment); });
    return gone;
  }

  get preferences(): string { return this.read().preferences; }
  setPreferences(text: string): void {
    let parsed: unknown = null;
    try { parsed = JSON.parse(text); } catch { parsed = null; }
    if (new TextEncoder().encode(text).length > 65_536 || typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
      throw new TransportFailure('Arguments', 'Connection preferences must be a JSON object up to 64 KB.');
    }
    this.write(document => { document.preferences = text; });
  }

  // Tokens, by the origin that owns them and the environment.
  token(origin: string, environment: string): string { return this.read().tokens[account(origin, environment)] ?? ''; }
  saveToken(token: string, origin: string, environment: string): void { this.write(document => { document.tokens[account(origin, environment)] = token; }); }
  forgetToken(origin: string, environment: string): void {
    if (!(account(origin, environment) in this.read().tokens)) return;
    this.write(document => { delete document.tokens[account(origin, environment)]; });
  }

  get origin(): string { return this.read().origin; }
  get focus(): string { return this.read().focus; }
  rememberOrigin(origin: string): void { if (this.read().origin !== origin) this.write(document => { document.origin = origin; }); }
  rememberFocus(focus: string): void { if (this.read().focus !== focus) this.write(document => { document.focus = focus; }); }

  activityClientId(make: () => string): string {
    const known = this.read().activityClientId;
    if (known) return known;
    const made = make();
    try { this.write(document => { document.activityClientId = made; }); } catch { /* an unsaved id lasts this launch */ }
    return made;
  }
}
