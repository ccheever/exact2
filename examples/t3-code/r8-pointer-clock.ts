// Lane r8-pointer, reworked by r13-store (F3): a wall time that is a real epoch.
//
// What a data source can know. The window's time is exactTime's `epochAtZero`
// plus its own clock (app.contract passes `wallTime.epochAtZero + now()`).
// Until the host has told the date `epochAtZero` is 0, so the window time is
// seconds since launch: a number, but not an instant. The runtime refuses
// `Date.now()` in data sources (js/src/prelude.js), so `clock()` has nothing
// real to offer there; it only has a value where the clock is not refused
// (the bun tests, a hypothetical host). A launch-relative or virtual reading
// (agent mode counts from 0) is never an instant either. So a time is an
// epoch only above 1e12 ms (2001-09-09), and anything below it is unknown.
//
// Writers (onboardingCompletedAt) must store an instant or nothing. D16 and
// the round-12 real-input pass stored `1970-01-01T00:01:37Z` because the
// finish command carried `now()` (ms since launch) as its time, and this
// module then returned that small number as if it were an epoch.
import { clock } from './sidebar-state';

/** Below 2001-09-09 (1e12 ms) a number is a count from launch (or from the agent's zero), not an instant. */
export const EPOCH_FLOOR = 1e12;

/** Whether a time is an instant (Unix ms) and not a launch-relative count. */
export function isEpoch(time: number): boolean {
  return Number.isFinite(time) && time > EPOCH_FLOOR;
}

/**
 * The instant a window time stands for, or null when it stands for none:
 * the window's own time when it is an epoch, else the runtime clock when that
 * is one, else unknown.
 */
export function wallEpoch(windowTime: number): number | null {
  if (isEpoch(windowTime)) return windowTime;
  const runtime = clock(0);
  return isEpoch(runtime) ? runtime : null;
}

/** The ISO stamp a writer stores for a window time, or '' when the instant is unknown (store nothing). */
export function wallIso(windowTime: number): string {
  const at = wallEpoch(windowTime);
  return at === null ? '' : new Date(at).toISOString();
}

/**
 * Kept for callers that want a number: the instant when one is known, else the
 * window time unchanged. Writers use `wallIso`; a persisted value never comes
 * from this fallback.
 */
export function epochNow(windowTime: number): number {
  return wallEpoch(windowTime) ?? windowTime;
}
