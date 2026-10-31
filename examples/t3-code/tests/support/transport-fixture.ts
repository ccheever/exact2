// A scripted T3 server and the window's side of the transport, for the transport tests: the server
// answers the HTTP steps and the socket's RPC frames; `Drive` stands in for app.contract's wiring (the
// tick, each slot's connect answer and socket resource, the delivering answer) and for the runtime that
// hands each socket event to its mapper. As app.contract runs it, the tick runs every 250 ms while the
// table says something is near and every second while connected, once when a bridge op starts or ends,
// and once when the delivering answer reports a table the tick has not published; the delivering answer
// is asked again only when a socket, a connect answer or the tick has reported a newer delivery count.
import { Transport, type Table } from '../../src/transport/transport';
import type { SocketHandle } from '../../src/transport/transport-session';
import type { SecretStore } from '../../src/transport/transport-catalog';
import type { Http, HttpInit } from '../../src/transport/transport-routes';

type Obj = Record<string, unknown>;

export class MemoryStore implements SecretStore {
  values = new Map<string, string>();
  writes = 0;
  get(name: string) { return this.values.get(name) ?? null; }
  set(name: string, value: string) { this.values.set(name, value); this.writes++; }
  forget(name: string) { this.values.delete(name); }
}

export class FakeSocket implements SocketHandle {
  readyState = 1;
  bufferedAmount = 0;
  sent: Obj[] = [];
  closed: { code: number; reason: string } | null = null;
  private unread = 0;
  /** Refuses every send, as exact2's handle does in a turn that may not send. */
  refusing = false;
  constructor(readonly url: string) {}
  send(data: string) {
    if (this.readyState !== 1) throw new Error('InvalidStateError: the socket is not open');
    if (this.refusing) throw new Error("socket.send is only available in a mutation's synchronous source, its socket's own mapper or an answer in flight");
    this.sent.push(JSON.parse(data) as Obj);
  }
  close(code = 1000, reason = '') { this.closed = { code, reason }; this.readyState = 3; }
  take(): Obj[] { const out = this.sent.slice(this.unread); this.unread = this.sent.length; return out; }
}

export type RpcHandler = (method: string, payload: Obj, id: string, server: FakeServer) => unknown | undefined;

/** One T3 server: its descriptor, pairing codes, tokens, tickets and RPC methods. */
export class FakeServer {
  descriptor: Obj;
  codes = new Set<string>(['code-1']);
  tokens = new Set<string>();
  tickets = new Set<string>();
  up = true;
  requests: Array<{ method: string; path: string; headers: Record<string, string>; body: string; timeoutMs?: number }> = [];
  /** How long a path takes to answer (ms): a request whose deadline is shorter fails as exact2's fetch does, kind `Timeout`. */
  delays: Record<string, number> = {};
  rpcs: Array<{ method: string; payload: Obj; id: string }> = [];
  methods: Record<string, RpcHandler> = {};
  /** Frames the server pushes on a stream (by request id), sent at the next turn. */
  outgoing: Array<{ socket: FakeSocket; frame: Obj }> = [];
  sockets: FakeSocket[] = [];
  routes: Record<string, (request: { method: string; body: string; headers: Record<string, string> }) => { status: number; body: unknown }> = {};
  /** Tickets minted for a route (`/api/auth/websocket-ticket/rpc`, `/media`); a server before 0.1.4 has only the bare path. */
  ticketRoutes = true;
  private serial = 0;
  constructor(readonly origin: string, environmentId = 'env-1', extra: Obj = {}) {
    this.descriptor = { environmentId, label: `Server ${environmentId}`, serverVersion: '0.0.46', orchestrationProtocolVersion: 2,
      capabilities: { connectionProbe: true }, platform: { os: 'macos', machine: 'arm64' }, ...extra };
    this.methods['server.probe'] = () => ({});
    this.methods['server.getConfig'] = () => ({ environment: this.descriptor });
    this.methods['server.reportClientActivity'] = () => ({});
  }

  http(method: string, target: string, init: HttpInit): { status: number; body: unknown } {
    const headers = init.headers ?? {};
    const body = init.bodyFrom ? `<file ${init.bodyFrom}>` : typeof init.body === 'string' ? init.body : init.body ? `<${init.body.length} bytes>` : '';
    this.requests.push({ method, path: target, headers, body, timeoutMs: init.timeoutMs });
    const custom = this.routes[target];
    if (custom) return custom({ method, body, headers });
    const bearer = (headers.Authorization ?? '').replace(/^Bearer /, '');
    if (target === '/.well-known/t3/environment') return { status: 200, body: this.descriptor };
    if (target === '/oauth/token') {
      const code = new URLSearchParams(body).get('subject_token') ?? '';
      if (!this.codes.has(code)) return { status: 401, body: { reason: 'invalid_credential' } };
      this.codes.delete(code);
      const token = `token-${++this.serial}`;
      this.tokens.add(token);
      return { status: 200, body: { token_type: 'Bearer', access_token: token } };
    }
    if (!this.tokens.has(bearer) && !target.startsWith('/api/attachments/upload/')) return { status: 401, body: { message: 'Unauthorized' } };
    if (target === '/api/auth/session') return { status: 200, body: { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'] } };
    const routed = this.ticketRoutes && ['/api/auth/websocket-ticket/rpc', '/api/auth/websocket-ticket/media'].includes(target);
    if (target === '/api/auth/websocket-ticket' || routed) { const ticket = `ticket-${++this.serial}`; this.tickets.add(ticket); return { status: 200, body: { ticket } }; }
    return { status: 404, body: { message: `No route ${target}` } };
  }

  accept(url: string): FakeSocket | null {
    const ticket = new URL(url).searchParams.get('wsTicket') ?? '';
    if (!this.up || !this.tickets.has(ticket)) return null;
    this.tickets.delete(ticket);
    const socket = new FakeSocket(url);
    this.sockets.push(socket);
    return socket;
  }

  /** The server's answers to what a socket sent: an Exit per request (or nothing for a stream), a Pong per Ping. */
  respond(socket: FakeSocket): Obj[] {
    const replies: Obj[] = [];
    for (const frame of socket.take()) {
      if (frame._tag === 'Ping') { replies.push({ _tag: 'Pong' }); continue; }
      if (frame._tag !== 'Request') continue;
      const method = String(frame.tag), id = String(frame.id), payload = (frame.payload ?? {}) as Obj;
      this.rpcs.push({ method, payload, id });
      const handler = this.methods[method];
      if (!handler) { replies.push({ _tag: 'Exit', requestId: id, exit: { _tag: 'Failure', cause: [{ _tag: 'Fail', error: { _tag: 'RpcNotFound', message: `No method ${method}` } }] } }); continue; }
      const value = handler(method, payload, id, this);
      if (value !== undefined) replies.push({ _tag: 'Exit', requestId: id, exit: { _tag: 'Success', value } });
    }
    for (const item of this.outgoing.splice(0)) if (item.socket === socket) replies.push(item.frame);
    return replies;
  }
  /** Pushes stream values on a request (a subscription) at the next turn. */
  chunk(requestId: string, values: unknown[], socket = this.sockets.at(-1)!) { this.outgoing.push({ socket, frame: { _tag: 'Chunk', requestId, values } }); }
  exit(requestId: string, exit: Obj, socket = this.sockets.at(-1)!) { this.outgoing.push({ socket, frame: { _tag: 'Exit', requestId, exit } }); }
  restart() { for (const socket of this.sockets) socket.readyState = 3; }
}

/** The window's side: app.contract's transport wiring, on a clock the test moves. */
export class Drive {
  readonly store = new MemoryStore();
  readonly transport: Transport;
  now = 1_000_000;
  table: Table;
  readonly servers = new Map<string, FakeServer>();
  private connects: Array<{ attempt: number; check: number; pending: boolean }> = Array.from({ length: 9 }, () => ({ attempt: -1, check: 0, pending: false }));
  private sockets: Array<FakeSocket | null> = Array.from({ length: 9 }, () => null);
  private updates: Array<{ step: number; url: string; pending: boolean; socket: FakeSocket | null; socketUrl: string }> = Array.from({ length: 4 }, () => ({ step: -1, url: '', pending: false, socket: null, socketUrl: '' }));
  private socketUrls: string[] = Array.from({ length: 9 }, () => '');
  readonly closedEvents: string[] = [];
  /** The newest delivery count a socket, a connect or update answer, or the tick reported; the one the delivering answer last saw. */
  private reported = 0;
  private delivered = -1;
  private lastPump = 0;
  /** Ticks run, by why (`answered`, `republished`, `soon`, `live`). */
  readonly pumps: Record<string, number> = {};
  /** Answers to the module's ops (`localBearer`, `status`, `links`). */
  module = { available: true, watch(_topic: string) {}, later: async (request: unknown) => this.moduleOp(request as Obj) };
  moduleCalls: Obj[] = [];
  primaryBearer = '';
  /** A request to one of these paths waits for its promise before the server answers. */
  holds: Record<string, Promise<void>> = {};

  constructor(...servers: FakeServer[]) {
    for (const server of servers) this.servers.set(server.origin, server);
    const http: Http = async (url, init) => {
      const parsed = new URL(url);
      const server = this.servers.get(parsed.origin);
      if (!server || !server.up) throw new Error(`Could not connect to ${parsed.origin}`);
      await this.holds[parsed.pathname];
      const delay = server.delays[parsed.pathname] ?? 0;
      if (delay > init.timeoutMs) {
        server.requests.push({ method: init.method ?? 'GET', path: parsed.pathname + parsed.search, headers: init.headers ?? {}, body: '', timeoutMs: init.timeoutMs });
        throw Object.assign(new Error(`the request timed out after ${init.timeoutMs} ms`), { name: 'FetchError', kind: 'Timeout' });
      }
      const reply = server.http(init.method ?? 'GET', parsed.pathname + parsed.search, init);
      return { status: reply.status, text: reply.body === undefined ? '' : JSON.stringify(reply.body) };
    };
    let serial = 0;
    this.transport = new Transport({ http, random: () => 0.5, uuid: () => `uuid-${++serial}` });
    this.transport.store = this.store;
    this.table = this.transport.pump(this.now);
    this.reported = this.table.deliver; this.lastPump = this.now;
  }

  /** The tick, run for `why`. */
  pump(why: string, reason = '') {
    this.pumps[why] = (this.pumps[why] ?? 0) + 1;
    this.table = this.transport.pump(this.now, reason);
    this.reported = Math.max(this.reported, this.table.deliver); this.lastPump = this.now;
  }
  private report(seq: number) { this.reported = Math.max(this.reported, seq); }

  private async moduleOp(request: Obj): Promise<unknown> {
    this.moduleCalls.push(request);
    if (request.op === 'localBearer') return { ok: true, generation: 0, value: { token: this.primaryBearer } };
    if (request.op === 'status') return { ok: true, generation: 0, value: { presentation: { scrollEnds: {} } } };
    return { ok: true, generation: 0, value: {} };
  }

  /** The bridge a client answer is handed; the tick publishes the table once as each op starts and once as it ends (transportAnswered). */
  get bridge() {
    const inner = this.transport.wrap(this.module)!;
    return { ...inner, later: (request: unknown) => {
      const reply = inner.later(request);
      this.pump('answered');
      return reply.finally(() => this.pump('answered'));
    } };
  }
  /** A bridge op, as the client sends it; its reply once the drive has run, and the window once it is quiet again. */
  async op(request: Obj, rounds = 40): Promise<Obj> {
    let settled: Obj | null = null;
    void this.bridge.later(request).then(reply => { settled = reply as Obj; });
    for (let round = 0; round < rounds && settled === null; round++) await this.turn();
    if (settled === null) throw new Error(`op ${String(request.op)} never settled`);
    await this.settle();
    return settled;
  }

  /** Moves the clock, running the tick at its cadences (transportSoon, transportLive), and runs the window until nothing changes. */
  async advance(ms: number, step = 250) {
    for (let moved = 0; moved < ms; moved += step) {
      this.now += Math.min(step, ms - moved);
      if (this.table.soon) this.pump('soon');
      else if (this.table.live && this.now - this.lastPump >= 1000) this.pump('live');
      await this.turn();
    }
    await this.settle();
  }
  async settle(rounds = 30) { for (let round = 0; round < rounds; round++) if (!await this.turn()) return; }

  /** One turn: the slots' connect answers and sockets, the servers' replies, the delivering answer, and the tick it asks for. */
  async turn(): Promise<boolean> {
    let progress = false;
    for (let index = 0; index < 9; index++) {
      const slot = (this.table as unknown as Record<string, { key: string; attempt: number; check: number; url: string }>)[`s${index}`]!;
      const connect = this.connects[index]!;
      if (slot.key && !connect.pending && (slot.attempt !== connect.attempt || slot.check !== connect.check)) {
        connect.pending = true; progress = true;
        const attempt = slot.attempt, check = slot.check;
        void this.transport.connect(index, slot.key, attempt, check, this.module).then(reply => {
          this.report(reply.seq);
          if (this.connects[index] === connect) Object.assign(connect, { attempt: reply.attempt, check, pending: false });
        });
      }
      const url = slot.url;
      if (url !== this.socketUrls[index]) {
        progress = true;
        const old = this.sockets[index];
        if (old) { old.readyState = 3; }
        this.socketUrls[index] = url; this.sockets[index] = null;
        if (url) {
          const server = this.servers.get(new URL(url).origin.replace(/^ws/, 'http'));
          const socket = server?.accept(url) ?? null;
          const session = this.transport.sessionAt(index);
          if (socket) { this.sockets[index] = socket; session?.socketEvent(url, { type: 'open' }, socket); }
          else session?.socketEvent(url, { type: 'error', kind: 'Network', message: 'Could not connect to the server.' }, undefined);
          this.report(this.transport.seq);
        }
      }
      const socket = this.sockets[index];
      if (socket && url) {
        const server = this.servers.get(new URL(url).origin.replace(/^ws/, 'http'))!;
        if (socket.readyState !== 1 || !server.up) {
          this.sockets[index] = null; this.closedEvents.push(url); progress = true;
          this.transport.sessionAt(index)?.socketEvent(url, { type: 'close', code: 1006, reason: '' }, undefined);
          this.report(this.transport.seq);
          continue;
        }
        for (const frame of server.respond(socket)) {
          progress = true;
          this.transport.sessionAt(index)?.socketEvent(url, { type: 'message', data: JSON.stringify(frame) }, socket);
          this.report(this.transport.seq);
        }
      }
    }
    for (let index = 0; index < 4; index++) if (this.updateTurn(index)) progress = true;
    if (this.reported !== this.delivered) {
      this.delivered = this.reported;
      if (this.transport.deliver().table > this.table.rev) { this.pump('republished'); progress = true; }
    }
    await new Promise(resolve => setTimeout(resolve, 0));
    return progress || this.connects.some(connect => connect.pending) || this.updates.some(update => update.pending);
  }

  /** An update slot: its step answer and its bare socket. */
  private updateTurn(index: number): boolean {
    let progress = false;
    const slot = (this.table as unknown as Record<string, { key: string; step: number }>)[`u${index}`]!;
    const update = this.updates[index]!;
    if (slot.key && !update.pending && slot.step !== update.step) {
      update.pending = true; progress = true;
      void this.transport.updateStep(index, slot.key, slot.step).then(reply => { this.report(reply.seq); Object.assign(update, { step: reply.step, url: reply.url, pending: false }); });
    }
    const url = slot.key && update.step === slot.step ? update.url : '';
    if (url !== update.socketUrl) {
      progress = true;
      if (update.socket) update.socket.readyState = 3;
      update.socketUrl = url; update.socket = null;
      if (url) {
        const server = this.servers.get(new URL(url).origin.replace(/^ws/, 'http'));
        update.socket = server?.accept(url) ?? null;
        if (update.socket) this.transport.outdated.socketEvent(index, url, { type: 'open' }, update.socket);
        else this.transport.outdated.socketEvent(index, url, { type: 'error', message: 'Could not connect.' }, undefined);
        this.report(this.transport.seq);
      }
    }
    if (update.socket && url) {
      const server = this.servers.get(new URL(url).origin.replace(/^ws/, 'http'))!;
      if (!server.up) { this.transport.outdated.socketEvent(index, url, { type: 'close', code: 1006 }, undefined); this.report(this.transport.seq); update.socket = null; return true; }
      for (const frame of server.respond(update.socket)) { progress = true; this.transport.outdated.socketEvent(index, url, { type: 'message', data: JSON.stringify(frame) }, update.socket); this.report(this.transport.seq); }
    }
    return progress;
  }

  socketAt(index: number) { return this.sockets[index]; }
}

export const pairing = (origin = 'http://127.0.0.1:4001', code = 'code-1') => ({ op: 'connect', origin, credential: code, scope: 'orchestration:read orchestration:operate' });
