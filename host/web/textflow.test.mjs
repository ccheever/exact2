// @ref LLP 1043.000 §3 D7/D8 — pure glue tests, beside existing bun:test glue tests.
import { test, expect } from 'bun:test';
import { fragmentDiff, framePhases, fontStyle, measurementCache } from './textflow-glue.js';
import { render } from '../../scripts/agent.mjs';
import { timerWake, createTimerScheduler } from './timer-glue.js';

test('plain agent layout prints web fragment ranges, shapes and refusal reasons', () => {
  const node = { id: 7, type: 'Text', flow_shapes: [{ kind: 'Circle', cx: 40, cy: 30, r: 12 }],
    flow: { coordinate_space: 'content', line_height: 24,
      fragments: [{ start: 0, end: 9, line: 2, x: 54, y: 48, width: 80 }] } };
  const layout = () => render('layout', { viewport: { w: 300, h: 200 }, clock: 0, nodes: [], node });
  expect(layout()).toContain('flow Circle cx=40 cy=30 r=12 (leaf content box)');
  expect(layout()).toContain('fragment bytes 0..9 · band 2 · 54,48 80×24 (leaf content box)');
  node.flow.skipped = 'height is not proven definite';
  expect(layout()).toContain('flow skipped: height is not proven definite');
  delete node.flow;
  node.fragments = [{ start: 0, end: 3, band: 0, x: 0, y: 0, width: 20, height: 24 }];
  expect(layout()).toContain('fragment bytes 0..3 · band 0 · 0,0 20×24 (leaf content box)');
  expect(layout()).toContain('flow Circle cx=40 cy=30 r=12 (leaf border box)');
});

const fragment = (start, end, x, y) => ({ start, end, x, y, hyphenated: false });

test('a one-pixel shape move touches only fragments in its crossed bands', () => {
  // Fixed breaks: full-width lines above/below, two free intervals in bands 1–2.
  const before = [fragment(0, 12, 0, 0), fragment(12, 18, 0, 20), fragment(18, 24, 70, 20),
    fragment(24, 30, 0, 40), fragment(30, 36, 70, 40), fragment(36, 48, 0, 60)];
  const after = before.map(f => ({ ...f, x: f.x === 70 ? 71 : f.x }));
  expect(fragmentDiff(before, after)).toEqual({ updates: [
    { index: 2, content: false, position: true }, { index: 4, content: false, position: true }], remove: 0 });
  expect(fragmentDiff(before, before).updates).toEqual([]);
  // Negative control: deleting the executor/result must dirty every old span.
  expect(fragmentDiff(before, []).remove).toBe(before.length);
});

test('pool indexes survive content changes, shrinking and regrowth without position writes', () => {
  const old = [fragment(0, 4, 0, 0), fragment(4, 8, 0, 20)];
  const pool = old.map((_, id) => ({ id }));
  const next = [fragment(0, 8, 0, 0)];
  const diff = fragmentDiff(old, next);
  expect(diff).toEqual({ updates: [{ index: 0, content: true, position: false }], remove: 1 });
  const grow = fragmentDiff(next, old);
  expect(grow.updates.map(u => pool[u.index])).toEqual(pool);
  expect(fragmentDiff(old, old, true).updates.every(u => u.content && !u.position)).toBe(true);
});

test('whole-frame reads precede every computation and write, including two contexts', () => {
  const events = [], state = [1, 2, 3];
  const result = framePhases(state, x => { expect(events.some(e => e[0] === 'w')).toBe(false); events.push(['r', x]); return x; },
    x => { expect(events.filter(e => e[0] === 'r').length).toBe(3); events.push(['c', x]); return x + 1; },
    x => { expect(events.filter(e => e[0] === 'c').length).toBe(3); events.push(['w', x]); });
  expect(result).toEqual([2, 3, 4]);
  expect(events.map(e => e[0]).join('')).toBe('rrrcccwww');
});

const style = { fontFamily: 'system-ui', fontStyle: 'italic', fontWeight: '600', fontSize: '16px',
  lineHeight: '24px', letterSpacing: '1.5px', direction: 'rtl' };

test('computed font recipe pins macOS system faces for both painting and canvas', () => {
  const font = fontStyle(style, true, 'ar');
  expect(font.font).toBe('italic 600 16px "Helvetica Neue"');
  expect(font.extra.lang).toBe('ar');
  expect(font.extra.direction).toBe('rtl');
  expect(fontStyle(style, false).font).toContain('system-ui');
  expect(fontStyle({ ...style, fontFamily: 'system-ui, -apple-system, ui-sans-serif, sans-serif' }, true).family)
    .toBe('"Helvetica Neue", "Helvetica Neue", "Helvetica Neue", sans-serif');
  expect(fontStyle({ ...style, fontWeight: '700' }, true).key).not.toBe(font.key);
});

test('each distinct font/segment measures once, emoji and grapheme spacing fallback apply', () => {
  const calls = [], ctx = { measureText: text => { calls.push(text); return { width: 40 }; } };
  const cache = measurementCache(ctx), font = fontStyle(style);
  expect(cache.measure(font, '👩🏽‍🚀a', 2, 1, 3)).toBe(40);
  expect(cache.measure(font, '👩🏽‍🚀a', 2, 1, 3)).toBe(40);
  expect(calls).toEqual(['👩🏽‍🚀a']);
  cache.measure(fontStyle({ ...style, fontWeight: '700' }), '👩🏽‍🚀a', 2, 1, 3);
  expect(calls.length).toBe(2);
  cache.clear(); cache.measure(font, '👩🏽‍🚀a', 2, 1, 3);
  expect(calls.length).toBe(3);
});

test('supported native letterSpacing is not added twice; oversized distinct inputs evict', () => {
  const ctx = { letterSpacing: '0px', measureText: text => ({ width: text.length }) };
  const cache = measurementCache(ctx), font = fontStyle(style);
  expect(cache.measure(font, 'abc', 3, 0)).toBe(3);
  expect(ctx.letterSpacing).toBe('1.5px');
  for (let i = 0; i < 20000; i++) cache.measure(font, `word${i}`, 5, 0);
  expect(cache.size).toBeLessThanOrEqual(8192);
  const huge = 'x'.repeat(1024 * 1024 + 1);
  expect(cache.measure(font, huge, huge.length, 0)).toBe(huge.length);
  expect(cache.size).toBeLessThanOrEqual(8192);
});

test('worst fragment array is linear and a stable 1000-frame geometry touches nothing', () => {
  const fragments = Array.from({ length: 65536 }, (_, i) => fragment(i, i + 1, i % 2 * 50, i * 20));
  expect(fragmentDiff([], fragments).updates.length).toBe(65536);
  for (let i = 0; i < 1000; i++) expect(fragmentDiff(fragments.slice(0, 60), fragments.slice(0, 60)).updates).toEqual([]);
});

// Minimal DOM test double: exercises the actual controller and its phase order,
// not browser line layout. Real metrics/selection remain an orchestrator check.
import { createTextFlow } from './textflow-glue.js';
function controllerFixture({ trim = false, clamp = false } = {}) {
  const events = [], frames = new Map(), timers = new Map(), fontEvents = new Map(); let serial = 0, paused = false, time = 0, interval = 16;
  const defaults = { ...style, fontFamily: 'serif', fontStyle: 'normal', fontWeight: '400', letterSpacing: '0px',
    transform: 'none', translate: 'none', scale: 'none', rotate: 'none', position: 'static', visibility: 'visible',
    paddingLeft: '0', paddingRight: '0', paddingTop: '0', paddingBottom: '0', borderLeftWidth: '0', borderRightWidth: '0',
    borderTopWidth: '0', borderBottomWidth: '0', overflowWrap: 'normal', textAlign: 'start', direction: 'ltr', color: 'black' };
  class Element {
    constructor(tag) {
      this.tagName = tag; this.childNodes = []; this.parentElement = null; this.dataset = {}; this.attrs = {};
      this.style = new Proxy({}, { set: (o, k, v) => { events.push(['write', tag, k]); o[k] = v; return true; } });
      this.computed = { ...defaults }; this.box = { x: 0, y: 0, width: 100, height: 200 };
      this.clientLeft = 0; this.clientTop = 0; this.scrollTop = 0; this.scrollLeft = 0;
    }
    get parentNode() { return this.parentElement; }
    get isConnected() { return this.root || !!this.parentElement?.isConnected; }
    get textContent() { return this.childNodes.map(n => n.textContent).join(''); }
    set textContent(value) { this.replaceChildren(new Text(value)); }
    append(...nodes) { for (const node of nodes) { node.remove(); node.parentElement = this; this.childNodes.push(node); events.push(['write', this.tagName, 'append']); } }
    replaceChildren(...nodes) { for (const n of [...this.childNodes]) n.remove(); this.append(...nodes); events.push(['write', this.tagName, 'replace']); }
    remove() { if (this.parentElement) { const p = this.parentElement; p.childNodes.splice(p.childNodes.indexOf(this), 1); this.parentElement = null; events.push(['write', this.tagName, 'remove']); } }
    contains(n) { for (; n; n = n.parentElement) if (n === this) return true; return false; }
    closest() { return null; }
    getBoundingClientRect() { events.push(['read', this.tagName]); return { ...this.box }; }
    getClientRects() { events.push(['read', this.tagName]); return [this.box]; }
    removeAttribute(name) { delete this.attrs[name]; }
    cloneNode() { const n = new Element(this.tagName); n.attrs = { ...this.attrs }; return n; }
    addEventListener() {}
    dispatchEvent() { return true; }
  }
  class Text extends Element {
    constructor(data) { super('#text'); this.data = data; }
    get textContent() { return this.data; }
    set textContent(value) { this.data = value; }
  }
  const doc = { body: new Element('body'), head: new Element('head'), documentElement: { lang: '' },
    fonts: { ready: Promise.resolve(), addEventListener(k, fn) { fontEvents.set(k, fn); }, removeEventListener(k) { fontEvents.delete(k); } },
    selection: null, getSelection() { return this.selection; }, addEventListener() {}, removeEventListener() {},
    createRange() { let root, node, offset; return { selectNodeContents(value) { root = value; }, setEnd(n, o) { node = n; offset = o; }, toString() {
      let text = '', done = false; const walk = n => { if (done) return; if (n === node) { text += n.textContent.slice(0, offset); done = true; }
        else if (n instanceof Text) text += n.data; else n.childNodes.forEach(walk); }; walk(root); return text;
    } }; },
    createElement(tag) { const el = new Element(tag); if (tag === 'canvas') el.getContext = () => ({ measureText: t => ({ width: t.length * 5 }) }); return el; },
    createTreeWalker(el) { const nodes = []; const visit = n => { if (n instanceof Text) nodes.push(n); else n.childNodes.forEach(visit); }; visit(el); let at = 0; return { nextNode() { this.currentNode = nodes[at++]; return !!this.currentNode; } }; },
  };
  doc.body.root = doc.head.root = true;
  const context = new Element('context'), p = new Element('paragraph'), link = new Element('a'), ball = new Element('ball');
  p.append(new Text('one two ')); link.textContent = 'three four'; link.attrs.href = '/story'; link.computed.fontWeight = '700'; p.append(link);
  doc.body.append(context); context.append(p, ball); ball.box = { x: 30, y: 0, width: 20, height: 20 };
  const saved = new Map();
  for (const [key, value] of Object.entries({ document: doc, getComputedStyle: el => { events.push(['read', el.tagName, 'style']); return el.computed; },
    ResizeObserver: class { observe() {} unobserve() {} disconnect() {} }, NodeFilter: { SHOW_TEXT: 4 } })) {
    saved.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); Object.defineProperty(globalThis, key, { configurable: true, value });
  }
  const calls = [], records = [], live = new Set(); let flowCount = 0, shift = 0, text = '';
  const views = new Map([[1, context], [2, p], [3, ball], [4, link]]);
  let controller;
  const request = (op, id, bytes) => {
    calls.push(op); records.push({ op, id, bytes: bytes.length });
    if (op === 0) {
      if (bytes.length > 65544) return { error: 'textflow exceeds 64 KiB source limit' };
      if (!live.has(id) && live.size >= 64) return { error: 'textflow exceeds 64 live paragraphs' };
      live.add(id);
    }
    if (op === 3) live.delete(id);
    if (op === 0) { text = new TextDecoder().decode(bytes.subarray(8)); return { ranges: [[0, text.length, 0, text.length]], graphemes: [...text].map((_, i) => [i, i + 1, i, i + 1]) }; }
    if (op === 1) return { prepared: true };
    if (op === 3) return {};
    flowCount++;
    expect(new DataView(bytes.buffer).getFloat32(8, true)).toBe(16); // Rust owns MIN_FRAGMENT_EM; JS supplies only the strut size
    const x = bytes.length > 24 ? new DataView(bytes.buffer).getFloat32(36, true) + 20 + shift : 0;
    return { shapes: bytes.length === 24 || x > 900 ? [] : [{ kind: 'Circle', cx: x, cy: 10, r: 10 }], complete: !clamp, clamped: clamp, height: 24, line_height: 24,
      fragments: [{ ...fragment(0, 8, 0, 0), utf16_start: 0, utf16_end: 8, paint_start: 0, paint_end: trim ? 7 : 8, available: 30, width: 25, line: 0 },
        { ...fragment(8, text.length, x, 0), utf16_start: 8, utf16_end: text.length, paint_start: 8, paint_end: text.length, available: 50, width: 40, line: 0 }] };
  };
  const batch = { ops: [{ op: 'textflow', contexts: [{ id: 1, exclusions: [3], paragraphs: [{ id: 2, definite: true }] }] }], timers: false };
  controller = createTextFlow({ views, request, agentMode: false, now: () => time, log: line => events.push(['log', line]),
    advance() { events.push(['commit']); if (paused) { controller.afterBatch({ ops: [], timers: true, timer_due_ms: time + interval }); return; } const b = { ops: [{ op: 'style', id: 3 }], timers: true, timer_due_ms: time + interval }; controller.beforeBatch(b); ball.box.x++; controller.afterBatch(b); },
    raf(fn) { frames.set(++serial, fn); return serial; }, cancel(id) { frames.delete(id); },
    delay(fn, ms) { events.push(["delay", ms]); timers.set(++serial, fn); return serial; }, clearDelay(id) { timers.delete(id); } });
  controller.afterBatch(batch);
  return { controller, context, p, link, ball, events, calls, records, live, frames, timers, fontEvents, batch,
    addParagraph(id, y) { const el = new Element('paragraph'); el.textContent = 'one two three four'; el.box.y = y;
      context.append(el); views.set(id, el); batch.ops[0].contexts[0].paragraphs.push({ id, definite: true }); return el; },
    pause(value) { paused = value; }, timer(ms) { interval = ms; controller.afterBatch({ ops: [], timers: true, timer_due_ms: time + ms }); }, poll(ms = interval) { time += ms; const pending = [...timers.values()]; timers.clear(); pending.forEach(fn => fn()); },
    select() { doc.selection = { anchorNode: p.childNodes[0], anchorOffset: 2, focusNode: link.childNodes[0], focusOffset: 5,
      setBaseAndExtent(a, ao, b, bo) { this.anchorNode = a; this.anchorOffset = ao; this.focusNode = b; this.focusOffset = bo; } }; },
    get selection() { return doc.selection; }, shift() { shift++; },
    get flowCount() { return flowCount; },
    async settle() {
      let done = false, error; const result = controller.settle().then(() => { done = true; }, e => { error = e; done = true; });
      for (let i = 0; i < 20 && !done; i++) { await Promise.resolve(); if (!done) { const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn()); } }
      if (!done) throw Error('fake RAF did not settle'); await result; if (error) throw error;
    },
    tick() { time += 1000 / 60; const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn()); },
    close() { controller.dispose(); for (const [key, value] of saved) { if (value) Object.defineProperty(globalThis, key, value); else delete globalThis[key]; } },
  };
}

test('actual controller pools spans, reads before writes, preserves inline links and cached preparation', async () => {
  const f = controllerFixture();
  try {
    f.events.length = 0; await f.controller.settle();
    expect(f.p.textContent).toBe('one two three four');
    const spans = [...f.p.childNodes];
    expect(spans.length).toBe(2);
    expect(spans[0].childNodes[0].style.cssText).not.toContain('color:');
    expect(spans[1].childNodes[0].tagName).toBe('a');
    expect(spans[1].childNodes[0].attrs.href).toBe('/story');
    const firstWrite = f.events.findIndex(e => e[0] === 'write'), lastRead = f.events.findLastIndex(e => e[0] === 'read');
    expect(firstWrite).toBeGreaterThan(lastRead);
    expect(f.calls.filter(op => op === 1).length).toBe(1);
    f.timer(16); f.events.length = 0; f.tick();
    expect(f.events[0]).toEqual(['commit']);
    expect(f.p.childNodes).toEqual(spans);
    expect(f.controller.facts(2).touched_spans).toBe(1);
    expect(f.calls.filter(op => op === 1).length).toBe(1);
    expect(f.events.findIndex(e => e[0] === 'write')).toBeGreaterThan(f.events.findLastIndex(e => e[0] === 'read'));
    // Same box, different authored shape must invalidate the layout key.
    const count = f.flowCount; f.shift(); f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.controller.settle();
    expect(f.flowCount).toBeGreaterThan(count);
    expect(f.p.childNodes).toEqual(spans);
    // Width/paint changes preserve Prepared when text and computed font match.
    const change = { ops: [{ op: 'style', id: 2 }], timers: false };
    f.controller.beforeBatch(change); f.p.box.width = 120; f.controller.afterBatch(change); await f.settle();
    expect(f.calls.filter(op => op === 1).length).toBe(1);
    const remove = { ops: [{ op: 'textflow', contexts: [] }], timers: false };
    f.controller.beforeBatch(remove); f.controller.afterBatch(remove); await f.controller.settle(); f.tick();
    expect(f.p.textContent).toBe('one two three four'); expect(f.p.childNodes[1]).toBe(f.link);
    expect(f.calls.at(-1)).toBe(3); expect(f.frames.size).toBe(0);
  } finally { f.close(); }
});

test('text and font-loading changes reprepare, with original text available for batch props', async () => {
  const f = controllerFixture();
  try {
    await f.controller.settle();
    const batch = { ops: [{ op: 'props', id: 4 }], timers: false };
    f.controller.beforeBatch(batch); expect(f.link.isConnected).toBe(true);
    f.link.textContent = 'new text'; f.controller.afterBatch(batch); await f.controller.settle();
    expect(f.p.textContent).toBe('one two new text'); expect(f.calls.filter(op => op === 1).length).toBe(2);
    f.fontEvents.get('loadingdone')(); await f.controller.settle();
    expect(f.calls.filter(op => op === 1).length).toBe(3);
  } finally { f.close(); }
});

test('quiet geometry keeps fast timers on RAF; a slow timer sleeps until its deadline', async () => {
  const f = controllerFixture();
  try {
    await f.controller.settle(); f.pause(true); f.timer(16);
    for (let i = 0; i < 5; i++) f.tick();
    expect(f.frames.size).toBe(1); expect(f.timers.size).toBe(0);
    f.timer(1000); expect(f.frames.size).toBe(0); expect(f.timers.size).toBe(1);
    f.events.length = 0; f.poll();
    expect(f.events.map(e => e[0])).toEqual(['commit', 'delay']); expect(f.events[1][1]).toBeCloseTo(1000); expect(f.frames.size).toBe(0);
    f.pause(false); f.poll(); expect(f.frames.size).toBe(1);
    f.tick(); expect(f.controller.facts(2).touched_spans).toBe(1);
    for (let i = 0; i < 3; i++) f.tick(); // settle the post-write geometry probe
    expect(f.timers.size).toBe(1); expect(f.frames.size).toBe(0);
    f.controller.afterBatch({ ops: [], timers: false });
    f.tick(); f.poll(); expect(f.timers.size).toBe(0);
  } finally { f.close(); }
});

test('selection source offsets survive fragment construction and paragraph style restoration', async () => {
  const f = controllerFixture();
  try {
    f.select(); await f.controller.settle();
    expect(f.selection.anchorNode.data).toBe('one two '); expect(f.selection.anchorOffset).toBe(2);
    expect(f.selection.focusNode.data).toBe('three four'); expect(f.selection.focusOffset).toBe(5);
    const batch = { ops: [{ op: 'style', id: 2 }], timers: false };
    f.controller.beforeBatch(batch); f.controller.afterBatch(batch); await f.controller.settle();
    expect(f.selection.anchorNode.isConnected).toBe(true); expect(f.selection.focusNode.isConnected).toBe(true);
    expect(f.selection.anchorOffset).toBe(2); expect(f.selection.focusOffset).toBe(5);
  } finally { f.close(); }
});

test('an exclusion outside the paragraph keeps walker fragments until the context goes away', async () => {
  const f = controllerFixture();
  try {
    await f.controller.settle(); f.ball.box.x = 1000;
    f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    expect(f.controller.facts(2).fragments.length).toBeGreaterThan(0); expect(f.controller.facts(2).shapes).toEqual([]);
    expect(f.p.childNodes[1]).not.toBe(f.link); expect(f.p.textContent).toBe('one two three four');
    f.ball.box.x = 30; f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    expect(f.calls.filter(op => op === 1).length).toBe(3);
    expect(f.p.childNodes[1]).not.toBe(f.link);
  } finally { f.close(); }
});

test('a percentage of auto height stays ordinary; an explicit containing height admits it', async () => {
  const f = controllerFixture();
  try {
    f.p.style.height = '100%'; await f.controller.settle();
    expect(f.controller.facts(2).skipped).toContain('not proven definite');
    expect(f.p.childNodes[1]).toBe(f.link); expect(f.calls).toEqual([]);
    const batch = { ops: [{ op: 'style', id: 1 }], timers: false };
    f.controller.beforeBatch(batch); f.context.style.height = '200px'; f.controller.afterBatch(batch); await f.controller.settle();
    expect(f.controller.facts(2).fragments.length).toBe(2);
  } finally { f.close(); }
});


test('visibility hidden exclusions still participate in layout', async () => {
  const f = controllerFixture();
  try {
    f.ball.computed.visibility = 'hidden';
    await f.controller.settle();
    expect(f.controller.facts(2).shapes.length).toBe(1);
  } finally { f.close(); }
});

test('fragment paint ranges exclude consumed hanging whitespace', async () => {
  const f = controllerFixture({ trim: true });
  try {
    await f.controller.settle();
    expect(f.p.childNodes[0].textContent).toBe('one two');
    expect(f.p.textContent).toBe('one twothree four');
  } finally { f.close(); }
});

test('auto-height overlap journals once per boot and a nonmeeting shape is legitimate', async () => {
  const f = controllerFixture();
  try {
    f.p.style.height = '100%';
    f.ball.box.x = 1000; await f.controller.settle();
    expect(f.events.filter(e => e[0] === 'log')).toEqual([]);
    f.ball.box.x = 30;
    for (let i = 0; i < 3; i++) {
      f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    }
    expect(f.events.filter(e => e[0] === 'log')).toEqual([['log', 'wrap-flow: text #2 has auto height and is not flowed (LLP 1043.000 stage 2)']]);
  } finally { f.close(); }
});

test('line-clamp paints a measured literal ellipsis in the final fragment', async () => {
  const f = controllerFixture({ clamp: true });
  try {
    f.p.computed.webkitLineClamp = '1';
    await f.controller.settle();
    expect(f.p.textContent.endsWith('…')).toBe(true);
    const final = f.controller.facts(2).fragments.at(-1);
    expect(final.width).toBeLessThanOrEqual(final.available);
  } finally { f.close(); }
});


test('nonmeeting paragraphs free preparations and moving shapes do not reread their boxes', async () => {
  const f = controllerFixture();
  try {
    await f.controller.settle(); f.ball.box.x = 1000;
    f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    expect(f.live.size).toBe(0);
    expect(f.records.filter(r => r.op === 2).at(-1).bytes).toBe(24);
    const before = f.flowCount; f.events.length = 0; f.ball.box.x++;
    f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    expect(f.flowCount).toBe(before);
    expect(f.events.filter(e => e[0] === 'read' && e[1] === 'paragraph')).toEqual([]);
    expect(f.controller.facts(2).fragments.length).toBeGreaterThan(0);
  } finally { f.close(); }
});

test('paragraph budget retains the nearest to viewport and journals the overflow', async () => {
  const f = controllerFixture();
  try {
    for (let i = 0; i < 64; i++) f.addParagraph(100 + i, i === 63 ? 0 : 10000 + i * 250);
    f.ball.box.height = 30000; f.controller.afterBatch(f.batch);
    await f.controller.settle();
    expect(f.controller.facts(163).fragments.length).toBeGreaterThan(0);
    expect(f.live.size).toBeLessThanOrEqual(64);
    expect(f.events.filter(e => e[0] === 'log').some(e => e[1].includes('64'))).toBe(true);
  } finally { f.close(); }
});

test('oversize text is journaled while ordinary source stays visible', async () => {
  const f = controllerFixture();
  try {
    f.p.textContent = 'x'.repeat(65537); await f.controller.settle();
    expect(f.p.textContent.length).toBe(65537);
    expect(f.events.filter(e => e[0] === 'log').some(e => e[1].includes('64 KiB'))).toBe(true);
  } finally { f.close(); }
});

// @ref LLP 1044.001 §7.4 — the bounded experiment must not promote a partial
// or differently shaped paragraph when the exclusions executor refuses it.
for (const bytes of [1024 * 1024, 4 * 1024 * 1024]) {
  test(`${bytes}-byte text keeps its authored DOM through width and font changes`, async () => {
    const f = controllerFixture();
    try {
      const unit = 'Café 🦀 東京 e\u0301 ';
      f.link.textContent = unit.repeat(Math.ceil(bytes / new TextEncoder().encode(unit).length));
      const source = f.p.textContent, nodes = [...f.p.childNodes], linkText = f.link.childNodes[0];
      f.select(); f.selection.focusOffset = linkText.data.length - 7;
      for (const width of [600, 1160, 600]) {
        const batch = { ops: [{ op: 'style', id: 2 }], timers: false };
        f.controller.beforeBatch(batch); f.p.box.width = width; f.controller.afterBatch(batch);
        await f.settle();
        expect(f.p.textContent).toBe(source);
        expect(f.p.childNodes).toEqual(nodes);
        expect(f.link.childNodes[0]).toBe(linkText);
        expect(f.link.attrs.href).toBe('/story');
        expect(f.selection.focusNode).toBe(linkText);
        expect(f.selection.focusOffset).toBe(linkText.data.length - 7);
        expect(f.controller.facts(2).fragments).toEqual([]);
        expect(f.controller.facts(2).skipped).toContain('64 KiB');
      }
      f.fontEvents.get('loadingdone')(); await f.settle();
      expect(f.p.textContent).toBe(source);
      expect(f.p.childNodes).toEqual(nodes);
      expect(f.link.childNodes[0]).toBe(linkText);
      expect(f.calls).not.toContain(1); // no approximate preparation or partial paint
      expect(f.calls).not.toContain(2);
    } finally { f.close(); }
  });
}

test('timer scheduling chooses RAF at 16ms, one timeout at 1000ms, and no agent wake', () => {
  expect(timerWake(116, 100)).toEqual({ kind: 'frame' });
  expect(timerWake(1100, 100)).toEqual({ kind: 'timeout', ms: 1000 });
  expect(timerWake(16, 1000)).toEqual({ kind: 'frame' });
  expect(timerWake(null, 100)).toEqual({ kind: 'none' });
  expect(timerWake(116, 100, true)).toEqual({ kind: 'none' });
  // JS timeout overflow must not turn a distant deadline into a busy loop.
  expect(timerWake(9e15, 0)).toEqual({ kind: 'timeout', ms: 2147483647 });
});

test('one scheduler catches up once before paint, replaces wakes, and cancels on disposal', () => {
  let now = 0, id = 0; const frames = new Map(), timers = new Map(), events = [];
  const clock = createTimerScheduler({ now: () => now, advance(time) {
    events.push(['advance', time]); clock.update(time + 16); clock.requestFrame();
  }, paint() { events.push(['paint']); },
    raf(fn) { frames.set(++id, fn); return id; }, cancel(id) { frames.delete(id); },
    delay(fn, ms) { timers.set(++id, { fn, ms }); return id; }, clearDelay(id) { timers.delete(id); } });
  clock.update(1000); clock.update(2000);
  expect(frames.size).toBe(0); expect(timers.size).toBe(1);
  expect([...timers.values()][0].ms).toBe(2000);
  clock.update(16); expect(timers.size).toBe(0); expect(frames.size).toBe(1);
  // Worst realistic input: a 500ms hitch plus several invalidations in one frame.
  now = 500; clock.requestFrame(); clock.requestFrame();
  const callback = [...frames.values()][0]; frames.clear(); callback();
  expect(events).toEqual([['advance', 500], ['paint']]);
  expect(frames.size).toBe(1); expect(timers.size).toBe(0);
  clock.dispose(); expect(frames.size + timers.size).toBe(0);
  clock.update(501); clock.requestFrame(); expect(frames.size + timers.size).toBe(0);
});
