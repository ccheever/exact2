// @exact/speech-synthesis's conformance suite on the web (LLP 1133.001 D7,
// LLP 1133.012 §10, stage 1): the fixture app (`test/fixture`) made with
// `exact new` outside the repo, depending on the package by `file:`, built for
// the web, and driven in each engine with the web implementation bound. No
// agent: an implementation's agent path would prove nothing about the
// browser's. An engine with no voices here is skipped, named.
//
// usage: bun packages/speech-synthesis/test/web.mjs [chrome] [chromium] [firefox] [webkit]
//   (every engine by default; `chrome` is the installed Google Chrome, `chromium`
//   Playwright's; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chrome', 'chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
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
const autoplay = ['--autoplay-policy=no-user-gesture-required'];
const launches = {
  chrome: [chromium, { channel: 'chrome', args: autoplay }],
  chromium: [chromium, { args: autoplay }],
  firefox: [firefox, { firefoxUserPrefs: { 'media.autoplay.default': 0 } }],
  webkit: [webkit, {}],
};

const { check, failed } = checker();

for (const engine of engines) {
  const [type, options] = launches[engine];
  let browser;
  try {
    browser = await type.launch(options);
  } catch (e) {
    console.log(`SKIP ${engine}: ${e.message.split('\n')[0]}`);
    continue;
  }
  const text = (page, testId) => page.getByTestId(testId).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, testId, want, ms = 15000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, testId))) return got; await page.waitForTimeout(50); }
    return got;
  };
  const tap = (page, testId) => page.getByTestId(testId).click();
  /** The state's word offsets seen while `testId`'s utterance speaks, until it ends. */
  const boundaries = async (page, testId) => {
    const seen = new Set(), end = Date.now() + 20000;
    while (Date.now() < end) {
      const [, speaking, , , charIndex] = (await text(page, 'state')).split(' ');
      if (speaking === 'true') seen.add(Number(charIndex));
      if ((await text(page, testId)).startsWith('ended')) break;
      await page.waitForTimeout(30);
    }
    return [...seen].sort((a, b) => a - b);
  };
  try {
    const page = await browser.newPage();
    page.on('crash', () => console.log(`${engine}: the page crashed`));
    await page.goto(origin);
    const voices = await until(page, 'voices', t => Number(t.split(' ')[0]) > 0, 5000);
    if (!(Number(voices.split(' ')[0]) > 0)) { console.log(`SKIP ${engine}: the engine lists no voices here (${voices})`); continue; }
    check(engine, 'speechSynthesisVoices lists the engine\'s voices', true, voices);
    const idle = await text(page, 'state');
    check(engine, 'idle: available, nothing speaking', /^true false false false 0 0 - 0 auto$/.test(idle), idle);

    // Two utterances, the second queued behind the first: the first's words reached in order, both ending `end`.
    await tap(page, 'say');
    const ids = await until(page, 'ids', t => /^utterance-\d+ utterance-\d+$/.test(t));
    check(engine, 'speak answers an id at once', /^utterance-\d+ utterance-\d+$/.test(ids), ids);
    const queued = await until(page, 'state', t => /^true true true false/.test(t));
    check(engine, 'one speaks while the other is queued', /^true true true false \d+ \d+ \S+ 1 /.test(queued), queued);
    const seen = await boundaries(page, 'first');
    check(engine, 'word boundaries reach the state in order', seen.length >= 3 && seen.includes(4), JSON.stringify(seen));
    const first = await until(page, 'first', t => t.startsWith('ended'));
    check(engine, 'the first ends `end`', first === 'ended end -', first);
    const last = await until(page, 'last', t => t.startsWith('ended'));
    check(engine, 'then the queued one speaks and ends `end`', last === 'ended end -', last);
    const after = await until(page, 'state', t => /^true false false false 0 0 - 0 auto$/.test(t));
    check(engine, 'the queue empty, the state is idle', /^true false false false 0 0 - 0 auto$/.test(after), after);

    // Pause holds the queue; resume lets it finish.
    await tap(page, 'say');
    await until(page, 'state', t => /^true true /.test(t) && Number(t.split(' ')[4]) > 0);
    await tap(page, 'pause');
    const paused = await until(page, 'state', t => / true \d+ \d+ /.test(t) && t.split(' ')[3] === 'true');
    const at = paused.split(' ')[4];
    await page.waitForTimeout(1500);
    const held = await text(page, 'state'), still = await text(page, 'first');
    check(engine, 'paused, the queue holds', held.split(' ')[3] === 'true' && held.split(' ')[4] === at && still.startsWith('paused'), `${held}; ${still}`);
    await tap(page, 'resume');
    const resumed = await until(page, 'last', t => t.startsWith('ended'));
    check(engine, 'resumed, both end', resumed === 'ended end -' && (await text(page, 'first')) === 'ended end -', `${await text(page, 'first')}; ${resumed}`);

    // Cancel: the speaking one `interrupted`, the queued one `canceled`, and the browser speaks no more.
    await tap(page, 'say');
    await until(page, 'state', t => /^true true true /.test(t));
    await tap(page, 'cancel');
    const interrupted = await until(page, 'first', t => t.startsWith('ended'));
    const canceled = await until(page, 'last', t => t.startsWith('ended'));
    check(engine, 'cancel: the speaking one interrupted, the queued one canceled', interrupted === 'ended error interrupted' && canceled === 'ended error canceled', `${interrupted}; ${canceled}`);
    // Firefox's own `speaking` stays true after any `cancel()`, its own page's too, so it says nothing here.
    if (engine === 'firefox') console.log('SKIP firefox the browser\'s own speaking flag: it stays true after cancel() on a bare page too');
    else {
      const quiet = await page.waitForFunction(() => !speechSynthesis.speaking && !speechSynthesis.pending, null, { timeout: 3000 }).then(() => true, () => false);
      check(engine, 'and the browser is silent within 3 s', quiet, `speaking ${await page.evaluate(() => speechSynthesis.speaking)}`);
    }

    // A voice the engine does not have.
    await tap(page, 'say-bad-voice');
    const bad = await until(page, 'first', t => t.includes('voice-unavailable'));
    check(engine, 'an unknown voice ends voice-unavailable', bad === 'ended error voice-unavailable', bad);

    // A playback utterance claims the Audio Session's playback type where the page has one.
    const hasSession = await page.evaluate(() => !!navigator.audioSession);
    await tap(page, 'say-playback');
    const claimed = await until(page, 'state', t => /^true true /.test(t));
    const type = await page.evaluate(() => navigator.audioSession?.type ?? null);
    check(engine, `a playback utterance claims ${hasSession ? 'navigator.audioSession playback' : 'nothing (no Audio Session here)'}`, hasSession ? claimed.endsWith(' playback') && type === 'playback' : claimed.endsWith(' playback'), `${claimed}; ${type}`);
    await tap(page, 'cancel');
    await until(page, 'state', t => / auto$/.test(t));
    const released = await page.evaluate(() => navigator.audioSession?.type ?? null);
    check(engine, 'and gives it back when the queue empties', (await text(page, 'state')).endsWith(' auto') && (!hasSession || released === 'auto'), `${await text(page, 'state')}; ${released}`);
  } catch (e) {
    check(engine, 'ran', false, e.stack ?? e.message);
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
