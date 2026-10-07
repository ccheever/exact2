// Abandon-safe plumbing for answers Exact may let go mid-flight (lane r3-protocol).
//
// Since exact2 #183 (#109) a watched topic that changes while an answer awaits a
// native reply lets that reply land and then asks once more, so the snapshot
// read no longer tags its requests and no native gate holds its topics. An
// answer Exact replaces for another reason (new arguments, a refresh) still
// never receives its pending native replies (LLP 1016 D5), so:
//  - Each WebSocket RPC carries a trace id the transport remembers while the
//    request is pending. A status read lists the pending traces, and any traced
//    call the transport already answered is acknowledged even though its reply
//    was dropped, so "Some requests are slow" counts only requests that are
//    really still waiting at the server. A status read also ends the traces of
//    an environment that is no longer the transport's, or of a transport that
//    is not connected: the transport failed those requests when its socket
//    closed, and the reference ends a request when its session closes.
import { bridgeReply, type Native } from './protocol';
import { obj, str, type Obj } from './domain';
import { trackRpc, isTracked } from './shell-slow';
import type { T3Client } from './client';

type Live = { acknowledge: () => void; requestId: string; environmentId: string; issuedAt: number };
type Traces = { serial: number; statusReads: number; live: Map<number, Live> };
const traces = new WeakMap<object, Traces>();
function state(client: object): Traces {
  let value = traces.get(client);
  if (!value) { value = { serial: 0, statusReads: 0, live: new Map() }; traces.set(client, value); }
  return value;
}

/** trackRpc plus a trace id the transport keeps while the request is pending. */
export function traceRpc(client: T3Client, request: unknown): { request: Obj; done: () => void } {
  const value = obj(request);
  if (value.op !== 'request') return { request: value, done: trackRpc(client, value) };
  const traced = state(client), trace = ++traced.serial;
  const acknowledge = trackRpc(client, value);
  traced.live.set(trace, { acknowledge, requestId: acknowledge.requestId, environmentId: client.environmentId, issuedAt: traced.statusReads });
  return { request: { ...value, trace }, done: () => { traced.live.delete(trace); acknowledge(); } };
}

/** Call before issuing a status read; pass the ticket to `settleTraces` with its reply. */
export function statusTicket(client: object): number { return ++state(client).statusReads; }

/**
 * A traced call issued before the previous status read, and not pending at the
 * transport now, was answered: its reply went to an answer Exact let go. A call
 * sent to another environment than the status names, or while the status says
 * the transport is not connected, has no socket left to answer it: the
 * transport failed it when the socket closed. Neither waits for a second read.
 */
export function settleTraces(client: T3Client, status: Obj, ticket: number): number {
  const traced = state(client);
  const environmentId = str(status.environmentId), disconnected = typeof status.state === 'string' && status.state !== 'connected';
  const pending = Array.isArray(status.traces) ? new Set(status.traces.filter((entry): entry is number => typeof entry === 'number')) : null;
  let settled = 0;
  for (const [trace, entry] of [...traced.live]) {
    if (!isTracked(client, entry.requestId)) { traced.live.delete(trace); continue; } // already ended: slowRequests ended it with its environment
    const gone = entry.issuedAt < ticket && (disconnected || (environmentId !== '' && entry.environmentId !== environmentId));
    const answered = pending !== null && entry.issuedAt < ticket - 1 && !pending.has(trace);
    if (!gone && !answered) continue;
    traced.live.delete(trace); entry.acknowledge(); settled++;
  }
  return settled;
}

/**
 * A status read for the traces still live, ending those the transport no longer lists. A refresh reads
 * the status only when a topic changes, so a quiet window keeps an answered request whose answer was
 * let go until the next refresh (or until T3Transport's 30 s deadline, shell-slow.ts). Nothing calls
 * this yet: shellView would, on each tick while `tracking(client)` (lane r13-slow report, shell.ts hunk).
 */
export async function settleLiveTraces(client: T3Client, native: Native): Promise<number> {
  if (!traces.get(client)?.live.size) return 0;
  const ticket = statusTicket(client);
  try {
    const status = await bridgeReply(native, { op: 'status' });
    return status.ok ? settleTraces(client, obj(status.value), ticket) : 0;
  } catch { return 0; }
}
