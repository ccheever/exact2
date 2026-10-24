// @exact/passkeys on Android (LLP 1133.011 §11 stage 3): the auth fixture's Contract and data code in an app `exact
// new` made outside the repo, depending on this package by `file:`, built as the Canvas reader's APK, which compiles
// Passkeys.kt against `androidx.credentials` 1.6.0 in the app's one locked, verified Gradle graph (LLP 1133.008 D8
// item 5). It checks the graph and the roster, prints the APK's size (D15's cost), and drives nothing: the emulator's
// real ceremonies need the fixture's public origin and its assetlinks.json (D14), and checklist.md is the phone's.
//
// usage: bun packages/passkeys/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const pkg = resolve(import.meta.dir, '..'), root = resolve(pkg, '../..');
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failed = 0;
const check = (name, ok, detail) => {
  if (!ok) failed++;
  console.log(`${ok ? 'PASS' : 'FAIL'} android ${name}${ok ? '' : `: ${detail}`}`);
};

const dir = mkdtempSync(join(tmpdir(), 'exact-passkeys-')), app = join(dir, 'passkeysfixture'), name = basename(app);
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
cpSync(resolve(root, 'apps/auth-fixture/app.contract'), join(app, 'app.contract'));
// The data code names its app, which here is the one `exact new` made.
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).id;
writeFileSync(join(app, 'app.ts'), readFileSync(resolve(root, 'apps/auth-fixture/app.ts'), 'utf8').replace("appId = 'com.exact.authfixture'", `appId = '${id}'`));
rmSync(join(app, 'app.test.contract'), { force: true });
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@exact/passkeys': `file:${pkg}` };
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
const lock = resolve(app, 'gradle.lockfile'), checksums = resolve(app, 'gradle-verification.xml');
try {
  const written = build('--write-locks');
  check('--write-locks builds the APK, compiling Passkeys.kt', written.status === 0, (written.stdout + written.stderr).slice(-3000));
  const locked = existsSync(lock) ? readFileSync(lock, 'utf8') : '';
  check('the lock names Credential Manager and its Play services provider',
    /^androidx\.credentials:credentials:1\.6\.0=/m.test(locked) && /^androidx\.credentials:credentials-play-services-auth:1\.6\.0=/m.test(locked), locked.slice(0, 400) || `no ${lock}`);
  const sums = existsSync(checksums) ? readFileSync(checksums, 'utf8') : '';
  check('the checksums cover the graph', /<component group="androidx\.credentials" name="credentials" version="1\.6\.0">/.test(sums) && /sha256/.test(sums), sums.slice(0, 400) || `no ${checksums}`);
  const generated = resolve(root, 'host/android/app/build', name, 'extensions/exact/app/ExactExtensions.kt');
  const roster = existsSync(generated) ? readFileSync(generated, 'utf8') : '';
  check('the roster names the passkeys extension', roster.includes('exact.passkeys.PasskeysModule'), roster || `no ${generated}`);
  const strict = build();
  check('a build checks the graph against the lock and checksums', strict.status === 0, (strict.stdout + strict.stderr).slice(-2000));
  const out = resolve(root, 'host/android/app/build', name);
  const apk = existsSync(out) ? readdirSync(out, { recursive: true }).map(String).find((f) => f.endsWith('.apk') && f.includes('release')) : null;
  if (apk) console.log(`APK ${apk}: ${(statSync(resolve(out, apk)).size / 1048576).toFixed(2)} MiB`);
} finally {
  rmSync(dirname(app), { recursive: true, force: true });
}
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
