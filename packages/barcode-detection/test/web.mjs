// @exact/barcode-detection's and @exact/text-detection's conformance suite on
// the web (LLP 1133.027 §12 stages 1 and 2): the shared fixture app built for
// the web and driven in each engine with the implementations bound, as in
// production.
//
// Chromium on macOS has `BarcodeDetector`, so the stills are detected for
// real in it (drive 5's barcodes); Firefox and WebKit have none, and answer
// an empty `formats` and `NotSupportedError`. No engine has `TextDetector`,
// so text reads `unavailable`, `not-supported` everywhere. Each image comes
// in through the file input, as a person's pick does.
//
// `agent.mjs` drives the fixture under the agent, on the web and Apple.
//
// usage: bun packages/barcode-detection/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { extname, join, resolve } from 'node:path';
import { STILLS, boxOf, checker, discard, images, makeFixture, near, root, run } from './fixture.mjs';

const args = process.argv.slice(2), engines = args.length ? args : ['chromium', 'firefox', 'webkit'];
const { check, failed } = checker();
const { app } = makeFixture();

run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
// 127.0.0.1 is a secure context, which `BarcodeDetector` needs.
const origin = `http://127.0.0.1:${server.port}`;
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const texts = (page, id) => page.getByTestId(id).allInnerTexts().catch(() => []);
  const text = async (page, id) => (await texts(page, id))[0] ?? '';
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(50); }
    return got;
  };
  try {
    const context = await browser.newContext(), page = await context.newPage(), errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin);
    const native = await page.evaluate(() => typeof globalThis.BarcodeDetector);
    const formats = await until(page, 'formats', t => /(true|false)$/.test(t));
    const reader = await until(page, 'reader', t => t.startsWith('unavailable'));
    check(engine, 'text: unavailable, not-supported', reader === 'unavailable not-supported 0', reader);
    if (native === 'undefined') {
      check(engine, 'no BarcodeDetector: formats empty, no tracks', formats.trim() === 'false', formats);
      // A path the grants cover, so the answer is the detector's absence (no pick: WebKit's headless file store
      // cannot store one).
      await page.getByTestId('detect-missing').click();
      const e = await until(page, 'found-error', t => t !== '');
      check(engine, 'a still there fails NotSupportedError', e === 'NotSupportedError', e);
    } else {
      check(engine, 'formats are the browser\'s, upc_a answered beside ean_13', /\bean_13\b/.test(formats) && /\bupc_a\b/.test(formats) && formats.endsWith(' true'), formats);
      let picks = 0, answers = 0;
      for (const [name, image, button, want] of STILLS) {
        await page.getByTestId('image').setInputFiles(images[image]);
        // A pick's path is `app:/tmp/picked/<pick>-<file>.<ext>`.
        await until(page, 'path', t => t.startsWith(`app:/tmp/picked/${++picks}-`));
        await page.getByTestId(button).click();
        // `answers` counts the detections answered, so each read is this still's.
        await until(page, 'answers', t => t === String(answers + 1));
        answers += 1;
        const found = await text(page, 'found'), lines = await texts(page, 'code');
        const codes = lines.map(c => c.split(' ').slice(0, 2).join(' '));
        check(engine, name, JSON.stringify(codes) === JSON.stringify(want), `${found} ${JSON.stringify(lines)} ${await text(page, 'found-error')}`);
        if (image === 'rotated') check(engine, 'the rotated JPEG\'s box is the upright one', found === '468x280 1' && near(boxOf(lines[0] ?? '')), `${found} ${lines}`);
      }
      await page.getByTestId('detect-unknown').click();
      check(engine, 'an unknown format: TypeError', await until(page, 'found-error', t => t === 'TypeError') === 'TypeError', await text(page, 'found-error'));
      await page.getByTestId('detect-outside').click();
      check(engine, 'a path outside the grants: NotAllowedError', await until(page, 'found-error', t => t === 'NotAllowedError') === 'NotAllowedError', await text(page, 'found-error'));
      await page.getByTestId('detect-missing').click();
      check(engine, 'a missing file: NotFoundError', await until(page, 'found-error', t => t === 'NotFoundError') === 'NotFoundError', await text(page, 'found-error'));
    }
    await page.getByTestId('read-missing').click();
    const r = await until(page, 'read-error', t => t !== '');
    check(engine, 'a text detection fails NotSupportedError', r === 'NotSupportedError', r);
    check(engine, 'no page errors', !errors.length, errors.join('; '));
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
discard(app);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
