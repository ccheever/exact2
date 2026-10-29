import { describe, expect, test } from 'bun:test';
import { citationJump, citedRow, revealCitation } from '../../src/timeline/citation-source';
import { toasts } from '../../src/shell/toast';
import { T3Client } from '../../src/client/client';
import { chatLocal, openCitedFold, timelineSnapshot, timelineView, transcriptRows } from '../../src/timeline/timeline-presentation';
import type { Native } from '../../src/client/protocol';

const rows = [
  { id: '["t1","u1"]', kind: 'user', body: 'question' },
  { id: '["t1","a1"]', kind: 'assistant', body: 'answer one' },
  { id: '["t1","u2"]', kind: 'user', body: 'a1 again' },
  { id: '["t1","a2"]', kind: 'assistant', body: 'answer two' },
];
/** The open thread's detail, holding the turn items `ids` (assistant answers, `u…` the user's) and older pages behind it. */
const loaded = (ids: string[], hasMore = false) => ({ projection: { visibleTurnItems: ids.map(id => ({ sourceThreadId: 't1', sourceItemId: id,
  item: { id, messageId: id, type: id.startsWith('u') ? 'user_message' : 'assistant_message' } })) }, hasMore, historyCursor: hasMore ? 'c0' : null }) as unknown as T3Client['thread'];

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
    Object.assign(client, { threadId: 't1', connection: 'connected', thread: loaded(['u1', 'a1', 'u2', 'a2']) });
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
    Object.assign(client, { threadId: 't1', connection: 'connected', thread: loaded(['u1', 'a1', 'u2', 'a2']) });
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
  // A newer View source lets the older go where it waits, on its own timer, so the older one's `finally` never runs.
  test('a newer View source clears the flag the one it replaced left set', async () => {
    const client = new T3Client();
    Object.assign(client, { threadId: 't1', connection: 'connected', thread: loaded(['u1', 'a1', 'u2', 'a2']) });
    client.shell.threads = [{ id: 't1' }, { id: 't2' }] as typeof client.shell.threads;
    timelineView(client).lastRows = { threadId: 't1', rows };
    const native = { available: true, watch() {}, async later(request: Record<string, unknown>) { return { ok: true, generation: request.generation, value: {} }; } } as unknown as Native;
    client.openSelected = () => new Promise(() => {}); // the replaced one never gets past opening its thread
    void chatLocal(client, native, 'chip-citation', 't2', 'b1');
    expect(timelineSnapshot(client).citing).toBe(true);
    await chatLocal(client, native, 'chip-citation', 't1', 'a2');
    expect([timelineSnapshot(client).citing, timelineSnapshot(client).citeJump.key]).toEqual([false, '["t1","a2"]']);
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

// useAssistantCitationTarget at T3 Code 74541bab17 (#18423): a cited answer a turn fold hides opens that fold, by the
// fold's own key; an imported turn has no run id and folds under `runless:<prompt>`.
describe('a cited answer inside a folded turn', () => {
  const at = (seconds: number) => new Date(Date.parse('2026-10-03T00:00:00.000Z') + seconds * 1000).toISOString();
  function folded(runId: string | null) {
    const item = (id: string, type: string, seconds: number, extra: Record<string, unknown>) =>
      ({ id, threadId: 't1', runId, nodeId: null, ordinal: seconds, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra });
    const items = [
      item('prompt', 'user_message', 0, { text: 'Investigate the issue.', messageId: 'm-prompt', inputIntent: 'turn_start', attachments: [], createdBy: 'user' }),
      item('source', 'assistant_message', 2, { text: 'The imported response contains this cited detail.', messageId: 'm-source', streaming: false }),
      item('answer', 'assistant_message', 9, { text: 'Investigation complete.', messageId: 'm-answer', streaming: false }),
    ];
    const projection = { thread: { id: 't1', projectId: 'p1' }, runs: runId ? [{ id: runId, status: 'completed', startedAt: at(0), completedAt: at(10) }] : [],
      attempts: [], nodes: [], checkpoints: [], visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) };
    const thread = { projection };
    const value = { threadId: 't1', projectId: 'p1', shell: { projects: [{ id: 'p1', workspaceRoot: '/work' }], threads: [{ id: 't1' }] },
      config: { providers: [] }, local: { deviceSettings: { timestampFormat: '24-hour' } }, thread, projection,
      restAccess: () => ({ call: async () => ({}) }) } as unknown as T3Client;
    return value;
  }
  const drawn = (value: T3Client) => transcriptRows(value).map(row => ({ id: row.id, kind: row.kind, body: String(row.body ?? '') }));
  test.each([['imported runless', null], ['standard', 'citation-run']] as const)('a %s turn opens, then its answer is placed', async (_label, runId) => {
    const value = folded(runId);
    expect(drawn(value).map(row => row.kind)).toEqual(['user', 'work', 'assistant']);
    expect(citedRow(drawn(value), 'message:source')).toBe(-1);
    const wakes: string[] = [];
    timelineView(value).lastRows = { threadId: 't1', rows: drawn(value) };
    // The window's data source reads the rows again when woken.
    const native = { available: true, watch() {}, async later(request: Record<string, unknown>) {
      if (request.op === 'wake') { wakes.push(String(request.topic)); timelineView(value).lastRows = { threadId: 't1', rows: drawn(value) }; }
      return { ok: true, generation: request.generation, value: {} };
    } } as unknown as Native;
    await chatLocal(value, native, 'chip-citation', 't1', 'message:source');
    expect(wakes).toEqual(['t3.notify']);
    expect(timelineSnapshot(value).citeJump.key).toBe('["t1","source"]');
    expect(drawn(value).map(row => row.kind)).toEqual(['user', 'work', 'assistant', 'assistant']);
    expect(openCitedFold(value, 'message:source')).toBe(false); // already open
    expect(openCitedFold(value, 'message:prompt')).toBe(false); // not an assistant answer
  });
});

// citationProcedure.ts loadCitedHistory (#25371): an answer outside the loaded history pages toward it.
describe('a citation to an answer not yet loaded', () => {
  const native = { available: true, watch() {}, async later(request: Record<string, unknown>) { return { ok: true, generation: request.generation, value: {} }; } } as unknown as Native;
  function opened(pages: string[][], fail = -1) {
    const client = new T3Client();
    Object.assign(client, { threadId: 't1', connection: 'connected', thread: loaded(['u9', 'a9'], pages.length > 0) });
    client.shell.threads = [{ id: 't1' }] as typeof client.shell.threads;
    const asked: string[] = [];
    client.history = async (_native: Native, through = '') => {
      asked.push(through);
      if (asked.length - 1 === fail) throw new Error('offline');
      const page = pages[asked.length - 1] ?? [];
      const older = loaded([...page, ...client.projection.visibleTurnItems.map(row => String((row as { item: { id: string } }).item.id))], asked.length < pages.length);
      client.thread = { ...older!, historyCursor: asked.length < pages.length ? `c${asked.length}` : null } as T3Client['thread'];
    };
    return { client, asked };
  }
  test('three pages back: each page is asked through the cited message, then the answer is placed', async () => {
    const { client, asked } = opened([['u3', 'a3'], ['u2', 'a2'], ['u1', 'a1']]);
    const placed: string[] = [];
    await revealCitation(client, native, 't1', 'a1', () => [{ id: '["t1","a1"]', kind: 'assistant', body: 'answer one' }], key => placed.push(key));
    expect(asked).toEqual(['a1', 'a1', 'a1']);
    expect(placed).toEqual(['["t1","a1"]']);
    expect(toasts(client)).toEqual([]);
  });
  test('twenty pages without the answer stop with "Could not load"; a page that loads nothing new stops as unavailable', async () => {
    // Every page moves the cursor and leaves more history, and none holds the answer.
    const { client, asked } = opened(Array.from({ length: 25 }, (_, page) => [`u${page + 10}`, `a${page + 10}`]));
    await revealCitation(client, native, 't1', 'a1', () => [], () => {});
    expect(asked.length).toBe(20);
    expect(toasts(client)).toEqual([expect.objectContaining({ kind: 'warning', title: 'Could not load the cited response' })]);
    // A page that leaves the cursor where it was: nothing older loads, so asking again would spin.
    const still = opened([['u3', 'a3'], ['u2', 'a2']]);
    still.client.history = async (_native: Native, through = '') => { still.asked.push(through); };
    await revealCitation(still.client, native, 't1', 'a1', () => [], () => {});
    expect(still.asked).toEqual(['a1']);
    expect(toasts(still.client)).toEqual([expect.objectContaining({ kind: 'warning', title: 'The cited response is unavailable' })]);
  });
  test('a removed answer, a failed page, or a user message toasts why and places nothing', async () => {
    const cases: [string[][], number, string, string][] = [
      [[], -1, 'a1', 'The cited response is unavailable'],
      [[['u1', 'a1']], 0, 'a1', 'Could not load the cited response'],
      [[], -1, 'u9', 'The citation does not refer to an assistant response'],
    ];
    for (const [pages, fail, id, title] of cases) {
      const { client } = opened(pages, fail);
      const placed: string[] = [];
      await revealCitation(client, native, 't1', id, () => [], key => placed.push(key));
      expect(placed).toEqual([]);
      expect(toasts(client)).toEqual([expect.objectContaining({ kind: 'warning', title })]);
    }
  });
});
