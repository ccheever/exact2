// The web's MIDI (LLP 1125 D6–D8): midi-glue.js over a fake Web MIDI — no
// API is `unavailable`; a permission already granted is taken silently; a
// request granted or refused; a message held until 50 ms before it is due
// and handed with its timestamp; a retraction before the hold; inputs
// opened and let go; a frame's input one delivery, Clock left out; ports
// reported again on a state change; a boot forgets what was held.
import { test, expect } from 'bun:test';
import '../midi-glue.js';

const install = globalThis.exact.installMidi;

/** A fake `navigator` with one output `out` and inputs `keys` and `pads`. */
function fake({ permission = 'prompt', refuse = false, api = true } = {}) {
  const sent = [];
  const port = (id, kind) => ({ id, name: `${id} name`, manufacturer: 'Fake', state: 'connected', type: kind, closed: 0, onmidimessage: null, close() { this.closed++; }, send(data, at) { sent.push([id, [...data], at]); } });
  const access = { inputs: new Map([['keys', port('keys', 'input')], ['pads', port('pads', 'input')]]), outputs: new Map([['out', port('out', 'output')]]), onstatechange: null };
  let asked = 0;
  const nav = api ? {
    permissions: { query: async ({ name }) => ({ name, state: permission }) },
    requestMIDIAccess: async opts => { asked++; expect(opts).toEqual({ sysex: false }); if (refuse) throw new Error('no'); return access; },
  } : {};
  return { nav, access, sent, asked: () => asked };
}

/** A clock the test moves, and the timers and frames on it. */
function clock() {
  let t = 1000;
  const timers = [], frames = [];
  const real = { setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout };
  globalThis.setTimeout = (f, ms = 0) => { const h = { f, at: t + ms }; timers.push(h); return h; };
  globalThis.clearTimeout = h => { const i = timers.indexOf(h); if (i >= 0) timers.splice(i, 1); };
  return {
    now: () => t,
    frame: f => frames.push(f),
    advance(ms) {
      const to = t + ms;
      for (;;) {
        timers.sort((a, b) => a.at - b.at);
        const h = timers[0];
        if (!h || h.at > to) break;
        timers.shift(); t = Math.max(t, h.at); h.f();
      }
      t = to;
    },
    paint() { for (const f of frames.splice(0)) f(); },
    restore() { Object.assign(globalThis, real); },
    pending: () => timers.length,
  };
}

const settle = () => new Promise(r => setImmediate(r));

function midi(opts = {}) {
  const f = fake(opts), c = clock(), delivered = [], lines = [];
  const out = install({ log: l => lines.push(l), origin: () => 1000, deliver: m => delivered.push(m), nav: f.nav, now: c.now, frame: c.frame });
  return { ...f, c, out, delivered, lines };
}

test('no Web MIDI is unavailable, and a request asks nothing', async () => {
  const m = midi({ api: false });
  m.out.ops([{ op: 'request' }]);
  await settle();
  expect(m.delivered).toEqual([{ ports: { state: 'unavailable' } }]);
  expect(m.out.state().state).toBe('unavailable');
  m.c.restore();
});

test('a permission already granted is taken silently; a prompt waits for the request', async () => {
  const granted = midi({ permission: 'granted' });
  await settle(); await settle();
  expect(granted.delivered).toEqual([{ ports: { state: 'granted',
    inputs: [{ id: 'keys', name: 'keys name', manufacturer: 'Fake', state: 'connected' }, { id: 'pads', name: 'pads name', manufacturer: 'Fake', state: 'connected' }],
    outputs: [{ id: 'out', name: 'out name', manufacturer: 'Fake', state: 'connected' }] } }]);
  granted.c.restore();
  const prompt = midi();
  await settle(); await settle();
  expect(prompt.delivered).toEqual([]);
  expect(prompt.asked()).toBe(0);
  prompt.out.ops([{ op: 'request' }]);
  await settle(); await settle();
  expect(prompt.asked()).toBe(1);
  expect(prompt.delivered[0].ports.state).toBe('granted');
  prompt.c.restore();
});

test('a refused request is denied', async () => {
  const m = midi({ refuse: true });
  m.out.ops([{ op: 'request' }]);
  await settle(); await settle();
  expect(m.delivered).toEqual([{ ports: { state: 'denied' } }]);
  expect(m.lines.some(l => l.startsWith('midi: access refused'))).toBe(true);
  m.c.restore();
});

test('a message is held until 50 ms before it is due and handed with its time', async () => {
  const m = midi({ permission: 'granted' });
  await settle(); await settle();
  // Runner time 0 is 1000 on performance.now(); the clock is at 1000.
  m.out.ops([{ op: 'send', id: 2, output: 'out', data: [0x80, 60, 0], at: 300 }, { op: 'send', id: 1, output: 'out', data: [0x90, 60, 100], at: 200 }, { op: 'send', id: 3, output: 'out', data: [0xb0, 7, 1], at: 30 }]);
  // Within the hold already: handed at once.
  expect(m.sent).toEqual([['out', [0xb0, 7, 1], 1030]]);
  m.c.advance(149);
  expect(m.sent.length).toBe(1);
  m.c.advance(1);
  expect(m.sent[1]).toEqual(['out', [0x90, 60, 100], 1200]);
  m.out.ops([{ op: 'retract', id: 2 }]);
  m.c.advance(500);
  expect(m.sent.length).toBe(2);
  expect(m.out.state()).toEqual({ kind: 'web-midi', state: 'granted', held: 0, late: 0 });
  // A send past its time is late, and counted.
  m.c.advance(100);
  m.out.ops([{ op: 'send', id: 4, output: 'out', data: [0xfc], at: 10 }]);
  expect(m.out.state().late).toBe(1);
  m.c.restore();
});

test('a boot forgets what an earlier runner left held', async () => {
  const m = midi({ permission: 'granted' });
  await settle(); await settle();
  m.out.ops([{ op: 'send', id: 1, output: 'out', data: [0x90, 60, 100], at: 500 }]);
  m.out.ops([], true);
  m.c.advance(1000);
  expect(m.sent).toEqual([]);
  expect(m.out.state().held).toBe(0);
  m.c.restore();
});

test('only opened inputs listen; a frame is one delivery, Clock left out', async () => {
  const m = midi({ permission: 'granted' });
  await settle(); await settle();
  const keys = m.access.inputs.get('keys'), pads = m.access.inputs.get('pads');
  m.out.ops([{ op: 'open', inputs: ['keys', 'pads'] }]);
  expect(typeof keys.onmidimessage).toBe('function');
  keys.onmidimessage({ data: Uint8Array.of(0x90, 60, 100), timeStamp: 1010 });
  keys.onmidimessage({ data: Uint8Array.of(0xf8), timeStamp: 1011 });
  pads.onmidimessage({ data: Uint8Array.of(0xb0, 7, 3), timeStamp: 1012 });
  m.delivered.length = 0;
  m.c.paint();
  expect(m.delivered).toEqual([{ messages: [['keys', 10, [0x90, 60, 100]], ['pads', 12, [0xb0, 7, 3]]] }]);
  m.out.ops([{ op: 'open', inputs: ['keys'] }]);
  expect(pads.onmidimessage).toBe(null);
  expect(pads.closed).toBe(1);
  expect(typeof keys.onmidimessage).toBe('function');
  m.c.restore();
});

test('a state change reports the ports again', async () => {
  const m = midi({ permission: 'granted' });
  await settle(); await settle();
  m.access.outputs.get('out').state = 'disconnected';
  m.access.onstatechange({});
  expect(m.delivered.at(-1).ports.outputs[0].state).toBe('disconnected');
  // A message for a disconnected output is not sent, and said.
  m.out.ops([{ op: 'send', id: 1, output: 'out', data: [0x90, 60, 100], at: 0 }]);
  expect(m.sent).toEqual([]);
  expect(m.lines.at(-1)).toBe('midi: out is not connected; message 1 not sent');
  m.c.restore();
});
