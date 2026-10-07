// Long composer writes that must not hold the window: a `/feedback` upload and a reset credit's
// redemption. In T3 Code each is an awaited atom command while the rest of the UI stays live
// (ChatView.tsx onSend's submitCodexFeedback; UsageLimits.tsx useResetCredit). The native data
// executor answers one request at a time, so a source answer that waited on such a reply held
// every other answer (the draft, the transcript, the "Sending feedback" state) until it came.
// Here the command only starts the request: T3Transport sends it and files the reply in the
// inbox under `composer-reply:<id>` (the rpc `deliver` key, T3Transport.swift) and wakes
// `t3.events`; the next snapshot's drain hands it to the waiting promise. A reply from an older
// connection never comes (the inbox resets), so a waiter of an older generation settles as
// interrupted the next time the composer is drawn.
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { obj, str, type Obj } from './domain';

export type DetachedReply = { ok: true; value: Obj } | { ok: false; error: ClientError; interrupted: boolean };
type Waiter = { generation: number; resolve: (reply: DetachedReply) => void };
const waiters = new WeakMap<object, Map<string, Waiter>>();
const KEY = 'composer-reply:';
let serial = 0;
const table = (client: object) => { let map = waiters.get(client); if (!map) waiters.set(client, map = new Map()); return map; };

/**
 * Sends `method` and returns at once with a promise of its reply (wrapped: an async function's
 * returned promise would be adopted and awaited). A request the transport refuses (not
 * connected) settles the promise as that failure, as an atom command's failure would.
 */
export async function startDetached(client: T3Client, native: Native, method: string, payload: Obj): Promise<{ reply: Promise<DetachedReply> }> {
  const key = `${KEY}${++serial}`, generation = client.generation;
  const reply = new Promise<DetachedReply>(resolve => table(client).set(key, { generation, resolve }));
  // The reference's command has no deadline; the transport keeps a delivered request up to five minutes.
  try { await client.call(native, { op: 'request', method, payload, deliver: key, timeout: 300 }, generation, true); }
  catch (error) {
    const waiter = table(client).get(key);
    table(client).delete(key);
    const failure = error instanceof ClientError ? error : new ClientError(error instanceof Error ? error.message : String(error));
    waiter?.resolve({ ok: false, error: failure, interrupted: failure.kind === 'superseded' || failure.kind === 'stale' });
  }
  return { reply };
}

/** One `composer-reply:` inbox entry (client.ts drain): the reply, or its typed failure. */
export function composerReplyEvent(client: T3Client, entry: Obj): boolean {
  const key = str(entry.key);
  if (!key.startsWith(KEY)) return false;
  const waiter = table(client).get(key);
  if (!waiter) return true;
  table(client).delete(key);
  const item = obj(entry.value);
  if (item._reply !== undefined) { waiter.resolve({ ok: true, value: obj(item._reply) }); return true; }
  const error = obj(item._replyError);
  const kind = str(error.kind, 'RPC');
  waiter.resolve({ ok: false, error: new ClientError(str(error.message, 'The server request failed.'), kind, error.uncertain === true, { reason: str(error.reason), detail: str(error.detail) }),
    interrupted: kind === 'Interrupt' || kind === 'Disconnected' });
  return true;
}

/** Waiters of an older connection: their replies never come, so they end as interrupted. */
export function settleStaleReplies(client: T3Client): void {
  for (const [key, waiter] of table(client)) {
    if (waiter.generation === client.generation) continue;
    table(client).delete(key);
    waiter.resolve({ ok: false, error: new ClientError('The connection changed. Refresh before continuing.', 'stale'), interrupted: true });
  }
}
