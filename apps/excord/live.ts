// The Discord session in module state (llp/0001 §4.2): one store, fed by the gateway's stream mapper
// and by the REST replies of the app's own requests, and the gateway state machine whose frames the
// mapper and the heartbeat mutation send. The data module has no clock: `clock.now` is the latest time a
// source was given, which the heartbeat task refreshes every second.
import type { NativeModule, Store as Secrets, StreamEvent, StreamSocket } from './app.contract.d.ts';
import { identity, restHeaders, type ClientIdentity, type Host } from './discord/client';
import { heartbeatDue, initialGateway, nextHeartbeatAt, step, type GatewayState, type StepResult } from './discord/gateway';
import { cmpId, makeNonce, type NonceState } from './discord/ids';
import * as rest from './discord/rest';
import * as S from './discord/store';

/** Where Discord is, rewritten by the bake (app.ts `origin`); the gateway and CDN follow from it. */
export interface Origins { rest: string; gateway: string; cdn: string; fixture: boolean }

const HOUR = 3_600_000;
// The identifies a session may spend in an hour (§2); the fixture's drives and tests relaunch freely.
const BUDGET = { live: { max: 4, windowMs: HOUR }, fixture: { max: 1000, windowMs: HOUR } };
const INFLATED_LIMIT = 16 * 1024 * 1024;

export const clock = { now: 0 };
export function at(ms: number): number {
  if (Number.isFinite(ms) && ms > clock.now) clock.now = ms;
  return clock.now;
}

export interface Live {
  origins: Origins;
  store: S.Store;
  gw: GatewayState | null;
  socket: StreamSocket | null;
  identity: ClientIdentity | null;
  token: string;
  limiter: rest.Limiter;
  nonces: NonceState;
  // The dispatches applied this session by sequence number, from the socket or a control reply.
  applied: Set<number>;
  // Messages that arrived in each channel since launch; a deletion never lowers it.
  arrived: Map<string, number>;
  // A version per channel that moves only when its messages change, which the window takes: a read
  // state's change re-asks the channel's record, not its history.
  history: Map<string, number>;
  // The guilds this session subscribed to (op 37), for the one being read.
  subscribed: Set<string>;
  typedAt: Map<string, number>;
  ackToken: string | null;
  pages: Map<string, AbortController>;
  // Sends that failed, by nonce, so a retry knows what to post.
  sends: Map<string, { channel: string; content: string; replyTo: string }>;
  attempt: number;
}

export function createLive(origins: Origins): Live {
  return { origins, store: S.createStore(), gw: null, socket: null, identity: null, token: '',
    limiter: rest.limiterInit(), nonces: { lastMs: null, sequence: 0 }, applied: new Set(), arrived: new Map(), history: new Map(),
    subscribed: new Set(), typedAt: new Map(), ackToken: null, pages: new Map(), sends: new Map(), attempt: 0 };
}

// The token is kept per backend, so a fixture build never reads a Discord token, nor the reverse.
export const tokenKey = (o: Origins): string => (o.fixture ? 'fixture-token' : 'discord-token');

/** The host the client identifies as (llp/0001 §2): the app's native module answers on Apple; the web
 * has none. macOS identifies as the web client until Charlie rules on the desktop identity (LLP 0002). */
export function hostOf(native: NativeModule | null | undefined): Host {
  if (!native?.available) return 'web';
  try {
    const answer = native.call({ op: 'host' }) as { host?: string };
    return answer.host === 'ios' ? 'ios' : 'web';
  } catch { return 'web'; }
}

function random(): number {
  try { const a = new Uint32Array(1); crypto.getRandomValues(a); return a[0] / 2 ** 32; } catch { return 0.5; }
}
function launchId(): string {
  try { return crypto.randomUUID(); } catch { return '00000000-0000-4000-8000-000000000000'; }
}

export function context(live: Live, body = false): rest.RestContext {
  return { origin: live.origins.rest, headers: live.identity ? restHeaders(live.identity, live.token, { body }) : {} };
}

/** Apply one dispatch once: the socket's copy and a control reply's copy share a sequence number. */
export function dispatch(live: Live, t: string, s: number, d: any): S.Changed {
  if (t === 'READY') live.applied.clear();
  if (live.applied.has(s)) return { channels: [], sidebar: false };
  live.applied.add(s);
  return applyEvent(live, t, d);
}

/** Apply an event the app learned of (a dispatch, or a REST reply standing for one). */
export function applyEvent(live: Live, t: string, d: any): S.Changed {
  const store = live.store;
  if (t === 'READY') live.subscribed.clear();
  if (t === 'MESSAGE_CREATE' && d?.channel_id && d.id) {
    const h = store.histories.get(d.channel_id);
    const nonce = d.nonce == null ? '' : String(d.nonce);
    const known = !!h?.messages.has(d.id) || (nonce !== '' && !!h?.pending.has(nonce));
    if (!known) live.arrived.set(d.channel_id, (live.arrived.get(d.channel_id) ?? 0) + 1);
  }
  const changed = S.applyDispatch(store, t, d, clock.now);
  if (t === 'READY' || HISTORY.test(t)) for (const id of changed.channels) touched(live, id);
  return changed;
}
const HISTORY = /^MESSAGE_(CREATE|UPDATE|DELETE|DELETE_BULK|REACTION_)/;
/** A local change to a channel's messages (a send, its failure, a retry). */
export function touched(live: Live, channel: string): void {
  live.history.set(channel, (live.history.get(channel) ?? 0) + 1);
}

export interface Snapshot {
  state: string; detail: string; me: string; fixture: boolean; sidebar: number;
  versions: { id: string; version: number; history: number }[]; retryIn: number; attempt: number;
}

export function snapshot(live: Live, state?: string, detail = ''): Snapshot {
  const gw = live.gw, store = live.store;
  const versions = [...store.versions].map(([id, version]) => ({ id, version, history: live.history.get(id) ?? 0 }));
  const retry = gw?.retryAtMs ?? null;
  return { state: state ?? gw?.phase ?? 'idle', detail: detail || (gw?.stoppedReason ?? ''), me: store.meId ?? '',
    fixture: live.origins.fixture, sidebar: S.sidebarVersion(store), versions,
    retryIn: retry === null ? 0 : Math.max(1, Math.ceil(retry - clock.now)), attempt: live.attempt };
}

function perform(live: Live, result: StepResult, socket: StreamSocket | null, secrets: Secrets | null): void {
  if (socket && socket.readyState === 1) for (const frame of result.send) socket.send(frame);
  for (const change of result.changes) {
    if (change.kind === 'dispatch') dispatch(live, change.t, change.s, change.d);
    else if (change.kind === 'token') {
      // Discord's refreshed credential replaces the kept one before the next request.
      live.token = change.token;
      try { secrets?.set(tokenKey(live.origins), change.token); } catch { /* kept in memory for this launch */ }
    }
  }
  if (result.close && socket && socket.readyState <= 1) socket.close(result.close.code, result.close.reason);
}

/** The gateway resource's answer: a socket whose mapper inflates each frame, steps the session and
 * answers a snapshot of versions. Its arguments never change; a reconnect is a refresh. */
export function connect(live: Live, secrets: Secrets, native: NativeModule | null | undefined): Snapshot | Promise<Snapshot> {
  const key = tokenKey(live.origins);
  let token = secrets.get(key);
  // The fixture's account signs in by itself; Discord's waits for the person (stage 2's sign-in).
  if (!token && live.origins.fixture) { token = 'fixture-token'; secrets.set(key, token); }
  if (!token) return snapshot(live, 'signed-out');
  live.token = token;
  if (!live.gw || live.gw.phase === 'stopped') {
    live.identity = identity(hostOf(native), { launchId: launchId(), heartbeatSessionId: launchId(), appState: 'focused' });
    live.gw = initialGateway({ identity: live.identity, token, gatewayUrl: live.origins.gateway,
      identifyBudget: live.origins.fixture ? BUDGET.fixture : BUDGET.live });
  } else live.gw.token = token;
  const gw = live.gw;
  const first = step(gw, gw.phase === 'backoff' ? { type: 'retry', nowMs: clock.now, jitter: random() }
    : { type: 'connect', nowMs: clock.now, jitter: random() });
  if (!first.connect || gw.phase !== 'connecting') return snapshot(live);
  live.attempt++;
  live.socket = null;
  return fetch(first.connect.url, {
    exactIndependentHttp: { maxResponseBytes: INFLATED_LIMIT },
    exactStream: (event: StreamEvent, socket?: StreamSocket) => {
      if (live.gw !== gw) return snapshot(live);
      if (socket) live.socket = socket;
      const handle = socket ?? live.socket;
      let result: StepResult;
      if (event.type === 'open') result = step(gw, { type: 'open', nowMs: clock.now });
      else if (event.type === 'message') {
        result = step(gw, { type: 'message', data: event.data, nowMs: clock.now, jitter: random() });
      } else {
        live.socket = null;
        result = step(gw, { type: 'close', code: event.code ?? 1006, reason: event.reason ?? event.message ?? '', nowMs: clock.now });
      }
      perform(live, result, handle, secrets);
      return snapshot(live);
    },
  });
}

/** The heartbeat task's mutation: op 1 when the session's interval is up. */
export function heartbeat(live: Live, nowMs: number): boolean {
  at(nowMs);
  const gw = live.gw;
  if (!gw || !live.socket || !heartbeatDue(gw, clock.now)) return false;
  perform(live, step(gw, { type: 'heartbeat', nowMs: clock.now }), live.socket, null);
  return true;
}
export const heartbeatAt = (live: Live): number | null => (live.gw ? nextHeartbeatAt(live.gw) : null);

/** Op 37 for the guild being read, once a session (§2: passive writes only for what a person is
 * looking at). Its payload is the desktop client's as recalled, unverified until a capture. */
export function subscribe(live: Live, guildId: string): void {
  if (!guildId || live.subscribed.has(guildId) || !live.socket || live.gw?.phase !== 'ready') return;
  live.socket.send(JSON.stringify({ op: 37, d: { subscriptions: { [guildId]: { typing: true, threads: true, activities: true } } } }));
  live.subscribed.add(guildId);
}

export class RestError extends Error {
  constructor(public status: number, message: string) { super(message); }
}

/** One REST request: admitted by the rate limiter (a request it would hold is refused, since a data
 * module cannot wait), then reported back to it. */
export async function call(live: Live, request: rest.RestRequest, signal?: AbortSignal): Promise<any> {
  const admitted = rest.admit(live.limiter, request, clock.now);
  if (!admitted.ok) {
    throw new RestError(429, Number.isFinite(admitted.waitMs)
      ? `Discord asks to wait ${Math.ceil(admitted.waitMs / 1000)} s` : 'Another request on this route is still out');
  }
  let response: Response;
  try {
    response = await fetch(request.url, { method: request.method, headers: request.headers, body: request.body, signal });
  } catch (error) {
    rest.onResponse(live.limiter, request, 0, () => null, null, clock.now);
    throw error;
  }
  const text = await response.text();
  let body: any = null;
  try { body = text ? JSON.parse(text) : null; } catch { body = null; }
  rest.onResponse(live.limiter, request, response.status, name => response.headers.get(name), body, clock.now);
  if (!response.ok) throw new RestError(response.status, typeof body?.message === 'string' ? body.message : `HTTP ${response.status}`);
  return body;
}

/** One history page into the store, the channel's only page in flight: a newer page aborts it. */
export async function page(live: Live, channelId: string, query: S.HistoryQuery): Promise<void> {
  live.pages.get(channelId)?.abort();
  const controller = new AbortController();
  live.pages.set(channelId, controller);
  const since = S.storeSeq(live.store);
  try {
    const messages = await call(live, rest.history(context(live), channelId, query), controller.signal);
    if (Array.isArray(messages)) S.applyHistoryPage(live.store, channelId, query, messages, { since });
  } finally {
    if (live.pages.get(channelId) === controller) live.pages.delete(channelId);
  }
}

export function nonce(live: Live, nowMs: number): string { return makeNonce(Math.round(at(nowMs)), live.nonces); }

export const newer = (a: string | null | undefined, b: string | null | undefined): boolean =>
  !!a && (!b || cmpId(a, b) > 0);
