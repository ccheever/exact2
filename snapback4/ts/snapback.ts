// Snapback4 for an Exact app's TypeScript: the client in Rust
// (`exact-snapback4-client`), driven here over `fetch`. Natively it is the
// app's native module (`exact_snapback4::Module` behind `native.call`); on
// the web it is the same Rust as wasm, persisted through Exact SQLite
// (`./web.ts`). Mount this directory in app.json:
//   "typescript": { "sources": { "snapback": "../exact2/snapback4/ts" } }
// and `import { openSnapback } from './snapback/snapback.ts'`.
//
// Data modules have no clock: pass `now` (milliseconds) from a source argument.

import { webDevice, type SqliteStorage } from './web.ts';
export type { SqliteStorage } from './web.ts';

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

/** A write as admitted: kept and predicted on this device, sent by a round. */
export type Write =
  | { id: string; state: 'pending'; newIds: string[] }
  | { id: string; state: 'failed'; why: Refusal };

/** A write the server refused, kept in the partition until dismissed: what
 * it was (`op`, `args`, so its input is not lost), why, and when written.
 * A refusal too large to store (over 8 MiB once encoded) keeps all but its
 * `args`, which are then null and `argsOmitted` is true. */
export interface Refused { id: string; op: string; args: Json; argsOmitted?: boolean; why: Refusal; at: number }

/** What became of a write. `result` is the server's, while remembered. */
export interface Outcome { id: string; state: 'pending' | 'sent' | 'failed' | 'unknown'; seq?: number; result?: Json; why?: Refusal }

/** A session as Snapback mints it; `expiresAt` in milliseconds. */
export interface Session { principal: string; kind: string; token: string; expiresAt: number }

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

/** Exact's native module, when the app links `exact_snapback4::Module`. */
export interface NativeModule { call(request: Request): unknown }

export interface Options {
  /** The app's id: the web partition's identity (natively, the baked one). */
  app: string;
  /** The partition's file: `app:/data/<name>.sqlite`. One per origin and viewer. */
  name: string;
  /** The Snapback origin, e.g. `https://api.example.com`. Grant `net.fetch` for it. */
  origin: string;
  /** The principal this device reads and writes as (`dev:alice`, a session's principal). */
  viewer: string;
  /** Credentials for each request: `{authorization: 'Bearer …'}`, or in development `{'x-snapback-persona': 'alice'}`. */
  headers?: () => Record<string, string>;
  /** The dispatcher's `storage`, Exact's own (the web persists the partition
   * there); only its `sqlite` is used. */
  storage: { readonly sqlite: { open(path: string): Promise<unknown> } };
  /** The dispatcher's `native`; absent on the web. */
  native?: NativeModule | null;
  /** Where the web's wasm is served (default `/assets/snapback4.wasm`). */
  wasm?: string;
}

function ok<T>(answer: Request): T {
  const denied = answer.denied as Refusal | undefined;
  if (denied) throw Object.assign(new Error(`${denied.code}: ${denied.message}`), { refusal: denied });
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
  refreshing?: Promise<Refreshed>;
  /** Which queries the server answers, by name; forgotten after each round
   * (a round can adopt a new backend). */
  routes: Map<string, 'device' | 'server'>;
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
interface Page { partitions: Map<string, Promise<Partition>>; closing: Map<string, Promise<unknown>> }
const pages = new WeakMap<object, Page>();
function pageOf(options: Options): Page {
  const owner = (options.native ?? options.storage) as object;
  let page = pages.get(owner);
  if (!page) pages.set(owner, page = { partitions: new Map(), closing: new Map() });
  return page;
}

function refusal(code: string, message: string, extra: Partial<Refusal> = {}): Error {
  return Object.assign(new Error(`${code}: ${message}`), { refusal: { code, message, ...extra } as Refusal });
}

async function openPartition(options: Options, path: string, page: Page): Promise<Partition> {
  await page.closing.get(path)?.catch(() => undefined);
  const device = options.native
    ? nativeDevice(options.native)
    : await webDevice(options.storage as SqliteStorage, options.app, path, options.wasm ?? '/assets/snapback4.wasm');
  try {
    const opened = ok<{ opened: boolean }>(await device.call({ op: 'open', path, origin: options.origin, viewer: options.viewer }));
    return { page, path, origin: options.origin, viewer: options.viewer, device, opened: opened.opened, clients: 0, rounds: Promise.resolve(), routes: new Map(), closing: false, cleaned: false };
  } catch (error) { await device.close?.(); throw error; }
}

export class Snapback {
  private closed = false;
  private constructor(private partition: Partition, readonly options: Options) {}

  /** Every call goes through here: a closed client refuses, rather than
   * reaching whatever partition its device holds now. */
  private call(request: Request): Promise<Request> {
    if (this.closed) return Promise.reject(new Error('Snapback4: this client is closed'));
    return this.partition.device.call(request);
  }

  /** Open the partition this device keeps. Offline is fine once it has
   * synced; a partition that never has opens in its first round, which the
   * first `read` or `write` starts if the app has not called `sync()`.
   *
   * Opening a partition this page already has open shares it: every source
   * may call `open` for itself. A partition belongs to one viewer, so a
   * second viewer needs its own `name` (for example `inbox-${persona}`). */
  static async open(options: Options): Promise<Snapback> {
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

  /** Whether this client can still be used: not closed, and its device has
   * not given up (the web's, when even rebuilding from the disk failed). An
   * app replaces an unusable client by opening the partition again: the new
   * device takes over what the old one could not save. */
  get usable(): boolean { return !this.closed && (this.partition.device.healthy?.() ?? true); }

  /** Whether the partition is open (it has synced at least once). */
  get isOpen(): boolean { return !this.closed && this.partition.opened; }

  /** A partition that has never synced opens in its first round: start one
   * (or wait for the one running) before reading or writing. Unreached, the
   * device has nothing to answer from: `E_OFFLINE`. */
  private async ready(): Promise<void> {
    if (this.partition.opened) return;
    const round = await this.sync();
    if (!this.partition.opened) {
      throw refusal('E_OFFLINE', `this device has never synced and the server was not reached${round.denied ? ` (${round.denied.code}: ${round.denied.message})` : ''}; connect once to open it (in a test, let the app sync once, with \`clock data\`, before \`fail fetch\`)`, { retryable: true });
    }
  }

  /** A named query. The device answers it from its partition, unless it
   * reads a table or view the device does not sync (`online only`): then the
   * server answers it (`POST /q/<name>`), and the answer says `server: true`.
   * Unreached, such a read answers `denied` with `E_OFFLINE`, never an empty
   * page that looks like one. */
  async read<T = Json>(name: string, args: Args, now: number): Promise<Read<T>> {
    const at = clock(now);
    await this.ready();
    let route = this.partition.routes.get(name);
    if (!route) {
      route = ok<'device' | 'server'>(await this.call({ op: 'route', name }));
      this.partition.routes.set(name, route);
    }
    if (route === 'server') return this.serverRead<T>(name, args);
    const local = ok<Read<T>>(await this.call({ op: 'read', name, args: args as Request, now: at }));
    // A read the device cannot vouch for (a total past its sync horizon, a
    // point it has not acquired) asks the server, as Snapback's own client
    // does, unless a write here is still unsent: then the device's
    // prediction stands until a round settles it.
    if (local.loading !== true && local.speculative !== true) return local;
    const queued = (await this.status()).queued ?? [];
    if (queued.length) return local;
    const served = await this.serverRead<T>(name, args);
    if (served.denied?.code === 'E_OFFLINE') return { ...local, offline: true };
    return served;
  }

  private async serverRead<T>(name: string, args: Args): Promise<Read<T>> {
    if (this.closed) throw new Error('Snapback4: this client is closed');
    const reply = await this.exchange({ method: 'POST', path: `/q/${encodeURIComponent(name)}`, body: { args: args as Request } });
    if (reply.error !== undefined) {
      return { server: true, denied: { code: 'E_OFFLINE', family: 'transport', retryable: true,
        message: `${name} reads data this device does not keep (online only), so the server answers it, and it was not reached: ${String(reply.error)}` } };
    }
    const body = (reply.body ?? {}) as { data?: T; complete?: boolean; next?: string | null; denied?: Refusal };
    if (body.denied) return { server: true, denied: body.denied };
    if (typeof reply.status === 'number' && (reply.status < 200 || reply.status >= 300)) {
      return { server: true, denied: { code: 'E_SERVER', family: 'transport', retryable: reply.status >= 500, message: `the server answered ${name} with HTTP ${reply.status}` } };
    }
    return { server: true, data: body.data, complete: body.complete ?? true, next: body.next ?? null };
  }

  /** Every page of a paged query (`next` cursors passed back as `args.c`;
   * the first page sends no cursor, so an unpaged query reads whole).
   * `limit`, if given, refuses a query that holds more rows than that; by
   * default every row is read, and only a cursor that does not advance stops. */
  async readAll<T = Json>(name: string, args: Args, now: number, cursor = 'c', limit = Infinity): Promise<T[]> {
    const rows: T[] = [];
    const seen = new Set<string>();
    let next: string | null = null;
    do {
      const page: Read<T[]> = await this.read<T[]>(name, next === null ? args : { ...args, [cursor]: next }, now);
      if (page.denied) throw Object.assign(new Error(`${page.denied.code}: ${page.denied.message}`), { refusal: page.denied });
      if (!Array.isArray(page.data)) throw new Error(`${name}: not on this device yet`);
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
   * ends `ok` has delivered the outbox, not had every write accepted. */
  async write(name: string, args: Args, now: number): Promise<Write>;
  async write(name: string, args: Args, now: number, key: string): Promise<Write | Outcome>;
  async write(name: string, args: Args, now: number, key?: string): Promise<Write | Outcome> {
    const at = clock(now);
    await this.ready();
    return ok<Write | Outcome>(await this.call({ op: 'write', name, args: args as Request, now: at, ...(key === undefined ? {} : { key }) }));
  }

  /** The id a write with this idempotency key has (or would have). */
  async writeId(key: string): Promise<string> {
    return ok<string>(await this.call({ op: 'write_id', key }));
  }

  /** What became of a write. */
  async outcome(id: string): Promise<Outcome> {
    return ok<Outcome>(await this.call({ op: 'outcome', id }));
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
    const run = partition.rounds.then(() => this.round());
    partition.rounds = run.catch(() => undefined);
    return run;
  }

  private async round(): Promise<Round> {
    let step = ok<{ fetch?: Request; done?: Round }>(await this.call({ op: 'sync' }));
    try {
      while (step.fetch) {
        const exchange = step.fetch.exchange;
        const reply = await this.exchange(step.fetch);
        try { step = ok(await this.call({ op: 'deliver', exchange, reply })); }
        catch (error) {
          // Only this round's own exchange can be cancelled.
          await this.call({ op: 'cancel', exchange }).catch(() => undefined);
          throw error;
        }
      }
    } finally { this.partition.routes.clear(); }
    const done = step.done ?? { ok: false };
    if (done.ok) this.partition.opened = true;
    return done;
  }

  /** Wait up to `wait` seconds for the server's head to move. `true`: call
   * `sync()`. Throws when the server is unreachable or refuses. */
  async poll(wait = 20): Promise<boolean> {
    const request = ok<{ fetch: Request }>(await this.call({ op: 'changes', wait }));
    const reply = await this.exchange(request.fetch);
    return ok<boolean>(await this.call({ op: 'changed', exchange: request.fetch.exchange, reply }));
  }

  /** Trade the session `headers()` sends for a fresh one (`POST
   * /auth/refresh`). The server retires the presented token before it
   * answers: keep the returned session at once and send its token from then
   * on. `E_AUTH` means the member signs in again; `offline` that the server
   * was not reached (the old token still works). */
  async refreshSession(now: number): Promise<Refreshed> {
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

  /** Close this client. It refuses every call from now on and says it is not
   * open. The partition closes with the last client that has it open;
   * closing that one again retries a cleanup that failed. */
  async close(): Promise<void> {
    const partition = this.partition;
    if (!this.closed) {
      this.closed = true;
      partition.clients--;
    }
    if (partition.clients > 0 || partition.cleaned) return;
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

  /** Writes the server refused, oldest first, until dismissed. */
  async refusals(): Promise<Refused[]> {
    return ok<Refused[]>(await this.call({ op: 'refusals' }));
  }

  /** Forget refusals the app has shown: these ids, or all of them. */
  async dismiss(ids?: string[]): Promise<void> {
    ok(await this.call(ids ? { op: 'dismiss', ids } : { op: 'dismiss' }));
  }

  private async exchange(fetchRequest: Request): Promise<Request> {
    const { method, path, body } = fetchRequest as { method: string; path: string; body?: Json };
    const headers: Record<string, string> = { ...(this.options.headers?.() ?? {}) };
    if (body !== undefined) headers['content-type'] = 'application/json';
    try {
      const response = await fetch(`${this.options.origin}${path}`, { method, headers, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
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
