// LLP 1011 §3: a symbol drawn for an SF Symbol (a role, or an icon with an
// `sf` source) is the size UIKit draws that symbol at the same font size, and
// on a button's face at UIKit's large symbol scale without making the button
// taller beside a title, UIKit's image padding from it (LLP 1069.011.001 D3);
// a tab's symbol keeps the tab bar's and the sidebar's box; an icon of
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
// A column, as the host writes one (button_css.rs `rows`).
const stacked = '--exact-button-leading-areas:&quot;image&quot; &quot;space&quot; &quot;title&quot;;--exact-button-trailing-areas:&quot;title&quot; &quot;space&quot; &quot;image&quot;;--exact-button-columns:minmax(var(--exact-button-title-min,min-content),auto);--exact-button-space:var(--exact-button-row-space);--exact-button-space-width:0px;--exact-button-space-height:var(--exact-button-row-gap,1em);--exact-button-stacked:1;';
const tab = (id, image) => `<button id="${id}" type="button" role="tab" aria-controls="p-${id}" aria-selected="false" data-native data-button-style="bordered" style="display:grid;flex:1">${image}${title('Home', '')}</button>`;
// Each button twice: with a symbol for an SF Symbol, and with an icon of the app's own in its place.
const twins = (id, make) => make(id, role) + make(`${id}-own`, own);
const content = `<div style="font-size:17px;display:flex;flex-direction:column;align-items:flex-start;gap:16px;padding:16px">
  ${button('lock-button', icon('lock', 'lock.fill') + title('Lock', 'font-size:13px;font-weight:600'))}${button('own-button', icon('own', '') + title('Lock', 'font-size:13px;font-weight:600'))}
  ${twins('titled', (id, image) => button(id, image(`${id}-i`, 'share') + title('Share')))}
  ${twins('alone', (id, image) => button(id, image(`${id}-i`, 'add')))}${button('alone-column', role('alone-column-i', 'add'), stacked)}
  ${twins('stacked', (id, image) => button(id, image(`${id}-i`, 'share') + title('Share'), stacked))}
  ${button('small-button', role('small', 'circle') + title('Defrost', ''), 'font-size:14px')}${button('lock-row', role('lock-row-i', 'lock') + title('Lock'))}
  ${button('tile', icon('tile-i', 'lock.fill') + title('Lock', ''), `font-size:15px;${stacked}`)}${button('tile-under', title('Lock', '') + icon('tile-under-i', 'lock.fill'), `font-size:15px;${stacked}`)}
  ${button('tile-tight', icon('tile-tight-i', 'lock.fill') + title('Lock', ''), `font-size:15px;height:auto;${stacked}`, 'data-exact-said="height"')}
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
  'host/web/glue.js': ['const views = new Map([...document.querySelectorAll("#exact-root img")].entries()), log = () => {};', ['tintFit', 'refreshSymbols'], 'refreshSymbols();'],
  'host/web-js/symbols.js': ['const All = true, journal = [], clock = { now: 0 }, appSource = () => {};', ['inked', 'refresh'], 'refresh();'],
};

for (const engine of ['chromium', 'firefox', 'webkit']) for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`${engine}: a symbol for an SF Symbol is SF's size, UIKit's image padding from its title (${file})`, async () => {
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
    // A button is as tall as it is with an icon of the app's own in the symbol's place, but over a title, where the
    // symbol overhangs only toward the title, by .125em, .375em taller; it is as wide where the symbol is alone or over
    // a title at least as wide as it, and beside a title wider by the symbol's growth, .5em, as UIKit lays out its
    // image's width. A tab's symbol and title sit where the icon's do.
    const offset = async (id, from) => { const [x, y, w, h] = await box(id), [x0, y0] = await box(from); return [round(x - x0), round(y - y0), w, h]; };
    const layout = async (lockButton, suffix) => [
      (await box(lockButton))[3], (await box(`titled${suffix}`))[3], (await box(`stacked${suffix}`))[2],
      ...(await Promise.all(['alone', 'bar', 'side'].map(async (id) => (await box(id + suffix)).slice(2)))),
      ...(await Promise.all(['bar', 'side'].flatMap((id) => [offset(`${id}${suffix}-i`, id + suffix), offset(`${id}${suffix} > span`, id + suffix)]))),
    ];
    expect(await layout('lock-button', '')).toEqual(await layout('own-button', '-own'));
    // A radio is drawn as its tab twin, a segment, and a segment's and a menu or chooser row's symbol keeps its 1em
    // box, as iOS draws those outside a button's face.
    const twin = async (id) => [(await box(id)).slice(2), (await box(`${id}-i`)).slice(2), await offset(`${id} > span`, id)];
    expect(await twin('radio')).toEqual(await twin('segment'));
    expect(await Promise.all(['segment-i', 'menu-row-i', 'chooser-row-i'].map(async (id) => (await box(id)).slice(2)))).toEqual([[17, 17], [17, 17], [17, 17]]);
    // Nor does a segment take a button face's image padding: its space is a space.
    expect(await page.evaluate(() => getComputedStyle(document.getElementById('segment'), '::before').width)).not.toBe('8px');
    const wider = async (id, own) => round((await box(id))[2] - (await box(own))[2]);
    const taller = async (id, own) => round((await box(id))[3] - (await box(own))[3]);
    const stackedTaller = await taller('stacked', 'stacked-own');
    expect([await wider('lock-button', 'own-button'), await wider('titled', 'titled-own'), stackedTaller, Math.abs(stackedTaller - .375 * 17) <= .05]).toEqual([10, 8.5, stackedTaller, true]);
    // The px from the symbol's ink to its title's, across (a row) or down (a column).
    const apart = async (id, column) => page.evaluate(async ([png, column]) => {
      const image = new Image();
      image.src = `data:image/png;base64,${png}`;
      await image.decode();
      const canvas = Object.assign(document.createElement('canvas'), { width: image.width, height: image.height }), context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      const { data } = context.getImageData(0, 0, image.width, image.height), [n, m] = column ? [image.height, image.width] : [image.width, image.height];
      const line = (a, test) => { for (let b = 0; b < m; b++) { const at = (column ? a * image.width + b : b * image.width + a) * 4; if (test(data[at], data[at + 1], data[at + 2])) return true; } return false; };
      let end = -1;
      for (let a = 0; a < n; a++) if (line(a, (r, g, b) => r - Math.max(g, b) > 127)) end = a;
      let title = end + 1;
      while (title < n && !line(title, (r, g, b) => Math.max(r, g, b) < 128)) title++;
      return (title - end - 1) / 2;
    }, [(await page.locator(`#${id}`).screenshot()).toString('base64'), column]);
    // Beside a title the symbol's ink is no nearer the title than the icon's. A symbol and its title are UIKit's image
    // padding apart, 8 px, with the box's margin around the ink and the title's line above its letters: a circle beside
    // a 14 px title, a lock beside a 17 px one, and a 20 px lock over a 15 px one (on iOS 11, 12 and 10 points).
    const gaps = [await apart('titled', false), await apart('titled-own', false), await apart('small-button', false), await apart('lock-row', false), await apart('tile', true)];
    const [symbolGap, iconGap, ...inked] = gaps;
    expect([gaps, symbolGap >= iconGap, inked.map((g, i) => g >= [9.5, 12, 10.5][i] && g <= [12, 14.5, 13][i])]).toEqual([gaps, true, [true, true, true]]);
    // Over or under its title the symbol overhangs toward the title alone.
    const margins = (id) => page.evaluate((id) => { const s = getComputedStyle(document.getElementById(id)); return [s.marginTop, s.marginBottom]; }, id);
    expect([await margins('tile-i'), await margins('tile-under-i')]).toEqual([['0px', '-2.5px'], ['-2.5px', '0px']]);
    // A symbol alone keeps its button square, in a column too.
    const [, , width, height] = await box('alone');
    expect([width, (await box('alone-column')).slice(2)]).toEqual([height, [width, height]]);
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
    // A stacked button's written height keeps no room, as its symbol stays within its place.
    expect([await edges('word'), await edges('titled-word'), await edges('tile-tight')]).toEqual([['clip', 0, 0], ['clip', 2.1, 2.1], ['clip', 0, 0]]);
    // The app's own icon: its viewBox at 20 px, so its ink is 640 by 840 of 960 units of that.
    const [ownWidth, ownHeight, inkWidth, inkHeight] = await measure('own');
    expect([ownWidth, ownHeight, Math.round(inkWidth), Math.round(inkHeight)]).toEqual([20, 20, 13, 18]);
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 30000);

// This guards source-before-geometry ordering, not the number of layouts in a refresh.
for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`chromium: scale-down measures a cached placeholder after assigning its source (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8');
  const suite = await suiteBrowser('chromium', { timeout: 10000 });
  try {
    const page = await suite.newPage();
    await page.setContent(`<div id="exact-root">
      ${own('cached', 'circle', 'font-size:20px;object-fit:scale-down')}
      ${own('padded', 'circle', 'font-size:20px;object-fit:scale-down;width:19px;height:19px;padding:3px')}
      ${button('titled-cached', own('following', 'circle', 'object-fit:scale-down') + title('Circle', 'font-size:26px;font-weight:600'))}
      </div><script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>`);
    // Decoding these exact URLs makes assigning src resolve the intrinsic size synchronously.
    await page.evaluate(async () => {
      globalThis.placeholders = await Promise.all([20, 26].map(async point => {
        const image = new Image();
        image.src = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
        await image.decode();
        return image;
      }));
    });
    const result = await page.evaluate(() => {
      const cached = document.getElementById('cached'), before = cached.clientWidth;
      globalThis.paint();
      return { before, after: cached.clientWidth, symbols: ['cached', 'padded', 'following'].map(id => {
        const el = document.getElementById(id), cs = getComputedStyle(el);
        return [id, cs.fontSize, el.style.getPropertyValue('--exact-symbol-fit')];
      }) };
    });
    expect(result).toEqual({ before: 0, after: 20, symbols: [
      ['cached', '20px', '20px 20px'], ['padded', '20px', 'contain'], ['following', '26px', '26px 26px'],
    ] });
  } finally { await suite.close(); }
}, 10000);

for (const [file, [stand, names, run]] of Object.entries(RUNTIMES)) test(`chromium: scale-down uses percentage padding after symbol sources resize the containing block (${file})`, async () => {
  const source = readFileSync(resolve(ROOT, file), 'utf8'), suite = await suiteBrowser('chromium', { timeout: 10000 });
  try {
    const page = await suite.newPage();
    await page.setContent(`<div id="exact-root">${[10, 30].map(width => `<div style="position:relative;display:inline-block">
      ${own(`grow-${width}`, 'circle', 'display:block;font-size:20px')}
      ${own(`padded-${width}`, 'circle', `position:absolute;left:0;top:0;font-size:20px;width:${width}px;height:${width}px;padding:10%;object-fit:scale-down`)}
      </div>`).join('')}</div><script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>`);
    const result = await page.evaluate(async () => {
      const url = point => `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
      globalThis.placeholders = await Promise.all([20, 100].map(async point => {
        const image = new Image(); image.src = url(point); await image.decode(); return image;
      }));
      const grows = [10, 30].map(width => document.getElementById(`grow-${width}`));
      for (const image of grows) { image.src = url(20); await image.decode(); }
      const sample = () => [10, 30].map(width => {
        const el = document.getElementById(`padded-${width}`), cs = getComputedStyle(el);
        return [parseFloat(cs.paddingLeft), el.clientWidth, el.style.getPropertyValue('--exact-symbol-fit')];
      });
      const samples = [];
      for (const size of [20, 100, 20]) {
        for (const image of grows) image.style.fontSize = `${size}px`;
        globalThis.paint(); samples.push(sample());
      }
      return samples;
    });
    const small = [[2, 14, 'contain'], [2, 34, '20px 20px']];
    expect(result).toEqual([small, [[10, 30, 'contain'], [10, 50, '20px 20px']], small]);
  } finally { await suite.close(); }
}, 10000);

test('chromium: loaded raster scale-down follows symbol source and title resizing in either sibling order', async () => {
  const file = 'host/web/glue.js', [stand, names, run] = RUNTIMES[file];
  const source = readFileSync(resolve(ROOT, file), 'utf8'), suite = await suiteBrowser('chromium', { timeout: 10000 });
  const cases = ['source-before', 'source-after', 'title-before', 'title-after'];
  try {
    const page = await suite.newPage();
    await page.setContent(`<div id="exact-root">${cases.map(id => {
      const titled = id.startsWith('title'), symbol = own(`symbol-${id}`, 'circle', `font-size:20px;display:block;flex:none;${titled ? 'width:1em;height:1em' : ''}`);
      const face = titled ? button(`button-${id}`, symbol + `<span id="title-${id}" data-exact-text style="font-size:20px">I</span>`, 'display:flex;flex:none;padding:0;border:0') : symbol;
      const raster = `<img id="raster-${id}" style="flex:1;min-width:0;height:40px;object-fit:scale-down;mask-size:var(--exact-tint-fit,contain)">`;
      return `<div style="display:flex;width:${titled ? 90 : 70}px;align-items:flex-start">${id.endsWith('before') ? face + raster : raster + face}</div>`;
    }).join('')}</div><script>${stand} ${glueDeclarations(source, names)} globalThis.paint = () => { ${run} };</script>`);
    const result = await page.evaluate(async cases => {
      const url = point => `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${point}" height="${point}"/>`)}`;
      globalThis.placeholders = await Promise.all([20, 40].map(async point => {
        const image = new Image(); image.src = url(point); await image.decode(); return image;
      }));
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 40;
      for (const id of cases) {
        const symbol = document.getElementById(`symbol-${id}`), raster = document.getElementById(`raster-${id}`);
        symbol.src = url(20); raster.src = canvas.toDataURL(); await Promise.all([symbol.decode(), raster.decode()]);
      }
      const samples = [];
      for (const size of [20, 40, 20]) {
        for (const id of cases) (document.getElementById(`title-${id}`) || document.getElementById(`symbol-${id}`)).style.fontSize = `${size}px`;
        globalThis.paint();
        samples.push(cases.map(id => {
          const raster = document.getElementById(`raster-${id}`);
          return [raster.complete, raster.naturalWidth, raster.clientWidth >= raster.naturalWidth, raster.style.getPropertyValue('--exact-tint-fit')];
        }));
      }
      return samples;
    }, cases);
    expect(result).toEqual(['auto', 'contain', 'auto'].map(fit => cases.map(() => [true, 40, fit === 'auto', fit])));
  } finally { await suite.close(); }
}, 10000);
