// The seam is public (LLP 1133.001 D8, LLP 1133.009 stage 3): two tests that
// need nothing an outside author lacks, in WebKit, whose mock microphone
// answers on every machine this has run on.
//
// - Out of the tree: both packages, packed as a registry carries them (`bun pm
//   pack`), installed into an app `exact new` made outside the repo, build
//   unchanged and record a take.
// - Replaced: a fixture implementation of `@exact/recording` outside the
//   tree, a direct dependency of the app, replaces the endorsed web
//   implementation (LLP 1133.001 D4's second rule) and records a stream that
//   `@exact/media-capture`'s endorsed implementation opened, reaching its
//   track through the session's track table (`startMediaRecorder(stream=)`).
//
// usage: bun packages/recording/test/seams.mjs
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { capture, checker, makeFixture, pkg, root, run } from './fixture.mjs';

const { webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { check, failed } = checker();
const scratch = mkdtempSync(join(tmpdir(), 'exact-recording-seams-'));

/** A built app's page in WebKit, granted: open, record a second, stop; the `saved` text. */
async function take(app) {
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
    await page.getByTestId('open').click();
    await until(page, 'opened', t => /^stream-\S+ 1$/.test(t));
    await page.getByTestId('record').click();
    await until(page, 'started', t => t.startsWith('recording'));
    await page.waitForTimeout(1000);
    await page.getByTestId('stop').click();
    return await until(page, 'saved', t => t !== 'none' && t !== '');
  } finally {
    await browser.close();
    server.stop();
  }
}

// Out of the tree: the tarballs, installed as any registry package is.
for (const dir of [pkg, capture]) run('bun', ['pm', 'pack', '--destination', scratch], dir);
const tarball = name => join(scratch, readdirSync(scratch).find(f => f.startsWith(name) && f.endsWith('.tgz')));
const packed = makeFixture({ recording: `file:${tarball('exact-recording')}`, mediaCapture: `file:${tarball('exact-media-capture')}` });
const installed = ['recording', 'media-capture'].map(n => JSON.parse(readFileSync(join(packed.app, `node_modules/@exact/${n}/package.json`), 'utf8')).name);
check('packed', 'the tarballs are installed, not links into the checkout', installed.join() === '@exact/recording,@exact/media-capture' && !readFileSync(join(packed.app, 'package.json'), 'utf8').includes(`file:${pkg}`), installed.join());
const packedTake = await take(packed.app);
check('packed', 'they build unchanged and record a take', /^app:\/data\/takes\/take-1\.\S+ audio\/\S+ true$/.test(packedTake), packedTake);
rmSync(dirname(packed.app), { recursive: true, force: true });

// Replaced: an implementation of @exact/recording outside the tree, writing what it hears as raw 16-bit PCM.
const dir = join(scratch, 'fixture-recording-web');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'package.json'), JSON.stringify({ name: '@fixture/recording-web', version: '1.0.0', exports: { '.': './exact.json' } }));
writeFileSync(join(dir, 'exact.json'), JSON.stringify({ kind: 'implementation', implements: '@exact/recording@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } }));
writeFileSync(join(dir, 'index.js'), `export function implementation({ tracks, files, now }) {
  const recorders = new Map();
  let next = 0;
  const info = r => ({ id: r.id, state: r.state, mimeType: 'audio/x-fixture', audioBitsPerSecond: null, path: r.path, duration: now() - r.at, reason: null, error: null, result: null });
  return {
    async startMediaRecorder({ stream, path }) {
      const entry = tracks.lookup(stream);
      if (entry?.kind !== 'stream') return { error: 'NotFoundError' };
      const r = { id: 'fixture-' + ++next, state: 'recording', path: path + '.pcm', at: now(), bytes: 0, chunks: [] };
      r.leave = tracks.subscribe(entry.tracks[0], {
        live(source) {
          const audio = new AudioContext(), node = audio.createScriptProcessor(4096, 1, 1);
          audio.createMediaStreamSource(new MediaStream([source])).connect(node);
          node.connect(audio.destination);
          node.onaudioprocess = e => { const f = e.inputBuffer.getChannelData(0), pcm = new Int16Array(f.length); f.forEach((v, i) => { pcm[i] = v * 32767; }); r.chunks.push(pcm.buffer); };
          r.audio = audio;
        },
        muted() {}, unmuted() {}, ended() {},
      }, { start: true });
      recorders.set(r.id, r);
      return info(r);
    },
    async stopMediaRecorder({ recorder }) {
      const r = recorders.get(recorder);
      if (!r) return { error: 'NotFoundError' };
      r.state = 'inactive';
      await r.audio?.close();
      r.leave();
      const all = new Uint8Array(r.chunks.reduce((n, c) => n + c.byteLength, 0));
      let at = 0;
      for (const c of r.chunks) { all.set(new Uint8Array(c), at); at += c.byteLength; }
      await files.write(r.path, all.buffer);
      return { path: r.path, mimeType: 'audio/x-fixture', duration: now() - r.at, size: all.byteLength };
    },
    async pauseMediaRecorder() { return { error: 'NotSupportedError' }; },
    async resumeMediaRecorder() { return { error: 'NotSupportedError' }; },
    async mediaRecorderState({ recorder }) { const r = recorders.get(recorder); return r ? info(r) : { id: '', state: 'inactive', mimeType: '', path: '', duration: 0 }; },
    async isTypeSupported({ type }) { return { type, supported: type === 'audio/x-fixture' }; },
    async audioLevel() { return { audioLevel: 0, totalAudioEnergy: 0, totalSamplesDuration: 0, levels: [] }; },
  };
}
`);
const replaced = makeFixture({ dependencies: { '@fixture/recording-web': `file:${dir}` } });
const replacedTake = await take(replaced.app);
check('replaced', 'an implementation outside the tree records the endorsed media-capture\'s track', /^app:\/data\/takes\/take-1\.pcm audio\/x-fixture true$/.test(replacedTake), replacedTake);
rmSync(dirname(replaced.app), { recursive: true, force: true });
rmSync(scratch, { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
