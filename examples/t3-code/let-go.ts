// A request Exact let go is not a failure (reference: apps/web/src/components/
// settings/ThemeSearchSection.tsx and OpenSourceLicenses.tsx, whose catch sets
// an error only `if (!controller.signal.aborted)`: a request a newer one
// replaced shows nothing).
//
// Exact lets an answer go when its resource asks again or a newer send replaces
// a mutation (LLP 1016 D5). Natively, the native calls that answer waits on then
// reject with a FetchError of kind "Aborted" ("the answer was let go before this
// reply; the request may already have been sent"), so its `catch` and `finally`
// run; the reply is dropped either way (docs/agent-pitfalls.md, "A superseded
// send's fetch rejects natively"). The clone already has a name for a request
// that lost its turn, ClientError kind 'superseded' (client.ts `ownedNative`).
// `letGoAware` turns the runtime's rejection into that one at the native seam,
// once per answer (app.ts), and `letGo` is the one test every catch that writes
// an error, a toast or a failure field makes first. A let-go write is not
// retried (it may have been sent) and not marked failed: its outcome comes
// back with the shell, as a send's does.
import { ClientError, type Native } from './protocol';

/**
 * A read whose result is kept (a workspace's status or refs) is sent once for its key, and again
 * only after RESEND_MS of wall time. Exact lets an answer go when it is asked again (every 500 ms
 * while a toast ticks), and its reply is dropped; a reply slower than the tick was otherwise asked
 * for on every tick (round 5: vcs.refreshStatus four times a second). Returns the mark to keep, or
 * null when this answer must not send.
 */
export type SentRead = { key: string; at: number };
export const RESEND_MS = 3000;
export function sendRead(sent: SentRead | undefined, key: string, now: number): SentRead | null {
  if (sent && sent.key === key && !(now > 0 && now - sent.at >= RESEND_MS)) return null;
  return { key, at: Math.max(now, 0) };
}

/** The runtime's rejection of a let-go answer's native call, or the clone's 'superseded'. */
export function letGo(error: unknown): boolean {
  if (error instanceof ClientError) return error.kind === 'superseded';
  const fields = error !== null && typeof error === 'object' ? error as { name?: unknown; kind?: unknown } : {};
  return fields.name === 'FetchError' && fields.kind === 'Aborted';
}

/**
 * `native` whose calls reject with ClientError 'superseded' once Exact lets the
 * answer go. The whole answer is let go, so a call or a watch its `catch` or
 * `finally`, or the code after a catch that swallowed the rejection, makes
 * afterwards is refused here too: natively it would run outside any answer and
 * be refused with a plain Error the catches could not tell apart ("native.watch
 * outside an answer" became the transcript banner when a refresh let go inside
 * fleet.sync's tolerated `environments` read went on to read the embedded server).
 */
export function letGoAware(native: Native): Native {
  let gone = '';
  const superseded = () => new ClientError(gone, 'superseded');
  return { available: native.available, watch: topic => { if (gone) throw superseded(); native.watch(topic); }, later: async request => {
    if (gone) throw superseded();
    try { return await native.later(request); }
    catch (error) {
      if (!letGo(error)) throw error;
      gone = error instanceof Error && error.message ? error.message : 'This operation was superseded.';
      throw superseded();
    }
  } };
}
