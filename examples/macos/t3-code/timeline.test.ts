import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Message, Obj } from './domain';
import { chatLocal, timelineMessages, transcriptRows } from './timeline-presentation';
import { commandDisplayText, commandProgramName, formatDuration, projectedWorkEntry, summarizeToolGroup, summaryIcon, summaryKind, thoughtText } from './timeline-worklog';
import type { Files, Native } from './protocol';

const base = Date.parse('2026-10-03T00:00:00.000Z');
const at = (seconds: number) => new Date(base + seconds * 1000).toISOString();
let ordinal = 0;
function item(id: string, type: string, seconds: number, extra: Obj = {}): Obj {
  return { id, threadId: 't1', runId: 'r1', nodeId: null, ordinal: ++ordinal, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra };
}
const command = (id: string, seconds: number, input: string, extra: Obj = {}) => item(id, 'command_execution', seconds, { input, output: '', exitCode: 0, ...extra });
const user = (id: string, seconds: number, text: string, extra: Obj = {}) => item(id, 'user_message', seconds, { text, messageId: `m-${id}`, inputIntent: 'turn_start', attachments: [], createdBy: 'user', ...extra });
const answer = (id: string, seconds: number, text: string, extra: Obj = {}) => item(id, 'assistant_message', seconds, { text, messageId: `m-${id}`, streaming: false, ...extra });
function client(items: Obj[], projection: Obj = {}): T3Client {
  const thread = { projection: { thread: { id: 't1', projectId: 'p1' }, runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(10) }],
    attempts: [], nodes: [], checkpoints: [], ...projection,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) } };
  return { threadId: 't1', projectId: 'p1', shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [] },
    config: { providers: [] }, local: { deviceSettings: { timestampFormat: '24-hour' } }, thread, projection: thread.projection } as unknown as T3Client;
}
const key = (id: string) => JSON.stringify(['t1', id]);
const rows = (value: T3Client) => transcriptRows(value);
const kinds = (value: Message[]) => value.map(row => row.kind);
const native = { available: true } as unknown as Native;
async function toggle(value: T3Client, op: string, id: string) {
  await chatLocal(value, native, op, id, '');
}

const richItems = () => [
  user('u', 0, 'fixture rich'),
  item('think', 'reasoning', 1, { text: '**Planning the fixture run** I will read the sources.', streaming: false }),
  answer('c1', 3, 'I will inspect the project files first.'),
  command('cat', 3, 'cat package.json'), command('sed', 4, "sed -n '1,80p' src/app.ts"),
  command('fail', 5, 'bun test --bail', { status: 'failed', exitCode: 1 }),
  item('edit', 'file_change', 6, { fileName: '/work/project/src/timeline/rows.ts' }),
  item('web', 'web_search', 7, { patterns: ['T3 Code timeline fixture'] }),
  answer('final', 9, '**Rich fixture complete.**'),
];

describe('settled turns', () => {
  test('fold everything before the answer behind "Worked for" and expand by run', async () => {
    const value = client(richItems());
    let result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'assistant']);
    expect(result[1]).toMatchObject({ title: 'Worked for 10s', groupId: 'r1', expanded: false });
    await toggle(value, 'fold', 'r1');
    result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'entry', 'assistant', 'group', 'assistant']);
    expect(result[2]!.activities![0]).toMatchObject({ label: 'Planning the fixture run I will read the sources.', icon: 'brain', reasoning: true });
    expect(result[3]).toMatchObject({ body: 'I will inspect the project files first.', meta: false });
    expect(result[4]).toMatchObject({ title: 'Ran 3 commands, changed 1 file, and performed 1 other action', icon: 'hammer', failed: false });
    await toggle(value, 'group', result[4]!.groupId!);
    result = rows(value);
    const details = result.find(row => row.kind === 'details')!;
    expect(details.activities!.map(activity => [activity.label, activity.icon, activity.failed])).toEqual([
      ['cat package.json', 'terminal', false], ["sed -n '1,80p' src/app.ts", 'terminal', false], ['bun test --bail', 'terminal', true],
      ['project/src/timeline/rows.ts', 'square-pen', false], ['T3 Code timeline fixture', 'globe', false]]);
    expect(details.activities![0]).toMatchObject({ body: 'cat package.json', result: 'Process exited with code 0', ok: true });
    expect(details.activities![2]).toMatchObject({ result: 'Process exited with code 1', ok: false, tone: 'failed' });
    expect(result.find(row => row.kind === 'group')).toMatchObject({ expanded: true });
  });

  test('an interrupted latest run says how long it ran before you stopped it', () => {
    const value = client([user('u', 0, 'go'), command('a', 1, 'ls'), answer('x', 2, 'Partial')],
      { runs: [{ id: 'r1', status: 'interrupted', startedAt: at(0), completedAt: at(65) }] });
    expect(rows(value)[1]).toMatchObject({ kind: 'work', title: 'You stopped after 1m 5s' });
  });

  test('a superseded attempt folds behind its own toggle and keeps the user prompt', async () => {
    const value = client([user('u', 0, 'go'), { ...answer('old', 1, 'Partial'), nodeId: 'n1' }, user('s', 2, 'steer', { inputIntent: 'steer' }),
      { ...answer('new', 3, 'Done'), nodeId: 'n2' }], {
      attempts: [{ id: 'a1', runId: 'r1', rootNodeId: 'n1', status: 'superseded' }, { id: 'a2', runId: 'r1', rootNodeId: 'n2', status: 'completed' }],
      runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(4) }] });
    let result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'user', 'assistant']);
    await toggle(value, 'fold', 'r1');
    result = rows(value);
    expect(result.find(row => row.kind === 'attempt')).toMatchObject({ title: 'Superseded attempt', detail: 'Partial output retained', groupId: 'a1', expanded: false });
    expect(result.some(row => row.body === 'Partial')).toBe(false);
    await toggle(value, 'attempt', 'a1');
    expect(rows(value).some(row => row.body === 'Partial')).toBe(true);
    expect(rows(value).find(row => row.kind === 'user' && row.body === 'steer')).toMatchObject({ intent: 'Steer', intentTip: 'Steered the active turn' });
  });

  test('a completed answer with no text reads "(empty response)"', () => {
    expect(rows(client([user('u', 0, 'hi'), answer('a', 1, '')]))[1]).toMatchObject({ kind: 'assistant', body: '(empty response)' });
    expect(rows(client([user('u', 0, 'hi'), answer('a', 1, '', { streaming: true, status: 'running' })]))
      .find(row => row.kind === 'assistant')).toMatchObject({ body: '' });
  });
});

describe('live runs', () => {
  const live = (items: Obj[]) => client(items, { runs: [{ id: 'r1', status: 'running', startedAt: at(0), completedAt: null }] });
  test('show "Working for", the latest tool as one live row and no fold', () => {
    const result = rows(live([user('u', 0, 'go'), command('a', 1, 'cat a.ts'), command('b', 2, 'rg -n x src', { status: 'running', exitCode: undefined })]));
    expect(kinds(result)).toEqual(['user', 'working', 'live']);
    expect(result[1]).toMatchObject({ startedMs: base });
    expect(result[2]).toMatchObject({ title: 'Running rg', icon: 'terminal', live: true, failed: false });
  });

  test('a pending approval is the live activity with its prompt', () => {
    const result = rows(live([user('u', 0, 'fixture approval'), item('q', 'approval_request', 1, { status: 'pending', requestId: 'q', prompt: 'Allow the fixture?' })]));
    expect(kinds(result)).toEqual(['user', 'working', 'live']);
    expect(result[2]).toMatchObject({ title: 'Allow the fixture?', icon: 'message-circle', live: true });
  });

  test('think while nothing runs, and after a failed tool', () => {
    expect(kinds(rows(live([user('u', 0, 'go')])))).toEqual(['user', 'working', 'thinking']);
    const failed = rows(live([user('u', 0, 'go'), command('a', 1, 'bun test', { status: 'failed', exitCode: 1 })]));
    // T3 hides a failed live tail and thinks again until the next tool runs.
    expect(kinds(failed)).toEqual(['user', 'working', 'thinking']);
  });

  test('a streaming thought is the live row with its plain-text heading', () => {
    const result = rows(live([user('u', 0, 'go'), item('t', 'reasoning', 1, { status: 'running', text: '**Reading** the files', streaming: true })]));
    expect(result[2]).toMatchObject({ kind: 'live', title: 'Reading the files', icon: 'brain' });
  });

  test('compaction becomes a divider labelled with its token counts', () => {
    const result = rows(client([user('u', 0, 'go'), item('c', 'compaction', 1, { beforeTokenCount: 12_400, afterTokenCount: 3_100 }), answer('a', 2, 'ok')]));
    expect(result.find(row => row.kind === 'compaction')).toMatchObject({ title: 'Context compacted 12.4K → 3.10K tokens' });
  });
});

describe('events and messages', () => {
  test('interrupts, forks and failures keep their reference labels', () => {
    const result = rows(client([user('u', 0, 'go'), item('i', 'run_interrupt_result', 1, { message: 'Stopped by user' }),
      item('f', 'fork', 2, { runId: null, source: { type: 'run', threadId: 'source' }, targetThreadId: 't1' }),
      item('e', 'error', 3, { status: 'failed', failure: { message: 'Intentional fixture failure.', class: 'provider_error' } })],
      { runs: [{ id: 'r1', status: 'failed', startedAt: at(0), completedAt: at(3) }] }));
    expect(result.find(row => row.kind === 'event')).toMatchObject({ title: 'Run interrupted', detail: 'Stopped by user', tone: 'danger', icon: 'x' });
    expect(result.find(row => row.kind === 'fork')).toMatchObject({ body: 'Forked from conversation', actionLabel: 'Open source conversation', targetId: 'source' });
    expect(result.find(row => row.kind === 'entry')!.activities![0]).toMatchObject({ label: 'Provider error', tone: 'provider-error', detail: 'Intentional fixture failure.' });
  });

  test('user rows carry attribution, queued intent and Edit from here for ready checkpoints', () => {
    const value = client([user('u', 0, 'one'), answer('a', 1, 'done'),
      user('q', 2, 'two', { runId: 'r2', inputIntent: 'queued_turn', createdBy: 'agent', senderThreadId: 'other' })],
      { checkpoints: [{ id: 'cp1', runId: 'r1', status: 'ready', appRunOrdinal: 1 }] });
    const result = rows(value);
    expect(result[0]).toMatchObject({ kind: 'user', revert: 0 });
    expect(result[2]).toMatchObject({ intent: 'Queued', intentTip: 'Queued behind the active turn', attribution: 'agent', targetId: 'other', revert: -1 });
  });

  test('every Message field is present for Contract', async () => {
    const contract = await Bun.file(new URL('./shapes.contract', import.meta.url)).text();
    const required = (name: string) => [...contract.split(`shape ${name}\n`)[1]!.split('\nshape ')[0]!.matchAll(/^  (\w+):/gm)].map(match => match[1]).sort();
    const value = client(richItems());
    await toggle(value, 'fold', 'r1');
    for (const row of timelineMessages(value, rows(value), base)) {
      expect(Object.keys(row).sort()).toEqual(required('Message'));
      for (const activity of row.activities) expect(Object.keys(activity).sort()).toEqual(required('Activity'));
    }
  });
});

describe('work-log presentation', () => {
  const entry = (value: Obj) => projectedWorkEntry({ visibility: 'local', item: { id: 'x', runId: 'r', status: 'completed', ...value } });
  test('summaries join at most two categories and count the rest', () => {
    const commands = [entry({ type: 'command_execution', input: 'ls' }), entry({ type: 'command_execution', input: 'pwd' })];
    expect(summarizeToolGroup(commands).summary).toBe('Ran 2 commands');
    expect(summarizeToolGroup([...commands, entry({ type: 'file_change', fileName: 'a.ts' }), entry({ type: 'web_search', patterns: ['x'] }), entry({ type: 'web_search', patterns: ['y'] })]).summary)
      .toBe('Ran 2 commands, changed 1 file, and performed 2 other actions');
    expect(summarizeToolGroup([entry({ type: 'reasoning', text: 'a' }), entry({ type: 'reasoning', text: 'b' })]).summary).toBe('Thought (×2)');
    expect(summaryIcon(summaryKind(commands))).toBe('terminal');
    expect(summaryIcon(summaryKind([entry({ type: 'web_search' })]))).toBe('globe');
  });
  test('command labels name the program the way the live row does', () => {
    expect(commandProgramName('cd src && bun test')).toBe('bun');
    expect(commandProgramName("bash -lc 'rg -n x src'")).toBe('rg');
    expect(commandProgramName('FOO=1 sudo npm run build')).toBe('npm');
    expect(commandDisplayText("/bin/zsh -lc 'git status'")).toBe('git status');
    expect(commandDisplayText('echo a | wc -l')).toBe('echo a | wc -l');
  });
  test('durations and thought headings match T3', () => {
    expect([formatDuration(912), formatDuration(9_960), formatDuration(42_000), formatDuration(162_000)]).toEqual(['912ms', '10s', '42s', '2m 42s']);
    expect(thoughtText('**Bold** and `code` [link](http://x)')).toBe('Bold and code link');
  });
});

describe('Edit from here waits for the rollback', () => {
  const { settleRollback } = require('./timeline-presentation') as typeof import('./timeline-presentation');
  function rollbackClient(states: Obj[]) {
    let tick = 0;
    const client = { threadId: 't1', get projection() { return states[Math.min(tick, states.length - 1)]; },
      refresh: async () => {}, restAccess: () => ({ call: async () => { tick++; return {}; } }) };
    return client as unknown as T3Client;
  }
  test('resolves once the message run is rolled back', async () => {
    const client = rollbackClient([{ thread: {}, runs: [{ id: 'r2', ordinal: 2, status: 'completed' }] }, { thread: {}, runs: [{ id: 'r2', ordinal: 2, status: 'rolled_back' }] }]);
    await settleRollback(client, {} as Native, {} as Files, 't1', 'c1', 'r2', 1);
  });
  test("rejects with the thread's rollback failure for this command", async () => {
    const client = rollbackClient([{ thread: {}, runs: [] }, { thread: { rollbackFailure: { requestId: 'c1', message: 'The provider could not roll back this conversation.' } }, runs: [] }]);
    await expect(settleRollback(client, {} as Native, {} as Files, 't1', 'c1', 'r2', 1)).rejects.toThrow('could not roll back');
  });
});

describe('copy feedback', () => {
  test('a copy marks its row Copied!, a refused one Failed to copy', async () => {
    const value = client(richItems()), copied: string[] = [];
    let refuse = false;
    Object.assign(value, { restAccess: () => ({ call: async (request: Obj) => { if (refuse) throw new Error('The clipboard refused.'); copied.push(String(request.text)); return { copied: true }; } }) });
    expect(await chatLocal(value, native, 'copy', key('final'), '')).toBe('Copied message');
    expect(copied).toEqual(['**Rich fixture complete.**']);
    const row = () => timelineMessages(value, rows(value), base).find(message => message.copied > 0)!;
    expect(row()).toMatchObject({ copyFailed: false });
    refuse = true;
    await expect(chatLocal(value, native, 'copy', key('final'), '')).rejects.toThrow('refused');
    expect(row()).toMatchObject({ copyFailed: true });
  });
});
