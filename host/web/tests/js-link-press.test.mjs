// The JS target's `press` on a link (host/web-js/rt.js `on`), in an app
// without a router: a plain click on an in-app link is the press and the
// browser's navigation is prevented; a `_blank` target, a `download` or a
// modified click is the browser's alone and the press does not run, as
// `router()` and the wasm host's input-glue.js say (Astra's batch 2
// review). rt.js (with the roster and router it re-exports) runs beside
// stand-ins for the modules it imports.
import { test, expect } from 'bun:test';
import { copyFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { standIns } from './stand-ins.mjs';

const dir = mkdtempSync(resolve(tmpdir(), 'exact-js-press-'));
const REAL = ['rt.js', 'open-streams.js', 'held.js', 'font-family.js', 'observers.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'route-pattern.js', 'schedule.js', 'budget.js', 'elements.js'];
for (const f of REAL) copyFileSync(resolve(new URL(`../../web-js/${f}`, import.meta.url).pathname), resolve(dir, f));
for (const [file, source] of standIns(REAL)) writeFileSync(resolve(dir, file), source);

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
  const { on, onPress } = await import(resolve(dir, 'rt.js'));
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
    on(a, 'press', () => presses++, onPress); // the press family's binder, as the generated module passes it
    const ev = a.click(mods);
    expect([JSON.stringify(attrs), JSON.stringify(mods), presses, ev.defaultPrevented]).toEqual([JSON.stringify(attrs), JSON.stringify(mods), app ? 1 : 0, app]);
  }
});

// An `href` that turns out empty is no link (the kernel's `Props::link`; LLP 1115 §3): the `<a>` keeps no `href`, so it
// has no link role, pointer or target, where `href=""` would be a link to the page itself. A bound `href` is written as
// it arrives and taken off as it empties (rt.js `P`).
test('an href that turns out empty is not written', async () => {
  const { P, sig, act, W } = await import(resolve(dir, 'rt.js'));
  // The page a URL is resolved against, for this test alone: test files share one process, and another's stand-in
  // document (frames-hatches.test.mjs leaves one) has no `baseURI`, so every `href` would be refused.
  const had = Object.getOwnPropertyDescriptor(globalThis, 'document');
  globalThis.document = { baseURI: 'https://app.example/' };
  try {
    const a = new Link({});
    Object.assign(a, { getAttribute: k => a.attrs.get(k) ?? null, setAttribute: (k, v) => a.attrs.set(k, v), removeAttribute: k => a.attrs.delete(k) });
    const to = sig('');
    P(a, 'href', () => to());
    expect(a.hasAttribute('href')).toBe(false);
    act(() => W(to, '/about'))();
    expect(a.attrs.get('href')).toBe('/about');
    act(() => W(to, ''))();
    expect(a.hasAttribute('href')).toBe(false);
  } finally { if (had) Object.defineProperty(globalThis, 'document', had); else delete globalThis.document; }
});
