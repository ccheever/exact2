// The JS target's `autofocus` at mount (host/web-js/focus.js; LLP 1035.000 D9, LLP 1102 §3.19):
// the wasm host's focusController rule. A field mounted by a later commit takes the focus from the
// body or from the control just pressed, once; never from another focused field. The DOM is a stand-in.
import { test, expect } from 'bun:test';

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
globalThis.document = { body, activeElement: body };
globalThis.getComputedStyle = () => ({ visibility: 'visible' });
globalThis.queueMicrotask ??= (f) => Promise.resolve().then(f);
const { autofocus, press } = await import('../../web-js/focus.js');

test('a field mounted after a press takes the focus from the pressed button, once', async () => {
  const start = field('start');
  mounted = [];
  autofocus(root, true); // boot: nothing to focus
  start.focus(); // the tap focused its button
  press(start);
  const name = field('name');
  mounted = [name];
  autofocus(root); // the commit that mounted it
  expect(name.focused).toBe(1);
  expect(document.activeElement).toBe(name);
  autofocus(root); // a later commit: offered once
  expect(name.focused).toBe(1);
  await Promise.resolve(); // the press ends with its microtasks
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

test('a hidden, disabled or inert field waits; the boot offers the rest no second time', () => {
  document.activeElement = body;
  const hidden = field('hidden', { shown: false }), off = field('off', { disabled: true }), shown = field('shown');
  mounted = [hidden, off, shown];
  autofocus(root);
  expect(shown.focused).toBe(1);
  expect(hidden.focused + off.focused).toBe(0);
  document.activeElement = body;
  const a = field('a'), b = field('b');
  mounted = [a, b];
  autofocus(root, true); // a boot: the first takes it, the rest are marked offered
  expect(a.focused).toBe(1);
  document.activeElement = body;
  autofocus(root);
  expect(b.focused).toBe(0);
});
