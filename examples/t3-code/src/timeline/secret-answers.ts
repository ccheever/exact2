// SecretRequestCard.tsx at T3 Code 9c161462c9 (MIT, LICENSE-T3): answers bypass persisted operations.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { wakeShell } from '../client/command-waits';
import { secretRequestAnswerInput, secretRequestFailureMessage } from './secret-request';

type Reply = { pending: boolean; submitted: boolean; error: string };
type Answer = Reply & { owner: string; thread: string; item: string; answer: { type: 'save'; secret: string } | { type: 'decline' } | null; claimed: boolean };
const states = new WeakMap<T3Client, Map<string, Answer>>();
const identity = (client: T3Client) => JSON.stringify([client.generation, client.environmentId, client.origin]);
const responses = (client: T3Client) => { let value = states.get(client); if (!value) { value = new Map(); states.set(client, value); } return value; };
export const secretAnswerKey = (client: T3Client, thread: string, item: string) => JSON.stringify([identity(client), thread, item]);
export function secretAnswerState(client: T3Client, thread: string, item: string): Reply {
  const state = responses(client).get(secretAnswerKey(client, thread, item));
  return state ? { pending: state.pending, submitted: state.submitted, error: state.error } : { pending: false, submitted: false, error: '' };
}
/** Queue only in memory. Neither the secret nor a retry payload enters client preferences or the transcript. */
export function enqueueSecretAnswer(client: T3Client, key: string, decision: string, secret: string): void {
  if (!client.writable) throw new ClientError(client.connection !== 'connected' ? 'Reconnect before answering.' : 'Wait for synchronization and check your connection permissions.');
  const row = arr(client.projection.visibleTurnItems).find(row => secretAnswerKey(client, str(row.sourceThreadId), str(row.sourceItemId)) === key);
  const item = obj(row?.item);
  if (!row || row.visibility !== 'local' || item.type !== 'secret_request' || item.secretStatus !== 'pending') throw new ClientError('This secret request no longer accepts an answer.');
  const current = responses(client).get(key);
  if (current?.pending || current?.submitted) return;
  const answer = secretRequestAnswerInput({ id: str(item.id), threadId: str(item.threadId) }, decision === 'decline' ? { type: 'decline' } : { type: 'save', secret });
  if (!answer) return;
  responses(client).set(key, { owner: identity(client), thread: answer.threadId, item: answer.turnItemId, answer: answer.answer, pending: true, claimed: false, submitted: false, error: '' });
}
export function secretAnswersNeeded(client: T3Client): boolean {
  const values = responses(client), owner = identity(client);
  for (const [key, state] of values) {
    if (state.owner !== owner) { state.answer = null; values.delete(key); continue; }
    if (state.pending && !state.claimed) return true;
  }
  return false;
}
/** A root read owns each request. Independent read slots can answer separate cards concurrently. */
export async function refreshSecretAnswer(client: T3Client, native: Native): Promise<boolean> {
  const values = responses(client), owner = identity(client);
  for (const [key, state] of values) {
    if (state.owner !== owner) { state.answer = null; values.delete(key); continue; }
    if (!state.pending || state.claimed || !state.answer) continue;
    state.claimed = true;
    const payload = { threadId: state.thread, turnItemId: state.item, answer: state.answer };
    // Retire the queued copy before the native boundary; only this invocation holds the value now.
    state.answer = null;
    try {
      await client.rpc(native, 'secrets.answerRequest', payload, true);
      state.submitted = true;
    } catch (error) {
      state.error = secretRequestFailureMessage(error instanceof ClientError ? { _tag: error.kind, message: error.message } : error);
    } finally {
      state.pending = false;
      // Clear a save's payload as soon as its request settles, including failures.
      if (payload.answer.type === 'save') payload.answer.secret = '';
      await wakeShell(native);
    }
    return true;
  }
  return false;
}
