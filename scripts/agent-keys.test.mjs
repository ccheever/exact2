// The driver's held key (agent-keys.mjs `typeFor`): it repeats while held, as
// a keyboard does (#140), each repeat a keydown whose `repeat` is true.
import { test, expect } from 'bun:test';
import { chordKeys } from './agent-playwright.mjs';
import { KEY_REPEAT, validateKey, typeFor, cdpKey, withHeldModifiers, browserKey, nativeKey, typeArguments, typeCommand, tapWords, intoArguments, MalformedOperation, clockSlices } from './agent-keys.mjs';
import { open } from './agent.mjs';
import { launchFacts } from './agent-launch.mjs';

test('tap midi is a MIDI input\'s message, its bytes two hex digits each; a testId `midi` keeps the other forms (LLP 1125 §4)', () => {
  expect(tapWords(['midi', 'keys', '90', '3c', '64'])).toEqual([null, { midi: { input: 'keys', data: [0x90, 0x3c, 0x64] } }]);
  expect(tapWords(['midi', 'at', '1', '2'])).toEqual(['midi', { at: [1, 2] }]);
  expect(tapWords(['midi'])).toEqual(['midi', {}]);
  // Its clock (D9): ticks in 50 ms slices, the last one short.
  expect(tapWords(['midi', 'clk', 'clock', '125', '110'])).toEqual([null, { midi: { input: 'clk', clock: { bpm: 125, ms: 110 } } }]);
  expect(tapWords(['midi', '"clk"', 'fa'])).toEqual([null, { midi: { input: 'clk', data: [0xfa] } }]);
  expect(() => tapWords(['midi', 'clk', 'clock', '0', '100'])).toThrow('a tempo above 0');
  expect(clockSlices({ bpm: 125, ms: 110 })).toEqual([{ to: 50, ticks: [20, 40] }, { to: 100, ticks: [60, 80, 100] }, { to: 110, ticks: [] }]);
});

test('into accepts CSS alignments and labels invalid values in the shared check and CLI', () => {
  expect(() => intoArguments({ key: 'row' })).not.toThrow();
  expect(tapWords(['list', 'into', 'row'])).toEqual(['list', { into: { key: 'row' } }]);
  for (const axis of ['block', 'inline']) {
    for (const alignment of ['start', 'center', 'end', 'nearest']) {
      const into = { key: 'row', [axis]: alignment };
      expect(() => intoArguments(into)).not.toThrow();
      expect(tapWords(['list', 'into', 'row', axis, alignment])).toEqual(['list', { into }]);
    }
    for (const alignment of ['banana', 'Start', 'toString', '', null, 0]) {
      const message = `no alignment ${alignment}: start, center, end or nearest`;
      expect(() => intoArguments({ [axis]: alignment })).toThrow(MalformedOperation);
      expect(() => intoArguments({ [axis]: alignment })).toThrow(message);
      const words = ['missing', 'into', 'row', axis, String(alignment)];
      const error = (() => { try { tapWords(words); } catch (e) { return e; } })();
      expect(error).toBeInstanceOf(MalformedOperation);
      expect(error.message).toBe(`tap ${words.join(' ')}: ${message}`);
    }
  }
  expect(tapWords(['list', 'into', 'row', 'block', 'end', 'inline', 'center']))
    .toEqual(['list', { into: { key: 'row', block: 'end', inline: 'center' } }]);
  expect(() => intoArguments({ block: { toString: null } })).toThrow(MalformedOperation);
});

test('tap validates both into alignments before looking up its target', async () => {
  const s = await open({ reuse: { host: 'web', browser: 'chrome', launchFacts: launchFacts({}), reset() {},
    ask() { throw new Error('malformed into reached a host'); } } });
  for (const axis of ['block', 'inline']) {
    const error = await s.tap('missing', { into: { key: 'row', [axis]: 'banana' } }).catch(e => e);
    expect(error).toBeInstanceOf(MalformedOperation);
    expect(error.message).toBe('no alignment banana: start, center, end or nearest');
  }
});

test('key phases and durations carry the malformed label; failed focus does not', async () => {
  for (const key of [browserKey, nativeKey]) await expect(key({ id: 2, opts: { key: 'Enter', phase: 'banana' } })).rejects.toBeInstanceOf(MalformedOperation);
  await expect(typeFor({ options: { key: 'Enter', for: -1 } })).rejects.toBeInstanceOf(MalformedOperation);
  expect(() => typeArguments(['field', 'key', 'Enter', 'for'])).toThrow(MalformedOperation);
  expect(() => tapWords(['strip', 'wheel', 'x', '0'])).toThrow(MalformedOperation);
  expect(() => typeCommand(String.raw`type "\q" text`)).toThrow(MalformedOperation);
  expect(() => cdpKey('toString')).toThrow(MalformedOperation);
  const refusal = await browserKey({ id: 2, opts: { key: 'Enter' }, evaluate: async () => false }).catch(e => e);
  expect(refusal.message).toBe('view 2 could not take focus');
  expect(refusal.malformed).toBeUndefined();
});

test('ContextMenu is a named physical key with no typed text', () => {
  expect(cdpKey('ContextMenu')).toEqual({ code: 'ContextMenu', key: 'ContextMenu', vk: 93, modifiers: 0, text: undefined, location: 0 });
  const event = { type: 'keyDown', code: 'ContextMenu', key: 'ContextMenu', windowsVirtualKeyCode: 93 };
  expect(withHeldModifiers('Input.dispatchKeyEvent', event, new Map()).type).toBe('rawKeyDown');
  expect(withHeldModifiers('Input.dispatchKeyEvent', { ...event, type: 'keyUp' }, new Map()).type).toBe('keyUp');
});

const hold = async (key, ms) => {
  const calls = [];
  const carrier = { input: async (id, kind, options) => { calls.push(`${options.phase}${options.repeat ? ' repeat' : ''}`); return { delivery: 'platform' }; } };
  await typeFor({ node: { id: 2 }, target: 'field', options: { key, for: ms }, carrier, clock: async spec => { calls.push(`clock ${spec}`); return {}; }, tagged: r => r });
  return calls;
};

test('a held key repeats on the virtual clock at macOS\'s default rate', async () => {
  expect(KEY_REPEAT).toEqual({ delay: 500, interval: 83 });
  expect(await hold('a', 1200)).toEqual(['down', 'clock +500', 'down repeat',
    ...Array.from({ length: 8 }, () => ['clock +83', 'down repeat']).flat(), 'clock +36', 'up']);
  // A chord repeats its key; a hold shorter than the delay repeats none.
  expect((await hold('Meta+w', 700)).filter(c => c === 'down repeat')).toHaveLength(3);
  expect(await hold('a', 500)).toEqual(['down', 'clock +500', 'up']);
});

test('a modifier alone does not repeat, as AppKit\'s flags change does not', async () => {
  for (const key of ['Meta', 'Shift', 'ControlRight', 'Alt+Shift']) expect(await hold(key, 1200)).toEqual(['down', 'clock +1200', 'up']);
  expect((await hold('Shift++', 600)).filter(c => c === 'down repeat')).toHaveLength(2);
});

test('key validation uses each browser carrier vocabulary without a target lookup', async () => {
  for (const carrier of [{ host: 'web', browser: 'chrome' }, { host: 'web', browser: 'firefox' }, { host: 'web', browser: 'webkit' }]) {
    await expect(validateKey('Banana', carrier)).rejects.toBeInstanceOf(MalformedOperation);
    for (const key of ['Enter', 'Shift+Enter', 'Meta+s', '+', 'Shift++']) await validateKey(key, carrier);
  }
  // A native host labels an unknown name in its own reply.
  for (const host of ['macos', 'ios', 'linux']) await validateKey('Banana', { host });
  for (const browser of ['firefox', 'webkit']) {
    await validateKey('F24', { host: 'web', browser });
    let error;
    try { chordKeys('F24'); } catch (e) { error = e; }
    expect(error.message).toContain('not on Playwright');
    expect(error.malformed).toBeUndefined();
  }
  expect(() => tapWords([])).toThrow('tap: needs a target');
});
