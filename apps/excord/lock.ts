// Lock Excord (LLP 1133.010 §9): the token, which is the whole account, kept behind the person's check. The
// grant line keeps every write of it in the state the person chose, the rotated token's included, so this file
// only reads it behind the check and moves it between locked and plain; the host makes each move.
import type { Store as Secrets } from './app.contract.d.ts';
import { tokenKey, type Live } from './live';

type Ack = { ok: boolean; error: string };
const OK: Ack = { ok: true, error: '' };
const refused = (error: unknown): Ack => ({ ok: false, error: error instanceof Error ? error.message : String(error) });
const REASON = { reason: 'reason.unlock' };

/** The launch's unlock: the token behind the check, into module state; the session answers again. `null`
 * means the platform dropped it (the passcode removed, say): the person signs in again. */
export async function unlockSession(live: Live, secrets: Secrets): Promise<Ack> {
  try {
    const token = await secrets.unlock(tokenKey(live.origins), REASON);
    if (token === null) {
      live.signedOut = true;
      return { ok: false, error: 'Sign in again' };
    }
    live.token = token;
    return OK;
  } catch (error) {
    return refused(error);
  }
}

/** Turn the lock on: the host moves the token to locked; the switch shows on once this resolves. */
export async function protectToken(live: Live, secrets: Secrets): Promise<Ack> {
  try { await secrets.protect(tokenKey(live.origins)); return OK; } catch (error) { return refused(error); }
}

/** Turn the lock off: the person's check, then the host moves the token to plain. */
export async function unprotectToken(live: Live, secrets: Secrets): Promise<Ack> {
  try { await secrets.unprotect(tokenKey(live.origins), REASON); return OK; } catch (error) { return refused(error); }
}

/** The fixture's account signs in again when the person asks; Discord's sign-in is stage 2's. */
export function signInAgain(live: Live, secrets: Secrets): Ack {
  if (!live.origins.fixture) return { ok: false, error: 'Sign in with Discord is not built yet' };
  live.signedOut = false;
  try { secrets.set(tokenKey(live.origins), 'fixture-token'); return OK; } catch (error) { return refused(error); }
}
