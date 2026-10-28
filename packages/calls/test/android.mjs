// @exact/calls on Android (LLP 1133.033 §12 stage 4): the fixture app (`test/fixture`) built as the Canvas reader's
// APK, which compiles the package's Kotlin against Core-Telecom and merges its manifest fragment. Checked: Core-Telecom
// joins the app's one locked, verified Gradle graph (`--write-locks`, then a strict build), the roster binds the
// implementation, `device.calls` derives the `Call` service as `phoneCall|microphone` with `MANAGE_OWN_CALLS` and
// `USE_FULL_SCREEN_INTENT`, and the notification's receiver is compiled in under its name. It drives nothing: no
// emulator boots on the Mac that wrote it, so drives 4 to 9 with `--bind calls` and drive 13 are owed (LLP 1133.020).
//
// usage: bun packages/calls/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();

const dir = mkdtempSync(join(tmpdir(), 'exact-calls-')), name = 'callsfixture', app = join(dir, name);
try {
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const strings = resolve(app, 'strings/en.json');
  mkdirSync(resolve(app, 'strings'), { recursive: true });
  writeFileSync(strings, JSON.stringify({ ...(existsSync(strings) ? JSON.parse(readFileSync(strings, 'utf8')) : {}), ...JSON.parse(readFileSync(join(here, 'fixture/strings/en.json'), 'utf8')) }, null, 2));
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/calls': `file:${pkg}`, '@exact/media-capture': `file:${resolve(root, 'packages/media-capture')}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
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
  const env = { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1' };
  const build = (...flags) => spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk', ...flags], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });

  const written = build('--write-locks');
  check('android', '--write-locks builds the APK', written.status === 0, (written.stdout + written.stderr).slice(-3000));
  const locked = existsSync(resolve(app, 'gradle.lockfile')) ? readFileSync(resolve(app, 'gradle.lockfile'), 'utf8') : '';
  check('android', 'the lock names Core-Telecom', /androidx\.core:core-telecom:1\.0\.1/.test(locked), locked.slice(0, 400) || 'no gradle.lockfile');
  const sums = existsSync(resolve(app, 'gradle-verification.xml')) ? readFileSync(resolve(app, 'gradle-verification.xml'), 'utf8') : '';
  check('android', 'the checksums cover it', /name="core-telecom" version="1\.0\.1"/.test(sums), sums.slice(0, 400) || 'no gradle-verification.xml');
  const strict = build();
  check('android', 'a build checks the graph against the lock and checksums', strict.status === 0, (strict.stdout + strict.stderr).slice(-2000));

  const out = resolve(root, 'host/android/app/build', name);
  const roster = existsSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt')) ? readFileSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt'), 'utf8') : '';
  check('android', 'the roster binds the implementation', roster.includes('exact.calls.CallsModule'), roster);
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const xml = merged ? readFileSync(resolve(out, merged), 'utf8') : '';
  const permissions = ['MANAGE_OWN_CALLS', 'FOREGROUND_SERVICE', 'FOREGROUND_SERVICE_PHONE_CALL', 'FOREGROUND_SERVICE_MICROPHONE', 'USE_FULL_SCREEN_INTENT'];
  const missing = permissions.filter((p) => !new RegExp(`<uses-permission\\s+android:name="android\\.permission\\.${p}"`).test(xml));
  check('android', 'device.calls derives its permissions', !missing.length, `missing ${missing.join(', ')}`);
  const service = xml.match(/<service[^>]*ForegroundService\$Call[^>]*>/)?.[0] ?? '';
  check('android', 'the Call service is phoneCall|microphone', /foregroundServiceType="(phoneCall\|microphone|microphone\|phoneCall)"/.test(service), service || merged || 'no merged manifest');
  const receiver = xml.match(/<receiver[^>]*exact\.calls\.CallActionReceiver[^>]*>/)?.[0] ?? '';
  check('android', 'the notification receiver merged, unexported', /android:exported="false"/.test(receiver), receiver);
  const dex = spawnSync('unzip', ['-p', resolve(out, 'outputs/apk/release/app-release.apk'), 'classes*.dex'], { maxBuffer: 1 << 28 }).stdout ?? Buffer.alloc(0);
  check('android', 'the receiver is compiled in under its name', dex.includes('Lexact/calls/CallActionReceiver;'), 'absent');
} finally {
  rmSync(dir, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
