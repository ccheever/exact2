// The driver's held key (agent-keys.mjs `typeFor`): it repeats while held, as
// a keyboard does (#140), each repeat a keydown whose `repeat` is true.
import { test, expect } from 'bun:test';
import { KEY_REPEAT, typeFor } from './agent-keys.mjs';

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
