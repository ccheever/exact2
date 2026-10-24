// The seam is public (LLP 1133.001 D8, LLP 1133.013 §13 stage 3): two tests
// that need nothing an outside author lacks.
//
// - Out of the tree: this package, packed as a registry would carry it
//   (`bun pm pack`, npm's tarball), installed into an app `exact new` made
//   outside the repo, builds unchanged and reads gravity in Chromium.
// - Replaced: a fixture implementation outside the tree, a direct dependency
//   of the app, replaces the endorsed web implementation and answers a
//   sampled source: each ask after the first carries, as `after`, the
//   `sequence` of the reader's last answer (D12 item 1).
//
// Not here: a fixture implementation feeding a canvas through the context's
// `surfaces.input`, which no host gives an implementation yet.
//
// usage: bun packages/motion/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-motion-seams-'));

/** A built app's page in Chromium with gravity overridden, and `read(page)`'s result. */
async function open(app, read) {
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
    await context.grantPermissions(['accelerometer', 'gyroscope'], { origin });
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send('Emulation.setSensorOverrideEnabled', { enabled: true, type: 'gravity', metadata: { available: true } });
    await page.goto(origin);
    let feeding = true;
    const feeder = (async () => {
      while (feeding) {
        await cdp.send('Emulation.setSensorOverrideReadings', { type: 'gravity', reading: { xyz: { x: 0, y: 4.9, z: 8.5 } } }).catch(() => {});
        await new Promise(r => setTimeout(r, 1000 / 60));
      }
    })();
    try { return await read(page); } finally { feeding = false; await feeder; }
  } finally {
    await browser.close();
    server.stop();
  }
}
const until = async (page, id, want) => {
  const end = Date.now() + 10000;
  let got = '';
  while (Date.now() < end && !want(got)) { got = await page.getByTestId(id).innerText().catch(() => ''); await page.waitForTimeout(50); }
  return got;
};

// Out of the tree: the tarball, installed as any registry package is.
run('bun', ['pm', 'pack', '--destination', scratch], pkg);
const tarball = join(scratch, readdirSync(scratch).find((f) => f.endsWith('.tgz')));
const packed = makeFixture({ motion: `file:${tarball}` });
const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/motion/package.json'), 'utf8'));
check('packed', 'the tarball is installed, not a link into the checkout', installed.name === '@exact/motion' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), JSON.stringify(installed));
const gravity = await open(packed.app, page => until(page, 'gravity', t => t === '0 4.9 8.5'));
check('packed', 'it builds unchanged and the web implementation answers', gravity === '0 4.9 8.5', gravity);
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation outside the tree that answers each sensor with
// the cursor it was given as x and its own sequence as y.
const dir = join(scratch, 'fixture-motion-web');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/motion-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/motion@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(dir, 'index.js'), `export function implementation() {
  let sequence = 0;
  const reading = args => ({ x: args.after ?? -1, y: ++sequence, z: 0, timestamp: 0, sequence, samples: [], dropped: 0, interrupted: false });
  const status = { available: true, state: 'granted', sensors: { accelerometer: true, linearAcceleration: true, gravity: true, gyroscope: true, magnetometer: false, absoluteOrientation: false, relativeOrientation: false, deviceMotion: true, deviceOrientation: true }, apple: null, android: null };
  return {
    async motionStatus() { return status; },
    async requestMotionPermission() { return status; },
    async gravitySensor(args) { return reading(args); },
    async accelerometer(args) { return reading(args); },
    async gyroscope(args) { return reading(args); },
    release() {},
    releaseArgs() {},
  };
}
`);
const replaced = makeFixture();
const manifest = JSON.parse(readFileSync(join(replaced.app, 'package.json'), 'utf8'));
manifest.dependencies['@fixture/motion-web'] = `file:${dir}`;
writeFileSync(join(replaced.app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], replaced.app);
const cursor = await open(replaced.app, page => until(page, 'gravity', t => { const [x, y] = t.split(' ').map(Number); return y >= 3 && x >= 0; }));
const [after, sequence] = cursor.split(' ').map(Number);
check('replaced', 'a direct dependency\'s implementation answers, each ask passing its reader\'s last sequence as `after`', sequence >= 3 && after >= 0 && after < sequence, cursor);

rmSync(dirname(replaced.app), { recursive: true, force: true });
rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
