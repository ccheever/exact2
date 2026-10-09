// What a drive hands an agent to read (LLP 1012 §7, 2026-10-08): the transcript's short `tap` line and its
// `state` one line a section, without an app's empty sections; a screenshot shrunk to `--scale` pixels a point. The fixture
// (scripts/fixtures/transcript.txt, agent.test.mjs) pins the forms; these pin the edges.
import { test, expect } from 'bun:test';
import { emptySection, render } from '../../scripts/agent-inspect.mjs';
import { open } from '../../scripts/agent.mjs';
import { parseFlags } from '../../scripts/agent-launch.mjs';
import { contactSheet, shrink } from '../../scripts/png.mjs';

test('state leaves out only a section that holds nothing, and judges no value', () => {
  for (const v of [null, [], {}]) expect(emptySection(v)).toBe(true);
  // Anything inside is a value, whatever it says: a request in flight, a route named `idle`, a title `none`, a
  // hidden keyboard that still insets the content, a task due at 0, an idle task, Linux's `unavailable`.
  for (const v of [[{}], { route: 'idle', stack: [] }, { title: 'none' }, { visible: false, overlap: 0, guide: 500 }, { init: 0 }, { ticker: null }, { unavailable: true }])
    expect(emptySection(v)).toBe(false);
  for (const v of [0, false, '', 'none']) expect(emptySection(v)).toBe(false);
  expect(render('state', { slots: { n: 0 }, derives: {}, pending: [], reorder: null, focus: { logical: null }, clock: 0, epoch: 1 }))
    .toBe('epoch 1 · clock 0 ms\nslots {"n":0}\nfocus {"logical":null}');
});

test("a world's state is never filtered: its empty busy list is the answer", () => {
  expect(render('state', { tick: 3, hash: '0x1', entities: [], busy: [], epoch: 2, incarnation: 1, clock: 0 }))
    .toBe('epoch 2 · incarnation 1 · clock 0 ms\ntick 3\nhash "0x1"\nentities []\nbusy []');
});

test('a tap is one line that keeps its delivery and every field it does not name', () => {
  expect(render('tap', { at: [1, 2], tapped: 7, target: 'go', delivery: 'host-activation', carrier: 'ios', mode: 'agent', clock: 0, epoch: 3, incarnation: 1 }))
    .toBe('tapped #7 "go" · delivery host-activation · epoch 3 · clock 0 ms');
  expect(render('tap', { tapped: 7, target: 'go', delivery: 'platform', note: 'no scratch store', native: { view: 'UIButton' }, clock: 5, epoch: 3, incarnation: 2 }))
    .toBe('tapped #7 "go" · delivery platform · epoch 3 · incarnation 2 · clock 5 ms · note="no scratch store" · native={"view":"UIButton"}');
  expect(render('type', { steps: [{ op: 'type', args: ['q', 'hi'], reply: { typed: 4, target: 'q', delivery: 'platform', clock: 0, epoch: 2 } }] }))
    .toBe('type q hi\ntyped #4 "q" · delivery platform · epoch 2 · clock 0 ms');
});

test('a given answer reads as given, and a delivered input whose answers did not land keeps its verb', () => {
  expect(render('tap', { ticket: 3, capability: 'pick', node: 9, answered: 'cancel', delivery: 'substituted', clock: 0, epoch: 5 }))
    .toBe('answered @3 "cancel" · delivery substituted · epoch 5 · clock 0 ms · capability="pick" · node=9');
  expect(render('tap', { tapped: 3, target: 'go', delivery: 'platform', landError: 'clock land: the session closed', carrier: 'web', mode: 'agent' }))
    .toBe('tapped #3 "go" · landing failed: clock land: the session closed · delivery platform');
});

test('a failed input reads as one first, and a phase or a drag keeps where the finger is', () => {
  // A form the carrier cannot deliver did not happen: never `tapped`.
  expect(render('tap', { tapped: 3, target: 'Sky off', delivery: 'unsupported', reason: 'linux has no pinch', pinch: 2, clock: 0, epoch: 2 }))
    .toBe('tap #3 "Sky off" · delivery unsupported · epoch 2 · clock 0 ms · reason="linux has no pinch" · pinch=2');
  // s.tap returns a refusal it can explain without throwing (agent.mjs `tapRefusal`): the line must not read as a press.
  expect(render('tap', { error: 'view 7 is covered by #9', tapped: 7, target: 'go', delivery: 'platform', clock: 0, epoch: 1 }))
    .toBe('ERROR view 7 is covered by #9 · tap #7 "go" · delivery platform · epoch 1 · clock 0 ms');
  // A `down` leaves the finger held: it reads `tap down`, never `tapped`.
  expect(render('tap', { contact: 18, phase: 'down', at: [349.04, 87.5], delivery: 'platform', tapped: 18, target: 'open-deck', carrier: 'web', mode: 'agent', clock: 0, epoch: 1, incarnation: 1 }))
    .toBe('tap down #18 "open-deck" · at 349.04,87.5 · delivery platform · epoch 1 · clock 0 ms · contact=18');
  expect(render('tap', { phase: 'move', at: [379.04, 87.5], delivery: 'platform', carrier: 'web', mode: 'agent', clock: 0, epoch: 1, incarnation: 1 }))
    .toBe('tap move · at 379.04,87.5 · delivery platform · epoch 1 · clock 0 ms');
  expect(render('tap', { tapped: 5, target: 'row', at: [10, 20], drag: { dx: 0, dy: -200 }, lifted: [10, -180], delivery: 'platform', clock: 300, epoch: 4 }))
    .toBe('tapped #5 "row" · at 10,20 · delivery platform · epoch 4 · clock 300 ms · drag={"dx":0,"dy":-200} · lifted=[10,-180]');
});

test('shrink averages by area: a box average at a whole factor, weighted at a fraction, never larger', () => {
  const data = Uint8Array.from({ length: 9 * 6 * 4 }, (_, i) => (i * 37) % 256), image = { width: 9, height: 6, data };
  const sheet = contactSheet([image], { columns: 1, maxWidth: 3 });
  expect(shrink(image, 3, 2)).toEqual({ width: sheet.width, height: sheet.height, data: sheet.data });
  expect(sheet.cell).toEqual([3, 2]); // film reads the sheet's own scale from it
  expect(shrink(image, 9, 6)).toEqual(image);
  const row = { width: 3, height: 1, data: Uint8Array.from([0, 0, 0, 255, 90, 90, 90, 255, 180, 180, 180, 255]) };
  expect([...shrink(row, 2, 1).data]).toEqual([30, 30, 30, 255, 150, 150, 150, 255]);
  expect(() => shrink(row, 4, 1)).toThrow('at most the image');
  expect(() => shrink(row, 1.5, 1)).toThrow('whole sizes');
});

test('--scale is a drive flag, before the operations, and a bad one is refused as typed', async () => {
  const { flags, rest } = parseFlags(['ios', '--scale', '1', 'screenshot', 'a.png']);
  expect(flags.scale).toBe(1);
  expect(rest).toEqual(['ios', 'screenshot', 'a.png']);
  expect(parseFlags(['ios', '--scale', '1x', 'tree']).flags.scale).toBe('1x');
  // Refused before anything launches, naming what was typed (not NaN).
  await expect(open({ host: 'web', scale: '1x' })).rejects.toThrow('--scale "1x"');
  await expect(open({ host: 'web', scale: 0 })).rejects.toThrow('--scale 0');
});
