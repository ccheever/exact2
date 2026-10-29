// @exact/health, @exact/pedometer and @exact/altimeter on Android (LLP 1133.026 §15 stage 3): the conformance fixture
// made outside the repo, built as the Canvas reader's APK, which compiles the three packages' Kotlin against Health
// Connect's client and Play services' Recording API in the app's one locked, verified Gradle graph (LLP 1133.008 D8
// item 5). It checks the graph, the roster, the permissions the read lists derive, the manifest fragment (D4, D19
// item 3) and the fixture stores carried, and prints the APK's size. `--emulator` then drives the fixture on the
// emulator ANDROID_SERIAL names (an arm64 one of API 34 or later, where Health Connect is built in): under the agent
// over the fixture store, the reads granted and a typed partial answer; bound (`--bind health`), the status Health
// Connect gives an app it never asked, which reads no data. checklist.md's Android part is a person's.
//
// usage: bun packages/health/test/android.mjs [--emulator]
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, makeFixture, pkg, root, run, texts } from './fixture.mjs';

const { app } = makeFixture(), name = basename(app);
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

const { check, failed } = checker();
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const env = { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' };
const out = resolve(root, 'host/android/app/build', name);
try {
  const built = spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk', '--write-locks'], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  check('android', 'the APK builds, compiling Health.kt, Pedometer.kt and Altimeter.kt', built.status === 0, (built.stdout + built.stderr).slice(-3000));
  const locked = existsSync(resolve(app, 'gradle.lockfile')) ? readFileSync(resolve(app, 'gradle.lockfile'), 'utf8') : '';
  check('android', "the lock names Health Connect's client and Play services' fitness",
    /^androidx\.health\.connect:connect-client:/m.test(locked) && /^com\.google\.android\.gms:play-services-fitness:/m.test(locked), locked.slice(0, 400) || 'no gradle.lockfile');
  const roster = existsSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt')) ? readFileSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt'), 'utf8') : '';
  check('android', 'the roster names the three modules',
    ['exact.health.HealthModule', 'exact.pedometer.PedometerModule', 'exact.altimeter.AltimeterModule'].every((c) => roster.includes(c)), roster);
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).map(String)
    .find((f) => f.includes('merged_manifests/release/') && f.endsWith('AndroidManifest.xml'));
  const manifest = merged ? readFileSync(resolve(out, merged), 'utf8') : '';
  const permission = (p) => new RegExp(`<uses-permission\\s+android:name="${p.replaceAll('.', '\\.')}"`).test(manifest);
  check('android', 'the read list derives READ_STEPS, READ_DISTANCE and READ_EXERCISE, and nothing else of Health',
    ['READ_STEPS', 'READ_DISTANCE', 'READ_EXERCISE'].every((p) => permission(`android.permission.health.${p}`))
      && !permission('android.permission.health.READ_ACTIVE_CALORIES_BURNED') && !permission('android.permission.health.READ_HEALTH_DATA_HISTORY'), manifest.slice(0, 2000));
  check('android', 'the pedometer derives ACTIVITY_RECOGNITION', permission('android.permission.ACTIVITY_RECOGNITION'), manifest.slice(0, 2000));
  check('android', "the fragment's rationale activity and its permission-usage alias are merged",
    manifest.includes('androidx.health.ACTION_SHOW_PERMISSIONS_RATIONALE') && manifest.includes('android.intent.action.VIEW_PERMISSION_USAGE')
      && manifest.includes('android.intent.category.HEALTH_PERMISSIONS') && manifest.includes('android.permission.START_VIEW_PERMISSION_USAGE'), manifest);
  check('android', 'Health Connect is queried, and the privacy policy carried',
    manifest.includes('<package android:name="com.google.android.apps.healthdata"') && manifest.includes('https://example.com/privacy'), manifest);
  const apk = existsSync(out) ? readdirSync(out, { recursive: true }).map(String).find((f) => f.endsWith('.apk') && f.includes('release')) : null;
  for (const file of ['health.json', 'other-health.json']) {
    const bytes = apk ? spawnSync('unzip', ['-p', resolve(out, apk), `assets/app/assets/health-fixture/${file}`], { maxBuffer: 1 << 24 }).stdout : null;
    check('android', `${file} is carried as the agent's store`, bytes?.length > 0 && sha(bytes) === sha(readFileSync(resolve(pkg, 'fixture', file))), `${bytes?.length ?? 0} bytes`);
  }
  if (apk) console.log(`APK ${apk}: ${(statSync(resolve(out, apk)).size / 1048576).toFixed(2)} MiB`);

  if (process.argv.includes('--emulator')) {
    if (!process.env.ANDROID_SERIAL) throw new Error('--emulator: name the emulator in ANDROID_SERIAL, so no other device is driven');
    /** One launch: each op's JSON reply. */
    const drive = (flags, ops) => {
      const r = spawnSync('bun', ['scripts/agent.mjs', 'android', '--reader', '--json', '--app', name, '--epoch', '2026-10-25T12:00:00Z', ...flags, ...ops],
        { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 26 });
      const replies = (r.stdout ?? '').split('\n').filter((l) => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map((l) => JSON.parse(l));
      if (replies.length < ops.length) console.log(r.stdout.slice(-3000), r.stderr.slice(-3000));
      return replies;
    };
    // 1: every read granted: the fixture's days and workouts, complete from 30 days back.
    {
      const t = texts(drive(['--permission', 'health=granted'], ['clock settle', 'tree']).at(-1));
      check('android', '1: granted, each type complete', t['health-status'] === 'true granted'
        && t['health-types'] === 'steps granted true, distance granted true, workouts granted true', `${t['health-status']} | ${t['health-types']}`);
      check('android', "1: 11 to 13 October's steps", t.days === '11625,8412,10461', t.days);
      check('android', "1: 12 October's two workouts", t.workouts === 'w7 walking, w8 other', t.workouts);
    }
    // 2: asked, and steps allowed alone: the rest read denied, and the workouts' read is refused.
    {
      const t = texts(drive([], ['clock settle', 'tap ask', 'clock settle', 'type @permission steps', 'clock settle', 'tree']).at(-1));
      check('android', '2: steps granted, the rest denied', t['health-types'] === 'steps granted true, distance denied false, workouts denied false', t['health-types']);
      check('android', '2: the workouts are refused, the steps read', t['workouts-error'] === 'NotAllowedError' && t.days === '11625,8412,10461', `${t['workouts-error']} | ${t.days}`);
    }
    // 3: bound: Health Connect's own status for an app it never asked, which reads no data; the barometer the emulator
    // simulates; and the pedometer's status, which depends on the image's step counter and Play services.
    {
      const t = texts(drive(['--bind', 'health', '--bind', 'pedometer', '--bind', 'altimeter'], ['clock +3000 real', 'clock settle', 'tree']).at(-1));
      check('android', '3: bound, Health Connect is available and every type prompt', t['health-status'] === 'true prompt'
        && t['health-types'] === 'steps prompt false, distance prompt false, workouts prompt false', `${t['health-status']} | ${t['health-types']}`);
      check('android', '3: bound, the barometer is available and asks nothing', t['height-status'] === 'true granted', t['height-status']);
      console.log(`bound pedometer status: ${t['steps-status']}`);
    }
  }
} finally {
  rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
