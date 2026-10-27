// @exact/gamepad on Android (LLP 1133.022 §21 stage 2): the fixture app made outside the repo and built as the
// Canvas reader's APK, which compiles the package's Kotlin; the roster binds the module and the manifest carries
// `VIBRATE`, which `playEffect` derives.
//
// With `--emulator <serial>` it then drives the APK on that running emulator under the agent with the
// implementation bound (`--bind gamepad`), on the wall clock, with a drive's pads (`EXACT_AGENT_PADS`): the
// process's registry lists them by family with `current` the pressed one; a rumble reaches the pad's recorder and
// completes after its span; a reset preempts a running one; a light reaches the pad and `setLight()` gives it back;
// a pad that disconnects ends its effect `preempted` and raises `gamepaddisconnected`. The recorder's lines are
// logcat's `exact` tag. A real pad's motor and light are device-checklist.md's.
//
// usage: bun packages/gamepad/test/android.mjs [--emulator <serial>] [--app <dir>]
//   (`--app`: the fixture made there, or reused and built again if it is; kept after; an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, makeFixture, root } from './fixture.mjs';

const argv = process.argv.slice(2), flag = (f) => argv.includes(f) ? argv[argv.indexOf(f) + 1] : null;
const emulator = flag('--emulator'), given = flag('--app') && resolve(flag('--app'));
const run = (cmd, args, cwd, env = process.env, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const wait = (ms) => new Promise((done) => setTimeout(done, ms));

const reused = given && existsSync(given);
const { app } = reused ? { app: given } : makeFixture(...(given ? [given] : [])), name = basename(app);
if (!reused) prepare();
function prepare() {
  // `exact new` writes no Canvas library: Weatherlight's, by path into this checkout.
  const template = resolve(root, 'apps/weatherlight/android'), crate = resolve(app, 'android');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  writeFileSync(resolve(crate, 'Cargo.toml'), readFileSync(resolve(template, 'Cargo.toml'), 'utf8')
    .replace('name = "weatherlight-android"', `name = "${name}-android"`).replaceAll('"../../../', `"${root}/`));
  writeFileSync(resolve(crate, 'build.rs'), readFileSync(resolve(template, 'build.rs'), 'utf8').replace('"bake Weatherlight"', '"bake the fixture"'));
  writeFileSync(resolve(crate, 'src/lib.rs'), readFileSync(resolve(template, 'src/lib.rs'), 'utf8'));
  const workspace = resolve(app, 'Cargo.toml');
  writeFileSync(workspace, readFileSync(workspace, 'utf8').replace('members = ["apple", "linux", "web"]', 'members = ["android", "apple", "linux", "web"]'));
  run('bun', ['exact.mjs', 'lock'], app);
}
const { check, failed } = checker();
const env = { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' };
try {
  const built = spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk'], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  check('android', 'the APK builds, compiling Gamepad.kt', built.status === 0, (built.stdout + built.stderr).slice(-3000));
  const out = resolve(root, 'host/android/app/build', name);
  const generated = resolve(out, 'extensions/exact/app/ExactExtensions.kt');
  const roster = existsSync(generated) ? readFileSync(generated, 'utf8') : '';
  check('android', 'the roster binds the gamepad module', roster.includes('exact.gamepad.GamepadModule'), roster || `no ${generated}`);
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const xml = merged ? readFileSync(resolve(out, merged), 'utf8') : '';
  check('android', 'playEffect derives VIBRATE into the manifest', /<uses-permission\s+android:name="android\.permission\.VIBRATE"/.test(xml), merged || 'no merged manifest');
  if (emulator) await drive();
} finally {
  if (!given) rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);

async function drive() {
  const adb = (...a) => run('adb', ['-s', emulator, ...a], root, process.env, true);
  const pads = [
    { index: 0, id: 'DualSense Wireless Controller', vendor: '054c', product: '0ce6', pressed: [0] },
    { index: 1, id: 'Xbox Wireless Controller', vendor: '045e', product: '0b13' },
  ];
  Object.assign(process.env, env, { ANDROID_SERIAL: emulator, EXACT_ANDROID_READER: '1' });
  const { open } = await import(resolve(root, 'scripts/agent.mjs'));
  const launch = (list) => {
    process.env.EXACT_AGENT_PADS = JSON.stringify(list);
    return open({ host: 'android', app: name, bind: 'gamepad', epoch: 'now', storage: `gamepadfixture-${process.pid}` });
  };
  const texts = async (s) => Object.fromEntries(((await s.tree()).nodes ?? []).filter((n) => n.props?.testId).map((n) => [n.props.testId, n.props.text ?? '']));
  /** `testId`'s text once `want` holds, read on the wall clock. */
  const read = async (s, testId, want, ms = 15000) => {
    const end = Date.now() + ms;
    let got = null;
    while (Date.now() < end) {
      try { if (want(got = (await texts(s))[testId])) return got; } catch { got = null; }
      await wait(300);
    }
    return got;
  };
  const logcat = () => adb('logcat', '-d', '-v', 'raw', 'exact:I', '*:S');

  adb('logcat', '-c');
  let s = await launch(pads);
  try {
    const listed = await read(s, 'list', (t) => t === 'true 2 0');
    check('emulator', "the registry's pads, the pressed one current", listed === 'true 2 0', listed);
    const described = (await texts(s)).pads;
    check('emulator', 'each by its family, with its labels and effects', described === '0:playstation:Square:dual-rumble 1:xbox:X:dual-rumble', described);
    await s.tap('buzz');
    const played = await read(s, 'played', (t) => t === 'complete', 5000);
    check('emulator', 'a rumble on the current pad completes after its span', played === 'complete', played);
    await s.tap('long');
    await wait(200);
    await s.tap('stop');
    const stopped = await read(s, 'stopped', (t) => t === 'complete', 5000);
    check('emulator', 'a reset preempts the running rumble', stopped === 'complete' && (await texts(s)).played === 'preempted', `${stopped} ${(await texts(s)).played}`);
    await s.tap('amber');
    await s.tap('unlit');
    await wait(500);
    const lines = logcat();
    check('emulator', 'the motors were asked as the call said', lines.includes('pad 0: rumble 1 0.5 for 200 ms'), lines.slice(-1500));
    check('emulator', 'the reset stopped them', lines.includes('pad 0: motors stopped'), lines.slice(-1500));
    check('emulator', 'the light reached the pad, and setLight() gave it back', lines.includes('pad 0: light #e8ad61') && lines.includes('pad 0: light given back'), lines.slice(-1500));
  } finally {
    await s.close().catch(() => {});
  }

  // A claim's lifetime: a pad that goes ends the effect playing on it. Its going is on the wall clock, so a 2 s
  // rumble starts every 1.5 s until it goes (each preempting the last); the last one is ended by the disconnect.
  s = await launch([{ ...pads[0], leaves: 8000 }, pads[1]]);
  try {
    check('emulator', 'a pad that goes is listed first', await read(s, 'list', (t) => t === 'true 2 0') === 'true 2 0', (await texts(s)).list);
    const end = Date.now() + 15000;
    while (Date.now() < end && (await texts(s)).list === 'true 2 0') { await s.tap('long'); await wait(1500); }
    const gone = await read(s, 'list', (t) => t === 'true 1 none', 10000);
    check('emulator', 'then gone', gone === 'true 1 none', gone);
    check('emulator', 'its going raises gamepaddisconnected', (await texts(s)).went === '0 ', JSON.stringify((await texts(s)).went));
    const ended = await read(s, 'played', (t) => t === 'preempted', 3000);
    check('emulator', 'and ends its effect, preempted', ended === 'preempted', ended);
  } finally {
    await s.close().catch(() => {});
  }
}
