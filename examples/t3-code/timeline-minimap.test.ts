import { describe, expect, test } from 'bun:test';
import { jumpToTurn, minimapCurrent, minimapItems, nativeTurns } from './timeline-minimap';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';

const rows = [
  { id: 'u1', kind: 'user', body: 'First  question\nwith two lines' }, { id: 'w', kind: 'work', body: '' },
  { id: 'a1', kind: 'assistant', body: 'Draft' }, { id: 'a2', kind: 'assistant', body: '**Final** answer\n\nhere' },
  { id: 'u2', kind: 'user', body: '' }, { id: 'u3', kind: 'user', body: 'Third' }, { id: 'a3', kind: 'assistant', body: 'Done.' },
];
describe('timeline minimap', () => {
  test('one strip per user message, previewing the turn\'s final answer', () => {
    const items = minimapItems(rows, new Set(['u3']));
    expect(items.map(item => [item.id, item.index, item.label, item.excerpt, item.inView, item.row])).toEqual([
      ['u1', 0, 'First question with two lines', '**Final** answer here', false, 0], ['u2', 1, 'User message', '', false, 4], ['u3', 2, 'Third', 'Done.', true, 5]]);
  });
  test('the current turn is the first in view, else the last above the viewport', () => {
    expect(minimapCurrent(minimapItems(rows, new Set(['u2', 'u3'])), 'u1')).toBe(1);
    expect(minimapCurrent(minimapItems(rows, new Set()), 'u1')).toBe(0);
    expect(minimapCurrent(minimapItems(rows, new Set()), '')).toBe(-1);
  });
  test('the native status names turns on screen by their row ids', () => {
    const turns = nativeTurns({ presentation: { turnsInView: ['u2', 3, 'u3'], turnAbove: 'u1' } } as unknown as T3Client);
    expect([...turns.inView]).toEqual(['u2', 'u3']);
    expect(turns.above).toBe('u1');
  });
  test('jumping asks the transcript for the row by id, or by strip position for the chevrons', async () => {
    const calls: Obj[] = [];
    const client = { restAccess: () => ({ call: async (request: Obj) => { calls.push(request); return {}; } }) } as unknown as T3Client;
    await jumpToTurn(client, {} as Native, 'u2', '', rows);
    await jumpToTurn(client, {} as Native, '', '2', rows);
    expect(calls).toEqual([{ op: 'timelineJump', id: 'u2', index: 4, count: 7 }, { op: 'timelineJump', id: 'u3', index: 5, count: 7 }]);
    await expect(jumpToTurn(client, {} as Native, '', '9', rows)).rejects.toThrow('no longer in this thread');
  });
});
