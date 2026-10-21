// The seam is public (LLP 1133.001 D8, LLP 1133.002 stage 3): two tests that
// need nothing an outside author lacks.
//
// - Out of the tree: this package, packed as a registry would carry it
//   (`bun pm pack`, npm's tarball), installed into an app `exact new` made
//   outside the repo, builds unchanged and answers in a browser.
// - Replaced: a fixture implementation outside the tree, a direct dependency
//   of the app, replaces the endorsed web implementation (D4's second rule)
//   and answers a real call; two such dependencies are refused until
//   `app.json` names one.
//
// usage: bun packages/geolocation/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { implementationFor, moduleInterfaces } = await import(resolve(root, 'js/bake/src/typescript.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-geolocation-seams-'));

/** A built app's page in Chromium, granted and placed, and `once`'s text after `locate`. */
async function locate(app) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const origin = `http://127.0.0.1:${server.port}`, browser = await chromium.launch();
  try {
    const context = await browser.newContext();
    await context.grantPermissions(['geolocation'], { origin });
    await context.setGeolocation({ latitude: 48.8566, longitude: 2.3522, accuracy: 50 });
    const page = await context.newPage();
    await page.goto(origin);
    await page.getByTestId('locate').click();
    const end = Date.now() + 10000;
    let once = '';
    while (Date.now() < end && !/^\d/.test(once)) { once = await page.getByTestId('once').innerText().catch(() => ''); await page.waitForTimeout(100); }
    return once;
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarball, installed as any registry package is.
run('bun', ['pm', 'pack', '--destination', scratch], pkg);
const tarball = join(scratch, readdirSync(scratch).find((f) => f.endsWith('.tgz')));
const packed = makeFixture({ geolocation: `file:${tarball}` });
const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/geolocation/package.json'), 'utf8'));
check('packed', 'the tarball is installed, not a link into the checkout', installed.name === '@exact/geolocation' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), JSON.stringify(installed));
const packedOnce = await locate(packed.app);
check('packed', 'it builds unchanged and the web implementation answers', packedOnce === '48.8566,2.3522,50 none', packedOnce);
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation outside the tree, answering a fixed position.
const implementation = (name, latitude) => {
  const dir = join(scratch, name.replace('/', '-'));
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name, version: '1.0.0', exports: { '.': './exact.json' } }));
  writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/geolocation@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
  writeFileSync(join(dir, 'index.js'), `export function implementation() {
  const at = { coords: { latitude: ${latitude}, longitude: 2.5, accuracy: 7, altitude: null, altitudeAccuracy: null, heading: null, speed: null }, timestamp: 0, apple: null, android: null };
  return {
    async geolocationStatus() { return { available: true, state: 'granted', accuracy: 'precise', apple: null, android: null }; },
    async getCurrentPosition() { return at; },
    async watchPosition() { return at; },
    async requestPreciseLocation() { return { error: 'NotSupportedError' }; },
    release() {},
  };
}
`);
  return `file:${dir}`;
};
const replaced = makeFixture({ dependencies: { '@fixture/geolocation-web': implementation('@fixture/geolocation-web', 1.5) } });
const replacedOnce = await locate(replaced.app);
check('replaced', 'a direct dependency\'s implementation answers the call', replacedOnce === '1.5,2.5,7 none', replacedOnce);

// Two direct dependencies for one target: refused until app.json names one.
const manifest = JSON.parse(readFileSync(join(replaced.app, 'package.json'), 'utf8'));
manifest.dependencies['@fixture/geolocation-other'] = implementation('@fixture/geolocation-other', 3.5);
writeFileSync(join(replaced.app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], replaced.app);
const iface = moduleInterfaces(replaced.app).find((i) => i.name === '@exact/geolocation');
let refusal = '';
try { implementationFor(replaced.app, iface, 'web'); } catch (e) { refusal = e.message; }
check('replaced', 'two implementations for one target are refused by name', refusal.startsWith('bake-implementation-ambiguous'), refusal);
const appJson = JSON.parse(readFileSync(join(replaced.app, 'app.json'), 'utf8'));
appJson.implementations = { '@exact/geolocation': { web: '@fixture/geolocation-other' } };
writeFileSync(join(replaced.app, 'app.json'), JSON.stringify(appJson, null, 2));
check('replaced', 'app.json names the one the app wants', implementationFor(replaced.app, iface, 'web')?.name === '@fixture/geolocation-other', 'another');
const namedOnce = await locate(replaced.app);
check('replaced', 'and that one answers', namedOnce === '3.5,2.5,7 none', namedOnce);

rmSync(dirname(replaced.app), { recursive: true, force: true });
rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
