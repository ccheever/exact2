// @exact/speech-recognition's conformance suite on the web (LLP 1133.001 D7,
// LLP 1133.012 §10, stage 1): the fixture app (`test/fixture`) made with
// `exact new` outside the repo, built for the web, and driven in each engine
// with the web implementations bound. No agent. Where an engine has no
// recognizer, or none that can run here, the suite checks that it says so.
//
// A Chrome-family track carries the known spoken file (`fixture.mjs`
// `spokenFile`): the engine's fake capture device plays it where that device
// answers; where it does not (a Mac whose CoreAudio hangs Chrome's capture),
// `getUserMedia` is replaced in the page by a `MediaStreamTrackGenerator`
// track written from the file's samples, and what the recognizer hears on
// that track is reported rather than checked.
//
// usage: bun packages/speech-recognition/test/web.mjs [chrome] [chromium] [firefox] [webkit]
//   (every engine by default; `chrome` is the installed Google Chrome, whose
//   on-device English model the suite installs once into its profile under
//   ~/.cache/exact/speech/chrome-profile; `chromium` is Playwright's, skipped
//   where its `SpeechRecognition.available()` crashes the page)
import { mkdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, PHRASE, root, run, spokenFile } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chrome', 'chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const spoken = spokenFile();

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(path === '/spoken.wav' ? spoken : join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;
const fake = ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', `--use-file-for-fake-audio-capture=${spoken}`];
const profile = join(homedir(), '.cache/exact/speech/chrome-profile');
mkdirSync(profile, { recursive: true });
// What each engine is expected to offer (D8): Chrome on a desktop takes a track; Safari only its own microphone;
// Firefox ships no recognizer it enables. Playwright passes `--disable-component-update`, which keeps Chrome
// from downloading its on-device model.
const launches = {
  chrome: { persistent: profile, type: chromium, options: { channel: 'chrome', args: fake, ignoreDefaultArgs: ['--disable-component-update'] }, inputs: 'track,microphone' },
  chromium: { type: chromium, options: { args: fake }, inputs: 'track,microphone' },
  firefox: { type: firefox, options: { firefoxUserPrefs: { 'media.navigator.streams.fake': true, 'media.navigator.permission.disabled': true } }, inputs: '' },
  webkit: { type: webkit, options: {}, inputs: 'microphone' },
};

/** In the page: `getUserMedia` answering one audio track written in real time, 48 kHz, from the spoken file's
 * 16-bit 16 kHz mono samples, looped. */
function generatedCapture() {
  navigator.mediaDevices.getUserMedia = async () => {
    const view = new DataView(await (await fetch('/spoken.wav')).arrayBuffer());
    let at = 12;
    while (String.fromCharCode(...new Uint8Array(view.buffer, at, 4)) !== 'data') at += 8 + view.getUint32(at + 4, true);
    const n = view.getUint32(at + 4, true) / 2, samples = new Float32Array(n);
    for (let i = 0; i < n; i++) samples[i] = view.getInt16(at + 8 + i * 2, true) / 32768;
    const track = new MediaStreamTrackGenerator({ kind: 'audio' }), writer = track.writable.getWriter();
    const FRAME = 4800;
    let i = 0, timestamp = 0;
    const tick = setInterval(() => {
      if (track.readyState === 'ended') return clearInterval(tick);
      const data = new Float32Array(FRAME);
      for (let k = 0; k < FRAME; k++) data[k] = samples[Math.floor((i + k) / 3) % n];
      i = (i + FRAME) % (n * 3);
      writer.write(new AudioData({ format: 'f32-planar', sampleRate: 48000, numberOfFrames: FRAME, numberOfChannels: 1, timestamp, data }));
      timestamp += 100000;
    }, 100);
    return new MediaStream([track]);
  };
}

const { check, failed } = checker();

for (const engine of engines) {
  const { type, options, persistent, inputs } = launches[engine];
  let browser, context;
  try {
    if (persistent) context = await type.launchPersistentContext(persistent, { ...options, headless: true });
    else { browser = await type.launch(options); context = await browser.newContext(); }
    if (engine !== 'firefox') await context.grantPermissions(['microphone'], { origin }).catch(() => {});
  } catch (e) {
    console.log(`SKIP ${engine}: ${e.message.split('\n')[0]}`);
    continue;
  }
  const text = (page, testId) => page.getByTestId(testId).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, testId, want, ms = 15000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, testId))) return got; await page.waitForTimeout(100); }
    return got;
  };
  const tap = (page, testId) => page.getByTestId(testId).click();
  try {
    // Does the engine's own capture answer here? Firefox's fake device answers no later page once one that opened
    // it has closed, and Firefox takes no track, so only the Chrome family is asked.
    // Playwright's Chromium, built with no on-device service, crashes the page in `SpeechRecognition.available()`.
    let generated = false;
    if (inputs.includes('track')) {
      const asks = async f => {
        const probe = await context.newPage();
        await probe.goto(`${origin}/probe`);
        const answer = await probe.evaluate(f).catch(e => `a crash (${e.message.split('\n')[0]})`);
        await probe.close().catch(() => {});
        return answer;
      };
      const available = await asks(() => SpeechRecognition.available?.({ langs: ['en-US'], processLocally: true }).then(String) ?? 'none');
      if (available.startsWith('a crash')) { console.log(`SKIP ${engine}: SpeechRecognition.available() gives ${available}`); continue; }
      const answers = await asks(() => Promise.race([navigator.mediaDevices.getUserMedia({ audio: true }).then(s => { s.getTracks().forEach(t => t.stop()); return 'ok'; }, e => e.name), new Promise(r => setTimeout(() => r('no answer'), 8000))]));
      if (answers !== 'ok') {
        console.log(`NOTE ${engine}: the engine's own getUserMedia gives ${answers} here; its tracks are generated from the spoken file`);
        await context.addInitScript(generatedCapture);
        generated = true;
      }
    }
    const page = await context.newPage();
    let crashed = false;
    page.on('crash', () => { crashed = true; });
    await page.goto(origin);
    const status = await until(page, 'status', t => /^(true|false) /.test(t));
    const [available, , offered, local] = status.split(' ');
    check(engine, `the status names what this engine takes (${inputs || 'nothing'})`, offered === (inputs || '-') && available === String(inputs !== ''), status);

    await tap(page, 'on-nothing');
    const none = await until(page, 'started-error', t => t !== '');
    check(engine, 'a start with no input is TypeError', none === 'TypeError', none);
    await tap(page, 'on-file');
    const file = await until(page, 'started-error', t => t !== 'TypeError' && t !== '');
    check(engine, 'a file is NotSupportedError: no browser transcribes one', file === 'NotSupportedError', file);
    if (available !== 'true') {
      await tap(page, 'on-microphone');
      const refused = await until(page, 'held-error', t => t !== '');
      check(engine, 'with no recognizer, a start is NotSupportedError', refused === 'NotSupportedError', refused);
      continue;
    }

    // On the device, where the engine can: its model installed once (Chrome downloads it on request).
    let availability = await until(page, 'availability', t => /^(unavailable|downloadable|downloading|available)$/.test(t));
    check(engine, `availability is a word (${availability})`, /^(unavailable|downloadable|downloading|available)$/.test(availability), availability);
    if (availability === 'downloadable' || availability === 'downloading') {
      await tap(page, 'install');
      const installed = await until(page, 'installed', t => t !== 'none', 300000);
      check(engine, 'installSpeechRecognition answers', /^(true|false)$/.test(installed), installed);
      for (let tries = 0; tries < 30 && availability !== 'available'; tries++) {
        await page.reload();
        availability = await until(page, 'availability', t => /^(unavailable|downloadable|downloading|available)$/.test(t));
        if (availability !== 'available') await page.waitForTimeout(5000);
      }
    }
    const onDevice = availability === 'available' && local === 'true';
    if (!onDevice) console.log(`NOTE ${engine}: availability ${availability}, local ${local}; recognition is asked to leave the device`);

    // A track: one capture, the recognition its consumer, on the device where the model is.
    if (offered.includes('track')) {
      await tap(page, 'open');
      const track = await until(page, 'track', t => /^track-\d+$/.test(t));
      check(engine, 'getUserMedia answers a track', /^track-\d+$/.test(track), `${track}; ${await text(page, 'open-error')}`);
      if (!onDevice) await tap(page, 'anywhere');
      await tap(page, 'on-track');
      const started = await until(page, 'started', t => t !== 'none');
      check(engine, `a recognition on the track starts${onDevice ? ' on the device' : ', saying it may leave it'}`, new RegExp(`^recognition-\\d+ ${!onDevice} -$`).test(started), `${started}; ${await text(page, 'started-error')}`);
      const listening = await until(page, 'heard', t => / (listening|ended) true /.test(t) || / ended /.test(t));
      if (onDevice) {
        check(engine, 'it listens, the track\'s audio reaching the recognizer', / listening true /.test(listening), listening);
        // The microphone is held exclusively while a lease on it is active (D12 item 4).
        await tap(page, 'on-microphone');
        const refused = await until(page, 'held-error', t => t !== '');
        check(engine, 'a microphone start while the track is leased is NotReadableError', refused === 'NotReadableError', refused);
        const finals = await until(page, 'finals', t => /milk/i.test(t), 20000);
        const hears = /pick up milk/i.test(finals), heard = `${finals || '(nothing)'} | ${await text(page, 'interim')} | ${await text(page, 'heard')}`;
        if (generated) console.log(`NOTE ${engine}: on the generated track the recognizer heard ${hears ? `"${PHRASE}"` : 'nothing'}: ${heard}`);
        else check(engine, `it hears "${PHRASE}"`, hears, heard);
        await tap(page, 'stop');
        const ended = await until(page, 'ended', t => t !== 'none');
        const [count, files] = ended.split(' ').map(Number);
        check(engine, 'stop answers its results and one transcript file for them', /^\d+ \d+ -$/.test(ended) && files === Math.min(1, count), ended);
      } else {
        const ends = await until(page, 'heard', t => / ended /.test(t), 20000);
        check(engine, 'where it cannot run, it ends with a reason', / ended .* (network|not-allowed|service-not-allowed|audio-capture|no-speech|language-not-supported)$/.test(ends), ends);
        await tap(page, 'anywhere');
      }
      check(engine, 'the page did not crash', !crashed, 'crashed');
      if (crashed) continue;
      if (onDevice) {
        // An ended track ends a recognition on it.
        await tap(page, 'on-track');
        await until(page, 'heard', t => / listening /.test(t));
        await tap(page, 'end-track');
        const cut = await until(page, 'heard', t => / ended /.test(t));
        check(engine, 'an ended track ends its recognition', / ended /.test(cut), cut);
      }
    }

    // The microphone: the browser's own capture under an exclusive hold. Where the recognizer cannot keep audio
    // on the device, `processLocally=true` opens nothing and ends `service-not-allowed` (D9).
    const microphone = async () => {
      const before = await text(page, 'held');
      await tap(page, 'on-microphone');
      return until(page, 'held', t => t !== before);
    };
    if (local !== 'true') {
      const held = await microphone();
      const ends = await until(page, 'heard', t => / ended /.test(t));
      check(engine, 'processLocally where audio may leave the device ends service-not-allowed', /^recognition-\d+ false$/.test(held) && / service-not-allowed$/.test(ends), `${held}; ${ends}`);
      await tap(page, 'anywhere');
    }
    const held = await microphone();
    check(engine, 'a microphone recognition starts', new RegExp(`^recognition-\\d+ ${local !== 'true'}$`).test(held), `${held}; ${await text(page, 'held-error')}`);
    await page.waitForTimeout(500);
    await tap(page, 'abort');
    const aborted = await until(page, 'ended', t => / (aborted|audio-capture|not-allowed|service-not-allowed|network)$/.test(t));
    check(engine, 'abort ends it, aborted unless the browser had already ended it with a reason', / (aborted|audio-capture|not-allowed|service-not-allowed|network)$/.test(aborted), `${aborted}; ${await text(page, 'heard')}`);
  } catch (e) {
    check(engine, 'ran', false, e.stack ?? e.message);
  } finally {
    await context.close().catch(() => {});
    await browser?.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
