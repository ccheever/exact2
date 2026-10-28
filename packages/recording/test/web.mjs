// @exact/recording's and @exact/media-capture's conformance suite on the web
// (LLP 1133.001 D7, LLP 1133.009 §11, stage 1): the fixture app
// (`test/fixture`) made with `exact new` outside the repo, depending on both
// packages by `file:`, built for the web, and driven in each engine with the
// web implementations bound. No agent: an implementation's agent path would
// prove nothing about the browser's. Each engine's fake audio device is the
// microphone, and the browser's own means grant it. An engine whose
// `getUserMedia` does not answer at all on this machine is skipped, named.
//
// usage: bun packages/recording/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

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

// Each engine's fake microphone, granted with no prompt. Firefox's pref grants without a prompt and leaves the
// Permissions API's state `prompt`.
const launches = {
  chromium: { args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] },
  firefox: { firefoxUserPrefs: { 'media.navigator.streams.fake': true, 'media.navigator.permission.disabled': true, 'media.autoplay.default': 0 } },
  webkit: {},
};
const grantedState = { chromium: 'granted', firefox: 'prompt', webkit: 'granted' };

// Counts the page's device opens and the browser tracks stopped, for the lease checks (D6).
const count = () => {
  const gum = MediaDevices.prototype.getUserMedia, stop = MediaStreamTrack.prototype.stop;
  globalThis.__media = { opens: 0, stops: 0 };
  MediaDevices.prototype.getUserMedia = function (...a) { globalThis.__media.opens++; return gum.apply(this, a); };
  MediaStreamTrack.prototype.stop = function () { if (this.readyState === 'live') globalThis.__media.stops++; return stop.call(this); };
};

/** The bytes the page's store holds at `path`, read from its IndexedDB as storage-fs.js keeps it: the app's
 * database, whose name a development build suffixes with its directory's digest. */
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

const { check, failed } = checker();

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch(launches[engine]);
  const text = (page, testId) => page.getByTestId(testId).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, testId, want, ms = 10000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, testId))) return got; await page.waitForTimeout(100); }
    return got;
  };
  const tap = (page, testId) => page.getByTestId(testId).click();
  try {
    const context = await browser.newContext();
    if (engine !== 'firefox') await context.grantPermissions(['microphone'], { origin });
    // Does Chromium's capture answer here at all? It does not on a Mac whose CoreAudio hangs it. (Firefox's fake
    // device answers no later page once a page that opened it has closed, so only Chromium is asked first.)
    if (engine === 'chromium') {
      const probe = await context.newPage();
      await probe.goto(origin);
      const answers = await probe.evaluate(() => Promise.race([navigator.mediaDevices.getUserMedia({ audio: true }).then(s => { s.getTracks().forEach(t => t.stop()); return 'ok'; }, e => e.name), new Promise(r => setTimeout(() => r('no answer'), 8000))]));
      await probe.close();
      if (answers !== 'ok') { console.log(`SKIP ${engine}: the browser's own getUserMedia gives ${answers} on this machine`); await context.close(); continue; }
    }

    // Not yet granted: the fact reads `prompt` (or `unknown` where the browser does not name the microphone).
    const asking = await browser.newContext();
    const fresh = await asking.newPage();
    await fresh.goto(origin);
    const prompt = await until(fresh, 'status', t => /^true (prompt|unknown)$/.test(t));
    check(engine, 'state prompt before a grant', /^true (prompt|unknown)$/.test(prompt), prompt);
    await asking.close();

    await context.addInitScript(count);
    const page = await context.newPage();
    await page.goto(origin);
    const granted = await until(page, 'status', t => t === `true ${grantedState[engine]}`);
    check(engine, `state ${grantedState[engine]} once granted`, granted === `true ${grantedState[engine]}`, granted);
    // What the engine's own device gives here, outside the implementation: a level, and a MediaRecorder's bytes.
    // Where it gives none, the checks of a take's content are skipped by name.
    // Firefox's fake device makes a tone and records it, and a page that probes it this way stops answering.
    const own = engine === 'firefox' ? { peak: 1, bytes: Infinity } : await page.evaluate(async () => {
      const s = await navigator.mediaDevices.getUserMedia({ audio: true });
      const ctx = new AudioContext(), a = ctx.createAnalyser(), buf = new Float32Array(2048);
      ctx.createMediaStreamSource(s).connect(a);
      await ctx.resume().catch(() => {});
      const r = new MediaRecorder(s);
      let bytes = 0;
      r.ondataavailable = e => { bytes += e.data.size; };
      await new Promise(done => { r.onstop = done; r.start(); setTimeout(() => r.stop(), 1000); });
      a.getFloatTimeDomainData(buf);
      const peak = Math.max(...buf.map(Math.abs));
      s.getTracks().forEach(t => t.stop());
      await ctx.close();
      return { peak, bytes };
    });
    const sounds = own.peak > 0, writes = own.bytes > 1000;
    if (!sounds) console.log(`SKIP ${engine} the level's content: the engine's own analyser hears silence from its device here`);
    if (!writes) console.log(`SKIP ${engine} the file's content: the engine's own MediaRecorder wrote ${own.bytes} bytes here`);

    await tap(page, 'open-nothing');
    const nothing = await until(page, 'opened-error', t => t !== '');
    check(engine, 'getUserMedia with no kind is TypeError', nothing === 'TypeError', nothing);

    await tap(page, 'open');
    const opened = await until(page, 'opened', t => /^stream-\d+ 1$/.test(t));
    check(engine, 'getUserMedia answers a stream with one track', /^stream-\d+ 1$/.test(opened), opened);
    const idle = await until(page, 'track', t => t === 'live true idle true');
    check(engine, 'a track no consumer joined is idle (D6)', idle === 'live true idle true', idle);
    const devices = await until(page, 'devices', t => Number(t) >= 1);
    check(engine, 'enumerateDevices lists an input once granted', Number(devices) >= 1, devices);

    await tap(page, 'record');
    const started = await until(page, 'started', t => t.startsWith('recording'));
    check(engine, 'startMediaRecorder records to the admitted path, with its extension', /^recording audio\/\S+ app:\/data\/takes\/take-1\.(webm|ogg|m4a)$/.test(started), started);
    const live = await until(page, 'track', t => t === 'live false - true');
    check(engine, 'a recorder leases its track: unmuted', live === 'live false - true', live);
    await tap(page, 'meter');
    await page.waitForTimeout(1500);
    const level = await until(page, 'level', t => Number(t.split(' ')[0]) >= 5 && t.split(' ')[1] === 'true');
    check(engine, 'the level is polled' + (sounds ? ' and hears the device' : ''), Number(level.split(' ')[0]) >= 5 && (!sounds || level.split(' ')[1] === 'true'), level);
    await tap(page, 'meter');

    await tap(page, 'pause');
    const paused = await until(page, 'rec', t => t.startsWith('paused'));
    check(engine, 'pause', /^paused \d+(\.\d+)? requested$/.test(paused), paused);
    await tap(page, 'resume');
    const resumed = await until(page, 'rec', t => t.startsWith('recording'));
    check(engine, 'resume', /^recording \d+(\.\d+)? -$/.test(resumed), resumed);
    await page.waitForTimeout(1200);
    await tap(page, 'stop');
    const saved = await until(page, 'saved', t => t !== 'none');
    check(engine, 'stop answers the file', new RegExp(`^app:/data/takes/take-1\\.\\S+ audio/\\S+ ${writes ? 'true' : '(true|false)'}$`).test(saved), saved);
    const path = saved.split(' ')[0], size = await stored(page, path);
    if (writes) check(engine, 'the file is in the app\'s store', size > 1000, `${path}: ${size} bytes`);
    const played = await page.waitForFunction(() => { const a = document.querySelector('[data-testid="playback"]'); return a && (a.readyState >= 1 || a.error) ? { ready: a.readyState, duration: a.duration, error: a.error?.code ?? null } : null; }, null, { timeout: 10000 }).then(h => h.jsonValue()).catch(e => ({ error: e.message }));
    if (writes) check(engine, 'the file plays (an audio element loads its metadata)', played.ready >= 1 && !played.error, JSON.stringify(played));
    const inactive = await until(page, 'rec', t => t.startsWith('inactive'));
    check(engine, 'a stopped recorder is sticky and inactive', /^inactive \d+(\.\d+)? requested$/.test(inactive), inactive);
    await tap(page, 'pause');
    const invalid = await until(page, 'info-error', t => t !== '');
    check(engine, 'pause after stop is InvalidStateError', invalid === 'InvalidStateError', invalid);

    // Idle after use: the browser's track stops, a reader joining opens nothing, and a start opens it again (D6).
    const idleAgain = await until(page, 'track', t => t === 'live true idle true');
    let media = await page.evaluate(() => globalThis.__media);
    check(engine, 'its last consumer gone, the track is idle and the browser\'s track stopped', idleAgain === 'live true idle true' && media.stops >= 1, `${idleAgain} ${JSON.stringify(media)}`);
    const opensBefore = media.opens;
    await tap(page, 'meter');
    await page.waitForTimeout(600);
    const silent = await text(page, 'level'), still = await text(page, 'track');
    media = await page.evaluate(() => globalThis.__media);
    check(engine, 'a reader joining an idle track hears silence and opens nothing', still === 'live true idle true' && silent.split(' ')[1] === 'false' && media.opens === opensBefore, `${still}; ${silent}; ${JSON.stringify(media)}`);
    await tap(page, 'record');
    const reopened = await until(page, 'track', t => t === 'live false - true');
    media = await page.evaluate(() => globalThis.__media);
    check(engine, 'a start opens an idle track again under its id', reopened === 'live false - true' && media.opens === opensBefore + 1, `${reopened} ${JSON.stringify(media)}`);
    await tap(page, 'meter');
    await page.waitForTimeout(2500);

    // A track that ends stops its recorders (track-ended).
    await tap(page, 'open');
    await until(page, 'opened', t => /^stream-\d+ 1$/.test(t));
    await tap(page, 'record');
    await until(page, 'rec', t => t.startsWith('recording'));
    await tap(page, 'end-track');
    const ended = await until(page, 'rec', t => t.startsWith('inactive'));
    check(engine, 'a track that ends stops its recorder', /^inactive \d+(\.\d+)? track-ended$/.test(ended), ended);
    const endedTrack = await until(page, 'track', t => t.startsWith('ended'));
    check(engine, 'stopTrack ends the track', endedTrack.startsWith('ended'), endedTrack);

    await tap(page, 'bogus');
    const unknown = await until(page, 'trk-error', t => t !== '');
    check(engine, 'an id the session does not hold is NotFoundError', unknown === 'NotFoundError', unknown);
    await tap(page, 'record-outside');
    const outside = await until(page, 'started-error', t => t !== '');
    check(engine, 'a path outside the fs.write grant is refused', outside === 'NotAllowedError', outside);

    // A type the browser claims records; one it does not is NotSupportedError.
    await tap(page, 'open');
    await until(page, 'opened', t => /^stream-\d+ 1$/.test(t));
    const claims = (await text(page, 'types')).split(' ')[1] === 'true';
    await tap(page, 'record-mp4');
    const mp4 = claims ? await until(page, 'started', t => t.includes('audio/mp4')) : await until(page, 'started-error', t => t === 'NotSupportedError');
    check(engine, `audio/mp4 ${claims ? 'records, as isTypeSupported claims' : 'is NotSupportedError, as isTypeSupported says'}`, claims ? /audio\/mp4 \S+\.m4a$/.test(mp4) : mp4 === 'NotSupportedError', mp4);

    // Last, since Firefox's fake device answers no later page once the one that opened it goes: the page goes
    // mid-take (a take is recording since the reopening above), and what the store received is kept (D6).
    const midway = `app:/data/takes/take-2${extname(path)}`;
    await page.reload();
    await page.waitForTimeout(500);
    const kept = await stored(page, midway);
    if (writes) check(engine, 'a page reloaded mid-take keeps what its store received', kept > 0, `${midway}: ${kept} bytes`);

    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
