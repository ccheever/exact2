export { pick } from './projection.ts';
import { ReleaseTracker, type ReleaseSnapshot } from './release.ts';
export type { ReleaseSnapshot } from './release.ts';
// @ref LLP 1086 D12 — server-only operations: no device, partition, or wasm import.
export interface Refusal { code: string; message: string; family?: string; retryable?: boolean }
export interface Op<A = Record<string, unknown>, R = unknown> {
  readonly kind: string; readonly name: string; readonly __args?: A; readonly __result?: R;
}
export type ArgsOf<O> = O extends Op<infer A, unknown> ? A : Record<string, unknown>;
export type ResultOf<O> = O extends Op<unknown, infer R> ? R : unknown;
export interface Options {
  origin: string;
  /** Password sessions in Exact’s secret.keep store; now is Contract epoch milliseconds. */
  auth?: AuthOptions;
  headers?: () => Record<string, string | undefined>;
  /** Download writes under the dispatcher's fs.write grant; no partition is opened. */
  storage?: { fs: { writeFile(path: string, bytes: Uint8Array): Promise<void> } } | null;
  name?: string;
}
export type Cursor = string & { readonly __cursor: 'Cursor' };
export type ReadCut = 'meter' | 'words' | null;
/** The server's /q envelope. Missing metadata on a denial is never filled in. */
export type Read<T> =
  | { data: T; complete?: boolean; cut?: ReadCut; next?: Cursor | null; server: true; denied?: never }
  | { data?: never; complete?: never; cut?: never; next?: never; server: true; denied: Refusal };
/** Rows retain every page's completeness and cut. A denial throws, never returns partial success. */
export type Rows<T> = T[] & { complete: boolean; cut: ReadCut; server: true };
export interface Session { principal: string; kind: string; token: string; expiresAt: number }
export interface SessionStore { get(key: string): string | null; set(key: string, value: string): void; forget(key: string): void }
export interface AuthOptions { store: SessionStore; key: string; now: number }
export interface AuthConfig { guests: boolean; password: boolean; development: boolean }
export type SignedIn = { ok: true; session: Session } | { ok: false; why: Refusal };
export type Refreshed = { ok: true; session: Session } | { ok: false; denied: Refusal; offline?: boolean };
// Exact creates a new secret facade per source answer; facade identity is not store identity.
// A module realm belongs to one app. Only scheduling is shared, never session values.
const authJobs = new Map<string, Promise<unknown>>();
// A later Contract sample may outlive the old token while its replacement is in flight.
const rotations = new Map<string, string | null>();
function sessionFrom(raw: string | null, now: number): Session | null {
  if (!Number.isFinite(now) || now < 0) throw refused({code: 'E_CLIENT', message: 'Pass Contract epoch milliseconds to restore/refreshSession.'});
  try {
    const s = JSON.parse(raw ?? 'null') as Session | null;
    return s && typeof s.principal === 'string' && s.principal !== '' && typeof s.kind === 'string'
      && typeof s.token === 'string' && s.token !== '' && Number.isFinite(s.expiresAt) && s.expiresAt > now ? s : null;
  } catch { return null; }
}
export type Sent<T> = { state: 'sent'; result: T } | { state: 'failed'; why: Refusal };
export interface Asset { id: string; type: string; bytes: number; name?: string; width?: number | null; height?: number | null; duration?: number | null }
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


const isRefusal = (value: unknown): value is Refusal => !!value && typeof value === 'object' && typeof (value as Refusal).code === 'string' && typeof (value as Refusal).message === 'string';

const nameOf = (op: string | Op<unknown, unknown>): string => typeof op === 'string' ? op : op.name;

/** Use when the schema syncs nothing. Calls go straight to /q and /m; writes
 * have no prediction or outbox, and an interrupted write has an unknown fate.
 * Never retry it automatically: the server may already have committed it. */
export class Snapback {
  constructor(private readonly options: Options) {}
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

  private headers(authenticate = true): Record<string, string> {
    const headers: Record<string, string> = {};
    for (const [key, value] of Object.entries(this.options.headers?.() ?? {})) if (value !== undefined) headers[key] = value;
    if (this.options.auth || !authenticate) {
      for (const key of Object.keys(headers)) if (['authorization', 'x-snapback-persona'].includes(key.toLowerCase())) delete headers[key];
      const session = authenticate ? this.session : null;
      if (session) headers.authorization = `Bearer ${session.token}`;
    }
    return headers;
  }
  /** Reads the shared secret store, so sibling sources see sign-in and sign-out immediately. */
  get session(): Session | null {
    const auth = this.options.auth;
    if (!auth) return null;
    const raw = auth.store.get(auth.key), session = sessionFrom(raw, auth.now);
    if (raw && !session && rotations.get(this.authKey()) !== raw) auth.store.forget(auth.key);
    return session;
  }
  private auth(): AuthOptions {
    if (!this.options.auth) throw refused({code: 'E_CLIENT', message: 'Pass auth: {store, key, now}; grant secret.keep for key.'});
    return this.options.auth;
  }
  private authKey(): string { return JSON.stringify([this.options.origin.replace(/\/+$/, ''), this.options.auth?.key]); }
  private authWork<T>(work: () => Promise<T>): Promise<T> {
    this.auth();
    const id = this.authKey();
    const next = (authJobs.get(id) ?? Promise.resolve()).catch(() => undefined).then(work);
    authJobs.set(id, next);
    const clear = () => { if (authJobs.get(id) === next) authJobs.delete(id); };
    next.then(clear, clear);
    return next;
  }
  private fence(): () => void {
    const auth = this.options.auth, raw = auth?.store.get(auth.key);
    if (auth && !this.session) throw refused({code: 'E_AUTH', message: 'Sign in before reading or writing.'});
    return () => {
      if (auth && raw !== auth.store.get(auth.key)) throw refused({code: 'E_STALE', message: 'The session changed while this request was in flight; read again.'});
    };
  }
  private unchanged(raw: string | null): void {
    const auth = this.auth();
    if (auth.store.get(auth.key) !== raw) throw refused({code: 'E_STALE', message: 'The session changed while authentication was in flight; restore the current account.'});
  }
  private invalidate(token: string | undefined): void {
    const auth = this.options.auth;
    if (auth && token && this.session?.token === token) auth.store.forget(auth.key);
  }
  /** A request using the old token may be denied while refresh has minted its replacement.
   * Let pending auth finish, then fence the old reply; never discard a successful rotation. */
  private async rejectSession(token: string | undefined, check: () => void): Promise<void> {
    if (!token) return;
    const pending = authJobs.get(this.authKey());
    if (pending) { await pending.catch(() => undefined); check(); }
    this.invalidate(token);
  }
  private async authRequest(path: string, body: unknown = {}): Promise<{ok: boolean; body: {session?: Session; denied?: Refusal}}> {
    try {
      const response = await fetch(this.options.origin.replace(/\/+$/, '') + path, {
        method: 'POST', headers: {...this.headers(), 'content-type': 'application/json'}, body: JSON.stringify(body),
      });
      const reply = await response.json().catch(() => null);
      if (!reply || typeof reply !== 'object' || Array.isArray(reply)) return {ok: false, body: {denied: {code: 'E_SERVER', message: `Invalid auth reply (HTTP ${response.status})`}}};
      return {ok: response.ok, body: reply};
    } catch (error) { return {ok: false, body: {denied: unreached(error)}}; }
  }
  private async signIn(path: string, credentials: { email: string; password: string } | Record<string, never>): Promise<SignedIn> {
    const auth = this.auth();
    if (this.session) throw refused({code: 'E_AUTH', message: 'Logout before signing in as another account.'});
    const raw = auth.store.get(auth.key);
    const reply = await this.authRequest(path, credentials);
    this.unchanged(raw);
    const session = sessionFrom(JSON.stringify(reply.body.session ?? null), auth.now);
    if (!reply.ok || !session) return {ok: false, why: reply.body.denied ?? {code: 'E_SERVER', message: 'Invalid sign-in reply.'}};
    auth.store.set(auth.key, JSON.stringify(session));
    return {ok: true, session};
  }
  signup(email: string, password: string): Promise<SignedIn> { return this.authWork(() => this.signIn('/auth/signup', { email, password })); }
  login(email: string, password: string): Promise<SignedIn> { return this.authWork(() => this.signIn('/auth/login', { email, password })); }
  /** Bare `use identity` enables guest sessions; logout before signup or login. */
  guest(): Promise<SignedIn> { return this.authWork(() => this.signIn('/auth/guest', {})); }
  /** Public discovery, available before sign-in. Never changes the stored session. */
  async authConfig(): Promise<AuthConfig> {
    const auth = this.options.auth, raw = auth?.store.get(auth.key);
    let response: Response;
    try { response = await fetch(this.options.origin.replace(/\/+$/, '') + '/auth/config', {headers: this.headers(false)}); }
    catch (error) { throw refused(unreached(error)); }
    const body = await response.json().catch(() => null);
    if (auth) this.unchanged(raw ?? null);
    if (!response.ok || !body || typeof body.guests !== 'boolean' || typeof body.password !== 'boolean' || typeof body.development !== 'boolean')
      throw refused(isRefusal(body?.denied) ? body.denied : {code: 'E_SERVER', message: 'Invalid auth configuration reply.'});
    return {guests: body.guests, password: body.password, development: body.development};
  }
  /** Restore on mount with Contract time; refresh near expiry, keeping a still-valid session offline. */
  restore(now: number): Promise<Session | null> {
    return this.authWork(async () => {
      const auth = this.auth(); sessionFrom(null, now); auth.now = Math.floor(now);
      const session = this.session;
      if (session && session.expiresAt - now <= 60_000) await this.refreshAuth(now);
      return this.session;
    });
  }
  refreshSession(now: number): Promise<Refreshed> { return this.authWork(() => this.refreshAuth(now)); }
  private async refreshAuth(now: number): Promise<Refreshed> {
    const auth = this.auth(); sessionFrom(null, now); auth.now = Math.floor(now);
    const session = this.session;
    if (!session) return {ok: false, denied: {code: 'E_AUTH', message: 'Sign in again.'}};
    const raw = auth.store.get(auth.key);
    const scope = this.authKey();
    rotations.set(scope, raw);
    try {
      const reply = await this.authRequest('/auth/refresh');
      this.unchanged(raw);
      const renewed = sessionFrom(JSON.stringify(reply.body.session ?? null), now);
      if (reply.ok && renewed && renewed.principal === session.principal) {
        auth.store.set(auth.key, JSON.stringify(renewed));
        return {ok: true, session: renewed};
      }
      const denied = reply.body.denied ?? {code: 'E_SERVER', message: 'Invalid session refresh reply.'};
      if (denied.code === 'E_AUTH') this.invalidate(session.token);
      return {ok: false, denied, ...(denied.code === 'E_OFFLINE' ? {offline: true} : {})};
    } finally { if (rotations.get(scope) === raw) rotations.delete(scope); }
  }
  /** Always forget locally, even offline. ok confirms server revocation. */
  logout(): Promise<{ok: boolean; offline?: boolean; denied?: Refusal}> {
    return this.authWork(async () => {
      const auth = this.auth();
      if (!this.session) { auth.store.forget(auth.key); return {ok: true}; }
      const raw = auth.store.get(auth.key);
      let reply;
      try { reply = await this.authRequest('/auth/logout'); }
      finally { this.unchanged(raw); auth.store.forget(auth.key); }
      return {ok: reply.ok, ...(reply.body.denied ? {denied: reply.body.denied} : {}), ...(reply.body.denied?.code === 'E_OFFLINE' ? {offline: true} : {})};
    });
  }
  private async exchange(path: string, args: unknown): Promise<Record<string, unknown>> {
    const check = this.fence(), token = this.session?.token;
    const headers = { ...this.headers(), 'content-type': 'application/json' };
    const releaseKey = this.releases.identity(this.headers());
    let response: Response;
    try { response = await fetch(this.options.origin.replace(/\/+$/, '') + path, { method: 'POST', headers, body: JSON.stringify({ args }) }); }
    catch (error) { const why = unreached(error); throw refused({...why, message: `${why.message}. A write may already have committed; read its state before retrying.`}); }
    const body = await response.json().catch(() => null) as Record<string, unknown> | null;
    check();
    this.releases.observe(response, releaseKey);
    if ((body?.denied as Refusal | undefined)?.code === 'E_AUTH' || (body?.why as Refusal | undefined)?.code === 'E_AUTH') await this.rejectSession(token, check);
    if (!body || typeof body !== 'object' || Array.isArray(body)) throw refused({ code: 'E_SERVER', message: `Invalid reply (HTTP ${response.status}); read state before retrying a write.` });
    if (!response.ok && !body.denied && !(body.state === 'failed' && body.why)) throw refused({ code: 'E_SERVER', message: `HTTP ${response.status}; read state before retrying a write.` });
    return body;
  }
  async read<O extends string | Op>(op: O, args: NoInfer<ArgsOf<O>>): Promise<Read<ResultOf<O>>> {
    const body = await this.exchange('/q/' + encodeURIComponent(nameOf(op)), args);
    if (!Object.hasOwn(body, 'data') && !body.denied) throw refused({code: 'E_SERVER', message: 'Invalid query envelope'});
    if (body.denied) {
      if (!isRefusal(body.denied)) throw refused({code: 'E_SERVER', message: 'Invalid query refusal'});
      return {server: true, denied: body.denied};
    }
    return { ...body, server: true } as Read<ResultOf<O>>;
  }
  /** Read every page; a refusal, invalid row page, repeated cursor or limit overflow throws. */
  async readAll<O extends string | Op<unknown, readonly unknown[]>>(op: O, args: NoInfer<ArgsOf<O>>, cursor = 'c', limit = Infinity): Promise<Rows<ResultOf<O> extends readonly (infer E)[] ? E : unknown>> {
    if (!cursor || limit < 0 || (limit !== Infinity && !Number.isInteger(limit))) throw refused({code: 'E_CLIENT', message: 'readAll needs a cursor argument name and a non-negative integer limit (or Infinity).'});
    type E = ResultOf<O> extends readonly (infer T)[] ? T : unknown;
    const check = this.fence();
    const rows = Object.assign([] as E[], {complete: true, cut: null as ReadCut, server: true as const});
    const seen = new Set<string>();
    let next: Cursor | null = null;
    do {
      check();
      const page: Read<unknown> = await this.read(nameOf(op), next === null ? args as Record<string, unknown> : {...args as Record<string, unknown>, [cursor]: next});
      if (page.denied) throw refused(page.denied);
      check();
      if (!Array.isArray(page.data)) throw refused({code: 'E_SERVER', message: `${nameOf(op)}: readAll needs a row list.`});
      if (page.complete !== true) rows.complete = false;
      if (page.cut) rows.cut = page.cut;
      rows.push(...page.data as E[]);
      if (rows.length > limit) throw refused({code: 'E_CLIENT', message: `${nameOf(op)}: more than ${limit} rows.`});
      next = page.next ?? null;
      if (next !== null) {
        if (typeof next !== 'string' || !next || seen.has(next)) throw refused({code: 'E_SERVER', message: `${nameOf(op)}: its pages do not end.`});
        seen.add(next);
      }
    } while (next !== null);
    return rows;
  }
  async send<O extends string | Op>(op: O, args: NoInfer<ArgsOf<O>>): Promise<Sent<ResultOf<O>>> {
    const body = await this.exchange('/m/' + encodeURIComponent(nameOf(op)), args);
    const why = body.state === 'failed' ? body.why : body.denied;
    if (isRefusal(why)) return { state: 'failed', why: why as Refusal };
    if (body.state !== 'sent' || !Object.hasOwn(body, 'result')) throw refused({ code: 'E_SERVER', message: 'Invalid mutation envelope; read state before retrying.' });
    return { state: 'sent', result: body.result as ResultOf<O> };
  }
  async upload(file: string | ArrayBuffer | ArrayBufferView, options: UploadOptions = {}): Promise<Uploaded> {
    const check = this.fence(), token = this.session?.token;
    const headers = { ...this.headers(), 'content-type': options.file?.type ?? 'application/octet-stream' };
    const query = options.file ? `?as=file&name=${encodeURIComponent(options.file.name)}` : '';
    const init: RequestInit & { exactBodyFrom?: string } = typeof file === 'string'
      ? { method: 'POST', headers, exactBodyFrom: file }
      // A copy: the bytes sent are these, whatever the caller does with its buffer meanwhile.
      : { method: 'POST', headers, body: Uint8Array.from(ArrayBuffer.isView(file) ? new Uint8Array(file.buffer, file.byteOffset, file.byteLength) : new Uint8Array(file)) };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets${query}`, init); } catch (error) { return { ok: false, why: unreached(error) }; }
    const body = await response.json().catch(() => null) as { asset?: Asset; denied?: Refusal } | null;
    check();
    if (body?.denied?.code === 'E_AUTH') await this.rejectSession(token, check);
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
    const check = this.fence(), token = this.session?.token;
    const fs = this.options.storage?.fs;
    if (!fs) return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: 'download writes an app file: pass the dispatcher\'s `storage`, and grant fs.write for the path' } };
    let response: Response;
    try { response = await fetch(`${this.options.origin.replace(/\/+$/, '')}/assets/${encodeURIComponent(id)}`, { headers: this.headers() }); } catch (error) { return { ok: false, why: unreached(error) }; }
    check();
    if (!response.ok) {
      const body = await response.json().catch(() => null) as { denied?: Refusal } | null;
      check();
      if (body?.denied?.code === 'E_AUTH') await this.rejectSession(token, check);
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
    const file = path ?? `app:/cache/snapback4-${encodeURIComponent(this.options.name ?? 'online')}-${encodeURIComponent(id)}${filename ? `-${filename}` : `.${EXTENSIONS[type] ?? 'bin'}`}`;
    try { await fs.writeFile(file, bytes); } catch (error) { return { ok: false, why: { code: 'E_CLIENT', family: 'client', message: `could not write ${file}: ${error instanceof Error ? error.message : String(error)}` } }; }
    check();
    return { ok: true, path: file, type, bytes: bytes.byteLength, ...(name === undefined ? {} : { name }) };
  }

}

export function refused(why: Refusal): Error {
  const error = Object.assign(new Error(why.message), { code: why.code, reason: why.code, refusal: why });
  if (why.code === 'E_OFFLINE') Object.defineProperties(error, {name: {value: 'FetchError', configurable: true}, kind: {value: 'Network', configurable: true, enumerable: true}});
  return error;
}
