import { describe, expect, test } from 'bun:test';
import { citationJump, citedRow, revealCitation } from '../../src/timeline/citation-source';
import { T3Client } from '../../src/client/client';
import { chatLocal, timelineSnapshot, timelineView } from '../../src/timeline/timeline-presentation';
import type { Native } from '../../src/client/protocol';

const rows = [
  { id: '["t1","u1"]', kind: 'user', body: 'question' },
  { id: '["t1","a1"]', kind: 'assistant', body: 'answer one' },
  { id: '["t1","u2"]', kind: 'user', body: 'a1 again' },
  { id: '["t1","a2"]', kind: 'assistant', body: 'answer two' },
];

describe('citation View source (AssistantCitationSource)', () => {
  test('the cited answer is an assistant row, never a user row that mentions its id', () => {
    expect(citedRow(rows, 'a2')).toBe(3);
    expect(citedRow(rows, 'a1')).toBe(1);
    expect(citedRow(rows, 'u1')).toBe(-1);
    expect(citedRow(rows, '')).toBe(-1);
    const served = [{ id: '["t","turn-item:message:u"]', kind: 'user', body: '' }, { id: '["t","turn-item:provider:codex:native-item:7a52"]', kind: 'assistant', body: '' }];
    expect(citedRow(served, 'message:provider:codex:native-item:7a52')).toBe(1);
    expect(citedRow(served, 'turn-item:provider:codex:native-item:7a52')).toBe(1);
    expect(citedRow(served, 'message:provider:codex:native-item:7a5')).toBe(-1);
  });
  test('the jump names the cited answer\'s row by the list\'s key', () => {
    expect(citationJump(rows, 3)).toBe('["t1","a2"]');
  });
  test('a citation on the open thread is placed at once; one whose thread is gone fails', async () => {
    const native = { available: true, watch() {}, async later(request: Record<string, unknown>) { return { ok: true, generation: request.generation, value: {} }; } } as unknown as Native;
    const client = new T3Client();
    Object.assign(client, { threadId: 't1', connection: 'connected' });
    client.shell.threads = [{ id: 't1' }] as typeof client.shell.threads;
    const placed: string[] = [];
    await revealCitation(client, native, 't1', 'a1', () => rows, key => placed.push(key));
    expect(placed).toEqual(['["t1","a1"]']);
    await expect(revealCitation(client, native, 'gone', 'a1', () => rows, key => placed.push(key))).rejects.toThrow('Thread no longer available');
  });
  // While View source places its answer the snapshot says so, and ChatColumn restores
  // no remembered position over it (timeline-anchor.test.ts); the flag clears once it lands or fails.
  test('the snapshot reports a citation in flight, and only then', async () => {
    const client = new T3Client();
    Object.assign(client, { threadId: 't1', connection: 'connected' });
    client.shell.threads = [{ id: 't1' }] as typeof client.shell.threads;
    timelineView(client).lastRows = { threadId: 't1', rows };
    const native = { available: true, watch() {}, async later(request: Record<string, unknown>) {
      return { ok: true, generation: request.generation, value: {} };
    } } as unknown as Native;
    expect(timelineSnapshot(client).citing).toBe(false);
    expect(timelineSnapshot(client).citeJump).toEqual({ key: '', nonce: 0 });
    await chatLocal(client, native, 'chip-citation', 't1', 'a2');
    // The request lands in the snapshot (ChatColumn citeBegin places it); a second one is a new nonce.
    expect([timelineSnapshot(client).citeJump, timelineSnapshot(client).citing]).toEqual([{ key: '["t1","a2"]', nonce: 1 }, false]);
    await chatLocal(client, native, 'chip-citation', 't1', 'a2');
    expect(timelineSnapshot(client).citeJump.nonce).toBe(2);
    client.threadId = 't2';
    expect(timelineSnapshot(client).citeJump).toEqual({ key: '', nonce: 0 });
    client.threadId = 't1';
    await expect(chatLocal(client, native, 'chip-citation', 'gone', 'a2')).rejects.toThrow('Thread no longer available');
    expect(timelineSnapshot(client).citing).toBe(false);
  });
});

describe('a quote that no longer reads in its answer', () => {
  const href = (text: string) => `t3-citation://v1/env/t/message:provider:codex:native-item:a7?text=${encodeURIComponent(text)}&start=0&end=4`;
  const rows = (quote: string) => [
    { id: '["t","turn-item:provider:codex:native-item:a7"]', kind: 'assistant', body: 'History **fixture** turn 7 complete.' },
    { id: '["t","turn-item:message:u9"]', kind: 'user', body: `[Assistant quote](${href(quote)}) fixture history` },
  ];
  test('the quote still reads (Markdown aside): no warning; a changed quote warns', async () => {
    const { quoteChanged } = await import('../../src/timeline/citation-source');
    expect(quoteChanged(rows('fixture turn 7'), 0, 'message:provider:codex:native-item:a7')).toBe(false);
    expect(quoteChanged(rows('History fixture turn 6 complete.'), 0, 'message:provider:codex:native-item:a7')).toBe(true);
    expect(quoteChanged(rows('anything'), 0, 'message:provider:codex:native-item:zz')).toBe(false);
  });
});
