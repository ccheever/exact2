// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r9-connect-onboarding.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r9-connect: a stored onboardingCompletedAt older than T3 Code itself is
// not a completion time. Builds before r8-pointer's D16 fix saved seconds since
// launch as an epoch (1970-01-01T00:01:58Z); such a value reads as unset, so the
// welcome decision stores the real time the next time it records completion.
// T3 Code's first commit (f194c9661c, "Monorepo electron init") is 2026-02-07.
//
// r13-store (F3) — what is done with the old values, recorded here and in the
// round-13 report:
//  - The WRITER is fixed (pages-welcome.ts, r8-pointer-clock.ts, app.contract's
//    finish/import commands): a completion time is the window's wall time, or
//    nothing. Round 8 saved `1970-01-01T00:01:58.501Z` and the round-12 real
//    first launch `1970-01-01T00:01:37Z`: both were `now()` (ms since launch)
//    passed as if it were an epoch. No new 1970 value can be written.
//  - Old values are NOT deleted or rewritten here. This reader keeps the r9
//    rule (a stamp before the app existed reads as unset), which is the one
//    place an old value is hidden. The reference has no such rule (any stored
//    string means "completed", FirstRunGate/firstRun.ts), so the difference is
//    kept narrow: a client with a saved environment records the real time at
//    its next welcome decision, at once and without showing the wizard; a
//    client with NO saved environment and an old stamp sees the wizard again,
//    where the reference would open the app. Dropping this rule instead would
//    keep 1970 on disk as the completion time; it needs r9-connect.test.ts's
//    "stored 1970 onboarding time is repaired" expectations changed.

/** The earliest instant a completion of this app's onboarding can carry. */
export const APP_EPOCH = Date.parse('2026-02-07T00:00:00.000Z');

const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/;

/** The stored completion time, or '' when it is missing, malformed or predates the app. */
export function storedCompletion(value: unknown): string {
  if (typeof value !== 'string' || !ISO.test(value)) return '';
  const at = Date.parse(value);
  return Number.isFinite(at) && at >= APP_EPOCH ? value : '';
}
