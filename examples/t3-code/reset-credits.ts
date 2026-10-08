// Banked reset credits and their redemption, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/usage/UsageLimits.tsx (OUTCOME_TEXT, useResetCredit, ResetCredits,
// resetCreditsSummary). Changes: useResetCredit's three useState hooks (confirming, busy, status)
// are one state value changed by `redeemStep`, a pure reducer, so the composer banner and the
// Usage page can keep it per opened panel (composer-controls-usage.ts) and tests can drive it;
// the command result is the clone's (`ok` with the outcome and warning, or the error's message
// when the server answered a typed error, else none). `now` is an argument, as in the reference.
import { formatDuration } from './usage-limits';
import { num, str, type Obj } from './domain';

export type ResetCreditOutcome = 'reset' | 'nothingToReset' | 'noCredit' | 'alreadyRedeemed';

export const OUTCOME_TEXT: Record<ResetCreditOutcome, string> = {
  reset: 'Reset applied. Your windows have cleared.',
  nothingToReset: 'Nothing to reset right now.',
  noCredit: 'No reset credit left.',
  alreadyRedeemed: 'That credit was already redeemed.',
};

/** `2 reset credits banked · next expires in 27d 23h`, or the short form for a popover. */
export function resetCreditsSummary(credits: Obj, now: number, compact = false): string {
  const expiresIn = typeof credits.nextExpiresAt === 'string' ? formatDuration(Date.parse(credits.nextExpiresAt) - now) : null;
  const count = num(credits.availableCount);
  if (count === 0) return 'No reset credits banked';
  if (compact) return `${count} banked${expiresIn ? ` · expires in ${expiresIn}` : ''}`;
  return `${count} ${count === 1 ? 'reset credit' : 'reset credits'} banked${expiresIn ? ` · next expires in ${expiresIn}` : ''}`;
}

/** useResetCredit's state: the confirm open, a redeem in flight, the last outcome's words. */
export type RedeemState = { readonly confirming: boolean; readonly busy: boolean; readonly status: string | null };
export const REDEEM_IDLE: RedeemState = { confirming: false, busy: false, status: null };

export type RedeemEvent =
  | { readonly type: 'ask' }
  | { readonly type: 'cancel' }
  | { readonly type: 'start' }
  | { readonly type: 'done'; readonly outcome: string; readonly warning?: string }
  | { readonly type: 'failed'; readonly message: string | null };

/**
 * setConfirming / redeem: "Use reset" asks; Cancel (or Escape) closes; "Use credit" closes the
 * confirm, sets busy and clears the old status; the reply sets the warning if the server sent
 * one, else the outcome's words; a failure says the error's message, else the generic line.
 */
export function redeemStep(state: RedeemState, event: RedeemEvent): RedeemState {
  switch (event.type) {
    case 'ask': return { ...state, confirming: true };
    case 'cancel': return { ...state, confirming: false };
    case 'start': return { confirming: false, busy: true, status: null };
    case 'done': return { ...state, busy: false, status: str(event.warning) || (OUTCOME_TEXT[event.outcome as ResetCreditOutcome] ?? null) };
    case 'failed': return { ...state, busy: false, status: event.message ?? 'Could not use the reset credit.' };
  }
}

/** ResetCredits: the row is hidden when nothing is banked and no outcome is showing. */
export function resetCreditsShown(credits: Obj, state: RedeemState): boolean {
  return !(num(credits.availableCount) === 0 && state.status === null);
}
