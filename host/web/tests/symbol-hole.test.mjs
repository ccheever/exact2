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
  'host/web-js/symbols.js': ['const All = true, journal = [], clock = { now: 0 }, appSource = () => {}, Images = new Set(document.querySelectorAll("#exact-root img")), image = e => Images.add(e);', ['inked', 'refresh'], 'refresh();'],
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

// The JS target's `iconForm` reads a source form as exact_kernel::Icon::parse does: each shared case
// (kernel/tests/fixtures/icon-forms.json, which contract/cli/tests/it/icon_forms.rs holds Icon::parse to). And `draw` puts
// it on the element: an even-odd form marked so, a nonzero one as before, one that is no icon drawing
// nothing (no path, viewBox or rule).
test('symbols.js iconForm and draw read a form as Icon::parse does', async () => {
  const source = readFileSync(resolve(ROOT, 'host/web-js/symbols.js'), 'utf8'), suite = await suiteBrowser('chromium', { timeout: 10000 });
  const { cases } = JSON.parse(readFileSync(resolve(ROOT, 'kernel/tests/fixtures/icon-forms.json'), 'utf8'));
  try {
    const page = await suite.newPage();
    await page.setContent(`<script>const Table = {}, image = () => {}; ${glueDeclarations(source, ['iconForm', 'draw'])} globalThis.iconForm = iconForm; globalThis.draw = draw;</script>`);
    const read = await page.evaluate((names) => names.map((name) => {
      const e = document.createElement('img');
      e.setAttribute('data-symbol-viewbox', 'stale'); e.toggleAttribute('data-symbol-nonzero', true); e.toggleAttribute('data-symbol-evenodd', true);
      globalThis.draw(e, name);
      return [globalThis.iconForm(name), e.getAttribute('data-symbol-path'), e.getAttribute('data-symbol-viewbox'), e.hasAttribute('data-symbol-evenodd'), e.hasAttribute('data-symbol-nonzero')];
    }), cases.map(([name]) => name));
    // A file icon is shown as written: nothing of a drawn path stays.
    const want = cases.map(([, icon]) => icon === null
      ? [null, '', null, false, false]
      : [[icon.path, 1, icon.view_box, icon.path ? '' : icon.svg, icon.even_odd ? 1 : 0], icon.path ? icon.path : null, icon.path ? icon.view_box : null, icon.path ? icon.even_odd : false, icon.path ? !icon.even_odd : false]);
    expect(read).toEqual(want);
  } finally { await suite.close(); }
}, 10000);

// A rule that changes: one image driven from a nonzero form to an even-odd one and back, by each
// runtime as its host changes it (the JS target's `draw`; the web host's element.rs attributes, swapped
// as a props batch swaps them), its painter run after each. The ring and its middle are sampled each
// time: the mask is rebuilt with the new `fill-rule`, and the previous rule's attribute is gone.
for (const engine of ['chromium', 'firefox', 'webkit']) for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`${engine}: an icon whose rule changes is drawn by its new rule (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8'), style = source.match(/'(@property --exact-tint[^']*)'/)[1];
  const js = file.endsWith('symbols.js');
  const declarations = glueDeclarations(source, js ? [...names, 'iconForm', 'draw'] : [...names, 'applyProps']);
  // The web host's side: element.rs's attributes for each rule (tests/it/icon_rule.rs holds them), and
  // the props op host.rs sends between two of them: `set` what is new or changed, `clear` what the new
  // map omits. glue.js's own applyProps applies it, its module's other parts stood in for.
  const host = `const syncMedia = () => {}, syncMarkup = () => {}, navigates = () => false, inputReady = true;
    const props = (rule) => ({ 'data-symbol-source': 'symbol:icon||0 0 24 24|' + (rule === 'evenodd' ? 'evenodd:' : '') + '${ring}|icons/flip.svg', 'data-symbol-path': '${ring}', 'data-symbol-fill': '', ['data-symbol-' + rule]: '', 'data-symbol-viewbox': '0 0 24 24', alt: '' });
    const diff = (was, now) => [Object.fromEntries(Object.entries(now).filter(([k, v]) => was[k] !== v)), Object.keys(was).filter((k) => !(k in now))];`;
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<style>${style} #exact-root img { --exact-tint: rgb(255, 0, 0) !important; }</style><div id="exact-root"><div style="padding:16px">${img('flip', 'nonzero')}</div></div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script>const Table = {}; ${stand} ${js ? '' : host} ${declarations} globalThis.paint = () => { ${run} }; globalThis.to = (rule) => { const e = document.getElementById('flip');
      if (${js}) { const form = 'icon||0 0 24 24|' + (rule === 'evenodd' ? 'evenodd:' : '') + '${ring}|icons/flip.svg'; e.setAttribute('data-symbol-source', 'symbol:' + form); draw(e, form); }
      else { const [set, clear] = diff(props(globalThis.rule ?? 'nonzero'), props(rule)); applyProps(e, set, clear); }
      globalThis.rule = rule; };</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 200, height: 100 }, deviceScaleFactor: 1 });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    const red = async (x, y) => {
      const box = await page.locator('#flip').boundingBox();
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
    const seen = [];
    for (const rule of ['nonzero', 'evenodd', 'nonzero']) {
      await page.evaluate((r) => { globalThis.to(r); globalThis.paint(); }, rule);
      await page.waitForFunction(() => [...document.images].every((e) => e.complete));
      await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
      const marks = await page.evaluate(() => { const e = document.getElementById('flip'); return [e.hasAttribute('data-symbol-nonzero'), e.hasAttribute('data-symbol-evenodd'), decodeURIComponent(e.symbolMask).match(/fill-rule="([a-z]+)"/)[1]]; });
      seen.push([rule, ...marks, await red(8, 8), await red(24, 24)]);
    }
    expect(seen).toEqual([
      ['nonzero', true, false, 'nonzero', true, true],
      ['evenodd', false, true, 'evenodd', true, false],
      ['nonzero', true, false, 'nonzero', true, true],
    ]);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 20000);
