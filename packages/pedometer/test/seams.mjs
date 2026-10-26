// The seam is public (LLP 1133.001 D8, LLP 1133.026 §15 stage 4): two tests that need nothing an outside author lacks.
//
// - Out of the tree: @exact/pedometer and @exact/altimeter, packed as a registry would carry them (`bun pm pack`),
//   installed into an app `exact new` made outside the repo, build unchanged and answer absence in a browser.
// - Replaced: a fixture implementation of the pedometer outside the tree, a direct dependency of the app, replaces
//   the endorsed web implementation (LLP 1133.001 D4's second rule) and answers a real call, from the view and from
//   data code.
//
// usage: bun packages/pedometer/test/seams.mjs
import { lstatSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-pedometer-seams-'));
const ids = ['steps-status', 'height-status', 'count', 'count-error', 'counted'];

/** A built app's page in Chromium: each of `ids`' text once `steps-status` and `counted` read. */
async function read(app) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const text = async () => Object.fromEntries(await Promise.all(ids.map(async (id) => [id, await page.getByTestId(id).innerText({ timeout: 1000 }).catch(() => '')])));
    const end = Date.now() + 15000;
    let t = await text();
    while (Date.now() < end && (!/^(true|false) /.test(t['steps-status']) || !t.counted)) { await page.waitForTimeout(100); t = await text(); }
    return t;
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarballs, installed as any registry package is.
const tarball = (dir) => {
  const into = mkdtempSync(join(scratch, 'pack-'));
  run('bun', ['pm', 'pack', '--destination', into, '--quiet'], dir);
  return `file:${join(into, readdirSync(into).find((f) => f.endsWith('.tgz')))}`;
};
const packed = makeFixture({ pedometer: tarball(pkg), altimeter: tarball(resolve(root, 'packages/altimeter')) });
const installed = ['pedometer', 'altimeter'].map((p) => join(packed.app, 'node_modules/@exact', p));
check('packed', 'the tarballs are installed, not links into the checkout',
  installed.every((d) => !lstatSync(d).isSymbolicLink() && JSON.parse(readFileSync(join(d, 'package.json'), 'utf8')).version === '1.0.0')
    && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), installed.join(' '));
const absent = await read(packed.app);
check('packed', 'it builds unchanged and both answer absence', absent['steps-status'] === 'false unknown' && absent['height-status'] === 'false unknown'
  && absent['count-error'] === 'NotSupportedError' && absent.counted === 'error NotSupportedError', JSON.stringify(absent));
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation outside the tree, counting 42 steps since a fixed moment.
const dir = join(scratch, 'fixture-pedometer-web');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/pedometer-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/pedometer@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(dir, 'index.js'), `export function implementation() {
  const status = { available: true, state: 'granted', apple: null, android: null };
  return {
    async pedometerStatus() { return status; },
    async requestPedometerPermission() { return status; },
    async pedometer() { return { steps: 42, since: 1790812800000, timestamp: 1790816400000, apple: null, android: null }; },
  };
}
`);
const replaced = makeFixture();
const manifest = JSON.parse(readFileSync(join(replaced.app, 'package.json'), 'utf8'));
manifest.dependencies['@fixture/pedometer-web'] = `file:${dir}`;
writeFileSync(join(replaced.app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], replaced.app);
const counted = await read(replaced.app);
check('replaced', "a direct dependency's implementation answers the status and the count",
  counted['steps-status'] === 'true granted' && counted.count === '42 1790812800000' && counted['count-error'] === '', JSON.stringify(counted));
check('replaced', "data code's reader reads it too", counted.counted === '42', counted.counted);
check('replaced', 'the altimeter keeps its endorsed implementation', counted['height-status'] === 'false unknown', counted['height-status']);

rmSync(dirname(replaced.app), { recursive: true, force: true });
rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
