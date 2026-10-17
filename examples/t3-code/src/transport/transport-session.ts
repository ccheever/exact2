// One authenticated connection to a T3 server (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// packages/client-runtime/src/rpc/client.ts same-session resubscribe, connection/supervisor.ts,
// driver.ts, resolver.ts, authorization/remote.ts). The connect flow is `/.well-known/t3/environment`,
// `/oauth/token` for a pairing, `/api/auth/session`, `/api/auth/websocket-ticket`, then `wsTicket` on
// the socket's URL.
//
// Exact drives it, since a data module has no timers and sends only from a mutation or its socket's
// mapper (LLP 1016.000 D7): an answer runs the HTTP steps (`connectFlow`), a stream resource owns the
// socket and hands each event to `socketEvent`, and the transport's tick (`tick`, `flush`) sends what
// the outbox holds, checks deadlines and pings, and starts the next attempt when its backoff is over.
// Replies that resume an answer are handed to `hooks.deliver`, which a separate answer runs, so the
// awaiting answer continues inside an answer of its own (exact2's prelude keeps a fetch made after a
// mapper-resolved await, and only then).
import { Inbox } from './transport-inbox';
import { RouteState, blocks, json, type Http, type HttpReply } from './transport-routes';
import { credentialOrigins, type Catalog, type Route } from './transport-catalog';
import {
  TransportFailure, MAXIMUM_BYTES, PING_INTERVAL_MS, PONG_TIMEOUT_MS, QUICK_PROBE_MS, FOREGROUND_PROBE_MS, SOCKET_OPEN_TIMEOUT_MS,
  BACKOFF_RESET_AFTER_MS, compatibility, credential as parseCredential, decode, encode, exchangeForm, failure as wireFailure, failureMessage,
  identifier, origin as parseOrigin, path as onOrigin, request as wireRequest, retriesStream, retryDelayMs, streamRetryMs, trimmed, utf8Length,
  type Frame,
} from './transport-wire';
import type { ActivityReporter } from './transport-activity';

export interface SocketHandle { readonly readyState: number; readonly bufferedAmount: number; send(data: string): void; close(code?: number, reason?: string): void }
export interface SocketEvent { type: string; data?: unknown; code?: number; reason?: string; kind?: string; message?: string; status?: number }
export type Reply = { ok: true; generation: number; value: unknown } | { ok: false; generation: number; error: ReturnType<TransportFailure['json']> };
type Done = (reply: Reply) => void;
type Obj = Record<string, unknown>;

export interface Hooks {
  catalog: Catalog;
  http: Http;
  /** The transport's clock: the last time a tick or an answer that knows the time said. */
  now(): number;
  wall(now: number): string;
  random(): number;
  uuid(): string;
  /** The embedded server's bearer (the module holds it in memory). */
  primaryBearer(): Promise<string>;
  activity: ActivityReporter | null;
  /** The status (or, for a background environment, the fleet) changed; `events` when the inbox has news for its reader. */
  changed(what: 'status' | 'events'): void;
  /** Runs `work` in the answer that delivers replies. */
  deliver(work: () => void): void;
}

const str = (value: unknown) => typeof value === 'string' ? value : '';
const obj = (value: unknown): Obj => typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Obj : {};

interface Call { done: Done; joiners: Done[]; deadline: number; trace: number | null; joinedTraces: number[] }

/** Ops whose `generation` must still be the session's. */
const GENERATIONAL = new Set(['http', 'request', 'subscribe', 'unsubscribe', 'events', 'ack', 'readChunk', 'releaseChunk']);
const TERMINAL_KINDS = ['Authentication', 'Credential', 'Protocol', 'Keychain', 'Address', 'Limit'];

export class Session {
  generation = 0;
  private serial = 0;
  origin = '';
  token = '';
  primary = false;
  private restoredFocus = '';
  private privateValues: string[] = [];
  private exchangeScope = '';
  descriptor: Obj = {};
  state = 'disconnected';
  message = 'Connect to your T3 server.';
  alive = true;
  wantsConnection = false;
  /** Consecutive failed attempts (retryDelayMs's ladder); reset only by a stable connection or a retry. */
  failures = 0;
  private connectedAt: number | null = null;
  private everConnected = false;
  private failureKind = '';
  private failureTrace = '';
  private lastHTTPTrace = '';
  private walkExhausted = false;
  readonly routes: RouteState;
  readonly inbox = new Inbox();
  private pending = new Map<string, Call>();
  private sharedReads = new Map<string, { id: string; cwd: string | null }>();
  private pendingDiffs = new Map<string, Done[]>();
  private streams = new Map<string, string>(); // request id -> the app's subscription key
  private activityLeases = new Map<string, object>();
  private streamRetries = new Map<string, number>();
  private failedStreams = new Map<string, string>();
  private retryDue = new Map<string, { id: string; at: number }>();
  private outbox: string[] = [];
  private outboxBytes = 0;
  private probing: { id: string; deadline: number; waiters: Done[] } | null = null;
  private probeUnanswered = false;
  private lastPong = 0;
  private nextPing = 0;
  private opening: Done | null = null;
  /** When the next attempt starts (ms), while the ladder waits. */
  reconnectAt: number | null = null;
  /** The socket's URL for the current attempt, once the connect flow has a ticket; '' while none. */
  url = '';
  /** Bumps whenever the socket the session wants changes (a new attempt, or none). */
  attempt = 0;
  /** The attempt a connect flow should run for; another attempt's flow stops. */
  private connectAttempt = -1;
  private pendingCredential = '';
  private walkFresh = false;
  private openDeadline = 0;
  /** Better-route checks asked for while connected (`check`), and the last one run. */
  check = 0;
  private checked = 0;
  private socket: SocketHandle | null = null;
  private abort: AbortController | null = null;
  private readonly lease = {};

  constructor(readonly key: string, private readonly hooks: Hooks, readonly remembersOrigin: boolean) {
    this.routes = new RouteState(hooks.http);
  }

  private nextID(): string { this.serial += 1; return `${this.generation}-${this.serial}`; }
  private ok(value: unknown): Reply { return { ok: true, generation: this.generation, value: value ?? null }; }
  private bad(failure: TransportFailure): Reply { return { ok: false, generation: this.generation, error: this.failure(failure).json() }; }
  private argument(text: string) { return new TransportFailure('Arguments', text); }
  private deliver(done: Done, reply: Reply) { this.hooks.deliver(() => done(reply)); }
  private finish(call: Call, reply: (generation: number) => Reply) {
    this.deliver(call.done, reply(this.generation));
    for (const joiner of call.joiners) this.deliver(joiner, reply(this.generation));
  }

  /** The bridge's ops for this connection; the reply as the native module gave it. */
  async perform(request: Obj): Promise<Reply> {
    if (!this.alive) return this.bad(new TransportFailure('Closed', 'The window was closed.'));
    try {
      const op = str(request.op);
      if (GENERATIONAL.has(op) && typeof request.generation === 'number' && request.generation !== this.generation) {
        throw new TransportFailure('stale', 'The connection changed before this operation was sent.');
      }
      switch (op) {
        case 'status': this.restoreOrigin(); return this.ok(this.status());
        case 'ids': {
          const count = typeof request.count === 'number' ? request.count : 1;
          if (!(count >= 1 && count <= 16)) throw this.argument('ids requires a count from 1 to 16.');
          return this.ok(Array.from({ length: count }, () => this.hooks.uuid()));
        }
        case 'connect': return await new Promise<Reply>(done => this.connect(request, done));
        case 'retry': return await new Promise<Reply>(done => this.retryNow(request, done));
        case 'disconnect': return this.disconnect(request);
        case 'http': return await this.readHTTP(request);
        case 'uploadAttachment': return await this.uploadAttachment(request);
        case 'request': return await new Promise<Reply>(done => this.rpc(request, done));
        case 'subscribe': return this.subscribe(request);
        case 'unsubscribe': {
          const key = str(request.key);
          if (!key) throw this.argument('unsubscribe requires a key.');
          this.unsubscribe(key); return this.ok({});
        }
        case 'events': return this.ok(this.inbox.read(Math.max(0, typeof request.after === 'number' ? request.after : 0)));
        case 'ack':
          this.inbox.acknowledge(typeof request.through === 'number' ? request.through : 0);
          // The reader must also drain events that arrived after its last page but before this ACK.
          return this.ok({ latest: this.inbox.latest });
        case 'readChunk': case 'releaseChunk': throw new TransportFailure('stale', 'The pending server response was released. Retry the read.');
        case 'environments': return this.ok({ saved: this.hooks.catalog.all });
        case 'connectionPreferences': return this.ok({ text: this.hooks.catalog.preferences });
        case 'setConnectionPreferences': {
          if (typeof request.text !== 'string') throw this.argument('setConnectionPreferences requires text.');
          this.hooks.catalog.setPreferences(request.text); return this.ok({ text: request.text });
        }
        case 'setEnvironmentEnabled': {
          const environment = str(request.environmentId);
          if (typeof request.origin !== 'string' || !environment || typeof request.enabled !== 'boolean') throw this.argument('setEnvironmentEnabled requires origin, environmentId and enabled.');
          parseOrigin(request.origin);
          if (!this.hooks.catalog.setEnabled(environment, request.enabled)) throw new TransportFailure('Missing', 'That environment is no longer saved on this device.');
          return this.ok({ saved: this.hooks.catalog.all });
        }
        case 'forgetEnvironment': return this.forgetEnvironment(request);
        case 'pairEnvironment': return await this.pairEnvironment(request);
        case 'setRoutes': return this.setRoutes(request);
        case 'prDiff': return await this.pullRequestDiff(request);
        default: throw this.argument('Unknown native operation.');
      }
    } catch (error) { return this.bad(this.failure(error)); }
  }

  destroy(): void {
    if (!this.alive) return;
    this.alive = false; this.wantsConnection = false; this.reconnectAt = null;
    this.retire(new TransportFailure('Closed', 'The window was closed.', true));
    this.token = ''; this.privateValues = []; this.inbox.reset();
  }

  // MARK: Status

  /** The focus a launch restores: `primary` from the catalog, else the remembered saved environment. */
  private restoreOrigin(): void {
    if (this.origin || !this.remembersOrigin) return;
    const catalog = this.hooks.catalog;
    this.restoredFocus = catalog.focus === 'primary' ? 'primary' : '';
    if (this.restoredFocus || !catalog.origin) return;
    // The key only ever names a saved environment; one no saved environment has is dropped, not restored.
    let saved = '';
    try { saved = parseOrigin(catalog.origin); } catch { saved = ''; }
    if (!saved || !catalog.entryForOrigin(saved)) { catalog.rememberOrigin(''); return; }
    this.origin = saved;
  }

  status(): Obj {
    const traces: number[] = [];
    for (const call of this.pending.values()) { if (call.trace !== null) traces.push(call.trace); traces.push(...call.joinedTraces); }
    return { state: this.state, origin: this.origin, environmentId: str(this.descriptor.environmentId), message: this.message, descriptor: this.descriptor,
      failureKind: this.failureKind, traceId: this.failureTrace, traces, activeRouteId: this.primary ? '' : this.routes.activeId,
      homeOrigin: this.primary ? '' : this.routes.home, primary: this.primary, focus: this.primary ? 'primary' : this.origin ? '' : this.restoredFocus };
  }

  private setStatus(next: string, text: string): void {
    const cleaned = this.clean(text);
    if (next === this.state && cleaned === this.message) return;
    this.state = next; this.message = cleaned;
    this.hooks.activity?.changed();
    const environment = str(this.descriptor.environmentId);
    if (next === 'connected' && environment) {
      const epoch = this.generation;
      this.hooks.activity?.connect(this.lease, environment, (payload, finished) => {
        if (!this.alive || this.state !== 'connected' || this.generation !== epoch) return finished();
        try { this.rpc({ method: 'server.reportClientActivity', payload }, () => finished()); } catch { finished(); }
      });
    }
    if (this.alive) this.hooks.changed('status');
  }

  clean(input: string): string {
    let result = input;
    for (const secret of [...this.privateValues, ...(this.token ? [this.token] : [])]) if (secret) result = result.split(secret).join('[redacted]');
    result = result.replace(/(wsTicket|token|access_token)=([^&\s]+)/gi, '$1=[redacted]');
    return result.slice(0, 700);
  }
  failure(error: unknown): TransportFailure {
    if (error instanceof TransportFailure) return new TransportFailure(error.kind, this.clean(error.message), error.uncertain, error.reason, this.clean(error.detail));
    return new TransportFailure('Network', this.clean(error instanceof Error ? error.message : String(error)));
  }

  // MARK: Connecting

  private connect(request: Obj, done: Done): void {
    let next: string, credential: string;
    try {
      next = parseOrigin(str(request.origin));
      const toPrimary = request.primary === true;
      credential = toPrimary ? '' : parseCredential(str(request.credential), next);
      this.reconnectAt = null; this.wantsConnection = false;
      this.retire(new TransportFailure('Replaced', 'The connection was replaced.', true));
      this.origin = next; this.descriptor = {}; this.token = ''; this.primary = toPrimary;
    } catch (error) { return done(this.bad(this.failure(error))); }
    this.failureKind = ''; this.failureTrace = ''; this.lastHTTPTrace = '';
    this.privateValues = credential ? [credential] : []; this.exchangeScope = str(request.scope);
    // The primary walks no saved routes: one loopback address, its bearer from the embedded server.
    if (this.primary) this.routes.stop(); else this.routes.load(next, this.hooks.catalog);
    this.wantsConnection = true; this.failures = 0; this.everConnected = false; this.opening = done;
    // The origin is remembered once its socket opens; a failed pairing leaves nothing behind.
    this.start(credential);
  }

  /** A new attempt: the next connect flow runs for it. */
  start(credential = ''): void {
    if (!this.wantsConnection || !this.alive || !this.origin) return;
    this.generation += 1; this.inbox.reset();
    this.setStatus(this.failures === 0 && !this.everConnected ? 'connecting' : 'reconnecting', `Connecting to ${hostOf(this.origin) || 'the server'}…`);
    this.pendingCredential = credential;
    // A pairing tries its one address, and so does the primary; a saved environment walks its routes (connectOverRoutes).
    this.walkFresh = !credential && !this.primary;
    this.url = ''; this.attempt += 1; this.connectAttempt = this.attempt;
  }

  /** Whether the connect flow for `attempt` should run (an answer asks before it fetches). */
  wantsFlow(attempt: number): boolean { return this.alive && this.wantsConnection && attempt === this.connectAttempt && !this.url && this.state !== 'connected'; }

  /**
   * The HTTP steps of one attempt, in the answer the transport asks for it: the route walk, the descriptor, the
   * token, the session check and the socket ticket. Resolves with the socket's URL, or '' when the attempt failed
   * or another replaced it (the failure is the session's).
   */
  async connectFlow(attempt: number): Promise<string> {
    if (!this.wantsFlow(attempt)) return '';
    const credential = this.pendingCredential;
    const walking = !credential && !this.primary;
    if (this.walkFresh) {
      this.walkFresh = false;
      if (!this.routes.routes.length) this.routes.load(this.origin, this.hooks.catalog);
      this.routes.beginWalk(this.hooks.catalog);
    }
    for (;;) {
      const epoch = this.generation;
      this.abort = new AbortController();
      if (walking) {
        const route = await this.routes.next();
        if (attempt !== this.connectAttempt || epoch !== this.generation) return '';
        if (!route) { this.walkExhausted = true; this.connectionFailed(this.routes.exhausted(), epoch); return ''; }
        try { this.origin = parseOrigin(route.origin); }
        catch { this.routes.record(new TransportFailure('Address', `${route.origin} is not an address this client sends credentials to.`)); continue; }
      }
      try {
        const url = await this.startRoute(credential, epoch);
        if (attempt !== this.connectAttempt || epoch !== this.generation) return '';
        this.url = url; this.openDeadline = this.hooks.now() + SOCKET_OPEN_TIMEOUT_MS;
        return url;
      } catch (error) {
        if (attempt !== this.connectAttempt || epoch !== this.generation) return '';
        const problem = this.failure(error);
        // connectOverRoutes: a route that fails moves on to the next; an incompatible server is the same on every route.
        if (this.routes.walksSeveral && problem.kind !== 'Protocol') {
          this.routes.record(problem); this.lastHTTPTrace = '';
          this.retireAttempt(false);
          continue;
        }
        this.connectionFailed(problem, epoch);
        return '';
      }
    }
  }

  private async startRoute(credential: string, epoch: number): Promise<string> {
    const descriptor = obj(await this.http('/.well-known/t3/environment', { epoch, authorized: false }));
    if (typeof descriptor.environmentId !== 'string') throw new TransportFailure('Protocol', 'The server did not identify its environment.');
    // Never send a credential to a route before its descriptor matched the saved environment.
    if (!credential && !this.primary && this.routes.environmentId && descriptor.environmentId !== this.routes.environmentId) {
      throw new TransportFailure('Network', `${hostOf(this.origin) || 'That address'} answered as a different environment.`);
    }
    const problem = compatibility(descriptor);
    if (problem) {
      // Keep the descriptor: the Environments row offers Update when the server can update itself.
      this.descriptor = descriptor;
      throw new TransportFailure('Protocol', problem.message);
    }
    const previousEnvironment = str(this.descriptor.environmentId);
    this.descriptor = descriptor;
    const environment = descriptor.environmentId;
    if (this.primary) {
      // The embedded server's bearer (memory only); none yet while it starts.
      this.token = await this.hooks.primaryBearer();
      if (!this.token) throw new TransportFailure('Starting', 'The local server is starting.');
    } else if (!credential) {
      // Each route reads the token it owns or borrows (a learned route uses its source route's).
      const owner = this.routes.current?.credential ?? this.origin;
      if (!this.token || previousEnvironment !== environment || this.routes.tokenOwner !== owner) {
        this.token = this.hooks.catalog.token(owner, environment);
        this.routes.tokenOwner = owner;
      }
      if (!this.token) throw new TransportFailure('Credential', 'Paste a pairing link or token from this T3 server.');
    } else {
      const grant = obj(await this.http('/oauth/token', { epoch, method: 'POST', body: exchangeForm(credential, this.exchangeScope), contentType: 'application/x-www-form-urlencoded', authorized: false }));
      if (grant.token_type !== 'Bearer' || !str(grant.access_token)) throw new TransportFailure('Protocol', 'The server did not issue a bearer access token.');
      this.token = str(grant.access_token); this.routes.tokenOwner = trimmed(this.origin);
      this.hooks.catalog.saveToken(this.token, this.origin, environment);
      this.privateValues = [];
    }
    const auth = obj(await this.http('/api/auth/session', { epoch }));
    if (auth.authenticated !== true) throw new TransportFailure('Authentication', 'The saved session expired. Pair with the server again.');
    const ticket = str(obj(await this.http('/api/auth/websocket-ticket', { epoch, method: 'POST' })).ticket);
    if (!ticket) throw new TransportFailure('Protocol', 'The server did not issue a socket ticket.');
    this.privateValues = [ticket];
    return socketURL(this.origin, ticket, true);
  }

  /** One request on the connected origin; the JSON reply, or the failure the reference reports. */
  async http(target: string, options: { epoch: number; method?: string; body?: string | Uint8Array; contentType?: string; authorized?: boolean; long?: boolean }): Promise<unknown> {
    if (!this.origin) throw new TransportFailure('Disconnected', 'Connect to a server first.');
    const url = onOrigin(target, this.origin);
    const headers: Record<string, string> = { 'x-t3-orchestration-protocol': '2', Accept: 'application/json' };
    if (options.body !== undefined) headers['Content-Type'] = options.contentType ?? 'application/json';
    if (options.authorized !== false) headers.Authorization = `Bearer ${this.token}`;
    let reply: HttpReply;
    try { reply = await this.hooks.http(url, { method: options.method ?? 'GET', headers, body: options.body, timeoutMs: options.long ? 60_000 : 30_000, signal: this.abort?.signal }); }
    catch (error) {
      if (!this.alive || this.generation !== options.epoch) throw new TransportFailure('Replaced', 'The connection changed during the request.');
      throw new TransportFailure('Network', this.clean(error instanceof Error ? error.message : String(error)));
    }
    if (!this.alive || this.generation !== options.epoch) throw new TransportFailure('Replaced', 'The connection changed during the request.');
    if (utf8Length(reply.text) > MAXIMUM_BYTES) throw new TransportFailure('Limit', 'The server response is too large.');
    const decoded = json(reply);
    if (reply.status < 200 || reply.status >= 300) {
      const trace = str(obj(decoded).traceId);
      if (trace) this.lastHTTPTrace = trace.slice(0, 200);
      const kind = reply.status === 401 || reply.status === 403 ? 'Authentication' : reply.status === 426 ? 'Protocol' : 'HTTP';
      throw new TransportFailure(kind, this.clean(failureMessage(decoded, reply.status)));
    }
    if (decoded === undefined) throw new TransportFailure('Protocol', 'The server returned invalid JSON.');
    return decoded;
  }

  // MARK: The socket

  /** One event of the socket resource for `url`; stale sockets are ignored. Runs in the socket's mapper. */
  socketEvent(url: string, event: SocketEvent, handle: SocketHandle | undefined): void {
    if (!this.alive || !url || url !== this.url) return;
    const epoch = this.generation;
    if (event.type === 'open') { if (handle) this.opened(handle); return; }
    if (event.type === 'message') {
      try {
        const data = typeof event.data === 'string' ? event.data : new TextDecoder().decode(event.data as ArrayBuffer);
        for (const frame of decode(data)) { if (this.generation !== epoch) return; this.consume(frame, epoch, handle); }
      } catch (error) { this.connectionFailed(this.failure(error), epoch); }
      if (handle) this.flush(handle);
      return;
    }
    if (event.type === 'close') {
      this.connectionFailed(new TransportFailure('Network', `The server connection closed (${event.code ?? 1006}).`), epoch);
      return;
    }
    if (event.type === 'error') {
      const status = event.status ?? 0;
      if (status === 401 || status === 403) return this.connectionFailed(new TransportFailure('Authentication', 'The server refused the socket credential. Pair again.'), epoch);
      if (status === 426) return this.connectionFailed(new TransportFailure('Protocol', 'The server refused orchestration protocol 2.'), epoch);
      this.connectionFailed(new TransportFailure('Network', event.message || 'The server WebSocket did not open.'), epoch);
    }
  }

  private opened(handle: SocketHandle): void {
    const now = this.hooks.now();
    this.socket = handle;
    this.connectedAt = now; this.everConnected = true; this.failureKind = ''; this.failureTrace = ''; this.openDeadline = 0;
    this.setStatus('connected', `Connected to ${str(this.descriptor.label) || hostOf(this.origin) || 'T3'}.`);
    const catalog = this.hooks.catalog;
    try {
      if (this.primary) {
        // Never saved, never a remembered origin: the focus token names it (its port can change).
        if (this.remembersOrigin) catalog.rememberFocus('primary');
      } else if (this.origin) {
        catalog.remember(this.origin, this.descriptor, this.hooks.wall(now));
        this.routes.landed(this.origin, str(this.descriptor.environmentId), catalog, now);
        // The remembered origin names the saved environment (its home), whichever route is in use.
        if (this.remembersOrigin) { catalog.rememberFocus(''); catalog.rememberOrigin(this.routes.home || this.origin); }
        this.routes.schedule(now);
      }
    } catch { /* the catalog keeps what it had */ }
    this.lastPong = now; this.nextPing = now + PING_INTERVAL_MS;
    if (this.opening) { const waiter = this.opening; this.opening = null; this.deliver(waiter, this.ok(this.status())); }
    this.flush(handle);
  }

  private consume(frame: Frame, epoch: number, handle: SocketHandle | undefined): void {
    const tag = str(frame._tag);
    if (tag === 'Pong') { this.lastPong = this.hooks.now(); return; }
    if (tag === 'Ping') { this.reply(handle, { _tag: 'Pong' }); return; }
    if (tag === 'Defect' || tag === 'ClientProtocolError') throw wireFailure(frame);
    const id = identifier(frame.requestId);
    if (id === null) throw new TransportFailure('Protocol', 'The server RPC response has no request ID.');
    if (tag === 'Chunk') {
      if (!Array.isArray(frame.values)) throw new TransportFailure('Protocol', 'An RPC chunk has no values.');
      const key = this.streams.get(id);
      if (key !== undefined) {
        this.streamRetries.delete(key); // the first value resets the retry ladder
        let notify = false;
        for (const value of frame.values) notify = this.inbox.append(epoch, key, id, value) || notify;
        // Acknowledged once every value is retained (or overflow is marked); a late cancelled stream's too.
        this.reply(handle, { _tag: 'Ack', requestId: frame.requestId });
        if (notify) this.hooks.changed('events');
      } else this.reply(handle, { _tag: 'Ack', requestId: frame.requestId });
      return;
    }
    const exit = obj(frame.exit);
    if (tag !== 'Exit' || typeof frame.exit !== 'object') throw new TransportFailure('Protocol', 'The server sent an unsupported RPC response.');
    const success = exit._tag === 'Success';
    const call = this.pending.get(id);
    if (call) {
      this.pending.delete(id);
      if (success) this.finish(call, generation => ({ ok: true, generation, value: exit.value ?? null }));
      else { const problem = this.failure(wireFailure(exit)); this.finish(call, generation => ({ ok: false, generation, error: problem.json() })); }
      return;
    }
    const key = this.streams.get(id);
    if (key === undefined) return;
    this.streams.delete(id);
    this.releaseLease(id);
    const problem = success ? null : this.failure(wireFailure(exit));
    const value: Obj = problem ? { _transportError: problem.json() } : { _streamEnded: true };
    // An expected failure resubscribes on this session after 250 ms doubling to 30 s; an authorization failure waits for the next session.
    if (!problem || retriesStream(problem.kind)) {
      const retries = this.streamRetries.get(key) ?? 0, delay = streamRetryMs(retries);
      this.streamRetries.set(key, retries + 1); this.failedStreams.set(key, id);
      value.retryAfterMs = delay;
      this.retryDue.set(key, { id, at: this.hooks.now() + delay });
    }
    if (this.inbox.append(epoch, key, id, value)) this.hooks.changed('events');
  }

  /** A frame the mapper answers on its own socket now (an Ack, a Pong); queued if the handle refuses. */
  private reply(handle: SocketHandle | undefined, frame: Frame): void {
    const text = encode(frame);
    if (handle) { try { handle.send(text); return; } catch { /* the outbox takes it */ } }
    this.send(text);
  }

  /** Whether there is anything for the tick to send. */
  get hasOutbox(): boolean { return this.outbox.length > 0; }

  /** Sends what the outbox holds, within the bound one turn may stage (256 operations, 1 MiB). */
  flush(handle: SocketHandle | null = this.socket): void {
    if (!handle || handle !== this.socket || handle.readyState !== 1) return;
    let frames = 0, bytes = 0;
    while (this.outbox.length && frames < 200) {
      const text = this.outbox[0]!, size = utf8Length(text);
      if (frames > 0 && (bytes + size > 768 * 1024 || handle.bufferedAmount + size > 1024 * 1024)) break;
      try { handle.send(text); } catch { return; }
      this.outbox.shift(); this.outboxBytes -= size; frames += 1; bytes += size;
    }
  }

  private send(text: string): void {
    if (!this.url) return;
    if (this.outbox.length >= 4096 || this.outboxBytes + utf8Length(text) > MAXIMUM_BYTES) {
      return this.connectionFailed(new TransportFailure('Limit', 'The server is not consuming outgoing messages.'), this.generation);
    }
    this.outbox.push(text); this.outboxBytes += utf8Length(text);
  }
  private sendFrame(frame: Frame): void {
    try { this.send(encode(frame)); } catch (error) { this.connectionFailed(this.failure(error), this.generation); }
  }

  // MARK: Requests and streams

  /** One RPC. A shared read joins an identical one still pending (an answer asked again sends nothing new). */
  rpc(request: Obj, done: Done): void {
    try {
      if (this.state !== 'connected') throw new TransportFailure('Disconnected', 'The server is not connected.');
      const method = str(request.method);
      if (!method) throw this.argument('request requires a method.');
      const payload = request.payload ?? {};
      const shareKey = request.share === true ? method + '\n' + stableJSON(payload) : null;
      // A request that is not shared may write (a branch switch): no later read of its workspace joins one sent before it.
      const cwd = typeof obj(payload).cwd === 'string' ? str(obj(payload).cwd) : null;
      if (shareKey === null && cwd !== null) for (const [key, read] of [...this.sharedReads]) if (read.cwd === cwd) this.sharedReads.delete(key);
      const trace = typeof request.trace === 'number' ? request.trace : null;
      if (shareKey !== null) {
        const shared = this.sharedReads.get(shareKey);
        const joined = shared ? this.pending.get(shared.id) : undefined;
        if (joined) { joined.joiners.push(done); if (trace !== null) joined.joinedTraces.push(trace); return; }
      }
      const id = this.nextID(), text = encode(wireRequest(id, method, payload));
      // A request may ask for a shorter deadline; a delivered one waits up to five minutes, a provider update fifteen.
      const deliverKey = str(request.deliver);
      const wait = Math.min(deliverKey ? 300 : 900, Math.max(1, typeof request.timeout === 'number' ? request.timeout : 30));
      const deadline = this.hooks.now() + wait * 1000;
      if (deliverKey && shareKey === null) {
        // composer-replies.ts: the reply joins the inbox, so no answer waits on a long write.
        const epoch = this.generation;
        this.pending.set(id, { done: reply => {
          if (this.generation !== epoch) return;
          const value = reply.ok ? { _reply: reply.value } : { _replyError: reply.error };
          if (this.inbox.append(epoch, deliverKey, id, value)) this.hooks.changed('events');
        }, joiners: [], deadline, trace, joinedTraces: [] });
        this.send(text);
        return done(this.ok({ id }));
      }
      this.pending.set(id, { done, joiners: [], deadline, trace, joinedTraces: [] });
      if (shareKey !== null) {
        for (const [key, read] of [...this.sharedReads]) if (!this.pending.has(read.id)) this.sharedReads.delete(key);
        this.sharedReads.set(shareKey, { id, cwd });
      }
      this.send(text);
    } catch (error) { done(this.bad(this.failure(error))); }
  }

  private subscribe(request: Obj): Reply {
    if (this.state !== 'connected') throw new TransportFailure('Disconnected', 'The server is not connected.');
    const method = str(request.method), key = str(request.key);
    if (!method || !key) throw this.argument('subscribe requires a key and method.');
    if (this.streams.size >= 16 && ![...this.streams.values()].includes(key)) throw new TransportFailure('Busy', 'Too many subscriptions are open.');
    const id = this.nextID(), text = encode(wireRequest(id, method, request.payload ?? {}));
    this.unsubscribe(key);
    this.failedStreams.delete(key);
    this.streams.set(id, key);
    const lease = {};
    this.activityLeases.set(id, lease);
    this.hooks.activity?.retain(lease, str(this.descriptor.environmentId), method, obj(request.payload));
    this.send(text);
    return this.ok({ id });
  }
  private unsubscribe(key: string): void {
    for (const [id, value] of [...this.streams]) {
      if (value !== key) continue;
      this.streams.delete(id);
      this.releaseLease(id);
      this.sendFrame({ _tag: 'Interrupt', requestId: id });
    }
  }
  private releaseLease(id: string): void {
    const lease = this.activityLeases.get(id);
    if (lease) { this.activityLeases.delete(id); this.hooks.activity?.release(lease); }
  }

  // MARK: The tick

  /** Deadlines, the probe, the Ping cadence, stream retries, the socket's opening and the next attempt. */
  tick(now: number): void {
    if (!this.alive) return;
    if (this.state === 'connected') {
      for (const [id, call] of [...this.pending]) {
        if (call.deadline > now) continue;
        this.pending.delete(id);
        this.sendFrame({ _tag: 'Interrupt', requestId: id });
        const timeout = new TransportFailure('Timeout', 'The server did not confirm the request. Refresh before trying it again.', true);
        this.finish(call, generation => ({ ok: false, generation, error: timeout.json() }));
      }
      if (this.probing && now >= this.probing.deadline) {
        const label = str(this.descriptor.label) || hostOf(this.origin) || 'The server';
        this.connectionFailed(new TransportFailure('Network', `${label} did not respond to a connection health check.`), this.generation);
      } else if (now - this.lastPong > PONG_TIMEOUT_MS) {
        this.connectionFailed(new TransportFailure('Network', 'The server stopped responding.'), this.generation);
      } else if (now >= this.nextPing) {
        this.nextPing = now + PING_INTERVAL_MS;
        this.sendFrame({ _tag: 'Ping' });
      }
      for (const [key, due] of [...this.retryDue]) {
        if (due.at > now) continue;
        this.retryDue.delete(key);
        if (this.failedStreams.get(key) !== due.id) continue;
        this.failedStreams.delete(key);
        if (this.inbox.append(this.generation, key, due.id, { _retryDue: true })) this.hooks.changed('events');
      }
      if (this.routes.nextCheckAt && now >= this.routes.nextCheckAt) { this.routes.schedule(now); this.check += 1; }
    } else if (this.url && this.openDeadline && now >= this.openDeadline) {
      this.connectionFailed(new TransportFailure('Network', 'The server WebSocket did not open.'), this.generation);
    }
    if (this.reconnectAt !== null && now >= this.reconnectAt && this.wantsConnection) { this.reconnectAt = null; this.start(); }
  }

  /** Whether the tick should come back within a second (a backoff, a stream retry, an opening socket or a probe). */
  get soon(): boolean {
    const now = this.hooks.now();
    if (this.reconnectAt !== null && this.reconnectAt - now < 1_500) return true;
    if (this.probing) return true;
    for (const due of this.retryDue.values()) if (due.at - now < 1_500) return true;
    return false;
  }

  // MARK: Failure and the ladder

  /** Ends the socket and everything waiting on it. The socket resource lets go of the socket when its URL clears. */
  retire(reason: TransportFailure, dropSocket = true): void {
    this.hooks.activity?.disconnect(this.lease);
    for (const id of [...this.activityLeases.keys()]) this.releaseLease(id);
    this.connectedAt = null;
    const waiters = this.probing?.waiters ?? []; this.probing = null;
    const failed = (generation: number): Reply => ({ ok: false, generation, error: this.failure(reason).json() });
    for (const waiter of waiters) this.deliver(waiter, failed(this.generation));
    this.socket = null; this.url = ''; this.openDeadline = 0;
    if (dropSocket) this.attempt += 1;
    this.outbox = []; this.outboxBytes = 0;
    this.abort?.abort(); this.abort = null;
    const calls = [...this.pending.values()]; this.pending.clear(); this.sharedReads.clear();
    for (const call of calls) this.finish(call, failed);
    for (const waiters of this.pendingDiffs.values()) for (const waiter of waiters) this.deliver(waiter, failed(this.generation));
    this.pendingDiffs.clear();
    this.streams.clear(); this.streamRetries.clear(); this.failedStreams.clear(); this.retryDue.clear();
    if (this.opening) { const waiter = this.opening; this.opening = null; this.deliver(waiter, failed(this.generation)); }
  }

  /** A walk's route failed: the walk moves on in a fresh generation (a new attempt when its socket had a URL). */
  private retireAttempt(dropSocket = true): void {
    this.retire(new TransportFailure('Replaced', 'The connection moved to another route.', true), dropSocket);
    this.generation += 1; this.inbox.reset();
  }

  connectionFailed(error: TransportFailure, epoch: number): void {
    if (!this.alive || this.generation !== epoch || !this.wantsConnection || this.reconnectAt !== null) return;
    const problem = this.failure(error);
    // connectOverRoutes: a route that fails moves on to the next; an incompatible server is the same on every route.
    if (this.routes.walksSeveral && problem.kind !== 'Protocol') {
      this.routes.record(problem); this.lastHTTPTrace = '';
      this.retireAttempt(); this.connectAttempt = this.attempt;
      return;
    }
    const now = this.hooks.now();
    const exhausted = this.walkExhausted; this.walkExhausted = false;
    if (this.routes.walking) this.routes.exhausted();
    this.failureKind = problem.kind; this.failureTrace = this.lastHTTPTrace; this.lastHTTPTrace = '';
    // The ladder restarts only after the lost connection stayed up 30 s.
    if (this.connectedAt !== null && now - this.connectedAt >= BACKOFF_RESET_AFTER_MS) this.failures = 0;
    // A health check asked and the connection failed before answering: reconnect now.
    const failedProbe = this.probeUnanswered; this.probeUnanswered = false;
    const waiters = this.probing?.waiters ?? []; if (this.probing) this.probing.waiters = [];
    this.retire(new TransportFailure(problem.kind, problem.message, true));
    // A walk that ended reports a transient error over a blocked one, and only a blocked one stops the ladder.
    // The primary keeps its ladder through its own server's restarts; only an incompatible server stops it.
    const terminal = this.primary ? problem.kind === 'Protocol' : exhausted ? blocks(problem) : TERMINAL_KINDS.includes(problem.kind) || !this.token;
    if (terminal) {
      this.wantsConnection = false; this.routes.stop();
      const environment = str(this.descriptor.environmentId);
      if (problem.kind === 'Authentication' && !this.primary && this.origin && environment) {
        try { this.hooks.catalog.forgetToken(this.routes.current?.credential ?? this.origin, environment); } catch { /* kept */ }
        this.token = '';
      }
      this.setStatus('error', problem.message);
      for (const waiter of waiters) this.deliver(waiter, this.ok(this.status()));
      return;
    }
    let delay: number;
    if (failedProbe) {
      // Only this first attempt skips the ladder; if it fails too, backoff resumes from the start.
      this.failures = 0; delay = 0;
      this.setStatus('reconnecting', `${problem.message} Reconnecting in 0s…`);
    } else {
      this.failures += 1;
      delay = retryDelayMs(this.failures - 1, this.hooks.random());
      this.setStatus('reconnecting', `${problem.message} Reconnecting in ${Math.round(delay / 1000)}s…`);
    }
    this.reconnectAt = now + delay;
    for (const waiter of waiters) this.deliver(waiter, this.ok(this.status()));
  }

  /** The connection buttons' Retry (supervisor retryNow): a live socket is probed, not replaced; a waiting retry runs now. */
  private retryNow(request: Obj, done: Done): void {
    try {
      const raw = str(request.origin);
      if (raw && this.origin && parseOrigin(raw) !== this.origin && !this.routes.routes.some(route => route.origin === trimmed(raw))) return this.connect(request, done);
      if (!this.origin) throw new TransportFailure('Disconnected', 'Connect to a server first.');
    } catch (error) { return done(this.bad(this.failure(error))); }
    if (this.wantsConnection && this.state === 'connected' && this.socket) return this.probe(QUICK_PROBE_MS, done);
    if (this.wantsConnection && this.reconnectAt !== null) {
      this.reconnectAt = null; this.failures = 0; this.opening = done; this.start(); return;
    }
    // An attempt in flight restarts; a stopped connection starts again from saved credentials.
    this.retire(new TransportFailure('Replaced', 'The connection was retried.', true));
    this.failureKind = ''; this.failureTrace = '';
    this.wantsConnection = true; this.failures = 0; this.opening = done;
    this.start();
  }

  private disconnect(request: Obj): Reply {
    this.wantsConnection = false; this.reconnectAt = null; this.routes.stop();
    this.retire(new TransportFailure('Disconnected', 'Disconnected from the server.', true));
    this.generation += 1; this.inbox.reset(); this.token = '';
    const environment = str(this.descriptor.environmentId);
    if (request.forget === true && !this.primary && this.origin && environment) {
      const gone = this.hooks.catalog.forget(environment);
      for (const owner of [...credentialOrigins(gone), this.origin]) this.hooks.catalog.forgetToken(owner, environment);
      let home = this.origin;
      try { home = parseOrigin(str(gone?.origin)); } catch { home = this.origin; }
      this.forgotten(home, true);
    }
    // A pairing that failed with nothing to go back to leaves no origin behind.
    if (request.abandon === true) { this.origin = ''; this.descriptor = {}; this.primary = false; }
    this.setStatus('disconnected', 'Disconnected.');
    return this.ok(this.status());
  }

  /**
   * An environment was forgotten on this device: nothing may name it afterwards, not this session's origin when it
   * was the focus and not the remembered origin. The remembered origin is cleared, not repointed, as the reference's
   * Remove keeps no "current" environment it could move to another.
   */
  private forgotten(gone: string, wasFocused: boolean): void {
    if (wasFocused) { this.origin = ''; this.descriptor = {}; this.failureKind = ''; this.failureTrace = ''; this.lastHTTPTrace = ''; }
    const catalog = this.hooks.catalog;
    if (!this.remembersOrigin || catalog.entryForOrigin(gone)) return;
    let saved = '';
    try { saved = parseOrigin(catalog.origin); } catch { saved = ''; }
    if (saved === gone) catalog.rememberOrigin('');
  }

  /** Returning to the app: probe a live socket within 15 s and look for a better route, or wake a waiting retry. */
  foreground(): void {
    if (!this.alive || !this.wantsConnection) return;
    if (this.state === 'connected' && this.socket) {
      this.probe(FOREGROUND_PROBE_MS, null);
      this.lookForBetterRoute();
      // The shell and thread streams resubscribe too, since a suspended app may have missed events.
      let notify = false;
      for (const [id, key] of this.streams) if (key === 'shell' || key === 'thread') notify = this.inbox.append(this.generation, key, id, { _retryDue: true }) || notify;
      if (notify) this.hooks.changed('events');
    } else if (this.reconnectAt !== null) { this.reconnectAt = null; this.failures = 0; this.start(); }
  }

  /** A reachability change: offline questions a live socket (3 s); back online wakes a waiting retry. */
  network(online: boolean): void {
    if (online) this.hooks.activity?.changed();
    if (!this.alive || !this.wantsConnection) return;
    if (!online) { if (this.state === 'connected' && this.socket) this.probe(QUICK_PROBE_MS, null); }
    else if (this.state === 'connected' && this.socket) this.lookForBetterRoute();
    else if (this.reconnectAt !== null) { this.reconnectAt = null; this.start(); }
  }

  /** server.probe (or server.getConfig before connectionProbe): a later quicker signal shortens a running probe. */
  private probe(timeout: number, waiter: Done | null): void {
    const deadline = this.hooks.now() + timeout;
    if (this.probing) {
      if (deadline < this.probing.deadline) this.probing.deadline = deadline;
      if (waiter) this.probing.waiters.push(waiter);
      return;
    }
    const capable = obj(this.descriptor.capabilities).connectionProbe === true;
    const id = this.nextID(), epoch = this.generation;
    this.probing = { id, deadline, waiters: waiter ? [waiter] : [] };
    this.probeUnanswered = true;
    this.pending.set(id, { done: reply => {
      const current = this.probing;
      if (!current || current.id !== id || this.generation !== epoch) return;
      if (reply.ok) {
        this.probing = null; this.probeUnanswered = false; this.lastPong = this.hooks.now();
        for (const waiting of current.waiters) this.deliver(waiting, this.ok(this.status()));
      } else this.connectionFailed(new TransportFailure('Network', reply.error.message || 'The connection health check failed.'), epoch);
    }, joiners: [], deadline: deadline + 60_000, trace: null, joinedTraces: [] });
    this.sendFrame(wireRequest(id, capable ? 'server.probe' : 'server.getConfig', {}));
  }

  /** A better-route check: asked for here, run by the connect answer (it fetches). */
  lookForBetterRoute(): void {
    if (this.alive && this.wantsConnection && this.state === 'connected' && this.socket && this.routes.activeIndex > 0) this.check += 1;
  }
  /** The check the connect answer runs for `check`; a better route that answers replaces the session. */
  async checkRoutes(check: number): Promise<void> {
    if (check <= this.checked || this.state !== 'connected') return;
    this.checked = check;
    const best = await this.routes.checkBetter(this.hooks.catalog, this.hooks.now());
    if (!best || !this.alive || !this.wantsConnection || this.state !== 'connected') return;
    this.retire(new TransportFailure('Replaced', 'The connection moved to a better route.', true));
    this.failures = 0; this.start();
  }

  // MARK: HTTP ops

  private async readHTTP(request: Obj): Promise<Reply> {
    if (this.state !== 'connected') throw new TransportFailure('Disconnected', 'The server is not connected.');
    const target = str(request.path);
    if (!target || (str(request.method) || 'GET') !== 'GET' || request.body !== undefined) throw this.argument('The client HTTP interface is read-only.');
    return this.ok(await this.http(target, { epoch: this.generation }));
  }

  private async uploadAttachment(request: Obj): Promise<Reply> {
    if (this.state !== 'connected') throw new TransportFailure('Disconnected', 'The server is not connected.');
    // A folded paste uploads as UTF-8 text and an attached file under its own type, up to 50MB; anything without a type is a captured PNG.
    const declared = str(request.contentType);
    const typed = !!declared && /^[A-Za-z0-9!#$&^_.+-]+\/[A-Za-z0-9!#$&^_.+-]+(;[ A-Za-z0-9=._-]*)?$/.test(declared);
    const target = str(request.path);
    let data: Uint8Array | null = null;
    try { data = typeof request.base64 === 'string' ? Uint8Array.from(atob(request.base64), char => char.charCodeAt(0)) : null; } catch { data = null; }
    if (!target.startsWith('/api/attachments/upload/') || !data || !data.length || data.length > (typed ? 50 : 10) * 1024 * 1024) {
      throw this.argument(typed ? 'Choose a file up to 50MB.' : 'Choose a valid captured PNG up to10MB.');
    }
    const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    if (!typed && !png.every((byte, index) => data![index] === byte)) throw this.argument('The captured image is not a PNG.');
    return this.ok(await this.http(target, { epoch: this.generation, method: 'POST', body: data, contentType: typed ? declared : 'image/png', authorized: false }));
  }

  /** `POST /api/pull-requests/diff` (PullRequestDiffInput; the reference's client gives it 60 s); identical reads join. */
  private async pullRequestDiff(request: Obj): Promise<Reply> {
    if (this.state !== 'connected') throw new TransportFailure('Disconnected', 'The server is not connected.');
    const body = pullRequestDiffBody(request.payload);
    const key = `${this.generation}\n${body}`;
    const joined = this.pendingDiffs.get(key);
    if (joined) return await new Promise<Reply>(done => joined.push(done));
    const waiters: Done[] = [];
    this.pendingDiffs.set(key, waiters);
    let reply: Reply;
    try { reply = this.ok(await this.http('/api/pull-requests/diff', { epoch: this.generation, method: 'POST', body, long: true })); }
    catch (error) { reply = this.bad(this.failure(error)); }
    if (this.pendingDiffs.get(key) === waiters) this.pendingDiffs.delete(key);
    for (const waiter of waiters) waiter(reply);
    return reply;
  }

  // MARK: Saved environments

  private forgetEnvironment(request: Obj): Reply {
    const environment = str(request.environmentId);
    if (typeof request.origin !== 'string' || !environment) throw this.argument('forgetEnvironment requires origin and environmentId.');
    const saved = parseOrigin(request.origin);
    const catalog = this.hooks.catalog;
    const gone = catalog.forget(environment);
    for (const owner of [...credentialOrigins(gone), saved]) catalog.forgetToken(owner, environment);
    const focusedId = str(this.descriptor.environmentId);
    // One entry per environment: its focus may be any of its routes. A saved duplicate of the primary goes without touching the primary.
    const focused = !this.primary && (focusedId === environment || (!focusedId && (this.origin === saved || this.routes.environmentId === environment)));
    if (focused) {
      this.wantsConnection = false; this.reconnectAt = null; this.routes.stop();
      this.retire(new TransportFailure('Disconnected', 'Disconnected from the server.', true));
      this.generation += 1; this.inbox.reset(); this.token = '';
      this.setStatus('disconnected', 'Disconnected.');
    }
    this.forgotten(saved, focused);
    return this.ok({ ...this.status(), forgotFocus: focused });
  }

  /** Pair another environment without touching the active connection: its descriptor, then the one-time code's token. */
  private async pairEnvironment(request: Obj): Promise<Reply> {
    const target = parseOrigin(str(request.origin));
    const code = parseCredential(str(request.credential), target);
    if (!code) throw new TransportFailure('Credential', 'Enter a pairing code.');
    const scrub = (text: string) => this.clean(text).split(code).join('[redacted]');
    const send = async (where: string, body?: string): Promise<Obj> => {
      let reply: HttpReply;
      const headers: Record<string, string> = { 'x-t3-orchestration-protocol': '2', Accept: 'application/json' };
      if (body !== undefined) headers['Content-Type'] = 'application/x-www-form-urlencoded';
      try { reply = await this.hooks.http(onOrigin(where, target), { method: body === undefined ? 'GET' : 'POST', headers, body, timeoutMs: 30_000 }); }
      catch (error) { throw new TransportFailure('Network', scrub(error instanceof Error ? error.message : String(error))); }
      const decoded = json(reply);
      if (reply.status < 200 || reply.status >= 300 || typeof decoded !== 'object' || decoded === null || Array.isArray(decoded)) {
        throw new TransportFailure(reply.status === 401 || reply.status === 403 ? 'Authentication' : 'HTTP', scrub(failureMessage(decoded, reply.status)));
      }
      return decoded as Obj;
    };
    const descriptor = await send('/.well-known/t3/environment');
    const environment = str(descriptor.environmentId);
    if (!environment) throw new TransportFailure('Protocol', 'The server did not identify its environment.');
    // Adding a route to a saved machine: checked before the one-time code is spent (onboarding.ts).
    const expected = str(request.expectedEnvironmentId);
    if (expected && environment !== expected) {
      throw new TransportFailure('Pairing', `That ${request.ssh === true ? 'host' : 'address'} reaches ${str(descriptor.label) || 'another environment'}, a different machine. Add it as its own environment instead.`);
    }
    const problem = compatibility(descriptor);
    if (problem) throw new TransportFailure('Protocol', problem.message);
    const grant = await send('/oauth/token', exchangeForm(code, str(request.scope)));
    if (grant.token_type !== 'Bearer' || !str(grant.access_token)) throw new TransportFailure('Protocol', 'The server did not issue a bearer access token.');
    // This machine's own server (the primary's environment id): the code is spent, nothing is saved (registry.ts register).
    const primaryId = str(request.primaryEnvironmentId);
    if (primaryId && primaryId === environment) return this.ok({ origin: target, environmentId: environment, label: str(descriptor.label), primary: true });
    this.hooks.catalog.saveToken(str(grant.access_token), target, environment);
    this.hooks.catalog.remember(target, descriptor, this.hooks.wall(this.hooks.now()));
    return this.ok({ origin: target, environmentId: environment, label: str(descriptor.label) });
  }

  /**
   * The app's route edits (connection-routes.ts). A removed paired route's token goes with it unless another route
   * still borrows it; removing the route in use reconnects; a route that now ranks above the one in use is checked.
   */
  private setRoutes(request: Obj): Reply {
    const environment = str(request.environmentId);
    if (!environment || !Array.isArray(request.routes)) throw this.argument('setRoutes requires environmentId and routes.');
    const catalog = this.hooks.catalog;
    const removed = catalog.setRoutes(environment, request.routes as Route[]);
    const kept = catalog.entryForEnvironment(environment);
    const owners = new Set([...credentialOrigins(kept), ...((Array.isArray(kept?.routes) ? kept!.routes : []) as Route[]).map(route => str(route.credential))]);
    for (const route of removed) {
      if (route.learned === true) continue;
      const owner = trimmed(str(route.credential) || str(route.origin));
      if (owner && !owners.has(owner)) { try { catalog.forgetToken(owner, environment); } catch { /* kept */ } }
    }
    if (this.routes.environmentId === environment) {
      const active = this.routes.activeId;
      this.routes.reload(catalog);
      if (active && removed.some(route => route.id === active) && this.wantsConnection) {
        this.retire(new TransportFailure('Replaced', 'The route in use was removed.', true));
        this.failures = 0; this.start();
      } else if (this.state === 'connected') {
        this.routes.schedule(this.hooks.now());
        this.lookForBetterRoute();
      }
    }
    return this.ok({ saved: catalog.all });
  }
}

/** `ws(s)://<host>/ws` with the socket ticket and this client's surface, as the reference's resolver builds it. */
export function socketURL(origin: string, ticket: string, gated: boolean): string {
  const url = new URL(origin);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = '/ws';
  const query = new URLSearchParams({ wsTicket: ticket });
  if (gated) query.set('orchestrationProtocol', '2');
  query.set('clientSurface', 'desktop'); query.set('clientOs', 'macos'); query.set('clientDeviceType', 'desktop');
  url.search = query.toString();
  return url.href;
}

export function hostOf(origin: string): string {
  try { return new URL(origin).hostname; } catch { return ''; }
}

/** JSON with object keys sorted, for a shared read's identity. */
export function stableJSON(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJSON).join(',')}]`;
  if (typeof value === 'object' && value !== null) {
    const record = value as Obj;
    return `{${Object.keys(record).sort().map(key => `${JSON.stringify(key)}:${stableJSON(record[key])}`).join(',')}}`;
  }
  return JSON.stringify(value) ?? 'null';
}

/** PullRequestDiffInput's fields and nothing else: projectId, host, repository, number, cursor and commit. */
export function pullRequestDiffBody(payload: unknown): string {
  const refused = new TransportFailure('Arguments', 'prDiff takes projectId, host, repository, number, cursor and commit.');
  if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) throw refused;
  const input = payload as Obj;
  const allowed = new Set(['projectId', 'host', 'repository', 'number', 'cursor', 'commit']);
  if (!Object.keys(input).every(key => allowed.has(key))) throw refused;
  const text = (key: string, required: boolean): string | undefined => {
    if (!(key in input)) { if (required) throw refused; return undefined; }
    const value = input[key];
    if (typeof value !== 'string' || !value.trim() || utf8Length(value) > 4096) throw refused;
    return value;
  };
  const number = input.number;
  if (typeof number !== 'number' || !Number.isInteger(number) || number <= 0) throw refused;
  const body: Obj = { projectId: text('projectId', true)!, repository: text('repository', true)!, number };
  for (const key of ['host', 'cursor', 'commit']) { const value = text(key, false); if (value !== undefined) body[key] = value; }
  return stableJSON(body);
}
