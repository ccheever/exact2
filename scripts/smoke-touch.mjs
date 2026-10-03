#!/usr/bin/env bun
// Real touches on an iOS simulator (LLP 1080.000 §5–§6): Caltrain driven
// through the touch runner (`--touch platform`), in both timing modes. Each
// `tap` must reply `delivery: platform` with the touch the app's window
// dispatched; what the tap did is read from `tree`, never from the reply.
// Held contacts reply `unsupported` until P3 passes. Prints the runner's
// start and per-tap times (G2) as observations, never a gate. A runner that
// does not start fails: this simulator is the supported destination.
//   bun scripts/smoke-touch.mjs [--build] [--sim <udid|name>]
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { open } from './agent.mjs';

const argv = process.argv.slice(2);
const ROOT = resolve(new URL('..', import.meta.url).pathname);
if (argv.includes('--sim')) process.env.EXACT_SIM = argv[argv.indexOf('--sim') + 1];
if (argv.includes('--build')) {
  const built = spawnSync('bun', ['host/apple/build.mjs', '--ios', 'caltrain-apple'], { cwd: ROOT, stdio: 'inherit' });
  if (built.status !== 0) { console.error('FAIL ios-touch build: host/apple/build.mjs --ios caltrain-apple'); process.exit(1); }
}
const failures = [];
const check = (ok, what) => { if (!ok) failures.push(what); return ok; };
const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
const t0 = Date.now();

for (const timing of ['agent', 'platform']) {
  const opened = Date.now();
  let s;
  try { s = await open({ host: 'ios', app: 'caltrain', touch: 'platform', timing }); }
  catch (error) { check(false, `${timing}: the session did not open: ${error.message}`); continue; }
  try {
    console.log(`${timing}: opened in ${Date.now() - opened} ms (the runner's start, then the app's)`);
    const taps = [];
    const tap = async (target) => {
      const at = Date.now();
      const r = await s.tap(target);
      taps.push(Date.now() - at);
      check(r.delivery === 'platform' && r.landed?.session === 'main' && r.touch?.type === 'direct', `${timing}: tap ${target} was not a real touch: ${JSON.stringify(r)}`);
      return r;
    };
    const change = await tap('change-station');
    let tree = await s.tree();
    check(byTestId(tree, 'station-search'), `${timing}: a real tap on change-station did not open the stations (landed on ${JSON.stringify(change.landed)})`);
    await s.type('station-search', 'Palo');
    await tap('station-paloalto');
    if (timing === 'platform') await s.clock('settle');
    tree = await s.tree();
    check(byTestId(tree, 'station-name')?.props.text === 'Palo Alto', `${timing}: after a real tap on station-paloalto the station is ${byTestId(tree, 'station-name')?.props.text}`);
    // A held contact stays unsupported (LLP 1080.000 D3), said so.
    const down = await s.tap('station-name', { down: true });
    check(down.delivery === 'unsupported' && /P3/.test(down.reason ?? ''), `${timing}: a held contact replied ${JSON.stringify(down)}`);
    console.log(`${timing}: taps ${taps.join(', ')} ms`);
  } catch (error) {
    check(false, `${timing}: ${error.message}`);
  } finally { await s.close(); }
}

console.log(`touch smoke: ${failures.length ? `${failures.length} failure(s)` : 'ok'} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
for (const f of failures) console.error(`FAIL ios-touch ${f}`);
process.exit(failures.length ? 1 : 0);
