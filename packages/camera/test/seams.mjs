// The seam is public (LLP 1133.001 D8, LLP 1133.008 stage 3): three tests
// that need nothing an outside author lacks, in Chromium with its fake camera.
//
// - Out of the tree: both packages, packed as a registry would carry them
//   (`bun pm pack`), installed into an app `exact new` made outside the repo,
//   build unchanged, the viewfinder included, and take a photo.
// - Replaced: a fixture implementation of @exact/camera outside the tree, a
//   direct dependency of the app, replaces the endorsed web implementation,
//   its view included (LLP 1133.001 D4).
// - A second consumer: a fixture package outside the tree reads a video
//   track through the session's track table (LLP 1133.008 D8 item 3), so the
//   track keeps its lease while the viewfinder is hidden, and lets it go
//   when its reader goes.
//
// usage: bun packages/camera/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-camera-seams-'));

/** A built app's page in Chromium with its fake camera, granted: `f(page, until, tap)`'s result. */
async function inPage(app, f) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const origin = `http://127.0.0.1:${server.port}`;
  const browser = await chromium.launch({ args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] });
  try {
    const context = await browser.newContext();
    await context.grantPermissions(['camera'], { origin });
    const page = await context.newPage();
    await page.goto(origin);
    const text = id => page.getByTestId(id).innerText({ timeout: 10000 }).catch(() => '');
    const until = async (id, want, ms = 10000) => {
      const end = Date.now() + ms;
      let got;
      while (Date.now() < end) { if (want(got = await text(id))) return got; await page.waitForTimeout(100); }
      return got;
    };
    return await f(page, until, id => page.getByTestId(id).click());
  } finally {
    await browser.close();
    server.stop();
  }
}

/** A package outside the tree: its manifest and files. */
function outside(name, files) {
  const dir = join(scratch, name.replace('/', '-'));
  mkdirSync(dir, { recursive: true });
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    writeFileSync(join(dir, file), typeof text === 'string' ? text : JSON.stringify(text, null, 2));
  }
  return `file:${dir}`;
}

// Out of the tree: both tarballs, installed as any registry package is.
for (const p of ['camera', 'media-capture']) run('bun', ['pm', 'pack', '--destination', join(scratch, 'packed')], resolve(root, 'packages', p));
const tarball = p => join(scratch, 'packed', readdirSync(join(scratch, 'packed')).find(f => f.startsWith(`exact-${p}-`)));
const packed = makeFixture({ camera: `file:${tarball('camera')}`, dependencies: { '@exact/media-capture': `file:${tarball('media-capture')}` } });
const installed = JSON.parse(readFileSync(join(packed.app, 'node_modules/@exact/camera/package.json'), 'utf8'));
check('packed', 'the tarballs are installed, not links into the checkout', installed.name === '@exact/camera' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), JSON.stringify(installed));
const packedRun = await inPage(packed.app, async (page, until, tap) => {
  await tap('open');
  const live = await until('live', t => t === 'video live false none true');
  const playing = await page.waitForFunction(() => document.querySelector('[data-testid="viewfinder"] video')?.videoWidth > 0, null, { timeout: 10000 }).then(() => true, () => false);
  await tap('shutter');
  const shot = await until('shot', t => t !== 'none', 15000);
  return { live, playing, shot };
});
check('packed', 'it builds unchanged: the viewfinder plays the camera and a photo is taken', packedRun.live === 'video live false none true' && packedRun.playing && /^app:\/tmp\/camera\/1\./.test(packedRun.shot), JSON.stringify(packedRun));
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation outside the tree, its own view and photo.
const replacement = outside('@fixture/camera-web', {
  'package.json': { name: '@fixture/camera-web', version: '1.0.0', exports: { '.': './exact.json' } },
  'exact.json': { kind: 'implementation', implements: '@exact/camera@^1.0', platform: 'web', targets: ['web'], views: ['exact-camera'], sources: { javascript: ['index.js'] } },
  'index.js': `class Box {
  constructor({ element }) { this.box = element.ownerDocument.createElement('div'); this.box.dataset.fixture = 'camera'; element.append(this.box); }
  setProps(props) { this.box.textContent = props.srcObject ?? ''; }
  destroy() { this.box.remove(); }
}
export const roster = { 'exact-camera': { view: Box } };
export function implementation() {
  return {
    async takePhoto() { return { path: 'app:/tmp/fixture.jpg', type: 'image/jpeg', size: 1, width: 2, height: 3 }; },
    async getPhotoCapabilities() { return { redEyeReduction: 'never', imageWidth: { min: 2, max: 2, step: 1 }, imageHeight: { min: 3, max: 3, step: 1 }, fillLightMode: [] }; },
    async picture() { return { path: '' }; },
  };
}
`,
});
const replaced = makeFixture({ dependencies: { '@fixture/camera-web': replacement } });
const replacedRun = await inPage(replaced.app, async (page, until, tap) => {
  await tap('open');
  await until('live', t => t.startsWith('video live'));
  const view = await page.evaluate(() => { const v = document.querySelector('[data-testid="viewfinder"]'); return { fixture: !!v?.querySelector('[data-fixture="camera"]'), video: !!v?.querySelector('video') }; });
  await tap('shutter');
  return { view, shot: await until('shot', t => t !== 'none') };
});
check('replaced', "a direct dependency's view replaces the endorsed viewfinder", replacedRun.view.fixture && !replacedRun.view.video, JSON.stringify(replacedRun.view));
check('replaced', 'and its implementation answers takePhoto', replacedRun.shot === 'app:/tmp/fixture.jpg image/jpeg true 2x3', replacedRun.shot);
rmSync(dirname(replaced.app), { recursive: true, force: true });

// A second consumer: a package that holds a video track through the track table while it is read.
const frames = outside('@fixture/frames', {
  'package.json': { name: '@fixture/frames', version: '1.0.0', exports: { '.': './exact.json', './web': './web/exact.json' } },
  'exact.json': { kind: 'interface', name: '@fixture/frames', version: '1.0.0', substitute: 'implementation',
    shapes: { Watching: { live: 'bool' } },
    names: [{ source: 'watching', live: true, options: { track: 'handle<track>' }, answers: 'Watching' }] },
  'web/exact.json': { kind: 'implementation', implements: '@fixture/frames@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } },
  'web/index.js': `export function implementation({ tracks, changed }) {
  const held = new Map();
  return {
    watching({ track }) {
      if (!track) return { live: false };
      if (!held.has(track)) {
        const w = { live: false };
        w.leave = tracks.subscribe(track, { live: () => { w.live = true; changed('frames.watching'); }, muted: () => { w.live = false; changed('frames.watching'); }, unmuted: () => { w.live = true; changed('frames.watching'); }, ended: () => {} }, { origin: 'app' });
        held.set(track, w);
      }
      return { live: held.get(track).live };
    },
    releaseArgs(name, args) { held.get(args.track)?.leave(); held.delete(args.track); },
    release() { for (const w of held.values()) w.leave(); held.clear(); },
  };
}
`,
});
const second = makeFixture({ dependencies: { '@fixture/frames': frames } });
const contract = readFileSync(join(second.app, 'app.contract'), 'utf8')
  .replace('use takePhoto,', 'use watching, Watching from "@fixture/frames"\nuse takePhoto,')
  .replace('  state placed = true\n', '  state placed = true\n  state reading = false\n  resource watched = watching(track=(reading ? track : "")) as shape Watching else empty()\n')
  .replace('  action unplace\n', '  action read\n    reading = !reading\n  action unplace\n')
  .replace('      if placed\n', '      text `${watched.live}` testId="watched"\n      button press=read testId="read"\n        text "Read"\n      if placed\n');
writeFileSync(join(second.app, 'app.contract'), contract);
const secondRun = await inPage(second.app, async (page, until, tap) => {
  await tap('open');
  await until('live', t => t === 'video live false none true');
  await tap('read');
  const watched = await until('watched', t => t === 'true');
  await tap('toggle');
  await page.waitForTimeout(500);
  const held = await until('live', t => t === 'video live false none true');
  await tap('read');
  const idle = await until('live', t => t === 'video live true idle true');
  return { watched, held, idle };
});
check('consumer', "a package outside the tree reads the camera's track through the track table", secondRun.watched === 'true', JSON.stringify(secondRun));
check('consumer', 'its reading keeps the lease while the viewfinder is hidden', secondRun.held === 'video live false none true', JSON.stringify(secondRun));
check('consumer', 'and its reader gone, the track is idle', secondRun.idle === 'video live true idle true', JSON.stringify(secondRun));
rmSync(dirname(second.app), { recursive: true, force: true });

rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
