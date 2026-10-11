// The drive's launch facts (agent-launch.mjs `launchFacts`, LLP 1027.000.000 D3); the fixed defaults and
// overrides are host/web/agent.test.mjs's "launch setup" test.
import { test, expect } from 'bun:test';
import { launchFacts, launchEnvironment, parseFlags } from './agent-launch.mjs';
import { timeReporter } from '../host/web/navigation.js';

// App farm round 2: a drive against a live backend opts into the machine's clock, read once at launch; the host is
// told those milliseconds, so `state.time.epochAtZero` is the launch instant and `clock +N` moves the date from it.
test('--epoch now is the machine clock read once at launch, reported as that instant', () => {
  const {flags} = parseFlags(['web', '--epoch', 'now', 'state']);
  expect(flags.epoch).toBe('now');
  const before = Date.now(), facts = launchFacts(flags), after = Date.now();
  expect(facts.epoch).toBeGreaterThanOrEqual(before);
  expect(facts.epoch).toBeLessThanOrEqual(after);
  expect(launchEnvironment(facts).EXACT_AGENT_EPOCH).toBe(String(facts.epoch));
  // A relaunch from the environment the driver hands a host keeps the instant it read, not a later one.
  expect(launchFacts({env:launchEnvironment(facts)}).epoch).toBe(facts.epoch);
  const fromEnv = launchFacts({env:{EXACT_AGENT_EPOCH:'now'}}).epoch;
  expect(Math.abs(fromEnv - Date.now())).toBeLessThan(1000);
  const time = timeReporter(new URLSearchParams({agent:'1', ...facts}), new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(time(0)).toEqual([facts.epoch, 0]);
  expect(time(60000)).toEqual([facts.epoch, 0]);
});
