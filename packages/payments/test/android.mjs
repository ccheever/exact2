// @exact/payments on Android (LLP 1133.034 §12 stage 3): the fixture app (`test/fixture`) built as the Canvas reader's
// APK, which compiles the package's Kotlin against Play services' wallet library and merges its manifest fragment under
// the allow-list (LLP 1133.030 D9). The build is checked for the roster binding the implementation, the callbacks
// service exported only to `BIND_PAYMENTS_CALLBACK_SERVICE` with Google's action, the wallet's meta-data switch, and
// the classes a manifest names compiled in under their names. No emulator boots on the Mac that built it, and a
// payment needs the Google test card suite's account (checklist.md); this prints the APK's path for that phone.
//
// usage: bun packages/payments/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
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

const dir = mkdtempSync(join(tmpdir(), 'exact-payments-')), name = 'paymentsfixture', app = join(dir, name);
try {
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/payments': `file:${pkg}` };
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
  // A new app's Maven graph has no lock yet: this build writes it in the scratch app (camera's suite checks the refusal).
  const built = spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk', '--write-locks'], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
  check('android', 'the fixture builds as an APK, its Kotlin compiled against play-services-wallet', built.status === 0, (built.stdout + built.stderr).slice(-3000));

  const out = resolve(root, 'host/android/app/build', name);
  const roster = existsSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt')) ? readFileSync(resolve(out, 'extensions/exact/app/ExactExtensions.kt'), 'utf8') : '';
  check('android', 'the roster binds the implementation', roster.includes('"payments" to { c -> exact.payments.PaymentsModule(c) }'), roster);
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).find((f) => String(f).includes('merged_manifests/release/') && String(f).endsWith('AndroidManifest.xml'));
  const xml = merged ? readFileSync(resolve(out, merged), 'utf8') : '';
  const service = xml.match(/<service[^>]*android:name="exact\.payments\.PaymentCallbacks"[^>]*>[\s\S]*?<\/service>/)?.[0] ?? '';
  check('android', 'the callbacks service is exported only to Google Pay\'s binder, with its one action',
    /android:exported="true"/.test(service) && /android:permission="com\.google\.android\.gms\.permission\.BIND_PAYMENTS_CALLBACK_SERVICE"/.test(service)
      && /com\.google\.android\.gms\.wallet\.callback\.PAYMENT_DATA_CALLBACKS/.test(service), service || (merged ?? 'no merged manifest'));
  check('android', 'the wallet API switch is on', /<meta-data\s+android:name="com\.google\.android\.gms\.wallet\.api\.enabled"\s+android:value="true"/.test(xml), merged ?? 'no merged manifest');
  // A payment asks the system for no permission; Play services' own manifest may add its own, which are not the grant's.
  check('android', 'payment.google-pay derives no permission of its own', !/PAYMENT|WALLET/.test((xml.match(/<uses-permission[^>]*>/g) ?? []).join('\n')), xml.match(/<uses-permission[^>]*>/g));

  const apk = resolve(out, 'outputs/apk/release/app-release.apk');
  const dex = spawnSync('unzip', ['-p', apk, 'classes*.dex'], { maxBuffer: 1 << 28 }).stdout ?? Buffer.alloc(0);
  const absent = ['PaymentCallbacks'].filter((c) => !dex.includes(`Lexact/payments/${c};`));
  check('android', 'the callbacks service is compiled in under its name', !absent.length, `missing ${absent.join(', ')}`);
  check('android', "Play services' wallet classes are in the APK", dex.includes('Lcom/google/android/gms/wallet/'), 'no com/google/android/gms/wallet classes');
  if (existsSync(apk)) {
    const kept = resolve(root, 'target/payments-fixture.apk');
    mkdirSync(resolve(root, 'target'), { recursive: true });
    copyFileSync(apk, kept);
    console.log(`APK: ${kept} (id ${id})`);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
