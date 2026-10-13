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

// LLP 1115.000 D17: the look's metrics are rows of the schema (`metrics`, the neutral column), which the kernel sizes
// a control by and a host that paints its own controls draws from. The sheet stays CSS a person reads, so this holds
// it to the table both ways: every length it writes is a row's, on a property that row is written on, and every row
// is written. A length that follows from rows (a pill's radius, a thumb's travel) is named beside its reason.
test('the neutral sheet\'s lengths and ratios are the metric table\'s neutral column', () => {
  const rows = JSON.parse(read('../../../kernel/tables/schema.json')).metrics.filter(([, kind]) => kind !== 'style');
  const value = new Map(rows.map((row) => [row[0], row.at(-1)]));
  const length = (name) => { const v = value.get(name); return typeof v === 'number' ? `${v}px` : v.replace(/^0\./, '.'); };
  // The properties each length is written on.
  const WHERE = {
    'control-height': ['min-height', 'min-width', 'height'], 'control-height-coarse': ['min-height', 'min-width', 'height'],
    'control-radius': ['border-radius'], 'surface-radius': ['border-radius'], 'mark-radius': ['border-radius'], 'segment-radius': ['border-radius'],
    'control-line': ['border', 'border-top', 'border-bottom', 'border-inline-end', '--exact-raised', '--exact-modal', 'box-shadow'], // a list row's separator is a line in its box
    'focus-ring-offset': ['outline-offset'],
    'mark-size': ['width', 'height', 'background'], 'mark-size-coarse': ['width', 'height'],
    'switch-width': ['width'], 'switch-height': ['height'], 'range-height': ['height'], 'range-track': ['height'], 'range-thumb-line': ['border'],
    'button-padding-block': ['padding'], 'button-padding-inline': ['padding'], 'button-padding-inline-symbol': ['padding-inline'],
    'field-padding-block': ['padding'], 'field-padding-block-coarse': ['padding-block'], 'field-padding-inline': ['padding'],
    'file-padding-block': ['padding'], 'file-gap': ['margin-inline-end'],
    'segmented-padding': ['padding'], 'segment-padding-block': ['padding'], 'segment-padding-inline': ['padding'], 'segment-height-coarse': ['min-height'],
    'segment-shadow-y': ['box-shadow'], 'segment-shadow-blur': ['box-shadow'],
    'tabbar-height': ['min-height'], 'tabbar-padding-block': ['padding'], 'tabbar-padding-inline': ['padding'], 'tabbar-font': ['font-size'],
    'tabbar-symbol': ['width', 'height'], 'tabbar-gap': ['--exact-button-space-height'],
    'sidebar-row-height': ['min-height'], 'sidebar-symbol': ['width', 'height'], 'sidebar-gap': ['--exact-button-space-width'],
    'back-padding-start': ['padding'], 'back-padding-end': ['padding'],
    'alert-width': ['width'], 'alert-margin': ['width'], 'alert-padding': ['padding'], 'alert-gap': ['gap'], 'alert-action-gap': ['gap'],
    'alert-title': ['font-size'], 'alert-message-gap': ['margin-bottom'],
    'menu-min-width': ['min-width'], 'menu-max-width': ['max-width'], 'menu-margin': ['max-width'], 'menu-padding': ['padding'], 'menu-offset': ['margin'],
    'menu-row-height': ['min-height'], 'menu-row-padding-block': ['padding'], 'menu-row-padding-inline': ['padding'], 'menu-row-gap': ['--exact-button-space-width'],
    'sheet-wide': ['@media'], 'sheet-width': ['width'], 'sheet-max-height': ['max-height'], 'sheet-margin': ['width', 'max-height'],
    'raised-shadow-y': ['--exact-raised'], 'raised-shadow-blur': ['--exact-raised'], 'modal-shadow-y': ['--exact-modal'], 'modal-shadow-blur': ['--exact-modal'],
  };
  // The look keeps this one from the shell's sheet, which is where it is written.
  const SHELL = { 'select-padding-end': ['padding-inline'] };
  // What follows from rows, or is no metric.
  const half = (name, unit) => `${value.get(name) / 2}${unit}`.replace(/^0\./, '.');
  const DERIVED = {
    'min-height': [`${2 * value.get('segmented-padding')}px`], // a segment is the control's height less the track's padding either side
    'border-radius': ['1em', half('range-track', 'px')], // the switch is a pill (any radius of half its height or more); the track's ends are round
    'margin-top': [half('range-track', 'px'), '.625em'], // a range's thumb is centred on its track: half the track less half the thumb
    'background-position': ['1em'], // the thumb travels the track's width less its height
    'box-shadow': ['64px'], // a hovered row's wash reaches the card's edge, past any inset a row has
    'font-size': ['1em'], // an alert's message is the body size
  };
  const lengths = (text) => [...text.matchAll(/(?<![\w.#])(\d*\.?\d+)(px|rem|em)\b/g)].map((m) => m[0]).filter((token) => parseFloat(token) !== 0);
  const declarations = (css) => [...css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/url\("[^"]*"\)/g, '').matchAll(/\{([^{}]*)\}/g)]
    .flatMap((body) => [...body[1].matchAll(/(?:^|;)\s*([-\w]+)\s*:\s*([^;]+)/g)].map((m) => [m[1], m[2]]))
    .concat([...css.matchAll(/@media \(([^)]*)\)/g)].map((m) => ['@media', m[1]]));
  const census = (css, where) => {
    const allowed = new Map(), written = new Set(), stray = [];
    for (const [name, properties] of Object.entries(where)) for (const p of properties) allowed.set(p, [...(allowed.get(p) ?? []), [name, length(name)]]);
    for (const [property, text] of declarations(css)) for (const token of lengths(text)) {
      const of = (allowed.get(property) ?? []).filter(([, v]) => v === token);
      for (const [name] of of) written.add(name);
      if (!of.length && !(where === WHERE && DERIVED[property]?.includes(token))) stray.push(`${property}: ${token}`);
    }
    return { written, stray: [...new Set(stray)] };
  };
  const sheetCensus = census(neutral, WHERE);
  expect(sheetCensus.stray).toEqual([]);
  expect(Object.keys(WHERE).filter((name) => !sheetCensus.written.has(name))).toEqual([]);
  expect(Object.keys(SHELL).filter((name) => !census(shell, SHELL).written.has(name))).toEqual([]);
  // Every length row is one a sheet writes.
  expect(rows.filter(([name, kind]) => kind === 'length' && !WHERE[name] && !SHELL[name]).map(([name]) => name)).toEqual([]);

  // The ratios: how each is spelled where the sheet writes it (the shell's sheet for a select's own line).
  const percent = (name) => `${Math.round(value.get(name) * 100)}%`, alpha = (name) => String(value.get(name)).replace(/^0\./, '.');
  const RATIO = {
    'control-line-alpha': `--exact-line: color-mix(in srgb, var(--exact-label) ${percent('control-line-alpha')}, transparent)`,
    'mark-dot': `radial-gradient(circle closest-side, currentColor ${percent('mark-dot')},`,
    'mark-disabled-alpha': `:disabled { opacity: ${alpha('mark-disabled-alpha')}; }`,
    'button-tint-alpha': `{ background-color: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-tint-alpha')}, transparent); }`,
    'button-hover-tinted': `:hover { background-color: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-tint-alpha')}, transparent); background-image: linear-gradient(color-mix(in srgb, currentColor ${percent('button-hover-tinted')}, transparent) 0 0); }`,
    'button-hover-filled': `:hover { background-color: var(--exact-accent, AccentColor); background-image: linear-gradient(color-mix(in srgb, currentColor ${percent('button-hover-filled')}, transparent) 0 0); }`,
    'button-pressed-tinted': `[data-button-style="bordered-tinted"])[data-pressed] { background-image: linear-gradient(color-mix(in srgb, currentColor ${percent('button-pressed-tinted')}, transparent) 0 0); }`,
    'button-pressed-filled': `[data-button-style="bordered-prominent"])[data-pressed] { background-image: linear-gradient(color-mix(in srgb, currentColor ${percent('button-pressed-filled')}, transparent) 0 0); }`,
    'select-line-height-coarse': `select:not([multiple]) { line-height: ${value.get('select-line-height-coarse')}; }`,
    'switch-thumb': `radial-gradient(circle closest-side, currentColor ${percent('switch-thumb')},`,
    'segment-shadow-alpha': `box-shadow: 0 1px 2px rgb(0 0 0 / ${alpha('segment-shadow-alpha')});`,
    'raised-shadow-alpha': `--exact-raised: 0 0 0 1px var(--exact-separator), 0 4px 12px rgb(0 0 0 / ${alpha('raised-shadow-alpha')});`,
    'modal-shadow-alpha': `--exact-modal: 0 0 0 1px var(--exact-separator), 0 16px 48px rgb(0 0 0 / ${alpha('modal-shadow-alpha')});`,
    'scrim-alpha': `--exact-scrim: rgb(0 0 0 / ${alpha('scrim-alpha')});`,
  };
  for (const [name, spelled] of Object.entries(RATIO)) expect([name, neutral.includes(spelled)]).toEqual([name, true]);
  expect(shell.includes(`select:not([multiple]) { appearance: none; font: inherit; letter-spacing: inherit; color: inherit; line-height: ${value.get('select-line-height')};`)).toBe(true);
  // A control's line is one height and its title one weight, wherever the sheets write them: a select's own line
  // aside, and a menu's rows, which are regular.
  const all = (css, property) => [...new Set(declarations(css).filter(([p]) => p === property).map(([, v]) => v.trim()))];
  expect(all(neutral, 'line-height').sort()).toEqual([String(value.get('control-line-height')), String(value.get('select-line-height-coarse'))]);
  expect(all(neutral, 'font-weight').sort()).toEqual(['400', String(value.get('control-weight'))]);
  expect(shell.includes(`font-weight: ${value.get('control-weight')}; border: none; border-radius: var(--exact-control-radius, ${length('control-radius')});`)).toBe(true);
  expect(rows.filter(([name, kind]) => kind === 'number' && !RATIO[name] && !['control-line-height', 'control-weight', 'select-line-height'].includes(name)).map(([name]) => name)).toEqual([]);
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
    <div role="list" data-listrows="true" id="rows" style="width:300px"><div id="row-1" style="height:44px">One</div><div id="row-2" data-exact-on="press" tabindex="0" style="height:44px">Two</div><div id="row-own" data-exact-said="border-bottom" style="height:44px;border-bottom:2px solid red">Its own line</div><div id="row-before" style="height:44px">Before a row with a top edge</div><div id="row-top" data-exact-said="border-top" style="height:44px;border-top:2px solid red">Its own top edge</div><div id="row-last" style="height:44px">Last</div></div>
    <div role="list" data-listrows="true" id="windowed" style="width:300px"><div aria-hidden="true" id="spacer" style="height:10px"></div><div role="listitem" data-listitemkey="a" id="cell-a"><div style="height:44px">A</div></div><div role="listitem" data-listitemkey="b" id="cell-b"><div data-exact-said="border-bottom" style="height:44px;border-bottom:1px solid red">B</div></div><div role="listitem" data-listitemkey="c" id="cell-c"><div style="height:44px">C</div></div></div>
    <div role="list" id="rows-none" style="width:300px"><div id="none-1" data-exact-on="press" style="height:44px">The author's</div><div style="height:44px">rows</div></div>
  </div>
  <div popover="auto" role="menu" id="menu" style="display:flex;flex-direction:column"><button type="button" data-native data-button-style="bordered" id="item" style="display:grid"><span data-exact-text>Rename</span></button><button type="button" data-native data-button-style="bordered" data-destructive="true" id="item-delete" style="display:grid"><span data-exact-text>Delete</span></button></div>
  <dialog id="alert" role="alertdialog" aria-label="Sure?"><div data-exact-text>It cannot be undone.</div><button type="button" data-native data-button-style="bordered" data-destructive="true" id="delete" style="display:grid"><span data-exact-text>Delete</span></button><button type="button" data-native data-button-style="bordered" id="cancel" style="display:grid"><span data-exact-text>Cancel</span></button></dialog>
</div>`;
// What the page answers with: a colour as the engine paints it (each serializes `color-mix()` its own way), a box.
const helpers = `
  const ink = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  window.rgba = (color) => { ink.clearRect(0, 0, 1, 1); ink.fillStyle = '#000'; ink.fillStyle = color; ink.fillRect(0, 0, 1, 1); const [r, g, b, a] = ink.getImageData(0, 0, 1, 1).data; return a ? [r, g, b, Math.round(a / 2.55) / 100] : [0, 0, 0, 0]; };
  window.css = (id, property, pseudo) => getComputedStyle(document.getElementById(id), pseudo)[property];
  window.color = (id, property = 'color', pseudo) => rgba(css(id, property, pseudo));
  window.box = (id) => { const r = document.getElementById(id).getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 100) / 100); };
  // A row's separator: the colour of the line drawn inside its bottom edge and the line's height, or null for none.
  window.separator = (id) => { const s = css(id, 'boxShadow'); if (s === 'none') return null; const c = s.match(/rgba?\\([^)]*\\)|color\\([^)]*\\)/)[0]; return [rgba(c), /inset/.test(s), s.replace(c, '').match(/-?[\\d.]+px/g)]; };
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

      // A list's rows (LLP 1115.000 D16): under each row but the last one line in the separator's role, inside the
      // row's box (a row keeps its height); none beside a row whose author wrote its own top or bottom edge; a
      // windowed list's rows the same through their wrappers, its spacers none. A list with no mark is untouched.
      const LINE_BETWEEN = [scheme === 'light' ? [0, 0, 0, 0.12] : [255, 255, 255, 0.16], true, ['0px', '-1px', '0px', '0px']];
      expect([at('rows'), await page.evaluate(() => ['row-1', 'row-2', 'row-own', 'row-before', 'row-top', 'row-last'].map(separator))])
        .toEqual([at('rows'), [LINE_BETWEEN, LINE_BETWEEN, null, null, LINE_BETWEEN, null]]);
      expect([at('row heights'), await page.evaluate(() => [box('row-1')[3], box('row-own')[3], box('rows')[3]])]).toEqual([at('row heights'), [44, 46, 268]]);
      expect([at('windowed rows'), await page.evaluate(() => ['spacer', 'cell-a', 'cell-b', 'cell-c'].map(separator))]).toEqual([at('windowed rows'), [null, LINE_BETWEEN, null, null]]);
      expect([at('no mark'), await page.evaluate(() => separator('none-1'))]).toEqual([at('no mark'), null]);
      // A row that presses: the hover's wash, a stronger one while held (never the dim), the ring inside the row.
      await page.hover('#row-2');
      const hovered = await page.evaluate(() => css('row-2', 'backgroundImage'));
      await page.mouse.down();
      const pressed = await page.evaluate(() => [css('row-2', 'backgroundImage'), css('row-2', 'opacity')]);
      await page.mouse.up();
      expect([at('row press'), hovered !== 'none', pressed[0] !== 'none' && pressed[0] !== hovered, pressed[1]]).toEqual([at('row press'), true, true, '1']);
      await page.hover('#none-1');
      await page.mouse.down();
      expect([at('no mark press'), await page.evaluate(() => css('none-1', 'opacity'))]).toEqual([at('no mark press'), '0.65']);
      await page.mouse.up();
      await page.keyboard.press('Tab');
      expect([at('row focus'), await page.evaluate(() => { const row = document.getElementById('row-2'); row.focus(); return [row.matches(':focus-visible'), css('row-2', 'outlineOffset'), css('row-2', 'outlineStyle') !== 'none']; })])
        .toEqual([at('row focus'), [true, '-2px', true]]);
      await page.evaluate(() => document.activeElement.blur());
      await page.mouse.move(1, 1);

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

      // An alert reads from the start. Its actions share the row equally (416 less 24 of padding each side and a 12 px
      // gap is 178 each), and a destructive one's edge is its red, where the other's is the look's line.
      const alert = await page.evaluate(() => { document.getElementById('alert').showModal(); return [css('alert', 'width'), css('alert', 'textAlign'), css('alert', 'borderTopLeftRadius'), color('alert', 'backgroundColor'), box('delete')[2], box('cancel')[2], css('delete', 'borderTopColor') === css('delete', 'color'), css('cancel', 'borderTopColor') === css('delete', 'borderTopColor')]; });
      expect([at('alert'), alert]).toEqual([at('alert'), ['416px', 'start', '12px', SURFACE[scheme], 178, 178, true, false]]);
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

// What the author writes wins (LLP 1115). In CSS a minimum outranks a size and a border box is never narrower than
// its padding, so the look's floor and padding would redraw `button width=84 height=24` as 84 by 36. The hosts name a
// written size (`data-exact-said`, host/web/src/element.rs) and the sheet steps aside on that axis.
test('a size the author wrote on a platform button is its box, and an unsized button keeps the look\'s floor', async () => {
  const browser = await launch('chromium');
  try {
    const page = await browser.newPage({ viewport: { width: 600, height: 400 }, colorScheme: 'light' });
    const button = (id, style, said) => `<button type="button" data-native data-button-style="bordered" id="${id}" style="${style}"${said ? ` data-exact-said="${said}"` : ''}>Go</button>`;
    await page.setContent(`<style>${sheet('neutral')}</style><div id="exact-root">${button('unsized', '')}${button('sized', 'width:84px;height:24px', 'width height')}${button('square', 'width:28px;height:28px', 'width height')}${button('short', 'height:24px', 'height')}${button('unsaid', 'width:28px;height:28px')}</div><script>${helpers}</script>`);
    const got = await page.evaluate(() => ({ unsized: box('unsized')[3], sized: box('sized').slice(2), square: box('square').slice(2), short: box('short')[3], unsaid: box('unsaid').slice(2) }));
    expect(got).toEqual({ unsized: 36, sized: [84, 24], square: [28, 28], short: 24, unsaid: [36, 36] }); // `unsaid`: the floor, were the host not to say
  } finally { await browser.close(); }
}, 90000);

// A part colour's alpha is its only opacity. Removing it restores the look's default.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: authored placeholder colour and reset in both looks`, async () => {
  const browser = await launch(engine);
  try {
    for (const look of ['apple', 'neutral']) {
      const page = await browser.newPage({ colorScheme: 'light' });
      await page.setContent(`<style>${sheet(look)}</style><div id="exact-root">
        <input data-native placeholder="Search"><textarea data-native placeholder="Notes"></textarea>
      </div>`);
      const sample = () => page.evaluate(() => [...document.querySelectorAll('input, textarea')].map(el => {
        const css = getComputedStyle(el, '::placeholder');
        return [css.color, css.opacity];
      }));
      const original = await sample();
      for (const color of ['rgb(255, 0, 0)', 'rgba(255, 0, 0, 0.5)']) {
        await page.evaluate(color => {
          for (const el of document.querySelectorAll('input, textarea')) el.style.setProperty('--exact-placeholder-color', color);
        }, color);
        expect([look, await sample()]).toEqual([look, [[color, '1'], [color, '1']]]);
        await page.evaluate(() => {
          for (const el of document.querySelectorAll('input, textarea')) el.disabled = true;
        });
        expect([look, 'disabled', await sample()]).toEqual([look, 'disabled', [[color, '1'], [color, '1']]]);
        await page.evaluate(() => {
          for (const el of document.querySelectorAll('input, textarea')) el.disabled = false;
        });
      }
      await page.evaluate(() => {
        for (const el of document.querySelectorAll('input, textarea')) el.removeAttribute('style');
      });
      expect([look, await sample()]).toEqual([look, original]);
      await page.close();
    }
  } finally { await browser.close(); }
}, 90000);
