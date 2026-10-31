// Test symbol updates with a minimal DOM, then verify their computed styles in Chrome.
import { afterAll, test, expect } from 'bun:test';
import { copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';

// symbols.js beside stand-ins for the runtime and the picker glue it loads.
const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-symbols-'));
afterAll(() => rmSync(dir, { recursive: true, force: true }));
copyFileSync(resolve(new URL('../../web-js/symbols.js', import.meta.url).pathname), resolve(dir, 'symbols.js'));
writeFileSync(resolve(dir, 'rt.js'), 'export const { After, PropHooks, Hooks, onEnd, inflight, journal, clock, data } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'picker-glue.js'), '');
const After = [], PropHooks = {}, inflight = { n: 0 };
globalThis.rtStandIn = { After, PropHooks, Hooks: {}, onEnd() {}, inflight, journal: [], clock: { now: 0 }, data: { appId: 'com.example' } };

class Img {
  constructor() { this.localName = 'img'; this.isConnected = true; this.attrs = new Map(); this.props = new Map(); this.classList = [];
    this.style = { setProperty: (k, v) => this.props.set(k, v), removeProperty: (k) => this.props.delete(k), getPropertyValue: (k) => this.props.get(k) ?? '' }; }
  setAttribute(k, v) { this.attrs.set(k, String(v)); }
  getAttribute(k) { return this.attrs.get(k) ?? null; }
  hasAttribute(k) { return this.attrs.has(k); }
  removeAttribute(k) { this.attrs.delete(k); }
  toggleAttribute(k, on) { if (on) this.attrs.set(k, ''); else this.attrs.delete(k); }
  set src(v) { this.setAttribute('src', v); }
  get alt() { return this.getAttribute('alt') ?? ''; }
  set alt(v) { this.setAttribute('alt', v); }
  get clientWidth() { return 24; } get clientHeight() { return 24; }
}
const el = new Img();
globalThis.document = {
  head: { append() {} }, createElement: () => ({}), styleSheets: [],
  getElementById: () => ({}),
  querySelectorAll: (sel) => (sel.includes('data-symbol-path') ? el.hasAttribute('data-symbol-path') : sel.includes('data-app-src') ? el.hasAttribute('data-app-src') : el.hasAttribute('data-symbol-source')) ? [el] : [],
};
globalThis.getComputedStyle = () => ({ fontSize: '17px', fontWeight: '400', objectFit: 'contain', paddingLeft: '0', paddingRight: '0', paddingTop: '0', paddingBottom: '0', getPropertyValue: () => '' });
globalThis.exact = { appURL: async (path) => `blob:test/${path}` };
globalThis.window = {};
globalThis.matchMedia = () => ({ addEventListener() {} });
globalThis.MutationObserver = class { observe() {} };
async function refreshSymbols() {
  for (const f of After) f();
  await Promise.resolve();
}

test('an image that leaves a symbol for an app:/ file keeps the file', async () => {
  const { symbols } = await import(resolve(dir, 'symbols.js'));
  symbols({ photo: ['M0 0L24 24', false] });
  expect(PropHooks.src(el, 'symbol:photo')).toBe(true);
  await refreshSymbols();
  expect(el.getAttribute('data-symbol-path')).toBe('M0 0L24 24');
  expect(el.getAttribute('src')).toStartWith('data:image/svg+xml');
  expect(PropHooks.src(el, 'app:/data/photo.png')).toBe(true);
  // The symbol is gone before the file lands.
  for (const a of ['data-symbol-path', 'data-symbol-source', 'data-symbol-fill']) expect(el.hasAttribute(a)).toBe(false);
  expect(el.style.getPropertyValue('--exact-symbol-mask')).toBe('');
  while (inflight.n) await Bun.sleep(1);
  expect(el.getAttribute('src')).toBe('blob:test/app:/data/photo.png');
  // A later commit's refresh leaves it there.
  await refreshSymbols();
  expect(el.getAttribute('src')).toBe('blob:test/app:/data/photo.png');
});

test('a symbol is decorative unless its author named it', async () => {
  const { symbols } = await import(resolve(dir, 'symbols.js'));
  symbols({ photo: ['M0 0L24 24', false] });
  const unnamed = new Img(), named = new Img();
  named.setAttribute('alt', 'Add a stop'); // the template's alt, from `alt` or `aria-label`
  expect(PropHooks.src(unnamed, 'symbol:photo')).toBe(true);
  expect(PropHooks.src(named, 'symbol:photo')).toBe(true);
  expect(unnamed.getAttribute('alt')).toBe('');
  expect(named.getAttribute('alt')).toBe('Add a stop');
});

// LLP 1011 §4: under `fill`, CSS's initial `object-fit`, a symbol keeps its
// aspect in a box of another shape, as on the Apple hosts; `none` draws it at
// its size.
test('a symbol keeps its aspect under object-fit: fill', async () => {
  const { symbols } = await import(resolve(dir, 'symbols.js'));
  symbols({ chevron: ['M8 6l6 6-6 6', false] });
  const was = globalThis.getComputedStyle, query = document.querySelectorAll;
  try {
    for (const [fit, size] of [['fill', 'contain'], ['cover', 'cover'], ['none', '17px 17px']]) {
      globalThis.getComputedStyle = () => ({ fontSize: '17px', fontWeight: '400', objectFit: fit, paddingLeft: '0', paddingRight: '0', paddingTop: '0', paddingBottom: '0', getPropertyValue: () => '' });
      const wide = new Img();
      document.querySelectorAll = (sel) => sel.includes('data-symbol-path') && wide.hasAttribute('data-symbol-path') ? [wide] : [];
      expect(PropHooks.src(wide, 'symbol:chevron')).toBe(true);
      await refreshSymbols();
      expect(wide.style.getPropertyValue('--exact-symbol-fit')).toBe(size);
    }
  } finally { globalThis.getComputedStyle = was; document.querySelectorAll = query; }
});

// The emitted entry installs symbols before mounting, then projects navigation.
// Use the real consumers and stylesheet: no exactViewport resource commits on resize.
test('symbol axes follow the current projected title and root font without another action', async () => {
  const root = resolve(import.meta.dir, '../../..');
  const axes = { auto: '', fixed: '--exact-symbol-size-authored:1;--exact-symbol-weight-authored:1;font-size:23px;font-weight:800;',
    size: '--exact-symbol-size-authored:1;font-size:23px;', weight: '--exact-symbol-weight-authored:1;font-weight:800;' };
  const tabs = Object.entries(axes).map(([id, style]) => `<button id="tab-${id}" role="tab" aria-controls="panel-${id}" aria-selected="${id === 'auto'}" data-button-style="bordered" style="display:grid;flex:1">
    <img id="${id}" data-symbol-source="symbol:book" data-symbol-path="M0 0L24 24" style="${style}"><span id="title-${id}" data-exact-text>Library</span></button>`).join('');
  const panels = Object.keys(axes).map(id => `<div id="panel-${id}" role="tabpanel"><div navigationkey="${id}">Panel</div></div>`).join('');
  const content = `<div style="height:400px;display:flex;flex-direction:column" navigationback="back" navigationkey="auto" id="nav">
    <div style="flex:1">${panels}</div><div role="tablist" style="display:flex;flex-direction:row">${tabs}</div></div>
    <button id="larger">Root 125%</button><button id="reset">Root medium</button><button id="title">Title 30/600</button><button id="inherit">Inherited title</button>`;
  const boot = `import { symbols } from '/symbols.js';
    import { After, Hooks, commit, C, sig, W, S } from '/rt.js';
    import { projectRoots } from '/document.js';
    import { navigation } from '/navigation.js';
    const size = sig(null), weight = sig(null);
    for (const e of document.querySelectorAll('[data-exact-text]')) {
      S(e, 'font-size', 'px', () => size()); S(e, 'font-weight', '', () => weight());
    }
    for (const image of document.querySelectorAll('img[data-symbol-source]')) Hooks.image(image);
    symbols({ book: ['M0 0L24 24', false] });
    commit(() => {});
    projectRoots(navigation, () => false, console.error, After);
    document.getElementById('larger').onclick = () => commit(() => C('setRootFontSize', ['125%']));
    document.getElementById('reset').onclick = () => commit(() => C('setRootFontSize', ['medium']));
    document.getElementById('title').onclick = () => commit(() => { W(size, 30); W(weight, 600); });
    document.getElementById('inherit').onclick = () => commit(() => { W(size, null); W(weight, null); });
    globalThis.ready = true;`;
  const html = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script type="module">${boot}</script>`);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname.slice(1);
    if (!path) return new Response(html, { headers: { 'content-type': 'text/html' } });
    // Unbundled rt.js re-exports perf.js, which reads this generated table.
    if (path === 'names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (!/^[A-Za-z0-9_.-]+\.js$/.test(path)) return new Response('Not found', { status: 404 });
    const file = [resolve(root, 'host/web-js', path), resolve(root, 'host/web', path)].find(existsSync);
    return file ? new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } }) : new Response('Not found', { status: 404 });
  } });
  let browser, stage = 'launch';
  const report = (kind, value) => console.log('symbol diagnostic', JSON.stringify({ stage, kind, value }));
  try {
    browser = await suiteBrowser('chromium', { timeout: 10000 });
    stage = 'page'; report('stage', stage);
    const page = await browser.newPage({ viewport: { width: 1280, height: 700 } });
    page.setDefaultTimeout(5000);
    const errors = [], observed = [], expected = [];
    page.on('pageerror', error => { errors.push(String(error)); report('pageerror', String(error)); });
    page.on('requestfailed', request => report('requestfailed', [request.url(), request.failure()]));
    page.on('response', response => { if (response.status() >= 400) report('http', [response.url(), response.status()]); });
    stage = 'navigation'; report('stage', stage);
    await page.goto(server.url.href);
    stage = 'ready'; report('stage', stage);
    await page.waitForFunction(() => globalThis.ready);
    async function sample(name, titleSize, titleWeight, sidebar) {
      const value = await page.evaluate(() => ({
        sidebar: document.getElementById('nav').hasAttribute('data-exact-sidebar'),
        images: ['auto', 'fixed', 'size', 'weight'].map(id => {
          const e = document.getElementById(id), title = document.getElementById('title-' + id);
          const css = getComputedStyle(e), text = getComputedStyle(title);
          const svg = new DOMParser().parseFromString(decodeURIComponent(e.getAttribute('src').split(',')[1]), 'image/svg+xml').documentElement;
          return [id, parseFloat(css.fontSize), Number(css.fontWeight), Number(svg.getAttribute('width')), parseFloat(text.fontSize), Number(text.fontWeight)];
        }),
      }));
      observed.push({ name, ...value });
      report('sample', observed.at(-1));
      expected.push({ name, sidebar, images: Object.keys(axes).map(id => [id,
        ['fixed', 'size'].includes(id) ? 23 : titleSize, ['fixed', 'weight'].includes(id) ? 800 : titleWeight,
        ['fixed', 'size'].includes(id) ? 23 : titleSize, titleSize, titleWeight]) });
    }
    async function click(id) { stage = id; report('stage', stage); await page.locator('#' + id).click(); }
    stage = 'initial-wide';
    await sample('initial-wide', 16, 500, true);
    await click('larger'); await sample('root125', 20, 500, true);
    stage = 'resize899'; report('stage', stage);
    await page.setViewportSize({ width: 899, height: 700 });
    await page.waitForFunction(() => !document.getElementById('nav').hasAttribute('data-exact-sidebar'));
    await sample('narrow125', 15, 500, false);
    stage = 'resize900'; report('stage', stage);
    await page.setViewportSize({ width: 900, height: 700 });
    await page.waitForFunction(() => document.getElementById('nav').hasAttribute('data-exact-sidebar'));
    await sample('wide125', 20, 500, true);
    await click('title'); await sample('own-title', 30, 600, true);
    await click('reset'); await sample('root-medium-own-title', 30, 600, true);
    await click('inherit'); await sample('inherited-again', 16, 500, true);
    await click('larger'); await sample('root125-again', 20, 500, true);
    await click('reset'); await sample('root-medium', 16, 500, true);
    expect(errors).toEqual([]);
    expect(observed).toEqual(expected);
  } catch (error) { report('failure', String(error)); throw error;
  } finally {
    stage = 'close'; report('stage', stage);
    try { await browser?.close(); } finally { server.stop(true); }
  }
}, 30000);


// LLP 1121 stage 9 B (D10): under a bound `color` the JS target decides after
// each commit whether a symbol draws in its colour (style.rs `symbol_marks`,
// symbols.js `inked`). Since LLP 1081 stage 2 (317ddfea6a) every page symbol
// draws in its `color`, as the Apple hosts do, written or not: a conditional
// colour that goes leaves the colour past it, one that comes back takes it again.
test('a symbol under a bound colour follows whether it has a value', async () => {
  const root = resolve(import.meta.dir, '../../..');
  const img = (id, ink) => `<img id="${id}" data-symbol-source="symbol:book" data-symbol-path="M0 0L24 24" data-symbol-ink="${ink}" style="width:16px;height:16px">`;
  const content = `<div style="--exact-accent:rgb(102, 82, 197)">
    <div id="bound" data-exact-ink="1">${img('alone', '10')}</div>
    <div style="color:rgb(0, 128, 0)"><div id="inner" data-exact-ink="1">${img('over', '11')}</div></div>
    <div id="implied" data-exact-ink="0">${img('unwritten', '10')}</div></div>
    <button id="off">Off</button><button id="on">On</button>`;
  const boot = `import { symbols } from '/symbols.js';
    import { Hooks, commit, sig, W, S } from '/rt.js';
    const c = sig('rgb(255, 0, 0)');
    for (const id of ['bound', 'inner', 'implied']) S(document.getElementById(id), 'color', '', () => c());
    for (const image of document.querySelectorAll('img[data-symbol-source]')) Hooks.image(image);
    symbols({ book: ['M0 0L24 24', false] });
    commit(() => {});
    document.getElementById('off').onclick = () => commit(() => W(c, null));
    document.getElementById('on').onclick = () => commit(() => W(c, 'rgb(255, 0, 0)'));
    globalThis.ready = true;`;
  const html = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script type="module">${boot}</script>`);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname.slice(1);
    if (!path) return new Response(html, { headers: { 'content-type': 'text/html' } });
    if (path === 'names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (!/^[A-Za-z0-9_.-]+\.js$/.test(path)) return new Response('Not found', { status: 404 });
    const file = [resolve(root, 'host/web-js', path), resolve(root, 'host/web', path)].find(existsSync);
    return file ? new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } }) : new Response('Not found', { status: 404 });
  } });
  let browser;
  try {
    browser = await suiteBrowser('chromium', { timeout: 10000 });
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.ready);
    // Each symbol's ink: its own `color` (named so), else the colour it shows.
    const ink = () => page.evaluate(() => new Promise(done => requestAnimationFrame(() => done(Object.fromEntries(['alone', 'over', 'unwritten']
      .map(id => { const s = getComputedStyle(document.getElementById(id)); return [id, [s.backgroundColor === s.color ? 'colour' : s.backgroundColor, s.color]]; }))))));
    const red = 'rgb(255, 0, 0)', green = 'rgb(0, 128, 0)';
    expect(await ink()).toEqual({ alone: ['colour', red], over: ['colour', red], unwritten: ['colour', red] });
    await page.locator('#off').click();
    const off = await ink();
    expect(off.over).toEqual(['colour', green]);
    expect([off.alone[0], off.unwritten[0]]).toEqual(['colour', 'colour']);
    expect(off.alone[1]).not.toBe(red);
    await page.locator('#on').click();
    expect(await ink()).toEqual({ alone: ['colour', red], over: ['colour', red], unwritten: ['colour', red] });
    expect(errors).toEqual([]);
  } finally {
    try { await browser?.close(); } finally { server.stop(true); }
  }
}, 30000);

// A symbol on a button's face draws in its title's colour, which the colour
// scheme changes without a commit: it follows, as on the web host (glue.js
// `mediaChanged`).
test('a symbol on a button follows its title across a colour scheme change', async () => {
  const root = resolve(import.meta.dir, '../../..');
  const content = `<button type="button" data-native data-button-style="bordered" style="display:grid;color:light-dark(rgb(1, 2, 3), rgb(4, 5, 6))">
    <img id="symbol" data-symbol-source="symbol:book" data-symbol-path="M0 0L24 24"><span data-exact-text>Library</span></button>`;
  const boot = `import { symbols } from '/symbols.js';
    import { Hooks, commit } from '/rt.js';
    for (const image of document.querySelectorAll('img[data-symbol-source]')) Hooks.image(image);
    symbols({ book: ['M0 0L24 24', false] });
    commit(() => {});
    globalThis.ready = true;`;
  const html = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script type="module">${boot}</script>`);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname.slice(1);
    if (!path) return new Response(html, { headers: { 'content-type': 'text/html' } });
    if (path === 'names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (!/^[A-Za-z0-9_.-]+\.js$/.test(path)) return new Response('Not found', { status: 404 });
    const file = [resolve(root, 'host/web-js', path), resolve(root, 'host/web', path)].find(existsSync);
    return file ? new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } }) : new Response('Not found', { status: 404 });
  } });
  let browser;
  try {
    browser = await suiteBrowser('chromium', { timeout: 10000 });
    const page = await browser.newPage({ colorScheme: 'light' });
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.ready);
    const ink = () => page.evaluate(() => new Promise(done => requestAnimationFrame(() => done(getComputedStyle(document.getElementById('symbol')).backgroundColor))));
    expect(await ink()).toBe('rgb(1, 2, 3)');
    await page.emulateMedia({ colorScheme: 'dark' });
    // The media query's `change` comes after the emulation's reply.
    await page.waitForFunction(() => getComputedStyle(document.getElementById('symbol')).backgroundColor === 'rgb(4, 5, 6)', null, { timeout: 2000 });
    expect(errors).toEqual([]);
  } finally {
    try { await browser?.close(); } finally { server.stop(true); }
  }
}, 30000);

// The same with the app's modes on (modes.js): a mode boundary (`data-x-b`) under a
// token colour writes it again as its implied row, no author's, and a symbol under it
// draws in that colour (LLP 1081 stage 2), as does one under a boundary whose own bound
// colour clears (the implied row stands there, as the runner installs it). The parent's
// own colour differs from the mode's row, so inheritance alone would show blue. A bound colour on a boundary
// (`Sx`) otherwise keeps its binding's provenance (`$ink`, rows.rs), not what the
// boundary leaves inline: with no token above, a clear is said; `none` gives the
// decision past it back; a value is said; a refused value is a clear.
test('a symbol under a mode boundary follows its implied row and its binding', async () => {
  const root = resolve(import.meta.dir, '../../..');
  const img = (id, attr) => `<img id="${id}" data-symbol-source="symbol:book" data-symbol-path="M0 0L24 24" ${attr} style="width:16px;height:16px">`;
  const content = `<div style="--exact-accent:rgb(102, 82, 197)">
    <div class="c1" style="color:rgb(0, 0, 255)">${img('beside', 'data-symbol-color')}
      <div id="pin" data-x-b="0" data-exact-ink="b">${img('under', 'data-symbol-ink="11"')}</div>
      <div id="bound" data-x-b="0" data-exact-ink="1b">${img('cleared', 'data-symbol-ink="11"')}</div></div>
    <div id="bare" data-x-b="0" data-exact-ink="1b">${img('alone', 'data-symbol-ink="10"')}</div></div>
    <button id="none">None</button><button id="red">Red</button><button id="bad">Bad</button><button id="paint">Paint</button>`;
  const boot = `import { symbols } from '/symbols.js';
    import { modes, Sx } from '/modes.js';
    import { Hooks, commit, sig, W, S } from '/rt.js';
    modes([], { c1: [0] }, ['color:rgb(0, 128, 0)'], {}, [], {});
    const c = sig('inherit');
    for (const id of ['bound', 'bare']) { const e = document.getElementById(id); Sx(S, e, 'color', '', () => { const v = c(); e.$ink = v != null; return v; }, 0); }
    for (const image of document.querySelectorAll('img[data-symbol-source]')) Hooks.image(image);
    symbols({ book: ['M0 0L24 24', false] });
    commit(() => {});
    document.getElementById('none').onclick = () => commit(() => W(c, null));
    document.getElementById('red').onclick = () => commit(() => W(c, 'rgb(255, 0, 0)'));
    document.getElementById('bad').onclick = () => commit(() => W(c, 'not a colour'));
    document.getElementById('paint').onclick = () => commit(() => W(c, document.getElementById('paint').dataset.v));
    globalThis.ready = true;`;
  const html = withLook(readFileSync(resolve(root, 'host/web/index.html'), 'utf8'))
    .replace('<div id="exact-root"></div>', `<div id="exact-root">${content}</div>`)
    .replace('<script type="module" src="./glue.js"></script>', `<script type="module">${boot}</script>`);
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname.slice(1);
    if (!path) return new Response(html, { headers: { 'content-type': 'text/html' } });
    if (path === 'names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (!/^[A-Za-z0-9_.-]+\.js$/.test(path)) return new Response('Not found', { status: 404 });
    const file = [resolve(root, 'host/web-js', path), resolve(root, 'host/web', path)].find(existsSync);
    return file ? new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } }) : new Response('Not found', { status: 404 });
  } });
  let browser;
  try {
    browser = await suiteBrowser('chromium', { timeout: 10000 });
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.ready);
    // Each symbol's ink, and the colour it would take (its own `color`).
    const ink = () => page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(() => done(Object.fromEntries(['beside', 'under', 'cleared', 'alone']
      .map(id => { const s = getComputedStyle(document.getElementById(id)); return [id, s.backgroundColor === s.color ? 'colour' : s.backgroundColor]; })))))));
    // Every symbol draws in its `color` since LLP 1081 stage 2 (317ddfea6a): what the binding and the implied
    // row decide is that colour, read below.
    const all = { beside: 'colour', under: 'colour', cleared: 'colour', alone: 'colour' };
    const colour = id => page.evaluate(id => getComputedStyle(document.getElementById(id)).color, id);
    const green = 'rgb(0, 128, 0)';
    expect(await ink()).toEqual(all);
    expect([await colour('beside'), await colour('under'), await colour('cleared')]).toEqual(['rgb(0, 0, 255)', green, green]);
    await page.locator('#none').click();
    expect(await ink()).toEqual(all);
    expect(await colour('cleared')).toBe(green);
    await page.locator('#red').click();
    expect(await ink()).toEqual(all);
    expect(await colour('cleared')).toBe('rgb(255, 0, 0)');
    // A value the row refuses is unset as a clear is: the implied row stands.
    await page.locator('#bad').click();
    expect(await colour('cleared')).toBe(green);
    // One clear or refusal after another keeps the implied row; a value is said
    // (the kernel's, modes_tests.rs `a_symbol_under_a_mode_boundary_…`).
    for (const [v, want] of [['inherit', green], ['unset', green], ['currentcolor', green], ['nope', green], ['nada', green], ['rgb(255, 0, 0)', 'rgb(255, 0, 0)'], ['inherit', green]]) {
      await page.evaluate(v => { document.getElementById('paint').dataset.v = v; }, v);
      await page.locator('#paint').click();
      expect([v, (await ink()).cleared, await colour('cleared')]).toEqual([v, 'colour', want]);
    }
    expect(errors).toEqual([]);
  } finally {
    try { await browser?.close(); } finally { server.stop(true); }
  }
}, 30000);
