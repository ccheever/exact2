// The driver's held key (agent-keys.mjs `typeFor`): it repeats while held, as
// a keyboard does (#140), each repeat a keydown whose `repeat` is true.
import { test, expect } from 'bun:test';
import { chordKeys } from './agent-playwright.mjs';
import { KEY_REPEAT, validateKey, typeFor, cdpKey, withHeldModifiers, browserKey, nativeKey, typeArguments, typeCommand, tapWords, intoArguments, MalformedOperation, clockSlices, padOp, postCommand, postInto, resizeArguments } from './agent-keys.mjs';
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

test('a pad on a world canvas: tap stick and type key Gamepad… reach the world as its pad 0, held across requests (LLP 1127.003 D3)', async () => {
  expect(tapWords(['world', 'stick', 'left', '0.7', '-0.2'])).toEqual(['world', { stick: 'left', value: [0.7, -0.2] }]);
  expect(tapWords(['world', 'stick', 'right', '1', '0', 'for', '1500'])).toEqual(['world', { stick: 'right', value: [1, 0], for: 1500 }]);
  for (const words of [['world', 'stick', 'middle', '0', '0'], ['world', 'stick', 'left', '1'], ['world', 'stick', 'left', '1', '0', 'over', '5']]) expect(() => tapWords(words)).toThrow(MalformedOperation);
  const sent = [];
  const s = {
    target: async () => ({ id: 7 }),
    op: async (req) => { sent.push(req); return { pad: 0, delivery: 'synthesized' }; },
    clock: async (spec) => { sent.push(spec); return {}; },
    landed: (r) => r,
  };
  const r = await padOp({ op: 'type', s, target: 'world', opts: { key: 'GamepadSouth', for: 100 }, host: 'linux', timing: 'agent' });
  expect(sent).toEqual([{ op: 'type', id: 7, world: true, key: 'GamepadSouth', phase: 'down' }, '+100', { op: 'type', id: 7, world: true, key: 'GamepadSouth', phase: 'up' }]);
  expect([r.typed, r.for, r.delivery]).toEqual([7, 100, 'synthesized']);
  sent.length = 0;
  await padOp({ op: 'tap', s, target: 'world', opts: { stick: 'left', value: [1, 0], for: 50 }, host: 'web', timing: 'agent' });
  expect(sent).toEqual([{ op: 'tap', id: 7, world: true, stick: 'left', value: [1, 0] }, '+50', { op: 'tap', id: 7, world: true, stick: 'left', value: [0, 0] }]);
  await expect(padOp({ op: 'tap', s, target: 'world', opts: { stick: 'left', value: [2, 0] } })).rejects.toBeInstanceOf(MalformedOperation);
});

test('a held contact\'s phase after a target says the phase takes none (euxy port: `tap dice-key up`)', () => {
  expect(() => tapWords(['dice-key', 'up'], true)).toThrow("a held contact's phases take no target: `tap up`");
  expect(() => tapWords(['dice-key', 'hold', '300'], true)).toThrow('`tap hold 300`');
  expect(() => tapWords(['dice-key', 'up'], false)).toThrow('`tap <target> down`, then `tap up`');
  expect(tapWords(['up'], true)).toEqual([null, { phase: 'up' }]);
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

test('a wheel takes one of gesture, fling, hold and release, and release is 0 0 alone (LLP 1142 D3)', () => {
  expect(tapWords(['list', 'wheel', '0', '-2000', 'fling'])).toEqual(['list', { wheel: [0, -2000], fling: true }]);
  expect(tapWords(['list', 'wheel', '0', '900', 'hold', 'at', '10', '20'])).toEqual(['list', { wheel: [0, 900], hold: true, at: [10, 20] }]);
  expect(tapWords(['list', 'wheel', '0', '0', 'release'])).toEqual(['list', { wheel: [0, 0], release: true }]);
  for (const words of [['fling', 'hold'], ['gesture', 'fling'], ['fling', 'modifiers', 'Control']]) expect(() => tapWords(['list', 'wheel', '0', '10', ...words])).toThrow(MalformedOperation);
  expect(() => tapWords(['list', 'wheel', '0', '10', 'release'])).toThrow(MalformedOperation);
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

test('a held state is a form of tap, by its own word (LLP 1122 D8)', () => {
  expect(tapWords(['post', 'hold-state', 'active'])).toEqual(['post', { holdState: 'active' }]);
  expect(tapWords(['post', 'hold-state', 'none'])).toEqual(['post', { holdState: null }]);
  expect(() => tapWords(['post', 'hold-state', 'pressed'])).toThrow(/hold-state takes/);
});

test('typed host refusals precede driver frames for arbitrary messages and native keys', async () => {
  const {hostReply, refusalMessage} = await import('./agent-keys.mjs');
  const reply = {error:'new refusal', refused:{host:'linux', subsystem:'gpu', message:'module blue: digest mismatch'}};
  expect(refusalMessage(reply)).toBe('host linux refused (gpu): module blue: digest mismatch');
  expect(refusalMessage({error:'GPU digest mismatch'})).toBeNull();
  try { hostReply(reply); throw new Error('accepted'); }
  catch (error) { expect(error.reply).toBe(reply); expect(error.stack.split('\n')[0]).toBe(refusalMessage(reply)); }
  await expect(nativeKey({id:2, opts:{key:'Enter'}, ask:async () => reply})).rejects.toThrow(refusalMessage(reply));
});

test('post is type <surface> message <text>: one word or one quoted string, the surface optional (LLP 1139.004 D1)', async () => {
  expect(postCommand('post spawn-gulper')).toEqual([null, { message: 'spawn-gulper' }]);
  expect(postCommand('post world "spawn gulper 3"')).toEqual(['world', { message: 'spawn gulper 3' }]);
  expect(postCommand("post 'spawn gulper'")).toEqual([null, { message: 'spawn gulper' }]);
  expect(typeCommand('type world message spawn gulper')).toEqual(['world', { message: 'spawn gulper' }]);
  expect(() => postCommand('post world spawn gulper')).toThrow('quote text of several words');
  expect(() => postCommand('post')).toThrow(MalformedOperation);
  // The wire's `type` with a `message`: a canvas the tree finds by its id, else the name as a surface's, else none.
  const sent = [], s = { find: async (t) => t === 'tank' ? { id: 7, world: { tick: 0 } } : t === 'title' ? { id: 3 } : undefined,
    op: async (req) => (sent.push(req), { posted: req.surface ?? 'world', delivery: 'posted' }), landed: async (r) => r };
  for (const target of [null, 'tank', 'title', 'ocean']) await postInto(s, target, 'spawn', { host: 'web', timing: 'agent' });
  expect(sent).toEqual([{ op: 'type', message: 'spawn' }, { op: 'type', message: 'spawn', id: 7 }, { op: 'type', message: 'spawn', surface: 'title' }, { op: 'type', message: 'spawn', surface: 'ocean' }]);
  await expect(postInto({ ...s, held: 'a drag' }, null, 'spawn', {})).rejects.toThrow('finger is down');
});

test('resize takes <w>x<h> and <w> <h>, and its refusal names both (LLP 1139.004 D4)', () => {
  expect(resizeArguments(['1280x800'])).toEqual([1280, 800]);
  expect(resizeArguments(['1280', '800'])).toEqual([1280, 800]);
  for (const words of [['1280'], ['1280', 'by', '800'], ['1280x'], []]) expect(() => resizeArguments(words)).toThrow('resize <w>x<h> or resize <w> <h>');
});
