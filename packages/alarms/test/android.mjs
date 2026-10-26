// @exact/alarms on Android (LLP 1133.030 §14 stage 2): the fixture app (`test/fixture`) built as the Canvas reader's
// APK, which compiles the package's Kotlin and merges its manifest fragment under the allow-list (D9). The build is
// checked for what the grant derives (D7), the fragment's three components (unexported, `directBootAware`, the service
// `systemExempted`) and the roster binding the implementation.
//
// With `--emulator <serial>` it then drives the APK on that running emulator under the agent with the implementation
// bound (`--bind alarms`), on the wall clock, since the system's alarms run on it: the status before asking, a request
// answered through the system's notification dialog and the exact-alarm Settings page, a one-minute countdown that
// `dumpsys alarm` lists as an alarm clock, pause and resume, the ring with its notification's Stop and Snooze, and a
// revocation that ends the alarm. That half has never run: no emulator booted on the Mac that built it (LLP 1133.030
// §14), so its first run is its test. What needs a person or a phone is checklist.md's.
//
// usage: bun packages/alarms/test/android.mjs [--emulator <serial>]
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), emulator = argv.includes('--emulator') ? argv[argv.indexOf('--emulator') + 1] : null;
const run = (cmd, args, cwd, env = process.env, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();

const dir = mkdtempSync(join(tmpdir(), 'exact-alarms-')), name = 'alarmfixture', app = join(dir, name);
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
  manifest.dependencies = { ...manifest.dependencies, '@exact/alarms': `file:${pkg}` };
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
  const built = spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk'], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  check('android', 'the fixture builds as an APK', built.status === 0, (built.stdout + built.stderr).slice(-3000));

  const out = resolve(root, 'host/android/app/build', name);
  const roster = existsSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt')) ? readFileSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt'), 'utf8') : '';
  check('android', 'the roster binds the implementation', roster.includes('"alarms" to { c -> exact.alarms.AlarmsModule(c) }'), roster);
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const xml = merged ? readFileSync(resolve(out, merged), 'utf8') : '';
  const permissions = ['SCHEDULE_EXACT_ALARM', 'POST_NOTIFICATIONS', 'RECEIVE_BOOT_COMPLETED', 'WAKE_LOCK', 'FOREGROUND_SERVICE', 'FOREGROUND_SERVICE_SYSTEM_EXEMPTED'];
  const missing = permissions.filter((p) => !new RegExp(`<uses-permission\\s+android:name="android\\.permission\\.${p}"`).test(xml));
  check('android', 'device.alarms derives its six permissions, and not USE_EXACT_ALARM', !missing.length && !xml.includes('USE_EXACT_ALARM'), `missing ${missing.join(', ')}`);
  const element = (tag, cls) => xml.match(new RegExp(`<${tag}[^>]*android:name="exact\\.alarms\\.${cls}"[^>]*>`))?.[0] ?? '';
  const service = element('service', 'AlarmService'), ring = element('receiver', 'AlarmReceiver'), system = element('receiver', 'AlarmSystemReceiver');
  check('android', 'the fragment merged: two receivers and the service, none exported, each directBootAware',
    [service, ring, system].every((e) => /android:exported="false"/.test(e) && /android:directBootAware="true"/.test(e)), [service, ring, system]);
  check('android', 'the ringing service is systemExempted', /android:foregroundServiceType="systemExempted"/.test(service), service);
  check('android', 'the system receiver hears the boot before unlock and the exact-alarm switch',
    /LOCKED_BOOT_COMPLETED/.test(xml) && /SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED/.test(xml), merged ?? 'no merged manifest');

  // R8 keeps the classes a manifest names by their names; the module itself it may rename.
  const dex = spawnSync('unzip', ['-p', resolve(out, 'outputs/apk/release/app-release.apk'), 'classes*.dex'], { maxBuffer: 1 << 28 }).stdout ?? Buffer.alloc(0);
  const absent = ['AlarmReceiver', 'AlarmSystemReceiver', 'AlarmService'].filter((c) => !dex.includes(`Lexact/alarms/${c};`));
  check('android', "the fragment's three classes are compiled in under their names", !absent.length, `missing ${absent.join(', ')}`);

  if (emulator) await import('./android-drive.mjs').then((m) => m.drive({ root, env, name, id, emulator, check, run }));
} finally {
  rmSync(dir, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
