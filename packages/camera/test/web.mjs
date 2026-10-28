// @exact/camera's and media-capture's video half's conformance suite on the
// web (LLP 1133.001 D7, LLP 1133.008 §10, stage 1): the fixture app
// (`test/fixture`) made with `exact new` outside the repo, depending on both
// packages by `file:`, built for the web, and driven in each engine with the
// web implementations bound, no agent. Each engine's fake camera is the
// camera, granted by the browser's own means: Chromium's
// `--use-fake-device-for-media-stream`, Firefox's `media.navigator.streams.fake`,
// WebKit's mock capture device. An engine whose `getUserMedia({video})` does
// not answer on this machine is skipped, named. Firefox has no
// `ImageCapture`, so its photos are the viewfinder's frame (D10).
//
// usage: bun packages/camera/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { readFileSync, rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, pkg, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();

// web/card.js carries the agent's picture as base64: it must be the fixture's bytes.
const card = readFileSync(resolve(pkg, 'fixture/card.jpg')).toString('base64');
check('package', "web/card.js is fixture/card.jpg's bytes", readFileSync(resolve(pkg, 'web/card.js'), 'utf8').includes(`'${card}'`), 'run the line in card.js\'s header again');

const { app, id } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;

const launches = {
  chromium: { args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] },
  firefox: { firefoxUserPrefs: { 'media.navigator.streams.fake': true, 'media.navigator.permission.disabled': true, 'media.autoplay.default': 0 } },
  webkit: {},
};
// Firefox's pref grants with no prompt and leaves the Permissions API's state `prompt`.
const grantedState = { chromium: 'granted', firefox: 'prompt', webkit: 'granted' };

// Counts the page's camera opens and the browser tracks stopped, for the lease checks (D3).
const count = () => {
  const gum = MediaDevices.prototype.getUserMedia, stop = MediaStreamTrack.prototype.stop;
  globalThis.__media = { opens: 0, stops: 0 };
  MediaDevices.prototype.getUserMedia = function (...a) { globalThis.__media.opens++; return gum.apply(this, a); };
  MediaStreamTrack.prototype.stop = function () { if (this.readyState === 'live') globalThis.__media.stops++; return stop.call(this); };
};

/** The size of the file the page's store holds at `path` (storage-fs.js's IndexedDB), or -1. */
const stored = (page, path) => page.evaluate(async ([prefix, path]) => {
  const name = (await indexedDB.databases()).map(d => d.name).find(n => n === prefix || n.startsWith(`${prefix}%40`) || n.startsWith(`${prefix}@`));
  if (!name) return -1;
  return new Promise(done => {
    const open = indexedDB.open(name);
    open.onsuccess = () => {
      try {
        const get = open.result.transaction('files').objectStore('files').get(path);
        get.onsuccess = () => { const c = get.result?.contents; done(c ? (c.byteLength ?? c.size) : -1); open.result.close(); };
        get.onerror = () => done(-1);
      } catch { done(-1); }
    };
    open.onerror = () => done(-1);
  });
}, [`exact-storage:${encodeURIComponent(id)}`, path]);

/** The viewfinder's video: its size and whether it plays a stream. */
const viewfinder = page => page.evaluate(() => {
  const v = document.querySelector('[data-testid="viewfinder"] video');
  return v ? { width: v.videoWidth, height: v.videoHeight, stream: !!v.srcObject, transform: v.style.transform } : null;
});

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch(launches[engine]);
  const text = (page, testId) => page.getByTestId(testId).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, testId, want, ms = 10000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, testId))) return got; await page.waitForTimeout(100); }
    return got;
  };
  const waitFor = async (f, want, ms = 10000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await f())) return got; await new Promise(r => setTimeout(r, 100)); }
    return got;
  };
  const tap = (page, testId) => page.getByTestId(testId).click();
  try {
    const context = await browser.newContext();
    if (engine !== 'firefox') await context.grantPermissions(['camera'], { origin }).catch(() => {});
    const probe = await context.newPage();
    await probe.goto(origin);
    const answers = await probe.evaluate(() => Promise.race([navigator.mediaDevices?.getUserMedia({ video: true }).then(s => { s.getTracks().forEach(t => t.stop()); return 'ok'; }, e => e.name) ?? 'no mediaDevices', new Promise(r => setTimeout(() => r('no answer'), 8000))]));
    await probe.close();
    if (answers !== 'ok') { console.log(`SKIP ${engine}: the browser's own getUserMedia({video}) gives ${answers} on this machine`); await context.close(); continue; }

    // The state is live: a grant while the app runs reaches it (`PermissionStatus`'s change).
    if (engine === 'chromium') {
      const asking = await browser.newContext();
      const fresh = await asking.newPage();
      await fresh.goto(origin);
      const prompt = await until(fresh, 'status', t => t === 'true prompt');
      await asking.grantPermissions(['camera'], { origin });
      const now = await until(fresh, 'status', t => t === 'true granted');
      check(engine, 'cameraStatus reads prompt, then granted when granted while running', prompt === 'true prompt' && now === 'true granted', `${prompt} ${now}`);
      await asking.close();
    }

    await context.addInitScript(count);
    const page = await context.newPage();
    await page.goto(origin);
    const granted = await until(page, 'status', t => t === `true ${grantedState[engine]}`);
    check(engine, `cameraStatus reads ${grantedState[engine]} once granted`, granted === `true ${grantedState[engine]}`, granted);

    await tap(page, 'open');
    const live = await until(page, 'live', t => t === 'video live false none true');
    check(engine, 'getUserMedia(video) answers a live video track the displayed viewfinder leases', live === 'video live false none true', live);
    const size = await until(page, 'size', t => /^[1-9]\d*x[1-9]\d*$/.test(t));
    check(engine, "the track's settings are the browser's", /^[1-9]\d*x[1-9]\d*$/.test(size), size);
    const shown = await waitFor(() => viewfinder(page), v => v?.width > 0);
    check(engine, 'the viewfinder plays the track', shown?.width > 0 && shown.stream, JSON.stringify(shown));
    const cameras = await until(page, 'cameras', t => Number(t) >= 1);
    check(engine, 'enumerateDevices lists the camera', Number(cameras) >= 1, cameras);
    const caps = await until(page, 'caps', t => Number(t.split(' ')[0]) > 0);
    check(engine, 'getPhotoCapabilities answers a size', Number(caps.split(' ')[0]) > 0, caps);

    await tap(page, 'shutter');
    const shot = await until(page, 'shot', t => t !== 'none', 15000);
    check(engine, 'takePhoto writes app:/tmp/camera/1 with its format', /^app:\/tmp\/camera\/1\.(jpg|png|webp) image\/\S+ true [1-9]\d*x[1-9]\d*$/.test(shot), `${shot} ${await text(page, 'shot-error')}`);
    const bytes = await stored(page, shot.split(' ')[0]);
    check(engine, "the photo is in the app's store", bytes > 1000, `${bytes} bytes`);
    await tap(page, 'keep');
    const kept = await until(page, 'shot', t => t.startsWith('app:/data/kept.'), 15000);
    check(engine, 'a written path gets its extension', /^app:\/data\/kept\.(jpg|png|webp) /.test(kept), kept);

    // Another camera under the same id (D1): the browser's track replaced, the app's id kept.
    // A camera that already faces the person (WebKit's mock) stays as it is.
    let media = await page.evaluate(() => globalThis.__media);
    const facing = await text(page, 'facing'), moves = facing !== 'user';
    await tap(page, 'front');
    const moved = await waitFor(() => page.evaluate(() => globalThis.__media), m => m.opens > media.opens, moves ? 10000 : 1000);
    const still = await until(page, 'live', t => t === 'video live false none true');
    check(engine, `applyConstraints(facingMode="user") from ${facing} ${moves ? 'opens the camera again' : 'keeps the camera'} under the same id`,
      moved.opens === media.opens + (moves ? 1 : 0) && still === 'video live false none true', `${still} ${JSON.stringify(moved)}`);
    const replayed = await waitFor(() => viewfinder(page), v => v?.width > 0);
    check(engine, 'and the viewfinder plays it', replayed?.width > 0, JSON.stringify(replayed));

    // Hidden, the viewfinder lets the camera go: idle, the browser's track stopped; shown, it opens again (D3).
    media = await page.evaluate(() => globalThis.__media);
    const opens = media.opens;
    await tap(page, 'toggle');
    const idle = await until(page, 'live', t => t === 'video live true idle true');
    media = await page.evaluate(() => globalThis.__media);
    check(engine, 'a hidden viewfinder leaves the track idle and the camera stopped', idle === 'video live true idle true' && media.stops >= 1, `${idle} ${JSON.stringify(media)}`);
    await tap(page, 'shutter');
    const refused = await until(page, 'shot-error', t => t === 'InvalidStateError');
    check(engine, 'takePhoto on an idle track is InvalidStateError', refused === 'InvalidStateError', refused);
    await tap(page, 'toggle');
    const again = await until(page, 'live', t => t === 'video live false none true');
    media = await page.evaluate(() => globalThis.__media);
    check(engine, 'shown again, the viewfinder reopens the camera under the same id', again === 'video live false none true' && media.opens === opens + 1, `${again} ${JSON.stringify(media)}`);
    const reshown = await waitFor(() => viewfinder(page), v => v?.width > 0);
    check(engine, 'and plays it', reshown?.width > 0, JSON.stringify(reshown));

    // Removed from the tree, the same.
    await tap(page, 'unplace');
    const gone = await until(page, 'live', t => t === 'video live true idle true');
    check(engine, 'a removed viewfinder leaves the track idle', gone === 'video live true idle true', gone);
    await tap(page, 'unplace');
    await until(page, 'live', t => t === 'video live false none true');

    await tap(page, 'stop');
    const ended = await until(page, 'live', t => t.startsWith('video ended'));
    check(engine, 'stopTrack ends the track', ended.startsWith('video ended'), ended);
    await tap(page, 'shutter');
    const afterEnd = await until(page, 'shot-error', t => t === 'InvalidStateError');
    check(engine, 'takePhoto on an ended track is InvalidStateError', afterEnd === 'InvalidStateError', afterEnd);

    // Both kinds are one request to the browser (D3).
    if (engine !== 'firefox') await context.grantPermissions(['camera', 'microphone'], { origin }).catch(() => {});
    const before = (await page.evaluate(() => globalThis.__media)).opens;
    await tap(page, 'open-both');
    const kinds = await until(page, 'kinds', t => t === 'audio,video');
    const asked = (await page.evaluate(() => globalThis.__media)).opens - before;
    check(engine, 'getUserMedia(audio, video) opens both with one request to the browser', kinds === 'audio,video' && asked === 1,
      `${kinds} ${await text(page, 'open-error')}, ${asked} requests`);
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
