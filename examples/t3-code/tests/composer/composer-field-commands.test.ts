import { describe, expect, test } from 'bun:test';
import { ComposerFieldCommands, type ComposerFieldSnapshot } from '../../src/composer/composer-field-commands';

const identity = { generation: 1, snapshotOwner: 'snapshot', owner: 'thread' };
const snapshot: ComposerFieldSnapshot = { ...identity, value: 'one', start: 3, end: 3, composing: false, live: true };
const edit = { ...identity, value: 'one', start: 3, end: 3, text: '!', selectionStart: 4, selectionEnd: 4 };
function setup() { const commands = new ComposerFieldCommands(); commands.observe(snapshot, false); return commands; }
const entered = { ...snapshot, value: 'one!', start: 4, end: 4 };

describe('composer shared field commands', () => {
  test('idle read and admission finish synchronously; queued is never applied', () => {
    const commands = setup();
    expect(commands.wait(0, identity)).toMatchObject({ ready: false });
    expect(commands.request(edit)).toEqual({ accepted: true, queued: true, applied: false, seq: 1, reason: '' });
    expect(commands.wait(0, identity)).toMatchObject({ ...edit, ready: true, seq: 1 });
    expect(commands.drain()).toEqual([]);
    commands.observe(entered, true);
    expect(commands.acknowledge(1, identity, entered, true).applied).toBe(true);
    expect(commands.drain()).toEqual([{ command: expect.objectContaining({ seq: 1 }), result: { applied: true, reason: '', value: 'one!', start: 4, end: 4 } }]);
    expect(commands.drain()).toEqual([]);
  });
  test('an ACK cannot invent input; subsequent typing cannot erase independently observed input', () => {
    const commands = setup(); commands.request(edit); commands.wait(0, identity);
    expect(commands.acknowledge(1, identity, entered, true).applied).toBe(false); commands.drain();
    commands.request(edit); commands.wait(1, identity); commands.observe(entered, true);
    const later = { ...entered, value: 'one!x', start: 5, end: 5 }; commands.observe(later, true);
    expect(commands.acknowledge(2, identity, later, true)).toMatchObject({ applied: true, value: later.value });
  });
  test('only one delivered edit can borrow an input and stale queued edits are refused once', () => {
    const commands = setup(); commands.request(edit); commands.request(edit); commands.wait(0, identity);
    expect(commands.wait(1, identity).ready).toBe(false);
    commands.observe(entered, true);
    expect(commands.drain()).toEqual([{ command: expect.objectContaining({ seq: 2 }), result: expect.objectContaining({ applied: false, reason: 'field source changed' }) }]);
    expect(commands.acknowledge(1, identity, entered, true).applied).toBe(true);
    expect(commands.wait(1, identity).ready).toBe(false);
  });
  test('next-source metadata is readable after ACK without a future-turn promise', () => {
    const commands = setup(); commands.request(edit); commands.wait(0, identity); commands.observe(entered, true);
    commands.request({ ...edit, value: 'one!', start: 4, end: 4, text: 'x', selectionStart: 5, selectionEnd: 5 });
    expect(commands.wait(1, identity).ready).toBe(false);
    commands.acknowledge(1, identity, entered, true);
    expect(commands.wait(1, identity)).toMatchObject({ seq: 2, ready: true });
    const next = { ...entered, value: 'one!x', start: 5, end: 5 }; commands.observe(next, true);
    expect(commands.acknowledge(2, identity, next, true).applied).toBe(true);
  });
  test('metadata-only observations cannot invent no-op input', () => {
    const commands = setup(); commands.request({ ...edit, text: '', selectionStart: 3, selectionEnd: 3 }); commands.wait(0, identity);
    commands.observe(snapshot, false);
    expect(commands.acknowledge(1, identity, snapshot, true).applied).toBe(false);
    commands.request({ ...edit, text: '', selectionStart: 3, selectionEnd: 3 }); commands.wait(1, identity); commands.observe(snapshot, true);
    commands.observe({ ...snapshot, start: 1, end: 1 }, false);
    expect(commands.acknowledge(2, identity, { ...snapshot, start: 1, end: 1 }, true)).toMatchObject({ applied: true, start: 1 });
  });
  test('identity/source/IME/range guards refuse admission synchronously', () => {
    const commands = setup();
    for (const stale of [{ ...edit, owner: 'other' }, { ...edit, snapshotOwner: 'other' }, { ...edit, generation: 2 }, { ...edit, value: 'two' }]) expect(commands.request(stale).reason).toBe('field source changed');
    for (const invalid of [{ ...edit, start: -1 }, { ...edit, end: 9 }, { ...edit, selectionEnd: 9 }, { ...edit, start: 1.5 }]) expect(commands.request(invalid).reason).toBe('field range invalid');
    commands.observe({ ...snapshot, composing: true }, false); expect(commands.request(edit).reason).toBe('field composing');
  });
  test('bounded16, source/owner reset, cancellation and disposal drain each refusal once', () => {
    const commands = setup(); for (let i = 0; i < 16; i++) commands.request(edit);
    expect(commands.request(edit).reason).toBe('field request queue full');
    commands.observe({ ...snapshot, generation: 2 }, false);
    expect(commands.drain()).toHaveLength(16); expect(commands.drain()).toEqual([]);
    commands.observe(snapshot, false); const queued = commands.request(edit); commands.cancel(queued.seq, 'wake failed');
    expect(commands.drain()[0]?.result.reason).toBe('wake failed');
    commands.dispose(); expect(commands.wait(0, identity).ready).toBe(false); expect(commands.request(edit).accepted).toBe(false);
  });
  test('stale ACK cannot complete another request', () => {
    const commands = setup(); commands.request(edit); commands.wait(0, identity);
    expect(commands.acknowledge(1, { ...identity, owner: 'other' }, entered, true).reason).toBe('field acknowledgement stale');
    expect(commands.drain()).toEqual([]); commands.dispose(); expect(commands.drain()[0]?.result.reason).toBe('field disposed');
  });
});
