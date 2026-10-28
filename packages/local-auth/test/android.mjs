// @exact/local-auth's conformance suite on Android (LLP 1133.010 D16, §11 stage 2), on Excord against its fixture:
// the implementation bound (`--bind local-auth`), the token in the reader's Keystore (`EXACT_STORE=real`), and the
// platform's own BiometricPrompt answered with the emulator's fingerprint (`adb emu finger touch`) or the PIN. A
// phone's Keystore is hardware; an emulator's is software, so this proves the flow, the key's gate and the
// platform's drops, not the hardware (D2).
//
// usage: bun packages/local-auth/test/android.mjs [--pin <pin>]
//   On adb's device (ANDROID_SERIAL), an emulator with a PIN set (`adb shell locksettings set-pin <pin>`, 1234 by
//   default) and fingerprint 1 enrolled (Settings, Security, Pixel Imprint, each touch `adb emu finger touch 1`).
//   adb must run as root (a Google APIs image) for the suite to list the app's files. Builds nothing: build the APK
//   first (scripts/agent-android.mjs build excord --apk). Starts Excord's fixture server on the port this checkout's
//   build bakes, reached through `adb reverse`, and stops it. The last check removes the PIN, which deletes the keys: it sets the PIN again
//   after, but fingerprint 1 must be enrolled again before the next run.
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { checker, excordFixture } from './fixture.mjs';

const root = resolve(import.meta.dir, '../../..');
const argv = process.argv.slice(2), pin = argv.includes('--pin') ? argv[argv.indexOf('--pin') + 1] : '1234';
const serial = process.env.ANDROID_SERIAL ?? spawnSync('adb', ['devices'], { encoding: 'utf8' }).stdout.split('\n').map((l) => l.split('\t')).find(([, s]) => s?.trim() === 'device')?.[0];
if (!serial) throw new Error('no Android device: start an emulator, or set ANDROID_SERIAL');
Object.assign(process.env, { ANDROID_SERIAL: serial, EXACT_ANDROID_READER: '1' });
const adb = (...a) => spawnSync('adb', ['-s', serial, ...a], { encoding: 'utf8' });
adb('root');
adb('wait-for-device');
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const { check, failed } = checker();
const ID = 'com.exact.excord', KEYS = `/data/data/${ID}/files/exact-presence/${ID}`;
const STORE = `/data/data/${ID}/cache/exact/${ID}/agent/local-auth-real`;
const files = (dir) => adb('shell', `ls -A ${dir} 2>/dev/null`).stdout.split('\n').filter((f) => f && !f.startsWith('.'));

const fixture = await excordFixture();
adb('reverse', `tcp:${fixture.port}`, `tcp:${fixture.port}`);
const fixtureState = () => fetch(`${fixture.origin}/control/state`).then((r) => r.json());

// A scratch store for the app's files (the gateway keeps its identify record there); the token stays the Keystore's.
const launch = () => open({ host: 'android', app: 'excord', bind: 'local-auth', storage: 'local-auth-real', env: { EXACT_STORE: 'real' } });
const ids = async (s) => new Set((await s.tree()).nodes.map((n) => n.props?.testId).filter(Boolean));
const signedIn = (st) => st.resources?.session?.ready === true && (st.streams ?? []).some((x) => x.name === 'gateway' && x.readyState === 1);
const locked = (st) => (st.store ?? []).some((n) => n.startsWith('fixture-token: locked'));
/** Wait on the wall clock, which the prompt and the Keystore run on, until `want` holds of the state. */
async function until(s, want, ms = 20000) {
  const end = Date.now() + ms;
  let got;
  while (Date.now() < end) {
    await s.clock('+250 real');
    if (want(got = await s.state())) return got;
  }
  return got;
}
/** The focused windows, one a display (`dumpsys window` names each display's): BiometricPrompt's is SystemUI's. */
const front = () => [...adb('shell', 'dumpsys window').stdout.matchAll(/mCurrentFocus=Window\{\S+ \S+ ([^}]*)\}/g)].map((m) => m[1]).join(' ');
const prompting = () => /BiometricPrompt|ConfirmDeviceCredential|ConfirmLock/.test(front());
/** Wait until the prompt is up (or `ms` passes): whether it came. */
async function prompt(s, ms = 10000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (prompting()) return true;
    await s.clock('+250 real');
  }
  return false;
}
/** The prompt answered by finger 1 until `want` holds of the state. */
async function touched(s, want, finger = 1) {
  let got;
  for (let i = 0; i < 16 && !want(got = await s.state()); i++) {
    if (prompting()) adb('emu', 'finger', 'touch', String(finger));
    await s.clock('+500 real');
  }
  return got;
}
/** The prompt answered with the PIN, through its "Use PIN" button, until `want` holds. */
async function withPin(s, want) {
  await prompt(s);
  const dump = () => { adb('shell', 'uiautomator dump /sdcard/exact-ui.xml'); return adb('shell', 'cat /sdcard/exact-ui.xml').stdout; };
  const button = /text="Use PIN"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/.exec(dump());
  if (button) {
    const [, x0, y0, x1, y1] = button.map(Number);
    adb('shell', 'input', 'tap', String((x0 + x1) >> 1), String((y0 + y1) >> 1));
    await s.clock('+1500 real');
  }
  adb('shell', 'input', 'text', pin);
  adb('shell', 'input', 'keyevent', 'KEYCODE_ENTER');
  return until(s, want);
}

try {
  let s = await launch();
  try {
    const status = (await until(s, (st) => st.resources?.lock?.available !== undefined && st.resources?.session !== undefined)).resources?.lock;
    check('android', 'the status: available, keeps locked secrets, the device credential, the platform\'s label', status?.available === true && status?.presenceSecrets === true && status?.deviceCredential === true && typeof status?.android?.label === 'string', JSON.stringify(status));
    let first = await until(s, (st) => signedIn(st) || (st.store ?? []).some((n) => /fixture-token: (locked|pending)/.test(n)), 60000);
    if (locked(first)) {
      // A run before this one left the lock on: unlock at launch, then turn it off to start plain.
      await prompt(s);
      await touched(s, signedIn);
      await s.tap('account');
      await s.tap('lock-switch');
      await prompt(s);
      first = await touched(s, (st) => !locked(st));
    }
    const st = await until(s, signedIn, 60000);
    check('android', 'signed in against the fixture, the token plain', signedIn(st) && !locked(st), JSON.stringify({ session: st.resources?.session, streams: st.streams, store: st.store }));
    await s.clock('settle');
    await s.tap('account');
    await s.tap('lock-switch');
    const asked = await prompt(s);
    check('android', 'turning the lock on shows BiometricPrompt', asked, front());
    const on = await touched(s, (x) => x.resources?.session?.lockOn === true);
    check('android', 'a touch passes the check, and the token moves into the Keystore', on.resources?.session?.lockOn === true && locked(on), JSON.stringify({ session: on.resources?.session, store: on.store }));
    const keys = files(KEYS), plain = files(`${STORE}/secrets`);
    check('android', 'one sealed item on disk, no plain copy', keys.length === 1 && keys[0].startsWith('fixture-token#') && !plain.length, JSON.stringify({ keys, plain }));
    const sealed = adb('shell', `cat ${KEYS}/${keys[0]}`).stdout;
    check('android', 'the item does not hold the token\'s bytes', keys.length === 1 && !sealed.includes('fixture-token-'), 'found');
    // A real stop and start: Home, then the app again; the activity's onStop and onStart report the page's visibility.
    adb('shell', 'input', 'keyevent', 'KEYCODE_HOME');
    // An activity Android has not stopped yet has not left: under load its onStop can come seconds after Home.
    const stopped = () => /state=STOPPED/.test(adb('shell', 'dumpsys activity activities').stdout.split('\n').filter((l, i, all) => all.slice(Math.max(0, i - 40), i).some((x) => x.includes(`${ID}/dev.exact.app.MainActivity`))).join('\n'));
    for (let i = 0; i < 40 && !stopped(); i++) await s.clock('+250 real');
    await s.clock('+250 real');
    adb('shell', 'am', 'start', '-n', `${ID}/dev.exact.app.MainActivity`);
    await s.clock('+1500 real');
    await s.clock('settle');
    const covered = await ids(s);
    check('android', 'a return from Home shows the lock screen', covered.has('lock-screen'), [...covered].join(' '));
    const socket = (await s.state()).streams?.find((x) => x.name === 'gateway')?.ticket;
    await s.tap('unlock');
    await prompt(s);
    await touched(s, () => false, 1);
    const back = await ids(s), now = await s.state();
    check('android', 'a touch takes the lock screen away, on the same socket', !back.has('lock-screen') && now.streams?.find((x) => x.name === 'gateway')?.ticket === socket, JSON.stringify({ ids: [...back], socket, streams: now.streams }));
  } finally { await s.close(); }

  // A relaunch begins locked: the prompt at launch over the Keystore key, then the channels.
  s = await launch();
  try {
    await s.clock('data');
    const covered = await ids(s);
    check('android', 'a relaunch begins on the lock screen, the account unshown', covered.has('lock-screen') && !covered.has('transcript'), [...covered].join(' '));
    // An unenrolled finger fails; the prompt stays, and a cancel leaves the lock screen with Unlock.
    await prompt(s);
    adb('emu', 'finger', 'touch', '2');
    await s.clock('+1000 real');
    check('android', 'an unenrolled finger does not unlock', !signedIn(await s.state()) && prompting(), front());
    adb('shell', 'input', 'keyevent', 'KEYCODE_BACK');
    await until(s, (x) => x.resources?.unlocked !== undefined || !prompting(), 5000);
    await s.clock('settle');
    const cancelled = await ids(s);
    check('android', 'Back cancels the prompt: the lock screen with Unlock', cancelled.has('lock-screen') && cancelled.has('unlock') && !prompting(), [...cancelled].join(' '));
    await s.tap('unlock');
    const loaded = await withPin(s, signedIn);
    check('android', 'the PIN opens the Keystore key, and the app signs in with the token', signedIn(loaded) && (await fixtureState()).identify_token?.startsWith('fixture-token'), JSON.stringify(loaded.resources?.session));
  } finally { await s.close(); }

  // Off again: the prompt, then the token plain.
  s = await launch();
  try {
    await prompt(s);
    await touched(s, signedIn);
    await s.clock('settle');
    await s.tap('account');
    await s.tap('lock-switch');
    await prompt(s);
    const off = await touched(s, (x) => !locked(x) && x.resources?.session?.lockOn === false);
    check('android', 'unprotect after a touch keeps the token plain, and deletes the sealed item', !locked(off) && !files(KEYS).length && files(`${STORE}/secrets`).length === 1, JSON.stringify({ store: off.store, keys: files(KEYS) }));
    await s.tap('account');
    await s.tap('lock-switch');
    await prompt(s);
    await touched(s, (x) => locked(x));
  } finally { await s.close(); }

  // The lock screen removed: Android deletes the key, and the app signs in again (D4, drive 10's passcode step).
  adb('shell', 'locksettings', 'clear', '--old', pin);
  s = await launch();
  try {
    const st = await until(s, (x) => (x.store ?? []).some((n) => n === 'fixture-token: absent') || (x.resources?.lock?.presenceSecrets === false && !locked(x)), 20000);
    await s.clock('settle');
    const shown = await ids(s);
    check('android', 'with no lock screen the key is gone: "Sign in again", the token absent', shown.has('signed-out') && !locked(st), JSON.stringify({ ids: [...shown], store: st.store, lock: st.resources?.lock }));
  } finally {
    await s.close();
    adb('shell', 'locksettings', 'set-pin', pin);
  }
} catch (e) {
  check('android', 'ran', false, `${e.stack ?? e} ${JSON.stringify(e.reply ?? '')}`);
} finally {
  fixture.stop();
}
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
