// Excord's lock with the real sheet on an iOS simulator (LLP 1133.010 §9 drive
// 10, the part a simulator can show): the implementation bound (`--bind
// local-auth`), the token in the simulator's keychain (`EXACT_STORE=real`),
// Face ID enrolled and matched from the simulator's BiometricKit
// notifications, against Excord's fixture server. The phone's and the Mac's
// parts stay by hand (device-checklist.md).
//
// usage: bun packages/local-auth/test/excord-sim.mjs --sim <udid>
//   Builds nothing: build Excord first (host/apple/build.mjs excord-apple
//   --ios --sim <udid>). Starts Excord's fixture server on the port this
//   checkout's build bakes, and stops it.
//   Leaves the token in the simulator's keychain; delete the app to wipe it.
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { checker, excordFixture } from './fixture.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), udid = argv[argv.indexOf('--sim') + 1];
if (!argv.includes('--sim') || !udid) throw new Error('usage: excord-sim.mjs --sim <udid>');
process.env.EXACT_SIM = udid;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const simctl = (...a) => spawnSync('xcrun', ['simctl', ...a], { encoding: 'utf8' });
const face = (result) => simctl('spawn', udid, 'notifyutil', '-p', `com.apple.BiometricKit_Sim.pearl.${result}`);
simctl('spawn', udid, 'notifyutil', '-s', 'com.apple.BiometricKit.enrollmentChanged', '1');
simctl('spawn', udid, 'notifyutil', '-p', 'com.apple.BiometricKit.enrollmentChanged');

const fixture = await excordFixture();

// A scratch store for the app's files (the gateway keeps its identify record there); the token stays the keychain's.
const launch = () => open({ host: 'ios', app: 'excord', bind: 'local-auth', storage: 'local-auth-real', env: { EXACT_STORE: 'real' } });
const ids = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
const signedIn = (st) => st.resources?.session?.ready === true && (st.streams ?? []).some((x) => x.name === 'gateway' && x.readyState === 1);
/** Wait on the wall clock until `want` holds of the state. */
async function until(s, want, ms = 20000) {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = await s.state())) return got;
  }
  return got;
}
/** The sheet is up for a check: matched until `want` holds. */
async function matched(s, want) {
  let got;
  for (let i = 0; i < 16 && !want(got = await s.state()); i++) {
    await s.clock('+500 real');
    face('match');
  }
  return got;
}
const locked = (st) => (st.store ?? []).some((n) => n.startsWith('fixture-token: locked'));

try {
  let s = await launch();
  try {
    const first = await until(s, (st) => signedIn(st) || (st.store ?? []).some((n) => /fixture-token: (locked|pending)/.test(n)));
    if (locked(first)) {
      // A run before this one left the lock on: unlock at launch, then turn it off to start plain.
      await matched(s, signedIn);
      await s.tap('account');
      await s.tap('lock-switch');
      await matched(s, (st) => !locked(st));
    }
    const st = await until(s, signedIn, 60000);
    check('excord', 'signed in against the fixture, the token plain in the keychain', signedIn(st) && !locked(st), JSON.stringify({ session: st.resources?.session, streams: st.streams, store: st.store }));
    await s.clock('settle');
    await s.tap('account');
    await s.tap('lock-switch');
    const on = await matched(s, (x) => x.resources?.session?.lockOn === true);
    check('excord', 'the switch asks the real sheet, then locks the token', on.resources?.session?.lockOn === true && locked(on), JSON.stringify({ session: on.resources?.session, store: on.store }));
    // A return covers the transcript; Unlock asks the sheet again.
    await s.prefer({ 'visibility-state': 'hidden' });
    await s.prefer({ 'visibility-state': 'visible' });
    await s.clock('+5');
    check('excord', 'a return shows the lock screen', (await ids(s)).has('lock-screen'), [...(await ids(s))].join(' '));
    await s.tap('unlock');
    await matched(s, () => false);
    const back = await ids(s);
    check('excord', 'a matched face takes the lock screen away', !back.has('lock-screen'), [...back].join(' '));
  } finally { await s.close(); }
  // A relaunch begins locked: the sheet at launch, then the channels.
  s = await launch();
  try {
    await s.clock('data');
    const covered = await ids(s);
    check('excord', 'a relaunch begins on the lock screen, the account unshown', covered.has('lock-screen') && !covered.has('transcript'), [...covered].join(' '));
    const loaded = await matched(s, signedIn);
    check('excord', 'the launch\'s sheet, matched, unlocks the keychain item and signs in', signedIn(loaded), JSON.stringify(loaded.resources?.session));  } finally { await s.close(); }
} catch (e) {
  check('excord', 'ran', false, e.stack ?? e);
} finally {
  fixture.stop();
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
