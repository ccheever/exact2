// The JS target's image source hook (host/web-js/symbols.js): an image that
// showed a symbol and then shows an `app:/` file shows the file, and no
// later refresh paints the symbol back (Astra's batch 2 review). The DOM is
// a small fake; the runtime and the picker glue are stand-ins.
import { test, expect } from 'bun:test';
import { copyFileSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';

// symbols.js beside stand-ins for the runtime and the picker glue it loads.
const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-symbols-'));
copyFileSync(resolve(new URL('../../web-js/symbols.js', import.meta.url).pathname), resolve(dir, 'symbols.js'));
writeFileSync(resolve(dir, 'rt.js'), 'export const { After, PropHooks, inflight, journal, clock, data } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'picker-glue.js'), '');
const After = [], PropHooks = {}, inflight = { n: 0 };
globalThis.rtStandIn = { After, PropHooks, inflight, journal: [], clock: { now: 0 }, data: { appId: 'com.example' } };

class Img {
  constructor() { this.localName = 'img'; this.attrs = new Map(); this.props = new Map(); this.classList = [];
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
  querySelectorAll: (sel) => (sel.includes('data-symbol-path') ? el.hasAttribute('data-symbol-path') : sel.includes('data-app-src') ? el.hasAttribute('data-app-src') : el.hasAttribute('data-symbol-source')) ? [el] : [],
};
globalThis.getComputedStyle = () => ({ fontSize: '17px', fontWeight: '400', objectFit: 'contain', paddingLeft: '0', paddingRight: '0', paddingTop: '0', paddingBottom: '0' });
globalThis.exact = { appURL: async (path) => `blob:test/${path}` };

test('an image that leaves a symbol for an app:/ file keeps the file', async () => {
  const { symbols } = await import(resolve(dir, 'symbols.js'));
  symbols({ photo: ['M0 0L24 24', false] });
  expect(PropHooks.src(el, 'symbol:photo')).toBe(true);
  for (const f of After) f();
  expect(el.getAttribute('data-symbol-path')).toBe('M0 0L24 24');
  expect(el.getAttribute('src')).toStartWith('data:image/svg+xml');
  expect(PropHooks.src(el, 'app:/data/photo.png')).toBe(true);
  // The symbol is gone before the file lands.
  for (const a of ['data-symbol-path', 'data-symbol-source', 'data-symbol-fill']) expect(el.hasAttribute(a)).toBe(false);
  expect(el.style.getPropertyValue('--exact-symbol-mask')).toBe('');
  while (inflight.n) await Bun.sleep(1);
  expect(el.getAttribute('src')).toBe('blob:test/app:/data/photo.png');
  // A later commit's refresh leaves it there.
  for (const f of After) f();
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
      globalThis.getComputedStyle = () => ({ fontSize: '17px', fontWeight: '400', objectFit: fit, paddingLeft: '0', paddingRight: '0', paddingTop: '0', paddingBottom: '0' });
      const wide = new Img();
      document.querySelectorAll = (sel) => sel.includes('data-symbol-path') && wide.hasAttribute('data-symbol-path') ? [wide] : [];
      expect(PropHooks.src(wide, 'symbol:chevron')).toBe(true);
      for (const f of After) f();
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
    import { After, commit, C, sig, W, S } from '/rt.js';
    import { projectRoots } from '/document.js';
    import { navigation } from '/navigation.js';
    const size = sig(null), weight = sig(null);
    for (const e of document.querySelectorAll('[data-exact-text]')) {
      S(e, 'font-size', 'px', () => size()); S(e, 'font-weight', '', () => weight());
    }
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
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true, timeout: 10000 });
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

