// The design server's clocks (LLP 1122 D4, tamber/1.17): the long poll's
// wait before a heartbeat (25 s), how long a client stays present after its
// last answer (35 s; 90 s after a GET long poll's, which re-asks after a
// backoff), and the sweep that drops the absent (every 5 s); and how long `tamber tool --via` waits for a dev loop
// still starting to answer (20 min: a cold first build took 502 s).
//
// TEST ONLY: the design tool's real-server tests shorten them, so a test of
// heartbeats or a lease runs in seconds, not minutes. Each EXACT_TAMBER_*_MS
// is read only when EXACT_TAMBER_TEST_TIMING=1 says this server runs under a
// test; any other serve ignores them (and a test shows it), and a test serve
// says on stderr which it took. One real-value test per clock runs in the slow
// lane (EXACT_SLOW_TESTS=1), and tamber-timing.test.mjs pins the real values.
export const REAL = Object.freeze({ poll: 25_000, present: 35_000, longPresent: 90_000, sweep: 5_000, viaWait: 20 * 60_000 });
const KNOBS = { poll: 'EXACT_TAMBER_POLL_MS', present: 'EXACT_TAMBER_PRESENT_MS', longPresent: 'EXACT_TAMBER_LONG_PRESENT_MS', sweep: 'EXACT_TAMBER_SWEEP_MS', viaWait: 'EXACT_TAMBER_VIA_WAIT_MS' };

/** The clocks `env` gives: the real ones unless it is a test's
 * (EXACT_TAMBER_TEST_TIMING=1), whose knobs may shorten each, never lengthen
 * it, to a whole number of ms from 50 up (anything else is refused). */
export function timingOf(env = process.env) {
  if (env.EXACT_TAMBER_TEST_TIMING !== '1') return REAL;
  const out = { ...REAL };
  for (const [key, name] of Object.entries(KNOBS)) {
    const said = env[name];
    if (said === undefined) continue;
    const ms = /^\d+$/.test(said) ? Number(said) : NaN;
    if (!(ms >= 50 && ms <= REAL[key])) throw new Error(`${name} is ${JSON.stringify(said)}: whole ms from 50 to ${REAL[key]} (test timing only)`);
    out[key] = ms;
  }
  return Object.freeze(out);
}

export const TIMING = timingOf();
if (TIMING !== REAL) console.error(`tamber: TEST timing (EXACT_TAMBER_TEST_TIMING=1): ${Object.entries(TIMING).map(([k, v]) => `${k} ${v} ms`).join(', ')}`);
