// @exact/motion on Android (LLP 1133.013 stage 4): the fixture app made outside the repo and built as the Canvas
// reader's APK, which compiles the package's Kotlin, and the roster that names its module. It drives nothing: the
// emulator's sensors are virtual and the agent's substitute answers under it; phone-checklist.md is the phone's.
//
// usage: bun packages/motion/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, makeFixture, root, run } from './fixture.mjs';

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
const env = { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' };
try {
  const built = spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk'], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  check('android', 'the APK builds, compiling Motion.kt', built.status === 0, (built.stdout + built.stderr).slice(-2000));
  const generated = resolve(root, 'host/android/app/build', name, 'extensions/exact/app/ExactExtensions.kt');
  const roster = existsSync(generated) ? readFileSync(generated, 'utf8') : '';
  check('android', 'the roster names the motion module', roster.includes('exact.motion.MotionModule'), roster || `no ${generated}`);
} finally {
  rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
