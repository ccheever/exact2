// The JS target's `press` on a link (host/web-js/rt.js `on`), in an app
// without a router: a plain click on an in-app link is the press and the
// browser's navigation is prevented; a `_blank` target, a `download` or a
// modified click is the browser's alone and the press does not run, as
// `router()` and the wasm host's input-glue.js say (Astra's batch 2
// review). rt.js runs beside stand-ins for the modules it imports.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-press-'));
copyFileSync(resolve(new URL('../../web-js/rt.js', import.meta.url).pathname), resolve(dir, 'rt.js'));
for (const [file, names] of Object.entries({ 'navigation.js': ['renderMarkup', 'reportPlace'], 'shape.js': ['conforms', 'eq'], 'pointer.js': ['pointer'], 'commands.js': ['commands'],
  'paint.js': ['paintList', 'paintFacts', 'paintFlush', 'paintOwn'], 'document.js': ['Docs', 'Head', 'head', 'markDocument', 'projectRoots'],
  'svg-transform.js': ['svgTransform'], 'dataset.js': ['ds'], 'hooks.js': ['hk'], 'perf.js': ['pf'], 'format.js': ['x_formatTime', 'x_formatDate', 'x_formatNumber'] }))
  writeFileSync(resolve(dir, file), names.map(n => `export const ${n} = () => {};`).join('\n'));

class Link {
  constructor(attrs) { this.localName = 'a'; this.attrs = new Map(Object.entries(attrs)); this.listeners = []; }
  get target() { return this.attrs.get('target') ?? ''; }
  hasAttribute(k) { return this.attrs.has(k); }
  matches(sel) { return sel.split(',').some(s => s.trim() === 'a[href]'); }
  contains(x) { return x === this; }
  closest() { return this; }
  addEventListener(type, f) { if (type === 'click') this.listeners.push(f); }
  click(mods = {}) { const ev = { target: this, button: 0, ...mods, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {} }; for (const f of this.listeners) f(ev); return ev; }
}

test('a link press is the app navigation; a browser-owned one is the browser alone', async () => {
  const { on } = await import(resolve(dir, 'rt.js'));
  const cases = [
    [{ href: '/next' }, {}, true],
    [{ href: '/next', target: '_self' }, {}, true],
    [{ href: 'https://example.com', target: '_blank' }, {}, false],
    [{ href: '/report.pdf', download: '' }, {}, false],
    [{ href: '/next' }, { metaKey: true }, false],
    [{ href: '/next' }, { button: 1 }, false],
  ];
  for (const [attrs, mods, app] of cases) {
    const a = new Link(attrs);
    let presses = 0;
    on(a, 'press', () => presses++);
    const ev = a.click(mods);
    expect([JSON.stringify(attrs), JSON.stringify(mods), presses, ev.defaultPrevented]).toEqual([JSON.stringify(attrs), JSON.stringify(mods), app ? 1 : 0, app]);
  }
});
