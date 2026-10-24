// @exact/local-auth's conformance suite on Apple (LLP 1133.010 D16), with the
// implementation bound (`--bind local-auth`) and the platform's stores
// (`EXACT_STORE=real`): LocalAuthentication's own sheet, and locked items in
// the keychain. The fixture app is the one `exact new` makes outside the repo
// (fixture.mjs).
//
// iOS: on a simulator the suite boots, with Face ID enrolled from its
// BiometricKit notifications (the Features menu's), each sheet matched or
// failed the same way, and the system's alerts answered through XCTest's
// SpringBoard (`touch: 'platform'`). A simulator has no Secure Enclave: this
// proves the flow and the refusals, not the hardware (D2).
// macOS: what a build without a provisioning profile does (D7): no locked
// secret is kept, a name that must be locked is refused, an optional one
// stays plain. It shows no sheet, so it needs no one at the Mac.
//
// usage: bun packages/local-auth/test/apple.mjs ios [--sim <udid>] | macos [--app <dir>]
//   ios without --sim creates a simulator, and shuts down and deletes it
//   after; --sim uses that one (booted here if it is not) and leaves it.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), host = argv.includes('macos') ? 'macos' : 'ios';
const given = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : null;
const { open, textOf } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const simctl = (...a) => spawnSync('xcrun', ['simctl', ...a], { encoding: 'utf8' });

let udid = given, made = false;
if (host === 'ios' && !udid) {
  const types = JSON.parse(simctl('list', '-j', 'devicetypes').stdout).devicetypes.filter((t) => /iPhone \d+ Pro$/.test(t.name));
  const runtime = JSON.parse(simctl('list', '-j', 'runtimes').stdout).runtimes.filter((r) => r.platform === 'iOS' && r.isAvailable).at(-1);
  udid = simctl('create', 'local-auth-suite', types.at(-1).identifier, runtime.identifier).stdout.trim();
  made = true;
}
if (host === 'ios') {
  simctl('boot', udid);
  simctl('bootstatus', udid, '-b');
  // Face ID enrolled, as the Features menu enrols it.
  simctl('spawn', udid, 'notifyutil', '-s', 'com.apple.BiometricKit.enrollmentChanged', '1');
  simctl('spawn', udid, 'notifyutil', '-p', 'com.apple.BiometricKit.enrollmentChanged');
}
const face = (result) => simctl('spawn', udid, 'notifyutil', '-p', `com.apple.BiometricKit_Sim.pearl.${result}`);

// `--app <dir>`: a fixture an earlier run made and built, as it stands.
const kept = argv.includes('--app') ? argv[argv.indexOf('--app') + 1] : null;
const { app, id } = kept ? { app: kept, id: JSON.parse(readFileSync(resolve(kept, 'app.json'), 'utf8')).app.id } : makeFixture();
const env = { ...process.env, ...(udid ? { EXACT_SIM: udid } : {}) };
if (!kept) run('bun', ['exact.mjs', host === 'ios' ? 'ios' : 'mac', ...(udid ? ['--sim', udid] : [])], app, env);
if (udid) process.env.EXACT_SIM = udid;

const launch = () => open({ host, app, bind: 'local-auth', env: { EXACT_STORE: 'real' }, ...(host === 'ios' ? { touch: 'platform' } : {}) });
const text = async (s, testId) => {
  const nodes = (await s.tree()).nodes, node = nodes.find((n) => n.props?.testId === testId);
  return node ? textOf(nodes, node) ?? '' : null;
};
/** Wait on the wall clock, which the sheet and the keychain run on, until `testId` reads as `want` says. */
async function until(s, testId, want, ms = 15000) {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = await text(s, testId))) return got;
  }
  return got;
}
/** A real touch, once the sheet has gone: the app's window is key again a moment after the check ends. */
async function tap(s, testId) {
  for (let i = 0; ; i++) {
    await s.clock('settle');
    try {
      return await s.tap(testId);
    } catch (e) {
      if (i > 20 || !/not the foreground|not in the foreground/.test(e.message)) throw e;
      await s.clock('+250 real');
    }
  }
}
/** A system alert's button, by its label, pressed through SpringBoard. */
async function press(s, label, tries = 20) {
  for (let i = 0; i < tries; i++) {
    const { elements = [] } = await s.springboard({ action: 'find', label });
    const [x, y, w, h] = elements[0]?.frame ?? [];
    if (w) return s.springboard({ action: 'tap', point: [x + w / 2, y + h / 2] });
    await s.clock('+250 real');
  }
  return null;
}
/** `testId`'s call shows the sheet: matched (or failed) until the call answers. */
async function sheet(s, testId, result, want) {
  await tap(s, testId);
  let got;
  for (let i = 0; i < 12 && !want(got = await text(s, 'wrote') ?? ''); i++) {
    await s.clock('+500 real');
    face(result);
  }
  return got;
}

try {
  if (host === 'macos') {
    const s = await launch();
    try {
      const status = await until(s, 'status', (t) => t?.split(' ')[1] === 'false');
      check('macos', 'without a profile no locked secret is kept here', status?.split(' ')[1] === 'false', status);
      const key = (await tap(s, 'set-key'), await until(s, 'wrote', (t) => t !== ''));
      check('macos', 'a name that must be locked is refused, never kept plain', key === 'NotSupportedError NotSupportedError', key);
      const token = (await tap(s, 'set-token'), await until(s, 'wrote', (t) => t === 'kept'));
      const facts = await until(s, 'facts', (t) => t?.startsWith('true'));
      check('macos', 'an optional name stays plain', token === 'kept' && facts === 'true false false false', `${token} / ${facts}`);
      const protect = (await tap(s, 'protect'), await until(s, 'wrote', (t) => t !== 'kept'));
      check('macos', 'protect rejects NotSupportedError', protect === 'NotSupportedError NotSupportedError', protect);
    } finally {
      await s.close();
    }
  } else {
    let s = await launch();
    try {
      const status = await until(s, 'status', (t) => t?.startsWith('true'));
      // A kept fixture (`--app`) has used Face ID before.
      check('ios', 'the status: available, keeps locked secrets, Face ID not yet asked', status === `true true ${kept ? 'granted' : 'prompt'} faceID`, status);
      // The first evaluation may ask for Face ID's permission (a phone does), then shows the sheet.
      await tap(s, 'authenticate');
      const asked = await press(s, 'OK', 8);
      console.log(`note: the Face ID permission alert ${asked ? 'was shown and allowed' : 'did not show'}`);
      let checked;
      for (let i = 0; i < 12 && (checked = await text(s, 'check')) === ''; i++) { await s.clock('+500 real'); face('match'); }
      check('ios', 'authenticate passes on a matched face, after the permission', checked === 'passed', checked);
      const granted = await until(s, 'status', (t) => t?.includes('granted'));
      check('ios', 'the permission reads granted after it', granted === 'true true granted faceID', granted);
      const key = (await tap(s, 'set-key'), await until(s, 'wrote', (t) => t === 'kept'));
      const facts = await until(s, 'facts', (t) => t?.endsWith('true true'));
      check('ios', 'a mandatory name is kept locked at once, in the keychain', key === 'kept' && facts === 'false false true true', `${key} / ${facts}`);
      const read = (await tap(s, 'read-key'), await until(s, 'wrote', (t) => t !== 'kept'));
      check('ios', 'get refuses a locked name', read.startsWith('InvalidAccessError'), read);
      await tap(s, 'set-token');
      await until(s, 'facts', (t) => t?.startsWith('true false'));
      const protect = (await tap(s, 'protect'), await until(s, 'wrote', (t) => t === 'protected'));
      const locked = await until(s, 'facts', (t) => t === 'true true true true');
      check('ios', 'protect moves the plain token into the keychain, with no sheet', protect === 'protected' && locked === 'true true true true', `${protect} / ${locked}`);
      const unlocked = await sheet(s, 'unlock', 'match', (t) => t.startsWith('unlocked') || /^[A-Z_]+ [A-Z_]+$/.test(t));
      check('ios', 'unlock reads the item after the sheet', unlocked === 'unlocked open', unlocked);
      const state = JSON.stringify(await s.state());
      check('ios', 'the locked key\'s bytes are nowhere in the state', !state.includes('locked-value'), 'locked-value in state');
      const failedFace = await sheet(s, 'unlock', 'nomatch', (t) => t !== 'unlocked open');
      if (failedFace === 'unlocked open' && !(await press(s, 'Cancel'))) throw new Error('no Cancel on the sheet after a face that did not match');
      const cancelled = await until(s, 'wrote', (t) => t !== 'unlocked open');
      check('ios', 'a face that does not match, then a cancel, fails the unlock', /^(USER_CANCEL|AUTHENTICATION_FAILED) /.test(cancelled), cancelled);
      await s.close();
      // A relaunch reads the locked item again from the keychain.
      s = await launch();
      const again = await until(s, 'facts', (t) => t === 'true true true true');
      check('ios', 'a relaunch finds both names locked', again === 'true true true true', again);
      const second = await sheet(s, 'unlock', 'match', (t) => t.startsWith('unlocked') || /^[A-Z_]+ [A-Z_]+$/.test(t));
      check('ios', 'and unlocks the token after the sheet', second === 'unlocked open', second);
      const unprotected = await sheet(s, 'unprotect', 'match', (t) => t === 'unprotected' || /^[A-Z_]+ [A-Z_]+$/.test(t));
      const plain = await until(s, 'facts', (t) => t === 'true false true true');
      check('ios', 'unprotect, after the sheet, keeps the token plain', unprotected === 'unprotected' && plain === 'true false true true', `${unprotected} / ${plain}`);
      await s.close();
      // A new install deletes what the last left in the keychain (LLP 1018 D7), which iOS keeps past a deletion.
      const bundle = simctl('get_app_container', udid, id).stdout.trim();
      const copy = resolve(process.env.TMPDIR ?? '/tmp', `local-auth-${Date.now()}.app`);
      spawnSync('cp', ['-R', bundle, copy]);
      simctl('uninstall', udid, id);
      simctl('install', udid, copy);
      spawnSync('rm', ['-rf', copy]);
      s = await launch();
      const fresh = await until(s, 'facts', (t) => t === 'false false false false');
      check('ios', 'a reinstall begins with nothing kept', fresh === 'false false false false', fresh);
    } finally {
      await s?.close();
    }
  }
} catch (e) {
  check(host, 'ran', false, `${e.stack ?? e} ${JSON.stringify(e.reply ?? '')}`);
} finally {
  if (made) { simctl('shutdown', udid); simctl('delete', udid); }
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
