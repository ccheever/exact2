// The web host's pads (gpu-pads.js, LLP 1127.003 D2): the Gamepad API's
// standard mapping polled into the focused world, a change at a time.
import { test, expect } from 'bun:test';
import { padReading, padPoller } from './gpu-pads.js';

const pad = (index, { buttons = {}, axes = [0, 0, 0, 0], mapping = 'standard' } = {}) => ({
  index, mapping, connected: true, axes,
  buttons: Array.from({ length: 17 }, (_, i) => ({ value: buttons[i] ?? 0, pressed: (buttons[i] ?? 0) > 0 })),
});

test('a reading quantizes to 1/256 and a pad without the standard mapping reads zeros', () => {
  const r = padReading(pad(0, { buttons: { 0: 1, 7: 0.3 }, axes: [0.70001, -1.2, -0.001, 0] }));
  expect(r.standard).toBe(true);
  expect(r.buttons[0]).toBe(1);
  expect(r.buttons[7]).toBe(77 / 256);
  expect(r.axes).toEqual([179 / 256, -1, 0, 0]);
  expect(Object.is(r.axes[2], -0)).toBe(false);
  // A digital button some browsers report pressed at value 0.
  const pressed = pad(0); pressed.buttons[1] = { value: 0, pressed: true };
  expect(padReading(pressed).buttons[1]).toBe(1);
  const other = padReading(pad(1, { mapping: '', buttons: { 0: 1 }, axes: [1, 1, 1, 1] }));
  expect(other).toEqual({ standard: false, buttons: Array(17).fill(0), axes: [0, 0, 0, 0] });
});

test('the poller sends only changes to the focused world, and a disconnect when a pad goes', () => {
  let pads = [pad(0, { axes: [0.5, 0, 0, 0] })], frames = [], focused = true;
  const listeners = {};
  globalThis.navigator ??= {};
  const saved = { getGamepads: navigator.getGamepads, requestAnimationFrame: globalThis.requestAnimationFrame, window: globalThis.window, document: globalThis.document };
  navigator.getGamepads = () => [...pads, null];
  globalThis.requestAnimationFrame = (fn) => { frames.push(fn); return frames.length; };
  globalThis.window = { addEventListener: (name, fn) => { listeners[name] = fn; } };
  globalThis.document = { hasFocus: () => focused };
  try {
    const sent = [], entry = { pads: new Map(), sendPad: (e) => sent.push(e) };
    padPoller(() => entry);
    const frame = () => frames.shift()();
    frame();
    expect(sent).toEqual([{ t: 'pad', index: 0, connected: true, standard: true, buttons: Array(17).fill(0), axes: [0.5, 0, 0, 0] }]);
    frame(); // the same reading: nothing
    expect(sent.length).toBe(1);
    pads = [pad(0, { buttons: { 0: 1 }, axes: [0.5, 0, 0, 0] })];
    frame();
    expect(sent[1].buttons[0]).toBe(1);
    focused = false; pads = [pad(0)];
    frame(); // an unfocused page sends nothing
    expect(sent.length).toBe(2);
    focused = true; pads = [];
    frame(); // the pad went away; the loop stops
    expect(sent[2]).toEqual({ t: 'pad', index: 0, connected: false, standard: true, buttons: Array(17).fill(0), axes: [0, 0, 0, 0] });
    expect(frames.length).toBe(0);
    pads = [pad(2)];
    listeners.gamepadconnected();
    frame();
    expect(sent[3]).toMatchObject({ index: 2, connected: true });
  } finally {
    navigator.getGamepads = saved.getGamepads;
    globalThis.requestAnimationFrame = saved.requestAnimationFrame;
    globalThis.window = saved.window;
    globalThis.document = saved.document;
  }
});
