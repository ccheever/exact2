// LLP 1115.000 D4 part B: the web's neutral look, which is every build's since its switch was thrown
// (host/web/look.mjs; Charlie, 2026-10-12).
//
// A build's shell is the shell's own stylesheet with the neutral sheet (neutral.css) after it, and what the
// author left unsaid is drawn the same in Chrome, Firefox and WebKit: the last test measures one page in
// each engine against one set of numbers. Its colours are the role table's neutral column, which the second
// test holds the sheet to. `EXACT_WEB_LOOK=apple` still builds LLP 1116 D1's look, the shell's own bytes: the
// first test holds the bytes and the third a handful of that look's computed styles. The nodes are as the
// hosts write them.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { DEFAULT_LOOK, LOOK, withLook } from '../look.mjs';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const shell = read('../index.html'), neutral = read('../neutral.css');
const sheet = (look) => withLook(shell, look).match(/<style>([\s\S]*?)<\/style>/)[1];

test('the switch is thrown: a build is the shell with the neutral sheet, and `apple` is still the shell\'s own bytes', () => {
  expect(DEFAULT_LOOK).toBe('neutral');
  // The neutral sheet is the end of the shell's own stylesheet, and says which look the page is.
  expect(withLook(shell, DEFAULT_LOOK)).toBe(shell.replace('</style>', () => neutral + '</style>'));
  expect(/:root \{ --exact-look: neutral;/.test(neutral)).toBe(true);
  // `EXACT_WEB_LOOK=apple` builds LLP 1116 D1's look as it was: the shell's own bytes, which name no look.
  expect(withLook(shell, 'apple')).toBe(shell);
  expect(shell.includes('--exact-look')).toBe(false);
  // A build draws the look its environment names, and the default where it names none.
  expect(LOOK).toBe(process.env.EXACT_WEB_LOOK || DEFAULT_LOOK);
  expect(withLook(shell)).toBe(withLook(shell, LOOK));
});

test('the neutral sheet\'s colours are the role table\'s neutral column', () => {
  const rows = JSON.parse(read('../../../kernel/tables/schema.json')).colors;
  const pair = new Map(rows.map(([name, , , , , , light, dark]) => [name, `light-dark(${light.padEnd(9, 'f')}, ${dark.padEnd(9, 'f')})`]));
  const root = /:root \{ --exact-look: neutral;([^}]*)\}/.exec(neutral)[1];
  const defined = new Map([...root.matchAll(/--exact-([a-z-]+): (light-dark\([^)]*\))/g)].map((m) => [m[1], m[2]]));
  // A role by its own name; a CSS system colour the sheet needs as a value by its name in lower case.
  const role = (name) => pair.get(name) ?? pair.get({ 'gray-text': 'GrayText' }[name]);
  for (const [name, value] of defined) expect([name, value]).toEqual([name, role(name)]);
  // Every role the two sheets name is one the neutral sheet defines: none is left at Apple's pair, which
  // is what the shell's own fallbacks say (element.rs holds those to the table's first column).
  const named = new Set([...(shell + neutral).matchAll(/var\(--exact-([a-z-]+)/g)].map((m) => m[1]).filter((name) => role(name)));
  expect([...named].filter((name) => !defined.has(name))).toEqual([]);
  expect([...defined.keys()].filter((name) => !named.has(name))).toEqual([]);
  // No other colour is written in it: a shadow's black and the transparent ends of a gradient aside.
  const body = neutral.replace(/:root \{[^}]*\}/, '').replace(/\/\*[\s\S]*?\*\//g, '').replace(/url\("[^"]*"\)/g, '');
  expect(body.match(/#[0-9a-f]{3,8}\b/gi)?.filter((hex) => !['#0000', '#fff'].includes(hex)) ?? []).toEqual([]);
});

// The page both browser tests draw: one of each control the looks differ on.
const fixture = `
<div id="exact-root">
  <main navigationKey="1" navigationBack="back" id="nav" style="display:flex;flex-direction:column;height:240px;position:relative">
    <div role="tabpanel" id="p1" style="flex:1;position:relative;color:rgb(255, 0, 0)">
      <div navigationKey="1" id="route"><header id="bar"><h1>Inbox</h1><button type="button" data-native data-button-style="bordered" id="bar-button" style="display:grid"><span data-exact-text>Edit</span></button></header></div>
      <div navigationKey="2" navigationPresentation="modal" id="sheet" style="position:absolute;inset:0;display:flex;flex-direction:column;visibility:hidden"><header><h2>New</h2></header><div style="height:120px"></div></div>
    </div>
    <div role="tablist" id="tabs" style="display:flex"><button type="button" role="tab" aria-controls="p1" aria-selected="true" data-native data-button-style="bordered" id="tab" style="display:grid;flex:1"><span data-exact-text>Inbox</span></button></div>
  </main>
  <div id="said" style="color:rgb(255, 0, 0)">
    <div style="display:flex;justify-content:space-between"><button type="button" data-native data-button-style="bordered" popovertarget="menu" id="invoker" style="display:grid"><span data-exact-text>Menu</span></button><button type="button" data-native data-button-style="bordered" popovertarget="menu" id="invoker-end" style="display:grid"><span data-exact-text>More</span></button></div>
    <div id="held" style="height:40px"></div>
    <button type="button" data-native data-button-style="bordered" id="bordered" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="filled" id="filled" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="gray" id="gray" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="bordered" data-destructive="true" id="destructive" style="display:grid"><span data-exact-text>Delete</span></button>
    <button type="button" data-native data-button-style="filled" disabled id="disabled" style="display:grid"><span data-exact-text>Button</span></button>
    <div style="accent-color:rgb(255, 221, 0);--exact-accent:rgb(255, 221, 0)"><button type="button" data-native data-button-style="filled" id="authored" style="display:grid"><span data-exact-text>Button</span></button></div>
    <input data-native id="field" value="Ada"><input data-native id="field-said" value="Ada" style="color:rgb(0, 128, 0)">
    <select id="select"><option>One</option></select><input type="date" data-exact-field id="date" value="2026-10-11">
    <input type="checkbox" data-exact-control id="check"><input type="checkbox" data-exact-control checked id="checked"><input type="checkbox" id="own" style="appearance:none">
    <input type="radio" data-exact-control checked id="radio"><input type="checkbox" switch data-exact-switch id="switch"><input type="checkbox" switch data-exact-switch checked id="switch-on">
    <div data-exact-progress id="spinner" style="display:grid;width:20px;height:20px"></div><div data-exact-progress data-exact-said="color" id="spinner-said" style="display:grid;width:20px;height:20px"></div>
    <div role="radiogroup" id="segments" style="display:flex"><button type="button" role="radio" aria-checked="true" data-native data-button-style="bordered" id="chosen" style="display:grid"><span data-exact-text>Day</span></button><button type="button" role="radio" aria-checked="false" data-native data-button-style="bordered" id="segment" style="display:grid"><span data-exact-text>Week</span></button></div>
    <div data-exact-on="press" id="pressable" style="width:120px;height:40px"></div>
    <div data-exact-text id="paragraph">A <a data-view href="/x" id="inline">link</a> in it.</div>
    <div data-liststyle="inset-grouped" id="list" style="background-color:light-dark(rgb(242, 242, 247), rgb(0, 0, 0))"><section><div id="card" style="border-radius:26px;background-color:light-dark(rgb(255, 255, 255), rgb(28, 28, 30))"><div>Row</div></div></section></div>
    <div data-liststyle="inset-grouped" data-exact-said="background-color" id="list-said" style="background-color:rgb(1, 2, 3)"></div>
  </div>
  <div popover="auto" role="menu" id="menu" style="display:flex;flex-direction:column"><button type="button" data-native data-button-style="bordered" id="item" style="display:grid"><span data-exact-text>Rename</span></button><button type="button" data-native data-button-style="bordered" data-destructive="true" id="item-delete" style="display:grid"><span data-exact-text>Delete</span></button></div>
  <dialog id="alert" role="alertdialog" aria-label="Sure?"><div data-exact-text>It cannot be undone.</div><button type="button" data-native data-button-style="bordered" id="cancel" style="display:grid"><span data-exact-text>Cancel</span></button></dialog>
</div>`;
// What the page answers with: a colour as the engine paints it (each serializes `color-mix()` its own way), a box.
const helpers = `
  const ink = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  window.rgba = (color) => { ink.clearRect(0, 0, 1, 1); ink.fillStyle = '#000'; ink.fillStyle = color; ink.fillRect(0, 0, 1, 1); const [r, g, b, a] = ink.getImageData(0, 0, 1, 1).data; return a ? [r, g, b, Math.round(a / 2.55) / 100] : [0, 0, 0, 0]; };
  window.css = (id, property, pseudo) => getComputedStyle(document.getElementById(id), pseudo)[property];
  window.color = (id, property = 'color', pseudo) => rgba(css(id, property, pseudo));
  window.box = (id) => { const r = document.getElementById(id).getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 100) / 100); };
  const lum = ([r, g, b]) => [r, g, b].map((v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }).reduce((s, v, i) => s + v * [0.2126, 0.7152, 0.0722][i], 0);
  window.contrast = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return Math.round((x + 0.05) / (y + 0.05) * 100) / 100; };
`;
const launch = async (engine) => {
  const playwright = await import('playwright-core');
  return playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: browserPath().executable } : {}) });
};

test('`EXACT_WEB_LOOK=apple`: a page looks as it did (a handful of computed styles of LLP 1116 D1\'s look)', async () => {
  const browser = await launch('chromium');
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 700 }, colorScheme: 'light' });
    await page.setContent(`<style>${sheet('apple')}</style>${fixture}<script>${helpers}</script>`);
    const got = await page.evaluate(() => { document.getElementById('alert').showModal(); return {
      look: getComputedStyle(document.documentElement).getPropertyValue('--exact-look'),
      bordered: [css('bordered', 'backgroundColor'), css('bordered', 'borderTopStyle'), css('bordered', 'borderRadius'), box('bordered')[3]],
      field: [css('field', 'borderTopColor'), css('field', 'color')],
      check: [css('check', 'appearance'), box('check').slice(2)],
      toggle: [css('switch', 'width', '::before'), css('switch', 'height', '::before')],
      bar: [css('bar', 'backgroundColor'), css('bar', 'backdropFilter'), css('bar', 'borderBottomWidth')],
      tabs: css('tabs', 'backgroundColor'),
      chosen: css('chosen', 'boxShadow') !== 'none',
      alert: [css('alert', 'width'), css('alert', 'textAlign'), css('alert', 'borderRadius')],
      list: [css('list', 'backgroundColor'), css('card', 'borderRadius')],
      menu: [css('menu', 'positionArea'), css('menu', 'boxShadow')],
      sheet: css('sheet', 'position'),
      pressable: css('pressable', 'backgroundImage'),
    }; });
    expect(got).toEqual({
      look: '',
      bordered: ['rgba(118, 118, 128, 0.12)', 'none', '8px', 34], // Apple's tertiary fill, no edge
      field: ['rgb(199, 199, 204)', 'rgb(255, 0, 0)'], // the page's ink, inherited
      check: ['auto', [13, 13]], // the browser's own
      toggle: ['38px', '22px'],
      bar: ['rgba(249, 249, 249, 0.92)', 'saturate(1.8) blur(20px)', '0px'],
      tabs: 'rgba(249, 249, 249, 0.92)',
      chosen: true,
      alert: ['300px', 'center', '14px'],
      list: ['rgb(242, 242, 247)', '26px'],
      menu: ['none', 'none'],
      sheet: 'absolute',
      pressable: 'none',
    });
  } finally { await browser.close(); }
}, 30000);

// One set of numbers for the three engines. A colour is what the engine paints: [r, g, b, alpha].
const INK = { light: [0, 0, 0, 1], dark: [255, 255, 255, 1] }, GROUND = { light: [255, 255, 255, 1], dark: [18, 18, 18, 1] };
const SURFACE = { light: [255, 255, 255, 1], dark: [30, 30, 30, 1] }, LINE = { light: [0, 0, 0, 0.45], dark: [255, 255, 255, 0.45] };
const SECONDARY = { light: [71, 71, 71, 1], dark: [189, 189, 189, 1] }, RED = { light: [201, 15, 26, 1], dark: [254, 115, 103, 1] };
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: the neutral look draws what the author left unsaid, by one set of numbers`, async () => {
  const browser = await launch(engine);
  try {
    for (const scheme of ['light', 'dark']) {
      const context = await browser.newContext({ viewport: { width: 1000, height: 700 }, colorScheme: scheme });
      const page = await context.newPage();
      // The navigation glue's `contextAt`, which both runtimes open a context menu through.
      await page.setContent(`<style>${sheet('neutral')}</style>${fixture}<script>${helpers}</script><script type="module">${read('../navigation.js')}\nwindow.contextAt = contextAt; window.ready = true;</script>`);
      await page.waitForFunction('window.ready === true');
      const at = (name) => [engine, scheme, name];
      const got = await page.evaluate(() => ({
        look: getComputedStyle(document.documentElement).getPropertyValue('--exact-look').trim(),
        page: [color('exact-root'), rgba(getComputedStyle(document.documentElement).backgroundColor)],
        bordered: [color('bordered'), color('bordered', 'backgroundColor'), color('bordered', 'borderTopColor'), css('bordered', 'borderTopWidth'), box('bordered')[3]],
        gray: [color('gray'), css('gray', 'borderTopStyle'), box('gray')[3]],
        destructive: color('destructive'),
        accent: [color('filled', 'backgroundColor'), color('filled'), color('tab')],
        authored: [color('authored', 'backgroundColor'), color('authored')],
        disabled: [color('disabled'), css('disabled', 'backgroundImage')],
        field: [color('field'), color('field', 'backgroundColor'), color('field', 'borderTopColor'), box('field')[3], color('field-said')],
        heights: [box('select')[3], box('date')[3]],
        check: [css('check', 'appearance'), box('check').slice(2), color('check', 'borderTopColor'), color('checked', 'backgroundColor'), color('checked')],
        own: [css('own', 'borderTopWidth'), css('own', 'content', '::before')],
        radio: [box('radio').slice(2), css('radio', 'borderRadius')],
        toggle: [css('switch', 'appearance'), css('switch', 'width', '::before'), css('switch', 'height', '::before'), color('switch', 'backgroundColor', '::before'), color('switch-on', 'backgroundColor', '::before')],
        spinner: [css('spinner', 'backgroundImage', '::before').includes('rgb(255, 0, 0)'), css('spinner-said', 'backgroundImage', '::before').includes('rgb(255, 0, 0)')],
        segments: [box('segments')[2] < 300, Math.abs(box('segments')[2] - box('chosen')[2] - box('segment')[2] - 6) < 1, box('segments')[3], css('chosen', 'boxShadow') !== 'none'],
        bar: [color('bar'), color('bar', 'backgroundColor'), css('bar', 'backdropFilter') ?? css('bar', 'webkitBackdropFilter'), css('bar', 'borderBottomWidth'), color('bar-button', 'borderTopColor')[3]],
        tabs: [color('tabs', 'backgroundColor'), css('tabs', 'borderTopWidth'), css('tabs', 'boxShadow')],
        list: [color('list', 'backgroundColor'), css('card', 'borderTopLeftRadius'), color('list-said', 'backgroundColor')],
        sheet: [color('sheet', 'backgroundColor'), css('sheet', 'position'), box('sheet')[2]],
      }));
      expect(got.look).toBe('neutral');
      expect([at('page'), got.page]).toEqual([at('page'), [INK[scheme], GROUND[scheme]]]);
      // The unsaid button: an outline in the label's ink, whatever the page's `color`; 36 px.
      expect([at('bordered'), got.bordered]).toEqual([at('bordered'), [INK[scheme], [0, 0, 0, 0], LINE[scheme], '1px', 36]]);
      expect([at('gray'), got.gray]).toEqual([at('gray'), [INK[scheme], 'solid', 36]]);
      expect([at('destructive'), got.destructive]).toEqual([at('destructive'), RED[scheme]]);
      // The accent is the browser's hue, so its numbers are each engine's own; what holds in all of them is that it
      // reads as text on the page (a selected tab's label) and that the ink on a fill of it reads.
      const [fill, onFill, asText] = got.accent;
      expect([at('accent as text'), await page.evaluate(([a, b]) => contrast(a, b), [asText, GROUND[scheme]]) >= 4.5]).toEqual([at('accent as text'), true]);
      expect([at('ink on the accent'), await page.evaluate(([a, b]) => contrast(a, b), [onFill, fill]) >= 4.5]).toEqual([at('ink on the accent'), true]);
      expect([at('ink on the accent'), onFill]).toEqual([at('ink on the accent'), scheme === 'light' ? [255, 255, 255, 1] : [0, 0, 0, 1]]);
      // An accent the author named is drawn as written, under the ink that reads on it.
      expect([at('authored'), got.authored]).toEqual([at('authored'), [[255, 221, 0, 1], [0, 0, 0, 1]]]);
      expect([at('disabled'), got.disabled]).toEqual([at('disabled'), [scheme === 'light' ? [112, 112, 112, 1] : [146, 146, 146, 1], 'none']]);
      // A field's ink is its own, not the page's; the node's own, written, wins. Every field is 36 px.
      expect([at('field'), got.field]).toEqual([at('field'), [INK[scheme], SURFACE[scheme], LINE[scheme], 36, [0, 128, 0, 1]]]);
      expect([at('heights'), got.heights]).toEqual([at('heights'), [36, 36]]);
      // The marks, drawn where the host says the appearance is the platform's; an author's box is left alone.
      expect([at('check'), got.check]).toEqual([at('check'), ['none', [20, 20], LINE[scheme], fill, onFill]]);
      expect([at('own'), got.own]).toEqual([at('own'), ['0px', 'none']]);
      expect([at('radio'), got.radio]).toEqual([at('radio'), [[20, 20], '50%']]);
      expect([at('switch'), got.toggle]).toEqual([at('switch'), ['none', '36px', '20px', LINE[scheme], fill]]);
      // The spinner is not the text's colour around it, unless its own colour is written.
      expect([at('spinner'), got.spinner]).toEqual([at('spinner'), [false, true]]);
      // A segmented track is the width of its segments, 36 px, its chosen segment a chip.
      expect([at('segments'), got.segments]).toEqual([at('segments'), [true, true, 36, true]]);
      // A plain bar: opaque, its own ink, a 1 px line, and an unsaid button in it has no outline.
      expect([at('bar'), got.bar]).toEqual([at('bar'), [INK[scheme], GROUND[scheme], 'none', '1px', 0]]);
      expect([at('tabs'), got.tabs]).toEqual([at('tabs'), [GROUND[scheme], '1px', 'none']]);
      // A grouped list: the neutral ground over the sheet's literal, 12 px cards; a ground the author wrote stays.
      expect([at('list'), got.list]).toEqual([at('list'), [scheme === 'light' ? [245, 245, 245, 1] : GROUND[scheme], '12px', [1, 2, 3, 1]]]);
      // A sheet in a wide window is a card.
      expect([at('sheet'), got.sheet]).toEqual([at('sheet'), [GROUND[scheme], 'fixed', 560]]);

      // The unsaid hover: a wash over a pressable nobody styled, and a link in running text underlines.
      await page.hover('#pressable');
      expect([at('hover'), await page.evaluate(() => css('pressable', 'backgroundImage') !== 'none')]).toEqual([at('hover'), true]);
      await page.hover('#inline');
      expect([at('link'), await page.evaluate(() => css('inline', 'textDecorationLine'))]).toEqual([at('link'), 'underline']);
      await page.mouse.move(1, 1);
      expect(await page.evaluate(() => [css('pressable', 'backgroundImage'), css('inline', 'textDecorationLine')])).toEqual(['none', 'none']);

      // A menu opens under its invoker, from its start; at the row's end, from its end. Its rows are rows.
      await page.click('#invoker');
      let [menu, invoker] = await page.evaluate(() => [box('menu'), box('invoker')]);
      expect([at('menu'), Math.abs(menu[0] - invoker[0]) <= 1, menu[1] - invoker[1] - invoker[3], menu[2]]).toEqual([at('menu'), true, 4, 176]);
      expect([at('menu rows'), await page.evaluate(() => [color('item'), color('item', 'backgroundColor'), css('item', 'borderTopStyle'), color('item-delete'), color('menu', 'backgroundColor')])])
        .toEqual([at('menu rows'), [INK[scheme], [0, 0, 0, 0], 'none', RED[scheme], SURFACE[scheme]]]);
      await page.keyboard.press('Escape');
      await page.click('#invoker-end');
      [menu, invoker] = await page.evaluate(() => [box('menu'), box('invoker-end')]);
      expect([at('menu at the end'), Math.abs(menu[0] + menu[2] - invoker[0] - invoker[2]) <= 1, menu[1] - invoker[1] - invoker[3]]).toEqual([at('menu at the end'), true, 4]);
      await page.keyboard.press('Escape');
      // A context menu opens at the pointer, in every engine (the release of the button that asked no longer
      // dismisses it), and in a corner it opens the other way, inside the window.
      await page.evaluate(() => document.getElementById('held').addEventListener('contextmenu', (event) => { event.preventDefault(); contextAt(event, () => [document.getElementById('menu'), event.currentTarget ?? document.getElementById('held')]); }));
      const held = await page.evaluate(() => box('held'));
      await page.mouse.click(held[0] + 200, held[1] + 20, { button: 'right' });
      await page.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
      expect([at('context menu'), (await page.evaluate(() => box('menu'))).slice(0, 2)]).toEqual([at('context menu'), [held[0] + 200, held[1] + 20]]);
      await page.keyboard.press('Escape');
      await page.waitForFunction(() => !document.getElementById('menu').getAttribute('style').includes('--exact-context')); // the point goes when it closes
      // (The corner's point is clear of a scroll bar an engine may draw in the window.)
      await page.evaluate(() => { const h = document.getElementById('held'); h.style.cssText = 'position:fixed;right:0;bottom:0;width:100px;height:60px'; });
      await page.mouse.click(960, 680, { button: 'right' });
      await page.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
      menu = await page.evaluate(() => box('menu'));
      expect([at('context menu in a corner'), menu[0] + menu[2], menu[1] + menu[3]]).toEqual([at('context menu in a corner'), 960, 680]);
      await page.keyboard.press('Escape');

      // An alert reads from the start, its actions at the end, each its own width.
      const alert = await page.evaluate(() => { document.getElementById('alert').showModal(); return [css('alert', 'width'), css('alert', 'textAlign'), css('alert', 'justifyContent'), css('alert', 'borderTopLeftRadius'), color('alert', 'backgroundColor'), box('cancel')[2] < 200]; });
      expect([at('alert'), alert]).toEqual([at('alert'), ['416px', 'start', 'flex-end', '12px', SURFACE[scheme], true]]);
      await context.close();
    }
    // A narrow window: the sheet is the page's own box, full-bleed. A coarse pointer: a control is 44 px.
    const narrow = await browser.newContext({ viewport: { width: 390, height: 700 }, ...(engine === 'chromium' ? { hasTouch: true, isMobile: true } : {}) });
    const page = await narrow.newPage();
    await page.setContent(`<meta name="viewport" content="width=device-width, initial-scale=1"><style>${sheet('neutral')}</style>${fixture}<script>${helpers}</script>`);
    expect([engine, await page.evaluate(() => css('sheet', 'position'))]).toEqual([engine, 'absolute']);
    if (engine === 'chromium') expect(await page.evaluate(() => [matchMedia('(pointer: coarse)').matches, box('bordered')[3], box('field')[3], box('select')[3]])).toEqual([true, 44, 44, 44]);
  } finally { await browser.close(); }
}, 90000);
