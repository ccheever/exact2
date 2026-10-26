// The bound on a type check that never ends (the design tool's `build.mjs --check` spun 4.7 days, found 2026-10-26):
// 10 minutes, a bound for "never", not a target (the clone's check takes seconds). A test may shorten it
// (EXACT_CHECK_BOUND_MS), read only under EXACT_TEST_TIMING=1, as the design server's clocks are
// (scripts/tamber-timing.mjs); a value not shorter, not a whole number of ms from 50, or set without the switch is
// ignored.
export const CHECK_BOUND_MS = 10 * 60_000;

export function checkBound(env = process.env) {
  if (env.EXACT_TEST_TIMING !== '1') return CHECK_BOUND_MS;
  const said = env.EXACT_CHECK_BOUND_MS, ms = /^\d+$/.test(said ?? '') ? Number(said) : NaN;
  return ms >= 50 && ms < CHECK_BOUND_MS ? ms : CHECK_BOUND_MS;
}

/** The bound in words: `10 min`, `1.5 s`. */
export const boundWords = ms => (ms >= 60_000 ? `${ms / 60_000} min` : `${ms / 1000} s`);
