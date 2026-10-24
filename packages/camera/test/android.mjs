// @exact/camera and media-capture's video half on Android (LLP 1133.008 stage 4): the fixture app made outside the
// repo, given a date control so Compose's AndroidX and CameraX's resolve in one APK (§10), and built as the Canvas
// reader's APK, which compiles both packages' Kotlin. It checks the one locked, verified Gradle graph (D8 item 5):
// the build refuses a graph with no lock, `--write-locks` writes the lock and checksums beside the app's other locks,
// a build checks against both, and an artifact the lock does not name is refused by name. It drives nothing;
// checklist.md is the phone's.
//
// usage: bun packages/camera/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, makeFixture, root, run } from './fixture.mjs';

const { app } = makeFixture(), name = basename(app);
// A date control: the plan uses `dates`, so the APK links Compose Material 3 beside CameraX.
const contract = readFileSync(resolve(app, 'app.contract'), 'utf8')
  .replace('  state placed = true\n', '  state placed = true\n  state day = "2026-10-23"\n')
  .replace('  action unplace\n', '  action setDay(value: string)\n    day = value\n  action unplace\n')
  .replace('      if placed\n', '      input type="date" value=day change=setDay aria-label="Day" testId="day"\n      if placed\n');
writeFileSync(resolve(app, 'app.contract'), contract);
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
const env = { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' };
const build = (...flags) => spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk', ...flags], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
const lock = resolve(app, 'gradle.lockfile'), checksums = resolve(app, 'gradle-verification.xml');
try {
  const unlocked = build();
  check('android', 'a graph with no lock is refused, naming how to write it', unlocked.status !== 0 && /--write-locks/.test(unlocked.stderr), unlocked.stderr.slice(-600));

  const written = build('--write-locks');
  check('android', '--write-locks builds the APK', written.status === 0, (written.stdout + written.stderr).slice(-2000));
  const locked = existsSync(lock) ? readFileSync(lock, 'utf8') : '';
  check('android', 'the lock names CameraX and Compose in one graph', /androidx\.camera:camera-core:1\.6\.2/.test(locked) && /androidx\.compose\.material3:material3:/.test(locked), locked.slice(0, 400) || `no ${lock}`);
  const sums = existsSync(checksums) ? readFileSync(checksums, 'utf8') : '';
  check('android', 'the checksums cover the graph', /<component group="androidx\.camera" name="camera-view" version="1\.6\.2">/.test(sums) && /sha256/.test(sums), sums.slice(0, 400) || `no ${checksums}`);
  check('android', "the reader's gradle/ keeps no verification file after the build", !existsSync(resolve(root, 'host/android/gradle/verification-metadata.xml')));

  const generated = resolve(root, 'host/android/app/build', name, 'extensions/exact/app/ExactExtensions.kt');
  const roster = existsSync(generated) ? readFileSync(generated, 'utf8') : '';
  check('android', 'the roster names both extensions and the viewfinder', roster.includes('"exact-camera" to "camera"') && roster.includes('exact.mediacapture.MediaCaptureModule') && roster.includes('exact.camera.CameraModule'), roster || `no ${generated}`);
  const merged = readdirSync(resolve(root, 'host/android/app/build', name), { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const manifest = merged ? readFileSync(resolve(root, 'host/android/app/build', name, merged), 'utf8') : '';
  check('android', 'the manifest asks CAMERA and requires no camera feature', manifest.includes('android.permission.CAMERA') && /android.hardware.camera.any"\s+android:required="false"/.test(manifest), merged ?? 'no merged manifest');

  const strict = build();
  check('android', 'a build checks the graph against the lock and checksums', strict.status === 0, (strict.stdout + strict.stderr).slice(-2000));

  writeFileSync(lock, locked.split('\n').filter((l) => !l.startsWith('androidx.camera:camera-view:')).join('\n'));
  const refused = build();
  check('android', 'an artifact the lock does not name is refused by name', refused.status !== 0 && /camera-view/.test(refused.stdout + refused.stderr), (refused.stdout + refused.stderr).slice(-1500));
} finally {
  rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
