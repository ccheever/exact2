// The seam is public (LLP 1133.001 D8, LLP 1133.012 stage 3): two tests that
// need nothing an outside author lacks, in WebKit, whose mock microphone
// answers on every machine this has run on.
//
// - Out of the tree: this package and @exact/media-capture, packed as a
//   registry carries them (`bun pm pack`), installed into an app `exact new`
//   made outside the repo, build unchanged: the status is the browser's, and
//   a track opens.
// - Replaced: a fixture implementation of `@exact/speech-recognition` outside
//   the tree, a direct dependency of the app, replaces the endorsed web
//   implementation (LLP 1133.001 D4's second rule) and hears a track that
//   `@exact/media-capture`'s endorsed implementation opened, reaching it
//   through the session's track table (`startSpeechRecognition(track=)`).
//
// usage: bun packages/speech-recognition/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { capture, checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-speech-recognition-seams-'));
process.on('exit', () => rmSync(scratch, { recursive: true, force: true }));

/** `app` built and opened in WebKit, the microphone granted; `drive(page, text, until)` answers what it read. */
async function opened(app, drive) {
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const origin = `http://127.0.0.1:${server.port}`, browser = await webkit.launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(() => '');
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got = '';
    while (Date.now() < end && !want(got = await text(page, id))) await page.waitForTimeout(100);
    return got;
  };
  try {
    const context = await browser.newContext();
    await context.grantPermissions(['microphone'], { origin });
    const page = await context.newPage();
    await page.goto(origin);
    return await drive(page, until);
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarballs, installed as any registry package is.
for (const dir of [pkg, capture]) run('bun', ['pm', 'pack', '--destination', scratch], dir);
const tarball = (name) => join(scratch, readdirSync(scratch).find((f) => f.startsWith(name) && f.endsWith('.tgz')));
const packed = makeFixture({ recognition: `file:${tarball('exact-speech-recognition')}`, mediaCapture: `file:${tarball('exact-media-capture')}` });
const installed = ['speech-recognition', 'media-capture'].map((n) => JSON.parse(readFileSync(join(packed.app, `node_modules/@exact/${n}/package.json`), 'utf8')).name);
check('packed', 'the tarballs are installed, not links into the checkout', installed.join() === '@exact/speech-recognition,@exact/media-capture' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), installed.join());
const [status, track] = await opened(packed.app, async (page, until) => {
  const status = await until(page, 'status', (t) => /^(true|false) /.test(t));
  await page.getByTestId('open').click();
  return [status, await until(page, 'track', (t) => /^track-/.test(t))];
});
// WebKit's recognizer takes the microphone and does not say it keeps audio on the device (the web suite's checks).
check('packed', 'they build unchanged: the status is the browser\'s, and a track opens', /^true \S+ microphone (true|false)$/.test(status) && /^track-\S+$/.test(track), `${status}; ${track}`);
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation of @exact/speech-recognition outside the tree that hears how much audio the track carries.
const dir = join(scratch, 'fixture-speech-recognition-web');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/speech-recognition-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/speech-recognition@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(dir, 'index.js'), `export function implementation({ changed, tracks }) {
  const recognitions = new Map();
  let next = 0;
  const shape = r => ({ id: r.id, state: r.state, audio: r.audio, sound: r.audio, speech: false, results: r.results, finalCount: r.results.length,
    interim: null, leavesDevice: false, error: null, message: null });
  const info = r => ({ id: r.id, leavesDevice: false, finalCount: r.results.length, transcript: null, files: 0, error: null });
  const none = { id: '', state: 'ended', audio: false, sound: false, speech: false, results: [], finalCount: 0, interim: null, leavesDevice: false, error: null, message: null };
  async function stop({ recognition }) {
    const r = recognitions.get(recognition);
    if (!r) return recognition ? { error: 'NotFoundError' } : info({ id: '', results: [] });
    end(r);
    return info(r);
  }
  const end = r => { r.state = 'ended'; r.audio = false; r.audioContext?.close(); r.leave?.(); changed('speech-recognition.speechRecognitionState'); };
  return {
    async startSpeechRecognition({ track }) {
      if (tracks.lookup(track)?.kind !== 'audio') return { error: 'InvalidStateError' };
      const r = { id: 'fixture-' + ++next, state: 'listening', audio: false, results: [], samples: 0 };
      r.leave = tracks.subscribe(track, {
        live(source) {
          const audio = r.audioContext = new AudioContext(), node = audio.createScriptProcessor(4096, 1, 1);
          audio.createMediaStreamSource(new MediaStream([source])).connect(node);
          node.connect(audio.destination);
          node.onaudioprocess = e => {
            r.audio = true;
            r.samples += e.inputBuffer.length;
            if (r.results.length === 0 && r.samples >= audio.sampleRate / 2) {
              r.results.push({ index: 0, alternatives: [{ transcript: 'half a second of ' + track, confidence: 1 }], start: 0, end: 500, apple: null });
              changed('speech-recognition.speechRecognitionState');
            }
          };
        },
        muted() {}, unmuted() {}, ended() { end(r); },
      }, { start: true, origin: 'app' });
      recognitions.set(r.id, r);
      return info(r);
    },
    stopSpeechRecognition: stop,
    abortSpeechRecognition: stop,
    async speechRecognitionState({ recognition }) { const r = recognitions.get(recognition); return r ? shape(r) : none; },
    async speechRecognitionStatus() { return { available: true, state: 'granted', inputs: ['track'], local: true, apple: null, android: null }; },
    async speechRecognitionAvailability() { return { availability: 'available' }; },
    async installSpeechRecognition() { return { installed: true }; },
  };
}
`);
const replaced = makeFixture({ dependencies: { '@fixture/speech-recognition-web': `file:${dir}` } });
const [started, heard, finals] = await opened(replaced.app, async (page, until) => {
  await page.getByTestId('open').click();
  await until(page, 'track', (t) => /^track-/.test(t));
  await page.getByTestId('on-track').click();
  const started = await until(page, 'started', (t) => /^fixture-1 /.test(t));
  const heard = await until(page, 'heard', (t) => / listening true false 1 /.test(t));
  return [started, heard, await until(page, 'finals', (t) => !!t)];
});
check('replaced', 'an implementation outside the tree starts on the endorsed media-capture\'s track', started === 'fixture-1 false -', started);
check('replaced', 'and hears its audio', / listening true false 1 false -$/.test(heard) && /^half a second of track-\S+$/.test(finals), `${heard}; ${finals}`);
rmSync(dirname(replaced.app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
