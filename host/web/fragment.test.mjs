// fragment.js, the web's scroll to a fragment (LLP 1153 D5), over stand-in elements: a scroll by the person counted from
// the navigation cancels it though the module arms later; a hold while pictures load ends at the app's own scroll, a
// newer navigation, or an entry that no longer shows the fragment; a fragment is all after the first `#`; and Back
// puts back the scrollers above the route as well as those in it, again after the browser's own scroll to the id.
import { test, expect, afterAll } from 'bun:test';

class El {
  constructor(attrs = {}, parent = null) {
    this.attrs = new Map(Object.entries(attrs)); this.parentElement = parent; this.children = []; parent?.children.push(this);
    this.scrollTop = 0; this.scrollLeft = 0; this.scrollHeight = 0; this.clientHeight = 0; this.scrollWidth = 0; this.clientWidth = 0;
    this.listeners = []; this.aligned = 0; this.isConnected = true;
  }
  get id() { return this.attrs.get('id') ?? ''; }
  getAttribute(k) { return this.attrs.get(k) ?? null; }
  hasAttribute(k) { return this.attrs.has(k); }
  setAttribute(k, v) { this.attrs.set(k, v); }
  removeAttribute(k) { this.attrs.delete(k); }
  all() { return this.children.flatMap(c => [c, ...c.all()]); }
  querySelectorAll(sel) {
    if (sel === '*') return this.all();
    if (sel === 'img') return this.all().filter(e => e.attrs.has('img'));
    const name = sel.slice(1, -1);
    return this.all().filter(e => e.attrs.has(name));
  }
  contains(e) { for (let x = e; x; x = x.parentElement) if (x === this) return true; return false; }
  closest() { return null; }
  matches() { return false; }
  addEventListener(type, f, o) { this.listeners.push({ type, f, signal: o?.signal }); }
  fire(type) { for (const l of this.listeners) if (l.type === type && !l.signal?.aborted) l.f({}); }
  scrollIntoView() { this.aligned++; }
  scrollTo({ top, left }) { this.scrollTop = top; this.scrollLeft = left; }
  focus() {}
}
const page = new El(), root = new El({}, page), port = new El({}, root);
Object.assign(port, { scrollHeight: 4000, clientHeight: 400 });
const route = new El({ navigationKey: '1' }, port), target = new El({ id: 'moves' }, route), picture = new El({ img: '' }, route);
picture.complete = false;
// Test files share one process: the stand-in document and history are put back after this file.
const had = Object.fromEntries(['document', 'history', 'requestAnimationFrame'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]));
afterAll(() => { for (const [k, d] of Object.entries(had)) if (d) Object.defineProperty(globalThis, k, d); else delete globalThis[k]; });
globalThis.document = { scrollingElement: page, activeElement: null };
globalThis.history = { state: { exact: 0, id: 1, url: '/item/7#moves' } };
const frames = [], frame = () => { for (const f of frames.splice(0)) f(); };
globalThis.requestAnimationFrame = f => frames.push(f);
const f = await import('./fragment.js');
const lines = [], log = l => lines.push(l), person = { personScrolls: 0 };

test('a fragment is everything after the first #', () => {
  expect(f.shows(1, 'a#b', { id: 1, url: '/item/7#a#b' })).toBe(true);
  expect(f.shows(1, 'a', { id: 1, url: '/item/7#a#b' })).toBe(false);
  expect(f.shows(1, '', { id: 1, url: '/item/7' })).toBe(true);
});

test("a scroll by the person since the navigation cancels its scroll, though the module armed after it", () => {
  person.personScrolls = 1;
  f.arm(root, { top: 1, fragment: 'moves', activation: 1, since: 0 }, person);
  f.scroll(root, 1, 'moves', log, 1);
  expect([target.aligned, lines.at(-1)]).toEqual([0, 'fragment #moves: the person scrolled first; the route stays where it is']);
});

test("the hold while pictures load ends at the app's own scroll, a newer navigation, or another entry shown", () => {
  const held = ends => {
    f.arm(root, { top: 1, fragment: 'moves', activation: 2, since: person.personScrolls }, person);
    f.scroll(root, 1, 'moves', log, 2);
    route.fire('load');
    expect(target.aligned).toBe(2); // the scroll, then the hold's own
    ends();
    route.fire('load');
    expect(target.aligned).toBe(2);
    target.aligned = 0; route.listeners = [];
  };
  held(() => f.cancel());
  held(() => f.arm(root, { top: 1, fragment: 'top', activation: 3, since: person.personScrolls }, person));
  held(() => { globalThis.history.state = { exact: 1, id: 1, url: '/item/7#top' }; });
  globalThis.history.state = { exact: 0, id: 1, url: '/item/7#moves' };
  held(() => { person.personScrolls++; });
});

test('Back puts back the scrollers above the route, as those in it', () => {
  picture.complete = true;
  port.scrollTop = 120;
  globalThis.history.state = { exact: 1, id: 1, url: '/item/7#other' };
  f.arm(root, { top: 1, fragment: 'other', activation: 4, pushed: true, since: person.personScrolls }, person); // entry 0 kept as it was
  port.scrollTop = 900;
  globalThis.history.state = { exact: 0, id: 1, url: '/item/7#moves' };
  f.arm(root, { top: 1, fragment: 'moves', activation: 5, traversal: true, since: person.personScrolls }, person);
  f.scroll(root, 1, 'moves', log, 5);
  expect(port.scrollTop).toBe(120);
  port.scrollTop = 900; // the browser's own, at its next frame's layout
  frame(); frame();
  expect(port.scrollTop).toBe(120);
});
