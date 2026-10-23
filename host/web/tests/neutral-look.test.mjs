// LLP 1115.000 D4 part B: one neutral web presentation in Chrome, Firefox and WebKit.
// The role and metric censuses hold the sheet to the schema; the browser oracle
// keeps the same geometry, colours and author precedence in all three engines.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { withLook, appleColors } from '../look.mjs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { glueDeclarations } from './glue-fixture.mjs';
import { decodePng } from '../../../scripts/png.mjs';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const shell = read('../index.html'), neutral = read('../neutral.css');
// How both runtimes draw a symbol: its mask in its tint, `--exact-tint`.
const symbolStyle = read('../glue.js').match(/symbolStyle\.textContent = '([^']*)'/)[1];
// The neutral look alone, its roles the neutral column's: the Apple colours a build lays over it, in every
// browser, are apple-colors.test.mjs's
const sheet = () => withLook(shell).match(/<style>([\s\S]*?)<\/style>/)[1].replace(appleColors(), '');

test('a build composes the neutral sheet, then Apple\'s colours, after the browser resets', () => {
  expect(withLook(shell)).toBe(shell.replace('</style>', () => neutral + appleColors() + '</style>'));
});

test('route header geometry is the schema\'s neutral default', () => {
  const rows = JSON.parse(read('../../../kernel/tables/schema.json')).metrics;
  const value = (name) => rows.find(([n]) => n === `header-${name}`).at(-1);
  const px = (name) => value(name) === 0 ? '0' : `${value(name)}px`;
  const rule = shell.match(/:where\(#exact-root \[navigationkey\] > header\) \{([^}]+)\}/)[1];
  const declarations = Object.fromEntries([...rule.matchAll(/([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2]]));
  expect(declarations).toEqual({ position: value('position'), top: px('top'), 'min-height': px('min-height'), padding: `${px('padding-block')} ${px('padding-inline')}` });
});

test('the neutral sheet\'s colours are the role table\'s neutral column', () => {
  const rows = JSON.parse(read('../../../kernel/tables/schema.json')).colors;
  const pair = new Map(rows.map(([name, , , , , , light, dark]) => [name, `light-dark(${light.padEnd(9, 'f')}, ${dark.padEnd(9, 'f')})`]));
  const root = /:root \{([^}]*)\}/.exec(neutral)[1];
  const defined = new Map([...root.matchAll(/--exact-([a-z-]+): (light-dark\([^)]*\))/g)].map((m) => [m[1], m[2]]));
  // A role by its own name; a CSS system colour the sheet needs as a value by its name in lower case.
  const role = (name) => pair.get(name) ?? pair.get({ 'gray-text': 'GrayText', 'field-text': 'FieldText' }[name]);
  for (const [name, value] of defined) expect([name, value]).toEqual([name, role(name)]);
  // Every role named by either sheet is defined in the neutral column.
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
  // A row's neutral value is its third cell when it has three, else its sixth; a row whose neutral value is unset is
  // another look's own (Android's Material rows) and no sheet's here.
  const neutralCell = (row) => (row.length === 3 ? row[2] : row[5]);
  const schema = JSON.parse(read('../../../kernel/tables/schema.json'));
  const rows = schema.metrics
    .filter((row) => !row[1].startsWith('style') && neutralCell(row) !== 'unset'); // `style` and `style length`: the grouped sheet's
  const value = new Map(rows.map((row) => [row[0], neutralCell(row)]));
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
    'button-image-gap': ['--exact-button-column-gap', '--exact-button-row-gap'],
    'tabbar-symbol': ['width', 'height'], 'tabbar-gap': ['--exact-button-space-height'],
    'sidebar-row-height': ['min-height'], 'sidebar-symbol': ['width', 'height'], 'sidebar-gap': ['--exact-button-space-width'],
    'back-padding-start': ['padding'], 'back-padding-end': ['padding'], 'bar-symbol-size': ['font-size'],
    'alert-width': ['width'], 'alert-margin': ['width'], 'alert-padding': ['padding'], 'alert-gap': ['gap'], 'alert-action-gap': ['gap'],
    'alert-title': ['font-size'], 'alert-message-gap': ['margin-bottom', 'margin-top'], 'alert-scroll-gutter': ['padding-inline-end'],
    'menu-min-width': ['min-width'], 'menu-max-width': ['max-width'], 'menu-margin': ['max-width'], 'menu-padding': ['padding'], 'menu-offset': ['margin'],
    'menu-row-height': ['min-height'], 'menu-row-padding-block': ['padding'], 'menu-row-padding-inline': ['padding'], 'menu-row-gap': ['--exact-button-space-width'],
    'sheet-wide': ['@media'], 'sheet-width': ['width'], 'sheet-height': ['height'], 'sheet-margin': ['width', 'height'], 'sheet-top': ['inset'],
    'list-row-inset': ['padding-left', 'padding-right'],
    'raised-shadow-y': ['--exact-raised'], 'raised-shadow-blur': ['--exact-raised'], 'modal-shadow-y': ['--exact-modal'], 'modal-shadow-blur': ['--exact-modal'],
  };
  // The look keeps this one from the shell's sheet, which is where it is written.
  const SHELL = { 'select-padding-end': ['padding-inline'] };
  // What follows from rows, or is no metric.
  const half = (name, unit) => `${value.get(name) / 2}${unit}`.replace(/^0\./, '.');
  // The large title's size at the web's 16 px root, from the text style table.
  const largeTitle = schema.textStyles.find(([name]) => name === 'large-title')[5][schema.textStyleBodies.indexOf(16)];
  const DERIVED = {
    'min-height': [`${2 * value.get('segmented-padding')}px`], // a segment is the control's height less the track's padding either side
    'border-radius': ['1em', half('range-track', 'px')], // the switch is a pill (any radius of half its height or more); the track's ends are round
    'margin-top': [half('range-track', 'px'), '.625em'], // a range's thumb is centred on its track: half the track less half the thumb
    'background-position': ['1em'], // the thumb travels the track's width less its height
    'box-shadow': ['64px'], // a hovered row's wash reaches the card's edge, past any inset a row has
    'font-size': ['1em'], // an alert's message is the body size
    'margin-block': [`${largeTitle / 16}rem`], // a bar's level-1 title takes the large title's line
    'margin': ['.5em', '1rem', '1em'], '--exact-symbol-away': ['.5em'], '--exact-symbol-toward': ['.5em'], // a button's symbol overhangs its 1em place by half its growth (a bar item's, its 1rem place)
    'padding-block': ['.125em'], // a written height keeps room for a titled button's symbol's ink past its place
    'width': ['1em'], 'height': ['1em'], // a bar item's symbol is its em square times its scale, as a button face's is
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
    'control-line-alpha': `--exact-line: color-mix(in srgb, rgb(from var(--exact-label) r g b / 1) ${percent('control-line-alpha')}, transparent)`,
    'mark-dot': `radial-gradient(circle closest-side, currentColor ${percent('mark-dot')},`,
    'mark-disabled-alpha': `:disabled { opacity: ${alpha('mark-disabled-alpha')}; }`,
    'button-tint-alpha': `{ background-color: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-tint-alpha')}, transparent); }`,
    'button-hover-tinted': `:hover { background-color: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-tint-alpha')}, transparent); background-image: linear-gradient(var(--exact-wash) 0 0); --exact-wash: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-hover-tinted')}, transparent); }`,
    'button-hover-filled': `:hover { background-color: var(--exact-accent, AccentColor); background-image: linear-gradient(color-mix(in srgb, currentColor ${percent('button-hover-filled')}, transparent) 0 0); }`,
    'button-pressed-tinted': `[data-button-style="bordered-tinted"])[data-pressed]:not(:disabled) { background-image: linear-gradient(var(--exact-wash) 0 0); --exact-wash: color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-pressed-tinted')}, transparent); }`,
    'button-pressed-alpha': [`{ background-color: light-dark(color-mix(in srgb, var(--exact-accent, AccentColor) ${percent('button-pressed-alpha')}, transparent),`, `& > :not(img), &::after { opacity: ${alpha('button-pressed-alpha')}; }`, `& > img { --exact-tint: color-mix(in srgb, currentColor ${percent('button-pressed-alpha')}, transparent); }`],
    'button-pressed-lighten': [`color-mix(in srgb, var(--exact-accent, AccentColor), #fff ${percent('button-pressed-lighten')})); background-image: none; }`, `{ background-image: linear-gradient(var(--exact-wash) 0 0); --exact-wash: color-mix(in srgb, #fff ${percent('button-pressed-lighten')}, transparent); }`],
    'bar-hover-alpha': `{ opacity: var(--exact-hover-dim, ${alpha('bar-hover-alpha')}); }`,
    'select-line-height-coarse': `--exact-control-line-height: ${value.get('select-line-height-coarse')};`,
    'switch-thumb': `radial-gradient(circle closest-side, currentColor ${percent('switch-thumb')},`,
    'segment-shadow-alpha': `box-shadow: 0 1px 2px rgb(0 0 0 / ${alpha('segment-shadow-alpha')});`,
    'raised-shadow-alpha': `--exact-raised: 0 0 0 1px var(--exact-separator), 0 4px 12px rgb(0 0 0 / ${alpha('raised-shadow-alpha')});`,
    'modal-shadow-alpha': `--exact-modal: 0 0 0 1px var(--exact-separator), 0 16px 48px rgb(0 0 0 / ${alpha('modal-shadow-alpha')});`,
    'scrim-alpha': `--exact-scrim: rgb(0 0 0 / ${alpha('scrim-alpha')});`,
    'symbol-scale': `{ --exact-symbol-scale: ${value.get('symbol-scale')}; }`,
    'button-symbol-scale': `{ --exact-symbol-scale: ${value.get('button-symbol-scale')}; margin: calc(`,
  };
  for (const [name, spelled] of Object.entries(RATIO)) expect([name, [spelled].flat().every((s) => neutral.includes(s))]).toEqual([name, true]);
  // The one ratio the table does not hold: iOS's large-title line height, about 41 pt for 34 pt, which a bar's
  // level-1 title takes at the large title's size.
  expect(neutral.includes(`margin-block: max(0px, (${largeTitle / 16}rem * 1.2 - 1lh) / 2);`)).toBe(true);
  expect(shell.includes(`line-height: var(--exact-control-line-height); --exact-control-line-height: ${value.get('select-line-height')};`)).toBe(true);
  // A control's line is one height and its title one weight, wherever the sheets write them: a select's own line
  // aside, and a menu's rows, which are regular.
  const all = (css, property) => [...new Set(declarations(css).filter(([p]) => p === property).map(([, v]) => v.trim()))];
  expect(all(neutral, 'line-height').sort()).toEqual([String(value.get('control-line-height')), 'var(--exact-control-line-height)']);
  // Every symbol's scale is one of the two rows: a bar item's restates the image's.
  expect(all(neutral, '--exact-symbol-scale').sort()).toEqual([String(value.get('symbol-scale')), String(value.get('button-symbol-scale'))]);
  expect(all(neutral, 'font-weight').sort()).toEqual(['400', String(value.get('control-weight'))]);
  expect(shell.includes(`font-weight: ${value.get('control-weight')}; --exact-said-font-weight: ; border-radius: var(--exact-control-radius, ${length('control-radius')});`)).toBe(true);
  expect(rows.filter(([name, kind]) => kind === 'number' && !RATIO[name] && !['control-line-height', 'control-weight', 'select-line-height'].includes(name)).map(([name]) => name)).toEqual([]);
});

// Touch sizes follow the device, whichever pointer it uses now (`any-pointer`, Media Queries 4 §5.3), so no control
// resizes as a mouse comes or goes; the agent's check asks for a touch target by the same rule.
test('the neutral sheet sizes for touch by the device, not the current pointer', () => {
  expect(read('../neutral.css').match(/@media[^{]*pointer[^{]*/g).filter((q) => q.trim() !== '@media (any-pointer: coarse)')).toEqual([]);
  expect(read('../../web-js/check.js').match(/\([a-z-]*pointer: coarse\)/g)).toEqual(['(any-pointer: coarse)']);
});

// The page each browser draws: one of each control.
const fixture = `
<div id="exact-root">
  <main navigationKey="1" navigationBack="back" id="nav" style="display:flex;flex-direction:column;height:240px;position:relative">
    <div role="tabpanel" id="p1" style="flex:1;position:relative;color:rgb(255, 0, 0)">
      <div navigationKey="1" id="route"><header id="bar"><h1>Inbox</h1><button type="button" data-native data-button-style="bordered" id="bar-button" style="display:grid"><span data-exact-text>Edit</span></button><button type="button" id="bar-plain" style="width:44px;height:44px"></button><button type="button" id="bar-said" style="width:44px;height:44px;--exact-hover-dim:1"></button><button type="button" id="bar-own" data-exact-on="press pointerdown" style="width:44px;height:44px;opacity:.5"></button></header></div>
      <div navigationKey="2" navigationPresentation="modal" id="sheet" style="position:absolute;inset:0;display:flex;flex-direction:column;visibility:hidden"><header><h2>New</h2></header><div style="height:120px"></div></div>
    </div>
    <div role="tablist" id="tabs" style="display:flex"><button type="button" role="tab" aria-controls="p1" aria-selected="true" data-native data-button-style="bordered" id="tab" style="display:grid;flex:1"><span data-exact-text>Inbox</span></button></div>
  </main>
  <div id="said" style="color:rgb(255, 0, 0)">
    <div style="display:flex;justify-content:space-between"><button type="button" data-native data-button-style="bordered" popovertarget="menu" id="invoker" style="display:grid"><span data-exact-text>Menu</span></button><button type="button" data-native data-button-style="bordered" popovertarget="chooser" id="chooser-invoker" style="display:grid"><span data-exact-text>Open in</span></button><button type="button" data-native data-button-style="bordered" commandfor="chooser" command="show-popover" id="chooser-command" style="display:grid"><span data-exact-text>Open with</span></button><button type="button" data-native data-button-style="bordered" popovertarget="asked" id="asked-invoker" style="display:grid"><span data-exact-text>Remove</span></button><button type="button" data-native data-button-style="bordered" popovertarget="named" id="named-invoker" style="display:grid"><span data-exact-text>Rename</span></button><button type="button" data-native data-button-style="bordered" popovertarget="kept" id="kept-invoker" style="display:grid"><span data-exact-text>Share</span></button><button type="button" data-native data-button-style="bordered" popovertarget="menu" id="invoker-end" style="display:grid"><span data-exact-text>More</span></button></div>
    <div id="held" style="height:40px"></div>
    <button type="button" data-native data-button-style="bordered" id="bordered" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="filled" id="filled" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="gray" id="gray" style="display:grid"><span data-exact-text>Button</span></button>
    <button type="button" data-native data-button-style="plain" id="plain" style="display:grid"><span data-exact-text>Manage</span></button><button type="button" data-native data-button-style="borderless" id="borderless" style="display:grid"><span data-exact-text>Manage</span></button><button type="button" data-native data-button-style="plain" id="plain-paint" style="display:grid;--exact-press-opacity-to:.5"><span data-exact-text>Manage</span></button><button type="button" data-native data-button-style="plain" id="plain-scale" style="display:grid;--exact-press:.9"><span data-exact-text>Manage</span></button><button type="button" data-native data-button-style="plain" disabled id="plain-off" style="display:grid"><span data-exact-text>Manage</span></button>
    <button type="button" data-native data-button-style="gray" id="small" style="display:grid;--exact-control-font-size:0.875rem;--exact-touch-floor:0px"><span data-exact-text>Small</span></button>
    <button type="button" data-native data-button-style="gray" id="medium" style="display:grid;--exact-control-font-size:1rem"><span data-exact-text>Medium</span></button>
    <button type="button" data-native data-button-style="gray" id="large" style="display:grid;--exact-control-font-size:1.125rem"><span data-exact-text>Large</span></button>
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
    <button data-exact-said="background-color" id="filled" style="width:120px;height:40px;background-color:#2f6b3f;color:#ffffff">Add</button>
    <div data-exact-text id="paragraph">A <a data-view href="/x" id="inline">link</a> in it.</div>
    <div data-liststyle="inset-grouped" id="list" style="background-color:light-dark(rgb(242, 242, 247), rgb(0, 0, 0))"><section><div id="card" style="border-radius:26px;background-color:light-dark(rgb(255, 255, 255), rgb(28, 28, 30))"><div>Row</div></div></section></div>
    <div data-liststyle="inset-grouped" data-exact-said="background-color" id="list-said" style="background-color:rgb(1, 2, 3)"></div>
    <div role="list" data-listrows="true" id="rows" style="width:300px"><div id="row-1" style="height:44px">One</div><div id="row-2" data-exact-on="press" tabindex="0" style="height:44px">Two</div><div id="row-own" data-exact-said="border-bottom" style="height:44px;border-bottom:2px solid red">Its own line</div><div id="row-before" style="height:44px">Before a row with a top edge</div><div id="row-top" data-exact-said="border-top" style="height:44px;border-top:2px solid red">Its own top edge</div><div id="row-last" style="height:44px">Last</div></div>
    <div role="list" data-listrows="true" id="windowed" style="width:300px"><div aria-hidden="true" id="spacer" style="height:10px"></div><div role="listitem" data-listitemkey="a" id="cell-a"><div style="height:44px">A</div></div><div role="listitem" data-listitemkey="b" id="cell-b"><div data-exact-said="border-bottom" style="height:44px;border-bottom:1px solid red">B</div></div><div role="listitem" data-listitemkey="c" id="cell-c"><div style="height:44px">C</div></div></div>
    <div role="list" id="rows-none" style="width:300px"><div id="none-1" data-exact-on="press" style="height:44px">The author's</div><div style="height:44px">rows</div></div>
  </div>
  <div popover="auto" role="alertdialog" aria-label="Open in" id="chooser" style="display:flex;flex-direction:column"><button type="button" data-native data-button-style="bordered" data-exact-on="press" popovertarget="chooser" popovertargetaction="hide" id="choice" style="display:grid"><span data-exact-text>Maps</span></button><button type="button" data-native data-button-style="bordered" data-exact-on="press" popovertarget="chooser" popovertargetaction="hide" id="choice-2" style="display:grid"><span data-exact-text>Google Maps</span></button><button type="button" data-native data-button-style="bordered" popovertarget="chooser" popovertargetaction="hide" id="choice-cancel" style="display:grid"><span data-exact-text>Cancel</span></button></div>
  <div popover="auto" role="alertdialog" aria-label="Remove?" id="asked" style="display:flex;flex-direction:column"><div data-exact-text>It cannot be undone.</div><button type="button" data-native data-button-style="bordered" data-exact-on="press" popovertarget="asked" popovertargetaction="hide" id="asked-remove" style="display:grid"><span data-exact-text>Remove</span></button><button type="button" data-native data-button-style="bordered" popovertarget="asked" popovertargetaction="hide" id="asked-cancel" style="display:grid"><span data-exact-text>Cancel</span></button></div>
  <div popover="manual" role="alertdialog" aria-label="Share" id="kept" style="display:flex;flex-direction:column"><button type="button" data-native data-button-style="bordered" data-exact-on="press" popovertarget="kept" popovertargetaction="hide" id="kept-mail" style="display:grid"><span data-exact-text>Mail</span></button><button type="button" data-native data-button-style="bordered" popovertarget="kept" popovertargetaction="hide" id="kept-cancel" style="display:grid"><span data-exact-text>Cancel</span></button></div>
  <div popover="auto" role="alertdialog" aria-label="Rename" id="named" style="display:flex;flex-direction:column"><input data-native id="name"><button type="button" data-native data-button-style="bordered" data-exact-on="press" popovertarget="named" popovertargetaction="hide" id="named-save" style="display:grid"><span data-exact-text>Save</span></button></div>
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
  return suiteBrowser(engine);
};

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
      await page.setContent(`<style>${sheet()}</style>${fixture}<script>${helpers}</script><script type="module">${read('../navigation.js')}\nwindow.contextAt = contextAt; window.ready = true;</script>`);
      await page.waitForFunction('window.ready === true');
      const at = (name) => [engine, scheme, name];
      const got = await page.evaluate(() => ({
        page: [color('exact-root'), rgba(getComputedStyle(document.documentElement).backgroundColor)],
        bordered: [color('bordered'), color('bordered', 'backgroundColor'), color('bordered', 'borderTopColor'), css('bordered', 'borderTopWidth'), box('bordered')[3]],
        gray: [color('gray'), css('gray', 'borderTopStyle'), box('gray')[3]],
        small: [css('small', 'fontSize'), Math.round(box('small')[3])],
        destructive: color('destructive'),
        accent: [color('filled', 'backgroundColor'), color('filled'), color('tab')],
        authored: [color('authored', 'backgroundColor'), color('authored')],
        disabled: [color('disabled'), css('disabled', 'backgroundImage')],
        field: [color('field'), color('field', 'backgroundColor'), color('field', 'borderTopColor'), box('field')[3], color('field-said')],
        heights: [box('select')[3], box('date')[3]],
        check: [css('check', 'appearance'), box('check').slice(2), color('check', 'borderTopColor'), color('checked', 'backgroundColor'), color('checked')],
        own: [css('own', 'borderTopWidth'), css('own', 'content', '::before')],
        radio: [box('radio').slice(2), css('radio', 'borderRadius')],
        toggle: [css('switch', 'appearance'), css('switch', 'width', '::before'), css('switch', 'height', '::before'), color('switch', 'backgroundColor', '::before'), color('switch-on', 'backgroundColor', '::before'), color('switch'), color('switch-on')],
        spinner: [css('spinner', 'backgroundImage', '::before').includes('rgb(255, 0, 0)'), css('spinner-said', 'backgroundImage', '::before').includes('rgb(255, 0, 0)')],
        segments: [box('segments')[2] < 300, Math.abs(box('segments')[2] - box('chosen')[2] - box('segment')[2] - 6) < 1, box('segments')[3], css('chosen', 'boxShadow') !== 'none'],
        bar: [color('bar'), color('bar', 'backgroundColor'), css('bar', 'backdropFilter') ?? css('bar', 'webkitBackdropFilter'), css('bar', 'borderBottomWidth'), color('bar-button', 'borderTopColor')[3]],
        tabs: [color('tabs', 'backgroundColor'), css('tabs', 'borderTopWidth'), css('tabs', 'boxShadow')],
        list: [color('list', 'backgroundColor'), css('card', 'borderTopLeftRadius'), color('list-said', 'backgroundColor')],
        sheet: [color('sheet', 'backgroundColor'), css('sheet', 'position'), box('sheet')[2], box('sheet')[3]],
      }));
      expect([at('page'), got.page]).toEqual([at('page'), [INK[scheme], GROUND[scheme]]]);
      // The unsaid button: an outline in the label's ink, whatever the page's `color`; 36 px.
      expect([at('bordered'), got.bordered]).toEqual([at('bordered'), [INK[scheme], [0, 0, 0, 0], LINE[scheme], '1px', 36]]);
      expect([at('gray'), got.gray]).toEqual([at('gray'), [INK[scheme], 'solid', 36]]);
      // A small control: the subheadline's 14 px on a 31.75 px box (Firefox's is 31.77: it lays out in 60ths of a
      // pixel). The fixture writes css.rs's properties inline, so this checks the sheet; css_tests.rs checks the mapping.
      expect([at('small'), got.small]).toEqual([at('small'), ['14px', 32]]);
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
      // A switch's thumb is white, off or on.
      expect([at('switch'), got.toggle]).toEqual([at('switch'), ['none', '36px', '20px', LINE[scheme], fill, [255, 255, 255, 1], [255, 255, 255, 1]]]);
      // The spinner is not the text's colour around it, unless its own colour is written.
      expect([at('spinner'), got.spinner]).toEqual([at('spinner'), [false, true]]);
      // A segmented track is the width of its segments, 36 px, its chosen segment a chip.
      expect([at('segments'), got.segments]).toEqual([at('segments'), [true, true, 36, true]]);
      // A plain bar: opaque, its own ink, a 1 px line, and an unsaid button in it has no outline.
      expect([at('bar'), got.bar]).toEqual([at('bar'), [INK[scheme], GROUND[scheme], 'none', '1px', 0]]);
      expect([at('tabs'), got.tabs]).toEqual([at('tabs'), [GROUND[scheme], '1px', 'none']]);
      // A grouped list: the neutral ground over the sheet's literal, 12 px cards; a ground the author wrote stays.
      expect([at('list'), got.list]).toEqual([at('list'), [scheme === 'light' ? [245, 245, 245, 1] : GROUND[scheme], '12px', [1, 2, 3, 1]]]);
      // A sheet in a wide window is a card, 560 wide and a sheet's height whatever it holds: 45rem, less the window's
      // 64 px margin in this 700 px window.
      expect([at('sheet'), got.sheet]).toEqual([at('sheet'), [GROUND[scheme], 'fixed', 560, 636]]);

      // The unsaid hover: a wash over a pressable nobody styled, and a link in running text underlines.
      await page.hover('#pressable');
      expect([at('hover'), await page.evaluate(() => css('pressable', 'backgroundImage') !== 'none')]).toEqual([at('hover'), true]);
      // A button its author filled washes in its own ink, at the filled button's share (LLP 1129 D1).
      await page.hover('#filled');
      const wash = await page.evaluate(() => [rgba(css('filled', 'backgroundImage').match(/rgba?\([^)]*\)|color\([^)]*\)/)[0]), color('filled')]);
      expect([at('filled'), wash[0]]).toEqual([at('filled'), [...wash[1].slice(0, 3), 0.12]]);
      await page.hover('#inline');
      expect([at('link'), await page.evaluate(() => css('inline', 'textDecorationLine'))]).toEqual([at('link'), 'underline']);
      await page.mouse.move(1, 1);
      expect(await page.evaluate(() => [css('pressable', 'backgroundImage'), css('inline', 'textDecorationLine')])).toEqual(['none', 'none']);
      // In a bar it dims instead, to the shell's press dim, with no wash, and at once: nothing moves, hovered or left,
      // pressed or let go. An author's hover look (here its variable) replaces the dim, and an opacity the author wrote
      // wins.
      const seen = (id) => page.evaluate((id) => [css(id, 'opacity'), css(id, 'backgroundImage'), document.getElementById(id).getAnimations().length], id);
      // A press, held then let go: its opacity each time, and whether anything moved rather than changed at once.
      const press = (id) => page.evaluate((id) => { const b = document.getElementById(id), held = []; for (const on of [true, false]) { b.toggleAttribute('data-pressed', on); held.push(css(id, 'opacity'), b.getAnimations().length); } return held; }, id);
      expect([at('bar at rest'), await seen('bar-plain')]).toEqual([at('bar at rest'), ['1', 'none', 0]]);
      await page.hover('#bar-plain');
      expect([at('bar hover'), await seen('bar-plain')]).toEqual([at('bar hover'), ['0.65', 'none', 0]]);
      expect([at('bar hover press'), await press('bar-plain')]).toEqual([at('bar hover press'), ['0.65', 0, '0.65', 0]]);
      await page.hover('#bar-said');
      expect([at('bar left'), await seen('bar-plain')]).toEqual([at('bar left'), ['1', 'none', 0]]);
      expect([at('bar hover said'), await seen('bar-said')]).toEqual([at('bar hover said'), ['1', 'none', 0]]);
      expect([at('bar press said'), await press('bar-said')]).toEqual([at('bar press said'), ['0.65', 0, '1', 0]]);
      await page.hover('#bar-own');
      expect([at('bar hover own'), await seen('bar-own')]).toEqual([at('bar hover own'), ['0.5', 'none', 0]]);
      expect([at('bar press own'), await press('bar-own')]).toEqual([at('bar press own'), ['0.5', 0, '0.5', 0]]);
      // A form control in a bar keeps its own hover.
      await page.evaluate(() => { const select = Object.assign(document.createElement('select'), { id: 'bar-select', innerHTML: '<option>One</option>' }); select.setAttribute('data-exact-on', 'press'); document.getElementById('bar-own').after(select); });
      await page.hover('#bar-select');
      expect([at('bar hover select'), (await seen('bar-select'))[0]]).toEqual([at('bar hover select'), '1']);
      await page.evaluate(() => document.getElementById('bar-select').remove());
      await page.mouse.move(1, 1);
      // A press with no hover (a finger) dims and comes back at once, as the shell's press does.
      expect([at('bar press'), await press('bar-plain')]).toEqual([at('bar press'), ['0.65', 0, '1', 0]]);

      // Held, a plain, borderless, filled or glass button shows UIKit's highlight. Its face fades: a title, the name a
      // sheet's bar draws in place of a symbol (LLP 1116.020 D2), and a symbol, by its tint, unless the symbol's colour
      // is its own, written inline (as the wasm runtime writes it) or in a class (the JS target's). The button itself,
      // and so its focus ring, keeps its opacity. A plain or borderless button takes no wash, hovered or held. A filled
      // one's fill is at 75% in light and mixed 20% with white in dark, the same for an opaque accent and a translucent
      // one, with no wash, hovered or not; glass takes a white wash. An authored press (a paint, a scale) adds to the
      // highlight, as on iOS (NativeButtonsIOS.swift highlights whatever press is written); a disabled button takes
      // none, and a menu's or a chooser's row keeps its own look.
      await page.evaluate((symbols) => {
        document.head.insertAdjacentHTML('beforeend', `<style>${symbols} #exact-root#exact-root .own-tint { --exact-tint: rgb(52, 199, 89); }</style>`);
        const symbol = (id, own = '') => `<img data-symbol-path="M0 0h1v1H0z" alt="" id="${id}" ${own}>`;
        document.getElementById('item').insertAdjacentHTML('afterend', '<button type="button" data-native data-button-style="plain" id="item-plain" style="display:grid"><span data-exact-text>Plain</span></button><button type="button" data-native data-button-style="filled" id="item-filled" style="display:grid"><span data-exact-text>Filled</span></button>');
        document.getElementById('said').insertAdjacentHTML('beforeend', `<div data-exact-chooser id="choices"><button type="button" data-native data-button-style="plain" data-exact-on="press" id="choice-plain" style="display:grid"><span data-exact-text>Plain</span></button></div><div id="held-looks"><button type="button" data-native data-button-style="filled" id="opaque" style="display:grid;--exact-accent:var(--exact-secondary-grouped-background)"><span data-exact-text>Lock</span></button><button type="button" data-native data-button-style="filled" id="translucent" style="display:grid;--exact-accent:var(--exact-quaternary-fill)"><span data-exact-text>Lock Trunk</span>${symbol('unsaid')}${symbol('own-inline', 'style="--exact-tint:rgb(52, 199, 89)"')}${symbol('own-class', 'class="own-tint"')}</button><button type="button" data-native data-button-style="glass" id="glass" style="display:grid"><span data-exact-text>Last Parked</span></button><button type="button" data-native data-button-style="filled" id="filled-own" style="display:grid;--exact-accent:var(--exact-secondary-grouped-background);--exact-press-opacity:.5"><span data-exact-text>Own</span></button><button type="button" data-native data-button-style="tinted" id="tinted" style="display:grid"><span data-exact-text>Tinted</span></button></div>`);
        document.querySelector('#sheet > header').insertAdjacentHTML('beforeend', '<button type="button" data-native data-button-style="plain" aria-label="Cancel" id="sheet-cancel"><img data-symbol-source="symbol:sf/xmark" alt=""></button>');
      }, symbolStyle);
      // A button's fill, its image's colour ('none' for none), its opacity and its face's (its first child's, or the
      // name its `::after` draws), and its symbols' ink, at rest and held.
      const highlight = (id, pseudo) => page.evaluate(([id, pseudo]) => {
        const b = document.getElementById(id), seen = [];
        for (const on of [false, true]) {
          b.toggleAttribute('data-pressed', on);
          const image = css(id, 'backgroundImage');
          seen.push([color(id, 'backgroundColor'), image === 'none' ? image : rgba(image.match(/rgba?\([^)]*\)|color\([^)]*\)/)[0]), css(id, 'opacity'), getComputedStyle(pseudo ? b : b.firstElementChild, pseudo).opacity, [...b.querySelectorAll('img')].map((img) => [rgba(getComputedStyle(img).backgroundColor), getComputedStyle(img).opacity])]);
        }
        b.removeAttribute('data-pressed');
        return seen;
      }, [id, pseudo]);
      const NONE = [0, 0, 0, 0];
      for (const id of ['plain', 'borderless']) {
        expect([at(id), await highlight(id)]).toEqual([at(id), [[NONE, 'none', '1', '1', []], [NONE, 'none', '1', '0.75', []]]]);
        await page.hover(`#${id}`);
        expect([at(`${id} hover`), (await highlight(id))[0]]).toEqual([at(`${id} hover`), [NONE, 'none', '1', '1', []]]);
      }
      await page.mouse.move(1, 1);
      expect([at('sheet symbol'), (await highlight('sheet-cancel', '::after')).map((seen) => seen[3])]).toEqual([at('sheet symbol'), ['1', '0.75']]);
      // The highlight of a fill: the same colour at 75% in light; in dark a fifth white over it.
      const lit = ([r, g, b, a]) => page.evaluate((c) => rgba(c), scheme === 'light' ? `rgb(${r} ${g} ${b} / ${a * 0.75})`
        : `rgb(${[r, g, b].map((v) => (0.8 * a * v + 0.2 * 255) / (0.8 * a + 0.2)).join(' ')} / ${0.8 * a + 0.2})`);
      for (const id of ['opaque', 'translucent', 'filled-own']) {
        const [rest, down] = await highlight(id);
        expect([at(id), rest.slice(1, 4), down.slice(0, 4)]).toEqual([at(id), ['none', '1', '1'], [await lit(rest[0]), 'none', '1', '0.75']]);
        await page.hover(`#${id}`);
        expect([at(`${id} hovered`), (await highlight(id))[1]]).toEqual([at(`${id} hovered`), down]);
        await page.mouse.move(1, 1);
      }
      // Its symbols, each its ink and opacity: one in the button's ink fades with the title, and one in a colour of its
      // own keeps it.
      const OWN = [[52, 199, 89, 1], '1'], [inks, heldInks] = (await highlight('translucent')).map((seen) => seen[4]);
      expect([at('symbols'), inks, heldInks]).toEqual([at('symbols'), [[inks[0][0], '1'], OWN, OWN], [[[...inks[0][0].slice(0, 3), 0.75], '1'], OWN, OWN]]);
      // Released (the input glue marks it), a button eases out of its press over a quarter second: a filled button's
      // fill, a face, a symbol in the button's ink, a sheet's word and a wash; a symbol in a colour of its own has
      // nothing to ease.
      const released = (id) => page.evaluate((id) => {
        const b = document.getElementById(id);
        b.setAttribute('data-pressed', '');
        getComputedStyle(b).opacity;
        b.removeAttribute('data-pressed');
        b.setAttribute('data-exact-released', '');
        // (WebKit eases an `::after` that draws nothing too.)
        const eases = b.getAnimations({ subtree: true }).filter((a) => !a.effect.pseudoElement || getComputedStyle(a.effect.target, a.effect.pseudoElement).content !== 'none');
        const seen = eases.map((a) => `${a.effect.target.id || a.effect.target.localName}${a.effect.pseudoElement ?? ''} ${a.transitionProperty} ${a.effect.getTiming().duration} ${a.effect.getTiming().easing}`).sort();
        for (const a of eases) a.finish();
        b.removeAttribute('data-exact-released');
        return seen;
      }, id);
      const EASED = { translucent: ['span opacity', 'translucent background-color', 'unsaid background-color'], 'sheet-cancel': ['sheet-cancel::after opacity'], glass: ['glass --exact-wash', 'span opacity'], bordered: ['bordered --exact-wash'], gray: ['gray --exact-wash'], tinted: ['tinted --exact-wash'] };
      for (const [id, eased] of Object.entries(EASED)) expect([at('released'), id, await released(id)]).toEqual([at('released'), id, eased.map((ease) => `${ease} 250 ease-in-out`)]);
      // A tinted button held shows its stronger wash under a pointer too.
      await page.hover('#tinted');
      expect([at('tinted hovered'), (await highlight('tinted')).map((seen) => seen[1][3])]).toEqual([at('tinted hovered'), [0.08, 0.16]]);
      await page.mouse.move(1, 1);
      const glass = await highlight('glass');
      expect([at('glass'), glass.map((seen) => seen.slice(1, 4))]).toEqual([at('glass'), [['none', '1', '1'], [[255, 255, 255, 0.2], '1', '0.75']]]);
      for (const id of ['plain-paint', 'plain-scale']) expect([at(id), (await highlight(id))[1][3]]).toEqual([at(id), '0.75']);
      for (const id of ['plain-off', 'item-plain', 'choice-plain']) expect([at(id), (await highlight(id))[1][3]]).toEqual([at(id), '1']);
      const [rest, down] = await highlight('disabled');
      expect([at('disabled'), down]).toEqual([at('disabled'), rest]);
      expect([at('menu row'), (await highlight('item-filled'))[1].slice(0, 2)]).toEqual([at('menu row'), [NONE, scheme === 'light' ? [0, 0, 0, 0.14] : [255, 255, 255, 0.16]]]);
      await page.evaluate(() => { for (const id of ['item-plain', 'item-filled', 'choices', 'held-looks', 'sheet-cancel']) document.getElementById(id).remove(); });

      // A list's rows (LLP 1115.000 D16): under each row but the last one line in the separator's role, inside the
      // row's box (a row keeps its height); none beside a row whose author wrote its own top or bottom edge; a
      // windowed list's rows the same through their wrappers, its spacers none. A list with no mark is untouched.
      const LINE_BETWEEN = [scheme === 'light' ? [0, 0, 0, 0.12] : [255, 255, 255, 0.16], true, ['0px', '-1px', '0px', '0px']];
      expect([at('rows'), await page.evaluate(() => ['row-1', 'row-2', 'row-own', 'row-before', 'row-top', 'row-last'].map(separator))])
        .toEqual([at('rows'), [LINE_BETWEEN, LINE_BETWEEN, null, null, LINE_BETWEEN, null]]);
      // A row's box is the cell's (LLP 1138): its own edge is inside its height, and its content is inset 16.
      expect([at('row heights'), await page.evaluate(() => [box('row-1')[3], box('row-own')[3], box('rows')[3]])]).toEqual([at('row heights'), [44, 44, 264]]);
      expect([at('row inset'), await page.evaluate(() => [css('row-1', 'paddingLeft'), css('row-1', 'paddingRight')])]).toEqual([at('row inset'), ['16px', '16px']]);
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
      // A chooser its invoker opens (an `alertdialog` popover of actions and no text) is drawn at it, as a menu headed
      // by its name (which it already says to assistive technology, so the heading is silent) and with no row for its
      // cancel; one with no invoker, or whose invoker is no longer shown, and an alert with text or a field, stay centred.
      await page.click('#chooser-invoker');
      const [chooser, opener] = await page.evaluate(() => [box('chooser'), box('chooser-invoker')]);
      expect([at('chooser'), Math.abs(chooser[0] - opener[0]) <= 1, chooser[1] - opener[1] - opener[3], await page.evaluate(() => [css('choice', 'borderTopStyle'), css('chooser', 'content', '::before'), document.getElementById('choice-cancel').getClientRects().length])])
        .toEqual([at('chooser'), true, 4, ['none', '"Open in" / ""', 0]]);
      await page.keyboard.press('Escape');
      // Popover `id` opened by clicking `by`, or by its `showPopover` with `by` as the source (after `by`'s menu closed,
      // for 'closed-menu'): whether it is marked, and whether it stands at `under`, else centred.
      const opened = (id, how, by, under) => page.evaluate(([id, how, by, under]) => {
        const c = document.getElementById(id), source = by ? document.getElementById(by) : undefined;
        if (how === 'click') source.click();
        else if (how === 'closed-menu') { source.parentElement.showPopover(); source.parentElement.hidePopover(); c.showPopover({ source }); }
        else c.showPopover(source && { source });
        const r = box(id), u = under && box(under), shown = [c.hasAttribute('data-exact-chooser'), u ? Math.abs(r[0] - u[0]) <= 1 && Math.round(r[1] - u[1] - u[3]) === 4 : Math.abs(r[0] + r[2] / 2 - innerWidth / 2) <= 1];
        c.hidePopover(); return shown;
      }, [id, how, by, under]);
      // A `manual` chooser ends by neither Escape nor a click outside, so it keeps its cancel's row.
      await page.click('#kept-invoker');
      expect([at('manual chooser'), await page.evaluate(() => { const c = document.getElementById('kept'), shown = [c.hasAttribute('data-exact-chooser'), document.getElementById('kept-cancel').getClientRects().length]; c.hidePopover(); return shown; })])
        .toEqual([at('manual chooser'), [true, 1]]);
      expect([at('chooser ways'), await opened('chooser', 'show'), await opened('chooser', 'show', 'bordered', 'bordered'), await opened('chooser', 'closed-menu', 'item'), await opened('asked', 'click', 'asked-invoker'), await opened('named', 'click', 'named-invoker'), await opened('chooser', 'click', 'chooser-command', 'chooser-command')])
        .toEqual([at('chooser ways'), [false, true], [true, true], [false, true], [false, true], [false, true], [true, true]]);
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
    // A touch device: a control is 44 px, but for a small one, which keeps its own box as UIKit's does. (A narrow
    // window's sheet is below.)
    const narrow = await browser.newContext({ viewport: { width: 390, height: 700 }, hasTouch: true, ...(engine === 'chromium' ? { isMobile: true } : {}) });
    const page = await narrow.newPage();
    await page.setContent(`<meta name="viewport" content="width=device-width, initial-scale=1"><style>${sheet()}</style>${fixture}<script>${helpers}</script>`);
    expect([engine, await page.evaluate(() => [matchMedia('(any-pointer: coarse)').matches, box('bordered')[3], box('field')[3], box('select')[3], box('check')[3], Math.round(box('small')[3]), css('small', 'minWidth')])]).toEqual([engine, [true, 44, 44, 44, 24, 32, '31.5px']]);
    // A control size's title follows the root as a text style does, so small, medium and large keep their order at a
    // root the app (`setRootFontSize`) or the agent (`prefer root-font-size`) sets.
    expect([engine, await page.evaluate(() => { document.documentElement.style.fontSize = '24px'; return ['small', 'medium', 'large'].map((id) => css(id, 'fontSize')); })]).toEqual([engine, ['21px', '24px', '27px']]);
  } finally { await browser.close(); }
}, 90000);

// A sheet in a narrow window (D4.8) is iOS's: from 12 px below the window's top to its bottom, over the tab bar, its top
// corners a surface's. One that opens slides up from the window's bottom edge, unless the person asks for less motion.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a sheet in a narrow window rises from the bottom edge`, async () => {
  const browser = await launch(engine);
  try {
    for (const reducedMotion of ['no-preference', 'reduce']) {
      const context = await browser.newContext({ viewport: { width: 400, height: 700 }, reducedMotion });
      const page = await context.newPage();
      await page.setContent(`<style>${sheet()}</style>${fixture}<script>${helpers}</script>`);
      const got = await page.evaluate(() => {
        const opened = document.getElementById('sheet').cloneNode(true);
        opened.id = 'opened';
        document.getElementById('p1').append(opened);
        const moving = opened.getAnimations().map((a) => a.transitionProperty).sort();
        // Where it is as it starts and halfway: from the window's bottom edge, upward.
        const at = (t) => { for (const a of opened.getAnimations()) { a.pause(); a.currentTime = t; } return Math.round(box('opened')[1]); };
        const path = moving.length ? [at(0), at(200)] : [];
        for (const a of opened.getAnimations()) a.finish();
        return [css('opened', 'position'), box('opened'), css('opened', 'borderTopLeftRadius'), css('opened', 'borderBottomLeftRadius'), box('tabs')[1] < box('opened')[1] + box('opened')[3], css('opened', 'zIndex'), moving, path.length === 0 || (path[0] === 712 && path[1] > 12 && path[1] < 712)];
      });
      expect([engine, reducedMotion, got]).toEqual([engine, reducedMotion, ['fixed', [0, 12, 400, 688], '12px', '0px', true, '1', reducedMotion === 'reduce' ? [] : ['box-shadow', 'translate'], true]]);
      await context.close();
    }
  } finally {
    await browser.close();
  }
}, 90000);

// As it closes, it slides back down (navigation.js `sheetLeaves`): the route a commit removes stays, out of input,
// accessibility and the routes, until it is off the window; under reduced motion it is gone at once.
// The wasm runtime's commit (glue.js `apply`), with the host pieces a destroy and a reorder do not reach inert.
const glueApply = `(() => {
  globalThis.exact ??= {};
  const views = window.glueViews = new Map(), retiredViews = new WeakSet(), root = document.getElementById('exact-root'), log = () => {};
  const presence = { live: null }, listSelection = null, page = null, agentMode = false, inputReady = false, inputHandlers = null;
  const motion = { destroy() {} }, arrange = { destroy() {} }, collections = { commit() {}, restyled() {}, jump() {} };
  const followedScrolls = new Map(), pendingScrolls = new Map(), lists = new WeakSet(), messageFrames = new WeakMap(), messageViews = new Map();
  const viewFor = (_, id) => views.get(id), prepareContexts = () => {}, forgetList = () => {}, followScroll = () => {}, settleFollow = () => {}, settleValue = () => {};
  const refreshSymbols = () => {}, syncLists = () => {}, focusAutofocus = () => {}, positionContexts = () => {}, markScrollDocument = () => {}, rememberScroll = () => {};
  ${glueDeclarations(read('../glue.js'), ['apply'])}
  return apply;
})()`;
const closing = `<div id="exact-root"><main id="nav" navigationKey="1" navigationBack="back" style="position:relative;height:600px">
  <div navigationKey="1" id="under"><header><h1>Inbox</h1></header></div>
  <div navigationKey="2" navigationPresentation="modal" id="leaving" style="position:absolute;inset:0"><header><h2>New</h2></header><button type="button" id="inside">Add</button><svg><defs><linearGradient id="shade"><stop offset="0" stop-color="red"/></linearGradient></defs><rect id="swatch" width="10" height="10" fill="url(#shade)"/><use id="again" href="#swatch"/></svg></div>
</main></div>`;
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a sheet slides back down as it closes`, async () => {
  const browser = await launch(engine);
  try {
    for (const reducedMotion of ['no-preference', 'reduce']) {
      const context = await browser.newContext({ viewport: { width: 400, height: 700 }, reducedMotion });
      const page = await context.newPage();
      await page.setContent(`<style>${sheet()}</style>${closing}<script type="module">${read('../navigation.js')}
        const host = document.getElementById('exact-root');
        navigation.connect(host, () => false, () => {});
        window.select = (key) => { document.getElementById('nav').setAttribute('navigationKey', key); navigation.project(host, () => {}); };
        window.sheetLeaves = sheetLeaves; window.animationClock = animationClock; window.glueApply = ${glueApply}; window.focusById = (id) => runFocusCommands([{ name: 'focus', args: [id] }], { root: host, ready: true, inertAncestor, log: () => {} });
        // A route of the stack (a sheet unless said), what a runtime does as it removes one, and its animations ended.
        window.route = (id, key, presentation = 'modal', html = '<header><h2>Over</h2></header>', parent = document.getElementById('nav')) => { const r = Object.assign(document.createElement('div'), { id }); r.setAttribute('navigationKey', key); if (presentation) r.setAttribute('navigationPresentation', presentation); r.innerHTML = html; parent.append(r); return r; };
        window.leave = (r) => { if (!sheetLeaves(r)) r.remove(); };
        window.finish = (...routes) => { for (const r of routes) for (const a of r.getAnimations()) a.finish(); };
        window.tick = () => new Promise((r) => setTimeout(r));
        select('2'); window.ready = true;</script>`);
      await page.waitForFunction('window.ready === true');
      // The commit removes the sheet once it has risen, asking first, as a runtime does, and names the route beneath.
      await page.evaluate(() => { const route = document.getElementById('leaving'); finish(route); leave(route); select('1'); });
      // It stays where it was, whole (its ids, so its gradient and `use` still resolve), and moves from where it stood
      // down to the window's bottom edge.
      const leaving = await page.evaluate(() => {
        const ghost = document.querySelector('[data-exiting]');
        if (!ghost) return null;
        const at = (t) => { for (const a of ghost.getAnimations()) { a.pause(); a.currentTime = t; } return Math.round(ghost.getBoundingClientRect().top); };
        const path = [at(0), at(200), at(399.9)];
        for (const a of ghost.getAnimations()) a.play();
        return [ghost.id, ghost.getAttribute('navigationKey'), ghost.getAttribute('aria-hidden'), ghost.inert, ghost.previousElementSibling?.id, ghost.querySelector('rect').getAttribute('fill'), document.getElementById('shade')?.closest('[data-exiting]') === ghost, path[0] === 12 && path[1] > 12 && path[2] > 700, document.getElementById('under').inert];
      });
      expect([engine, reducedMotion, leaving]).toEqual([engine, reducedMotion, reducedMotion === 'reduce' ? null : ['leaving', '', 'true', true, 'under', 'url(#shade)', true, true, false]]);
      const gone = await page.evaluate(async () => {
        await Promise.all(document.querySelector('[data-exiting]')?.getAnimations().map((a) => (a.finish(), a.finished)) ?? []);
        await tick();
        return document.querySelector('[data-exiting], #leaving') === null;
      });
      expect([engine, reducedMotion, gone]).toEqual([engine, reducedMotion, true]);
      // A sheet that opens while one leaves ends it, as they may share ids; one it uncovers does not, and it slides
      // down over that one. While one is over the other, only the one above draws the scrim.
      const uncovered = await page.evaluate(async () => {
        const below = route('below', '6'); select('6');
        const above = route('above', '7'); select('7');
        await tick();
        finish(below, above);
        const scrims = [below, above].map((r) => getComputedStyle(r).boxShadow !== 'none').concat(getComputedStyle(document.getElementById('under')).visibility === 'visible');
        leave(above);
        select('6');
        await tick();
        const out = [above.isConnected, !above.isConnected ? 'gone' : above.compareDocumentPosition(below) & Node.DOCUMENT_POSITION_PRECEDING && getComputedStyle(above).zIndex === getComputedStyle(below).zIndex ? 'over' : 'under', scrims];
        finish(below, above); below.remove(); above.remove();
        select('1');
        return out;
      });
      expect([engine, reducedMotion, uncovered]).toEqual([engine, reducedMotion, reducedMotion === 'reduce' ? [false, 'gone', [false, true, true]] : [true, 'over', [false, true, true]]]);
      // A full-screen route it uncovers keeps it too; a sheet that replaces it in the same commit ends it; one closed as
      // it still rises turns back from where it is; one that holds media, a field's painted decorations, or an open
      // dialog or popover closes at once; and one that holds a native view slides, showing what its module left in it.
      const more = await page.evaluate(async () => {
        const settle = (...routes) => { finish(...routes); for (const r of routes) r.remove(); select('1'); };
        const full = route('full', '8', 'fullscreen'); select('8');
        const sheet = route('over-full', '9'); select('9');
        await tick();
        leave(sheet);
        select('8');
        await tick();
        const overFull = sheet.isConnected;
        settle(full, sheet);
        const first = route('first-of-two', '10'); select('10');
        await tick();
        // (A replace keeps the visit's key.)
        const second = route('second-of-two', '10');
        leave(first);
        select('10');
        await tick();
        const replaced = first.isConnected;
        settle(first, second);
        await tick();
        const rising = route('rising', '12'); select('12');
        leave(rising);
        select('1');
        const turned = rising.isConnected && (() => { for (const a of rising.getAnimations()) { a.pause(); a.currentTime = 0; } return Math.round(rising.getBoundingClientRect().top); })();
        settle(rising);
        const playing = route('playing', '13', 'modal', '<video></video>'); select('13');
        await tick();
        leave(playing);
        select('1');
        // (An open dialog or popover is in the top layer, where the slide would leave it frozen.)
        const alerting = route('alerting', '14', 'modal', '<dialog></dialog><div popover></div>'); select('14');
        await tick();
        alerting.querySelector('[popover]').showPopover();
        leave(alerting);
        select('1');
        const menuing = route('menuing', '15', 'modal', '<dialog></dialog>'); select('15');
        await tick();
        menuing.firstChild.showModal();
        leave(menuing);
        select('1');
        const topLayer = alerting.isConnected || menuing.isConnected;
        settle(alerting, menuing);
        const native = route('native', '16', 'modal', '<photo-editor></photo-editor>'); native.firstChild.exactNative = { id: 1 }; select('16');
        await tick();
        leave(native);
        const nativeSlides = native.isConnected;
        finish(native);
        select('1');
        // (Decorations are painted where a field gives its value's ranges, `createValueRange`; elsewhere it slides, as
        // any sheet does unless the person asks for less motion.)
        const decorated = route('decorated', '20', 'modal', '<textarea data-decorations="{}"></textarea>'); select('20');
        await tick();
        leave(decorated);
        select('1');
        const slides = decorated.isConnected === (!matchMedia('(prefers-reduced-motion: reduce)').matches && !('createValueRange' in HTMLTextAreaElement.prototype));
        finish(decorated);
        return [overFull, replaced, turned === false ? false : turned > 600, playing.isConnected, topLayer, nativeSlides, slides];
      });
      expect([engine, reducedMotion, more]).toEqual([engine, reducedMotion, reducedMotion === 'reduce' ? [false, false, false, false, false, false, true] : [true, false, true, false, false, true, true]]);
      const ended = await page.evaluate(async () => {
        const open = (id, key) => { const r = route(id, key); select(key); return r; };
        const first = open('first', '3');
        await tick();
        leave(first);
        select('1');
        const leftThen = first.isConnected;
        open('second', '4');
        await tick();
        return [leftThen, first.isConnected];
      });
      expect([engine, reducedMotion, ended]).toEqual([engine, reducedMotion, [reducedMotion !== 'reduce', false]]);
      // It keeps its ids while it leaves, for its own references, but a focus the same commit asks for by an id it shares
      // lands on the live route's: one that opens over it ends it at once, and an ordinary route it uncovers (which
      // does not end it) is found first. (A field outside the app's root with that id is none of the app's.)
      const ids = await page.evaluate(async () => {
        const field = (id, key, presentation = 'modal') => route(id, key, presentation, '<input id="title" aria-label="Title">');
        const outside = document.body.insertBefore(Object.assign(document.createElement('input'), { id: 'title' }), document.body.firstChild);
        const first = field('first-title', '17'); select('17');
        await tick();
        finish(first);
        leave(first);
        select('1');
        await tick();
        const again = field('again-title', '18'); select('18'); focusById('title');
        const reopened = [first.isConnected, document.activeElement.parentElement?.id];
        finish(again);
        document.activeElement.blur();
        const page = field('page-title', '19', '');
        leave(again);
        select('19'); focusById('title');
        const pushed = [again.isConnected, document.activeElement.parentElement?.id];
        finish(again);
        await tick();
        page.remove(); outside.remove(); select('1');
        return [reopened, pushed];
      });
      expect([engine, reducedMotion, ids]).toEqual([engine, reducedMotion, [[false, 'again-title'], [reducedMotion !== 'reduce', 'page-title']]]);
      // One that opens in another stack leaves it be.
      const elsewhere = await page.evaluate(async () => {
        const nav = document.getElementById('nav'), other = Object.assign(document.createElement('main'), { id: 'nav-2' });
        other.setAttribute('navigationKey', 'a'); other.setAttribute('navigationBack', 'back'); other.innerHTML = '<div navigationKey="a"></div>';
        document.getElementById('exact-root').append(other);
        const open = (parent, id, key) => { const r = route(id, key, 'modal', '', parent); parent.setAttribute('navigationKey', key); select(nav.getAttribute('navigationKey')); return r; };
        const mine = open(nav, 'mine', '14');
        await tick();
        leave(mine);
        select('1');
        open(other, 'theirs', 'b');
        await tick();
        const stays = mine.isConnected;
        finish(mine);
        other.remove(); mine.remove(); select('1');
        return stays;
      });
      expect([engine, reducedMotion, elsewhere]).toEqual([engine, reducedMotion, reducedMotion !== 'reduce']);
      // The wasm runtime asks too: its commit destroys the sheet and then what it holds (parent first, as the kernel
      // orders them), and lays out the stack without it, which passes over a leaving sheet.
      const wasm = await page.evaluate(async () => {
        const nav = document.getElementById('nav'), under = document.getElementById('under'), sheet = route('wasm-sheet', '15', 'modal', '<button type="button" id="wasm-inside">Add</button>');
        select('15');
        await tick();
        const inside = sheet.firstElementChild;
        for (const [id, el] of [[1, nav], [2, under], [3, sheet], [4, inside]]) glueViews.set(id, el);
        nav.setAttribute('navigationKey', '1');
        glueApply({ ops: [{ op: 'destroy', id: 3 }, { op: 'destroy', id: 4 }, { op: 'children', id: 1, ids: [2] }] });
        const out = [sheet.isConnected && sheet.hasAttribute('data-exiting'), inside.parentElement === sheet, [...nav.children].map((c) => c.id)];
        finish(sheet);
        await tick();
        return [...out, sheet.isConnected];
      });
      expect([engine, reducedMotion, wasm]).toEqual([engine, reducedMotion, reducedMotion === 'reduce' ? [false, true, ['under'], false] : [true, true, ['under', 'wasm-sheet'], false]]);
      // Under the agent's clock (glue.js's `at`): a sheet a commit at 100 ms closes starts then, so at 700 ms it is gone.
      const clocked = await page.evaluate(async () => {
        let t = 100;
        const clock = animationClock(() => t, () => null, () => {}), r = route('clocked', '5');
        select('5');
        await tick();
        clock.register(0);
        leave(r);
        select('1');
        clock.register(t);
        t = 700;
        clock.seek(t);
        await tick();
        return r.isConnected;
      });
      expect([engine, reducedMotion, clocked]).toEqual([engine, reducedMotion, false]);
      await context.close();
    }
  } finally {
    await browser.close();
  }
}, 90000);

// A bar's level-1 title takes at least the large title's line, 33 px at 1.2 (neutral.css): the bar keeps that height
// with a smaller title, a tighter line or no text, and the title stays centred beside a button. The sheet reads the
// role and level, not the tag: the JS target makes a level chosen between literals an `h2`.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a bar's level-1 title keeps the large title's line`, async () => {
  const browser = await launch(engine);
  try {
    const page = await browser.newPage({ viewport: { width: 600, height: 2400 }, colorScheme: 'light' });
    const title = (tag, level, style = 'font-size:27px', text = 'Inbox') => `<${tag} role="heading" aria-level="${level}" style="${style};font-weight:700">${text}</${tag}>`;
    const route = (id, header, attributes = '', before = '') => `<div navigationKey="${id}" id="${id}"${attributes}>${before}<header>${header}</header></div>`;
    // A row as a bar with a button lays it out (`display: flex`, centred). Each text's baseline is the bottom of the
    // empty inline-block after it.
    const mark = '<i style="display:inline-block"></i>';
    const row = (id, style) => `<div navigationKey="${id}" id="${id}"><header style="display:flex;align-items:center;gap:8px">${title('h1', 1, style, `Inbox${mark}`)}<button style="font-size:17px">Edit${mark}</button></header></div>`;
    const clamp = 'font-size:27px;width:80px;display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:1;overflow:hidden';
    const routes = [
      route('page', title('h1', 1)), route('computed', title('h2', 1)),
      route('small', title('h1', 1, 'font-size:17px')), route('tight', title('h1', 1, 'font-size:27px;line-height:1')),
      route('boxed', `<div>${title('h1', 1)}</div>`), route('empty', title('h1', 1, undefined, '')), route('space', title('h1', 1, undefined, ' ')),
      route('clamped', title('h1', 1, clamp, 'Inbox of every')), route('tall', title('h1', 1, 'font-size:27px;height:44px')),
      route('tapped', `<button style="display:flex;padding:0;border:0;background:none;font:inherit">${title('span', 1)}</button>`),
      route('own', title('h1', 1, 'font-size:27px;margin:0')), route('short', title('h1', 1, 'font-size:27px;height:20px;overflow:hidden')), route('second', title('h2', 2)), route('later', title('h1', 1), '', '<div></div>'),
      route('dialog', `<dialog open>${title('h1', 1)}</dialog>`), route('popover', `<div popover>${title('h1', 1)}</div>`),
      row('row-large', 'font-size:33px'), row('row-page', 'font-size:27px'), row('row-small', 'font-size:17px'),
    ];
    // Each presented route and the routes pushed after it are a stack of their own.
    const presented = ['modal', 'fullscreen'].map((p) => `<div>${route(p, title('h1', 1), ` navigationPresentation="${p}"`)}${route(`${p}-pushed`, title('h1', 1))}</div>`).join('')
      + `<div>${route('leaving', title('h1', 1), ' navigationPresentation="modal" data-exiting')}${route('after-leaving', title('h1', 1))}</div>`
      + `<div>${route('closing', title('h1', 1), ' navigationPresentation="modal" data-exiting')}${route('closing-pushed', title('h1', 1), ' data-exiting')}</div>`;
    // In standards mode, as the shell's page is.
    await page.setContent(`<!doctype html><style>${sheet()}</style><div id="exact-root"><main navigationKey="page">${routes.join('')}</main>
      ${presented}<div id="line" style="font-size:27px;font-weight:700">Inbox</div></div>`);
    // Each bar's height, its heading's height and top margin, and the centre of each cap height in the bar from its
    // top (a cap height is 0.7 of the size).
    const got = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll('[id] > header')].map((bar) => {
      const top = bar.getBoundingClientRect().top, heading = bar.querySelector('[role="heading"]'), round = (v) => Math.round(v * 10) / 10;
      const caps = [...bar.querySelectorAll('i')].map((i) => i.getBoundingClientRect().bottom - top - 0.35 * parseFloat(getComputedStyle(i).fontSize));
      return [bar.parentElement.id, { bar: round(bar.getBoundingClientRect().height), heading: round(heading.getBoundingClientRect().height), margin: getComputedStyle(heading).marginTop, caps }];
    })));
    const line = await page.evaluate(() => Math.round(document.getElementById('line').getBoundingClientRect().height * 10) / 10);
    // The shell's bar is 6 px of padding either side of its line and a 1 px edge; the large title's line is 33 px at 1.2.
    const large = Math.round((33 * 1.2 + 13) * 10) / 10;
    for (const id of ['page', 'computed', 'small', 'tight', 'boxed', 'empty', 'space', 'clamped', 'tapped', 'after-leaving', 'row-page', 'row-small']) expect([engine, id, got[id].bar]).toEqual([engine, id, large]);
    // Beside a button the title's caps stay centred on the button's, within the pixel a 33 px title's are.
    for (const id of ['row-large', 'row-page', 'row-small']) expect([engine, id, Math.abs(got[id].caps[0] - got[id].caps[1]) <= 1]).toEqual([engine, id, true]);
    // The heading's box is its own but at least a line: one clamped line, the author's 44 px, a line for 20 px.
    expect([engine, got.clamped.heading, got.tall.heading, got.short.heading]).toEqual([engine, line, 44, line]);
    // The rest are the shell's bar around one line of their own.
    for (const id of ['own', 'second', 'later', 'modal', 'modal-pushed', 'fullscreen', 'fullscreen-pushed', 'closing-pushed']) expect([engine, id, got[id].bar]).toEqual([engine, id, line + 13]);
    expect([engine, got.dialog.margin, got.popover.margin]).toEqual([engine, '0px', '0px']);
    // At a 20 px root, with the size in `rem` as the JS target writes `title1`, the line is the large title's there.
    await page.setContent(`<!doctype html><style>${sheet()} html { font-size: 20px; }</style><div id="exact-root"><main navigationKey="page">
      ${route('page', title('h1', 1, 'font-size:1.6875rem'))}</main></div>`);
    expect([engine, await page.evaluate(() => Math.round(document.querySelector('#page > header').getBoundingClientRect().height * 10) / 10)]).toEqual([engine, Math.round((2.0625 * 20 * 1.2 + 13) * 10) / 10]);
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
    await page.setContent(`<style>${sheet()}</style><div id="exact-root">${button('unsized', '')}${button('sized', 'width:84px;height:24px', 'width height')}${button('square', 'width:28px;height:28px', 'width height')}${button('short', 'height:24px', 'height')}${button('unsaid', 'width:28px;height:28px')}</div><script>${helpers}</script>`);
    const got = await page.evaluate(() => ({ unsized: box('unsized')[3], sized: box('sized').slice(2), square: box('square').slice(2), short: box('short')[3], unsaid: box('unsaid').slice(2) }));
    expect(got).toEqual({ unsized: 36, sized: [84, 24], square: [28, 28], short: 24, unsaid: [36, 36] }); // `unsaid`: the floor, were the host not to say
  } finally { await browser.close(); }
}, 90000);

// LLP 1116.020 D2: a sheet's bar says the name of a button whose face is only a symbol. A width its author wrote for
// the symbol, inline (the wasm runtime) or in a class (the JS target), leaves the word its padding and its wash, as
// UIKit sizes a bar item to its title: 16 px either side of the word, inside the bar, in either direction. Two long
// names in a narrow bar both stay in it, each word ending in an ellipsis.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: a sheet's bar word keeps its padding whatever width its author wrote`, async () => {
  const browser = await launch(engine);
  try {
    const page = await browser.newPage({ viewport: { width: 400, height: 300 }, colorScheme: 'light' });
    const word = (id, attributes) => `<button type="button" data-native data-button-style="bordered" aria-label="Cancel" id="${id}" ${attributes}><img data-symbol-source="symbol:sf/xmark" alt=""></button>`;
    await page.setContent(`<!doctype html><style>${sheet()} #exact-root#exact-root .sized { width: 44px; height: 44px; display: flex; }</style><div id="exact-root">
      <div navigationKey="1" navigationPresentation="modal"><header id="bar" style="display:flex;align-items:center"><div style="flex:1;min-width:0"><h2>Last Parked</h2></div>
      ${word('inline', 'data-exact-said="width height" style="width:44px;height:44px;display:flex"')}${word('class', 'data-exact-said="width height" class="sized"')}${word('unsized', 'style="display:grid"')}</header></div>
    </div><script>${helpers}</script>`);
    // Its padding, the room its box gives beyond its word, its height, and whether it is inside the bar's padding.
    const fit = (id) => page.evaluate((id) => {
      const [x, , width, height] = box(id), [bar, , barWidth] = box('bar');
      return [[css(id, 'paddingInlineStart'), css(id, 'paddingInlineEnd')], Math.round(width - parseFloat(css(id, 'width', '::after'))), height, x >= bar + 16 && x + width <= bar + barWidth - 16];
    }, id);
    // A pixel in each of its paddings, half way down.
    const pixels = async (id) => {
      const { width, height, data } = decodePng(await page.locator(`#${id}`).screenshot());
      return [4, width - 5].map((x) => [...data.subarray((Math.floor(height / 2) * width + x) * 4, (Math.floor(height / 2) * width + x) * 4 + 3)]);
    };
    for (const dir of ['rtl', 'ltr']) {
      await page.evaluate((dir) => { document.documentElement.dir = dir; }, dir);
      for (const id of ['inline', 'class', 'unsized']) {
        const at = [engine, dir, id];
        expect([at, await fit(id)]).toEqual([at, [['16px', '16px'], 34, id === 'unsized' ? 36 : 44, true]]);
        // The pointer's wash fills the padding on both sides: a pixel in each matches the other and not the ground.
        const [ground] = await pixels(id);
        await page.hover(`#${id}`);
        const [start, end] = await pixels(id);
        expect([at, start, end.join() === start.join(), start.join() !== ground.join()]).toEqual([at, start, true, true]);
        await page.mouse.move(1, 299);
      }
    }
    await page.evaluate(() => { const b = document.getElementById('class'); b.removeAttribute('data-exact-said'); b.setAttribute('data-exact-width', ''); b.setAttribute('data-exact-height', ''); });
    expect([engine, 'bound width', await fit('class')]).toEqual([engine, 'bound width', [['16px', '16px'], 34, 44, true]]);
    await page.setViewportSize({ width: 320, height: 300 });
    await page.evaluate(() => { for (const [id, name] of [['inline', 'Abbrechen und verwerfen'], ['class', 'Speichern und schließen']]) document.getElementById(id).setAttribute('aria-label', name); document.getElementById('unsized').remove(); document.getElementById('inline').after(document.querySelector('#bar > div')); });
    const narrow = [await fit('inline'), await fit('class'), await page.evaluate(() => [box('inline')[0] + box('inline')[2] <= box('class')[0], css('inline', 'textOverflow', '::after')])];
    expect([engine, 'narrow', narrow]).toEqual([engine, 'narrow', [[['16px', '16px'], 34, 44, true], [['16px', '16px'], 34, 44, true], [true, 'ellipsis']]]);
    // A button its author made a block keeps the word a block of its own, which `text-overflow` ends.
    await page.evaluate(() => { document.getElementById('inline').style.display = 'block'; });
    expect([engine, 'narrow block', await fit('inline'), await page.evaluate(() => css('inline', 'display', '::after'))]).toEqual([engine, 'narrow block', [['16px', '16px'], 34, 44, true], 'block']);
  } finally { await browser.close(); }
}, 90000);

// A part colour's alpha is its only opacity. Removing it restores the look's default.
for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: authored placeholder colour and reset in both schemes`, async () => {
  const browser = await launch(engine);
  try {
    for (const scheme of ['light', 'dark']) {
      const page = await browser.newPage({ colorScheme: scheme });
      await page.setContent(`<style>${sheet()}</style><div id="exact-root">
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
        expect([scheme, await sample()]).toEqual([scheme, [[color, '1'], [color, '1']]]);
        await page.evaluate(() => {
          for (const el of document.querySelectorAll('input, textarea')) el.disabled = true;
        });
        expect([scheme, 'disabled', await sample()]).toEqual([scheme, 'disabled', [[color, '1'], [color, '1']]]);
        await page.evaluate(() => {
          for (const el of document.querySelectorAll('input, textarea')) el.disabled = false;
        });
      }
      await page.evaluate(() => {
        for (const el of document.querySelectorAll('input, textarea')) el.removeAttribute('style');
      });
      expect([scheme, await sample()]).toEqual([scheme, original]);
      await page.close();
    }
  } finally { await browser.close(); }
}, 90000);
