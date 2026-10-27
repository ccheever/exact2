// The seam is public (LLP 1133.001 D8, LLP 1133.031 §12 stage 5): two tests
// that need nothing an outside author lacks.
//
// - Out of the tree: this package, packed as a registry would carry it
//   (`bun pm pack`, npm's tarball), installed into an app `exact new` made
//   outside the repo, builds unchanged: for the web, where Chromium answers
//   the page's battery; for macOS, where the Apple implementation is bound
//   and read (`apple.mjs macos`); for Android, where the APK compiles it and
//   merges its manifest fragment (`android.mjs`). Each suite makes its
//   fixture from the tarball (`EXACT_DEVICE_PACKAGE`).
// - Replaced: a fixture implementation outside the tree, a direct dependency
//   of the app, replaces the endorsed web implementation and answers every
//   source, `powerState` included, which the endorsed one cannot.
//
// usage: bun packages/device/test/seams.mjs [web] [macos] [android]
//   (all three by default; Android needs what android.mjs names)
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker, here, makeFixture, pkg, root, run } from './fixture.mjs';

const argv = process.argv.slice(2), targets = argv.length ? argv : ['web', 'macos', 'android'];
const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-device-seams-'));

/** A built app's page in Chromium, and the fixture's three facts once each has answered. */
async function facts(app) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  // The build writes under the app's target directory, which CARGO_TARGET_DIR moves (scripts/app.mjs).
  const dist = join(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(app, 'target'), 'web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const read = (id) => page.getByTestId(id).innerText().catch(() => '');
    const end = Date.now() + 10000;
    let got = [];
    while (Date.now() < end) {
      got = await Promise.all(['battery', 'network', 'power'].map(read));
      if (got.every(Boolean)) break;
      await page.waitForTimeout(50);
    }
    return got.join(' / ');
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarball, installed as any registry package is.
run('bun', ['pm', 'pack', '--destination', scratch], pkg);
const tarball = join(scratch, readdirSync(scratch).find((f) => f.endsWith('.tgz')));
const device = `file:${tarball}`;
if (targets.includes('web')) {
  const packed = makeFixture(undefined, { device });
  const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/device/package.json'), 'utf8'));
  check('packed', 'the tarball is installed, not a link into the checkout', installed.name === '@exact/device' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), JSON.stringify(installed));
  const read = await facts(packed.app);
  check('packed', 'it builds unchanged for the web and Chromium answers', read === '1 true / unknown false / NotSupportedError', read);
  rmSync(dirname(packed.app), { recursive: true, force: true });
}
for (const [target, suite, args] of [['macos', 'apple.mjs', ['macos']], ['android', 'android.mjs', []]]) {
  if (!targets.includes(target)) continue;
  const r = spawnSync('bun', [join(here, suite), ...args], { cwd: root, env: { ...process.env, EXACT_DEVICE_PACKAGE: device }, encoding: 'utf8', maxBuffer: 1 << 28 });
  for (const line of r.stdout.split('\n').filter((l) => /^(PASS|FAIL|RECORD) /.test(l))) console.log(`  ${line}`);
  check('packed', `it builds unchanged for ${target}, and ${suite} passes over it`, r.status === 0, (r.stdout + r.stderr).slice(-2000));
}

// Replaced: an implementation outside the tree that answers each source with values no platform gives here.
if (targets.includes('web')) {
  const dir = join(scratch, 'fixture-device-web');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/device-web', version: '1.0.0', exports: { '.': './exact.json' } }));
  writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/device@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
  writeFileSync(join(dir, 'index.js'), `export function implementation() {
  return {
    async getBattery() { return { level: 0.37, charging: false }; },
    async connection() { return { type: 'cellular', saveData: true }; },
    async powerState() { return { lowPowerMode: true, thermalState: 'fair' }; },
    release() {},
  };
}
`);
  const replaced = makeFixture();
  const manifest = JSON.parse(readFileSync(join(replaced.app, 'package.json'), 'utf8'));
  manifest.dependencies['@fixture/device-web'] = `file:${dir}`;
  writeFileSync(join(replaced.app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], replaced.app);
  const read = await facts(replaced.app);
  check('replaced', 'a direct dependency\'s implementation answers every source in place of the endorsed one', read === '0.37 false / cellular true / true fair', read);
  rmSync(dirname(replaced.app), { recursive: true, force: true });
}

rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
