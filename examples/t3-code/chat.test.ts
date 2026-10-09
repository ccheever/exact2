import { describe, expect, test } from 'bun:test';
import { timelineMessages, messageTime } from './timeline-presentation';
import { chatCommand } from './chat-commands';
import type { T3Client } from './client';
import type { Message, Obj } from './domain';
import type { Files, Native } from './protocol';

const at = '2026-10-03T13:13:00.000Z';
function fakeClient(checkpoints: Obj[] = []) {
  return { threadId: 't1', projection: { checkpoints }, local: { deviceSettings: { timestampFormat: '24-hour' } } } as unknown as T3Client;
}
// An answer of run r1 the provider can fork (transcriptRows sets canFork from the projected item).
const row = (id: string, kind: string, extra: Partial<Message> = {}): Message => ({ id, kind, title: kind, body: '', createdAt: at, runId: 'r1', sourceThreadId: 't1', completed: true, canFork: true, ...extra });

describe('timeline rows', () => {
  test('the first row carries the 16pt lead and an answer owns its own actions', () => {
    const rows = timelineMessages(fakeClient(), [row('u', 'user'), row('w', 'work'), row('a', 'assistant')], Date.parse(at));
    expect(rows.map(message => message.first)).toEqual([true, false, false]);
    expect(rows[2]).toMatchObject({ actionsId: 'a', actionsFork: true, actionsTime: messageTime(at, Date.parse(at), '24-hour'), attached: false });
    expect(rows[0]).toMatchObject({ actionsId: '', actionsFork: false });
  });

  test('changed files attach to the answer of the same run and carry its actions below them', () => {
    const files = [{ path: 'fixture-result.md', additions: 3, deletions: 0 }];
    const rows = timelineMessages(fakeClient([{ id: 'cp', appRunOrdinal: 1 }]),
      [row('u', 'user'), row('a', 'assistant'), row('c', 'checkpoint', { checkpointId: 'cp', files })], Date.parse(at));
    expect(rows[1]).toMatchObject({ actionsId: '', attached: false });
    expect(rows[2]).toMatchObject({ attached: true, actionsId: 'a', actionsFork: true, checkpointOrdinal: 1, additions: 3, deletions: 0 });
    expect(rows[2]!.files).toEqual([{ id: 'file:fixture-result.md', path: 'fixture-result.md', additions: 3, deletions: 0, kind: 'file', name: 'fixture-result.md',
      depth: 0, expanded: false, icon: 'markdown', spacer: false, plus: '+3', minus: '-0' }]);
    expect(rows[2]).toMatchObject({ fileCount: 1, targetId: 'fixture-result.md' });
  });

  test('a checkpoint from another run, another thread or without files stays standalone', () => {
    const files = [{ path: 'a.ts', additions: 1, deletions: 1 }];
    const rows = timelineMessages(fakeClient(), [row('a', 'assistant'), row('c', 'checkpoint', { runId: 'r2', files }),
      row('b', 'assistant'), row('d', 'checkpoint', { files: [] }), row('e', 'checkpoint', { sourceThreadId: 'source', files })], Date.parse(at));
    expect(rows.map(message => message.id)).toEqual(['a', 'c', 'b', 'd']);
    expect(rows[1]).toMatchObject({ attached: false, actionsId: '' });
    expect(rows[0]).toMatchObject({ actionsId: 'a' });
    expect(rows[3]).toMatchObject({ attached: false });
    expect(rows[2]).toMatchObject({ actionsId: 'b' });
  });

  test('timestamps follow the selected 12/24-hour format and say yesterday across midnight', () => {
    const now = Date.parse('2026-10-04T12:00:00');
    expect(messageTime('2026-10-04T09:05:00', now, '24-hour')).toBe(new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit', hour12: false }).format(new Date('2026-10-04T09:05:00')));
    expect(messageTime('2026-10-03T21:05:00', now, '12-hour')).toStartWith('yesterday at ');
    expect(messageTime('not a date', now, 'locale')).toBe('');
  });
});

describe('sidebar row actions', () => {
  function client(capabilities: Obj = { threadSettlement: true }) {
    const dispatched: Obj[] = [];
    const value = {
      shell: { threads: [{ id: 't1', title: 'Settled one' }, { id: 't2', title: 'Live one' }] },
      config: { environment: { capabilities } },
      restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
        dispatch: async (_storage: Files, payload: Obj, description: string) => { dispatched.push({ ...payload, description }); return {}; } }),
    } as unknown as T3Client;
    return { value, dispatched };
  }
  const native = {} as Native, storage = {} as Files;

  test('settle and un-settle dispatch the row thread, not the selected thread', async () => {
    const { value, dispatched } = client();
    await chatCommand(value, native, storage, 'settle', 't2');
    await chatCommand(value, native, storage, 'unsettle', 't1');
    expect(dispatched).toEqual([
      { type: 'thread.settle', commandId: 'id-0', threadId: 't2', description: 'Settle Live one' },
      { type: 'thread.unsettle', commandId: 'id-0', threadId: 't1', reason: 'user', description: 'Un-settle Settled one' },
    ]);
  });

  test('missing threads, unsupported servers and unknown actions are refused before dispatch', async () => {
    const { value, dispatched } = client();
    await expect(chatCommand(value, native, storage, 'settle', 'gone')).rejects.toThrow('no longer available');
    await expect(chatCommand(value, native, storage, 'archive', 't1')).rejects.toThrow('Unknown thread action');
    const unsupported = client({ threadSettlement: false });
    await expect(chatCommand(unsupported.value, native, storage, 'settle', 't1')).rejects.toThrow('does not support');
    expect(dispatched).toEqual([]);
    expect(unsupported.dispatched).toEqual([]);
  });
});

describe('sidebar search excerpts', () => {
  test('a server match labels the speaker and marks every case-folded hit, title matches included', () => {
    const { searchMatch } = require('./sidebar-presentation') as typeof import('./sidebar-presentation');
    const match = searchMatch({ title: 'fixture complete' }, 'genuine', { source: 'assistant', snippet: '# Fixture complete\n\nThe GENUINE backend, genuine output.' });
    expect(match.matchLabel).toBe('Agent:');
    expect(match.matchUser).toBe(false);
    expect(match.matchParts.map(part => [part.text, part.hit])).toEqual([
      ['# Fixture complete The ', false], ['GENUINE', true], [' backend, ', false], ['genuine', true], [' output.', false]]);
    expect(searchMatch({ title: 'fixture complete fork' }, 'fork', { source: 'user', snippet: 'fork here too' }).matchLabel).toBe('You:');
  });

  test('without a server match, or with an empty query, there is no excerpt', () => {
    const { searchMatch } = require('./sidebar-presentation') as typeof import('./sidebar-presentation');
    expect(searchMatch({ title: 'x', latestVisibleMessage: { role: 'user', text: 'needle' } }, 'needle').matchLabel).toBe('');
    expect(searchMatch({ title: 'a' }, '  ', { source: 'user', snippet: 'b' }).matchLabel).toBe('');
  });
});

describe('snooze', () => {
  test('presets follow the shared rules: evening only with an hour left, next week unless it is tomorrow', () => {
    const { snoozePresets } = require('./sidebar-presentation') as typeof import('./sidebar-presentation');
    const sundayMorning = new Date(2026, 9, 4, 0, 9).getTime();
    expect(snoozePresets(sundayMorning, '12-hour').map(preset => preset.id)).toEqual(['hour', 'three-hours', 'evening', 'tomorrow']);
    const wednesdayEvening = new Date(2026, 9, 7, 17, 30).getTime();
    const late = snoozePresets(wednesdayEvening, '24-hour');
    expect(late.map(preset => preset.id)).toEqual(['hour', 'three-hours', 'tomorrow', 'next-week']);
    expect(new Date(late[2]!.until).getHours()).toBe(9);
    expect(new Date(late[3]!.until).getDay()).toBe(1);
    expect(late[0]!.until).toBe(new Date(wednesdayEvening + 3_600_000).toISOString());
    expect(snoozePresets(0, 'locale')).toEqual([]);
  });

  test('snooze and wake dispatch for the row thread and validate the wake time', async () => {
    const dispatched: Obj[] = [];
    const value = { shell: { threads: [{ id: 't1', title: 'Live' }] }, config: { environment: { capabilities: { threadSnooze: true } } },
      restAccess: () => ({ ids: async () => ['c1'], dispatch: async (_s: Files, payload: Obj) => { dispatched.push(payload); return {}; } }) } as unknown as T3Client;
    await chatCommand(value, {} as Native, {} as Files, 'snooze', 't1', '2026-10-04T09:00:00.000Z');
    await chatCommand(value, {} as Native, {} as Files, 'unsnooze', 't1');
    expect(dispatched).toEqual([{ type: 'thread.snooze', commandId: 'c1', threadId: 't1', snoozedUntil: '2026-10-04T09:00:00.000Z' },
      { type: 'thread.unsnooze', commandId: 'c1', threadId: 't1', reason: 'user' }]);
    await expect(chatCommand(value, {} as Native, {} as Files, 'snooze', 't1', 'later')).rejects.toThrow('Choose when');
    const off = { ...value, config: { environment: { capabilities: { threadSnooze: false } } } } as unknown as T3Client;
    await expect(chatCommand(off, {} as Native, {} as Files, 'snooze', 't1', '2026-10-04T09:00:00.000Z')).rejects.toThrow('does not support snoozing');
  });
});

test('every TypeScript source app.ts answers is registered with the mixed Rust/TypeScript owner', async () => {
  const app = await Bun.file(new URL('./app.ts', import.meta.url)).text();
  const rust = await Bun.file(new URL('./macos/src/markdown.rs', import.meta.url)).text();
  const listed = new Set([...rust.split('&["renderMarkdown"]')[0]!.split('exact_data::Mixed::new(')[1]!.matchAll(/"([A-Za-z]+)"/g)].map(match => match[1]));
  const answered = [...app.matchAll(/source === '([A-Za-z]+)'/g)].map(match => match[1]!);
  expect(answered.filter(source => !listed.has(source))).toEqual([]);
});

test('message search keeps only the current query and marks the server snippet', async () => {
  const { runThreadSearch, serverMatches, searchMatch } = require('./sidebar-presentation') as typeof import('./sidebar-presentation');
  const calls: Obj[] = [];
  const client = { query: 'genuine', restAccess: () => ({ request: async (method: string, payload: Obj) => { calls.push({ method, ...payload });
    return { matches: [{ threadId: 't1', projectId: 'p1', source: 'assistant', snippet: '# Fixture complete The genuine T3 backend', messageCreatedAt: null }] }; } }) } as unknown as T3Client;
  await runThreadSearch(client, {} as Native, 'genuine');
  expect(calls).toEqual([{ method: 'orchestration.searchThreads', query: 'genuine', limit: 50 }]);
  const match = serverMatches(client).get('t1')!;
  expect(searchMatch({ title: 'fixture complete' }, 'genuine', match).matchParts.filter(part => part.hit).map(part => part.text)).toEqual(['genuine']);
  (client as unknown as { query: string }).query = 'other';
  expect(serverMatches(client).size).toBe(0);
  await runThreadSearch(client, {} as Native, 'g');
  expect(calls).toHaveLength(1);
});
