// `prefer`'s facts by their names (agent-prefer.mjs `preferGroups`): a fact answers to its near spellings
// (LLP 1116.007 D3.5: `prefer color-scheme dark`), and an unknown fact or value is still refused.
import { test, expect } from 'bun:test';
import { PREFERENCES, mediaHeard, preferGroups, preferWeb } from './agent-prefer.mjs';
import { MalformedOperation } from './agent-keys.mjs';
import { pageReporter } from '../host/web/navigation.js';

test('a near spelling of a fact is that fact', () => {
  for (const name of ['prefers-color-scheme', 'color-scheme', 'colorScheme', 'prefersColorScheme', 'Color_Scheme']) {
    expect(preferGroups({ [name]: 'dark' }).media).toEqual({ 'prefers-color-scheme': 'dark' });
  }
  expect(preferGroups({ 'reduced-motion': 'reduce', contrast: 'more' }).media).toEqual({ 'prefers-reduced-motion': 'reduce', 'prefers-contrast': 'more' });
  expect(preferGroups({ canShare: 'false', rootFontSize: '20' }).page).toEqual({ 'can-share': false, 'root-font-size': 20 });
  expect(preferGroups({ displayMode: 'fullscreen' }).media).toEqual({ 'display-mode': 'fullscreen' });
  expect(preferGroups({ segments: '2x1', gap: '40' }).fold).toEqual({ cols: 2, rows: 1, gap: 40 });
});

test('the midi group names what a request finds and the ports by name (LLP 1125 §4)', () => {
  expect(preferGroups({ 'midi.state': 'granted', 'midi.outputs': 'OP-XY, IAC', 'midi.inputs': 'keys' }).midi).toEqual({ state: 'granted', outputs: ['OP-XY', 'IAC'], inputs: ['keys'] });
  expect(preferGroups({ 'midi.outputs': '' }).midi).toEqual({ outputs: [] });
  expect(() => preferGroups({ 'midi.state': 'maybe' })).toThrow(MalformedOperation);
});

test('an unknown fact or a value it does not take is refused naming what is expected', () => {
  for (const facts of [{ 'prefers-color-scheme': 'banana' }, { theme: 'dark' }, { 'has-focus': undefined }]) expect(() => preferGroups(facts)).toThrow(MalformedOperation);
  expect(() => preferGroups({ 'color-scheme': 'blue' })).toThrow(/^prefer: prefers-color-scheme blue: expected .*prefers-color-scheme dark\|light/);
  expect(() => preferGroups({ theme: 'dark' })).toThrow(/^prefer: theme dark: expected /);
  expect(() => preferGroups({ gap: '4' })).toThrow('prefer: gap needs segments');
});

test('Chrome refuses display-mode emulation instead of reporting an unapplied override', async () => {
  await expect(preferWeb({ media: { 'display-mode': 'fullscreen' }, page: {} }))
    .rejects.toThrow('Chrome prefer cannot emulate display-mode fullscreen');
  expect((await preferWeb({ media: { 'display-mode': 'fullscreen' }, page: {} }).catch(e => e)).malformed).toBeUndefined();
});

test('segments and gap are malformed before a display-mode refusal', async () => {
  const invalid = [
    ...['0x1', '1x0', '-1x1', '1.5x1', '1x1.5', `${'9'.repeat(400)}x1`].map(segments => ({ segments })),
    ...[-1, NaN, Infinity, 'banana'].map(gap => ({ segments: '2x1', gap })),
  ];
  for (const facts of invalid) {
    const error = await (async () => preferWeb(preferGroups({ 'display-mode': 'fullscreen', ...facts })))().catch(e => e);
    expect(error).toBeInstanceOf(MalformedOperation);
    expect(error.malformed).toBe(true);
  }
  expect(preferGroups({ segments: '1x1', gap: 0 }).fold).toEqual({ cols: 1, rows: 1, gap: 0 });
  expect(preferGroups({ segments: '2x3', gap: 0.5 }).fold).toEqual({ cols: 2, rows: 3, gap: 0.5 });
});

test('prefer mode is the app modes group (LLP 1121 D13): each pin, the scheme left alone', () => {
  const { media, mode } = preferGroups({ mode: 'dark-style=dim, density=compact' });
  expect(mode).toEqual({ 'dark-style': 'dim', density: 'compact' });
  expect(media).toEqual({});
  expect(() => preferGroups({ mode: 'dim' })).toThrow(/mode <axis>=<value>/);
  expect(() => preferGroups({ mode: 'dark-style:dim' })).toThrow(/mode <axis>=<value>/);
});

test('page preference refusals omit the malformed property', () => {
  const { prefer } = pageReporter(true);
  try { prefer({ 'keyboard-navigation': true }); throw new Error('expected a refusal'); }
  catch (error) { expect(error.message).toContain('not a page fact'); expect(Object.hasOwn(error, 'malformed')).toBe(false); }
  try { prefer({ banana: true }); throw new Error('expected malformed input'); }
  catch (error) { expect(error.malformed).toBe(true); }
});

test('pointer and hover are media facts, emulated in Chrome by its touch screen (LLP 1132 D10)', async () => {
  expect(preferGroups({ pointer: 'coarse', hover: 'none' }).media).toEqual({ pointer: 'coarse', hover: 'none' });
  expect(() => preferGroups({ pointer: 'pen' })).toThrow(/^prefer: pointer pen: expected .*pointer coarse\|fine\|none/);
  const drive = media => {
    const calls = [];
    const done = preferWeb({ media, page: {}, emulated: {}, call: async (method, params) => calls.push([method, params]), frame: async () => {}, evaluate: async () => ({}) });
    return done.then(() => calls, error => { throw Object.assign(error, { calls }); });
  };
  expect(await drive({ pointer: 'coarse' })).toEqual([['Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 }]]);
  expect(await drive({ pointer: 'fine', hover: 'hover' })).toEqual([['Emulation.setTouchEmulationEnabled', { enabled: false }]]);
  expect(await drive({ hover: 'none', 'prefers-color-scheme': 'dark' })).toEqual([
    ['Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 }],
    ['Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] }],
  ]);
  for (const media of [{ pointer: 'none' }, { pointer: 'coarse', hover: 'hover' }, { pointer: 'fine', hover: 'none' }]) {
    const error = await drive(media).catch(e => e);
    expect(error.message).toMatch(/^Chrome prefer cannot emulate .*: its touch emulation pairs pointer coarse with hover none/);
    expect(error.calls).toEqual([]);
  }
});

test('a media prefer waits in the page for each query it flips before the next step (the authored media flake)', async () => {
  // preferWeb: arm the page's wait, emulate, await that wait, then the frames; a document gone meanwhile is said.
  const steps = [];
  const drive = (heard = true) => preferWeb({ media: { 'prefers-reduced-motion': 'reduce' }, page: {}, emulated: {},
    call: async (method) => steps.push(method), frame: async () => steps.push('frame'),
    evaluate: async (expression) => { steps.push(expression.includes('__exactMediaHeard?.[') ? 'await' : expression.includes('function mediaHeard') ? 'arm' : 'read'); return expression.includes('__exactMediaHeard?.[') ? (heard ? true : 'gone') : {}; } });
  await drive();
  expect(steps).toEqual(['arm', 'Emulation.setEmulatedMedia', 'await', 'frame', 'read']);
  await expect(drive(false)).rejects.toThrow('prefer: the page changed documents while its media changed: prefer again');

  // mediaHeard, on a stand-in page whose queries flip when told: only queries whose truth changes are waited on,
  // a range feature's lower values included (color-gamut p3 to srgb flips p3's), and none at all is no wait.
  const saved = { matchMedia: globalThis.matchMedia, heard: globalThis.__exactMediaHeard };
  const matches = new Map(), lists = new Map();
  globalThis.matchMedia = (query) => {
    if (!lists.has(query)) { const listeners = new Set(); lists.set(query, { get matches() { return matches.get(query) ?? false; }, addEventListener: (_, f) => listeners.add(f), removeEventListener: (_, f) => listeners.delete(f), listeners }); }
    return lists.get(query);
  };
  const flip = (query, value) => { matches.set(query, value); for (const f of [...lists.get(query)?.listeners ?? []]) f(); };
  try {
    matches.set('(prefers-reduced-motion: no-preference)', true);
    matches.set('(color-gamut: srgb)', true); matches.set('(color-gamut: p3)', true);
    mediaHeard({ 'prefers-reduced-motion': 'reduce', 'color-gamut': 'srgb' }, PREFERENCES, 'a');
    expect([...lists.values()].filter(l => l.listeners.size).length).toBe(3); // reduce, no-preference, p3
    let done = false;
    globalThis.__exactMediaHeard.a.then(() => { done = true; });
    flip('(prefers-reduced-motion: reduce)', true); flip('(prefers-reduced-motion: no-preference)', false);
    await Bun.sleep(1);
    expect(done).toBe(false); // p3's query has not been heard yet
    flip('(color-gamut: p3)', false);
    await Bun.sleep(1);
    expect(done).toBe(true);
    expect([...lists.values()].every(l => l.listeners.size === 0)).toBe(true);
    // Already so: nothing to hear, settled at once.
    mediaHeard({ 'prefers-reduced-motion': 'reduce', 'color-gamut': 'srgb' }, PREFERENCES, 'b');
    expect(await globalThis.__exactMediaHeard.b).toBe(true);
  } finally { globalThis.matchMedia = saved.matchMedia; globalThis.__exactMediaHeard = saved.heard; }
});
