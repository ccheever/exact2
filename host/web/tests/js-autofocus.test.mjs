// The JS target's `autofocus` at mount (host/web-js/focus.js; LLP 1035.000 D9, LLP 1102 §3.19):
// the wasm host's focusController rule. A field mounted by a later commit takes the focus from the
// body or from the control just pressed, once; never from another focused field. The DOM is a stand-in.
import { test, expect, afterAll } from 'bun:test';

const field = (name, { shown = true, disabled = false, inert = false } = {}) => ({
  name, shown, disabled, inert, focused: 0,
  getClientRects() { return this.shown ? [{}] : []; },
  closest(sel) { return sel === '[inert]' && this.inert ? this : null; },
  matches(sel) { return sel === ':disabled' && this.disabled; },
  focus() { this.focused++; globalThis.document.activeElement = this; },
  contains(el) { return el === this; },
});
const body = { name: 'body' };
let mounted = [];
const root = { querySelectorAll: () => mounted };
const had = { document: Object.getOwnPropertyDescriptor(globalThis, 'document'), style: Object.getOwnPropertyDescriptor(globalThis, 'getComputedStyle') };
const listeners = {};
globalThis.document = { body, activeElement: body, addEventListener: (kind, f) => { (listeners[kind] ??= []).push(f); } };
const fire = (type, target) => { for (const f of listeners[type] ?? []) f({ type, target }); };
globalThis.getComputedStyle = () => ({ visibility: 'visible' });
// Test files share one process: the stand-ins go when this file is done.
afterAll(() => {
  for (const [name, d] of [['document', had.document], ['getComputedStyle', had.style]]) {
    if (d) Object.defineProperty(globalThis, name, d); else delete globalThis[name];
  }
});
globalThis.queueMicrotask ??= (f) => Promise.resolve().then(f);
const { autofocus, press, hold, within, offerAll } = await import('../../web-js/focus.js');

test('a field mounted by the press takes the focus from the pressed button, once', () => {
  const start = field('start');
  mounted = [];
  autofocus(root); // boot: nothing to focus
  start.focus(); // the tap focused its button
  const done = press(start);
  const name = field('name');
  mounted = [name];
  autofocus(root); // the commit the press ran
  done();
  expect(name.focused).toBe(1);
  expect(document.activeElement).toBe(name);
  autofocus(root); // a later commit: offered once
  expect(name.focused).toBe(1);
});

test('a field a view transition mounts after the press returns still takes the focus', () => {
  const start = field('start2');
  start.focus();
  const done = press(start);
  const held = hold(); // shared.js deferred the tree update to startViewTransition's callback
  done(); // the click handler returned
  const name = field('deferred');
  within(held, () => { mounted = [name]; autofocus(root); }); // the transition's callback runs the update
  expect(document.activeElement).toBe(name);
});

test('a commit after the press has run does not take the focus from the pressed button', () => {
  const start = field('start3');
  start.focus();
  press(start)(); // the press and its own commits are done
  expect(hold()).toBe(null); // an unrelated commit deferring its update holds no press
  const late = field('late-answer'); // say a mutation's answer mounts it later
  mounted = [late];
  autofocus(root);
  expect(late.focused).toBe(0);
  expect(document.activeElement).toBe(start);
});

test('a key pressed while a press is held supersedes it', () => {
  const start = field('start4');
  start.focus();
  const done = press(start);
  const held = hold();
  done();
  fire('keydown', start); // the person tabs or types before the transition's update runs
  const name = field('after-key');
  within(held, () => { mounted = [name]; autofocus(root); });
  expect(name.focused).toBe(0);
  expect(document.activeElement).toBe(start);
});

test('a held update runs with its own press: another control pressed meanwhile is not robbed', () => {
  const a = field('press-a'), b = field('press-b');
  a.focus();
  const doneA = press(a);
  const heldA = hold();
  doneA();
  fire('pointerdown', b); // the person presses another control
  b.focus();
  press(b)(); // its press ran and mounted nothing
  const late = field('a-tail');
  within(heldA, () => { mounted = [late]; autofocus(root); }); // A's transition update runs now
  expect(late.focused).toBe(0);
  expect(document.activeElement).toBe(b);
});

test('pressing the same control again keeps the held press', () => {
  const start = field('start5');
  start.focus();
  const done = press(start);
  const held = hold();
  done();
  fire('pointerdown', start);
  const kept = field('kept');
  within(held, () => { mounted = [kept]; autofocus(root); });
  expect(document.activeElement).toBe(kept);
});

test('a key supersedes every live press, not only the newest', () => {
  const start = field('start7');
  start.focus();
  const first = press(start);
  const held = hold(); // the first press's update waits on a view transition
  first();
  press(start)(); // the same control pressed again
  fire('keydown', start);
  const name = field('after-two-presses');
  within(held, () => { mounted = [name]; autofocus(root); });
  expect(name.focused).toBe(0);
  expect(document.activeElement).toBe(start);
});

test('a held update that throws still lets its press go', () => {
  const start = field('start6');
  start.focus();
  const done = press(start);
  const held = hold();
  done();
  expect(() => within(held, () => { throw new Error('tail'); })).toThrow('tail');
  const after = field('after-throw');
  mounted = [after];
  autofocus(root); // no press is running: nothing may take the focus
  expect(after.focused).toBe(0);
});

test('a mounted field never takes the focus from another field', () => {
  const typing = field('typing');
  typing.focus();
  const late = field('late');
  mounted = [late];
  autofocus(root);
  expect(late.focused).toBe(0);
  expect(document.activeElement).toBe(typing);
});

test('a hidden, disabled or inert field waits, and takes the focus once it is shown', () => {
  document.activeElement = body;
  const hidden = field('hidden', { shown: false }), off = field('off', { disabled: true }), shown = field('shown');
  mounted = [hidden, off, shown];
  autofocus(root);
  expect(shown.focused).toBe(1);
  expect(hidden.focused + off.focused).toBe(0);
  document.activeElement = body;
  hidden.shown = true;
  autofocus(root);
  expect(hidden.focused).toBe(1);
});

test('a carried restart offers every rebuilt field: none takes the focus later', () => {
  document.activeElement = body;
  const a = field('a', { shown: false }), b = field('b');
  mounted = [a, b];
  offerAll(root);
  a.shown = true;
  autofocus(root);
  expect(a.focused + b.focused).toBe(0);
});
