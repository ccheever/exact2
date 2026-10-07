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

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Request = { [key: string]: Json };

export interface Refusal { code: string; family?: string; message: string; retryable?: boolean; [key: string]: Json | undefined }

/** A named query's answer, as the device reads it. Rows a pending write
 * predicted carry `pending: true`. */
export interface Read<T = Json> {
  data?: T;
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
  /** The dispatcher's `storage` (the web persists the partition there). */
  storage: SqliteStorage;
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

export class Snapback {
  private closed = false;
  private cleaned = false;
  private refreshing?: Promise<Refreshed>;
  private constructor(private device: Device, readonly options: Options, private opened: boolean) {}

  /** Every call goes through here: a closed client refuses, rather than
   * reaching whatever partition its device holds now. */
  private call(request: Request): Promise<Request> {
    if (this.closed) return Promise.reject(new Error('Snapback4: this client is closed'));
    return this.device.call(request);
  }

  /** Open the partition this device keeps. Offline is fine once it has
   * synced; a partition that never has opens in the first `sync()`. */
  static async open(options: Options): Promise<Snapback> {
    const path = `app:/data/${options.name}.sqlite`;
    const device = options.native
      ? nativeDevice(options.native)
      : await webDevice(options.storage, options.app, path, options.wasm ?? '/assets/snapback4.wasm');
    try {
      const opened = ok<{ opened: boolean }>(await device.call({ op: 'open', path, origin: options.origin, viewer: options.viewer }));
      return new Snapback(device, options, opened.opened);
    } catch (error) { await device.close?.(); throw error; }
  }

  /** Whether this client can still be used: not closed, and its device has
   * not given up (the web's, when even rebuilding from the disk failed). An
   * app replaces an unusable client by opening the partition again: the new
   * device takes over what the old one could not save. */
  get usable(): boolean { return !this.closed && (this.device.healthy?.() ?? true); }

  /** Whether the partition is open (it has synced at least once). */
  get isOpen(): boolean { return this.opened; }

  /** A named query, answered on this device from its partition. */
  async read<T = Json>(name: string, args: Request, now: number): Promise<Read<T>> {
    return ok<Read<T>>(await this.call({ op: 'read', name, args, now: clock(now) }));
  }

  /** Every page of a paged query (`next` cursors passed back as `args.c`).
   * `limit`, if given, refuses a query that holds more rows than that; by
   * default every row is read, and only a cursor that does not advance stops. */
  async readAll<T = Json>(name: string, args: Request, now: number, cursor = 'c', limit = Infinity): Promise<T[]> {
    const rows: T[] = [];
    const seen = new Set<string>();
    let next: string | null = null;
    do {
      const page: Read<T[]> = await this.read<T[]>(name, { ...args, [cursor]: next }, now);
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
   * the same key again admits nothing: it answers what became of the first. */
  async write(name: string, args: Request, now: number): Promise<Write>;
  async write(name: string, args: Request, now: number, key: string): Promise<Write | Outcome>;
  async write(name: string, args: Request, now: number, key?: string): Promise<Write | Outcome> {
    return ok<Write | Outcome>(await this.call({ op: 'write', name, args, now: clock(now), ...(key === undefined ? {} : { key }) }));
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
   * A round already running answers `{ok:false, busy:true}` at once. */
  async sync(): Promise<Round> {
    let step = ok<{ fetch?: Request; done?: Round }>(await this.call({ op: 'sync' }));
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
    const done = step.done ?? { ok: false };
    if (done.ok) this.opened = true;
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
    // One at a time: a caller while one is in flight shares its answer.
    this.refreshing ??= (async () => {
      const request = ok<{ fetch: Request }>(await this.call({ op: 'refresh' }));
      const reply = await this.exchange(request.fetch);
      try { return ok<Refreshed>(await this.call({ op: 'refreshed', exchange: request.fetch.exchange, reply, now: clock(now) })); }
      catch (error) {
        await this.call({ op: 'cancel', exchange: request.fetch.exchange }).catch(() => undefined);
        throw error;
      }
    })().finally(() => { this.refreshing = undefined; });
    return this.refreshing;
  }

  /** Close the partition. The client refuses every call from now on, and
   * says it is not open; closing again retries a cleanup that failed. */
  async close(): Promise<void> {
    this.closed = true;
    this.opened = false;
    if (this.cleaned) return;
    await this.device.call({ op: 'close' }).catch(() => undefined);
    // A cleanup the device deferred (it will run it) or that failed can be
    // asked for again.
    this.cleaned = (await this.device.close?.()) !== false;
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
