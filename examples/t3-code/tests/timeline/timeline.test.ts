import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Message, Obj } from '../../src/client/domain';
import { chatLocal, timelineMessages, transcriptRows } from '../../src/timeline/timeline-presentation';
import { commandDisplayText, commandProgramName, displayFailed, formatDuration, liveThoughtLine, projectedWorkEntry, summarizeToolGroup, summaryIcon, summaryKind, thoughtText } from '../../src/timeline/timeline-worklog';
import type { Files, Native } from '../../src/client/protocol';
import { appPath } from '../support/app-files';
import { availableRevertTurnCounts, revertTurnCounts, type Entry } from '../../src/timeline/timeline-rows';
import { threadFindState } from '../../src/timeline/thread-find-runtime';
import { liveEvent } from '../../src/client/live-streams';
import { contractComponent } from '../support/contract-action-fixture';
import { withTimers } from '../support/timers';

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
    expect(details.activities![0]).toMatchObject({ body: 'cat package.json', result: '', ok: true });
    expect(details.activities![2]).toMatchObject({ result: 'exit 1', ok: false, tone: 'failed' });
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
    expect(result.find(row => row.kind === 'attempt')).toMatchObject({ title: 'Superseded attempt', detail: 'Cut off by a steer', groupId: 'a1', expanded: false });
    expect(result.some(row => row.body === 'Partial')).toBe(false);
    await toggle(value, 'attempt', 'a1');
    expect(rows(value).some(row => row.body === 'Partial')).toBe(true);
    expect(rows(value).find(row => row.kind === 'user' && row.body === 'steer')).toMatchObject({ intent: 'Steer', intentTip: 'Steered the active turn' });
  });

  test('a reply that finished before the steer stays in the turn, not in an attempt fold (#17761)', async () => {
    const value = client([user('u', 0, 'go'), { ...answer('old', 1, 'Whole answer', { completedAt: at(1) }), nodeId: 'n1' }, user('s', 2, 'steer', { inputIntent: 'steer' }),
      { ...answer('new', 3, 'Done'), nodeId: 'n2' }], {
      attempts: [{ id: 'a1', runId: 'r1', rootNodeId: 'n1', status: 'superseded', completedAt: at(2) }, { id: 'a2', runId: 'r1', rootNodeId: 'n2', status: 'completed' }] });
    expect(rows(value).some(row => row.kind === 'attempt')).toBe(false);
    await toggle(value, 'fold', 'r1');
    expect(rows(value).find(row => row.body === 'Whole answer')).toMatchObject({ kind: 'assistant' });
    // Cut off: the reply closed after the steer stamped the attempt.
    const cut = client([user('u', 0, 'go'), { ...answer('old', 1, 'Partial', { completedAt: at(3) }), nodeId: 'n1' }, user('s', 2, 'steer', { inputIntent: 'steer' })], {
      attempts: [{ id: 'a1', runId: 'r1', rootNodeId: 'n1', status: 'superseded', completedAt: at(2) }] });
    expect(rows(cut).find(row => row.kind === 'attempt')).toMatchObject({ detail: 'Cut off by a steer' });
  });

  test('a steer splits the settled turn: the total time above it, "Continued for" over each segment after (#22783)', async () => {
    const value = client([user('u', 0, 'go'), command('a', 4, 'ls'), user('s', 10, 'steer', { inputIntent: 'steer' }), command('b', 15, 'pwd'), answer('x', 30, 'Done')],
      { runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(30) }] });
    const segment = `r1:steer:${key('s')}`;
    let result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'user', 'work', 'assistant']);
    expect(result[1]).toMatchObject({ title: 'Worked for 30s', groupId: 'r1' });
    expect(result[3]).toMatchObject({ title: 'Continued for 20s', groupId: segment, createdAt: at(10) });
    await toggle(value, 'fold', segment);
    result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'user', 'work', 'entry', 'assistant']);
    expect(result[4]!.activities![0]).toMatchObject({ label: 'pwd' });
    // Back-to-back steers split once, after the last of them.
    const twice = rows(client([user('u', 0, 'go'), command('a', 4, 'ls'), user('s', 10, 'one', { inputIntent: 'steer' }),
      user('t', 11, 'two', { inputIntent: 'promoted_queued_to_steer' }), command('b', 15, 'pwd'), answer('x', 30, 'Done')]));
    expect(kinds(twice)).toEqual(['user', 'work', 'user', 'user', 'work', 'assistant']);
    expect(twice[4]).toMatchObject({ title: 'Continued for 19s', groupId: `r1:steer:${key('t')}` });
  });

  test('a todo update is a quiet row where the agent wrote it, opening to every step (#25401)', async () => {
    const todo = item('todo', 'todo_list', 2, { steps: [{ text: 'Read', status: 'completed' }, { text: 'Write tests', status: 'running' }, { text: 'Ship', status: 'pending' }] });
    const value = client([user('u', 0, 'go'), todo, answer('x', 3, 'Done')]);
    await toggle(value, 'fold', 'r1');
    const group = rows(value).find(row => row.kind === 'group')!;
    expect(group).toMatchObject({ title: 'Tasks 1/3 · Write tests' });
    await toggle(value, 'group', group.groupId!);
    expect(rows(value).find(row => row.kind === 'details')!.activities![0]).toMatchObject({ label: 'Tasks 1/3 · Write tests', icon: 'list-todo', body: '✓ Read\n◉ Write tests\n○ Ship' });
  });

  test('a running subagent card stays out of its settled turn\'s fold (#16878)', async () => {
    const agent = (id: string, status: string) => item(id, 'subagent', 1, { status, providerTurnId: 'pt1', childThreadId: `c-${id}`, title: id });
    const value = client([user('u', 0, 'go'), command('a', 1, 'ls'), agent('s1', 'running'), agent('s2', 'completed'), answer('x', 2, 'Done')]);
    const kept = rows(value);
    expect(kept.find(row => row.kind === 'subagents' || row.kind === 'subagent')).toBeDefined();
    expect(kept.some(row => row.kind === 'work')).toBe(true);
    const settled = rows(client([user('u', 0, 'go'), command('a', 1, 'ls'), agent('s1', 'completed'), answer('x', 2, 'Done')]));
    expect(settled.some(row => row.kind === 'subagents' || row.kind === 'subagent')).toBe(false);
  });

  test('a resumed subagent folds with the turn that started it, not under the later prompt (#19338)', () => {
    const value = client([user('u1', 0, 'first', { runId: 'r1' }), command('a', 1, 'ls'),
      item('s1', 'subagent', 2, { runId: 'r2', status: 'completed', providerTurnId: 'pt2', childThreadId: 'c1', title: 'helper' }), answer('x', 3, 'One'),
      user('u2', 4, 'second', { runId: 'r2' }), { ...command('b', 5, 'pwd'), runId: 'r2' }, { ...answer('y', 6, 'Two'), runId: 'r2' }],
      { runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(3) }, { id: 'r2', status: 'completed', startedAt: at(4), completedAt: at(6) }] });
    const result = rows(value);
    expect(kinds(result)).toEqual(['user', 'work', 'assistant', 'user', 'work', 'assistant']);
    expect(result[1]).toMatchObject({ runId: 'r1' });
    expect(result[4]).toMatchObject({ runId: 'r2' });
  });

  test('an inline render or MCP app is the turn\'s output: it never folds', () => {
    const html = item('h', 'dynamic_tool', 1, { toolName: 't3-code.html_render', output: { htmlRender: { attachmentId: 'a', title: 'Quarterly chart', height: 260 } } });
    const app = item('m', 'dynamic_tool', 1, { toolName: 'charts.show', output: { t3McpApp: { attachmentId: 'b', server: 'charts', tool: 'show', resourceUri: 'ui://charts/show' } } });
    for (const output of [html, app]) {
      const result = rows(client([user('u', 0, 'render it'), output, answer('x', 2, 'Called html_render.')]));
      expect(kinds(result)).toEqual(['user', 'inline-document', 'assistant']);
    }
    // Other work in the turn folds; the render stays out.
    const mixed = rows(client([user('u', 0, 'render it'), command('a', 1, 'ls'), html, answer('x', 2, 'Done')]));
    expect(kinds(mixed)).toEqual(['user', 'work', 'inline-document', 'assistant']);
  });

  test('a delegation\'s tool row gives way to its child\'s card, which keeps the turn unfolded while it runs', () => {
    const delegate = item('d', 'dynamic_tool', 1, { toolName: 't3-code.delegate_task', input: { title: 'Long child' }, output: { taskId: 'node:child' } });
    const child = (status: string) => item('s', 'subagent', 1, { status, subagentId: 'node:child', origin: 'app_owned', title: 'Long child', providerTurnId: 'pt1', childThreadId: 'thread:child' });
    const running = rows(client([user('u', 0, 'delegate'), delegate, child('running'), answer('x', 2, 'Called delegate_task.')]));
    expect(running.some(row => row.kind === 'work')).toBe(false);
    expect(running.find(row => row.kind === 'subagents' || row.kind === 'subagent')).toBeDefined();
    const finished = rows(client([user('u', 0, 'delegate'), delegate, child('completed'), answer('x', 2, 'Called delegate_task.')]));
    expect(kinds(finished)).toEqual(['user', 'work', 'assistant']);
    expect(finished[1]).toMatchObject({ title: 'Worked for 10s' });
    // Until the returned task id names a child, the call stays an ordinary tool row.
    const unmatched = rows(client([user('u', 0, 'delegate'), { ...delegate, output: { taskId: 'node:other' } }, child('running'), answer('x', 2, 'Called delegate_task.')]));
    expect(unmatched[1]).toMatchObject({ kind: 'work', title: 'Worked for 10s' });
  });

  test('a failed run\'s unfinished tool call draws no row; its secret card and error do', () => {
    const value = client([user('u', 0, 'ask'), item('t', 'dynamic_tool', 1, { toolName: 't3-code.request_secret', status: 'running' }),
      item('s', 'secret_request', 1, { status: 'cancelled', label: 'GitHub token', requestId: 'r' }),
      item('e', 'error', 2, { status: 'failed', parentItemId: null, message: 'Provider turn failed.', failure: { message: 'Provider turn failed.' } })],
    { runs: [{ id: 'r1', status: 'failed', startedAt: at(0), completedAt: at(2) }] });
    const result = rows(value);
    expect(result.some(row => row.activities?.some(activity => activity.label.includes('secret')))).toBe(false);
    expect(kinds(result)).toEqual(['user', 'secret', 'entry']);
  });

  test('a completed answer with no text reads "(empty response)"', () => {
    expect(rows(client([user('u', 0, 'hi'), answer('a', 1, '')]))[1]).toMatchObject({ kind: 'assistant', body: '(empty response)' });
    expect(rows(client([user('u', 0, 'hi'), answer('a', 1, '', { streaming: true, status: 'running' })]))
      .find(row => row.kind === 'assistant')).toMatchObject({ body: '' });
  });
});

describe('thought traces', () => {
  test('an open thought highlights its fences as ChatMarkdown does', async () => {
    const value = client([user('u', 0, 'go'), item('think', 'reasoning', 1, { text: 'Plan:\n\n```ts\nconst answer = 42;\n```', streaming: false }), answer('a', 3, 'Done.')]);
    await toggle(value, 'fold', 'r1');
    const thought = () => rows(value).find(row => row.activities?.some(activity => activity.id === key('think')))!;
    expect(thought().code).toEqual([]);
    await chatLocal(value, native, 'item-detail', key('think'), 'open');
    thought();
    (await import('../support/highlights-fixture')).settleHighlights(); // the fence waits for the lexers (highlights.ts)
    const [block] = thought().code!;
    expect(block).toMatchObject({ id: `${key('think')}:0`, code: 'const answer = 42;' });
    expect(block!.tokens.some(token => token.text.includes('const') && token.cls !== '')).toBeTrue();
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

  test('a run whose provider turn ended stops thinking while the server settles it (#21366)', () => {
    const settling = (status: string) => client([user('u', 0, 'go')], { runs: [{ id: 'r1', status: 'running', startedAt: at(0), completedAt: null, activeAttemptId: 'a1' }],
      providerTurns: [{ id: 't0', runAttemptId: 'a1', ordinal: 1, status: 'completed' }, { id: 't1', runAttemptId: 'a1', ordinal: 2, status }] });
    expect(kinds(rows(settling('failed')))).toEqual(['user', 'working']);
    expect(kinds(rows(settling('completed')))).toEqual(['user', 'working']);
    expect(kinds(rows(settling('running')))).toEqual(['user', 'working', 'thinking']);
    // The run is latestUnheldRun's: a held queued follow-up after it does not hide the ended turn.
    const held = client([user('u', 0, 'go')], { runs: [{ id: 'r1', ordinal: 1, status: 'running', startedAt: at(0), completedAt: null, activeAttemptId: 'a1' },
      { id: 'r2', ordinal: 2, status: 'queued', queueHeld: true }], providerTurns: [{ id: 't1', runAttemptId: 'a1', ordinal: 1, status: 'failed' }] });
    expect(kinds(rows(held))).toEqual(['user', 'working']);
  });

  test('a streaming thought is the live row: its plain-text line above, Thinking as the status', () => {
    const result = rows(live([user('u', 0, 'go'), item('t', 'reasoning', 1, { status: 'running', text: '**Reading** the files', streaming: true })]));
    expect(result[2]).toMatchObject({ kind: 'live', title: 'Thinking', detail: 'Reading the files', icon: 'brain' });
  });

  test('the live row carries the latest thought while a later tool runs (#16284)', () => {
    const result = rows(live([user('u', 0, 'go'), item('t1', 'reasoning', 1, { text: 'First idea.' }), item('t2', 'reasoning', 2, { text: '**Found the cause**\nIt is the cache.' }),
      command('b', 3, 'rg cause', { status: 'running', exitCode: undefined })]));
    expect(result[2]).toMatchObject({ kind: 'live', title: 'Running rg', detail: 'Found the cause' });
    // While the thought is itself the live entry, the status line just says Thinking.
    const thinking = rows(live([user('u', 0, 'go'), item('t', 'reasoning', 1, { status: 'running', text: 'Looking at it. Then more.', streaming: true })]));
    expect(thinking[2]).toMatchObject({ kind: 'live', title: 'Thinking', detail: 'Looking at it.' });
  });

  test('active tool entries keep their order (#18853)', async () => {
    const value = live([user('u', 0, 'go'), command('a', 1, 'cat a.ts'), command('b', 2, 'cat b.ts'), command('c', 3, 'rg x', { status: 'running', exitCode: undefined })]);
    const live0 = rows(value).find(row => row.kind === 'live')!;
    await toggle(value, 'group', live0.groupId!);
    expect(rows(value).find(row => row.kind === 'details')!.activities!.map(activity => activity.id)).toEqual([key('a'), key('b'), key('c')]);
  });

  test('a thread read names the thread by its live title and links it (#13140)', async () => {
    const read = (id: string, seconds: number, extra: Obj = {}) => item(id, 'dynamic_tool', seconds, { toolName: 't3_thread_read', input: { threadId: ' t9 ' }, ...extra });
    const value = live([user('u', 0, 'go'), read('r', 1), read('q', 2, { status: 'running' })]);
    (value.shell as { threads: Obj[] }).threads = [{ id: 't9', title: 'Renamed plan' }];
    const result = rows(value);
    expect(result[2]).toMatchObject({ kind: 'live', title: 'Reading thread “Renamed plan”' });
    const settled = client([user('u', 0, 'go'), read('r', 1), answer('a', 2, 'ok')]);
    (settled.shell as { threads: Obj[] }).threads = [{ id: 't9', title: 'Renamed plan' }];
    await toggle(settled, 'fold', 'r1');
    const entry = rows(settled).find(row => row.kind === 'entry');
    expect(entry?.activities?.[0]).toMatchObject({ label: 'Read thread “Renamed plan”', linkPrefix: 'Read thread', linkTitle: 'Renamed plan', linkThreadId: 't9' });
    // An archived or unknown thread keeps the generic label.
    (settled.shell as { threads: Obj[] }).threads = [{ id: 't9', title: 'Renamed plan', archivedAt: at(1) }];
    expect(rows(settled).find(row => row.kind === 'entry')?.activities?.[0]).toMatchObject({ label: 'Read a T3 thread', linkThreadId: '' });
  });

  test('compaction becomes a divider labelled with its token counts, drawn around a real arrow (#20326)', () => {
    const result = rows(client([user('u', 0, 'go'), item('c', 'compaction', 1, { beforeTokenCount: 12_400, afterTokenCount: 3_100 }), answer('a', 2, 'ok')]));
    const divider = result.find(row => row.kind === 'compaction')!;
    expect(divider).toMatchObject({ title: 'Context compacted 12.4K → 3.10K tokens' });
    expect(divider.activities![0]).toMatchObject({ detail: '12.4K', result: '3.10K' });
  });
  test('a failed or stopped compaction says so (#15631)', () => {
    for (const [status, title] of [['failed', 'Context compaction failed'], ['cancelled', 'Context compaction stopped'], ['interrupted', 'Context compaction stopped']]) {
      const divider = rows(client([user('u', 0, 'go'), item('c', 'compaction', 1, { status }), answer('a', 2, 'ok')])).find(row => row.kind === 'compaction')!;
      expect(divider).toMatchObject({ title });
      expect(divider.activities![0]!.detail ?? '').toBe('');
    }
  });
});

describe('compactions T3 Code starts (#24334, #25837)', () => {
  const running = { runs: [{ id: 'r1', status: 'running', startedAt: at(0), completedAt: null }] };
  const prompt = (idleCompaction?: string) => user('u', 0, '/compact', idleCompaction ? { idleCompaction } : {});
  const compaction = (id: string, status: string, extra: Obj = {}) => item(id, 'compaction', 1, { status, beforeTokenCount: 899_000, ...extra });

  test('a bare /compact turn is one live row; only compacting on return speaks through the Working row', () => {
    for (const [reason, live] of [
      ['beforeCacheExpires', { kind: 'compaction', live: true, title: 'Compacting before the cache expires…' }],
      ['onReturn', { kind: 'working', live: true, icon: 'minimize', title: 'Compacting for', startedMs: Date.parse(at(0)) }],
      [undefined, { kind: 'compaction', live: true, title: 'Compacting context' }],
    ] as const) {
      for (const items of [[prompt(reason)], [prompt(reason), compaction('c', 'running')]]) expect(rows(client(items, running))).toMatchObject([live]);
    }
  });
  test('the finished divider leads with why T3 Code compacted, and a compaction-only run leaves no Working row', () => {
    const done = compaction('c', 'completed', { afterTokenCount: 19_000 });
    for (const [reason, title] of [['beforeCacheExpires', 'Context automatically compacted before cache expiry 899K → 19K tokens'],
      ['onReturn', 'Context compacted before sending 899K → 19K tokens'], [undefined, 'Context compacted 899K → 19K tokens']] as const) {
      const settled = rows(client([prompt(reason), done]));
      expect(settled).toMatchObject([{ kind: 'compaction', live: false, title }]);
      expect(settled[0]!.activities![0]).toMatchObject({ body: title.split(' 899K')[0], detail: '899K', result: '19K' });
      expect(rows(client([prompt(reason), done], running))).toMatchObject([{ kind: 'compaction', live: false, title }]);
      const continuing = rows(client([prompt(reason), done, answer('a', 2, 'Continuing', { streaming: true })], running));
      expect(continuing.filter(row => row.kind === 'compaction')).toHaveLength(1);
      expect(continuing.filter(row => row.kind === 'working')).toMatchObject([{ title: 'Working for', live: false }]);
    }
  });
  test('a failed compaction reads in the destructive colour with its reason', () => {
    const retries = rows(client([prompt('onReturn'), compaction('c1', 'failed', { summary: ' retry needed ' }), compaction('c2', 'completed')]))
      .filter(row => row.kind === 'compaction');
    expect(retries).toHaveLength(2);
    expect(retries[0]).toMatchObject({ failed: true, title: 'Context compaction failed: retry needed' });
    expect(retries[0]!.activities![0]).toMatchObject({ label: 'Context compaction failed', output: 'retry needed' });
    expect(retries[1]).toMatchObject({ failed: false });
    expect(new Set(retries.map(row => row.id)).size).toBe(2);
  });
  test('a completed divider stays while the settled work folds', () => {
    for (const text of ['/compact', 'Continue']) {
      const result = rows(client([user('u', 0, text), command('cat', 1, 'cat a.ts'), compaction('c', 'completed', { beforeTokenCount: 100_000, afterTokenCount: 20_000 })]));
      expect(kinds(result)).toEqual(text === '/compact' ? ['work', 'compaction'] : ['user', 'work', 'compaction']);
      expect(result.at(-1)).toMatchObject({ title: 'Context compacted 100K → 20K tokens' });
    }
  });
  test('an answered compact-on-return question folds into the compaction it asked for', () => {
    const question = 'This session is 1h 5m old and uses 412,000 tokens. Compact it before continuing?';
    const answered = item('q', 'user_input_request', 1, { requestId: 'rq', questions: [],
      questionAnswer: { requestId: 'rq', questionTextById: { resume: question }, answers: { resume: 'Compact and continue' }, attachmentsByQuestionId: {} } });
    const after = (items: Obj[], projection: Obj = {}) => rows(client([user('u', 0, 'Continue'), answered, ...items], projection)).slice(1);
    // The question is answered mid-turn, so the Working row times the compaction, not the turn (#26299).
    expect(after([compaction('c', 'running')], running)).toMatchObject([{ kind: 'working', title: 'Compacting for', startedMs: Date.parse(at(1)) }]);
    expect(after([compaction('c', 'completed')])).toMatchObject([{ kind: 'compaction', title: 'Context compacted after 1h 5m idle' }]);
    const later = after([compaction('c', 'completed'), compaction('c2', 'running')], running);
    expect(later.filter(row => row.kind === 'compaction')).toMatchObject([{ title: 'Context compacted after 1h 5m idle', live: false }, { title: 'Compacting context', live: true }]);
    expect(later.filter(row => row.kind === 'working')).toMatchObject([{ title: 'Working for' }]);
  });
  test('a skipped compaction shows as stopped, without a separate interrupted divider', () => {
    const interrupted = { runs: [{ id: 'r1', status: 'interrupted', startedAt: at(0), completedAt: at(9) }] };
    const result = item('i', 'run_interrupt_result', 2, { status: 'interrupted', message: 'Skipped compaction to send a queued message' });
    expect(rows(client([prompt('onReturn'), compaction('c', 'interrupted'), result], interrupted))).toMatchObject([{ kind: 'compaction', title: 'Context compaction stopped' }]);
    // Stopped before the provider started: no compaction row says so, so the interrupt stays.
    expect(rows(client([prompt('onReturn'), result], interrupted)).map(row => row.id)).toContain(key('i'));
  });
});

describe('events and messages', () => {
  test('interrupts, forks and failures keep their reference labels', () => {
    const result = rows(client([user('u', 0, 'go'), item('i', 'run_interrupt_result', 1, { message: 'Stopped by user' }),
      item('f', 'fork', 2, { runId: null, source: { type: 'run', threadId: 'source' }, targetThreadId: 't1' }),
      item('e', 'error', 3, { status: 'failed', failure: { message: 'Intentional fixture failure.', class: 'provider_error' } })],
      { runs: [{ id: 'r1', status: 'failed', startedAt: at(0), completedAt: at(3) }] }));
    expect(result.find(row => row.kind === 'event')).toMatchObject({ title: 'Run interrupted', detail: 'Stopped by user', tone: 'neutral', icon: 'circle-stop' });
    expect(result.find(row => row.kind === 'fork')).toMatchObject({ body: 'Forked from conversation', actionLabel: 'Open source conversation', targetId: 'source' });
    expect(result.find(row => row.kind === 'entry')!.activities![0]).toMatchObject({ label: 'Provider error', tone: 'provider-error', detail: 'Intentional fixture failure.' });
  });

  test('user rows carry attribution, no queued marker and Edit from here to the run before them', () => {
    const value = client([user('u', 0, 'one'), answer('a', 1, 'done'),
      user('q', 2, 'two', { runId: 'r2', inputIntent: 'queued_turn', createdBy: 'agent', senderThreadId: 'other' })],
      { thread: { id: 't1', projectId: 'p1', activeProviderThreadId: 'pt1', modelSelection: { instanceId: 'codex' } },
        providerThreads: [{ id: 'pt1', providerSessionId: 's1', providerInstanceId: 'codex' }],
        runs: [{ id: 'r1', ordinal: 1, status: 'completed', activeAttemptId: 'a1', startedAt: at(0), completedAt: at(10) }],
        attempts: [{ id: 'a1', providerTurnId: 'turn1' }], providerTurns: [{ id: 'turn1', providerThreadId: 'pt1', runAttemptId: 'a1', ordinal: 1 }],
        checkpoints: [{ id: 'cp0', status: 'ready', ordinalWithinScope: 0, appRunOrdinal: null }, { id: 'cp1', runId: 'r1', status: 'ready', appRunOrdinal: 1 }] });
    const result = rows(value);
    expect(result[0]).toMatchObject({ kind: 'user', revert: 0 });
    // A queued message that was sent carries no "From queue" marker (#21474).
    expect(result[2]).toMatchObject({ intent: '', intentTip: '', attribution: 'agent', targetId: 'other', revert: 1 });
  });

  test('a restart continuation reads "Sent by T3 Code"; a legacy scheduled prompt loses its prefix (#17723)', () => {
    const result = rows(client([user('u', 0, 'Continue.', { createdBy: 'agent', creationSource: 'server' }),
      user('s', 1, '[Triggered by schedule task: Daily]\n\nRun it', { messageId: 'scheduled-task-message:task-1:3:scheduled' }),
      user('p', 2, 'hi', { createdBy: 'agent', creationSource: 'mcp' })]));
    expect(result.filter(row => row.kind === 'user').map(row => [row.attribution, row.body, row.detail])).toEqual([
      ['t3code', 'Continue.', ''], ['automation', 'Run it', 'task-1'], ['agent', 'hi', '']]);
  });

  test('automation and other agents\' messages fold to one line under their author (#22638, #23135)', () => {
    const value = client([user('s', 0, `Nightly digest\n\nRead yesterday's commits.\n${'Summarize them.\n'.repeat(12)}`, { scheduledTaskId: 'task-9' }),
      user('a', 1, '\n\n  Please review [notes.md](t3-context://v1/file/file_1)  \nThey need a second read.', { createdBy: 'agent', creationSource: 'mcp', senderThreadId: 'sender' }),
      user('b', 2, 'hello', { createdBy: 'agent', creationSource: 'mcp', senderThreadId: 'gone' }), user('u', 3, 'mine')]);
    value.shell.threads = [{ id: 'sender', title: '  Release prep  ' }] as typeof value.shell.threads;
    const users = rows(value).filter(row => row.kind === 'user');
    expect(users.map(row => [row.attribution, row.title, row.summary, row.collapsible])).toEqual([
      ['automation', 'Automation', 'Nightly digest', false], ['agent', 'Release prep', 'Please review notes.md', false],
      ['agent', 'Agent', 'hello', false], ['', 'You', '', false]]);
  });

  test('a folded message opens while find\'s match is inside it, chevron or not, stays open after, and closes with its thread (findRevealRef)', async () => {
    const value = client([user('s', 0, 'Nightly digest\n\nRead the commits.', { scheduledTaskId: 'task-9' }), user('u', 1, 'the commits are mine')]);
    const folded = () => rows(value).find(row => row.attribution === 'automation')!;
    expect(folded().expanded).toBe(false);
    const find = threadFindState(value).model, at = (entryId: string) => ({ complete: true, snapshotSequence: 0, totalMatches: 2, activeIndex: 0, match: { entryId, runId: null, occurrence: 0 }, navigation: [] });
    find.open = true; find.query = 'commits'; find.result = at('m-s');
    expect(folded().expanded).toBe(true);
    // The painter reveals the fold on every paint while the active match is inside, so the chevron cannot close it.
    await toggle(value, 'message', key('s'));
    expect(folded().expanded).toBe(true);
    // The match moves on and the fold stays open, until the chevron closes it.
    find.result = at('m-u');
    expect(folded().expanded).toBe(true);
    await toggle(value, 'message', key('s'));
    expect(folded().expanded).toBe(false);
    // The row's state is its own: another thread and back, and the fold is closed again.
    await toggle(value, 'message', key('s'));
    expect(folded().expanded).toBe(true);
    value.threadId = 't2'; rows(value); value.threadId = 't1';
    expect(folded().expanded).toBe(false);
    // The footer (Open automation, the time, the actions) shows only while the fold is open.
    const row = contractComponent('timeline.contract', 'UserRow');
    const footer = (expanded: boolean) => row.drawn('label="Open automation"', row.state(), { message: { ...folded(), expanded }, threadFind: { open: true, query: 'commits', entry: '' } });
    expect([footer(true), footer(false)]).toEqual([true, false]);
  });

  test('a sent sound plays in its audio chip, and its reference leaves the prompt (renderContextReference returns null)', () => {
    const sound = (text: string, id: string, seconds: number) => user(id, seconds, text, { attachments: [{ type: 'file', id: `att-${id}`, name: 'memo.wav', mimeType: 'audio/wav' }],
      context: { version: 1, records: [{ version: 1, kind: 'file', contextId: `file_${id}`, label: 'memo.wav', attachmentId: `att-${id}`, name: 'memo.wav' }] } });
    const shown = rows(client([sound('Listen to [memo.wav](t3-context://v1/file/file_a) now', 'a', 0), sound('[memo.wav](t3-context://v1/file/file_b)', 'b', 1)]))
      .filter(row => row.kind === 'user');
    expect(shown.map(row => [row.body, row.audio?.length, row.chips?.length])).toEqual([['Listen to  now', 1, 0], ['', 1, 0]]);
  });

  test('an automation is named by its live task, reads its context references as labels, and copies without the old prefix', async () => {
    const value = client([user('s', 0, 'Run [notes_v2.md](t3-context://v1/file/file_1) and ![](t3-context://v1/image/image_2) now', { scheduledTaskId: 'task-9',
      context: { version: 1, records: [{ version: 1, kind: 'file', contextId: 'file_1', label: 'notes_v2.md', attachmentId: 'att', name: 'notes_v2.md' }] } }),
    user('l', 1, '[Triggered by schedule task: Daily]\n\nRun it', { messageId: 'scheduled-task-message:task-1:3:scheduled' })]);
    Object.assign(value, { environmentId: 'env-1', generation: 1 });
    const fallback = rows(value).find(row => row.attribution === 'automation')!;
    expect(fallback.summary).toBe('Run [notes_v2.md](t3-context://v1/file/file_1) and ![](t3-context://v1/image/image_2) now');
    liveEvent(value, { key: 'scheduled-tasks', subscriptionId: 'sub-1', value: { tasks: [{ id: 'task-9', title: 'Nightly digest of the repository' }] } });
    const [task, legacy] = rows(value).filter(row => row.attribution === 'automation');
    expect([task!.summary, task!.body, task!.chips.length]).toEqual(['Nightly digest of the repository', 'Run notes\\_v2.md and image\\_2 now', 0]);
    expect(legacy!.body).toBe('Run it');
    const copied: string[] = [];
    Object.assign(value, { restAccess: () => ({ call: async (request: { text: string }) => { copied.push(request.text); return {}; } }) });
    await chatLocal(value, native, 'copy', key('l'), '');
    expect(copied).toEqual(['Run it']);
  });

  test('short messages with links or quotes do not collapse; the visible text counts (#16624)', () => {
    const link = `[label](${'x'.repeat(700)})`;
    const result = rows(client([user('l', 0, link), user('n', 1, Array.from({ length: 9 }, (_, index) => `line ${index}`).join('\n'))]));
    expect(result.filter(row => row.kind === 'user').map(row => row.collapsible)).toEqual([false, true]);
  });

  test('copy sits before fork in an answer\'s actions', async () => {
    const source = await Bun.file(appPath('timeline.contract')).text();
    const actions = source.split('component RowActions')[1]!;
    expect(actions.indexOf('label="Copy message"')).toBeLessThan(actions.indexOf('label="Fork from this response"'));
  });

  test('every Message field is present for Contract', async () => {
    const contract = await Bun.file(appPath('shapes.contract')).text();
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
    // The unit is chosen after rounding (#20299).
    expect([formatDuration(59_400), formatDuration(59_500), formatDuration(3_599_500)]).toEqual(['59s', '1m', '1h']);
    expect(thoughtText('**Bold** and `code` [link](http://x)')).toBe('Bold and code link');
  });
  test('reasoning prose never reads as a failed tool (#21146)', () => {
    const thought = entry({ type: 'reasoning', text: 'That exit code 2 is just from listing the deleted directory, which is expected.' });
    expect(displayFailed(thought)).toBe(false);
    expect(displayFailed(entry({ type: 'command_execution', input: 'ls', output: 'ls: x: No such file or directory' }))).toBe(true);
  });
  test('the live thought line: a bold-only heading wins, else the first sentence (liveThoughtLine)', () => {
    expect(liveThoughtLine('**Tracing the cache**\nIt reads twice.')).toBe('Tracing the cache');
    expect(liveThoughtLine('Found it in `rows.ts`. Next I will fix it.')).toBe('Found it in rows.ts.');
    expect(liveThoughtLine('- Check [the docs](http://x) first? Then go.')).toBe('Check the docs first?');
    expect(liveThoughtLine('Version 1.2 is "done." Then more')).toBe('Version 1.2 is "done."');
  });
});

describe('Edit from here after a rewind (#18982)', () => {
  const message = (id: string, runId: string, visibility = 'local', inputIntent = 'turn_start'): Entry =>
    ({ kind: 'message', id, createdAt: '', role: 'user', row: { visibility }, item: { messageId: id, runId, inputIntent } });
  const ready = (runId: string, appRunOrdinal: number) => ({ runId, appRunOrdinal, status: 'ready' });
  test('each turn rewinds to the run before it, the first to the thread start once its checkpoint is loaded', () => {
    // Run ordinals keep increasing after a rewind: r5 follows r1 here, so it rewinds to r1's checkpoint.
    const entries = [message('m1', 'r1'), message('m5', 'r5'), message('m6', 'r6')];
    const checkpoints = [ready('r1', 1), ready('r5', 5)];
    expect([...revertTurnCounts(entries, checkpoints, { threadStartCheckpointReady: true })]).toEqual([['m1', 0], ['m5', 1], ['m6', 5]]);
    expect([...revertTurnCounts(entries, checkpoints)]).toEqual([['m5', 1], ['m6', 5]]);
  });
  test('inherited history, the unsettled run and turns the provider cannot rewind to offer no Edit from here', () => {
    const entries = [message('m0', 'r0', 'inherited'), message('m1', 'r1'), message('m2', 'r2')];
    const checkpoints = [ready('r1', 1)];
    expect([...revertTurnCounts(entries, checkpoints, { threadStartCheckpointReady: true, unsettledRunId: 'r2' })]).toEqual([['m1', 0]]);
    expect([...revertTurnCounts(entries, checkpoints, { threadStartCheckpointReady: true, availableRevertTurnCounts: new Set([1]) })]).toEqual([['m2', 1]]);
  });
  test('the provider conversation can rewind to the start and to each run whose latest provider turn ran on the active provider thread', () => {
    const projection = { thread: { activeProviderThreadId: 'pt', modelSelection: { instanceId: 'claude' } },
      providerThreads: [{ id: 'pt', providerSessionId: 's', providerInstanceId: 'claude' }],
      runs: [{ ordinal: 1, activeAttemptId: 'a1' }, { ordinal: 2, activeAttemptId: 'a2' }, { ordinal: 3, activeAttemptId: null }],
      attempts: [{ id: 'a1' }, { id: 'a2', providerTurnId: 'old' }],
      providerTurns: [{ id: 't1', runAttemptId: 'a1', ordinal: 1, providerThreadId: 'pt' }, { id: 'old', runAttemptId: null, ordinal: 2, providerThreadId: 'other' }] };
    expect([...availableRevertTurnCounts(projection)]).toEqual([0, 1]);
    expect([...availableRevertTurnCounts({ ...projection, thread: { ...projection.thread, modelSelection: { instanceId: 'codex' } } })]).toEqual([]);
  });
});

describe('Edit from here waits for the rollback', () => {
  const { settleRollback, rollbackWitness } = require('../../src/timeline/timeline-presentation') as typeof import('../../src/timeline/timeline-presentation');
  // The projection moves on to its next state at each 200 ms pause, on the fake timers' clock.
  function rollbackClient(states: Obj[]) {
    const start = Date.now();
    const client = { threadId: 't1', get projection() { return states[Math.min(Math.floor((Date.now() - start) / 200), states.length - 1)]; }, refresh: async () => {} };
    return client as unknown as T3Client;
  }
  const thread = { id: 't1', activeProviderThreadId: 'pt1' };
  const before = { thread, providerThreads: [{ id: 'pt1', providerInstanceId: 'codex', lastRunOrdinal: 2 }], runs: [{ id: 'r1', ordinal: 1, status: 'completed' }, { id: 'r2', ordinal: 2, status: 'completed' }] };
  test('resolves once the provider thread is back at the target and the latest settled run after it rolled back (#18982)', async () => {
    const witness = rollbackWitness(before, 1);
    expect(witness.runId).toBe('r2');
    const landed = await withTimers(async () => {
      const client = rollbackClient([before,
        { ...before, runs: [before.runs[0], { id: 'r2', ordinal: 2, status: 'rolled_back' }] },
        { ...before, providerThreads: [{ id: 'pt1', providerInstanceId: 'codex', lastRunOrdinal: 1 }], runs: [before.runs[0], { id: 'r2', ordinal: 2, status: 'rolled_back' }] }]);
      await settleRollback(client, {} as Native, {} as Files, 't1', 'c1', witness, 1);
      return client.projection;
    });
    expect(landed.providerThreads).toEqual([{ id: 'pt1', providerInstanceId: 'codex', lastRunOrdinal: 1 }]);
  });
  test('a rewind to the start lands on a fresh root provider session', async () => {
    const witness = rollbackWitness(before, 0);
    await withTimers(() => settleRollback(rollbackClient([{ ...before, providerThreads: [...before.providerThreads, { id: 'pt2', providerInstanceId: 'codex', ownerNodeId: null, appThreadId: 't1', lastRunOrdinal: null }],
      runs: before.runs.map(run => ({ ...run, status: 'rolled_back' })) }]), {} as Native, {} as Files, 't1', 'c1', witness, 0));
  });
  test('with no settled run after the target the thread has changed', () => {
    expect(() => rollbackWitness({ thread, providerThreads: [], runs: [{ id: 'r1', ordinal: 1, status: 'completed' }] }, 1)).toThrow('The thread has changed. Select a message to rewind again.');
  });
  test("rejects with the thread's rollback failure for this command", async () => {
    const witness = rollbackWitness(before, 1);
    await expect(withTimers(() => settleRollback(rollbackClient([before, { ...before, thread: { ...thread, rollbackFailure: { requestId: 'c1', message: 'The provider could not roll back this conversation.' } } }]),
      {} as Native, {} as Files, 't1', 'c1', witness, 1))).rejects.toThrow('could not roll back');
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
