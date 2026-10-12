// Slow request notice. The first half is a port of the reference's request latency state
// (MIT reference at 1e2ecbd975: apps/web/src/rpc/requestLatencyState.ts, tests in
// requestLatencyState.test.ts; the toast in components/SlowRpcRequestToastCoordinator.tsx and the
// observer in connection/platform.ts rpcRequestObserver). Each unary RPC is timed from the moment
// it is sent and ends with its own reply: 15 s, or 120 s for server.updateProvider,
// server.refreshProviders and server.updateServer. One request past its threshold joins a single
// "Some requests are slow" warning that is updated in place and closed when the last one ends.
//
// exact2-required changes to the port:
//  - There are no timers and no `Date.now()`. Every function takes `now`, and a request becomes
//    slow when `getSlowRpcAckRequests(state, now)` finds `startedAtMs + thresholdMs <= now`. The
//    shell's wall time (shellView's `now`) is the only clock.
//  - The state is a value (`createRequestLatencyState`), one per client, so a test or a second
//    window never shares requests. `trackRpcRequestSent` takes `now` before the optional `tag`.
//  - The reference ends a request with `Effect.ensuring` when its session closes or its fiber is
//    interrupted; an Exact answer that is let go never resumes, so its `finally` never runs. The
//    client side below ends the requests of an environment that is switched off, disconnected or
//    replaced instead (`slowRequests`).
//  - A request ends at the latest when T3Transport has failed it: the transport interrupts any RPC
//    that is still pending after 30 s (its timer ticks each second) and sends the failure to the
//    answer that asked. A let-go answer never sees that reply, so `slowRequests` ends the request
//    itself once it is older than the transport allows (TRANSPORT_REQUEST_DEADLINE_MS).
//  - A request is timed from the first shell time that is newer than the one the shell had when it
//    was sent. A data source has no clock, and the shell's time stands still while nothing ticks,
//    so the last shell time can be minutes old: using it as the start made a new request look
//    several minutes old (the false "waiting longer than 15s" toast after A off and B on).
import type { T3Client } from './client';
import { pushToast, dismissToast, updateToast, toasts } from './toast';
import { obj, str } from './domain';

export const SLOW_RPC_ACK_THRESHOLD_MS = 15_000;
/**
 * Some requests are slow by design: they shell out to a package manager on the server and only
 * respond once the install finishes. Warning about those after 15 s is noise, so they get a much
 * longer leash.
 */
export const LONG_RUNNING_RPC_ACK_THRESHOLD_MS = 120_000;
export const MAX_TRACKED_RPC_ACK_REQUESTS = 256;
/** T3Transport.rpc: a request not answered in 30 s is interrupted and failed by a timer that ticks each second. */
export const TRANSPORT_REQUEST_DEADLINE_MS = 31_000;

export interface SlowRpcAckRequest {
  readonly requestId: string;
  readonly startedAt: string;
  readonly startedAtMs: number;
  readonly tag: string;
  readonly thresholdMs: number;
}
export interface RequestLatencyState { pending: Map<string, SlowRpcAckRequest>; slow: SlowRpcAckRequest[] }

const UNTRACKED_RPC_ACK_METHODS = new Set(['previewAutomation.connect', 'server.getUsageSummary']);
const LONG_RUNNING_RPC_ACK_METHODS = new Set(['server.updateProvider', 'server.refreshProviders', 'server.updateServer']);

export const createRequestLatencyState = (): RequestLatencyState => ({ pending: new Map(), slow: [] });

export function shouldTrackRpcAck(method: string): boolean {
  return !method.includes('subscribe') && !method.startsWith('pullRequests.') && !UNTRACKED_RPC_ACK_METHODS.has(method);
}
export function rpcAckThresholdMs(method: string): number {
  return LONG_RUNNING_RPC_ACK_METHODS.has(method) ? LONG_RUNNING_RPC_ACK_THRESHOLD_MS : SLOW_RPC_ACK_THRESHOLD_MS;
}

/**
 * Starts the slow-request timer for one in-flight unary RPC. `method` is the bare WS method (used
 * to decide whether and how long to wait); `tag` is the human-readable label shown in the toast,
 * which defaults to the method.
 */
export function trackRpcRequestSent(state: RequestLatencyState, requestId: string, method: string, now: number, tag = method): void {
  if (!shouldTrackRpcAck(method)) return;
  clearTrackedRpcRequest(state, requestId);
  evictOldestPendingRpcRequestIfNeeded(state);
  state.pending.set(requestId, { requestId, startedAt: new Date(now).toISOString(), startedAtMs: now, tag, thresholdMs: rpcAckThresholdMs(method) });
}

export function acknowledgeRpcRequest(state: RequestLatencyState, requestId: string): void {
  clearTrackedRpcRequest(state, requestId);
  if (!state.slow.some(request => request.requestId === requestId)) return;
  state.slow = state.slow.filter(request => request.requestId !== requestId);
}

/** The requests past their threshold at `now`, in the order their timers would have fired. */
export function getSlowRpcAckRequests(state: RequestLatencyState, now: number): ReadonlyArray<SlowRpcAckRequest> {
  const due = [...state.pending.values()].map((request, order) => ({ request, order, at: request.startedAtMs + request.thresholdMs }))
    .filter(entry => entry.at <= now).sort((a, b) => a.at - b.at || a.order - b.order);
  for (const { request } of due) {
    state.pending.delete(request.requestId);
    appendSlowRpcAckRequest(state, request);
  }
  return [...state.slow];
}

export function resetRequestLatencyStateForTests(state: RequestLatencyState): void {
  state.pending.clear();
  state.slow = [];
}

function clearTrackedRpcRequest(state: RequestLatencyState, requestId: string): void { state.pending.delete(requestId); }
function appendSlowRpcAckRequest(state: RequestLatencyState, request: SlowRpcAckRequest): void {
  const requests = [...state.slow, request];
  state.slow = requests.length <= MAX_TRACKED_RPC_ACK_REQUESTS ? requests : requests.slice(-MAX_TRACKED_RPC_ACK_REQUESTS);
}
function evictOldestPendingRpcRequestIfNeeded(state: RequestLatencyState): void {
  while (state.pending.size >= MAX_TRACKED_RPC_ACK_REQUESTS) {
    const oldest = state.pending.keys().next().value;
    if (oldest === undefined) return;
    clearTrackedRpcRequest(state, oldest);
  }
}

// ── The client's side ─────────────────────────────────────────────────────

/**
 * One request the client sent. `anchor` is the shell time when it was sent (null when the shell had
 * none yet): the clock starts at the first shell time after it, `startedAt`, and the latency state
 * holds the request from then on.
 */
type Entry = { requestId: string; method: string; tag: string; environmentId: string; generation: number; anchor: number | null; startedAt: number | null };
type SlowState = { latency: RequestLatencyState; entries: Map<string, Entry>; toastId: number | null; lastNow: number };
const states = new WeakMap<T3Client, SlowState>();
let nextId = 1;
function slowState(client: T3Client): SlowState {
  let state = states.get(client);
  if (!state) { state = { latency: createRequestLatencyState(), entries: new Map(), toastId: null, lastNow: 0 }; states.set(client, state); }
  return state;
}
function end(state: SlowState, requestId: string): void {
  acknowledgeRpcRequest(state.latency, requestId);
  state.entries.delete(requestId);
}
function start(state: SlowState, entry: Entry, now: number): void {
  entry.startedAt = now;
  trackRpcRequestSent(state.latency, entry.requestId, entry.method, now, entry.tag);
}

/** The acknowledgement `trackRpc` returns; `requestId` names the request for `isTracked`. */
export type RpcAck = (() => void) & { requestId: string };

/**
 * rpcRequestObserver for one bridge request; returns its acknowledgement. Only `request` ops are
 * WebSocket RPCs; HTTP reads and local ops are not. With `now` (a test, or a caller that has the
 * shell's time) the clock starts at once; without it, at the first shell time newer than the one
 * the shell had now.
 */
export function trackRpc(client: T3Client, request: unknown, now?: number): RpcAck {
  const value = obj(request);
  const method = str(value.method);
  if (value.op !== 'request' || !method || !shouldTrackRpcAck(method)) return Object.assign(() => undefined, { requestId: '' });
  const state = slowState(client);
  const requestId = `${client.environmentId}:${nextId++}`;
  const entry: Entry = { requestId, method, tag: `${method} · ${client.environmentId}`, environmentId: client.environmentId, generation: client.generation, anchor: state.lastNow || null, startedAt: null };
  state.entries.set(requestId, entry);
  while (state.entries.size > MAX_TRACKED_RPC_ACK_REQUESTS) end(state, state.entries.keys().next().value!);
  if (typeof now === 'number' && Number.isFinite(now) && now > 0) start(state, entry, now);
  return Object.assign(() => end(state, requestId), { requestId });
}

/** Whether a request is still tracked (a reply, a status read or its environment's end has not ended it). */
export function isTracked(client: T3Client, requestId: string): boolean {
  return slowState(client).entries.has(requestId);
}

/** Whether a request is still being timed (the window keeps its clock running). */
export function tracking(client: T3Client): boolean {
  return slowState(client).entries.size > 0;
}

/**
 * A request sent to an environment that is no longer this client's, under a connection that has
 * since been replaced, or while the client is not connected: its socket is gone, so no reply will
 * come. The reference ends it when the session closes.
 */
function environmentGone(client: T3Client, entry: Entry): boolean {
  return entry.environmentId !== client.environmentId || entry.generation !== client.generation || client.connection !== 'connected';
}

/** describeSlowRequests: the smallest threshold the batch has passed, in seconds. */
export function describeSlow(requests: { thresholdMs: number }[]): string {
  const count = requests.length;
  const seconds = Math.round(Math.min(...requests.map(request => request.thresholdMs)) / 1000);
  return `${count} request${count === 1 ? '' : 's'} waiting longer than ${seconds}s.`;
}

/** new Date(startedAt).toLocaleTimeString() as en-US prints it. */
export function startedLabel(at: number): string {
  const date = new Date(at);
  const hours = date.getHours(), minutes = date.getMinutes(), seconds = date.getSeconds();
  const pad = (value: number) => String(value).padStart(2, '0');
  return `Started ${hours % 12 || 12}:${pad(minutes)}:${pad(seconds)} ${hours < 12 ? 'AM' : 'PM'}`;
}

/** Advance the clocks to `now` and keep the one warning in step with the slow set. */
export function slowRequests(client: T3Client, now: number): void {
  if (!Number.isFinite(now) || now <= 0) return;
  const state = slowState(client);
  for (const entry of [...state.entries.values()]) {
    if (environmentGone(client, entry) || (entry.startedAt !== null && now - entry.startedAt >= TRANSPORT_REQUEST_DEADLINE_MS)) end(state, entry.requestId);
    else if (entry.startedAt === null && (entry.anchor === null || now > entry.anchor)) start(state, entry, now);
  }
  state.lastNow = Math.max(state.lastNow, now);
  const slow = getSlowRpcAckRequests(state.latency, now);
  const live = state.toastId !== null && toasts(client).some(toast => toast.id === state.toastId);
  if (slow.length === 0) {
    if (live) dismissToast(client, state.toastId!);
    state.toastId = null;
    return;
  }
  const content = {
    description: describeSlow([...slow]),
    details: slow.map(entry => ({ id: entry.requestId, title: entry.tag, subtitle: startedLabel(entry.startedAtMs) })),
    expandLabels: { expand: 'Show requests', collapse: 'Hide requests' },
  };
  // Closed by its × while requests are still slow: it stays closed until the set empties.
  if (state.toastId === null) state.toastId = pushToast(client, { kind: 'warning', title: 'Some requests are slow', timeoutMs: 0, ...content });
  else if (live) updateToast(client, state.toastId, content);
}
