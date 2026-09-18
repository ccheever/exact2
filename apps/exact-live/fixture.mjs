#!/usr/bin/env bun
// Exact Live's isolated API-only instance; reuse the existing bounded protocol.
// Public deployment forwards TWO HTTPS origins to these two loopback listeners.
import { createFixture } from '../completion-storm/fixture.mjs';

export const fixtureOptions = Object.freeze({
  port: 4339, controlPort: 4340, dist: null,
  maxWaves: 8, maxHeld: 512, holdMs: 30_000,
});

if (import.meta.main) {
  if (process.argv.length > 2) throw Error('Usage: bun apps/exact-live/fixture.mjs');
  const fixture = await createFixture(fixtureOptions);
  console.log(`Exact Live Jobs API only: data ${fixture.data}, control ${fixture.control}`);
  console.log('128 selected lanes are independent requests, not 128 simultaneous browser connections.');
  console.log('Release includes later-admitted lanes; held waves expire after 30 seconds.');
  console.log('One shared bounded fixture, not per-visitor/account isolation.');
  let closing = false;
  const stop = () => {
    if (closing) return;
    closing = true;
    fixture.close().then(() => process.exit(0), error => { console.error(error); process.exit(1); });
  };
  process.once('SIGINT', stop);
  process.once('SIGTERM', stop);
}
