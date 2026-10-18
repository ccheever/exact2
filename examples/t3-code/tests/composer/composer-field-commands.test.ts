import { describe, expect, test } from 'bun:test';
import { ComposerFieldCommands, type ComposerFieldSnapshot } from '../../src/composer/composer-field-commands';

const identity = { generation: 1, snapshotOwner: 'snapshot', owner: 'thread' };
const snapshot: ComposerFieldSnapshot = { ...identity, value: 'one', start: 3, end: 3, composing: false, live: true };
const edit = { ...identity, value: 'one', start: 3, end: 3, text: '!', selectionStart: 4, selectionEnd: 4 };
function setup() { const commands = new ComposerFieldCommands(); commands.observe(snapshot, false); return commands; }

describe('composer shared field commands', () => {
  test('pending resource wakes and input can arrive before independent acknowledgement', async () => {
    const commands = setup(), waiting = commands.wait(0, identity), pending = commands.request(edit);
    const command = await waiting;
    expect(command).toEqual({ ...edit, focus: true, then: '', history: 0, citation: 0, completion: 0, seq: 1, ready: true });
    commands.observe({ ...snapshot, value: 'one!', start: 4, end: 4 }, true);
    const result = commands.acknowledge(1, identity, { value: 'one!', start: 4, end: 4, composing: false }, true);
    expect(await pending).toEqual(result);
    expect(result).toEqual({ applied: true, reason: '', value: 'one!', start: 4, end: 4 });
  });
  test('an ACK cannot invent input, while later typing cannot erase an observed applied edit', async () => {
    const commands = setup(), pending = commands.request(edit);
    const fabricated = commands.acknowledge(1, identity, { value: 'one!', start: 4, end: 4, composing: false }, true);
    expect(fabricated.applied).toBe(false);
    expect((await pending).applied).toBe(false);
    const applied = commands.request(edit);
    await commands.wait(1, identity);
    commands.observe({ ...snapshot, value: 'one!', start: 4, end: 4 }, true);
    commands.observe({ ...snapshot, value: 'one!x', start: 5, end: 5 }, true);
    commands.acknowledge(2, identity, { value: 'one!x', start: 5, end: 5, composing: false }, true);
    expect((await applied).applied).toBe(true);
  });
  test('one delivered edit cannot acknowledge another queued edit of the same source', async () => {
    const commands = setup(), first = commands.request(edit), queued = commands.request(edit);
    await commands.wait(0, identity);
    const nextResource = commands.wait(1, identity);
    expect(nextResource).toBeInstanceOf(Promise);
    commands.observe({ ...snapshot, value: 'one!', start: 4, end: 4 }, true);
    expect((await queued).reason).toBe('field source changed');
    commands.acknowledge(1, identity, { value: 'one!', start: 4, end: 4, composing: false }, true);
    expect((await first).applied).toBe(true);
    commands.dispose();
    expect((await nextResource).ready).toBe(false);
  });
  test('a new-source queued edit wakes after the previous ACK without borrowing its input', async () => {
    const commands = setup(), first = commands.request(edit);
    await commands.wait(0, identity);
    commands.observe({ ...snapshot, value: 'one!', start: 4, end: 4 }, true);
    const second = commands.request({ ...edit, value: 'one!', start: 4, end: 4, text: 'x', selectionStart: 5, selectionEnd: 5 });
    const waiting = commands.wait(1, identity);
    expect(waiting).toBeInstanceOf(Promise);
    commands.acknowledge(1, identity, { value: 'one!', start: 4, end: 4, composing: false }, true);
    expect((await first).applied).toBe(true);
    expect((await waiting).seq).toBe(2);
    commands.observe({ ...snapshot, value: 'one!x', start: 5, end: 5 }, true);
    commands.acknowledge(2, identity, { value: 'one!x', start: 5, end: 5, composing: false }, true);
    expect((await second).applied).toBe(true);
  });
  test('moved/focus/resource observations cannot invent no-op input and ACK retains the post-selection caret', async () => {
    const commands = setup(), noop = { ...edit, text: '', selectionStart: 3, selectionEnd: 3 };
    const refused = commands.request(noop);
    await commands.wait(0, identity);
    commands.observe(snapshot, false);
    expect(commands.acknowledge(1, identity, { value: 'one', start: 3, end: 3, composing: false }, true).applied).toBe(false);
    expect((await refused).applied).toBe(false);
    const applied = commands.request(noop, { focus: false, then: 'models' });
    expect(await commands.wait(1, identity)).toMatchObject({ seq: 2, focus: false, then: 'models' });
    commands.observe(snapshot, true);
    commands.observe({ ...snapshot, start: 1, end: 1 }, false);
    commands.acknowledge(2, identity, { value: 'one', start: 1, end: 1, composing: false }, true);
    expect(await applied).toEqual({ applied: true, reason: '', value: 'one', start: 1, end: 1 });
    const copy = commands.read();
    if (copy) copy.start = 0;
    expect(commands.read()?.start).toBe(1);
  });
  test('guards identity, source, composition and source/output ranges', async () => {
    const commands = setup();
    for (const stale of [{ ...edit, owner: 'other' }, { ...edit, snapshotOwner: 'other' }, { ...edit, generation: 2 }, { ...edit, value: 'two' }]) {
      expect((await commands.request(stale)).reason).toBe('field source changed');
    }
    for (const invalid of [{ ...edit, start: -1 }, { ...edit, end: 9 }, { ...edit, selectionEnd: 9 }, { ...edit, start: 1.5 }]) {
      expect((await commands.request(invalid)).reason).toBe('field range invalid');
    }
    commands.observe({ ...snapshot, composing: true }, false);
    expect((await commands.request(edit)).reason).toBe('field composing');
  });
  test('source changes reject stale requests and never deliver them again', async () => {
    const commands = setup(), pending = commands.request(edit);
    commands.observe({ ...snapshot, value: 'typed' }, true);
    expect((await pending).reason).toBe('field source changed');
    const waiting = commands.wait(0, identity);
    commands.dispose();
    expect((await waiting).ready).toBe(false);
  });
  test('bounded requests, owner reset and disposal complete every pending caller', async () => {
    const commands = setup(), pending = Array.from({ length: 16 }, () => commands.request(edit));
    expect((await commands.request(edit)).reason).toBe('field request queue full');
    commands.observe({ ...snapshot, generation: 2 }, false);
    expect((await Promise.all(pending)).every(result => result.reason === 'field owner changed')).toBe(true);
    const waiting = commands.wait(0, { ...identity, generation: 2 });
    commands.dispose();
    expect((await waiting).ready).toBe(false);
    expect((await commands.request(edit)).reason).toBe('field unavailable');
  });
  test('replaced resource retires its previous wait and stale ACK cannot complete another request', async () => {
    const commands = setup(), old = commands.wait(0, identity), current = commands.wait(0, identity);
    expect((await old).ready).toBe(false);
    const pending = commands.request(edit), command = await current;
    expect(commands.acknowledge(command.seq, { ...identity, owner: 'other' }, { value: 'one!', start: 4, end: 4, composing: false }, true).reason).toBe('field acknowledgement stale');
    commands.dispose();
    expect((await pending).reason).toBe('field disposed');
  });
});
