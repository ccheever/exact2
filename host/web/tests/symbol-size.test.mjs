// LLP 1011 §3: a symbol drawn for an SF Symbol (a role, or an icon with an
// `sf` source) is the size UIKit draws that symbol at the same font size, and
// on a button's face at UIKit's large symbol scale without making the button
// larger; a tab's symbol keeps the tab bar's and the sidebar's box; an icon of
// the app's own draws its viewBox at 1em, as the Apple hosts draw its path; and
// a page built ahead of time does not move when the symbols are painted. Each
// runtime's symbol painter (glue.js `refreshSymbols`, symbols.js `refresh`)
// under the shell and the neutral sheet, in each engine, measured from the
// screen.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { glueDeclarations } from './glue-fixture.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
const schema = JSON.parse(readFileSync(resolve(ROOT, 'kernel/tables/schema.json'), 'utf8'));
const roles = Object.fromEntries(schema.symbols.map(([role, , d]) => [role, d]));
const scale = schema.metrics.find(([name]) => name === 'symbol-scale')[2];
// SF's own at the same size and weight, in px: [image height, ink width, ink height]. Measured with AppKit
// (`NSImage(systemSymbolName:)`, macOS 27), which draws an SF Symbol at UIKit's size: lock.fill on an iOS 27 button
// measured 17.7 by 25.3 of ink, and a small button's circle, from its 15-point title, 19 by 19.
const SF = {
  lock: [29.6, 17.8, 25.2], // lock.fill on a button: its title's semibold, UIKit's large scale, 20 pt
  small: [21, 17.8, 17.8], // circle on a small button: its 14 px title, the large scale
  back: [18, 8.7, 15], // chevron.backward, semibold, 17 pt
  book: [21.8, 21.6, 18.2], // book, regular, 20 pt (a wide one)
  circle: [23.6, 20, 20], // circle, regular, 20 pt
};
// An icon's path that fills the bounds of Material's lock (`lock`, filled) in its 960-unit grid.
const lock = 'M160-920H800V-80H160Z';
const icon = (id, sf, d = lock, box = '0 -960 960 960', css = '--exact-symbol-size-authored:1;font-size:20px') => `<img id="${id}" data-symbol-source="symbol:icon|${sf}|${box}|${d}|icons/${id}.svg" data-symbol-path="${d}" data-symbol-fill data-symbol-nonzero data-symbol-viewbox="${box}" style="${css}">`;
const role = (id, name, css = '') => `<img id="${id}" data-symbol-source="symbol:${name}" data-symbol-path="${roles[name]}" style="${css}">`;
// The same path as an icon of the app's own: what main drew for the role, a 1em box.
const own = (id, name, css = '') => icon(id, '', roles[name], '0 0 24 24', css);
const title = (text, css = 'font-size:17px') => `<span data-exact-text style="${css}">${text}</span>`;
const button = (id, inner, css = '', attributes = '') => `<button id="${id}" type="button" data-native data-button-style="bordered" style="display:grid;${css}" ${attributes}>${inner}</button>`;
const stacked = '--exact-button-leading-areas:&quot;image&quot; &quot;space&quot; &quot;title&quot;;--exact-button-columns:auto';
const tab = (id, image) => `<button id="${id}" type="button" role="tab" aria-controls="p-${id}" aria-selected="false" data-native data-button-style="bordered" style="display:grid;flex:1">${image}${title('Home', '')}</button>`;
// Each button twice: with a symbol for an SF Symbol, and with an icon of the app's own in its place.
const twins = (id, make) => make(id, role) + make(`${id}-own`, own);
const content = `<div style="font-size:17px;display:flex;flex-direction:column;align-items:flex-start;gap:16px;padding:16px">
  ${button('lock-button', icon('lock', 'lock.fill') + title('Lock', 'font-size:13px;font-weight:600'))}${button('own-button', icon('own', '') + title('Lock', 'font-size:13px;font-weight:600'))}
  ${twins('titled', (id, image) => button(id, image(`${id}-i`, 'share') + title('Share')))}
  ${twins('alone', (id, image) => button(id, image(`${id}-i`, 'add')))}
  ${twins('stacked', (id, image) => button(id, image(`${id}-i`, 'share') + title('Share'), stacked))}
  ${button('small-button', role('small', 'circle') + title('Defrost', ''), 'font-size:14px')}
  ${role('back', 'back', 'font-size:17px;font-weight:600')}${role('book', 'book', 'font-size:20px')}${role('circle', 'circle', 'font-size:20px')}
  ${role('sized', 'circle', 'width:24px;height:24px')}
  ${button('loose', role('loose-i', 'circle') + title('Settings'))}${button('loose-alone', role('loose-alone-i', 'circle'))}
  ${button('written', role('written-i', 'circle') + title('Settings'), 'width:300px;--exact-button-align:start', 'data-exact-said="width"')}
  ${button('written-rtl', role('written-rtl-i', 'circle') + title('Settings'), 'width:300px;--exact-button-align:start', 'data-exact-said="width" dir="rtl"')}
  ${button('tight', role('tight-i', 'circle'), 'width:20px;height:20px', 'data-exact-said="width height"')}
  <div style="display:flex">${['near-a', 'near-b'].map((id) => button(id, role(`${id}-i`, 'add'), 'width:20px;height:20px', 'data-exact-said="width height"')).join('')}</div>
  <div navigationPresentation="modal" style="visibility:hidden"><header>${button('word', role('word-i', 'add'), 'width:50px;height:30px', 'data-exact-said="width height" aria-label="Discard everything"')}${button('titled-word', role('titled-word-i', 'add') + title('Discard'), 'width:120px;height:30px', 'data-exact-said="width height" aria-label="Discard everything"')}</header></div>
  ${twins('bar', (id, image) => `<div role="tablist" style="display:flex;width:200px">${tab(id, image(`${id}-i`, 'home'))}</div>`)}
  ${twins('side', (id, image) => `<div data-exact-sidebar style="display:flex"><div role="tablist" style="display:flex;flex-direction:column;width:200px">${tab(id, image(`${id}-i`, 'home'))}</div></div>`)}
  <div role="tablist" style="display:flex">${button('segment', role('segment-i', 'home') + title('Home', ''), '', 'role="tab" aria-selected="false"')}</div>
  <div role="radiogroup" style="display:flex">${button('radio', role('radio-i', 'home') + title('Home', ''), '', 'role="radio" aria-checked="false"')}</div>
  <div role="menu">${button('menu-row', role('menu-row-i', 'home') + title('Home', ''))}</div><div data-exact-chooser>${button('chooser-row', role('chooser-row-i', 'home') + title('Home', ''), '', 'data-exact-on="press"')}</div>
</div>`;
// Each runtime's painter, its module's other parts stood in for.
const RUNTIMES = {
  'host/web/glue.js': ['const views = new Map([...document.querySelectorAll("#exact-root img")].entries()), log = () => {}, tintFit = () => {};', ['refreshSymbols'], 'refreshSymbols();'],
  'host/web-js/symbols.js': ['const All = true, journal = [], clock = { now: 0 }, appSource = () => {};', ['inked', 'refresh'], 'refresh();'],
};

for (const engine of ['chromium', 'firefox', 'webkit']) for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`${engine}: a symbol for an SF Symbol is SF's size and a button keeps its size (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8');
  const style = source.match(/'(@property --exact-tint[^']*)'/)[1];
  const html = withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<style>${style} #exact-root img { --exact-tint: rgb(255, 0, 0) !important; }</style><div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>`);
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 400, height: 1600 }, deviceScaleFactor: 2 });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    await page.setContent(html);
    const settled = async () => {
      await page.waitForFunction(() => [...document.images].every((e) => e.complete));
      await page.evaluate(() => Promise.all(document.getAnimations().map((a) => a.finished)).then(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)))));
    };
    // Nothing moves when the painter runs on a page built ahead of time, whose symbols have the source document.rs
    // writes (its own test holds the size): the font size across, times `symbol-scale` for a symbol drawn for an SF
    // Symbol.
    const boxes = () => page.evaluate(() => [...document.querySelectorAll('#exact-root [id]')].map((e) => [e.id, ...Object.values(e.getBoundingClientRect().toJSON()).slice(0, 4)]));
    await page.evaluate((scale) => {
      for (const e of document.querySelectorAll('#exact-root img')) {
        const size = parseFloat(getComputedStyle(e).fontSize) * (e.dataset.symbolPath && !e.dataset.symbolSource.startsWith('symbol:icon||') ? scale : 1);
        e.src = `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='${size}' height='${size}'/%3E`;
      }
    }, scale);
    await settled();
    const built = await boxes();
    await page.evaluate(() => globalThis.paint());
    await settled();
    expect(await boxes()).toEqual(built);
    const round = (n) => Math.round(n * 100) / 100, box = async (id) => Object.values(await page.locator(`#${id}`).boundingBox()).map(round);
    // The box of the red ink on the screen within an element's box and `outset` px around it, in px.
    const ink = async (id, outset = 0) => {
      const { x, y, width, height } = await page.locator(`#${id}`).boundingBox();
      const clip = { x: x - outset, y: y - outset, width: width + 2 * outset, height: height + 2 * outset };
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
        return [(right - left + 1) / 2, (bottom - top + 1) / 2];
      }, (await page.screenshot({ clip })).toString('base64'));
    };
    // An image's box and its ink.
    const measure = async (id) => [...(await box(id)).slice(2), ...(await ink(id))];
    // Within 15% of SF's: the image's box height and the ink's width and height.
    const near = (web, sf) => Math.abs(web / sf - 1) <= 0.15;
    const drawn = [];
    for (const [id, sf] of Object.entries(SF)) {
      const [, ...web] = await measure(id);
      drawn.push([id, ...web, web.every((value, i) => near(value, sf[i]))]);
    }
    expect(drawn).toEqual(drawn.map((row) => [...row.slice(0, -1), true]));
    // A button is as tall as it is with an icon of the app's own in the symbol's place, and as wide where the symbol
    // is alone or over a title at least as wide as it; beside a title it is wider by the symbol's growth, .5em, as UIKit
    // lays out its image's width. A tab's symbol and title sit where the icon's do.
    const offset = async (id, from) => { const [x, y, w, h] = await box(id), [x0, y0] = await box(from); return [round(x - x0), round(y - y0), w, h]; };
    const layout = async (lockButton, suffix) => [
      (await box(lockButton))[3], (await box(`titled${suffix}`))[3],
      ...(await Promise.all(['alone', 'stacked', 'bar', 'side'].map(async (id) => (await box(id + suffix)).slice(2)))),
      ...(await Promise.all(['bar', 'side'].flatMap((id) => [offset(`${id}${suffix}-i`, id + suffix), offset(`${id}${suffix} > span`, id + suffix)]))),
    ];
    expect(await layout('lock-button', '')).toEqual(await layout('own-button', '-own'));
    // A radio is drawn as its tab twin, a segment, and a segment's and a menu or chooser row's symbol keeps its 1em
    // box, as iOS draws those outside a button's face.
    const twin = async (id) => [(await box(id)).slice(2), (await box(`${id}-i`)).slice(2), await offset(`${id} > span`, id)];
    expect(await twin('radio')).toEqual(await twin('segment'));
    expect(await Promise.all(['segment-i', 'menu-row-i', 'chooser-row-i'].map(async (id) => (await box(id)).slice(2)))).toEqual([[17, 17], [17, 17], [17, 17]]);
    const wider = async (id, own) => round((await box(id))[2] - (await box(own))[2]);
    expect([await wider('lock-button', 'own-button'), await wider('titled', 'titled-own')]).toEqual([10, 8.5]);
    // Beside a title the symbol's ink is no nearer the title than the icon's.
    const gap = async (id) => page.evaluate(async (png) => {
      const image = new Image();
      image.src = `data:image/png;base64,${png}`;
      await image.decode();
      const canvas = Object.assign(document.createElement('canvas'), { width: image.width, height: image.height }), context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      const { data } = context.getImageData(0, 0, image.width, image.height);
      const column = (x, test) => { for (let y = 0; y < image.height; y++) { const at = (y * image.width + x) * 4; if (test(data[at], data[at + 1], data[at + 2])) return true; } return false; };
      let right = -1;
      for (let x = 0; x < image.width; x++) if (column(x, (r, g, b) => r - Math.max(g, b) > 127)) right = x;
      let title = right + 1;
      while (title < image.width && !column(title, (r, g, b) => Math.max(r, g, b) < 128)) title++;
      return (title - right - 1) / 2;
    }, (await page.locator(`#${id}`).screenshot()).toString('base64'));
    const [symbolGap, iconGap] = [await gap('titled'), await gap('titled-own')];
    expect([symbolGap, symbolGap >= iconGap]).toEqual([symbolGap, true]);
    const [, , width, height] = await box('alone');
    expect(width).toBe(height);
    expect((await box('sized')).slice(2)).toEqual([24, 24]);
    // Nothing clips the ink past its place, to within a pixel of antialiasing: not a written width with no padding,
    // at its start in either direction, nor a written size smaller than the symbol.
    const loose = await ink('loose', 8), alone = await ink('loose-alone', 8);
    const whole = async (id, [width, height]) => { const [w, h] = await ink(id, 8); return [id, w, h, Math.abs(w - width) <= 1 && Math.abs(h - height) <= 1]; };
    const unclipped = await Promise.all([whole('written-i', loose), whole('written-rtl-i', loose), whole('tight', alone)]);
    expect(unclipped).toEqual(unclipped.map((row) => [...row.slice(0, -1), true]));
    // A symbol past its button takes no hits: a point just inside one of two such buttons side by side is that one's,
    // and one just outside it is not, as UIView's hit test keeps to its bounds.
    const hit = (id, edge, dx) => page.evaluate(([id, edge, dx]) => {
      const r = document.getElementById(id).getBoundingClientRect();
      return document.elementFromPoint(r[edge] + dx, r.top + r.height / 2)?.closest('button')?.id ?? null;
    }, [id, edge, dx]);
    const hits = [await hit('near-a', 'left', -1), await hit('near-a', 'right', -1), await hit('near-b', 'left', 1), await hit('near-b', 'right', 1)];
    expect(hits).toEqual([null, 'near-a', 'near-b', null]);
    // A sheet bar's word, drawn in its symbol's place, is clipped and padded as on main; a titled button there with
    // a label keeps its symbol's room.
    // (Firefox lays out in sixtieths of a px.)
    const edges = (id) => page.evaluate((id) => { const s = getComputedStyle(document.getElementById(id)); return [s.overflow, ...[s.paddingTop, s.paddingBottom].map((p) => Math.round(parseFloat(p) * 10) / 10)]; }, id);
    expect([await edges('word'), await edges('titled-word')]).toEqual([['clip', 0, 0], ['clip', 2.1, 2.1]]);
    // The app's own icon: its viewBox at 20 px, so its ink is 640 by 840 of 960 units of that.
    const [ownWidth, ownHeight, inkWidth, inkHeight] = await measure('own');
    expect([ownWidth, ownHeight, Math.round(inkWidth), Math.round(inkHeight)]).toEqual([20, 20, 13, 18]);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 30000);
