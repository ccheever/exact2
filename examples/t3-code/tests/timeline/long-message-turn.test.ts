// A data turn over a thread whose messages hold one line of a million characters (presentation.ts snapshot): the
// line's image scan, chips, collapse check and compaction check read it with searches rather than character by
// character, and its minimap label is compacted once, so each turn stays well inside the runner's 100 ms budget.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { snapshot } from '../../src/client/presentation';
import type { Obj } from '../../src/client/domain';

const at = (seconds: number) => new Date(Date.parse('2026-10-03T00:00:00.000Z') + seconds * 1000).toISOString();
let ordinal = 0;
const item = (id: string, type: string, seconds: number, extra: Obj): Obj => ({ id, threadId: 't1', runId: 'r1', nodeId: null, ordinal: ++ordinal,
  status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra });

/** A connected client on a thread of one user message holding `line` and a reply that repeats it. */
function client(line: string): T3Client {
  const items = [item('u', 'user_message', 0, { text: line, messageId: 'm-u', inputIntent: 'turn_start', attachments: [], createdBy: 'user' }),
    item('a', 'assistant_message', 3, { text: `Stand-in reply: ${line}`, messageId: 'm-a', streaming: false })];
  const value = new T3Client();
  Object.assign(value, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, config: { providers: [] },
    shell: { projects: [{ id: 'p1', title: 'previews', workspaceRoot: '/work/project' }], threads: [{ id: 't1', projectId: 'p1', title: 'Long image line' }] } });
  value.thread = { projection: { thread: { id: 't1', projectId: 'p1', title: 'Long image line' }, runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(10) }],
    runtimeRequests: [], turnItems: items, visibleTurnItems: items.map((entry, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: entry.id, item: entry })) },
  sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  return value;
}

describe('a data turn over a thread of one long line', () => {
  test('takes a few milliseconds a turn, however many `![` the line holds', () => {
    // Each took about 35 ms a turn in Bun while the scans read the line character by character on every turn; the
    // app's Hermes took 109 to 126 ms for a quarter of this length, over the runner's budget, and refused the turn.
    for (const line of [`Unclosed images: ${'![a'.repeat(340_000)}`, `Unclosed images: ${'![a '.repeat(256_000)}`]) {
      const value = client(line);
      expect(snapshot(value, 0).messages.find(message => message.kind === 'user')?.body).toBe(line);
      const start = performance.now();
      for (let turn = 1; turn <= 5; turn++) snapshot(value, turn);
      const perTurn = (performance.now() - start) / 5;
      expect([line.slice(0, 21), perTurn < 15]).toEqual([line.slice(0, 21), true]);
    }
  });
});
