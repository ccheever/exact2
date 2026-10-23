// The gateway session as a pure state machine (llp/0001 §4.1): `step` takes one socket event and
// returns the frames to send, the dispatches decoded, and what to close or open. The socket is the
// Contract stream resource's and the heartbeat clock a `task every`'s; time and jitter are arguments.
// Frames are JSON over `zlib-stream`: one inflate context per connection, each message ending in
// the sync-flush marker 00 00 ff ff.
import { Unzlib } from 'fflate';
import { identifyPayload, type ClientIdentity } from './client.ts';

export type GatewayPhase =
  | 'idle' | 'connecting' | 'awaiting-hello' | 'identifying'
  | 'resuming' | 'ready' | 'backoff' | 'stopped';

export type GatewayEvent =
  | { type: 'connect'; nowMs: number; jitter?: number }
  | { type: 'open'; nowMs: number; jitter?: number }
  | { type: 'message'; data: ArrayBuffer | Uint8Array | string; nowMs: number; jitter?: number }
  // A message the caller decoded itself (`gather`, `inflate`, `inflated`), or failed to.
  | { type: 'payload'; payload?: unknown; invalid?: boolean; nowMs: number; jitter?: number }
  | { type: 'close'; code: number; reason?: string; nowMs: number }
  | { type: 'heartbeat'; nowMs: number }
  | { type: 'retry'; nowMs: number; jitter: number };

export type GatewayChange =
  | { kind: 'dispatch'; t: string; s: number; d: any }
  // The caller persists this credential and rebuilds its REST headers before more requests.
  | { kind: 'token'; token: string }
  | { kind: 'status'; phase: GatewayPhase; detail?: string };

export interface StepResult {
  state: GatewayState;
  send: string[];
  changes: GatewayChange[];
  close?: { code: number; reason: string };
  connect?: { url: string; atMs: number };
}

interface ConnectionInflater {
  stream: Unzlib;
  pending: Uint8Array[];
  byteLength: number;
  tail: number;
  // The packet's text so far, decoded as it is inflated so that a large one's bytes are not held as
  // well, and the bytes of a character the last chunk split.
  text: string[];
  carry: Uint8Array | null;
  // Bytes inflated on this connection.
  out: number;
}

export interface GatewayState {
  identity: ClientIdentity;
  token: string;
  gatewayUrl: string;
  phase: GatewayPhase;
  sessionId: string | null;
  resumeGatewayUrl: string | null;
  seq: number | null;
  identifies: number[];
  identifyBudget: { max: number; windowMs: number };
  stoppedReason: string | null;
  heartbeatIntervalMs: number | null;
  heartbeatAtMs: number | null;
  awaitingAck: boolean;
  lastHeartbeatAtMs: number | null;
  lastAckAtMs: number | null;
  retryAtMs: number | null;
  reconnectAttempts: number;
  jitter: number;
  inflater: ConnectionInflater | null;
}

// Recalled Discord web-client backoff range, unverified: roughly 1 s through 60 s.
// Equal-width positive jitter gives [base, 2*base), capped at 60 s.
const BACKOFF_BASE_MS = 1_000;
const BACKOFF_CAP_MS = 60_000;
const HOUR_MS = 3_600_000;
// A normal (1000/1001) client close invalidates Discord's resumable session.
const RECONNECT_CLOSE = 4000;

export function initialGateway(opts: {
  identity: ClientIdentity;
  token: string;
  gatewayUrl: string;
  identifyBudget?: { max: number; windowMs: number };
  identifies?: number[];
}): GatewayState {
  const budget = opts.identifyBudget ?? { max: 4, windowMs: HOUR_MS };
  if (!Number.isSafeInteger(budget.max) || budget.max < 0 ||
      !Number.isFinite(budget.windowMs) || budget.windowMs <= 0) {
    throw new Error('invalid identify budget');
  }
  if (opts.identifies?.some(time => !Number.isFinite(time))) {
    throw new Error('invalid persisted identify time');
  }
  return {
    identity: opts.identity, token: opts.token, gatewayUrl: opts.gatewayUrl,
    phase: 'idle', sessionId: null, resumeGatewayUrl: null, seq: null,
    identifies: [...(opts.identifies ?? [])], identifyBudget: { ...budget },
    stoppedReason: null, heartbeatIntervalMs: null, heartbeatAtMs: null,
    awaitingAck: false, lastHeartbeatAtMs: null, lastAckAtMs: null,
    retryAtMs: null, reconnectAttempts: 0, jitter: 0.5, inflater: null,
  };
}

function inflater(): ConnectionInflater {
  // Each chunk decodes on its own, so a U+FEFF that starts one is text, not a byte order mark.
  const decoder = new TextDecoder('utf-8', { ignoreBOM: true });
  const context: ConnectionInflater = {
    stream: new Unzlib(chunk => {
      if (!chunk.length) return;
      context.out += chunk.length;
      const bytes = context.carry ? join([context.carry, chunk], context.carry.length + chunk.length) : chunk;
      const whole = completeUtf8(bytes);
      context.text.push(decoder.decode(bytes.subarray(0, whole)));
      context.carry = whole < bytes.length ? bytes.slice(whole) : null;
    }),
    pending: [], byteLength: 0, tail: 0, text: [], carry: null, out: 0,
  };
  return context;
}

/** How much of `bytes` is whole UTF-8 characters: a sequence the end cuts short is left out. */
function completeUtf8(bytes: Uint8Array): number {
  for (let i = bytes.length - 1; i >= Math.max(0, bytes.length - 4); i--) {
    const byte = bytes[i];
    if ((byte & 0xc0) === 0x80) continue;
    const size = byte >= 0xf0 ? 4 : byte >= 0xe0 ? 3 : byte >= 0xc0 ? 2 : 1;
    return bytes.length - i >= size ? bytes.length : i;
  }
  return bytes.length;
}

function join(chunks: Uint8Array[], byteLength: number): Uint8Array {
  if (chunks.length === 1) return chunks[0];
  const bytes = new Uint8Array(byteLength);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

/** Gather one socket message's bytes: the whole compressed packet once its flush marker has come
 * (the marker can itself straddle socket messages), else null. */
export function gather(state: GatewayState, data: ArrayBuffer | Uint8Array): Uint8Array | null {
  const context = state.inflater;
  if (!context) throw new Error('missing connection inflater');
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  if (!bytes.length) return null;
  context.pending.push(bytes);
  context.byteLength += bytes.length;
  for (let i = Math.max(0, bytes.length - 4); i < bytes.length; i++) {
    context.tail = ((context.tail << 8) | bytes[i]) >>> 0;
  }
  if (context.byteLength < 4 || context.tail !== 0x0000ffff) return null;
  const compressed = join(context.pending, context.byteLength);
  context.pending = [];
  context.byteLength = 0;
  context.tail = 0;
  return compressed;
}

/** Inflate part of a gathered packet, in order: a large one is inflated a slice at a time. Z_SYNC_FLUSH
 * ends a packet, not the zlib stream or its dictionary. Answers the bytes the slice inflated to. */
export function inflate(state: GatewayState, slice: Uint8Array): number {
  const context = state.inflater;
  if (!context) throw new Error('missing connection inflater');
  const before = context.out;
  context.stream.push(slice, false);
  return context.out - before;
}

/** The text of the packet inflated so far. */
export function inflated(state: GatewayState): string {
  const context = state.inflater;
  if (!context) throw new Error('missing connection inflater');
  if (context.carry) throw new Error('a packet ended inside a character');
  const text = context.text.join('');
  context.text.length = 0;
  return text;
}

function decode(state: GatewayState, data: ArrayBuffer | Uint8Array | string): unknown | undefined {
  if (typeof data === 'string') return JSON.parse(data);
  const packet = gather(state, data);
  if (!packet) return undefined;
  inflate(state, packet);
  return JSON.parse(inflated(state));
}

function status(result: StepResult, phase: GatewayPhase, detail?: string): void {
  result.state.phase = phase;
  result.changes.push({ kind: 'status', phase, ...(detail === undefined ? {} : { detail }) });
}

function forgetSession(state: GatewayState): void {
  state.sessionId = null;
  state.resumeGatewayUrl = null;
  state.seq = null;
}

function clearConnection(state: GatewayState): void {
  state.inflater = null;
  state.heartbeatIntervalMs = null;
  state.heartbeatAtMs = null;
  state.awaitingAck = false;
  state.lastHeartbeatAtMs = null;
  state.lastAckAtMs = null;
}

function beginConnection(result: StepResult): void {
  clearConnection(result.state);
  result.state.inflater = inflater();
  result.state.retryAtMs = null;
  status(result, 'connecting');
}

function resumable(state: GatewayState): boolean {
  return state.sessionId !== null && state.seq !== null;
}

/** The URL the next connection opens: the resume URL while the session can resume. */
export function connectionUrl(state: GatewayState): string {
  const url = new URL(resumable(state) && state.resumeGatewayUrl ? state.resumeGatewayUrl : state.gatewayUrl);
  for (const [name, value] of new URLSearchParams(state.identity.gatewayQuery)) {
    url.searchParams.set(name, value);
  }
  return url.toString();
}

function reconnect(result: StepResult, nowMs: number, detail: string, closeSocket: boolean, delayOverrideMs?: number): void {
  const state = result.state;
  if (state.phase === 'backoff' || state.phase === 'stopped') return;
  const base = Math.min(BACKOFF_CAP_MS, BACKOFF_BASE_MS * 2 ** Math.min(state.reconnectAttempts, 6));
  const delay = delayOverrideMs ?? Math.min(BACKOFF_CAP_MS, base * (1 + state.jitter));
  state.reconnectAttempts++;
  clearConnection(state);
  state.retryAtMs = nowMs + delay;
  status(result, 'backoff', detail);
  if (closeSocket) result.close = { code: RECONNECT_CLOSE, reason: detail };
  result.connect = { url: connectionUrl(state), atMs: state.retryAtMs };
}

function stop(result: StepResult, reason: string, closeSocket: boolean): void {
  const state = result.state;
  clearConnection(state);
  state.retryAtMs = null;
  state.stoppedReason = reason;
  status(result, 'stopped', reason);
  if (closeSocket) result.close = { code: RECONNECT_CLOSE, reason };
}

function heartbeat(result: StepResult, nowMs: number): void {
  const state = result.state;
  result.send.push(JSON.stringify({ op: 1, d: state.seq }));
  state.awaitingAck = true;
  state.lastHeartbeatAtMs = nowMs;
  if (state.heartbeatIntervalMs !== null) {
    state.heartbeatAtMs = nowMs + state.heartbeatIntervalMs;
  }
}

function hello(result: StepResult, interval: unknown, nowMs: number): void {
  const state = result.state;
  if (state.heartbeatIntervalMs !== null) return;
  if (typeof interval !== 'number' || !Number.isFinite(interval) || interval <= 0) {
    throw new Error('invalid heartbeat interval');
  }
  state.heartbeatIntervalMs = interval;
  state.heartbeatAtMs = nowMs + interval * state.jitter;
  state.awaitingAck = false;
  if (resumable(state)) {
    status(result, 'resuming');
    result.send.push(JSON.stringify({ op: 6, d: { token: state.token, session_id: state.sessionId, seq: state.seq } }));
    return;
  }
  if (state.identifies.length >= state.identifyBudget.max) {
    const window = state.identifyBudget.windowMs === HOUR_MS ? 'hour' : `${state.identifyBudget.windowMs} ms`;
    stop(result, `identify budget exhausted: ${state.identifies.length} in the last ${window}`, true);
    return;
  }
  state.identifies.push(nowMs);
  status(result, 'identifying');
  result.send.push(JSON.stringify({ op: 2, d: identifyPayload(state.identity, state.token) }));
}

function frame(result: StepResult, value: unknown, nowMs: number): void {
  if (!value || typeof value !== 'object' || !('op' in value) || typeof value.op !== 'number') {
    throw new Error('invalid gateway envelope');
  }
  const payload = value as { op: number; s?: unknown; t?: unknown; d?: any };
  const state = result.state;
  switch (payload.op) {
    case 0: {
      if (typeof payload.s !== 'number' || !Number.isSafeInteger(payload.s) || typeof payload.t !== 'string') {
        throw new Error('invalid dispatch envelope');
      }
      state.seq = payload.s;
      if (payload.t === 'READY') {
        if (typeof payload.d?.session_id !== 'string') throw new Error('missing READY session');
        state.sessionId = payload.d.session_id;
        state.resumeGatewayUrl = typeof payload.d.resume_gateway_url === 'string' ? payload.d.resume_gateway_url : null;
        if (typeof payload.d.auth_token === 'string' && payload.d.auth_token && payload.d.auth_token !== state.token) {
          state.token = payload.d.auth_token;
          result.changes.push({ kind: 'token', token: state.token });
        }
      }
      if (payload.t === 'READY' || payload.t === 'RESUMED') {
        state.reconnectAttempts = 0;
        state.retryAtMs = null;
        status(result, 'ready');
      }
      result.changes.push({ kind: 'dispatch', t: payload.t, s: payload.s, d: payload.d });
      break;
    }
    case 1:
      if (state.heartbeatIntervalMs !== null) heartbeat(result, nowMs);
      break;
    case 7:
      reconnect(result, nowMs, 'server requested reconnect', true);
      break;
    case 9:
      if (payload.d !== true) forgetSession(state);
      // INVALID_SESSION requires a random 1–5 s delay before a fresh identify.
      reconnect(result, nowMs, 'invalid session', true, payload.d === true ? undefined : 1_000 + 4_000 * state.jitter);
      break;
    case 10:
      hello(result, payload.d?.heartbeat_interval, nowMs);
      break;
    case 11:
      state.awaitingAck = false;
      state.lastAckAtMs = nowMs;
      break;
  }
}

const FATAL_CLOSE: Record<number, string> = {
  4004: 'authentication failed',
  4010: 'invalid shard',
  4011: 'sharding required',
  4012: 'invalid API version',
  4013: 'invalid intents',
  4014: 'disallowed intents',
};

// State is owned by one connection driver and consumed in place, including its inflater.
// The caller delivers only the current socket's events, executes sends/closes, and opens
// result.connect at/after atMs. A retry at that deadline starts the next attempt; its
// jitter seeds that attempt's first heartbeat and next backoff. connect/open can seed
// the initial jitter, and message.jitter supplies INVALID_SESSION's fresh random draw.
export function step(state: GatewayState, event: GatewayEvent): StepResult {
  if (!Number.isFinite(event.nowMs)) throw new Error('invalid gateway time');
  if ('jitter' in event && event.jitter !== undefined) {
    if (!Number.isFinite(event.jitter) || event.jitter < 0 || event.jitter >= 1) {
      throw new Error('gateway jitter must be in [0, 1)');
    }
  }
  const result: StepResult = { state, send: [], changes: [] };
  if (state.phase === 'stopped') return result;
  state.identifies = state.identifies.filter(time => time > event.nowMs - state.identifyBudget.windowMs);
  if ('jitter' in event && event.jitter !== undefined) state.jitter = event.jitter;
  switch (event.type) {
    case 'connect':
      if (state.retryAtMs !== null && event.nowMs < state.retryAtMs) {
        result.connect = { url: connectionUrl(state), atMs: state.retryAtMs };
      } else {
        beginConnection(result);
        result.connect = { url: connectionUrl(state), atMs: event.nowMs };
      }
      break;
    case 'open':
      if (state.phase === 'connecting') status(result, 'awaiting-hello');
      break;
    case 'message':
      if (state.phase === 'idle' || state.phase === 'connecting' || state.phase === 'backoff') break;
      try {
        const payload = decode(state, event.data);
        if (payload !== undefined) frame(result, payload, event.nowMs);
      } catch {
        // Never surface frame contents: malformed frames can include account data.
        reconnect(result, event.nowMs, 'invalid gateway payload', true);
      }
      break;
    case 'payload':
      if (state.phase === 'idle' || state.phase === 'connecting' || state.phase === 'backoff') break;
      try {
        if (event.invalid) throw new Error('invalid');
        frame(result, event.payload, event.nowMs);
      } catch {
        reconnect(result, event.nowMs, 'invalid gateway payload', true);
      }
      break;
    case 'close':
      if (FATAL_CLOSE[event.code]) {
        forgetSession(state);
        stop(result, `gateway close ${event.code}: ${FATAL_CLOSE[event.code]}`, false);
      } else {
        if ([1000, 1001, 4007, 4009].includes(event.code)) forgetSession(state);
        if (state.phase !== 'backoff' && state.phase !== 'idle') {
          reconnect(result, event.nowMs, `gateway close ${event.code}`, false);
        }
      }
      break;
    case 'heartbeat':
      if (!heartbeatDue(state, event.nowMs)) break;
      if (state.awaitingAck) reconnect(result, event.nowMs, 'heartbeat ACK missing', true);
      else heartbeat(result, event.nowMs);
      break;
    case 'retry':
      if (state.phase !== 'backoff' || state.retryAtMs === null) break;
      result.connect = { url: connectionUrl(state), atMs: Math.max(event.nowMs, state.retryAtMs) };
      if (event.nowMs >= state.retryAtMs) beginConnection(result);
      break;
  }
  return result;
}

export function heartbeatDue(state: GatewayState, nowMs: number): boolean {
  return state.heartbeatAtMs !== null && nowMs >= state.heartbeatAtMs;
}

export function nextHeartbeatAt(state: GatewayState): number | null {
  return state.heartbeatAtMs;
}

export function snapshot(state: GatewayState) {
  return {
    phase: state.phase,
    sessionId: state.sessionId,
    seq: state.seq,
    identifiesInWindow: state.identifies.length,
    identifies: [...state.identifies],
    stoppedReason: state.stoppedReason,
    heartbeatIntervalMs: state.heartbeatIntervalMs,
    nextHeartbeatAtMs: state.heartbeatAtMs,
    awaitingAck: state.awaitingAck,
    lastHeartbeatAtMs: state.lastHeartbeatAtMs,
    lastAckAtMs: state.lastAckAtMs,
    retryAtMs: state.retryAtMs,
    reconnectAttempts: state.reconnectAttempts,
  };
}
