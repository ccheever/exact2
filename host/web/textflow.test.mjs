// @ref LLP 1043.000 §3 D7/D8 — pure glue tests, beside existing bun:test glue tests.
import { test, expect } from 'bun:test';
import { baselineIn, fragmentBaseline, fragmentDiff, framePhases, fontStyle, forgetTextBoxEdges, measurementCache, textBoxCut } from './textflow-glue.js';
import { render } from '../../scripts/agent.mjs';
import { chromium } from '../../scripts/agent-launch.mjs';
import { timerWake, createTimerScheduler } from './timer-glue.js';

const { executable: chrome, unavailable: browserUnavailable } = chromium();
const browserTest = need('chrome', browserUnavailable);

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
  // Tabular figures change advances: they are part of the measurement key (LLP 1053 G4).
  expect(fontStyle({ ...style, fontVariantNumeric: 'tabular-nums' }, true).key).not.toBe(font.key);
  expect(font.extra.fontVariantNumeric).toBe('normal');
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
import { need } from '../../scripts/coverage.mjs';
function controllerFixture({ trim = false, clamp = false, empty = false, ascent = () => [16, 4], paragraph = { id: 2, definite: true } } = {}) {
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
    createElement(tag) { const el = new Element(tag); if (tag === 'canvas') el.getContext = () => ({ font: '', measureText(t) { const [up, down] = ascent(this.font); return { width: t.length * 5, fontBoundingBoxAscent: up, fontBoundingBoxDescent: down, actualBoundingBoxAscent: t === 'x' ? 10 : 14 }; } }); return el; },
    createTreeWalker(el) { const nodes = []; const visit = n => { if (n instanceof Text) nodes.push(n); else n.childNodes.forEach(visit); }; visit(el); let at = 0; return { nextNode() { this.currentNode = nodes[at++]; return !!this.currentNode; } }; },
  };
  doc.body.root = doc.head.root = true;
  const context = new Element('context'), p = new Element('paragraph'), link = new Element('a'), ball = new Element('ball');
  p.append(new Text('one two ')); link.textContent = 'three four'; link.attrs.href = '/story'; link.computed.fontWeight = '700'; p.append(link);
  doc.body.append(context); context.append(p, ball); ball.box = { x: 30, y: 0, width: 20, height: 20 };
  ball.computed.shapeOutside = 'circle()'; ball.computed.shapeMargin = '0px';
  const saved = new Map();
  for (const [key, value] of Object.entries({ document: doc, getComputedStyle: el => { events.push(['read', el.tagName, 'style']); return el.computed; },
    ResizeObserver: class { observe() {} unobserve() {} disconnect() {} }, NodeFilter: { SHOW_TEXT: 4 } })) {
    saved.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); Object.defineProperty(globalThis, key, { configurable: true, value });
  }
  const calls = [], records = [], live = new Set(); let flowCount = 0, shift = 0, text = '';
  const views = new Map([[1, context], [2, p], [3, ball], [4, link]]);
  let controller;
  const request = (op, id, bytes) => {
    // Flow requests keep the paragraph band limit and the first shape's leaf-relative y.
    const floats = op === 2 ? new DataView(bytes.buffer, bytes.byteOffset) : null;
    calls.push(op); records.push({ op, id, bytes: bytes.length, height: floats?.getFloat32(16, true), shapeY: bytes.length > 24 ? floats?.getFloat32(40, true) : undefined });
    if (op === 0) {
      if (bytes.length > 65544) return { error: 'textflow exceeds 64 KiB source limit' };
      if (!live.has(id) && live.size >= 64) return { error: 'textflow exceeds 64 live paragraphs' };
      live.add(id);
    }
    if (op === 3) live.delete(id);
    if (op === 0) { text = new TextDecoder().decode(bytes.subarray(8)); return { ranges: [[0, text.length, 0, text.length]], graphemes: [...text].map((_, i) => [i, i + 1, i, i + 1]) }; }
    if (op === 1) return { prepared: true };
    if (op === 6) live.clear();
    if (op >= 3) return {};
    flowCount++;
    expect(new DataView(bytes.buffer).getFloat32(8, true)).toBe(16); // Rust owns MIN_FRAGMENT_EM; JS supplies only the strut size
    const x = bytes.length > 24 ? new DataView(bytes.buffer).getFloat32(36, true) + 20 + shift : 0;
    if (empty) return { shapes: [], complete: true, clamped: false, height: 0, line_height: 24, fragments: [] };
    return { shapes: bytes.length === 24 || x > 900 ? [] : [{ kind: 'Circle', cx: x, cy: 10, r: 10 }], complete: !clamp, clamped: clamp, height: 24, line_height: 24,
      fragments: [{ ...fragment(0, 8, 0, 0), utf16_start: 0, utf16_end: 8, paint_start: 0, paint_end: trim ? 7 : 8, available: 30, width: 25, line: 0 },
        { ...fragment(8, text.length, x, 0), utf16_start: 8, utf16_end: text.length, paint_start: 8, paint_end: text.length, available: 50, width: 40, line: 0 }] };
  };
  const batch = { ops: [{ op: 'textflow', contexts: [{ id: 1, exclusions: [3], paragraphs: [paragraph] }] }], timers: false };
  controller = createTextFlow({ views, request, agentMode: false, now: () => time, log: line => events.push(['log', line]),
    advance() { events.push(['commit']); if (paused) { controller.afterBatch({ ops: [], timers: true, timer_due_ms: time + interval }); return; } const b = { ops: [{ op: 'style', id: 3 }], timers: true, timer_due_ms: time + interval }; controller.beforeBatch(b); ball.box.x++; controller.afterBatch(b); },
    raf(fn) { frames.set(++serial, fn); return serial; }, cancel(id) { frames.delete(id); },
    delay(fn, ms) { events.push(["delay", ms]); timers.set(++serial, fn); return serial; }, clearDelay(id) { timers.delete(id); } });
  controller.afterBatch(batch);
  return { controller, context, p, link, ball, events, calls, records, live, frames, timers, fontEvents, batch,
    addParagraph(id, y, info = { definite: true }) { const el = new Element('paragraph'); el.textContent = 'one two three four'; el.box.y = y;
      context.append(el); views.set(id, el); batch.ops[0].contexts[0].paragraphs.push({ id, ...info }); return el; },
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
    expect(f.calls.filter(op => op === 4).length).toBe(1); // motion reuses parsed CSS
    // Same box, different authored shape must invalidate the layout key.
    const count = f.flowCount; f.shift(); f.ball.computed.shapeOutside = 'inset(2px)'; f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle();
    expect(f.flowCount).toBeGreaterThan(count);
    expect(f.calls.filter(op => op === 4).length).toBe(2);
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
    expect(f.events.filter(e => e[0] === 'log')).toEqual([['log', 'textflow #2: a percentage height is not proven definite; ordinary text retained']]);
  } finally { f.close(); }
});

// CSS `text-box` on a flowed paragraph (Inline Layout 3 §4.2): its spans are
// no longer trimmed by the browser, so the glue cuts as the native hosts do,
// from the root font in its 24 px line box (ascent 16, descent 4, so the
// baseline at 18): to the cap height (14) the lines move up 4, and to the
// baseline the height loses 6 more. A trim that changes reflows.
test('a flowed paragraph is trimmed as the native hosts trim it', async () => {
  const f = controllerFixture({ paragraph: { id: 2, definite: false, refusal: null } });
  try {
    f.p.computed.textBoxTrim = 'trim-both'; f.p.computed.textBoxEdge = 'cap alphabetic';
    await f.controller.settle();
    expect(f.controller.facts(2).fragments.map(x => x.y)).toEqual([-4, -4]);
    expect(f.p.style.height).toBe('14px');
    f.p.computed.textBoxTrim = 'none';
    const change = { ops: [{ op: 'style', id: 2 }], timers: false };
    f.controller.beforeBatch(change); f.controller.afterBatch(change); await f.settle();
    expect(f.controller.facts(2).fragments.map(x => x.y)).toEqual([0, 0]);
    expect(f.p.style.height).toBe('24px');
  } finally { f.close(); }
});

// The native hosts give a line's fragments one baseline: a fragment of only
// the 16px root text moves down to where the 32px link's fragment paints its
// baseline, trimmed or not.
test('every flowed fragment paints its baseline at the line\'s', async () => {
  const f = controllerFixture({ ascent: font => font.includes('32px') ? [24, 8] : [12, 4], paragraph: { id: 2, definite: false, refusal: null } });
  try {
    f.link.computed.fontSize = '32px';
    await f.controller.settle();
    expect(f.controller.facts(2).fragments.map(x => x.y)).toEqual([4, 0]);
  } finally { f.close(); }
});

// CSS trims a formatted line's edges: a flow with no line keeps its zero
// height, even where a short line box would make the cuts negative.
test('a flowed paragraph with no line is not trimmed', async () => {
  const f = controllerFixture({ empty: true, paragraph: { id: 2, definite: false, refusal: null } });
  try {
    f.p.computed.textBoxTrim = 'trim-both'; f.p.computed.textBoxEdge = 'text'; f.p.computed.lineHeight = f.link.computed.lineHeight = '10px';
    await f.controller.settle();
    expect(f.controller.facts(2).fragments).toEqual([]);
    expect(f.p.style.height).toBe('0px');
  } finally { f.close(); }
});

// @ref LLP 1043.000 §8 — the kernel admits auto height; the glue writes the flowed height.
test('an admitted auto-height paragraph flows unbounded and takes the flowed height', async () => {
  const f = controllerFixture({ paragraph: { id: 2, definite: false, refusal: null } });
  try {
    await f.controller.settle();
    expect(f.controller.facts(2).fragments.length).toBe(2);
    expect(f.records.filter(r => r.op === 2).at(-1).height).toBe(3.4028234663852886e38);
    expect(f.p.style.height).toBe('24px'); // the fake walker's height, content-box
    f.p.computed.boxSizing = 'border-box'; f.p.computed.paddingTop = '5px'; f.p.computed.borderBottomWidth = '1px';
    const change = { ops: [{ op: 'style', id: 2 }], timers: false };
    f.controller.beforeBatch(change); f.controller.afterBatch(change); await f.settle();
    expect(f.p.style.height).toBe('30px');
    const remove = { ops: [{ op: 'textflow', contexts: [] }], timers: false };
    f.controller.beforeBatch(remove); f.controller.afterBatch(remove); await f.settle();
    expect(f.p.style.height).toBe(undefined); // the authored (absent) height is restored
    expect(f.p.childNodes[1]).toBe(f.link);
  } finally { f.close(); }
});

test('a later auto paragraph is flowed where the earlier one\'s written height puts it, in one frame', async () => {
  const f = controllerFixture({ paragraph: { id: 2, definite: false, refusal: null } });
  try {
    const next = f.addParagraph(5, 0, { definite: false, refusal: null });
    Object.defineProperty(next, 'box', { get: () => ({ x: 0, y: 200 + (Number.parseFloat(f.p.style.height) || 200), width: 100, height: 24 }) });
    f.ball.box = { x: 30, y: 10, width: 20, height: 220 }; // meets 5 only once it has moved up
    f.controller.afterBatch(f.batch); await f.controller.settle();
    // p is 200 tall until its flowed 24px is written; then 5 sits at 224, meets the ball, flows.
    expect(next.style.height).toBe('24px');
    expect(f.records.filter(r => r.op === 2 && r.id === 5).map(r => r.shapeY)).toEqual([10 - 224]);
  } finally { f.close(); }
});

test('a refused auto paragraph keeps ordinary text and journals the kernel\'s reason once', async () => {
  const f = controllerFixture({ paragraph: { id: 2, definite: false, refusal: 'its wrapping context is flex' } });
  try {
    for (let i = 0; i < 3; i++) { f.controller.afterBatch({ ops: [{ op: 'style', id: 3 }], timers: false }); await f.settle(); }
    expect(f.p.childNodes[1]).toBe(f.link); expect(f.calls.filter(op => op === 2)).toEqual([]);
    expect(f.controller.facts(2).skipped).toBe('its wrapping context is flex');
    expect(f.events.filter(e => e[0] === 'log')).toEqual([['log', 'wrap-flow: text #2 has auto height and is not flowed: its wrapping context is flex (LLP 1043.000 §8)']]);
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
    // Its font does not reset the line's height, which its baseline is placed by
    const find = n => n.textContent === '…' && n.style?.cssText ? n : n.childNodes?.map(find).find(Boolean);
    expect(find(f.p)?.style.cssText).toContain('line-height:24px');
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

// @ref LLP 1073 D5 — while the batch says frames, each animation frame is one
// present at its timestamp (never a catch-up), and a slow timer keeps its timeout.
test('a batch that wants frames presents every animation frame and keeps timer timeouts', () => {
  let now = 0, id = 0; const frames = new Map(), timers = new Map(), events = [];
  const run = ts => { const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn(ts)); };
  const clock = createTimerScheduler({ now: () => now, advance(time) { events.push(['advance', time]); },
    present(ts) { events.push(['present', ts]); clock.update(5000, true); },
    raf(fn) { frames.set(++id, fn); return id; }, cancel(id) { frames.delete(id); },
    delay(fn, ms) { timers.set(++id, { fn, ms }); return id; }, clearDelay(id) { timers.delete(id); } });
  clock.update(5000, true);
  expect(frames.size).toBe(1); expect(timers.size).toBe(1);
  now = 16; run(16); now = 900; run(900);
  expect(events).toEqual([['present', 16], ['present', 900]]);
  expect(frames.size).toBe(1); expect(timers.size).toBe(1);
  clock.update(5000); expect(frames.size).toBe(0); expect(timers.size).toBe(1);
  const agent = createTimerScheduler({ now: () => 0, advance() {}, present() {}, agentMode: true,
    raf(fn) { frames.set(++id, fn); return id; }, cancel(id) { frames.delete(id); }, delay() { throw new Error('timeout'); } });
  agent.update(null, true); expect(frames.size).toBe(0);
});

// A real DOM is needed here: document capture runs before the clone forwards
// its event to the detached original. A mock dispatch misses the double route.
// Run an in-page fixture (a function) in headless Chrome over these glue modules; its result by value.
async function inChrome(fixture, modules = ['/textflow-glue.js', '/timer-glue.js', '/input-glue.js', '/prompt-glue.js'], files = {}, runtime = false) {
  const { spawn } = await import('node:child_process');
  const { mkdtempSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { join } = await import('node:path');
  const { Cdp } = await import('../../scripts/agent.mjs');
  const dir = mkdtempSync(join(tmpdir(), 'exact-flow-input-'));
  const served = new Set(modules);
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (files[path]) return new Response(Bun.file(files[path]));
    if (runtime && path === '/names.js') return new Response('export const sourceTypes = {};', { headers: { 'content-type': 'text/javascript' } });
    if (runtime && path.endsWith('.js')) {
      const js = Bun.file(new URL('../web-js' + path, import.meta.url));
      return js.exists().then(exists => new Response(exists ? js : Bun.file(new URL('.' + path, import.meta.url)), { headers: { 'content-type': 'text/javascript' } }));
    }
    return served.has(path) ? new Response(Bun.file(new URL('.' + path, import.meta.url)))
      : new Response('<!doctype html><body><main id="root"></main>', { headers: { 'content-type': 'text/html' } });
  }});
  let child;
  try {
    child = spawn(chrome,
      ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--disable-background-networking', `--user-data-dir=${dir}`, 'about:blank'],
      { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    child.on('exit', () => cdp.fail('Chrome closed'));
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await cdp.send('Page.navigate', { url: `http://127.0.0.1:${server.port}` }, sessionId);
    const result = await cdp.send('Runtime.evaluate', { expression: `(${fixture})()`, awaitPromise: true, returnByValue: true }, sessionId);
    if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
    return result.result.value;
  } finally {
    if (child && child.exitCode === null) { const exit = new Promise(r => child.once('exit', r)); child.kill(); await exit; }
    server.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}

browserTest('flowed links keep press ownership through document capture', async () => {
  expect(await inChrome(flowInputFixture)).toEqual([
    { press: 1, navigate: 0, prevented: true },
    { press: 1, navigate: 0, prevented: true },
    { press: 0, navigate: 1, prevented: true },
    { press: 0, navigate: 0, prevented: false },
  ]);
}, 20000);

// Exercise the runtime listener installation with Chrome's dispatchEvent, not a
// test double that orders capture listeners itself. The dispatch sink stands in
// for a Contract action; both hosts' actual listener functions run unchanged.
browserTest('same-target capture precedes a key registered first in both web runtimes', async () => {
  expect(await inChrome(targetKeyFixture, ['/glue.js'], {
    '/rt.js': new URL('../web-js/rt.js', import.meta.url).pathname,
  })).toEqual([
    [['capture', 'key'], ['capture']],
    [['capture', 'key'], ['capture']],
  ]);
}, 60000);

async function targetKeyFixture() {
  const [glue, rt] = await Promise.all(['/glue.js', '/rt.js'].map(url => fetch(url).then(r => r.text())));
  const attach = glue.slice(glue.indexOf('function attach('), glue.indexOf('function viewFor('));
  const on = rt.slice(rt.indexOf('export function on('), rt.indexOf('// Each event family')).replace('export ', '');
  const onKey = rt.split('\n').find(line => line.startsWith('export const onKey =')).replace('export ', '');
  const bindWasm = new Function('el', 'heard', `
    let keyEvent = null;
    const views = new Map([[1, el]]), retiredViews = new WeakSet(), inputReady = true, page = null;
    const send = () => {}, writeIn = s => s, now = () => 0, keyChord = e => e.key;
    const compositionFlush = new Map(), composing = new Set();
    const wasm = { exact_dispatch(id, kind, key) {
      heard.push(kind === 44 ? 'capture' : 'key');
      if (kind === 44 && key === 'x') keyEvent.exactStopped = true;
    }};
    ${attach}
    attach(el, 1, ['key', 'keycapture']);
  `);
  const bindJs = new Function('el', 'heard', `
    let KeyEvent = null;
    const clock = { agent: true, timers: [], now: 0 }, Mutations = [], OnHooks = {};
    ${on}
${onKey}
    on(el, 'key', () => heard.push('key'), onKey);
    on(el, 'keycapture', key => { heard.push('capture'); if (key === 'x') KeyEvent.$stopped = true; }, onKey);
  `);
  return [bindWasm, bindJs].map(bind => {
    const el = document.createElement('input'), heard = [], result = [];
    document.getElementById('root').append(el); bind(el, heard); el.focus();
    for (const key of ['a', 'x']) {
      heard.length = 0;
      el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
      result.push([...heard]);
    }
    el.remove(); return result;
  });
}

browserTest('a flowed keydown forwards once with capture and bubble handlers', async () => {
  expect(await inChrome(flowKeyFixture, undefined, {}, true)).toEqual({ events: ['keydown'], deliveries: [
    { heard: ['column', 'capture', 'bubble'], prevented: false },
    { heard: ['column'], prevented: false },
    { heard: ['column', 'capture'], prevented: false },
    { heard: ['column', 'capture', 'bubble'], prevented: true },
    { heard: ['column', 'capture', 'bubble'], prevented: true },
    { heard: ['column', 'capture', 'bubble'], prevented: true },
  ] });
}, 60000);

async function flowKeyFixture() {
  const { createTextFlow } = await import('/textflow-glue.js');
  const { on, onKey, Hosts } = await import('/rt.js');
  const root = document.getElementById('root'), views = new Map(), heard = [], results = [];
  root.innerHTML = '<div style="position:relative"><div id="paragraph" style="width:200px;height:100px;font:16px/24px serif"><a href="/story" tabindex="0">flowed link</a></div><div id="ball" style="position:absolute;left:80px;top:0;width:20px;height:20px"></div></div>';
  const parent = root.firstElementChild, paragraph = parent.firstElementChild, original = paragraph.firstElementChild, ball = parent.lastElementChild;
  on(parent, 'keycapture', key => {
    heard.push('column');
    if (key === 'x') Hosts.stopPropagation();
    if (key === 'z') Hosts.preventDefault();
  }, onKey);
  // Exact's stopPropagation marks the native event so its default stays available.
  on(original, 'key', key => { heard.push('bubble'); if (key === 'p') Hosts.preventDefault(); }, onKey);
  on(original, 'keycapture', key => { heard.push('capture'); if (key === 'y') Hosts.stopPropagation(); if (key === 'Enter') Hosts.preventDefault(); }, onKey);
  views.set(1, parent); views.set(2, paragraph); views.set(3, ball);
  const request = op => op === 0 ? { ranges: [[0, 11, 0, 11]], graphemes: Array.from({ length: 11 }, (_, i) => [i, i + 1, i, i + 1]) }
    : op === 1 ? { prepared: true } : { shapes: [{ kind: 'Circle', cx: 90, cy: 10, r: 10 }], complete: true, height: 24, line_height: 24,
      fragments: [{ start: 0, end: 11, utf16_start: 0, utf16_end: 11, paint_start: 0, paint_end: 11, x: 0, y: 0, width: 70, available: 80, line: 0 }] };
  const flow = createTextFlow({ views, request, agentMode: true, advance() {} });
  flow.afterBatch({ ops: [{ op: 'textflow', contexts: [{ id: 1, exclusions: [3], paragraphs: [{ id: 2, definite: true }] }] }], timers: false });
  await flow.settle();
  const clone = paragraph.querySelector('[data-flow-fragment] a');
  if (!clone || clone === original) throw Error('fixture must key a flowed clone');
  clone.focus();
  for (const key of ['b', 'x', 'y', 'z', 'p', 'Enter']) {
    heard.length = 0;
    const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
    clone.dispatchEvent(event);
    results.push({ heard: [...heard], prevented: event.defaultPrevented });
  }
  flow.dispose();
  return { events: original.exactFlowEvents, deliveries: results };
}

// A batch that puts flowed paragraphs back to their text (the JS target's every commit, a topology
// change on the wasm host) flows them again in the same task: no frame paints them unflowed, and a
// scroller at its end, which the shorter text clamped, keeps its offset. WebKit kept the clamp, and
// pressing a button under textflow's article scrolled the page 85 px (conformance, 2026-10-07).
browserTest('a batch that restores flowed text reflows it at once and keeps a clamped scroll', async () => {
  const r = await inChrome(flowScrollFixture);
  expect(r.flowedBefore).toBe(true);
  expect(r.flowedAfter).toBe(true);
  expect(r.atEnd).toBeGreaterThan(0);
  expect(r.after).toBe(r.atEnd);
  // Through the batch the flowed height is held, even across a host's whole-cssText style write,
  // so the scroller never clamps; after the reflow the hold is gone.
  expect(r.held).toBe(300);
  expect(r.during).toBe(r.atEnd);
  expect(r.holdAfter).toBe(null);
  expect(r.authored).toBe(0); // a scroll the batch makes on purpose stands
  expect(r.unflowed).toBe(null); // a paragraph that stops flowing keeps no held height
  expect(r.scaledHold).toBe(300); // the flowed layout height, not a transformed ancestor's on-screen 150
}, 20000);


async function flowInputFixture() {
  const { createTextFlow } = await import('/textflow-glue.js');
  const { createInputHandlers } = await import('/input-glue.js');
  const root = document.getElementById('root'), views = new Map(), results = [];
  let presses = 0, navigates = 0;
  globalThis.exact = { wasm: { exact_route_match: () => 1, exact_log() {} }, writeIn: s => s,
    navigate() { navigates++; return { ops: [{ op: 'router' }] }; } };
  createInputHandlers({ root, views, retiredViews: new WeakSet(), ready: () => true, inertAncestor: () => false, dispatch() {} });
  for (const [press, navigate, modified] of [[true, true, false], [true, false, false], [false, true, false], [true, true, true]]) {
    presses = navigates = 0;
    root.innerHTML = '<div navigationBack="/" style="position:relative"><div id="paragraph" style="width:200px;height:100px;font:16px/24px serif"><a href="/story">flowed link</a></div><div id="ball" style="position:absolute;left:80px;top:0;width:20px;height:20px"></div></div>';
    const parent = root.firstElementChild, paragraph = parent.firstElementChild, ball = parent.lastElementChild;
    parent.exactHandlers = navigate ? ['navigate'] : [];
    // Static insets must stay inert when absolute flow fragments mount.
    paragraph.style.top = press ? '50px' : '50%';
    paragraph.style.left = press ? '25px' : '25%';
    const before = paragraph.getBoundingClientRect();
    const original = paragraph.firstElementChild;
    original.exactHandlers = press ? ['press'] : [];
    original.exactFlowEvents = press ? ['click'] : [];
    if (press) original.addEventListener('click', e => { presses++; e.preventDefault(); });
    views.clear(); views.set(1, parent); views.set(2, paragraph); views.set(3, ball);
    const request = (op, id, bytes) => {
      if (op === 0) return { ranges: [[0, 11, 0, 11]], graphemes: Array.from({ length: 11 }, (_, i) => [i, i + 1, i, i + 1]) };
      if (op === 1) return { prepared: true };
      if (op !== 2) return {};
      return { shapes: [{ kind: 'Circle', cx: 90, cy: 10, r: 10 }], complete: true, height: 24, line_height: 24,
        fragments: [{ start: 0, end: 11, utf16_start: 0, utf16_end: 11, paint_start: 0, paint_end: 11, x: 0, y: 0, width: 70, available: 80, line: 0 }] };
    };
    const flow = createTextFlow({ views, request, agentMode: true, advance() {} });
    flow.afterBatch({ ops: [{ op: 'textflow', contexts: [{ id: 1, exclusions: [3], paragraphs: [{ id: 2, definite: true }] }] }], timers: false });
    await flow.settle();
    const after = paragraph.getBoundingClientRect();
    if (after.x !== before.x || after.y !== before.y || getComputedStyle(paragraph).position !== 'static')
      throw Error('flow activated static insets');
    const fragment = paragraph.querySelector('[data-flow-fragment]').getBoundingClientRect();
    if (Math.abs(fragment.x - before.x) > 0.1 || Math.abs(fragment.y - before.y) > 0.1)
      throw Error('flow fragments escaped the static paragraph');
    const clone = paragraph.querySelector('a');
    if (!clone || clone === original) throw Error('fixture must click a flowed clone');
    const event = new MouseEvent('click', { bubbles: true, cancelable: true, ctrlKey: modified });
    clone.dispatchEvent(event);
    results.push({ press: presses, navigate: navigates, prevented: event.defaultPrevented });
    flow.dispose();
  }
  return results;
}

async function flowScrollFixture() {
  const { createTextFlow } = await import('/textflow-glue.js');
  const root = document.getElementById('root'), views = new Map();
  root.innerHTML = '<div id="port" style="height:200px;overflow:auto"><div style="height:150px"></div><div id="context" style="position:relative"><div id="paragraph" style="width:200px;font:16px/24px serif">flowed text</div><div id="ball" style="position:absolute;left:80px;top:0;width:20px;height:20px"></div></div></div>';
  const port = document.getElementById('port'), parent = document.getElementById('context'), paragraph = document.getElementById('paragraph'), ball = document.getElementById('ball');
  views.set(1, parent); views.set(2, paragraph); views.set(3, ball);
  // The walker's flowed height (300 px) is far taller than the ordinary one line (24 px).
  const request = (op) => {
    if (op === 0) return { ranges: [[0, 11, 0, 11]], graphemes: Array.from({ length: 11 }, (_, i) => [i, i + 1, i, i + 1]) };
    if (op === 1) return { prepared: true };
    if (op !== 2) return {};
    return { shapes: [{ kind: 'Circle', cx: 90, cy: 10, r: 10 }], complete: true, height: 300, line_height: 24,
      fragments: [{ start: 0, end: 11, utf16_start: 0, utf16_end: 11, paint_start: 0, paint_end: 11, x: 0, y: 276, width: 70, available: 80, line: 11 }] };
  };
  const flow = createTextFlow({ views, request, agentMode: true, advance() {} });
  flow.afterBatch({ ops: [{ op: 'textflow', contexts: [{ id: 1, exclusions: [3], paragraphs: [{ id: 2, definite: false, refusal: null }] }] }], timers: false });
  await flow.settle();
  const flowedBefore = !!paragraph.querySelector('[data-flow-fragment]');
  port.scrollTop = port.scrollHeight;
  const atEnd = port.scrollTop;
  const batch = { ops: [{ op: 'children' }], timers: false };
  flow.beforeBatch(batch);
  paragraph.style.cssText = 'width:200px;font:16px/24px serif'; // the wasm host's style op replaces cssText
  const held = parseFloat(getComputedStyle(paragraph).minHeight), during = port.scrollTop; // forces layout
  flow.afterBatch(batch);
  // Read in the same task: nothing has painted.
  const result = { flowedBefore, atEnd, held, during, flowedAfter: !!paragraph.querySelector('[data-flow-fragment]'), after: port.scrollTop, holdAfter: paragraph.dataset.flowHold ?? null };
  // An offset authored while the paragraph is unflowed (a commit's scrollTop) is not taken back.
  flow.beforeBatch(batch); port.scrollTop = 0; flow.afterBatch(batch);
  result.authored = port.scrollTop;
  // Under a scaled ancestor the hold is the flowed layout height, not the on-screen rect.
  port.style.transform = 'scale(0.5)';
  flow.beforeBatch(batch); result.scaledHold = parseFloat(getComputedStyle(paragraph).minHeight); flow.afterBatch(batch);
  port.style.transform = '';
  // Its exclusion moves away, still in the context: the paragraph is ordinary text again, and the
  // height held for it goes too (it once stayed, a stale min-height, after textflow's type-size).
  ball.style.top = '2000px';
  flow.beforeBatch(batch); flow.afterBatch(batch);
  result.unflowed = paragraph.dataset.flowHold ?? null;
  flow.dispose();
  return result;
}


// A flowed paragraph in a font with no `H` or `x` (Noto Sans Arabic: cap height
// 416 of its 1000 units) is cut to that font's own cap height, as the browser's
// `text-box` and the native hosts cut it, not to a fallback glyph's ink.
browserTest('a flowed paragraph is trimmed to its own font\'s cap height, whatever glyphs it lacks', async () => {
  const fixture = async () => {
    const face = new FontFace('Arabic', 'url(/arabic.ttf)');
    document.fonts.add(await face.load());
    const { textBoxCut } = await import('/textflow-glue.js');
    const context = document.createElement('canvas').getContext('2d');
    context.font = '20px Arabic';
    const m = context.measureText('H'), baseline = (30 - (m.fontBoundingBoxAscent + m.fontBoundingBoxDescent)) / 2 + m.fontBoundingBoxAscent;
    const cut = textBoxCut({ textBoxTrim: 'trim-both', textBoxEdge: 'cap alphabetic' }, { font: '20px Arabic' }, 30, context);
    return { cap: baseline - cut.top, fallbackInk: m.actualBoundingBoxAscent };
  };
  const { cap, fallbackInk } = await inChrome(fixture, ['/textflow-glue.js', '/timer-glue.js'], { '/arabic.ttf': new URL('../linux/tests/fonts/NotoSansArabic.ttf', import.meta.url) });
  expect(Math.abs(cap - 20 * 0.416)).toBeLessThan(0.5);
  expect(Math.abs(fallbackInk - 20 * 0.416)).toBeGreaterThan(1); // the fallback's H is not the font's cap height
}, 20000);

// A line box shorter than the font grows when trimmed to its text edges, as
// Apple and Linux grow it: the cuts keep their sign.
test('a trim to the text edges of a short line box is a negative cut', () => {
  const context = { font: '', measureText: t => ({ fontBoundingBoxAscent: 16, fontBoundingBoxDescent: 4, actualBoundingBoxAscent: t === 'x' ? 10 : 14 }) };
  const cut = textBoxCut({ textBoxTrim: 'trim-both', textBoxEdge: 'text' }, { font: '16px short' }, 10, context);
  expect(cut).toEqual({ top: -5, bottom: -5 });
});

// A font that loads changes the metrics a trim was cut from: the edges are
// measured again once it has (`refreshFonts` forgets them).
test('a trim is cut again from the metrics of a font that loaded', () => {
  let ascent = 16;
  const context = { font: '', measureText: t => ({ fontBoundingBoxAscent: ascent, fontBoundingBoxDescent: 4, actualBoundingBoxAscent: t === 'x' ? 10 : 14 }) };
  const style = { textBoxTrim: 'trim-start', textBoxEdge: 'text' }, font = { font: '16px loading' };
  expect(textBoxCut(style, font, 20, context).top).toBe(0);
  ascent = 20;
  expect(textBoxCut(style, font, 20, context).top).toBe(0); // held until a font loads
  forgetTextBoxEdges();
  expect(textBoxCut(style, font, 20, context).top).toBe(-2);
});

// A larger inline run lowers the line's baseline, so the trim is cut from
// where the line paints it, with the root font's own edges.
test('a larger inline run moves the baseline a trim is cut from', () => {
  const metrics = { '16px root': [12, 4, 11], '32px big': [24, 8, 22] };
  const context = { font: '', measureText(t) { const [ascent, descent, cap] = metrics[this.font]; return { fontBoundingBoxAscent: ascent, fontBoundingBoxDescent: descent, actualBoundingBoxAscent: t === 'x' ? 8 : cap }; } };
  const style = { textBoxTrim: 'trim-both', textBoxEdge: 'cap alphabetic' }, root = { font: '16px root' }, big = { font: '32px big' };
  expect(textBoxCut(style, root, 48, context)).toEqual({ top: 28 - 11, bottom: 48 - 28 });
  const baseline = Math.max(baselineIn(root, 48, context), baselineIn(big, 48, context));
  expect(baseline).toBe(32);
  expect(textBoxCut(style, root, 48, context, baseline)).toEqual({ top: 32 - 11, bottom: 48 - 32 });
});

// A clamped fragment that keeps only its ellipsis paints it in the run the
// clamp ended in: that run's font sets the fragment's baseline.
test('an ellipsis-only fragment paints at its ellipsis run\'s baseline', () => {
  const state = { runs: [{ start: 0, end: 4 }, { start: 4, end: 8 }, { start: 8, end: 12 }], runEnds: [4, 8, 12] };
  const baselines = [16, 16, 20, 40];
  const ellipsisOnly = { paint_start: 8, paint_end: 8, ellipsis: true, ellipsisRun: 1 };
  expect(fragmentBaseline(ellipsisOnly, state, baselines)).toBe(20);
  expect(fragmentBaseline({ ...ellipsisOnly, ellipsis: false }, state, baselines)).toBe(16);
  expect(fragmentBaseline({ paint_start: 0, paint_end: 12, ellipsis: false }, state, baselines)).toBe(40);
});
