// reset-credits.ts: resetCreditsSummary and the redeem reducer (useResetCredit's state). The
// reference has no test for either; these follow UsageLimits.tsx:203-306 line by line.
import { describe, expect, test } from 'bun:test';
import { OUTCOME_TEXT, REDEEM_IDLE, redeemStep, resetCreditsShown, resetCreditsSummary } from './reset-credits';

const now = Date.parse('2026-09-03T12:00:00.000Z');

describe('resetCreditsSummary', () => {
  test('counts the banked credits and when the next one expires', () => {
    expect(resetCreditsSummary({ availableCount: 2, nextExpiresAt: '2026-10-01T11:00:00.000Z' }, now)).toBe('2 reset credits banked · next expires in 27d 23h');
    expect(resetCreditsSummary({ availableCount: 1, nextExpiresAt: '2026-09-03T14:30:00.000Z' }, now)).toBe('1 reset credit banked · next expires in 2h 30m');
    expect(resetCreditsSummary({ availableCount: 3 }, now)).toBe('3 reset credits banked');
  });
  test('has a short form for a popover and says when none is banked', () => {
    expect(resetCreditsSummary({ availableCount: 2, nextExpiresAt: '2026-10-01T11:00:00.000Z' }, now, true)).toBe('2 banked · expires in 27d 23h');
    expect(resetCreditsSummary({ availableCount: 0, nextExpiresAt: '2026-10-01T11:00:00.000Z' }, now)).toBe('No reset credits banked');
  });
});

describe('redeemStep', () => {
  test('Use reset asks; Cancel closes the confirm and sends nothing', () => {
    const asked = redeemStep(REDEEM_IDLE, { type: 'ask' });
    expect(asked).toEqual({ confirming: true, busy: false, status: null });
    expect(redeemStep(asked, { type: 'cancel' })).toEqual(REDEEM_IDLE);
  });
  test('Use credit closes the confirm, is busy and clears the last outcome', () => {
    const before = { confirming: true, busy: false, status: 'Nothing to reset right now.' };
    expect(redeemStep(before, { type: 'start' })).toEqual({ confirming: false, busy: true, status: null });
  });
  test('each outcome has its words; a warning wins over them', () => {
    const busy = redeemStep(REDEEM_IDLE, { type: 'start' });
    for (const [outcome, text] of Object.entries(OUTCOME_TEXT)) expect(redeemStep(busy, { type: 'done', outcome })).toEqual({ confirming: false, busy: false, status: text });
    expect(redeemStep(busy, { type: 'done', outcome: 'reset', warning: 'Redeemed, but the hub cooldown could not be cleared.' }).status).toBe('Redeemed, but the hub cooldown could not be cleared.');
  });
  test("a failure says the server's message, else the generic line", () => {
    const busy = redeemStep(REDEEM_IDLE, { type: 'start' });
    expect(redeemStep(busy, { type: 'failed', message: 'This provider does not bank reset credits.' }).status).toBe('This provider does not bank reset credits.');
    expect(redeemStep(busy, { type: 'failed', message: null })).toEqual({ confirming: false, busy: false, status: 'Could not use the reset credit.' });
  });
  test('the row hides at zero credits until an outcome shows', () => {
    expect(resetCreditsShown({ availableCount: 0 }, REDEEM_IDLE)).toBe(false);
    expect(resetCreditsShown({ availableCount: 0 }, { ...REDEEM_IDLE, status: 'No reset credit left.' })).toBe(true);
    expect(resetCreditsShown({ availableCount: 1 }, REDEEM_IDLE)).toBe(true);
  });
});
