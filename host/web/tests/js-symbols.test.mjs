// The JS target's image source hook (host/web-js/symbols.js): an image that
// showed a symbol and then shows an `app:/` file shows the file, and no
// later refresh paints the symbol back (Astra's batch 2 review). The DOM is
// a small fake; the runtime and the picker glue are stand-ins.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

// symbols.js beside stand-ins for the runtime and the picker glue it loads.
const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-symbols-'));
copyFileSync(resolve(new URL('../../web-js/symbols.js', import.meta.url).pathname), resolve(dir, 'symbols.js'));
writeFileSync(resolve(dir, 'rt.js'), 'export const { After, PropHooks, inflight, journal, clock, data } = globalThis.rtStandIn;\n');
writeFileSync(resolve(dir, 'picker-glue.js'), '');
const After = [], PropHooks = {}, inflight = { n: 0 };
globalThis.rtStandIn = { After, PropHooks, inflight, journal: [], clock: { now: 0 }, data: { appId: 'com.example' } };

class Img {
  constructor() { this.localName = 'img'; this.attrs = new Map(); this.props = new Map(); this.classList = []; this.alt = 'x';
    this.style = { setProperty: (k, v) => this.props.set(k, v), removeProperty: (k) => this.props.delete(k), getPropertyValue: (k) => this.props.get(k) ?? '' }; }
  setAttribute(k, v) { this.attrs.set(k, String(v)); }
  getAttribute(k) { return this.attrs.get(k) ?? null; }
  hasAttribute(k) { return this.attrs.has(k); }
  removeAttribute(k) { this.attrs.delete(k); }
  toggleAttribute(k, on) { if (on) this.attrs.set(k, ''); else this.attrs.delete(k); }
  set src(v) { this.setAttribute('src', v); }
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
