// The Discord session in module state (llp/0001 §4.2): one store, fed by the gateway's stream mapper
// and by the REST replies of the app's own requests, and the gateway state machine whose frames the
// mapper and the heartbeat mutation send. The data module has no clock: `clock.now` is the latest time a
// source was given, from the gateway's first turn (its launch epoch) on, which the heartbeat task moves
// on every second.
import type { NativeModule, Store as Secrets, StreamEvent, StreamSocket } from './app.contract.d.ts';
import { identity, restHeaders, type ClientIdentity, type Host } from './discord/client';
import { heartbeatDue, initialGateway, nextHeartbeatAt, step, type GatewayState, type StepResult } from './discord/gateway';
import { cmpId, makeNonce, type NonceState } from './discord/ids';
import * as rest from './discord/rest';
import * as S from './discord/store';

/** Where Discord is, rewritten by the bake (app.ts `origin`); the gateway and CDN follow from it. */
export interface Origins { rest: string; gateway: string; cdn: string; fixture: boolean }

const HOUR = 3_600_000;
// The identifies a session may spend in an hour (§2), counted across launches; the fixture's drives and
// tests relaunch freely.
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
  // Moves with any change outside the channel on screen: the lists' version.
  sidebar: number;
  // The channel on screen ("" on the lists) and its guild, which op 37 subscribes.
  viewing: { channel: string; guild: string };
  // The guild and channel the session's last op 37 named; a fresh READY forgets it.
  subscribed: string;
  // The identify record as last kept in secrets.
  kept: string;
  typedAt: Map<string, number>;
  ackToken: string | null;
  // Channels with a history page in flight: one at a time each (§2).
  paging: Set<string>;
  // Sends that failed, by nonce, so a retry knows what to post.
  sends: Map<string, { channel: string; content: string; replyTo: string }>;
  attempt: number;
}

export function createLive(origins: Origins): Live {
  return { origins, store: S.createStore(), gw: null, socket: null, identity: null, token: '',
    limiter: rest.limiterInit(), nonces: { lastMs: null, sequence: 0 }, applied: new Set(), arrived: new Map(),
    history: new Map(), sidebar: 0, viewing: { channel: '', guild: '' }, subscribed: '', kept: '', typedAt: new Map(),
    ackToken: null, paging: new Set(), sends: new Map(), attempt: 0 };
}

// Secrets are kept per backend, so a fixture build never reads a Discord token, nor the reverse.
export const tokenKey = (o: Origins): string => (o.fixture ? 'fixture-token' : 'discord-token');
export const identifiesKey = (o: Origins): string => (o.fixture ? 'fixture-identifies' : 'discord-identifies');

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

const HISTORY = /^MESSAGE_(CREATE|UPDATE|DELETE|DELETE_BULK|REACTION_)/;

/** Apply an event the app learned of (a dispatch, or a REST reply standing for one). */
export function applyEvent(live: Live, t: string, d: any): S.Changed {
  const store = live.store;
  if (t === 'READY') live.subscribed = '';
  if (t === 'MESSAGE_CREATE' && d?.channel_id && d.id) {
    const h = store.histories.get(d.channel_id);
    const nonce = d.nonce == null ? '' : String(d.nonce);
    const known = !!h?.messages.has(d.id) || (nonce !== '' && !!h?.pending.has(nonce));
    if (!known) live.arrived.set(d.channel_id, (live.arrived.get(d.channel_id) ?? 0) + 1);
  }
  const changed = S.applyDispatch(store, t, d, clock.now);
  const channels = new Set(changed.channels);
  // A deleted message the store never held changes nothing it keeps, but a held reply to it now
  // draws "Original message was deleted".
  if ((t === 'MESSAGE_DELETE' || t === 'MESSAGE_DELETE_BULK') && d?.channel_id && !channels.has(d.channel_id)
    && S.repliesTo(store, d.channel_id, t === 'MESSAGE_DELETE' ? [d.id] : d.ids ?? [])) channels.add(d.channel_id);
  if (t === 'READY' || HISTORY.test(t)) for (const id of channels) touched(live, id);
  if (changed.sidebar || [...channels].some(id => id !== live.viewing.channel)) live.sidebar++;
  return { channels: [...channels], sidebar: changed.sidebar };
}

/** A local change to a channel's messages (a send, its failure, a retry). */
export function touched(live: Live, channel: string): void {
  live.history.set(channel, (live.history.get(channel) ?? 0) + 1);
}

export interface Snapshot {
  state: string; detail: string; me: string; fixture: boolean; sidebar: number;
  channel: string; version: number; history: number; retryIn: number; attempt: number;
}

/** The gateway resource's answer, the same size whatever the account: the connection, the lists'
 * version, and the versions of the channel on screen. */
export function snapshot(live: Live, state?: string, detail = ''): Snapshot {
  const gw = live.gw, store = live.store, channel = live.viewing.channel;
  const retry = gw?.retryAtMs ?? null;
  return { state: state ?? gw?.phase ?? 'idle', detail: detail || (gw?.stoppedReason ?? ''), me: store.meId ?? '',
    fixture: live.origins.fixture, sidebar: live.sidebar, channel,
    version: channel ? S.channelVersion(store, channel) : 0, history: channel ? live.history.get(channel) ?? 0 : 0,
    retryIn: retry === null ? 0 : Math.max(1, Math.ceil(retry - clock.now)), attempt: live.attempt };
}

function perform(live: Live, result: StepResult, socket: StreamSocket | null, secrets: Secrets | null): void {
  if (socket && socket.readyState === 1) for (const frame of result.send) socket.send(frame);
  let ready = false;
  for (const change of result.changes) {
    if (change.kind === 'dispatch') {
      dispatch(live, change.t, change.s, change.d);
      ready ||= change.t === 'READY';
    } else if (change.kind === 'token') {
      // Discord's refreshed credential replaces the kept one before the next request.
      live.token = change.token;
      try { secrets?.set(tokenKey(live.origins), change.token); } catch { /* kept in memory for this launch */ }
    }
  }
  // Each identify is kept before the next launch can spend another (§2: a crash loop stops).
  const record = live.gw ? JSON.stringify(live.gw.identifies) : live.kept;
  if (secrets && record !== live.kept) {
    try { secrets.set(identifiesKey(live.origins), record); live.kept = record; } catch { /* counted for this launch */ }
  }
  // A fresh session subscribes again to what is on screen, as the official client does after IDENTIFY.
  if (ready && socket) subscribe(live, socket);
  if (result.close && socket && socket.readyState <= 1) socket.close(result.close.code, result.close.reason);
}

/** The session resource's answer: the kept token and identify record, read into module state. It is
 * the only answer that reads them, so the gateway's writes of them re-ask it, never the socket. */
export function loadSession(live: Live, secrets: Secrets): { ready: boolean; signedIn: boolean } {
  const key = tokenKey(live.origins);
  let token = secrets.get(key);
  // The fixture's account signs in by itself; Discord's waits for the person (stage 2's sign-in).
  if (!token && live.origins.fixture) { token = 'fixture-token'; secrets.set(key, token); }
  live.token = token ?? '';
  if (live.gw && token) live.gw.token = token;
  live.kept = secrets.get(identifiesKey(live.origins)) ?? '[]';
  return { ready: true, signedIn: !!token };
}

function keptIdentifies(live: Live): number[] {
  try {
    const kept = JSON.parse(live.kept);
    return Array.isArray(kept) ? kept.filter((t): t is number => typeof t === 'number' && Number.isFinite(t)) : [];
  } catch { return []; }
}

/** The gateway resource's answer: a socket whose mapper inflates each frame, steps the session and
 * answers a snapshot of versions. It waits for the session resource (`ready`) and takes the launch's
 * epoch, so the session's clock is real from its first step; a reconnect is a refresh. */
export function connect(live: Live, secrets: Secrets, native: NativeModule | null | undefined, ready: boolean,
  epoch: number): Snapshot | Promise<Snapshot> {
  at(epoch);
  if (!ready || clock.now <= 0) return snapshot(live, 'idle');
  const token = live.token;
  if (!token) return snapshot(live, 'signed-out');
  if (!live.gw || live.gw.phase === 'stopped') {
    live.identity = identity(hostOf(native), { launchId: launchId(), heartbeatSessionId: launchId(), appState: 'focused' });
    live.gw = initialGateway({ identity: live.identity, token, gatewayUrl: live.origins.gateway,
      identifyBudget: live.origins.fixture ? BUDGET.fixture : BUDGET.live, identifies: keptIdentifies(live) });
  } else live.gw.token = token;
  const gw = live.gw;
  const first = step(gw, gw.phase === 'backoff' ? { type: 'retry', nowMs: clock.now, jitter: random() }
    : { type: 'connect', nowMs: clock.now, jitter: random() });
  if (!first.connect || gw.phase !== 'connecting') return snapshot(live);
  const attempt = ++live.attempt;
  live.socket = null;
  return fetch(first.connect.url, {
    exactIndependentHttp: { maxResponseBytes: INFLATED_LIMIT },
    exactStream: (event: StreamEvent, socket?: StreamSocket) => {
      // Only the newest connection's events reach the session.
      if (live.gw !== gw || live.attempt !== attempt) return snapshot(live);
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

/** Op 37 for the guild and channel on screen, when they change and after each fresh READY (§2:
 * passive writes only for what a person is looking at). The payload is the desktop client's as
 * recalled, unverified until a capture. */
function subscribe(live: Live, socket: StreamSocket | null = live.socket): void {
  const { guild, channel } = live.viewing;
  const name = `${guild}/${channel}`;
  if (!guild || live.subscribed === name || !socket || socket.readyState !== 1 || live.gw?.phase !== 'ready') return;
  socket.send(JSON.stringify({ op: 37, d: { subscriptions: { [guild]: {
    typing: true, threads: true, activities: true, channels: { [channel]: [[0, 99]] } } } } }));
  live.subscribed = name;
}

/** The view's channel changed ("" for the lists): the snapshot's versions follow it, and op 37 names it. */
export function view(live: Live, channel: string): void {
  live.viewing = { channel, guild: channel ? S.channel(live.store, channel)?.guild_id ?? '' : '' };
  subscribe(live);
}

export class RestError extends Error {
  constructor(public status: number, message: string) { super(message); }
}

/** One REST request: admitted by the rate limiter (a request it would hold is refused, since a data
 * module cannot wait), then reported back to it. */
export async function call(live: Live, request: rest.RestRequest): Promise<any> {
  const admitted = rest.admit(live.limiter, request, clock.now);
  if (!admitted.ok) {
    throw new RestError(429, Number.isFinite(admitted.waitMs)
      ? `Discord asks to wait ${Math.ceil(admitted.waitMs / 1000)} s` : 'Another request on this route is still out');
  }
  let response: Response;
  try {
    response = await fetch(request.url, { method: request.method, headers: request.headers, body: request.body });
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

/** One history page into the store, verifying what it holds; one in flight per channel. */
export async function page(live: Live, channelId: string, query: S.HistoryQuery): Promise<void> {
  if (live.paging.has(channelId)) throw new RestError(429, 'A page of this channel is still loading');
  live.paging.add(channelId);
  const since = S.storeSeq(live.store);
  try {
    const messages = await call(live, rest.history(context(live), channelId, query));
    if (Array.isArray(messages)) {
      S.applyHistoryPage(live.store, channelId, query, messages, { since });
      touched(live, channelId);
    }
  } finally {
    live.paging.delete(channelId);
  }
}

export function nonce(live: Live, nowMs: number): string { return makeNonce(Math.round(at(nowMs)), live.nonces); }

export const newer = (a: string | null | undefined, b: string | null | undefined): boolean =>
  !!a && (!b || cmpId(a, b) > 0);
