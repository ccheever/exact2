export { pick } from './projection.ts';
import { ReleaseTracker, type ReleaseSnapshot } from './release.ts';
export type { ReleaseSnapshot } from './release.ts';
// Snapback4 for an Exact app's TypeScript: the client in Rust
// (`exact-snapback4-client`), driven here over `fetch`. Natively it is the
// app's native module (`exact_snapback4::Module` behind `native.call`); on
// the web it is the same Rust as wasm, persisted through Exact SQLite
// (`./web.ts`). Mount this directory in app.json:
//   "typescript": { "sources": { "snapback4": "<the exact2 checkout>/snapback4/ts" } }
// and `import { Snapback } from './snapback4/snapback.ts'`. README.md has
// a complete app.
//
// Data modules have no clock: pass `now` (milliseconds) from a source argument.

import { webDevice, type SqliteStorage } from './web.ts';
export type { SqliteStorage } from './web.ts';

// Published operation declarations reach auth's Pick<Storage, ...>. This is
// a structural type only; Exact provides no localStorage runtime global.
declare global {
  interface Storage {
    readonly length: number;
    clear(): void;
    getItem(key: string): string | null;
    key(index: number): string | null;
    removeItem(key: string): void;
    setItem(key: string, value: string): void;
  }
}

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Request = { [key: string]: Json };
/** A query's or a write's arguments: any JSON object (`undefined` fields are left out). */
export type Args = Record<string, unknown>;

export interface Refusal { code: string; family?: string; message: string; retryable?: boolean; [key: string]: Json | undefined }

/** A named query's answer, as the device reads it. Rows a pending write
 * predicted carry `pending: true`. */
export interface Read<T = Json> {
  data?: T;
  /** The server answered: the query reads an `online only` table or view,
   * or the device could not vouch for its answer (`loading`, `speculative`). */
  server?: boolean;
  /** The device could not vouch for this answer and the server was not
   * reached: show it as unavailable, not as data. */
  offline?: boolean;
  complete?: boolean;
  next?: string | null;
  loading?: boolean;
  denied?: Refusal;
  [key: string]: unknown;
}

/** Narrow a device Read without treating a refusal or an unavailable page as data. */
export function hasData<T>(read: Read<T>): read is Read<T> & { data: T } {
  return !read.denied && Object.hasOwn(read, 'data') && read.data !== undefined;
}

/** Every row of a query (`readAll`), and whether that is all of them:
 * `complete` false when a page was not the whole answer; `offline` true when
 * a page was the device's partial with the server unreached. */
export type Rows<T> = T[] & { complete: boolean; offline: boolean };

/** A write as admitted: kept and predicted on this device, sent by a round;
 * or refused by the device itself (its prediction refused it, as the server
 * would on the same rows), which is final: never sent, and `outcome(id)` and
 * `refusals()` answer it as they answer a refusal the server gave. */
export type Write =
  | { id: string; state: 'pending'; newIds: string[] }
  | { id: string; state: 'failed'; why: Refusal };

/** A refused write (by the server or by the device), kept in the partition
 * until dismissed: what
 * it was (`op`, `args`, so its input is not lost), why, and when written.
 * A refusal too large to store (over 8 MiB once encoded) keeps all but its
 * `args`, which are then null and `argsOmitted` is true. */
export interface Refused { id: string; op: string; args: Json; argsOmitted?: boolean; why: Refusal; at: number }

/** What became of a write. `result` is the server's, while remembered. */
export type Outcome<R = Json> =
  | { id: string; state: 'pending'; seq?: number; result?: never; why?: never }
  | { id: string; state: 'sent'; seq?: number; result?: R; why?: never }
  | { id: string; state: 'failed'; seq?: number; result?: never; why: Refusal }
  | { id: string; state: 'unknown'; seq?: number; result?: never; why?: never };

/** What `send` came to: the write's fate after its round, and the round
 * (absent when the device refused the write, which then never went out).
 * `pending` only when that round did not reach the server or was refused
 * (`round.offline`, `round.retry`, `round.denied`): the write is kept and
 * the next round sends it. */
export type Sent<R = Json> = Outcome<R> & { round?: Round };

/** An operation handle Snapback generates (`api.recent` in the backend's
 * `snapback/generated/api.ts`, a `Query` or `Mutation`): its name, and, as
 * types only, its arguments and result. Passed where a name goes, it types
 * the call: its arguments are checked (an unknown or missing field fails
 * the build, named) and its result is the server's type, so no hand-written
 * row type or `<T>` is needed. README.md, "Typed operations". */
export interface Op<A = Args, R = Json> { readonly kind: string; readonly name: string; readonly __args?: A; readonly __result?: R }
export type QueryOp<A = Args, R = Json> = Op<A, R> & { readonly kind: 'query' };
export type MutationOp<A = Args, R = Json> = Op<A, R> & { readonly kind: 'mutation' };
/** What a call by `O` takes: a handle's arguments, or any object for a name. */
export type ArgsOf<O> = O extends Op<infer A, unknown> ? A : Args;
/** What a call by `O` answers: a handle's result, or `T` for a name. */
export type ResultOf<O, T = Json> = O extends Op<unknown, infer R> ? R : T;
const nameOf = (op: string | Op<unknown, unknown>): string => typeof op === 'string' ? op : op.name;

/** A session as Snapback mints it; `expiresAt` in milliseconds. */
export interface Session { principal: string; kind: string; token: string; expiresAt: number }
/** Exact's app-scoped secret store, under `secret.keep <key>`. */
export interface SessionStore { get(key: string): string | null; set(key: string, value: string): void; forget(key: string): void }
export interface AuthOptions { store: SessionStore; key: string; now: number }
export type SignedIn = { ok: true; session: Session } | { ok: false; why: Refusal };

function sessionFrom(raw: string | null, now: number): Session | null {
  if (!raw) return null;
  try {
    const s = JSON.parse(raw) as Session;
    return s && typeof s.principal === 'string' && s.principal !== '' && typeof s.kind === 'string'
      && typeof s.token === 'string' && s.token !== '' && Number.isFinite(s.expiresAt) && s.expiresAt > now ? s : null;
  } catch { return null; }
}

/** What a refresh came to: the replacement session, or why not. */
export type Refreshed = { ok: true; session: Session } | { ok: false; offline?: boolean; denied: Refusal };

/** A round's end: caught up, or why not. `offline`: the server was not
 * reached; `retry`: it asked to be asked again; `busy`: a round is already
 * running; `stale`: this round was replaced (the client was reopened). */
export interface Round { ok: boolean; offline?: boolean; retry?: boolean; busy?: boolean; stale?: boolean; denied?: Refusal }

export interface Status {
  open: boolean;
  online: boolean | null;
  syncing: boolean;
  denied: Refusal | null;
  /** Increments whenever rows or the outbox may have changed: read again. */
  revision: number;
  watermark?: number;
  acquired?: boolean;
  queued?: { id: string; op: string; args?: Json }[];
}

/** The device: one JSON call in, `{ok}` or `{denied}` out; a host failure throws. */
export interface Device { call(request: Request): Promise<Request>; close?(): Promise<boolean | void>; healthy?(): boolean }

/** What the server established about an uploaded file (`POST /assets`):
 * pass `{ id }` as an `image` or `video` argument. */
export interface Asset { id: string; type: string; bytes: number; name?: string; width?: number | null; height?: number | null; duration?: number | null }
export interface UploadAndPlaceOptions extends UploadOptions { argument: string; id?: string }
export interface UploadOptions { file?: { type: string; name: string } }
/** `upload`'s answer: the asset, or why not (`E_ASSET_TYPE`,
 * `E_ASSET_TOO_LARGE`, `E_ASSET_BOUNDS`, `E_RATE_LIMIT`, `E_OFFLINE`, …). */
export type Uploaded = { ok: true; asset: Asset } | { ok: false; why: Refusal };
/** `download`'s answer: the app file holding the bytes (an `image` or
 * `video` source), or why not (`E_ASSET_UNKNOWN`: missing, or hidden by its
 * row's read rule; `E_ASSET_VIEWED`; `E_OFFLINE`). */
export type Downloaded = { ok: true; path: string; type: string; bytes: number; name?: string } | { ok: false; why: Refusal };

const EXTENSIONS: Record<string, string> = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'video/mp4': 'mp4', 'text/csv': 'csv', 'text/tab-separated-values': 'tsv', 'text/plain': 'txt', 'text/markdown': 'md', 'text/html': 'html', 'application/json': 'json', 'application/xml': 'xml', 'application/yaml': 'yaml' };

/** A fetch that rejected: unreached is `E_OFFLINE`; Exact's own refusal
 * before sending (`exactBodyFrom` naming a missing or ungranted file) is
 * `E_CLIENT`. */
function unreached(error: unknown): Refusal {
  const kind = (error as { kind?: string } | null)?.kind, message = error instanceof Error ? error.message : String(error);
  return kind === 'Refused' || kind === 'Unsupported'
    ? { code: 'E_CLIENT', family: 'client', message }
    : { code: 'E_OFFLINE', family: 'transport', retryable: true, message: `the server was not reached: ${message}` };
}

/** Exact's native module, when the app links `exact_snapback4::Module`. */
export interface NativeModule {
  call(request: Request): unknown;
  /** False at bake (and on a host that configured no module): the device's
   * fact, not the build's. */
  readonly available?: boolean;
}

export interface Options {
  /** The app's id: the web partition's identity (natively, the baked one). */
  app: string;
  /** The partition's file: `app:/data/<name>.sqlite`. One per origin and viewer. */
  name: string;
  /** The Snapback origin, e.g. `https://api.example.com`. Grant `net.fetch` for it. */
  origin: string;
  /** The principal this device reads and writes as (`dev:alice`, a session's principal). */
  viewer?: string;
  /** Password identity: no persona headers or viewer needed. The session is
   * kept under `key`; `now` comes from Contract, never Date.now(). */
  auth?: AuthOptions;
  /** Credentials for each request: `{authorization: 'Bearer …'}`, or in development `{'x-snapback-persona': 'alice'}`. */
  headers?: () => Record<string, string | undefined>;
  /** The dispatcher's `storage`, Exact's own (the web persists the partition
   * there); its `sqlite`, and its `fs` for `download`. */
  storage: { readonly sqlite: { open(path: string): Promise<unknown> }; readonly fs?: { readFile?(path: string): Promise<ArrayBuffer | Uint8Array>; writeFile(path: string, data: Uint8Array): Promise<void> } };
  /** The dispatcher's `native`; absent on the web. */
  native?: NativeModule | null;
  /** Choose the web device when the dispatcher supplies an unrelated native
   * module (for example a camera hatch). Otherwise `native` selects it. */
  device?: 'web' | 'native';
  /** Where the web's wasm is served (default `/assets/snapback4.wasm`). */
  wasm?: string;
}

/** A typed domain failure: the thrown error carries the refusal code as
 * `reason`, so Contract's `failure(resourceOrMutation).code` preserves it
 * (`SLOT_FULL`, `NOT_MANAGER`, `E_OFFLINE`), as does `.reason`.
 * Infrastructure errors without a domain reason use Exact's six classes.
 * `message` is `CODE: message`; `refusal` retains the complete envelope. */
export function refused(why: Refusal): Error {
  const error = Object.assign(new Error(`${why.code}: ${why.message}`), { refusal: why, code: why.code, reason: why.code });
  // What `failure(x)` reads as `offline` on every host (host/web-js/shape.js
  // and js/src/prelude.js `failureCode`): a fetch's `Network` rejection.
  // Defined, not assigned: natively Error.prototype's `name` is read-only
  // (frozen intrinsics), and assigning it throws.
  if (why.code === 'E_OFFLINE') Object.defineProperties(error, { name: { value: 'FetchError', configurable: true }, kind: { value: 'Network', configurable: true, enumerable: true } });
  return error;
}

function ok<T>(answer: Request): T {
  const denied = answer.denied as Refusal | undefined;
  if (denied) throw refused(denied);
  return answer.ok as T;
}

/** Milliseconds since the epoch, as the device takes them: whole. */
function clock(now: number): number {
  if (!Number.isFinite(now) || now < 0) throw new Error(`Snapback4: now must be milliseconds since the epoch, not ${now}`);
  return Math.floor(now);
}

function nativeDevice(native: NativeModule): Device {
  return { async call(request) { return native.call(request) as Request; } };
}

/** A partition as this page holds it: one device per file, whatever number
 * of clients opened it. Exact's storage locks an open database's file, so a
 * second device on the same file would be refused as busy; every client of
 * the partition shares this one instead, and its rounds run one at a time. */
interface Partition {
  page: Page;
  path: string;
  origin: string;
  viewer: string;
  device: Device;
  /** It has synced at least once (or was kept from an earlier run). */
  opened: boolean;
  /** Clients that have it open; the device closes with the last. */
  clients: number;
  /** The rounds, one after another: a `sync()` while one runs waits its turn. */
  rounds: Promise<unknown>;
  /** The exchange of a round that stopped and could not be cancelled then. */
  abandoned?: string;
  refreshing?: Promise<Refreshed>;
  /** The change poll in flight: a second `poll()` shares it (a second poll
   * would supersede the first, whose reply the client then refuses). */
  polling?: Promise<boolean>;
  /** Which queries the server answers, by name; forgotten after each round
   * (a round can adopt a new backend). */
  routes: Map<string, 'device' | 'server' | 'held'>;
  /** Its entry in `partitions`, while it is there. */
  entry?: Promise<Partition>;
  /** The last client closed it: a new `open` makes another. */
  closing: boolean;
  cleaned: boolean;
}

/** The partitions a page has open, by path, and those still closing (a
 * reopen waits for its file to be let go), kept per storage (natively, per
 * module): the lock is the storage's. Awaiting another answer's promise here
 * is Exact's own pattern for one open at a time (`withBooks` in
 * docs/contract-for-humans.md). */
interface Page { partitions: Map<string, Promise<Partition>>; closing: Map<string, Promise<unknown>>; authJobs: Map<string, Promise<unknown>> }
const pages = new WeakMap<object, Page>();
// A native executor owns one module for this app; every answer gets a new facade.
const nativePages = new Map<string, Page>();
function pageOf(options: Options): Page {
  const owner = options.storage as object;
  let page = options.native ? nativePages.get(options.app) : pages.get(owner);
  if (!page) {
    page = { partitions: new Map(), closing: new Map(), authJobs: new Map() };
    if (options.native) nativePages.set(options.app, page);
    else pages.set(owner, page);
  }
  return page;
}

function refusal(code: string, message: string, extra: Partial<Refusal> = {}): Error {
  return refused({ code, message, ...extra } as Refusal);
}

async function openPartition(options: Options & { viewer: string }, path: string, page: Page): Promise<Partition> {
  // At bake there is no module (and no storage): refused as storage is then,
  // `kind: 'Unavailable'`, `code: 'bake'`, so the bake leaves the answer to
  // the device and an app needs one check for it on every executor.
  if (options.native && options.native.available === false) {
    throw Object.assign(new Error('Snapback4: the native module is not available: at bake the device answers when the app runs; at run time, link exact_snapback4::Module (README)'), { kind: 'Unavailable', code: 'bake' });
  }
  await page.closing.get(path)?.catch(() => undefined);
  const device = options.native
    ? nativeDevice(options.native)
    : await webDevice(options.storage as SqliteStorage, options.app, path, options.wasm ?? '/assets/snapback4.wasm');
  try {
    const opened = ok<{ opened: boolean }>(await device.call({ op: 'open', path, origin: options.origin, viewer: options.viewer }));
    return { page, path, origin: options.origin, viewer: options.viewer, device, opened: opened.opened, clients: 0, rounds: Promise.resolve(), routes: new Map(), closing: false, cleaned: false };
  } catch (error) { await device.close?.(); throw error; }
}

async function retirePartition(partition: Partition): Promise<void> {
  partition.closing = true;
  const { partitions, closing } = partition.page;
  if (partition.entry && partitions.get(partition.path) === partition.entry) partitions.delete(partition.path);
  const cleanup = (async () => {
    await partition.device.call({ op: 'close' }).catch(() => undefined);
    // A cleanup the device deferred (it will run it) or that failed can be
    // asked for again.
    partition.cleaned = (await partition.device.close?.()) !== false;
  })();
  closing.set(partition.path, cleanup);
  try { await cleanup; } finally { if (closing.get(partition.path) === cleanup) closing.delete(partition.path); }
}

export class Snapback {
  private closed = false;
  private releases = new ReleaseTracker();
  /** Observed generation changes, from release() or ordinary traffic; unsubscribe on teardown. */
  subscribeGeneration(listener: (generation: number) => void): () => void { return this.releases.subscribe(listener); }
  /** Authenticated served catalog. Offline/malformed/stale catalogs keep every available() gate closed. */
  async release(): Promise<ReleaseSnapshot> {
    const check = this.fence(), headers = this.headers(), key = this.releases.identity(headers);
    let response: Response;
    try { response = await fetch(this.options.origin.replace(/\/+$/, '') + '/', {headers}); }
    catch { check(); return {loading: true}; }
    const value = await response.json().catch(() => null);
    check();
    this.releases.observe(response, key);
    return this.releases.decode(response.ok || value?.denied ? value : null, key);
  }

  private constructor(private held: Partition | undefined, readonly options: Options, private baseName = options.name) {}
  private get partition(): Partition {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    if (!this.held || (this.options.auth && this.session?.principal !== this.held.viewer))
      throw refusal('E_AUTH', 'sign in before reading or writing this partition');
    if (this.held.closing) throw refusal('E_STALE', 'this account partition was retired; open the current account again');
    return this.held;
  }
  get session(): Session | null {
    const auth = this.options.auth;
    return auth ? sessionFrom(auth.store.get(auth.key), auth.now) : null;
  }
  private headers(): Record<string, string> {
    const headers: Record<string, string> = {};
    for (const [key, value] of Object.entries(this.options.headers?.() ?? {})) if (value !== undefined) headers[key] = value;
    if (this.options.auth) {
      delete headers['x-snapback-persona'];
      delete headers.authorization;
      const session = this.session;
      if (session) headers.authorization = `Bearer ${session.token}`;
    }
    return headers;
  }
  private authWork<T>(work: () => Promise<T>): Promise<T> {
    const jobs = pageOf(this.options).authJobs;
    const key = `${this.options.app}:${this.options.origin}:${this.options.auth?.key}`;
    const next = (jobs.get(key) ?? Promise.resolve()).catch(() => undefined).then(work);
    jobs.set(key, next);
    const clear = () => { if (jobs.get(key) === next) jobs.delete(key); };
    next.then(clear, clear);
    return next;
  }

  private invalidate(token: string | undefined): void {
    const auth = this.options.auth;
    if (auth && token && this.session?.token === token) auth.store.forget(auth.key);
  }

  /** A lossless filename for any principal, including `user:<ULID>`. */
  static partitionName(principal: string): string {
    return `p-${Array.from(new TextEncoder().encode(principal), byte => byte.toString(16).padStart(2, '0')).join('')}`;
  }

  /** Every call goes through here: a closed client refuses, rather than
   * reaching whatever partition its device holds now. */
  private call(request: Request): Promise<Request> {
    if (this.closed) return Promise.reject(new Error('Snapback4: this client is closed'));
    const check = this.fence(), partition = this.partition;
    return partition.device.call(request).then(answer => { check(); return answer; });
  }

  private fence(): () => void {
    const partition = this.partition;
    return () => {
      if (this.closed || partition.closing || this.held !== partition || (this.options.auth && this.session?.principal !== partition.viewer))
        throw refusal('E_STALE', 'the account changed while this request was in flight');
    };
  }

  /** Open the partition this device keeps. Offline is fine once it has
   * synced; a partition that never has opens in its first round, which the
   * first `read` or `write` starts if the app has not called `sync()`.
   *
   * Opening a partition this page already has open shares it: every source
   * may call `open` for itself. A partition belongs to one viewer, so a
   * second viewer needs its own `name` (for example `inbox-${persona}`). */
  static async open(options: Options): Promise<Snapback> {
    if (options.device === 'web') options = { ...options, native: null };
    if (options.device === 'native' && !options.native) throw new Error('Snapback4: device native needs the linked Snapback module');
    if (options.auth) {
      const auth = options.auth, now = clock(auth.now);
      const raw = auth.store.get(auth.key), session = sessionFrom(raw, now);
      if (raw && !session) auth.store.forget(auth.key);
      const db = new Snapback(undefined, { ...options });
      if (session) {
        if (session.expiresAt - now <= 60_000) {
          const renewed = await db.refreshSession(now);
          if (!renewed.ok && !renewed.offline) return db;
        }
        if (db.session) await db.bindSession();
      }
      return db;
    }
    if (!options.viewer) throw refusal('E_AUTH', 'Snapback.open needs a persona viewer, or auth with the app secret store and Contract time');
    return this.openViewer({ ...options, viewer: options.viewer });
  }

  private static async openViewer(options: Options & { viewer: string }): Promise<Snapback> {
    if (!/^[A-Za-z0-9_%][-A-Za-z0-9._%]*$/.test(options.name))
      throw refusal('E_PARTITION_NAME', `Snapback4 needs app:/data/<safe filename>.sqlite; received ${JSON.stringify(options.name)}. Use Snapback.partitionName(principal).`);
    const path = `app:/data/${options.name}.sqlite`;
    const page = pageOf(options);
    const { partitions } = page;
    for (;;) {
      let pending = partitions.get(path);
      if (!pending) {
        const opening = openPartition(options, path, page);
        partitions.set(path, opening);
        opening.catch(() => { if (partitions.get(path) === opening) partitions.delete(path); });
        pending = opening;
      }
      const partition = await pending;
      partition.entry ??= pending;
      // Closed by its last client while this open waited: open it afresh.
      if (partition.closing) {
        if (partitions.get(path) === pending) partitions.delete(path);
        continue;
      }
      // A device that gave up (its saved data could not be reloaded) is
      // replaced: the next device takes over what it could not save.
      if (partition.device.healthy?.() === false) {
        if (partitions.get(path) === pending) partitions.delete(path);
        continue;
      }
      if (partition.viewer !== options.viewer || partition.origin !== options.origin) {
        throw refusal('E_PARTITION_VIEWER', `${path} is open in this page for ${partition.viewer} at ${partition.origin}, and a partition belongs to one viewer and one origin. Give each viewer its own name (for example \`${options.name}-${options.viewer.replace(/[^A-Za-z0-9_-]/g, '-')}\`) and grant sqlite.open for each.`);
      }
      partition.clients++;
      return new Snapback(partition, options);
    }
  }

  private async bindSession(): Promise<void> {
    const session = this.session;
    if (!session) return;
    if (this.held?.viewer === session.principal && !this.held.closing) return;
    if (this.held) { await this.close(); this.closed = false; this.held = undefined; }
    const name = `${this.baseName}-${Snapback.partitionName(session.principal)}`;
    if (this.options.native) {
      // One native device per module: retire every previous file, including
      // references held by another source or left after a refused session.
      const page = pageOf(this.options), path = `app:/data/${name}.sqlite`;
      for (const opening of [...page.partitions.values()]) {
        const previous = await opening;
        if (previous.path !== path && !previous.cleaned) await retirePartition(previous);
      }
    }
    const bound = await Snapback.openViewer({ ...this.options, name, viewer: session.principal });
    this.held = bound.held;
    this.options.name = name;
    this.options.viewer = session.principal;
  }

  private async signIn(path: string, email: string, password: string): Promise<SignedIn> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const auth = this.options.auth;
    if (!auth) throw refusal('E_CLIENT', 'signup/login needs Snapback.open auth: { store, key, now }');
    // Change identity only after logout, so outstanding work cannot cross users.
    if (this.session) throw refusal('E_AUTH', 'logout before signing in as another account');
    const reply = await this.exchange({ method: 'POST', path, body: { email, password } });
    if (reply.error) return { ok: false, why: { code: 'E_OFFLINE', message: String(reply.error), retryable: true } };
    const body = reply.body as { session?: Session; denied?: Refusal } | null;
    const session = sessionFrom(body?.session ? JSON.stringify(body.session) : null, 0);
    if (typeof reply.status === 'number' && reply.status >= 200 && reply.status < 300 && session) {
      auth.store.set(auth.key, JSON.stringify(session));
      await this.bindSession();
      return { ok: true, session };
    }
    return { ok: false, why: body?.denied ?? { code: 'E_AUTH', message: 'the server refused sign-in' } };
  }
  signup(email: string, password: string): Promise<SignedIn> { return this.authWork(() => this.signIn('/auth/signup', email, password)); }
  login(email: string, password: string): Promise<SignedIn> { return this.authWork(() => this.signIn('/auth/login', email, password)); }

  /** Local sign-out always happens, including offline. `ok` confirms revocation. */
  async logout(): Promise<{ ok: boolean; denied?: Refusal; offline?: boolean }> {
    return this.authWork(() => this.signOut());
  }
  private async signOut(): Promise<{ ok: boolean; denied?: Refusal; offline?: boolean }> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const auth = this.options.auth;
    if (!auth) throw refusal('E_CLIENT', 'logout needs Snapback.open auth: { store, key, now }');
    const principal = this.session?.principal ?? this.held?.viewer;
    let reply: Request = {};
    try { if (this.session) reply = await this.exchange({ method: 'POST', path: '/auth/logout' }); }
    finally {
      auth.store.forget(auth.key);
      // An auth handle may predate a sibling's sign-in and own no device.
      // Logout still retires that account's shared partitions, including
      // siblings that supply their credentials through headers instead.
      if (principal) for (const opening of [...pageOf(this.options).partitions.values()]) {
        const partition = await opening.catch(() => null);
        if (partition?.origin === this.options.origin && partition.viewer === principal && !partition.cleaned)
          await retirePartition(partition);
      }
      await this.releaseHandle(true); this.closed = false; this.held = undefined;
      this.options.name = this.baseName;
      this.options.viewer = undefined;
    }
    if (reply.error) return { ok: false, offline: true };
    const body = reply.body as { denied?: Refusal } | null;
    if (body?.denied) return { ok: false, denied: body.denied };
    return { ok: reply.status === undefined || (Number(reply.status) >= 200 && Number(reply.status) < 300) };
  }

  /** One partition open at a time, chosen by a key: the persona or sign-in
   * switch (see `Switcher`). `options` names each key's partition; give each
   * its own `name`. */
  static switcher<K extends string = string>(options: (key: K) => SwitchedOptions): Switcher<K> {
    return switcher(options);
  }

  /** Whether this client can still be used (including sign-in when signed
   * out; `session` is the authentication check): not closed, and its device has
   * not given up (the web's, when even rebuilding from the disk failed). An
   * app replaces an unusable client by opening the partition again: the new
   * device takes over what the old one could not save. */
  get usable(): boolean { return !this.closed && !this.held?.closing && (this.held?.device.healthy?.() ?? true); }

  /** Whether the partition is open (it has synced at least once). */
  get isOpen(): boolean { return !this.closed && !this.held?.closing && (this.held?.opened ?? false); }

  /** A partition that has never synced opens in its first round: start one
   * (or wait for the one running) before reading or writing. Unreached, the
   * device has nothing to answer from: `E_OFFLINE`. */
  private async ready(): Promise<void> {
    if (this.partition.opened) return;
    const round = await this.sync();
    if (this.partition.opened) return;
    // The server's own refusal (a session it does not take) is that refusal.
    if (round.denied && round.offline !== true && round.denied.code !== 'E_OFFLINE') throw refused(round.denied);
    throw refusal('E_OFFLINE', `this device has never synced and the server was not reached${round.denied ? ` (${round.denied.code}: ${round.denied.message})` : ''}; connect once to open it (in a test, let the app sync once, with \`clock data\`, before \`fail fetch\`)`, { family: 'transport', retryable: true });
  }

  /** A named query. The device answers it from its partition, unless it
   * reads a table or view the device does not sync (`online only`): then the
   * server answers it (`POST /q/<name>`), and the answer says `server: true`.
   * Unreached, such a read answers `denied` with `E_OFFLINE`, never an empty
   * page that looks like one. */
  async read<T = Json, O extends string | QueryOp<unknown, unknown> = string>(op: O, given: NoInfer<ArgsOf<O>>, now: number): Promise<Read<ResultOf<O, T>>> {
    const name = nameOf(op), args = given as Args;
    type R = ResultOf<O, T>;
    const at = clock(now);
    if (this.options.auth) this.options.auth.now = at;
    const check = this.fence();
    await this.ready();
    check();
    let route = this.partition.routes.get(name);
    if (!route) {
      route = ok<'device' | 'server' | 'held'>(await this.call({ op: 'route', name }));
      check();
      this.partition.routes.set(name, route);
    }
    if (route === 'server') return this.serverRead<R>(name, args);
    const local = ok<Read<R>>(await this.call({ op: 'read', name, args: args as Request, now: at }));
    check();
    // Whose answer (CLIENT-AND-OPERATIONS.md, "The device reply"): the
    // device's when it is not `unknown`, when it is `retained` history, or
    // when the schema holds the query to the device; otherwise the server's.
    // `E_PREDICT`/`E_NATIVE` with `unknown` mean evaluation never began.
    if (route === 'held' || local.unknown !== true || local.retained === true) return local;
    // The prediction fence, coarse: while any write is queued, the device's
    // answer (with its predicted rows) stands; a speculative one is not an
    // answer, so it shows as loading. Checked again when the server replies.
    const placeholder = (extra: Partial<Read<R>>): Read<R> =>
      local.speculative === true || local.loading === true
        ? { loading: true, unknown: true, ...extra }
        : { ...local, ...extra };
    if (((await this.status()).queued ?? []).length) return placeholder({});
    const served = await this.serverRead<R>(name, args);
    check();
    if (served.denied?.code === 'E_OFFLINE') return placeholder({ offline: true });
    if (((await this.status()).queued ?? []).length) return placeholder({});
    return served;
  }

  private async serverRead<T>(name: string, args: Args): Promise<Read<T>> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const check = this.fence();
    const token = this.session?.token;
    const reply = await this.exchange({ method: 'POST', path: `/q/${encodeURIComponent(name)}`, body: { args: args as Request } });
    check();
    if (reply.error !== undefined) {
      return { server: true, denied: { code: 'E_OFFLINE', family: 'transport', retryable: true,
        message: `${name} reads data this device does not keep (online only), so the server answers it, and it was not reached: ${String(reply.error)}` } };
    }
    const body = (reply.body ?? {}) as { data?: T; complete?: boolean; next?: string | null; denied?: Refusal };
    if (body.denied) {
      if (body.denied.code === 'E_AUTH') this.invalidate(token);
      return { server: true, denied: body.denied };
    }
    if (typeof reply.status === 'number' && (reply.status < 200 || reply.status >= 300)) {
      return { server: true, denied: { code: 'E_SERVER', family: 'transport', retryable: reply.status >= 500, message: `the server answered ${name} with HTTP ${reply.status}` } };
    }
    return { server: true, data: body.data, complete: body.complete ?? true, next: body.next ?? null };
  }

  /** One query page, with the named application refusals represented by null.
   * The refusal stays in `denied`; transport/auth/input failures still throw.
   * An empty view is an explicit application choice, never a complete list. */
  async readOptional<T = Json, O extends string | QueryOp<unknown, unknown> = string>(op: O, given: NoInfer<ArgsOf<O>>, now: number, emptyOn: readonly string[]): Promise<Read<ResultOf<O, T> | null>> {
    const answer = await this.read<T, O>(op, given, now);
    if (!answer.denied) return answer;
    const why = answer.denied;
    if (why.retryable || why.code.startsWith('E_') || !emptyOn.includes(why.code)) throw refused(why);
    return { ...answer, data: null, complete: false };
  }

  /** Every page of a paged query (`next` cursors passed back as `args.c`;
   * the first page sends no cursor, so an unpaged query reads whole).
   * `limit`, if given, refuses a query that holds more rows than that; by
   * default every row is read, and only a cursor that does not advance stops.
   * The rows say whether they are the whole answer: `complete` is false when
   * any page was not (rows past a sync horizon, a capped scan, or the
   * device's partial with the server unreached, which also sets `offline`).
   * Show such a list as partial, never as all there is. */
  async readAll<T = Json, O extends string | QueryOp<unknown, readonly unknown[]> = string>(op: O, given: NoInfer<ArgsOf<O>>, now: number, cursor = 'c', limit = Infinity): Promise<Rows<ResultOf<O, T[]>[number]>> {
    const check = this.fence();
    type E = ResultOf<O, T[]>[number];
    const name = nameOf(op), args = given as Args;
    const rows = Object.assign([] as E[], { complete: true, offline: false });
    const seen = new Set<string>();
    let next: string | null = null;
    do {
      check();
      const page: Read<E[]> = await this.read<E[]>(name, next === null ? args : { ...args, [cursor]: next }, now);
      if (page.denied) throw refused(page.denied);
      if (!Array.isArray(page.data)) throw new Error(`${name}: not on this device yet`);
      if (page.complete === false || page.offline === true) rows.complete = false;
      if (page.offline === true) rows.offline = true;
      rows.push(...page.data);
      next = page.next ?? null;
      if (rows.length > limit) throw new Error(`${name}: more than ${limit} rows`);
      if (next !== null) {
        if (seen.has(next)) throw new Error(`${name}: its pages do not end`);
        seen.add(next);
      }
    } while (next);
    return rows;
  }

  /** Admit a write: kept and predicted now, sent by the next `sync()`.
   * With an idempotency `key` (the app's name for this intent, such as the
   * draft version a post publishes), its id derives from the key, and writing
   * the same key again admits nothing: it answers what became of the first.
   * Whether the server took it is `outcome(id)` after a round: a round that
   * ends `ok` has delivered the outbox, not had every write accepted.
   * `failed` here is final and `outcome(id)` keeps answering it; a keyed
   * write is never refused by a prediction (the server decides it), and the
   * same key with other input answers `E_WRITE_ID_REUSE` while `outcome(id)`
   * still answers the first write's fate. */
  async write<O extends string | MutationOp<unknown, unknown>, K extends string | undefined = undefined>(op: O, args: NoInfer<ArgsOf<O>>, now: number, key?: K): Promise<K extends string ? Write | Outcome<ResultOf<O>> : Write> {
    const at = clock(now);
    if (this.options.auth) this.options.auth.now = at;
    const check = this.fence();
    await this.ready();
    check();
    return ok(await this.call({ op: 'write', name: nameOf(op), args: args as Request, now: at, ...(key === undefined ? {} : { key }) }));
  }

  /** Write, sync, and answer what became of the write: a mutation source's
   * one call. `write` only admits (it never sends), so an `outcome` asked
   * before a round is `pending`; `send` runs the round between them. `sent`
   * carries the server's `result`; `failed` its `why` (a `require … else
   * CODE`'s code, `E_RULE`), or the device's when it refused at once;
   * `pending` means the round did not deliver it (offline: kept, and sent
   * by the next round). `key` is `write`'s idempotency key. */
  async send<R = Json, O extends string | MutationOp<unknown, unknown> = string>(op: O, args: NoInfer<ArgsOf<O>>, now: number, key?: string): Promise<Sent<ResultOf<O, R>>> {
    const check = this.fence();
    const written = await this.write(nameOf(op), args as Args, now, key);
    check();
    if (written.state !== 'pending') return written as Sent<ResultOf<O, R>>;
    const round = await this.sync();
    check();
    return { ...await this.outcome<ResultOf<O, R>>(written.id), round };
  }

  /** The id a write with this idempotency key has (or would have). */
  async writeId(key: string): Promise<string> {
    return ok<string>(await this.call({ op: 'write_id', key }));
  }

  /** What became of a write: `pending` until a round sends it, `sent` or
   * `failed` (the server's verdict, or the device's at `write`), `unknown`
   * if this device never admitted it. It reads; it never sends. A `sync()`
   * that ends `ok`, asked for after the write, has sent it, so its outcome
   * is then `sent` or `failed` (however many other sources sync at once);
   * `send` is write, sync and outcome in one call. */
  async outcome<R = Json>(id: string): Promise<Outcome<R>> {
    return ok<Outcome<R>>(await this.call({ op: 'outcome', id }));
  }

  async status(): Promise<Status> {
    return ok<Status>(await this.call({ op: 'status' }));
  }

  /** One round: open if needed, sync to the head, send the outbox, sync again.
   * Rounds on a partition run one at a time: a `sync()` while another runs
   * (from this client or another source's) waits for it, then runs its own,
   * so what was written meanwhile is sent too. */
  sync(): Promise<Round> {
    if (this.closed) return Promise.reject(new Error('Snapback4: this client is closed'));
    const partition = this.partition;
    const check = this.fence();
    const run = partition.rounds.then(() => { check(); return this.round(); });
    partition.rounds = run.catch(() => undefined);
    return run.then(done => {
      // E_AUTH is a completed refusal, even though it forgets the session.
      if (done.denied?.code !== 'E_AUTH') check();
      return done;
    });
  }

  private async round(): Promise<Round> {
    const partition = this.partition;
    const check = this.fence();
    const call = (request: Request): Promise<Request> => {
      try { check(); } catch (error) { return Promise.reject(error); }
      return partition.device.call(request).then(answer => { check(); return answer; });
    };
    // A round an earlier answer could neither finish nor cancel is cancelled
    // first. Natively that is a superseded answer's round: its reply still
    // comes, but the device refuses calls once its answer has ended, and
    // left there the client would answer `busy` to every round after it.
    const abandoned = partition.abandoned;
    if (abandoned !== undefined) {
      await call({ op: 'cancel', exchange: abandoned });
      if (partition.abandoned === abandoned) partition.abandoned = undefined;
    }
    let step = ok<{ fetch?: Request; done?: Round }>(await call({ op: 'sync' }));
    let token: string | undefined;
    try {
      while (step.fetch) {
        const exchange = step.fetch.exchange as string;
        try {
          token = this.session?.token;
          const reply = await this.exchange(step.fetch);
          step = ok(await call({ op: 'deliver', exchange, reply }));
        } catch (error) {
          // Only this round's own exchange can be cancelled; one that cannot
          // be now is cancelled by the next round.
          await call({ op: 'cancel', exchange }).catch(() => { partition.abandoned = exchange; });
          throw error;
        }
      }
    } finally { partition.routes.clear(); }
    const done = step.done ?? { ok: false };
    if (done.ok) partition.opened = true;
    if (done.denied?.code === 'E_AUTH') this.invalidate(token);
    return done;
  }

  /** Wait up to `wait` seconds for the server's head to move. `true`: call
   * `sync()`. Throws when the server is unreachable or refuses. A poll while
   * another client of the partition has one in flight shares its answer. */
  async poll(wait = 20): Promise<boolean> {
    const check = this.fence();
    await this.ready();
    check();
    const partition = this.partition;
    // One at a time for every client of the partition: a caller while one is
    // in flight shares its answer (a second poll would supersede it).
    partition.polling ??= (async () => {
      const request = ok<{ fetch: Request }>(await this.call({ op: 'changes', wait }));
      const reply = await this.exchange(request.fetch);
      check();
      return ok<boolean>(await this.call({ op: 'changed', exchange: request.fetch.exchange, reply }));
    })().finally(() => { partition.polling = undefined; });
    const changed = await partition.polling;
    check();
    return changed;
  }

  /** Trade the session `headers()` sends for a fresh one (`POST
   * /auth/refresh`). The server retires the presented token before it
   * answers: keep the returned session at once and send its token from then
   * on. `E_AUTH` means the member signs in again; `offline` that the server
   * was not reached (the old token still works). */
  async refreshSession(now: number): Promise<Refreshed> {
    if (this.options.auth) {
      return this.authWork(() => this.refreshAuth(now));
    }
    const partition = this.partition;
    // One at a time, for every client of the partition: a caller while one
    // is in flight shares its answer.
    partition.refreshing ??= (async () => {
      const request = ok<{ fetch: Request }>(await this.call({ op: 'refresh' }));
      // Whatever stops this before its answer is delivered (`headers()`
      // throwing, a closed client) lets the refresh go; cancelling one already
      // answered does nothing.
      try {
        const reply = await this.exchange(request.fetch);
        return ok<Refreshed>(await this.call({ op: 'refreshed', exchange: request.fetch.exchange, reply, now: clock(now) }));
      } catch (error) {
        await this.call({ op: 'cancel', exchange: request.fetch.exchange }).catch(() => undefined);
        throw error;
      }
    })().finally(() => { partition.refreshing = undefined; });
    return partition.refreshing;
  }

  private async refreshAuth(now: number): Promise<Refreshed> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    this.options.auth!.now = clock(now);
    const auth = this.options.auth!, session = this.session;
    if (!session) return { ok: false, denied: { code: 'E_AUTH', message: 'sign in again' } };
    const reply = await this.exchange({ method: 'POST', path: '/auth/refresh' });
    if (reply.error) return { ok: false, offline: true, denied: { code: 'E_OFFLINE', message: String(reply.error), retryable: true } };
    const body = reply.body as { session?: Session; denied?: Refusal } | null;
    const renewed = sessionFrom(body?.session ? JSON.stringify(body.session) : null, 0);
    if (Number(reply.status) >= 200 && Number(reply.status) < 300 && renewed && renewed.principal === session.principal) {
      auth.store.set(auth.key, JSON.stringify(renewed));
      return { ok: true, session: renewed };
    }
    const denied = body?.denied ?? { code: 'E_AUTH', message: 'the server refused session refresh' };
    if (denied.code === 'E_AUTH') auth.store.forget(auth.key);
    return { ok: false, denied };
  }

  /** Close this client. It refuses every call from now on and says it is not
   * open. The partition closes with the last client that has it open;
   * closing that one again retries a cleanup that failed. */
  async close(): Promise<void> { await this.releaseHandle(false); }

  private async releaseHandle(force: boolean): Promise<void> {
    const partition = this.held;
    if (!partition) { this.closed = true; return; }
    if (!this.closed) {
      this.closed = true;
      partition.clients--;
    }
    if (partition.cleaned || (!force && partition.clients > 0)) return;
    await retirePartition(partition);
  }

  /** Refused writes (by the server or by the device at `write`), oldest
   * first, until dismissed. */
  async refusals(): Promise<Refused[]> {
    return ok<Refused[]>(await this.call({ op: 'refusals' }));
  }

  /** Forget refusals the app has shown: these ids, or all of them. */
  async dismiss(ids?: string[]): Promise<void> {
    ok(await this.call(ids ? { op: 'dismiss', ids } : { op: 'dismiss' }));
  }

  /** Durable bytes plus placing mutation, admitted offline after the first sync.
   * The next sync round uploads and places them. The path may be removed after this resolves.
   * argument names the omitted media argument; id is an optional stable intent key. */
  async uploadAndPlace<A extends Args, R, K extends Extract<keyof A, string>>(op: MutationOp<A, R>, args: Omit<A, K>, path: string, now: number, options: UploadAndPlaceOptions & {argument: K}): Promise<Outcome<R>> {
    const check = this.fence();
    const selected = JSON.parse(JSON.stringify({args, options})) as {args: Args; options: UploadAndPlaceOptions};
    const fs = this.options.storage?.fs;
    if (!fs?.readFile) throw refusal('E_CLIENT', 'uploadAndPlace needs storage.fs.readFile under fs.read for the selected path');
    await this.ready(); check();
    const bytes = new Uint8Array(await fs.readFile(path)); check();
    // Hex and the enclosing outbox record must fit Exact's 16 MiB storage/call boundary.
    if (!bytes.length || bytes.length > 4 << 20) throw refusal('E_UPLOAD_BYTES', 'select a nonempty file of at most 4 MiB for Exact durable upload storage');
    const upload = {bytes: Array.from(bytes, b => b.toString(16).padStart(2,'0')).join(''), argument: selected.options.argument,
      ...(selected.options.file ? {file: selected.options.file} : {})};
    return ok<Outcome<R>>(await this.call({op:'upload_and_place', name:op.name, args:selected.args as Json, now:clock(now), key:selected.options.id ?? crypto.randomUUID(), upload}));
  }

  /** Inspect a kept dependency or its terminal write; retry a blocked upload via retryUpload. */
  async uploads(): Promise<Json[]> {
    const entries = ok<Json[]>(await this.call({op:'queued'}));
    return entries.filter(e => !!e && typeof e==='object' && !Array.isArray(e) && !!e.upload).map(e => {
      const entry=e as Request, upload=entry.upload as Request;
      return {id:entry.id, state:upload.state, why:upload.why ?? null, asset:upload.asset ?? null};
    });
  }
  async retryUpload(id: string): Promise<void> {
    ok(await this.call({op:'upload_state', id, viewer:this.partition.viewer, state:'queued'}));
  }

  /** Upload a file's bytes (`POST /assets`, under this client's `headers`):
   * an app file by path (the picked `app:/tmp/picked/…`; the host sends it,
   * under `fs.read`, and the bytes never enter JavaScript) or bytes. Online
   * only, as in Snapback: offline it answers `E_OFFLINE` and nothing is
   * kept, so keep the path and ask again. Never retried here (a data module
   * has no timers): `E_RATE_LIMIT` is the app's to ask again. Then pass
   * `{ id: asset.id }` to the mutation; a write naming it offline queues
   * unpredicted, and the server judges it. */
  async upload(file: string | ArrayBuffer | ArrayBufferView, options: UploadOptions = {}): Promise<Uploaded> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const check = this.fence();
    const headers = { ...this.headers(), 'content-type': options.file?.type ?? 'application/octet-stream' };
    const query = options.file ? `?as=file&name=${encodeURIComponent(options.file.name)}` : '';
    const init: RequestInit & { exactBodyFrom?: string } = typeof file === 'string'
      ? { method: 'POST', headers, exactBodyFrom: file }
      // A copy: the bytes sent are these, whatever the caller does with its buffer meanwhile.
      : { method: 'POST', headers, body: Uint8Array.from(ArrayBuffer.isView(file) ? new Uint8Array(file.buffer, file.byteOffset, file.byteLength) : new Uint8Array(file)) };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets${query}`, init); } catch (error) { return { ok: false, why: unreached(error) }; }
    check();
    const body = await response.json().catch(() => null) as { asset?: Asset; denied?: Refusal } | null;
    check();
    if (response.ok && body?.asset?.id) return { ok: true, asset: body.asset };
    return { ok: false, why: body?.denied ?? { code: 'E_SERVER', family: 'transport', retryable: response.status >= 500, message: `the server answered the upload with HTTP ${response.status}` } };
  }

  /** Download an asset (`GET /assets/<id>`, under this client's `headers`,
   * so its row's read rule decides) into an app file, and answer its path
   * for an `image` or `video`: by default
   * `app:/cache/snapback4-<name>-<id>.<jpg|png|webp|mp4>` (grant `fs.write
   * app:/cache`). Online only, and nothing is purged here: the file stays
   * until the app removes it, so give a `once` asset (one view per
   * principal, which this download spends) a path under `app:/tmp`. */
  async download(id: string, path?: string): Promise<Downloaded> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const check = this.fence();
    const fs = this.options.storage?.fs;
    if (!fs) return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: 'download writes an app file: pass the dispatcher\'s `storage`, and grant fs.write for the path' } };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets/${encodeURIComponent(id)}`, { headers: this.headers() }); } catch (error) { return { ok: false, why: unreached(error) }; }
    check();
    if (!response.ok) {
      const body = await response.json().catch(() => null) as { denied?: Refusal } | null;
      return { ok: false, why: body?.denied ?? { code: 'E_SERVER', family: 'transport', retryable: response.status >= 500, message: `the server answered asset ${id} with HTTP ${response.status}` } };
    }
    const type = (response.headers.get('content-type') ?? '').split(';')[0].trim();
    const disposition = response.headers.get('content-disposition') ?? '';
    const encoded = /filename\*=UTF-8''([^;]+)/i.exec(disposition)?.[1];
    let name = /filename="([^"]*)"/i.exec(disposition)?.[1];
    if (encoded) try { name = decodeURIComponent(encoded); } catch { /* Keep the plain filename. */ }
    // An attachment name is metadata, never an app path supplied by the server.
    const filename = name?.replace(/[/\\\x00-\x1f\x7f]/g, '_');
    const bytes = new Uint8Array(await response.arrayBuffer());
    check();
    const file = path ?? `app:/cache/snapback4-${this.options.name}-${id}${filename ? `-${filename}` : `.${EXTENSIONS[type] ?? 'bin'}`}`;
    try { await fs.writeFile(file, bytes); } catch (error) { return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: `could not write ${file}: ${error instanceof Error ? error.message : String(error)}` } }; }
    check();
    return { ok: true, path: file, type, bytes: bytes.byteLength, ...(name === undefined ? {} : { name }) };
  }

  private async exchange(fetchRequest: Request): Promise<Request> {
    const { method, path, body, bodyBytes } = fetchRequest as { method: string; path: string; body?: Json; bodyBytes?: string };
    const headers = this.headers();
    const releaseKey = this.releases.identity(headers);
    for (const key of Object.keys(headers)) if (key.toLowerCase() === 'x-snapback-device') delete headers[key];
    Object.assign(headers, fetchRequest.headers ?? {});
    if (body !== undefined) headers['content-type'] = 'application/json';
    if (this.held && !path.startsWith('/auth/')) {
      const engine = await this.call({ op: 'engine_info' });
      if (!engine.ok) throw new Error('E_DEVICE_VERSION: device has no engine identity; run bun exact.mjs snapback --rebuild');
      headers['x-snapback-device'] = JSON.stringify(engine.ok);
    }
    try {
      const response = await fetch(`${this.options.origin.replace(/\/+$/, '')}${path}`, { method, headers, ...(bodyBytes ? {body: Uint8Array.from(bodyBytes.match(/../g) ?? [], b => parseInt(b,16))} : body === undefined ? {} : { body: JSON.stringify(body) }) });
      this.releases.observe(response, releaseKey);
      const text = await response.text();
      let parsed: Json = null;
      try { parsed = JSON.parse(text) as Json; } catch { parsed = null; }
      return { status: response.status, body: parsed };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  }
}

export const openSnapback = (options: Options) => Snapback.open(options);

/** What a switcher opens for a key: `Options` less the dispatcher's
 * `storage` and `native`, which `use` passes. */
export type SwitchedOptions = Omit<Options, 'storage' | 'native' | 'name'> & { name?: string };

/** One partition open at a time, chosen by a key (a persona, a signed-in
 * principal): what `Snapback.switcher` returns. */
export interface Switcher<K extends string = string> {
  /** Run `work` on the key's client. A key other than the current one first
   * lets the current one go: its work in flight finishes, then its partition
   * closes (what it keeps stays for its next open), then the key's opens. An
   * open that fails (`code: 'bake'` at build time, `E_OFFLINE`) rejects this
   * call and is tried again by the next; a client whose device gave up
   * (`usable` false) is reopened. */
  use<T>(key: K, storage: Options['storage'], native: Options['native'], work: (db: Snapback) => Promise<T>): Promise<T>;
  /** Let the current client go, once its work in flight is done (a sign-out). */
  close(): Promise<void>;
  /** The key now in use, or null. */
  readonly key: K | null;
}

/** A switcher: the persona or sign-in switch every app with more than one
 * viewer needs (natively the module holds one partition, so the old one must
 * close before the new one opens, and only once its work is done). `options`
 * names each key's partition; give each its own `name`.
 *
 *   const personas = Snapback.switcher((persona: string) => ({ app: appId, name: `inbox-${persona}`,
 *     origin, viewer: `dev:${persona}`, headers: () => ({ 'x-snapback-persona': persona }) }));
 *   // in a source: personas.use(persona, storage, native, async db => { await db.sync(); … })
 */
function switcher<K extends string = string>(options: (key: K) => SwitchedOptions): Switcher<K> {
  type Held = { key: K; opening: Promise<Snapback>; client?: Snapback; busy: Set<Promise<unknown>> };
  let current: Held | null = null;
  // Clients are let go one after another, and the next open waits for the last.
  let released: Promise<void> = Promise.resolve();
  const release = (held: Held | null): Promise<void> => {
    if (!held) return released;
    const before = released;
    released = (async () => {
      await before;
      await Promise.allSettled([...held.busy]);
      await (await held.opening.catch(() => null))?.close();
    })().catch(() => undefined);
    return released;
  };
  return {
    get key() { return current?.key ?? null; },
    use(key, storage, native, work) {
      if (current?.key !== key || current.client?.usable === false) {
        const opening = release(current).then(() => {
          const selected = options(key);
          const name = selected.name ?? Snapback.partitionName(key);
          return Snapback.open({ ...selected, name: /^[A-Za-z0-9_%][-A-Za-z0-9._%]*$/.test(name) ? name : Snapback.partitionName(name), storage, native });
        });
        const entry: Held = { key, opening, busy: new Set() };
        current = entry;
        opening.then(client => { entry.client = client; }, () => { if (current === entry) current = null; });
      }
      const held = current!;
      const run = held.opening.then(work);
      held.busy.add(run);
      const done = () => { held.busy.delete(run); };
      run.then(done, done);
      return run;
    },
    close() {
      const held = current;
      current = null;
      return release(held);
    },
  };
}
