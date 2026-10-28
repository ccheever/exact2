// @exact/maps on Android (LLP 1133.021 §15 stage 3): Map Demo's view made outside the repo by `exact new`, built as
// the Canvas reader's APK, which compiles Maps.kt against the Maps SDK in the app's one locked, verified Gradle graph
// (LLP 1133.008 D8 item 5). The Maps key (§9 item 7, D18) comes from the variable `app.json` names, here a value that
// is not a key: the build writes it as `com.google.android.geo.API_KEY`; without it a development build says so and
// leaves the meta-data out, and a release build is refused `maps-key-missing` before it compiles. No map is drawn:
// the APK is not installed anywhere (checklist.md's Android part is a person's, with a real key).
//
// `androidFixture` and `apkOf` are the geocoding and geofencing suites' too.
//
// usage: bun packages/maps/test/android.mjs
//   (an NDK of clang 18 or newer in ANDROID_NDK_HOME, and a JDK 17 or newer in JAVA_HOME)
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const root = resolve(import.meta.dir, '../../..');

const run = (cmd, args, cwd) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};

/** A suite's PASS or FAIL line; `failed()` counts the FAILs. */
export function checker() {
  let failures = 0;
  const check = (name, ok, detail) => {
    if (!ok) failures++;
    console.log(`${ok ? 'PASS' : 'FAIL'} android ${name}${ok ? '' : `: ${typeof detail === 'string' ? detail : JSON.stringify(detail)}`}`);
  };
  return { check, failed: () => failures };
}

/** An app `exact new` made in a temporary directory, its `app.contract` replaced by `contract`, `grants` in its
 * `app.ts` (which has no data sources), `strings` merged into its `strings/en.json` and `packages` into its
 * `app.json`, depending on each of `uses` (this checkout's `packages/<name>`) packed as a registry carries it, with
 * Weatherlight's Canvas library as its `android/` crate. `done()` removes it. */
export function androidFixture(name, { contract, uses, grants = '', strings = {}, packages = {} }) {
  const dir = mkdtempSync(join(tmpdir(), `exact-${name}-`)), app = join(dir, name);
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  writeFileSync(join(app, 'app.contract'), contract);
  rmSync(join(app, 'app.test.contract'), { force: true });
  // No data sources: what the fixture shows comes from its package.
  const id = readFileSync(join(app, 'app.ts'), 'utf8').match(/export const appId = '([^']+)'/)[1];
  writeFileSync(join(app, 'app.ts'), [`import type { Answer, Sources } from './app.contract.d.ts';`, '',
    `export const appId = '${id}';`, `export const grants = ${JSON.stringify(grants)};`, '', 'const sources: Sources = {};',
    'export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);', ''].join('\n'));
  const en = join(app, 'strings/en.json');
  mkdirSync(join(app, 'strings'), { recursive: true });
  writeFileSync(en, JSON.stringify({ ...(existsSync(en) ? JSON.parse(readFileSync(en, 'utf8')) : {}), ...strings }, null, 2));
  const json = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8'));
  json.packages = { ...json.packages, ...packages };
  writeFileSync(join(app, 'app.json'), JSON.stringify(json, null, 2));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  for (const used of uses) {
    const into = mkdtempSync(join(dir, 'pack-'));
    run('bun', ['pm', 'pack', '--destination', into, '--quiet'], resolve(root, 'packages', used));
    manifest.dependencies = { ...manifest.dependencies, [`@exact/${used}`]: `file:${join(into, readdirSync(into).find((f) => f.endsWith('.tgz')))}` };
  }
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
  const out = resolve(root, 'host/android/app/build', name);
  /** `agent-android.mjs build … --apk` with `flags`, under `env` added to this process's. */
  const build = (flags = [], env = {}) => spawnSync('bun', [resolve(root, 'scripts/agent-android.mjs'), 'build', name, '--apk', ...flags],
    { cwd: root, env: { ...process.env, EXACT_APP_DIR: app, EXACT2: root, HERMES_LEAN_SYS_OFFLINE: '1', ...env }, encoding: 'utf8', maxBuffer: 1 << 28 });
  return { app, out, build, done: () => rmSync(dir, { recursive: true, force: true }) };
}

/** What the build left under `out`: the roster, the merged release manifest, the lock and the dex. */
export function apkOf(app, out) {
  const read = (file) => (existsSync(file) ? readFileSync(file, 'utf8') : '');
  const merged = existsSync(out) && readdirSync(out, { recursive: true }).map(String)
    .find((f) => f.includes('merged_manifests/release/') && f.endsWith('AndroidManifest.xml'));
  const apk = resolve(out, 'outputs/apk/release/app-release.apk');
  return {
    roster: read(resolve(out, 'extensions/exact/app/ExactExtensions.kt')),
    manifest: merged ? read(resolve(out, merged)) : '',
    lock: read(resolve(app, 'gradle.lockfile')),
    dex: existsSync(apk) ? spawnSync('unzip', ['-p', apk, 'classes*.dex'], { maxBuffer: 1 << 28 }).stdout ?? Buffer.alloc(0) : Buffer.alloc(0),
  };
}

if (import.meta.main) {
  const { check, failed } = checker();
  const demo = readFileSync(resolve(root, 'apps/map-demo/app.contract'), 'utf8');
  const fixture = androidFixture('mapfixture', {
    contract: demo, uses: ['maps'],
    packages: { '@exact/maps': JSON.parse(readFileSync(resolve(root, 'apps/map-demo/app.json'), 'utf8')).packages['@exact/maps'] },
  });
  const KEY = 'com.google.android.geo.API_KEY', meta = (xml) => xml.match(new RegExp(`<meta-data[^>]*android:name="${KEY.replaceAll('.', '\\.')}"[^>]*>`))?.[0] ?? '';
  try {
    {
      const r = fixture.build(['--write-locks'], { EXACT_GOOGLE_MAPS_API_KEY: 'test-not-a-key' });
      check('the APK builds, compiling Maps.kt against the Maps SDK', r.status === 0, (r.stdout + r.stderr).slice(-3000));
      const { roster, manifest, lock, dex } = apkOf(fixture.app, fixture.out);
      check("the lock names Play services' maps", /^com\.google\.android\.gms:play-services-maps:20\.1\.0=/m.test(lock), lock.slice(0, 400) || 'no gradle.lockfile');
      check('the roster binds the module and its view', roster.includes('"maps" to { c -> exact.maps.MapsModule(c) }') && roster.includes('"exact-map" to "maps"'), roster);
      check('the key is the meta-data the Maps SDK reads, from the variable', /android:value="test-not-a-key"/.test(meta(manifest)), manifest.slice(0, 3000));
      // R8 may rename the SDK's classes, not move them out of their package.
      check("the Maps SDK's classes are compiled in", dex.includes('Lcom/google/android/gms/maps/'), `${dex.length} bytes of dex`);
    }
    {
      const r = fixture.build([], { EXACT_GOOGLE_MAPS_API_KEY: '' });
      check('without the variable a development build builds', r.status === 0, (r.stdout + r.stderr).slice(-3000));
      check('and says the key is absent', r.stderr.includes('EXACT_GOOGLE_MAPS_API_KEY is unset'), r.stderr.slice(-1500));
      check('and carries no key', meta(apkOf(fixture.app, fixture.out).manifest) === '', meta(apkOf(fixture.app, fixture.out).manifest));
    }
    {
      const r = fixture.build(['--release'], { EXACT_GOOGLE_MAPS_API_KEY: '' });
      check('without the variable a release build is refused maps-key-missing, before it compiles',
        r.status !== 0 && r.stderr.includes('maps-key-missing') && !/Compiling|Task :app/.test(r.stdout + r.stderr), (r.stdout + r.stderr).slice(-1500));
    }
  } finally {
    fixture.done();
  }
  console.log(failed() ? `${failed()} failed` : 'all passed');
  process.exit(failed() ? 1 : 0);
}
