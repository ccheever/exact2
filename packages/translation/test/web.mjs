// @exact/translation's conformance suite on the web (LLP 1133.027 §12 stages
// 0 and 3): the vision and translation fixture app built for the web and
// driven in each engine with the implementation bound.
//
// No engine here translates headless: Playwright's Chromium has
// `Translator`, whose `availability()` never answers without Chrome's
// on-device model component, and Firefox and WebKit have none. So each engine
// is driven as it is, where an engine with no API reads `unavailable`,
// `not-supported` and a create fails `NotSupportedError`, and then with a
// stand-in `Translator` and `LanguageDetector` (below) written to the
// specification's interface (https://webmachinelearning.github.io/translation-api/),
// which holds the implementation to it (D16): availability by pair, a create
// inside the press with its download's progress, translate, detect and
// destroy. It proves nothing of a browser's own models: that is a person in
// Chrome (`test/checklist.md`).
//
// `agent.mjs` drives the fixture under the agent, on the web and Apple (§9's
// script: drives 7 and 8's module half).
//
// usage: bun packages/translation/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { extname, join, resolve } from 'node:path';
import { checker, discard, makeFixture, root, run } from '../../barcode-detection/test/fixture.mjs';

const args = process.argv.slice(2), engines = args.length ? args : ['chromium', 'firefox', 'webkit'];
const { check, failed } = checker();
const { app } = makeFixture();

run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const texts = (page, id) => page.getByTestId(id).allInnerTexts().catch(() => []);
  const text = async (page, id) => (await texts(page, id))[0] ?? '';
  const until = async (page, id, want, ms = 10000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(50); }
    return got;
  };
  try {
    // As the engine is.
    {
      const context = await browser.newContext(), page = await context.newPage(), errors = [];
      page.on('pageerror', e => errors.push(e.message));
      await page.goto(origin);
      const native = await page.evaluate(() => typeof globalThis.Translator);
      if (native === 'undefined') {
        const a = await until(page, 'pair', t => t.startsWith('unavailable'));
        check(engine, 'with no Translator: unavailable, not-supported', a === 'unavailable not-supported -1', a);
        check(engine, 'and no LanguageDetector', await until(page, 'identifier', t => t.startsWith('unavailable')) === 'unavailable not-supported', await text(page, 'identifier'));
        await page.getByTestId('create').click();
        const e = await until(page, 'translator-error', t => t !== '');
        check(engine, 'a create there fails NotSupportedError', e === 'NotSupportedError', e);
      } else {
        // An engine with the API answers its own state; no create is pressed, since one would start a download.
        const a = await until(page, 'pair', t => /^(unavailable|downloadable|downloading|available) /.test(t), 5000);
        console.log(`NOTE ${engine} has Translator; ja to en reads ${JSON.stringify(a)} (Chromium headless never answers `availability()` without the model component; the page shows the reader's empty value meanwhile)`);
      }
      check(engine, 'no page errors', !errors.length, errors.join('; '));
      await context.close();
    }
    // The stand-in.
    const context = await browser.newContext();
    await context.addInitScript(standIn);
    const page = await context.newPage(), errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(origin);
    const a = await until(page, 'pair', t => t.startsWith('downloadable'));
    check(engine, 'availability is the browser\'s', a === 'downloadable - -1', a);
    await page.getByTestId('pair-same').click();
    const same = await until(page, 'pair', t => t.startsWith('unavailable'));
    const askedSame = await page.evaluate(() => globalThis.__standIn.asked.some(p => p.targetLanguage === 'en-GB'));
    check(engine, 'en to en-GB is one language, never asked of the browser', same === 'unavailable unsupported-options -1' && !askedSame, `${same} ${askedSame}`);
    await page.getByTestId('pair-ja-en').click();
    await until(page, 'pair', t => t.startsWith('downloadable'));
    await page.getByTestId('create').click();
    const made = await until(page, 'translator', t => t.startsWith('translator-'));
    check(engine, 'a create downloads, then answers the translator', made === 'translator-1 ja en -', made);
    const created = await page.evaluate(() => globalThis.__standIn.created);
    check(engine, 'the create was called inside the press, its download watched', created?.event === 'click' && created.monitor === true && created.sourceLanguage === 'ja', JSON.stringify(created));
    const after = await until(page, 'pair', t => t.startsWith('available'));
    check(engine, 'availability reads available after the download', after === 'available - -1', after);
    await page.getByTestId('translate').click();
    const translated = await until(page, 'translated', t => t !== 'none');
    check(engine, 'translate answers the browser\'s text', translated === 'Stand-in: 明日の会議は十時からです。', translated);
    await page.getByTestId('create-detector').click();
    await page.waitForFunction(() => globalThis.__standIn.detectors > 0, null, { timeout: 5000 }).catch(() => {});
    await page.getByTestId('identify').click();
    const langs = await until(page, 'language', t => t !== 'none');
    check(engine, 'detectLanguage answers the browser\'s results', langs === 'ja 0.8', JSON.stringify(await texts(page, 'language')));
    await page.getByTestId('destroy').click();
    await page.waitForFunction(() => globalThis.__standIn.destroyed > 0, null, { timeout: 5000 }).catch(() => {});
    check(engine, 'destroy destroys the browser\'s translator', await page.evaluate(() => globalThis.__standIn.destroyed) === 1, 'not destroyed');
    await page.getByTestId('translate').click();
    const gone = await until(page, 'translated-error', t => t !== '');
    check(engine, 'a translate after it fails InvalidStateError', gone === 'InvalidStateError', gone);
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

/** A stand-in `Translator` and `LanguageDetector` with the specification's interface: `downloadable` until a create downloads. */
function standIn() {
  const state = globalThis.__standIn = { pack: 'downloadable', asked: [], created: null, detectors: 0, destroyed: 0 };
  const download = async o => {
    if (state.pack === 'available') return;
    const m = new EventTarget();
    o.monitor?.(m);
    for (const loaded of [0, 0.5, 1]) { m.dispatchEvent(Object.assign(new Event('downloadprogress'), { loaded })); await new Promise(r => setTimeout(r, 20)); }
    state.pack = 'available';
  };
  globalThis.Translator = class Translator {
    static async availability(o) { state.asked.push(o); return state.pack; }
    static async create(o) {
      state.created = { event: globalThis.event?.type ?? null, monitor: typeof o.monitor === 'function', sourceLanguage: o.sourceLanguage, targetLanguage: o.targetLanguage };
      await download(o);
      return new Translator(o);
    }
    constructor(o) { this.sourceLanguage = o.sourceLanguage; this.targetLanguage = o.targetLanguage; this.inputQuota = Infinity; }
    async translate(input) { return `Stand-in: ${input}`; }
    destroy() { state.destroyed += 1; }
  };
  globalThis.LanguageDetector = class LanguageDetector {
    static async availability() { return 'available'; }
    static async create() { state.detectors += 1; return new LanguageDetector(); }
    async detect() { return [{ detectedLanguage: 'ja', confidence: 0.8 }]; }
    destroy() {}
  };
}
