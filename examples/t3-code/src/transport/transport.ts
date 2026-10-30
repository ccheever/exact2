// The client's server connections in the data module: the focused environment's session, one
// session per switched-on background environment (Settings › Connections, at most eight), the
// outdated hosts' update jobs, the connection catalog and the activity reporter. T3 Code's own client
// is shared TypeScript too (packages/client-runtime, used by its web, desktop and mobile builds); this is
// that logic, driven the way exact2 lets a data module talk to a server:
//
// - Each session's socket is a stream resource, `transportSocket(slot, url)`, granted by `net.websocket
//   ws:` and `wss:` (servers are chosen at run time). Its mapper answers Pings and acknowledges chunks in
//   the frame's own turn and flushes what waits (LLP 1016.000 D7 and its 2026-10-16 amendment).
// - The answer that asks for a request sends it on that socket and awaits the reply (D7's amendment of
//   2026-10-18): a resource's answer, or a command's once its source has answered with a promise.
// - The transport's tick, a mutation (`transportPump`), checks deadlines and pings, starts the next attempt
//   when its backoff is over, runs the activity reporter's debounce and cadence, and publishes the table:
//   every 250 ms while something is near, every second while connected, and once each time an answer that
//   may wait on the server starts or ends, or the table the delivering answer sees changes.
// - The HTTP steps of an attempt run in the answer that asked for the connection (`connect`, `retry`), or,
//   for the ladder's attempts and a better-route check, in the slot's `transportConnect`; the tick then
//   publishes the socket's URL to the slot's socket resource.
// - Replies are resolved in `transportDeliver`, an answer asked again whenever a socket, a connect answer
//   or the tick has news, so the answer that awaited a reply continues inside an answer (and may fetch).
//
// The rest of the client still reaches the connection through the native bridge's operations (`connect`,
// `request`, `subscribe`, `events`, …): `wrap` answers them here and passes every other operation to the
// app's module.
import { Session, type SocketEvent, type SocketHandle, type Reply } from './transport-session';
import { Catalog, type SecretStore } from './transport-catalog';
import { ActivityReporter } from './transport-activity';
import { OutdatedHosts, UPDATE_SLOTS } from './transport-outdated';
import { TransportFailure, notConnected, letGoAbort, random as platformRandom, refillRandom, uuid as platformUuid } from './transport-wire';
import type { Http, HttpReply } from './transport-routes';

type Obj = Record<string, unknown>;
const str = (value: unknown) => typeof value === 'string' ? value : '';
const obj = (value: unknown): Obj => typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Obj : {};

/** The bridge the client calls: the app's native module, or this transport in front of it. */
export interface Bridge { available: boolean; watch(topic: string): void; later(request: unknown): Promise<unknown> }

export const FLEET_LIMIT = 8;
export const SESSION_SLOTS = FLEET_LIMIT + 1;
const FOCUSED = 'focused';

/** The ops of the focused connection this transport answers (the rest are the module's). */
const SESSION_OPS = new Set(['status', 'ids', 'connect', 'retry', 'disconnect', 'http', 'previewStreamAccess', 'deviceHubAccess', 'uploadAttachment', 'request', 'subscribe', 'unsubscribe', 'events', 'ack',
  'readChunk', 'releaseChunk', 'environments', 'connectionPreferences', 'setConnectionPreferences', 'setEnvironmentEnabled', 'forgetEnvironment',
  'pairEnvironment', 'setRoutes', 'prDiff', 'readSession']);

export interface Slot { key: string; attempt: number; check: number; url: string }
export interface UpdateSlot { key: string; step: number }
/** What the tick publishes for the window: the sockets it wants, when it wants to run again, and its revision. */
export interface Table {
  deliver: number; rev: number; soon: boolean; live: boolean;
  s0: Slot; s1: Slot; s2: Slot; s3: Slot; s4: Slot; s5: Slot; s6: Slot; s7: Slot; s8: Slot;
  u0: UpdateSlot; u1: UpdateSlot; u2: UpdateSlot; u3: UpdateSlot;
}

/**
 * fetch with exact2's deadline and no redirects followed (a credential never follows one). Each request may overlap
 * the others and settle in any order, as the native transport's did: exact2 runs a source's fetches one at a time
 * unless it says so (LLP 1041 §8.4), and a route check that waits out its 2.5 s would hold every other read.
 */
export const fetchHttp: Http = async (url, init) => {
  const exact = { exactTimeout: init.timeoutMs, exactIndependentHttp: { maxResponseBytes: init.maxBytes ?? 1024 * 1024 } };
  const response = await fetch(url, { method: init.method ?? 'GET', headers: init.headers, ...(init.bodyFrom ? { exactBodyFrom: init.bodyFrom } : { body: init.body }), signal: init.signal,
    redirect: 'manual', ...exact } as RequestInit);
  const reply: HttpReply = { status: response.status, text: await response.text() };
  return reply;
};

/** A module call's failure read as no reply, except a let-go answer's, which goes on up (client/let-go.ts). */
const passLetGo = (error: unknown): null => { if (letGoAbort(error)) throw error; return null; };

export interface TransportHost { http: Http; random(): number; uuid(): string }
export const platformHost: TransportHost = { http: fetchHttp, random: platformRandom, uuid: platformUuid };

export class Transport {
  readonly catalog: Catalog;
  readonly activity: ActivityReporter;
  readonly outdated: OutdatedHosts;
  readonly focused: Session;
  private fleet = new Map<string, { session: Session; slot: number }>();
  private queue: Array<() => void> = [];
  /** Bumps whenever the delivering answer has work or something it reports changed. */
  seq = 0;
  /** Status, events and fleet changes, for the snapshot; fleet changes alone, for Settings › Connections. */
  news = 0;
  /** The table's revision: it moves whenever what the tick publishes, but the delivery count, changes. */
  private rev = 0;
  private revKey = '';
  fleetNews = 0;
  private clock = 0;
  store: SecretStore | null = null;
  /** The app's module: the embedded server's bearer and the presentation a status adds. */
  private module: Bridge | null = null;
  /** The module topics each client source watched (see `wrap`). */
  private watched = new Map<string, Set<string>>();

  constructor(private readonly host: TransportHost = platformHost) {
    this.catalog = new Catalog(() => this.store);
    this.activity = new ActivityReporter(() => this.catalog.activityClientId(() => this.host.uuid()), now => this.wall(now));
    this.outdated = new OutdatedHosts({ catalog: this.catalog, http: host.http, now: () => this.clock, wall: now => this.wall(now), changed: () => this.fleetChanged(),
      compatible: environment => { for (const session of this.sessions) if (str(session.descriptor.environmentId) === environment) session.resumeCompatible(); } });
    this.focused = this.session(FOCUSED, true);
  }

  now(): number { return this.clock; }
  /** A time an answer knows (the window's clock, coarser than the tick's), taken only before the first tick. */
  observe(now: number): void { if (!this.clock && Number.isFinite(now) && now > 0) this.clock = now; }
  wall(now: number): string { return new Date(now).toISOString(); }
  private bump(): void { this.seq += 1; }
  private fleetChanged(): void { this.news += 1; this.fleetNews += 1; this.bump(); }

  private session(key: string, focused: boolean): Session {
    return new Session(key, {
      catalog: this.catalog, http: this.host.http, now: () => this.clock, wall: now => this.wall(now),
      random: () => this.host.random(), uuid: () => this.host.uuid(),
      primaryBearer: async () => {
        const reply = obj(await this.module?.later({ op: 'localBearer' }).catch(passLetGo));
        return reply.ok === true ? str(obj(reply.value).token) : '';
      },
      activity: this.activity,
      changed: () => { if (focused) { this.news += 1; this.bump(); } else this.fleetChanged(); },
      deliver: work => { this.queue.push(work); this.bump(); },
    }, focused);
  }

  /** The session a slot holds: 0 is the focused one, 1 to 8 background environments. */
  sessionAt(slot: number): Session | null {
    if (slot === 0) return this.focused;
    for (const entry of this.fleet.values()) if (entry.slot === slot) return entry.session;
    return null;
  }
  get sessions(): Session[] { return [this.focused, ...[...this.fleet.values()].map(entry => entry.session)]; }

  // MARK: Sources

  /** A session's socket (a stream resource); its mapper hands each event to the session. */
  socket(slot: number, url: string): unknown {
    refillRandom();
    if (!url) return { state: 'idle', seq: this.seq };
    return fetch(url, { exactStream: (event: SocketEvent, handle?: SocketHandle) => {
      try { this.sessionAt(slot)?.socketEvent(url, event, handle); } catch { /* the session keeps its state */ }
      return { state: event.type, seq: this.seq };
    } } as RequestInit);
  }

  /** The HTTP steps of a slot's attempt, or the work it was asked for while connected (a route check, the session read). */
  async connect(slot: number, key: string, attempt: number, check: number, module: Bridge | null | undefined): Promise<{ attempt: number; seq: number }> {
    if (module?.available) this.module = module;
    refillRandom();
    const session = this.sessionAt(slot);
    if (!session || !key || (slot === 0 ? key !== FOCUSED : session.key !== key)) return { attempt, seq: this.seq };
    if (session.wantsFlow(attempt)) await session.connectFlow(attempt).catch(() => '');
    else if (session.flowRunning === attempt) await session.joinFlow(attempt);
    else if (check > 0) await session.background().catch(() => undefined);
    // The delivering answer is asked again, and sees the socket's URL in the table it reports.
    this.bump();
    return { attempt, seq: this.seq };
  }

  /** An update job's step: the descriptor and ticket that open its socket, or one poll while the host restarts. */
  async updateStep(slot: number, key: string, step: number): Promise<{ step: number; url: string; seq: number }> {
    const url = key ? await this.outdated.step(slot, key, step) : '';
    this.bump();
    return { step, url: url || (key ? this.outdated.socketFor(slot, key) : ''), seq: this.seq };
  }
  updateSocket(slot: number, url: string): unknown {
    if (!url) return { state: 'idle', seq: this.seq };
    return fetch(url, { exactStream: (event: SocketEvent, handle?: SocketHandle) => {
      try { this.outdated.socketEvent(slot, url, event, handle); } catch { /* the job keeps its state */ }
      return { state: event.type, seq: this.seq };
    } } as RequestInit);
  }

  /**
   * Resolves the replies that came in, in this answer, and reports what changed: news for the snapshot and
   * Settings › Connections, and the table's revision, which asks the tick to publish a changed table.
   */
  deliver(): { news: number; fleet: number; table: number } {
    refillRandom();
    for (let rounds = 0; this.queue.length && rounds < 64; rounds++) {
      for (const work of this.queue.splice(0)) { try { work(); } catch { /* one reply's failure is its own */ } }
    }
    return { news: this.news, fleet: this.fleetNews, table: this.table().rev };
  }

  /**
   * The tick, a mutation (it sends): the clock, then each session's deadlines, pings and backoff, the update jobs'
   * polls, the activity reporter, and the outbox. `reason` names a window signal: returning to the app, or the
   * network going away or coming back.
   */
  pump(now: number, reason = ''): Table {
    refillRandom();
    if (Number.isFinite(now) && now > 0) this.clock = now;
    for (const session of this.sessions) {
      if (reason === 'foreground') session.foreground();
      else if (reason === 'online' || reason === 'offline') session.network(reason === 'online');
      session.tick(this.clock);
    }
    this.outdated.tick(this.clock);
    this.activity.tick(this.clock);
    for (const session of this.sessions) session.flush();
    return this.table();
  }

  table(): Table {
    const slot = (index: number): Slot => {
      const session = this.sessionAt(index);
      return session ? { key: index === 0 ? FOCUSED : session.key, attempt: session.attempt, check: session.check, url: session.url } : { key: '', attempt: 0, check: 0, url: '' };
    };
    const updates = this.outdated.slotTable();
    const update = (index: number): UpdateSlot => updates[index] ?? { key: '', step: 0 };
    const sessions = this.sessions;
    const table: Omit<Table, 'deliver' | 'rev'> = {
      soon: sessions.some(session => session.soon) || this.activity.soon || this.outdated.busy,
      live: sessions.some(session => session.wantsConnection || session.state === 'connected') || this.activity.live,
      s0: slot(0), s1: slot(1), s2: slot(2), s3: slot(3), s4: slot(4), s5: slot(5), s6: slot(6), s7: slot(7), s8: slot(8),
      u0: update(0), u1: update(1), u2: update(2), u3: update(3),
    };
    const key = JSON.stringify(table);
    if (key !== this.revKey) { this.revKey = key; this.rev += 1; }
    return { deliver: this.seq, rev: this.rev, ...table };
  }

  // MARK: The bridge

  /**
   * The bridge an answer of `source` hands the client: this transport's ops, and the module's for the rest.
   * exact2 takes a topic only from an answer still in flight, and code that awaited a reply resumes in the
   * delivering answer, which has replied; so a topic is remembered for its source and watched again at the
   * start of that source's next answer, which is when a native reply would have let the old one watch it.
   */
  wrap<T extends Bridge>(module: T | null | undefined, source = ''): T | Bridge | null | undefined {
    if (!module?.available) return module;
    this.module = module;
    const topics = this.watched.get(source) ?? new Set<string>();
    for (const topic of topics) module.watch(topic);
    const watch = (topic: string) => {
      if (!topics.size) this.watched.set(source, topics);
      topics.add(topic);
      try { module.watch(topic); } catch { /* resumed after its answer replied: the next answer watches it */ }
    };
    return { available: true, watch, later: request => this.later(module, request) };
  }

  async later(module: Bridge, request: unknown): Promise<unknown> {
    const value = obj(request), op = str(value.op);
    if (typeof value.fleet === 'string') return this.fleetOp(value.fleet, op, value);
    if (op === 'activityFacts') {
      this.activity.facts(value.visible === true, value.focused === true);
      return { ok: true, generation: 0, value: {} };
    }
    if (op === 'status') return this.status(module, value);
    // The terminal page's socket goes to its own environment's server, focused or in the background.
    if (op === 'terminalSocket') {
      const session = this.sessions.find(entry => entry.state === 'connected' && str(entry.descriptor.environmentId) === str(value.environmentId));
      if (!session) return { ok: false, generation: 0, error: notConnected().json() };
      return session.terminalSocketAccess().catch(error => ({ ok: false, generation: 0, error: (error instanceof TransportFailure ? error : new TransportFailure('Network', String(error))).json() }));
    }
    // The client recorded something the window shows (a draft's launch, draft-launch.ts): the snapshot is read again.
    if (op === 'snapshotNews') { this.news += 1; this.bump(); return { ok: true, generation: 0, value: {} }; }
    if (SESSION_OPS.has(op)) return this.focused.perform(value);
    return module.later(request);
  }

  /** The focused status, with the presentation the module adds. */
  private async status(module: Bridge, request: Obj): Promise<Reply> {
    const reply = await this.focused.perform(request);
    const presented = obj(await module.later({ op: 'status' }).catch(passLetGo));
    const facts = obj(presented.value);
    if (reply.ok) reply.value = { ...obj(reply.value), presentation: obj(facts.presentation) };
    return reply;
  }

  private async fleetOp(key: string, op: string, request: Obj): Promise<unknown> {
    const failure = (kind: string, message: string) => ({ ok: false, generation: 0, error: new TransportFailure(kind, message).json() });
    if (!key.includes('\n') || key.length > 2048) return failure('Arguments', 'fleet requires an origin and environment key.');
    // Hosts too old for this client (upstream 22e9d35613): probe, pair and update them without a session.
    if (op.startsWith('fleetOutdated')) return this.outdated.perform(op, key, request);
    if (op === 'fleetStop') {
      const entry = this.fleet.get(key);
      if (entry) { this.fleet.delete(key); entry.session.destroy(); this.fleetChanged(); }
      return { ok: true, generation: 0, value: { stopped: !!entry } };
    }
    let entry = this.fleet.get(key);
    if (!entry) {
      // Only a connect may open a background environment; reads of an unknown key report it disconnected.
      if (op !== 'connect') {
        if (op === 'status') return { ok: true, generation: 0, value: { state: 'disconnected', origin: '', environmentId: '', message: '', descriptor: {}, failureKind: '', traceId: '' } };
        return failure('Disconnected', 'That environment is not connected.');
      }
      if (this.fleet.size >= FLEET_LIMIT) return failure('Limit', 'Too many environments are switched on.');
      const taken = new Set([...this.fleet.values()].map(item => item.slot));
      let slot = 1;
      while (taken.has(slot)) slot += 1;
      entry = { session: this.session(key, false), slot };
      this.fleet.set(key, entry);
      this.fleetChanged();
    }
    const forwarded = { ...request };
    delete forwarded.fleet;
    return entry.session.perform(forwarded);
  }

  destroy(): void {
    for (const session of this.sessions) session.destroy();
    this.fleet.clear();
    this.outdated.destroy();
  }
}

export const UPDATE_SLOT_COUNT = UPDATE_SLOTS;
