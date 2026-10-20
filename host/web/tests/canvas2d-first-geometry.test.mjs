// A Canvas 2D canvas's first geometry without ResizeObserver (canvas2d-glue.js
// `firstGeometry`): a page in a frame scrolled out of view gets no observer
// entries, and readiness waits on every watched canvas's first geometry
// (`settled`). A second on, the content box is read from layout (fractional,
// as the observer's); a canvas gone before then waits for nothing, a newer
// watch of its id owns its own wait, and a new element's first geometry is
// always sent (LLP 1122 queue item (3)'s QUEUE follow-up, review round 2).
// The module runs in a vm with stand-in browser globals.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const source = readFileSync(new URL('../canvas2d-glue.js', import.meta.url), 'utf8');

function glue({ agent = false } = {}) {
  const timers = [], sent = [], observed = new Set();
  const views = new Map();
  const context = vm.createContext({
    location: { href: agent ? 'http://localhost/?agent=1' : 'http://localhost/' },
    devicePixelRatio: 2,
    matchMedia: () => ({ addEventListener() {} }),
    document: { createElement: () => ({ getContext: () => ({ measureText: () => ({}) }) }), fonts: { addEventListener() {} } },
    ResizeObserver: class { observe(el) { observed.add(el); } unobserve(el) { observed.delete(el); } },
    getComputedStyle: el => el.style,
    setTimeout: (fn, ms) => { timers.push([fn, ms]); return timers.length; },
    requestAnimationFrame: () => 0,
    URL, Map, Set, Promise, DataView, Math, Number, String, Object, Array,
    console,
  });
  context.globalThis = context;
  context.exact = { send: x => sent.push(x), wasm: { exact_canvas_geometry: (id, w, h, scale) => ({ id, w, h, scale }), exact_canvas_fonts: () => null }, writeIn: x => x };
  vm.runInContext(source, context);
  const g = context.exact.canvas2dGlue({ views, now: () => 0, applyBatch() {}, generation: () => 0 });
  const el = (id, style) => ({ dataset: { view: String(id) }, children: [], style: { width: '100.25px', height: '50px', boxSizing: 'content-box', paddingLeft: '0px', paddingRight: '0px', paddingTop: '0px', paddingBottom: '0px', ...style } });
  const settledNow = async () => { let done = false; g.settled().then(() => { done = true; }); await Promise.resolve(); await Promise.resolve(); return done; };
  return { g, views, timers, sent, observed, el, settledNow, fire: () => timers.splice(0).forEach(([fn]) => fn()) };
}

test('no observer entry a second on: the content box from layout, fractional, and readiness settles', async () => {
  const w = glue(), a = w.el(7);
  w.views.set(7, a);
  w.g.op({ id: 7, watch: true });
  expect([await w.settledNow(), w.timers.map(([, ms]) => ms)]).toEqual([false, [1000]]);
  w.fire();
  expect(w.sent).toEqual([{ id: 7, w: 100.25, h: 50, scale: 2 }]);
  expect(await w.settledNow()).toBe(true);
  // A box that sizes its border: less its padding and border.
  const b = glue(), box = b.el(3, { width: '120px', height: '60px', boxSizing: 'border-box', paddingLeft: '4px', paddingRight: '6px', paddingTop: '1px', paddingBottom: '1px', borderLeftWidth: '2px', borderRightWidth: '2px', borderTopWidth: '1px', borderBottomWidth: '1px' });
  b.views.set(3, box);
  b.g.op({ id: 3, watch: true });
  b.fire();
  expect(b.sent).toEqual([{ id: 3, w: 106, h: 56, scale: 2 }]);
  // Under the agent, whose clock owns time: no timer.
  const c = glue({ agent: true });
  c.views.set(7, c.el(7));
  c.g.op({ id: 7, watch: true });
  expect(c.timers).toEqual([]);
});

test('a canvas gone before its second waits for nothing; a newer watch of its id owns its own wait; a new element always reports', async () => {
  // Destroyed (glue.js's destroy op takes it out of the views) before the second: readiness settles, nothing is sent.
  const w = glue(), a = w.el(7);
  w.views.set(7, a);
  w.g.op({ id: 7, watch: true });
  w.views.delete(7);
  w.fire();
  expect([await w.settledNow(), w.sent, w.observed.has(a)]).toEqual([true, [], false]);
  // Replaced by a new element of that id (a restart): the old wait's second leaves the new one waiting.
  const r = glue(), first = r.el(7), second = r.el(7);
  r.views.set(7, first);
  r.g.op({ id: 7, watch: true });
  r.timers.splice(0)[0][0]();
  expect(r.sent).toEqual([{ id: 7, w: 100.25, h: 50, scale: 2 }]);
  r.views.set(7, second);
  r.g.op({ id: 7, watch: true });
  expect(await r.settledNow()).toBe(false);
  // Its own second reports it, though its size is the old one's: a new canvas to its runner.
  r.fire();
  expect(r.sent).toEqual([{ id: 7, w: 100.25, h: 50, scale: 2 }, { id: 7, w: 100.25, h: 50, scale: 2 }]);
  expect(await r.settledNow()).toBe(true);
  // An old element's second that fires after a newer watch: it owns nothing, so the new wait stands.
  const s = glue(), one = s.el(5), two = s.el(5);
  s.views.set(5, one);
  s.g.op({ id: 5, watch: true });
  s.views.set(5, two);
  s.g.op({ id: 5, watch: true });
  s.timers.splice(0, 1)[0][0]();
  expect([await s.settledNow(), s.sent]).toEqual([false, []]);
  s.fire();
  expect([await s.settledNow(), s.sent.length]).toEqual([true, 1]);
});
