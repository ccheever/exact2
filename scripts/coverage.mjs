// coverage — how a test that cannot run here says so.
//
// A test that cannot run is skipped for one of two reasons, and its name says
// which. `missing`: a prerequisite this run should have had (a browser, a
// toolchain, a built fixture). `n/a`: it does not apply here (another
// platform). With EXACT_REQUIRED=1 (the async lane, which owns the coverage of
// its checks) a missing prerequisite fails the test instead of skipping it, so
// a check cannot pass having tested nothing; `n/a` still skips. Without it, a
// developer's machine skips both, naming the reason.
//
// Discovery (chromium(), hermesBundle(), …) stays a pure answer; the policy is
// applied here, where a test is chosen, and can be given explicitly.
import { test as bunTest } from 'bun:test';

/** Whether a missing prerequisite fails rather than skips (EXACT_REQUIRED=1). */
export const required = (env = process.env) => env.EXACT_REQUIRED === '1';

const stand = (fn, test) => Object.assign(fn, { serial: fn, skip: test.skip, todo: test.todo });

/**
 * `test` when nothing is missing; otherwise a stand-in that names
 * `missing <kind>: <reason>` and skips, or under EXACT_REQUIRED fails.
 * `kind` names the prerequisite (chrome, firefox, webkit, hermes, fixture, …).
 */
export function need(kind, missing, { test = bunTest, env = process.env } = {}) {
  if (!missing) return test;
  const why = `missing ${kind}: ${missing}`;
  if (required(env)) return stand((name) => test(`${name} — ${why}`, () => { throw new Error(`${why} (EXACT_REQUIRED=1)`); }), test);
  console.warn(`SKIP: ${why}`);
  return stand((name, ...args) => test.skip(`${name} — ${why}`, ...args), test);
}

/** `test`, or a skip naming why it does not apply here (never a failure). */
export function notApplicable(reason, { test = bunTest } = {}) {
  if (!reason) return test;
  return stand((name, ...args) => test.skip(`${name} — n/a: ${reason}`, ...args), test);
}
