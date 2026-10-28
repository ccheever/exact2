// @exact/clipboard on the web with no agent (LLP 1133.006 §9 stage 1 items 4
// and 7): the fixture app built for the web and driven in Chrome, the oracle,
// whose clipboard the suite reads back. Each image lands as the pixels it
// should (the red PNG red, the APNG's frame 0 green), text as written, and in
// the A/B race B is what the clipboard holds.
//
// usage: bun packages/clipboard/test/web.mjs
import { join, extname, resolve } from 'node:path';
import { KINDS, checker, discard, makeFixture, root, run } from './fixture.mjs';

const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

// Where the build writes: the app's target, or a CARGO_TARGET_DIR the shell names.
const dist = join(process.env.CARGO_TARGET_DIR ? resolve(app, process.env.CARGO_TARGET_DIR) : join(app, 'target'), 'web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const { check, failed } = checker();
const chrome = process.env.EXACT_CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const browser = await chromium.launch({ executablePath: chrome, headless: true });
try {
  const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  await page.goto(`http://localhost:${server.port}/`);
  const text = id => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  /** What the clipboard holds: its types, its text, and the image's size and top-left pixel. */
  const held = () => page.evaluate(async () => {
    const [item] = await navigator.clipboard.read();
    const out = { types: [...(item?.types ?? [])] };
    if (out.types.includes('text/plain')) out.text = await (await item.getType('text/plain')).text();
    if (out.types.includes('image/png')) {
      const bitmap = await createImageBitmap(await item.getType('image/png'));
      const c = new OffscreenCanvas(1, 1).getContext('2d');
      c.drawImage(bitmap, 0, 0, 1, 1);
      out.image = { width: bitmap.width, height: bitmap.height, pixel: [...c.getImageData(0, 0, 1, 1).data].slice(0, 3).join(',') };
    }
    return out;
  });
  const supports = await text('supports');
  check('chrome', 'ClipboardItem.supports answers from the declaration', supports === 'image/png=true text/plain=true text/html=false image/svg+xml=false', supports);
  const pixel = { png: '255,0,0', 'png-bytes': '255,0,0', both: '255,0,0', apng: '0,255,0' };
  for (const [kind, answer, landed] of KINDS) {
    const before = await text('wrote');
    await page.evaluate(() => navigator.clipboard.writeText('before'));
    await page.getByTestId(kind).click();
    const end = Date.now() + 10000;
    let got = before;
    while (Date.now() < end && (got = await text('wrote')) === before) await page.waitForTimeout(50);
    check('chrome', `${kind}: answers ${answer}`, got === answer, got);
    const h = await held();
    if (!landed) { check('chrome', `${kind}: leaves the clipboard as it was`, h.text === 'before', JSON.stringify(h)); continue; }
    const ok = landed.types.every(t => h.types.includes(t)) && (landed.text === undefined || h.text === landed.text)
      && (!landed.image || (h.image?.width === 8 && h.image?.height === 8 && h.image.pixel === pixel[kind]));
    check('chrome', `${kind}: the clipboard holds ${JSON.stringify(landed)}${pixel[kind] ? ` in ${pixel[kind]}` : ''}`, ok, JSON.stringify(h));
  }
} finally {
  await browser.close();
  server.stop();
}
discard(app);
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
