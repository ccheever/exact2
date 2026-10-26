// The Discord session in module state (llp/0001 §4.2): one store, fed by the gateway's stream mapper
// and by the REST replies of the app's own requests, and the gateway state machine whose frames the
// mapper and the heartbeat mutation send. The data module has no clock: `clock.now` is the latest time a
// source was given, from the gateway's first turn (its launch epoch) on, which the heartbeat task moves
// on every second.
import type { NativeModule, Storage, Store as Secrets, StreamEvent, StreamSocket } from './app.contract.d.ts';
import { identity, restHeaders, type ClientIdentity, type Host } from './discord/client';
import { gather, heartbeatDue, inflate, inflated, initialGateway, nextHeartbeatAt, step, type GatewayState, type StepResult } from './discord/gateway';
import { cmpId, makeNonce, type NonceState } from './discord/ids';
import * as rest from './discord/rest';
import { startScan, step as scanReady, withoutGuilds, type ReadyScan } from './discord/ready';
import * as S from './discord/store';

/** Where Discord is, rewritten by the bake (app.ts `origin`); the gateway and CDN follow from it. */
export interface Origins { rest: string; gateway: string; cdn: string; fixture: boolean }

const HOUR = 3_600_000;
// The identifies a session may spend in an hour (§2), counted across launches; the fixture's drives and
// tests relaunch freely.
const BUDGET = { live: { max: 4, windowMs: HOUR }, fixture: { max: 1000, windowMs: HOUR } };
const INFLATED_LIMIT = 16 * 1024 * 1024;
// A packet that inflates past INLINE_TEXT waits for `digest`, as does every frame after it while one
// waits. On Hermes a turn has 100 ms, and a mapper cannot yield: a 3 MB READY took 129 ms to inflate,
// parse and apply in one on an M1 Max. fflate took about 40 ms an inflated megabyte, so a packet is
// fed to it PUSH_BYTES at a time until a turn has inflated INFLATE_OUT (the mapper's, INLINE_TEXT):
// DEFLATE's limit of about 1,032 to 1 (zlib.net/zlib_tech.html) inflates one push to at most a
// megabyte, whatever the text. Pushes of 1 KB cost about a fifth more than one push of the whole.
// The mapper's turn has no continuation: one the host refuses for its budget fails the stream (a
// megabyte READY took 868 ms there on a busy M5). So it inflates and parses only a small frame
// itself, and a larger one waits for `digest`, whose turns keep their progress if refused.
const INLINE_TEXT = 64 * 1024;
const PUSH_BYTES = 1024;
// The mapper's pushes: DEFLATE's limit inflates 64 bytes to at most about 66 KB, so its turn stops near
// INLINE_TEXT however compressible the packet is.
const INLINE_PUSH = 64;
const INFLATE_OUT = 1024 * 1024;
// A frame's text longer than this is scanned (`discord/ready.ts`) rather than parsed whole. On an
// M1 Max JSON.parse took about 7.5 ms a megabyte, the scan 23, and applying guilds 8: a turn scans
// SCAN_CHARS, or parses and applies PARSE_CHARS of guilds, in about 25 ms.
const SCAN_FROM = 2 * 1024 * 1024;
const SCAN_CHARS = 1024 * 1024;
const PARSE_CHARS = 1536 * 1024;
// What a turn of `digest` spends before it yields, in characters parsed: a step that inflates, scans or
// applies guilds spends all of it.
const TURN = 1024 * 1024;

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
  // The identify record as last read from app storage, and the IDENTIFY the mapper holds until a
  // newer record is written there (`recordIdentify`, then `sendIdentify`).
  kept: number[];
  held: string | null;
  // How the held IDENTIFY's record went: "" while it is being written, "ok", or why it failed.
  recorded: string;
  typedAt: Map<string, number>;
  ackToken: string | null;
  // Channels with a history page in flight: one at a time each (§2).
  paging: Set<string>;
  // `channel:anchor` for an anchor whose page did not hold it (a deleted message, a link past the
  // channel's end, an empty channel): the window shows the channel's end rather than ask again.
  missed: Set<string>;
  // Sends that failed, by nonce, so a retry knows what to post.
  // A send's files are its picked copies (`app:/tmp/picked/…`), uploaded when it posts; `uploaded` keeps
  // each file's upload as it lands, so a retry uploads only what did not.
  sends: Map<string, { channel: string; content: string; replyTo: string; mention: boolean; files: Upload[]; uploaded?: (rest.Uploaded | undefined)[] }>;
  attempt: number;
  // The kept credentials have been read: a missing token is then signed out, not loading.
  ready: boolean;
  // Socket events waiting for `digest`, in arrival order, the first holding its progress; and the
  // newest `digest`, which takes the queue from any before it.
  queue: Queued[];
  drainer: number;
  // Frames a turn could not send (the web sends only from an answer's first turn), in order, and a
  // close likewise: the heartbeat task's mutation sends them.
  outbox: string[];
  closing: { code: number; reason: string } | null;
  // Moves at each sign-out: a REST reply asked for before it is not applied.
  account: number;
  // Counts snapshots: the view takes the newer of the stream's and `digest`'s.
  snapshots: number;
  // The person signed out, or the platform dropped the locked token: the fixture's account does not sign itself in
  // again until the person asks (Lock Excord, LLP 1133.010 §9).
  signedOut: boolean;
}

// Socket bytes not yet gathered into a packet, a gathered packet and how much of it is inflated, a
// packet's text, or the socket's end. A large text's scan, its READY without the guilds and the next
// guild to apply are kept on it; `whole` is a text that parses at once.
type Queued = { type: 'bytes'; data: ArrayBuffer } | { type: 'packet'; data: Uint8Array; at: number }
  | { type: 'text'; data: string; scan?: ReadyScan; rest?: any; next?: number; whole?: boolean }
  | { type: 'close'; code: number; reason: string };

export function createLive(origins: Origins): Live {
  return { origins, store: S.createStore(), gw: null, socket: null, identity: null, token: '',
    limiter: rest.limiterInit(), nonces: { lastMs: null, sequence: 0 }, applied: new Set(), arrived: new Map(),
    history: new Map(), sidebar: 0, viewing: { channel: '', guild: '' }, subscribed: '', kept: [], held: null, recorded: '', typedAt: new Map(),
    ackToken: null, paging: new Set(), missed: new Set(), sends: new Map(), attempt: 0, ready: false,
    queue: [], drainer: 0, outbox: [], closing: null, account: 0, snapshots: 0, signedOut: false };
}

// Kept per backend, so a fixture build never reads a Discord token or record, nor the reverse.
export const tokenKey = (o: Origins): string => (o.fixture ? 'fixture-token' : 'discord-token');
export const identifiesPath = (o: Origins): string => `app:/data/${o.fixture ? 'fixture' : 'discord'}-identifies.json`;

/** The host the client identifies as (llp/0001 §2): the app's native module names it (Swift on Apple, Rust
 * on Windows and Android); a browser has none. */
const HOSTS: readonly Host[] = ['ios', 'macos', 'windows', 'android'];
export function hostOf(native: NativeModule | null | undefined): Host {
  if (!native?.available) return 'web';
  try {
    const host = (native.call({ op: 'host' }) as { host?: string }).host as Host;
    return HOSTS.includes(host) ? host : 'web';
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

const HISTORY = /^MESSAGE_(CREATE|UPDATE|DELETE|DELETE_BULK|REACTION_|POLL_VOTE_)/;

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
  const person = d?.user?.id ? S.user(store, String(d.user.id)) : null;
  const changed = S.applyDispatch(store, t, d, clock.now);
  const channels = new Set(changed.channels);
  // A deleted message the store never held changes nothing it keeps, but a held reply to it now
  // draws "Original message was deleted".
  if ((t === 'MESSAGE_DELETE' || t === 'MESSAGE_DELETE_BULK') && d?.channel_id && !channels.has(d.channel_id)
    && S.repliesTo(store, d.channel_id, t === 'MESSAGE_DELETE' ? [d.id] : d.ids ?? [])) channels.add(d.channel_id);
  if (t === 'READY' || HISTORY.test(t)) for (const id of channels) touched(live, id);
  // A role's colour is drawn in the names of the open channel's rows, and a server's name in a boost's line
  // (`render.ts`).
  if (/^GUILD_(ROLE_|UPDATE)/.test(t) && live.viewing.channel && S.channel(store, live.viewing.channel)?.guild_id === (d?.guild_id ?? d?.id)) {
    touched(live, live.viewing.channel);
  }
  // A person's new name or picture redraws the open channel, where a row may name them.
  if (t === 'GUILD_MEMBER_UPDATE' && person && live.viewing.channel && S.user(store, person.id) !== person) touched(live, live.viewing.channel);
  if (changed.sidebar || [...channels].some(id => id !== live.viewing.channel)) live.sidebar++;
  return { channels: [...channels], sidebar: changed.sidebar };
}

/** A local change to a channel's messages (a send, its failure, a retry). */
export function touched(live: Live, channel: string): void {
  live.history.set(channel, (live.history.get(channel) ?? 0) + 1);
}

export interface Upload { path: string; name: string; type: string; size: number; width: number; height: number }

export interface Snapshot {
  state: string; detail: string; me: string; meName: string; fixture: boolean; queued: number; seq: number; sidebar: number;
  channel: string; version: number; history: number; members: number; retryIn: number; attempt: number;
}

/** The gateway resource's answer, the same size whatever the account: the connection, the lists'
 * version, and the versions of the channel on screen. */
export function snapshot(live: Live, state?: string, detail = ''): Snapshot {
  const gw = live.gw, store = live.store, channel = live.viewing.channel;
  const retry = gw?.retryAtMs ?? null;
  const phase = !live.token && live.ready ? 'signed-out' : live.held ? 'recording' : gw?.phase ?? 'idle';
  return { state: state ?? phase, detail: detail || (gw?.stoppedReason ?? ''), me: store.meId ?? '',
    meName: store.meId ? S.user(store, store.meId)?.global_name || S.user(store, store.meId)?.username || '' : '',
    fixture: live.origins.fixture, queued: live.queue.length, seq: ++live.snapshots, sidebar: live.sidebar, channel,
    version: channel ? S.channelVersion(store, channel) : 0, history: channel ? live.history.get(channel) ?? 0 : 0,
    members: live.viewing.guild ? store.memberLists.get(live.viewing.guild)?.revision ?? 0 : 0,
    retryIn: retry === null ? 0 : Math.max(1, Math.ceil(retry - clock.now)), attempt: live.attempt };
}

function perform(live: Live, result: StepResult, socket: StreamSocket | null, secrets: Secrets | null): void {
  for (const frame of result.send) {
    // An IDENTIFY waits until its record is in app storage (§2: a crash loop stops at the budget only if
    // every identify it spends is counted, so one whose record cannot be kept is not sent).
    if (frame.startsWith('{"op":2,')) { live.held = frame; live.recorded = ''; continue; }
    if (socket && socket.readyState === 1) sendFrame(live, socket, frame);
  }
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
  // A fresh session subscribes again to what is on screen, as the official client does after IDENTIFY.
  if (ready && socket) subscribe(live, socket);
  // Discord refused the token (it was revoked, or the password changed): it is forgotten, and the
  // person signs in again.
  if (live.gw?.phase === 'stopped' && live.gw.stoppedReason?.startsWith('gateway close 4004')) forget(live, secrets);
  if (result.close && socket && socket.readyState <= 1) {
    try { socket.close(result.close.code, result.close.reason); } catch { live.closing = result.close; }
  }
}

/** Send a frame, after any a turn could not send; one this turn cannot send waits for the heartbeat. */
function sendFrame(live: Live, socket: StreamSocket, frame: string): void {
  if (!live.outbox.length) {
    try { socket.send(frame); return; } catch { /* sent from the heartbeat task's next mutation */ }
  }
  live.outbox.push(frame);
}

/** Send what earlier turns could not, from a mutation's first turn. */
function flush(live: Live): void {
  const socket = live.socket;
  if (!socket || socket.readyState !== 1) { live.outbox = []; live.closing = null; return; }
  while (live.outbox.length) { socket.send(live.outbox[0]); live.outbox.shift(); }
  if (live.closing) { const { code, reason } = live.closing; live.closing = null; socket.close(code, reason); }
}

/** A connection's end: its queued events, unsent frames and running `digest` go with it. */
function retire(live: Live): void {
  live.attempt++;
  live.queue = [];
  live.outbox = [];
  live.closing = null;
  // An IDENTIFY held for the retired connection is not that of the next, whose HELLO holds its own.
  live.held = null;
  live.recorded = '';
}

/** The session resource's answer: the kept token and identify record, read into module state. It is
 * the only answer that reads them, so the gateway's writes of them re-ask it, never the socket. A token
 * kept locked is read by `unlockSession` behind the person's check; until then the session is not
 * ready, so the gateway opens nothing, and the answer carries no token either way. */
export async function loadSession(live: Live, secrets: Secrets, storage: Storage): Promise<{ ready: boolean; signedIn: boolean; locked: boolean; lockOn: boolean }> {
  const key = tokenKey(live.origins);
  const lockOn = secrets.locked(key);
  if (lockOn && !live.token) return { ready: false, signedIn: true, locked: true, lockOn };
  let token = lockOn ? live.token : secrets.get(key);
  // The fixture's account signs in by itself; Discord's waits for the person (stage 2's sign-in).
  if (!token && live.origins.fixture && !live.signedOut) { token = 'fixture-token'; secrets.set(key, token); }
  live.token = token ?? '';
  live.ready = true;
  if (live.gw && token) live.gw.token = token;
  let kept: unknown = [];
  try { kept = JSON.parse(new TextDecoder().decode(await storage.fs.readFile(identifiesPath(live.origins)))); }
  catch { /* none yet, or unreadable: the record starts empty and is written before the first identify */ }
  live.kept = Array.isArray(kept) ? kept.filter((t): t is number => typeof t === 'number' && Number.isFinite(t)) : [];
  return { ready: true, signedIn: !!token, locked: false, lockOn };
}

/** Write the identify record, the held IDENTIFY's included, to app storage, and await it. */
export async function recordIdentify(live: Live, storage: Storage): Promise<void> {
  if (!live.held || !live.gw) return;
  // A write that lands after its connection was retired speaks for that connection's IDENTIFY, not the next's.
  const attempt = live.attempt, held = live.held, identifies = [...live.gw.identifies];
  let recorded: string;
  try {
    await storage.fs.atomicWriteFile(identifiesPath(live.origins), new TextEncoder().encode(JSON.stringify(identifies)));
    live.kept = identifies;
    recorded = 'ok';
  } catch (error) {
    recorded = `the identify record could not be kept (${error instanceof Error ? error.message : String(error)})`;
  }
  if (live.attempt === attempt && live.held === held) live.recorded = recorded;
}

/** Send the held IDENTIFY once its record is kept; else stop, saying why, and close the socket. */
export function sendIdentify(live: Live): void {
  const gw = live.gw, frame = live.held;
  if (!gw || !frame || !live.recorded) return;
  live.held = null;
  if (live.recorded === 'ok') { if (live.socket?.readyState === 1) live.socket.send(frame); return; }
  gw.phase = 'stopped';
  gw.stoppedReason = live.recorded;
  if (live.socket && live.socket.readyState <= 1) live.socket.close(4000, 'identify record not kept');
}

/** A token as a person pastes it: the `Authorization` header's value, or the quoted string the web
 * client keeps, with any `Bearer`/`Bot` scheme, which a user token does not take, refused. */
export function pastedToken(text: string): string | null {
  const token = text.trim().replace(/^"(.*)"$/, '$1').trim();
  if (!/^[A-Za-z0-9_\-.]{50,}$/.test(token)) return null;
  return token;
}

/** Keep a token the person gave, and connect with it at the gateway's next answer. */
export function signIn(live: Live, secrets: Secrets, text: string): string {
  if (live.origins.fixture) return 'The fixture signs itself in.';
  const token = pastedToken(text);
  if (!token) return "That doesn't look like a Discord token: it is one long word of letters, digits, dots, dashes and underscores.";
  secrets.set(tokenKey(live.origins), token);
  live.token = token;
  if (live.gw?.phase === 'stopped') live.gw = null;
  return '';
}

/** Forget the token and the account: the socket closes and the lists empty. */
export function signOut(live: Live, secrets: Secrets): void {
  if (live.socket && live.socket.readyState <= 1) live.socket.close(1000, 'signed out');
  live.gw = null;
  forget(live, secrets);
}

/** Forget the token, the account and the connection's work. A stopped gateway is kept, to say why. */
function forget(live: Live, secrets: Secrets | null): void {
  try { secrets?.forget(tokenKey(live.origins)); } catch { /* forgotten for this launch */ }
  live.token = '';
  live.socket = null;
  retire(live);
  live.account++;
  live.limiter = rest.limiterInit();
  live.held = null;
  live.ackToken = null;
  live.sends.clear();
  live.missed.clear();
  live.viewing = { channel: '', guild: '' };
  live.subscribed = '';
  S.forgetAccount(live.store);
  live.sidebar++;
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
      identifyBudget: live.origins.fixture ? BUDGET.fixture : BUDGET.live, identifies: live.kept });
  } else live.gw.token = token;
  const gw = live.gw;
  const first = step(gw, gw.phase === 'backoff' ? { type: 'retry', nowMs: clock.now, jitter: random() }
    : { type: 'connect', nowMs: clock.now, jitter: random() });
  if (!first.connect || gw.phase !== 'connecting') return snapshot(live);
  retire(live);
  const attempt = live.attempt;
  live.socket = null;
  return fetch(first.connect.url, {
    exactIndependentHttp: { maxResponseBytes: INFLATED_LIMIT },
    exactStream: (event: StreamEvent, socket?: StreamSocket) => {
      // Only the newest connection's events reach the session.
      if (live.gw !== gw || live.attempt !== attempt) return snapshot(live);
      if (socket) live.socket = socket;
      const handle = socket ?? live.socket;
      const waiting = live.queue.length > 0;
      const later = (item: Queued) => { live.queue.push(item); return snapshot(live); };
      let result: StepResult;
      if (event.type === 'open') result = step(gw, { type: 'open', nowMs: clock.now });
      else if (event.type === 'message') {
        if (typeof event.data === 'string') {
          if (waiting || event.data.length > INLINE_TEXT) return later({ type: 'text', data: event.data });
          result = step(gw, { type: 'message', data: event.data, nowMs: clock.now, jitter: random() });
        } else {
          if (waiting) return later({ type: 'bytes', data: event.data });
          let text: string | null = null;
          try {
            const packet = gather(gw, event.data);
            if (!packet) return snapshot(live);
            const item = { type: 'packet' as const, data: packet, at: 0 };
            if (!inflateSome(gw, item, INLINE_TEXT, INLINE_PUSH)) return later(item);
            text = inflated(gw);
          } catch { /* a packet that does not inflate is an invalid payload */ }
          if (text !== null && text.length > INLINE_TEXT) return later({ type: 'text', data: text });
          result = step(gw, { type: 'payload', ...parsed(text), nowMs: clock.now, jitter: random() });
        }
      } else {
        const close = { type: 'close' as const, code: event.code ?? 1006, reason: event.reason ?? event.message ?? '' };
        if (waiting) return later(close);
        live.socket = null;
        result = step(gw, { ...close, nowMs: clock.now });
      }
      perform(live, result, handle, secrets);
      return snapshot(live);
    },
  });
}

/** The digest mutation's answer: the queued socket events, in order, a step of work a turn. Each step
 * leaves its progress on the queued event, so a later digest goes on from where one the host stopped
 * (a turn over budget is refused, and its continuation never runs) left off. Its snapshot is newer than
 * the stream's last one, which the view takes until the stream's next. */
export async function digest(live: Live, secrets: Secrets, nowMs: number): Promise<Snapshot> {
  at(nowMs);
  const gw = live.gw, attempt = live.attempt, me = ++live.drainer;
  const current = () => live.gw === gw && live.attempt === attempt && live.drainer === me;
  let spent = 0;
  while (gw && current() && live.queue.length) {
    spent += work(live, gw, secrets);
    if (spent < TURN || !live.queue.length) continue;
    spent = 0;
    await scheduler.yield();
  }
  return snapshot(live);
}

/** Inflate more of a packet, until `out` bytes have been inflated or it ends: true when it has. */
function inflateSome(gw: GatewayState, item: { data: Uint8Array; at: number }, out: number, push = PUSH_BYTES): boolean {
  for (let made = 0; item.at < item.data.length && made < out;) made += inflate(gw, item.data.subarray(item.at, item.at += push));
  return item.at >= item.data.length;
}

/** One step of the queue's first event: a megabyte of a packet inflated, a step of a scan, a parse, the READY
 * without its guilds, or PARSE_CHARS of its guilds. The event leaves the queue with its last step.
 * Answers the step's cost, TURN for one that should have a turn of its own. */
function work(live: Live, gw: GatewayState, secrets: Secrets): number {
  const item = live.queue[0];
  const payload = (decoded: { payload?: unknown; invalid?: boolean }) =>
    perform(live, step(gw, { type: 'payload', ...decoded, nowMs: clock.now, jitter: random() }), live.socket, secrets);
  if (item.type === 'close') {
    live.queue.shift();
    live.socket = null;
    perform(live, step(gw, { type: 'close', code: item.code, reason: item.reason, nowMs: clock.now }), null, secrets);
    return 0;
  }
  try {
    if (item.type === 'bytes') {
      const packet = gather(gw, item.data);
      if (packet) live.queue[0] = { type: 'packet', data: packet, at: 0 };
      else live.queue.shift();
      return 0;
    }
    if (item.type === 'packet') {
      if (inflateSome(gw, item, INFLATE_OUT)) live.queue[0] = { type: 'text', data: inflated(gw) };
      return TURN;
    }
    const text = item.data;
    if (text.length > SCAN_FROM && !item.whole) {
      if (!item.scan) item.scan = startScan(text);
      if (item.scan.at < text.length) { scanReady(item.scan, SCAN_CHARS); return TURN; }
      const spans = item.scan.spans;
      if (!spans) { item.whole = true; return 0; }
      if (item.rest === undefined) {
        const rest = JSON.parse(withoutGuilds(text, spans));
        // Only a READY's guilds are applied a part at a time.
        if (rest?.t === 'READY') item.rest = rest;
        else item.whole = true;
        return TURN;
      }
      if (item.next === undefined) { item.next = 0; payload({ payload: item.rest }); return TURN; }
      // The READY's guilds, as GUILD_CREATE would bring each, with the account's own member
      // (`merged_members[i]`).
      const members: unknown[] = item.rest?.d?.merged_members ?? [];
      for (let applied = 0; item.next < spans.elements.length && applied < PARSE_CHARS;) {
        const i = item.next++, [from, to] = spans.elements[i];
        try {
          const guild = JSON.parse(text.slice(from, to));
          applyEvent(live, 'GUILD_CREATE', Array.isArray(members[i]) ? { ...guild, members: members[i] } : guild);
        } catch { /* one guild that does not parse is left out; GUILD_CREATE brings it again */ }
        applied += to - from;
      }
      if (item.next >= spans.elements.length) live.queue.shift();
      return TURN;
    }
    live.queue.shift();
    payload(parsed(text));
    return text.length;
  } catch {
    // A packet that does not inflate, or a READY that does not parse, is an invalid payload.
    live.queue.shift();
    payload({ invalid: true });
    return TURN;
  }
}

/** A packet's text as the gateway's payload, or invalid (null: it did not inflate). */
function parsed(text: string | null): { payload?: unknown; invalid?: boolean } {
  if (text === null) return { invalid: true };
  try { return { payload: JSON.parse(text) }; } catch { return { invalid: true }; }
}

/** The heartbeat task's mutation: op 1 when the session's interval is up. */
export function heartbeat(live: Live, nowMs: number): boolean {
  at(nowMs);
  const gw = live.gw;
  flush(live);
  subscribe(live);
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
  // A send refused where it was made (the web sends only from an answer's first turn, and `digest`
  // applies a large READY later) is made by the next heartbeat tick, which asks again.
  try {
    socket.send(JSON.stringify({ op: 37, d: { subscriptions: { [guild]: {
      typing: true, threads: true, activities: true, channels: { [channel]: [[0, 99]] } } } } }));
  } catch { return; }
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
  if (!live.token) throw new RestError(0, 'Signed out');
  // The limits are the account's: a reply that lands after a sign-out settles the old one's.
  const limiter = live.limiter, account = live.account;
  const admitted = rest.admit(limiter, request, clock.now);
  if (!admitted.ok) {
    throw new RestError(429, Number.isFinite(admitted.waitMs)
      ? `Discord asks to wait ${Math.ceil(admitted.waitMs / 1000)} s` : 'Another request on this route is still out');
  }
  let response: Response;
  try {
    response = await fetch(request.url, { method: request.method, headers: request.headers, body: request.body });
  } catch (error) {
    rest.onResponse(limiter, request, 0, () => null, null, clock.now);
    throw error;
  }
  const text = await response.text();
  let body: any = null;
  try { body = text ? JSON.parse(text) : null; } catch { body = null; }
  rest.onResponse(limiter, request, response.status, name => response.headers.get(name), body, clock.now);
  if (live.account !== account) throw new RestError(0, 'Signed out');
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
