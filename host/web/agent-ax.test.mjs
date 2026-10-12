// @ref LLP 1115.000 D14.4 — `tree --ax`'s comparison reads each host through its own platform's table, which is
// where a platform's own form of a control is expected: UIKit's toggle value (D13.3) has its oracle here. Hand-written
// replies, as agent.test.mjs's (LLP 1080.002 D6).
import { expect, test } from 'bun:test';
import { axParity } from '../../scripts/agent-ax.mjs';

const reply = (elements, source = 'chrome-cdp') => ({ epoch: 3, incarnation: 1, clock: 0, ax: { source, platform: '', order: 'tree',
  coverage: { roots: ['document'], complete: true, visited: elements.length }, modal: { present: false }, elements } });
const el = (role, name, testId, more = {}) => ({ i: 0, parent: null, id: 10, via: 'self', role, name, testId, states: {}, native: { role }, ...more });
const details = (a, b) => axParity(a, b).findings.map(f => f.detail);
const toggle = (value, more = {}) => reply([el('checkbox', 'Milk', 'milk', { value, native: { role: ['button', 'toggleButton'], class: 'ExactCheckbox' }, ...more })], 'uikit');

test('tree --ax: a checkbox is UIKit\'s toggle there, its value "1" or "0", and that is no disagreement with the web', () => {
  const on = reply([el('checkbox', 'Milk', 'milk', { states: { checked: true } })]), off = reply([el('checkbox', 'Milk', 'milk', { states: { checked: false } })]);
  expect(details(on, toggle('1'))).toEqual([]);
  expect(details(off, toggle('0'))).toEqual([]);
  expect(details(reply([el('checkbox', 'Milk', 'milk', { states: { checked: 'mixed' } })]), toggle('2'))).toEqual([]);
  // The state is still compared, and a toggle with no value of UIKit's has no state.
  expect(details(on, toggle('0'))).toEqual(['checked true (web) vs false (uikit)']);
  expect(details(on, toggle(undefined))).toEqual(['checked missing on uikit (true vs —)']);
  // Safari's English word, which the host wrote before: not the platform's value, so the state is missing.
  expect(details(on, toggle('checked'))).toEqual(['checked missing on uikit (true vs —)']);
  expect(details(off, toggle('unchecked'))).toEqual(['checked missing on uikit (false vs —)']);
});

test('tree --ax: which control a UIKit toggle stands for is its author\'s word, carried by the host\'s reply', () => {
  const check = reply([el('checkbox', 'Milk', 'milk', { states: { checked: true } })]);
  // A toggle the host does not call a checkbox is a button: an `aria-pressed` one is, on the web too.
  expect(details(check, toggle('1', { role: 'button' }))).toEqual(['role checkbox (web) vs button (uikit)']);
  expect(details(reply([el('button', 'Bold', 'milk', { states: { pressed: true } })]), toggle('1', { role: 'button', name: 'Bold' }))).toEqual([]);
  // A checkbox with no toggle trait is not UIKit's form of one.
  expect(details(check, toggle('1', { native: { role: ['button'] } }))).toEqual(['role checkbox (web) vs button (uikit)']);
  // A switch: a box with the role, a toggle; and UIKit's own `UISwitch`, by its class.
  const wifi = reply([el('switch', 'Wi-Fi', 'wifi', { states: { checked: false } })]);
  expect(details(wifi, reply([el('switch', 'Wi-Fi', 'wifi', { value: '0', native: { role: ['button', 'toggleButton'] } })], 'uikit'))).toEqual([]);
  expect(details(wifi, reply([el('switch', 'Wi-Fi', 'wifi', { value: '0', native: { role: ['button'], class: 'UISwitch' } })], 'uikit'))).toEqual([]);
  expect(details(wifi, reply([el('switch', 'Wi-Fi', 'wifi', { value: '1', native: { role: ['button'], class: 'UISwitch' } })], 'uikit'))).toEqual(['checked false (web) vs true (uikit)']);
});

test('tree --ax: each platform has its row, AppKit\'s its own forms, and a platform with none is refused', () => {
  const web = reply([el('checkbox', 'Milk', 'milk', { states: { checked: true } })]);
  const mac = (more = {}) => reply([el('checkbox', 'Milk', 'milk', { states: { checked: true }, native: { role: 'AXCheckBox' }, ...more })], 'appkit');
  expect(details(web, mac())).toEqual([]);
  expect(details(web, mac({ states: {} }))).toEqual(['checked missing on appkit (true vs —)']);
  expect(details(reply([el('switch', 'Milk', 'milk', { states: { checked: true } })]), mac({ native: { role: 'AXCheckBox', subrole: 'AXSwitch' } }))).toEqual([]);
  // A missing element is unjoined, as before.
  expect(axParity(web, reply([], 'uikit')).unjoined).toEqual(['milk']);
  expect(() => axParity(web, reply([], 'gtk'))).toThrow(/no platform table for gtk/);
});
