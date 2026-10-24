// LLP 1121 stage 14 C2: an icon whose SVG fills even-odd draws its holes on
// the web. Its element is marked `data-symbol-evenodd` (element.rs, or the JS
// target's `draw`, which reads the form's `evenodd:` marker), and each
// runtime's painter (glue.js `refreshSymbols`, symbols.js `refresh`) writes
// the mask's `<path>`s with `fill-rule="evenodd"`: a ring of two squares
// wound alike is clear in its middle. The same path as a nonzero icon is
// filled through, as before. Measured from the screen, in each engine.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { glueDeclarations } from './glue-fixture.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
// Two squares wound alike in a 24 box: even-odd leaves the inner one clear.
const ring = 'M2 2H22V22H2Z M8 8H16V16H8Z';
const img = (id, rule) => `<img id="${id}" data-symbol-source="symbol:icon||0 0 24 24|${rule === 'evenodd' ? 'evenodd:' : ''}${ring}|icons/${id}.svg" data-symbol-path="${ring}" data-symbol-fill data-symbol-${rule} data-symbol-viewbox="0 0 24 24" style="--exact-symbol-size-authored:1;font-size:48px;width:48px;height:48px">`;
const content = `<div style="display:flex;gap:16px;padding:16px">${img('odd', 'evenodd')}${img('nonzero', 'nonzero')}</div>`;
// Each runtime's painter, its module's other parts stood in for.
const RUNTIMES = {
  'host/web/glue.js': ['const views = new Map([...document.querySelectorAll("#exact-root img")].entries()), log = () => {};', ['tintFit', 'refreshSymbols'], 'refreshSymbols();'],
  'host/web-js/symbols.js': ['const All = true, journal = [], clock = { now: 0 }, appSource = () => {};', ['inked', 'refresh'], 'refresh();'],
};

for (const engine of ['chromium', 'firefox', 'webkit']) for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`${engine}: an even-odd icon draws its hole, a nonzero one is filled through (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8');
  const style = source.match(/'(@property --exact-tint[^']*)'/)[1];
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<style>${style} #exact-root img { --exact-tint: rgb(255, 0, 0) !important; }</style><div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 200, height: 100 }, deviceScaleFactor: 1 });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    await page.evaluate(() => globalThis.paint());
    await page.waitForFunction(() => [...document.images].every((e) => e.complete));
    await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
    // Whether the screen is red at (x, y) px within an element's box.
    const red = async (id, x, y) => {
      const box = await page.locator(`#${id}`).boundingBox();
      const png = await page.screenshot({ clip: { x: box.x + x, y: box.y + y, width: 1, height: 1 } });
      return page.evaluate(async (data) => {
        const image = new Image();
        image.src = `data:image/png;base64,${data}`;
        await image.decode();
        const canvas = Object.assign(document.createElement('canvas'), { width: 1, height: 1 }), context = canvas.getContext('2d');
        context.drawImage(image, 0, 0);
        const [r, g, b] = context.getImageData(0, 0, 1, 1).data;
        return r > 200 && g < 80 && b < 80;
      }, png.toString('base64'));
    };
    // (8, 8) is on the ring (viewBox 4, 4); (24, 24) its middle (12, 12).
    expect([await red('odd', 8, 8), await red('odd', 24, 24), await red('nonzero', 8, 8), await red('nonzero', 24, 24)]).toEqual([true, false, true, true]);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 20000);

// The JS target's `draw` reads a computed source's form: the `evenodd:`
// marker is the icon's rule, stripped from its path; a nonzero form is as
// before; a rule it doesn't know draws nothing.
test('symbols.js draw reads an icon form\'s fill rule', async () => {
  const source = readFileSync(resolve(ROOT, 'host/web-js/symbols.js'), 'utf8'), suite = await suiteBrowser('chromium', { timeout: 10000 });
  try {
    const page = await suite.newPage();
    await page.setContent(`<script>const Table = {}; ${glueDeclarations(source, ['iconForm', 'draw'])} globalThis.draw = draw;</script>`);
    const drawn = await page.evaluate(() => ['icon||0 0 24 24|evenodd:M0 0h4v4H0Z;M1 1h2v2H1Z|icons/r.svg', 'icon|heart|0 0 24 24|M0 0h4v4H0Z|icons/h.svg', 'icon||0 0 24 24|winding:M0 0h4|icons/w.svg'].map((name) => {
      const e = document.createElement('img');
      globalThis.draw(e, name);
      return [e.getAttribute('data-symbol-path'), e.hasAttribute('data-symbol-evenodd'), e.hasAttribute('data-symbol-nonzero')];
    }));
    expect(drawn).toEqual([['M0 0h4v4H0Z;M1 1h2v2H1Z', true, false], ['M0 0h4v4H0Z', false, true], ['', false, true]]);
  } finally { await suite.close(); }
}, 10000);
