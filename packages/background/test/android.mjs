// @exact/background on Android (LLP 1133.017 §14 stage 3): the fixture app (`test/fixture`) built as the Canvas
// reader's APK, which compiles the package's Kotlin, with WorkManager in the one locked, verified Gradle graph (LLP
// 1133.008 D8 item 5): a graph with no lock is refused, `--write-locks` writes the lock and checksums beside the app's
// other locks, and a build checks against both. The roster binds the implementation, and WorkManager's library
// manifest merges its job service and the permissions it uses, so the package declares none. With `--emulator
// <serial>` it drives the APK on that running emulator under the agent with the implementation bound (`--bind
// background`): the status, a tag fired at once as the implementation's own wake, and `registerPeriodicSync`.
//
// usage: bun packages/background/test/android.mjs [--emulator <serial>]
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), emulator = argv.includes('--emulator') ? argv[argv.indexOf('--emulator') + 1] : null;
const run = (cmd, args, cwd, env = process.env, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();

// The fixture's handler pulls from this server, whose origin the build writes into the app; the emulator reaches it
// at its own 127.0.0.1 (`adb reverse`).
let pulls = 0;
const puller = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch() { pulls++; return new Response('ok'); } });

const dir = mkdtempSync(join(tmpdir(), 'exact-background-')), name = 'bgfixture', app = join(dir, name);
try {
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__PULL_ORIGIN__', `http://127.0.0.1:${puller.port}`));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/background': `file:${pkg}` };
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

  const unlocked = build();
  check('android', 'a graph with no lock is refused, naming how to write it', unlocked.status !== 0 && /--write-locks/.test(unlocked.stderr), unlocked.stderr.slice(-600));
  const written = build('--write-locks');
  check('android', '--write-locks builds the APK', written.status === 0, (written.stdout + written.stderr).slice(-2000));
  const locked = existsSync(lock) ? readFileSync(lock, 'utf8') : '';
  check('android', 'the lock names WorkManager', /androidx\.work:work-runtime:2\.12\.0/.test(locked), locked.slice(0, 400) || `no ${lock}`);
  const sums = existsSync(checksums) ? readFileSync(checksums, 'utf8') : '';
  check('android', 'the checksums cover it', /<component group="androidx\.work" name="work-runtime" version="2\.12\.0">/.test(sums) && /sha256/.test(sums), sums.slice(0, 400) || `no ${checksums}`);

  const generated = resolve(root, 'host/android/app/build', name, 'extensions/exact/app/ExactExtensions.kt');
  const roster = existsSync(generated) ? readFileSync(generated, 'utf8') : '';
  check('android', 'the roster binds the implementation', roster.includes('exact.background.BackgroundModule'), roster || `no ${generated}`);
  const merged = readdirSync(resolve(root, 'host/android/app/build', name), { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const xml = merged ? readFileSync(resolve(root, 'host/android/app/build', name, merged), 'utf8') : '';
  check('android', "the manifest carries WorkManager's job service and the network state permission", xml.includes('androidx.work.impl.background.systemjob.SystemJobService')
    && xml.includes('android.permission.ACCESS_NETWORK_STATE'), merged ?? 'no merged manifest');

  const strict = build();
  check('android', 'a build checks the graph against the lock and checksums', strict.status === 0, (strict.stdout + strict.stderr).slice(-2000));

  if (emulator) {
    run('adb', ['-s', emulator, 'reverse', `tcp:${puller.port}`, `tcp:${puller.port}`], root);
    Object.assign(process.env, env, { ANDROID_SERIAL: emulator, EXACT_ANDROID_READER: '1' });
    const { open } = await import(resolve(root, 'scripts/agent.mjs'));
    const s = await open({ host: 'android', app: name, bind: 'background', storage: `bgfixture-${process.pid}` });
    try {
      const status = await until(s, 'status', (t) => /^true /.test(t ?? ''), 30_000);
      check('android', 'the status reads true granted true true', status === 'true granted true true', status);
      await s.tap('queue-t1');
      const pulled = await until(s, 'pulled', (t) => t === 't1 true false', 30_000);
      check('android', 'registered online, the wake runs the handler, whose request lands', pulled === 't1 true false' && pulls === 1, `${pulled} pulls=${pulls}`);
      const retired = await until(s, 'tags', (t) => t === '', 15_000);
      check('android', 'its invocation completed, the tag is retired', retired === '', `${retired} ${JSON.stringify(await texts(s))}`);
      await s.tap('try-periodic');
      const periodic = await until(s, 'periodic', (t) => t !== 'none', 15_000);
      check('android', 'registerPeriodicSync registers', periodic === 'registered', periodic);
      if (failed()) console.log((await s.logs({ all: true })).lines.filter((l) => /background|wake/.test(l)).join('\n'));
    } finally {
      await s.close();
    }
    const jobs = run('adb', ['-s', emulator, 'shell', 'dumpsys', 'jobscheduler'], root, process.env, true);
    console.log(`note android WorkManager's jobs for the fixture: ${jobs.split('\n').filter((l) => /SystemJobService/.test(l) && /bgfixture/.test(l)).length}`);
  }
} finally {
  puller.stop(true);
  rmSync(dir, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
