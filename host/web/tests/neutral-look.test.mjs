// LLP 1115.000 D4 part B: the web's neutral look, behind its switch (host/web/look.mjs).
//
// With the switch off, which is how every build is made today, a build's shell is the shell's own bytes and
// a page looks as LLP 1116 D1 drew it: the first two tests hold that, the second by a handful of computed
// styles. With it on, the neutral sheet (neutral.css) follows the shell's, and what the author left unsaid
// is drawn the same in Chrome, Firefox and WebKit: the last test measures one page in each engine against
// one set of numbers. Its colours are the role table's neutral column, which the third test holds the
// sheet to. The nodes are as the hosts write them.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { DEFAULT_LOOK, withLook } from '../look.mjs';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const shell = read('../index.html'), neutral = read('../neutral.css');
const sheet = (look) => withLook(shell, look).match(/<style>([\s\S]*?)<\/style>/)[1];

test('the switch is off: a build is the shell\'s own bytes, and the neutral sheet is in no page', () => {
  expect(DEFAULT_LOOK).toBe('apple');
  expect(withLook(shell)).toBe(shell);
  expect(withLook(shell, 'apple')).toBe(shell);
  expect(shell.includes('--exact-look')).toBe(false);
  // Thrown, the neutral sheet is the end of the shell's own stylesheet, and says which look the page is.
  expect(withLook(shell, 'neutral')).toBe(shell.replace('</style>', () => neutral + '</style>'));
  expect(/:root \{ --exact-look: neutral;/.test(neutral)).toBe(true);
});

test('the neutral sheet\'s colours are the role table\'s neutral column', () => {
  const rows = JSON.parse(read('../../../kernel/tables/schema.json')).colors;
  const pair = new Map(rows.map(([name, , , , , , light, dark]) => [name, `light-dark(${light.padEnd(9, 'f')}, ${dark.padEnd(9, 'f')})`]));
  const root = /:root \{ --exact-look: neutral;([^}]*)\}/.exec(neutral)[1];
  const defined = new Map([...root.matchAll(/--exact-([a-z-]+): (light-dark\([^)]*\))/g)].map((m) => [m[1], m[2]]));
  // A role by its own name; a CSS system colour the sheet needs as a value by its name in lower case.
  const role = (name) => pair.get(name) ?? pair.get({ 'gray-text': 'GrayText' }[name]);
  for (const [name, value] of defined) expect([name, value]).toEqual([name, role(name)]);
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

test('the switch is off: a page looks as it did (a handful of computed styles of LLP 1116 D1\'s look)', async () => {
  const browser = await launch('chromium');
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 700 }, colorScheme: 'light' });
    await page.setContent(`<style>${sheet(DEFAULT_LOOK)}</style>${fixture}<script>${helpers}</script>`);
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

