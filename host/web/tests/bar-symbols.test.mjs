// LLP 1115.000 D4.15: a symbol alone on a bar item's face is the bar's symbol size, the neutral look's
// `bar-symbol-size` (UIKit's 22 points over its 17-point body, times the web's body), over any size written on it or
// on the item: a symbol drawn for an SF Symbol in its `symbol-scale` box, on a native button's face too, which stays
// as tall as a titled one, and an icon of the app's own at its em. The bar is a header the navigation glue marks where
// iOS makes one (kernel/src/bars.rs `header_bar`); a header iOS makes no bar of (a route with `navigationBar="none"`,
// a header with no heading text, one whose author wrote `appearance: none`) keeps its symbols' sizes, as do a symbol
// beside words (a count's, the title's chevron), one inside a box of its item's and any symbol outside a bar. Once the
// glue has marked the bar, nothing moves when the symbols are painted. Each runtime's symbol painter (glue.js
// `refreshSymbols`, symbols.js `refresh`) runs under the shell, the neutral sheet and the navigation glue, in each
// engine, measured from the screen.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { glueDeclarations } from './glue-fixture.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
const schema = JSON.parse(readFileSync(resolve(ROOT, 'kernel/tables/schema.json'), 'utf8'));
const roles = Object.fromEntries(schema.symbols.map(([role, , d]) => [role, d]));
const metric = (name) => schema.metrics.find(([n]) => n === name)[2];
const [barSize, scale] = [metric('bar-symbol-size'), metric('symbol-scale')];
// An icon's path that fills the bounds of Material's build symbol in its 960-unit grid, with an `sf` source or none.
const wrench = 'M160-880H800V-80H160Z';
// `font` is the size document.rs gives the image's transparent source: the font size the image is written with, or
// inherits, times `symbol-scale` for a symbol drawn for an SF Symbol.
const icon = (id, sf, font, attributes = '') => `<img id="${id}" data-font="${sf ? font * scale : font}" data-symbol-source="symbol:icon|${sf}|0 -960 960 960|${wrench}|icons/wrench.svg" data-symbol-path="${wrench}" data-symbol-fill data-symbol-nonzero data-symbol-viewbox="0 -960 960 960" ${attributes}>`;
const role = (id, name, font, css = '') => `<img id="${id}" data-font="${font * scale}" data-symbol-source="symbol:${name}" data-symbol-path="${roles[name]}" style="${css}">`;
const item = (id, inner, attributes = '') => `<button type="button" id="${id}" data-exact-on="press" ${attributes}>${inner}</button>`;
const native = (id, inner) => `<button type="button" id="${id}" data-native data-button-style="bordered" style="display:grid">${inner}</button>`;
// The JS target writes an author's rows as classes under the root (rows.rs), the wasm runtime inline (glue.js).
const authored = '<style>#exact-root#exact-root { & .sized { font-size: 17px; width: 22px; height: 20px; object-fit: scale-down; --exact-symbol-size-authored: 1; } }</style>';
// A navigation root of one route, whose header holds `heading` (a heading text, as the hosts write one) and `items`.
const route = (id, items, { attributes = '', header = '', heading = '<h2 role="heading" data-exact-text>Login</h2>' } = {}) => `<main navigationKey="1" navigationBack="back" style="display:block;width:390px">
  <div navigationKey="1" id="${id}" ${attributes}><header ${header} style="display:flex;flex-wrap:wrap;align-items:center;gap:4px">${heading}${items}</header></div>
</main>`;
const back = (id) => item(`${id}-back`, role(`${id}-i`, 'back', 17, 'font-size:17px;width:20px;height:20px'));
const content = `${authored}<div style="display:flex;flex-direction:column;align-items:flex-start;gap:16px">
  ${route('bar', `
    ${item('sf', icon('sf-i', 'wrench.and.screwdriver', 17, 'class="sized"'))}${item('sf-free', icon('sf-free-i', 'wrench.and.screwdriver', 16))}
    ${item('own', icon('own-i', '', 17, 'class="sized"'))}${item('own-free', icon('own-free-i', '', 16))}
    ${item('loose', role('loose-i', 'circle', 17, 'font-size:17px;width:20px;height:20px'))}${item('loose-free', role('loose-free-i', 'circle', 16))}
    ${item('fonted', role('fonted-i', 'circle', 13), 'style="font-size:13px"')}
    ${item('bell', role('bell-i', 'add', 15, 'font-size:15px'))}${item('counted', role('counted-i', 'add', 15, 'font-size:15px') + '<span data-exact-text>3</span>')}
    ${native('native', role('native-i', 'share', 13, 'font-size:13px;--exact-symbol-size-authored:1'))}${native('native-free', role('native-free-i', 'share', 16))}${native('worded', '<span data-exact-text>Edit</span>')}
    ${item('nested', `<div>${role('nested-i', 'share', 16, 'width:20px;height:20px')}</div>`)}
    <div id="outer" data-exact-on="press"><span data-exact-text>Outer</span><div id="inner" data-exact-on="press">${role('inner-i', 'add', 15, 'font-size:15px')}</div></div>
    <div role="tablist">${item('tab', role('tab-i', 'add', 16, 'width:20px;height:20px'), 'role="tab"')}</div>`)}
  ${route('titled', '', { heading: item('title', `<h2 role="heading" data-exact-text>Inbox</h2>${role('title-i', 'chooser', 11, 'font-size:11px')}<div id="tapped" data-exact-on="press">${role('tapped-i', 'add', 15, 'font-size:15px')}</div>`) })}
  ${route('full', back('full'), { attributes: 'data-navigationbar="none"' })}
  ${route('headless', back('headless'), { heading: '' })}
  ${route('row', back('row'), { heading: '<div role="heading" aria-level="1"><span data-exact-text>Login</span></div>' })}
  ${route('own-bar', back('own-bar'), { header: 'data-exact-said="appearance"' })}
  ${item('page', role('page-i', 'add', 16))}${role('sized', 'circle', 17, 'font-size:17px;width:20px;height:20px')}
  <header>${item('unrouted', role('unrouted-i', 'add', 16))}</header>
</div>`;
// Each runtime's painter, its module's other parts stood in for.
const RUNTIMES = {
  'host/web/glue.js': ['const views = new Map([...document.querySelectorAll("#exact-root img")].entries()), log = () => {}, tintFit = () => {};', ['refreshSymbols'], 'refreshSymbols();'],
  'host/web-js/symbols.js': ['const All = true, journal = [], clock = { now: 0 }, appSource = () => {}, Images = new Set(document.querySelectorAll("#exact-root img")), image = e => Images.add(e);', ['inked', 'refresh'], 'refresh();'],
};

for (const engine of ['chromium', 'firefox', 'webkit']) for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`${engine}: a bar item's symbol is the bar's size (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8');
  const style = source.match(/'(@property --exact-tint[^']*)'/)[1];
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', () => `<style>${style} #exact-root img { --exact-tint: rgb(255, 0, 0) !important; }</style><div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', () => `<script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>
      <script type="module">${readFileSync(resolve(ROOT, 'host/web/navigation.js'), 'utf8')}
        const shell = document.getElementById('exact-root'); navigation.connect(shell, () => false, () => {}); globalThis.project = () => navigation.project(shell, () => {});</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 400, height: 900 }, deviceScaleFactor: 4 });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    await page.waitForFunction(() => globalThis.project);
    const settled = async () => {
      await page.waitForFunction(() => [...document.images].every((e) => e.complete));
      await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
    };
    // The glue marks the one header iOS makes a bar of. Then nothing moves when the painter runs on the page, whose
    // symbols have the source document.rs writes (which writes the mark too, `document.rs`'s own test).
    await page.evaluate(() => globalThis.project());
    expect(await page.evaluate(() => [...document.querySelectorAll('header[data-exact-bar]')].map((h) => h.parentElement.id))).toEqual(['bar', 'titled']);
    const boxes = () => page.evaluate(() => [...document.querySelectorAll('#exact-root [id]')].map((e) => [e.id, ...Object.values(e.getBoundingClientRect().toJSON()).slice(0, 4)]));
    await page.evaluate(() => {
      for (const e of document.querySelectorAll('#exact-root img')) e.src = `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='${e.dataset.font}' height='${e.dataset.font}'/%3E`;
    });
    await settled();
    const built = await boxes();
    await page.evaluate(() => globalThis.paint());
    await settled();
    expect(await boxes()).toEqual(built);
    const round = (n) => Math.round(n * 100) / 100, size = async (id) => Object.values(await page.locator(`#${id}`).boundingBox()).slice(2).map(round);
    // The box of the red ink on the screen within an element's box, in px.
    const ink = async (id) => {
      const png = await page.locator(`#${id}`).screenshot();
      return page.evaluate(async (png) => {
        const image = new Image();
        image.src = `data:image/png;base64,${png}`;
        await image.decode();
        const canvas = Object.assign(document.createElement('canvas'), { width: image.width, height: image.height }), context = canvas.getContext('2d');
        context.drawImage(image, 0, 0);
        const { data } = context.getImageData(0, 0, image.width, image.height);
        let [left, top, right, bottom] = [Infinity, Infinity, -1, -1];
        for (let y = 0; y < image.height; y++) for (let x = 0; x < image.width; x++) {
          const at = (y * image.width + x) * 4;
          if (data[at] - Math.max(data[at + 1], data[at + 2]) > 127) [left, top, right, bottom] = [Math.min(left, x), Math.min(top, y), Math.max(right, x), Math.max(bottom, y)];
        }
        return [(right - left + 1) / 4, (bottom - top + 1) / 4];
      }, png.toString('base64'));
    };
    // A symbol on a bar item's face is the bar's size, 22/17 of the 16 px body, in a box `symbol-scale` times that for
    // a symbol drawn for an SF Symbol (a native button's face included) and its em for an icon of the app's own, and is
    // drawn at it: whatever size is written on it or on its item, as the same symbol with nothing written (its ink to
    // within a pixel, which Chrome rasterizes a little differently at another place).
    const bar = 16 * parseFloat(barSize), near = (a, b, within) => a.every((v, i) => Math.abs(v - b[i]) <= within);
    const point = (id) => page.evaluate((id) => parseFloat(decodeURIComponent(document.getElementById(id).symbolMask).match(/width="([\d.]+)"/)[1]), id);
    const items = [];
    for (const [id, times, free = `${id}-free-i`] of [['sf', scale], ['own', 1], ['loose', scale], ['fonted', scale, 'loose-free-i'], ['native', scale]]) {
      const [box, drawn, unwritten, at] = [await size(`${id}-i`), await ink(`${id}-i`), await ink(free), await point(`${id}-i`)];
      items.push([id, box, at, drawn, unwritten, near(box, [bar * times, bar * times], 0.06) && Math.abs(at - bar * times) <= 0.06 && near(drawn, unwritten, 1)]);
    }
    expect(items).toEqual(items.map((row) => [...row.slice(0, -1), true]));
    // A native button with a symbol alone is as tall as one with a word beside it.
    expect([(await size('native'))[1], (await size('native-free'))[1]]).toEqual([(await size('worded'))[1], (await size('worded'))[1]]);
    // An item inside the title's pressable is an item; one inside another item is not, as the kernel's walk has them.
    // Beside words (a bell that gains a count, the title's chevron), inside a box of the item's or another item, in a
    // bar's tablist, in a header iOS makes no bar of (a route with no bar, a header with no heading text, one whose
    // heading is a row, one whose author wrote `appearance: none`), outside a route's header or a bar, a symbol keeps
    // its size.
    expect([await size('bell-i'), await size('tapped-i')]).toEqual([await size('loose-i'), await size('loose-i')]);
    expect(await Promise.all(['counted-i', 'title-i', 'inner-i', 'nested-i', 'tab-i', 'full-i', 'headless-i', 'row-i', 'own-bar-i', 'unrouted-i', 'page-i', 'sized'].map(size)))
      .toEqual([[15 * scale, 15 * scale], [11 * scale, 11 * scale], [15 * scale, 15 * scale], [20, 20], [20, 20], [20, 20], [20, 20], [20, 20], [20, 20], [16 * scale, 16 * scale], [16 * scale, 16 * scale], [20, 20]].map((box) => box.map(round)));
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 30000);

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: authored empty text remains part of a custom bar face`, async () => {
  const cases = ['bar-button', 'bar-pressable', 'sheet-button', 'sheet-pressable'];
  const content = cases.map(id => {
    const tag = id.endsWith('button') ? 'button' : 'div';
    const face = role(`${id}-i`, 'add', 13, 'font-size:13px;width:20px;height:20px')
      + '<span data-exact-text style="display:block;width:12px;height:8px;background:red"></span>';
    return `<div navigationkey="${id}" ${id.startsWith('sheet') ? 'navigationPresentation="modal"' : ''}><header data-exact-bar>
      <${tag} id="${id}" data-exact-on="press" aria-label="Add" style="display:flex">${face}</${tag}>
    </header></div>`;
  }).join('');
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', '');
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage();
    await page.setContent(html);
    const got = await page.evaluate(cases => cases.map(id => {
      const item = document.getElementById(id), image = item.querySelector('img'), text = item.querySelector('span');
      const s = getComputedStyle(image), t = getComputedStyle(text), r = text.getBoundingClientRect();
      return { id, image: [s.width, s.height, s.fontSize, s.display], text: [r.width, r.height, t.backgroundColor],
        label: getComputedStyle(item, '::after').content };
    }), cases);
    expect(got).toEqual(cases.map(id => ({ id, image: ['20px', '20px', '13px', 'block'], text: [12, 8, 'rgb(255, 0, 0)'], label: 'none' })));
  } finally { await suite.close(); }
}, 10000);
