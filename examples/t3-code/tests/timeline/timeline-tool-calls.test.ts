import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Message, Obj } from '../../src/client/domain';
import { chatLocal, transcriptRows } from '../../src/timeline/timeline-presentation';
import { dynamicToolTitle, claudeSkillInvocation, projectedWorkEntry, summarizeToolGroup, summaryIcon, summaryKind } from '../../src/timeline/timeline-worklog';
import { summarizeT3ToolCalls, t3SummaryAction, t3ToolDefinition, t3ToolPresentation, t3ActionPriority } from '../../src/timeline/timeline-t3tools';
import type { Native } from '../../src/client/protocol';

const base = Date.parse('2026-10-03T00:00:00.000Z');
const at = (seconds: number) => new Date(base + seconds * 1000).toISOString();
let ordinal = 0;
function item(id: string, type: string, seconds: number, extra: Obj = {}): Obj {
  return { id, threadId: 't1', runId: 'r1', nodeId: null, ordinal: ++ordinal, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra };
}
const user = (id: string, seconds: number, text: string, extra: Obj = {}) => item(id, 'user_message', seconds, { text, messageId: `m-${id}`, inputIntent: 'turn_start', attachments: [], createdBy: 'user', ...extra });
const answer = (id: string, seconds: number, text: string, extra: Obj = {}) => item(id, 'assistant_message', seconds, { text, messageId: `m-${id}`, streaming: false, ...extra });
const command = (id: string, seconds: number, input: string, extra: Obj = {}) => item(id, 'command_execution', seconds, { input, output: '', exitCode: 0, ...extra });
const tool = (id: string, seconds: number, toolName: string, input: Obj, extra: Obj = {}) => item(id, 'dynamic_tool', seconds, { toolName, input, output: null, ...extra });
function client(items: Obj[], projection: Obj = {}): T3Client {
  const thread = { projection: { thread: { id: 't1', projectId: 'p1' }, runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(10) }],
    attempts: [], nodes: [], checkpoints: [], subagents: [], ...projection,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) } };
  return { threadId: 't1', projectId: 'p1', shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [] },
    config: { providers: [] }, local: { deviceSettings: { timestampFormat: '24-hour' } }, thread, projection: thread.projection } as unknown as T3Client;
}
const live = (items: Obj[], projection: Obj = {}) => client(items, { runs: [{ id: 'r1', status: 'running', startedAt: at(0), completedAt: null }], ...projection });
const kinds = (value: Message[]) => value.map(row => row.kind);
const native = { available: true } as unknown as Native;
const work = (value: Obj) => projectedWorkEntry({ visibility: 'local', item: value });
const key = (id: string) => JSON.stringify(['t1', id]);
/** Opens every fold and work group, then returns the rows. */
async function opened(value: T3Client): Promise<Message[]> {
  for (const row of rows(value)) if (row.kind === 'work' && !row.expanded) await chatLocal(value, native, 'fold', row.groupId!, '');
  for (const row of rows(value)) if (row.kind === 'group' && !row.expanded) await chatLocal(value, native, 'group', row.groupId!, '');
  return rows(value);
}

describe('Claude skill calls (43bd667)', () => {
  test('titles a skill from its input, in progress as well as completed', () => {
    expect(dynamicToolTitle('Skill', { skill: 'full-send' })).toBe('Skill: full-send');
    expect(claudeSkillInvocation('Skill', { skill: 'claude-api', args: ' pricing ' })).toEqual({ name: 'claude-api', args: 'pricing' });
    expect(dynamicToolTitle('Skill', { skill: ' ' })).toBeUndefined();
    expect(dynamicToolTitle('Read', { skill: 'full-send' })).toBeUndefined();
    expect(dynamicToolTitle('cua_repl.js', { title: ' Click Save ' })).toBe('Click Save');
    const running = rows(live([user('u', 0, 'go'), tool('s', 1, 'Skill', { skill: 'full-send', args: 'ship it' }, { status: 'running' })]));
    expect(running.find(row => row.kind === 'live')).toMatchObject({ title: 'Skill: full-send' });
  });

  test('expands to its trimmed arguments as plain text, and only when it has them', async () => {
    const view = client([user('u', 0, 'go'), tool('s', 1, 'Skill', { skill: 'full-send', args: '  ship it  ' }), tool('n', 2, 'Skill', { skill: 'plain' }), answer('a', 3, 'done')]);
    const entries = (await opened(view)).flatMap(row => row.activities ?? []);
    expect(entries.find(entry => entry.id === key('s'))).toMatchObject({ label: 'Skill: full-send', output: 'ship it', body: '', expandable: true });
    expect(entries.find(entry => entry.id === key('n'))).toMatchObject({ label: 'Skill: plain', output: '', expandable: false });
  });

  test('read paths and skill arguments remain above withheld output while loading', async () => {
    const view = client([user('u', 0, 'go'), tool('r', 1, 'Read', { file_path: 'src/app.ts' }, { outputOmitted: true }),
      tool('s', 2, 'Skill', { skill: 'full-send', args: 'ship it' }, { outputOmitted: true }), answer('a', 3, 'done')]);
    const entries = (await opened(view)).flatMap(row => row.activities ?? []);
    expect(entries.find(entry => entry.id === key('r'))).toMatchObject({ body: '/work/project/src/app.ts', output: 'Loading output…' });
    expect(entries.find(entry => entry.id === key('s'))).toMatchObject({ body: 'ship it', output: 'Loading output…' });
  });

  test('a read expands to the paths it read, not the inspector', async () => {
    const view = client([user('u', 0, 'go'), tool('r', 1, 'Read', { file_path: 'src/app.ts' }), answer('a', 3, 'done')]);
    const entry = (await opened(view)).flatMap(row => row.activities ?? []).find(activity => activity.id === key('r'));
    expect(entry).toMatchObject({ label: 'Read project/src/app.ts', output: '/work/project/src/app.ts', body: '' });
  });
});

describe('inspector input (df4ae529ea)', () => {
  test('a dynamic tool shows key value lines and a shell command stays plain without INPUT headings', async () => {
    const value = client([user('u', 0, 'go'), tool('t', 1, 'Fetch', { url: 'https://example.com', retries: 2 }), command('c', 2, 'ls -la'), answer('a', 3, 'done')]);
    const result = await opened(value);
    expect(result.flatMap(row => row.activities ?? []).find(entry => entry.id === key('t'))?.body).toBe('url https://example.com\nretries 2');
    expect(result.flatMap(row => row.activities ?? []).find(entry => entry.id === key('c'))?.body).toBe('ls -la');
    expect(result.flatMap(row => row.code ?? [])).toEqual([]);
  });
});

describe('working timer (736130c)', () => {
  test('a wake run counts from the work it continues', () => {
    const value = client([user('u', 0, 'go')], { runs: [{ id: 'r1', status: 'running', requestedAt: at(50), startedAt: at(60), workStartedAt: at(5), completedAt: null }] });
    expect(rows(value).find(row => row.kind === 'working')).toMatchObject({ startedMs: base + 5_000 });
    const plain = client([user('u', 0, 'go')], { runs: [{ id: 'r1', status: 'running', requestedAt: at(50), startedAt: at(60), completedAt: null }] });
    expect(rows(plain).find(row => row.kind === 'working')).toMatchObject({ startedMs: base + 60_000 });
  });
});

describe('runless turns (db51460)', () => {
  test('imported V1 turns stay folded once a V2 run starts working', () => {
    const imported = [user('v1', 0, 'old', { runId: null }), command('old-cmd', 1, 'ls', { runId: null }), answer('old-a', 2, 'Old answer', { runId: null })];
    const value = live([...imported, user('u', 20, 'new', { runId: 'r1' })]);
    const result = rows(value);
    expect(kinds(result).slice(0, 3)).toEqual(['user', 'work', 'assistant']);
    expect(result[1]).toMatchObject({ title: 'Worked for 2s' });
  });

  test('a provider-native subagent keeps its latest runless response open while it works', () => {
    const projection = { runs: [], thread: { id: 't1', projectId: 'p1', creationSource: 'provider', lineage: { relationshipToParent: 'subagent' } },
      nodes: [{ id: 'n', kind: 'root_turn', runId: null, status: 'running', startedAt: at(0) }] };
    const result = rows(client([user('u', 0, 'task', { runId: null }), command('c', 1, 'ls', { runId: null }), answer('a', 2, 'progress', { runId: null })], projection));
    expect(kinds(result)).not.toContain('work');
    expect(kinds(result)).toContain('working');
  });
});

describe('thinking after a failed tool (70e5a40)', () => {
  test('discloses the run’s tool calls under the Thinking row', async () => {
    const value = live([user('u', 0, 'go'), command('a', 1, 'cat a.ts'), command('b', 2, 'bun test', { status: 'failed', exitCode: 1 })]);
    let result = rows(value);
    const thinking = result.find(row => row.kind === 'thinking')!;
    expect(thinking).toMatchObject({ groupId: `work-group:${JSON.stringify(['t1', 'a'])}`, expanded: false });
    await chatLocal(value, native, 'group', thinking.groupId!, '');
    result = rows(value);
    expect(kinds(result)).toEqual(['user', 'working', 'thinking', 'details']);
    expect(result[2]).toMatchObject({ expanded: true });
    expect(result[3]!.activities!.map(activity => activity.label)).toEqual(['cat a.ts', 'bun test']);
  });
});

describe('subagent notifications (dab26f5)', () => {
  const child = { id: 'sa', childThreadId: 'child', title: 'Subagent: /root/review_docs', status: 'running', progress: 'Reading files', result: null, startedAt: at(0) };
  test('a known subagent’s notification is its card, frozen at the reported outcome', () => {
    const note = item('n', 'notification', 4, { summary: 'Subagent finished', outcome: 'completed', source: { kind: 'subagent', childThreadId: 'child' } });
    const result = rows(client([user('u', 0, 'go'), note], { subagents: [{ ...child, status: 'completed', completedAt: at(3), result: 'All docs reviewed' }] }));
    const card = result.find(row => row.kind === 'subagent')!;
    expect(card.activities![0]).toMatchObject({ label: 'Review Docs', result: 'Finished', body: 'All docs reviewed', tone: 'success', status: 'event', targetId: 'child', output: '00:00' });
  });
  test('a subagent resumed since its notification shows its live status, not the outcome reported then (#18099)', () => {
    const note = item('n', 'notification', 4, { summary: 'Subagent failed', outcome: 'failed', source: { kind: 'subagent', childThreadId: 'child' } });
    // Still running (a late notification), or settled again after a run that started past the notification.
    for (const agent of [child, { ...child, status: 'completed', startedAt: at(6), completedAt: at(8), result: 'Done again' }]) {
      const card = rows(client([user('u', 0, 'go'), note], { subagents: [agent] })).find(row => row.kind === 'subagent')!.activities![0]!;
      expect(card).toMatchObject({ status: agent.status, failed: false, targetId: 'child' });
      expect(card.result).not.toBe('Failed');
    }
    // Settled before the notification arrived: the reported outcome stands.
    const settled = { ...child, status: 'failed', startedAt: at(1), completedAt: at(3), result: 'Boom' };
    expect(rows(client([user('u', 0, 'go'), note], { subagents: [settled] })).find(row => row.kind === 'subagent')!.activities![0]).toMatchObject({ status: 'event', result: 'Failed', failed: true });
  });
  test('an update has no status dot; an unknown child falls back to the work row', () => {
    const update = item('n', 'notification', 4, { summary: 'Progress', outcome: 'updated', source: { kind: 'subagent', childThreadId: 'child' } });
    expect(rows(client([user('u', 0, 'go'), update], { subagents: [child] })).find(row => row.kind === 'subagent')!.activities![0]).toMatchObject({ result: 'Updated', tone: 'none' });
    const stranger = item('n', 'notification', 4, { summary: 'Subagent finished', outcome: 'failed', source: { kind: 'subagent', childThreadId: 'other' } });
    expect(rows(client([user('u', 0, 'go'), stranger], { subagents: [child] })).find(row => row.kind === 'entry')!.activities![0]).toMatchObject({ label: 'Subagent finished' });
  });
});

describe('T3 pull request tools (18b2132)', () => {
  test('resolve through provider prefixes and name the PR they act on', () => {
    expect(t3ToolDefinition('mcp__t3-code__watch_pull_request')?.summaryAction).toBe('watch-pr');
    expect(t3ToolDefinition('t3-code.unwatch_pull_request')?.summaryAction).toBe('unwatch-pr');
    expect(t3ToolDefinition('mcp__other__watch_pull_request')).toBeNull();
    const running = work(item('w', 'dynamic_tool', 1, { status: 'running', toolName: 'mcp__t3-code__watch_pull_request', input: { url: 'https://github.com/a/b/pull/12' }, output: null }));
    const value = live([user('u', 0, 'go'), running.item]);
    expect(rows(value).find(row => row.kind === 'live')).toMatchObject({ title: 'Watching PR #12', icon: 'pull-request' });
    // A linked change request is named in its host's term (#23999).
    const link = (url: string) => t3ToolPresentation(t3ToolDefinition('t3-code.link_pull_request'), 'completed', { arguments: { url } })?.displayName;
    expect(link('https://github.com/acme/web/pull/7')).toMatch(/ PR #7$/);
    expect(link('https://gitlab.com/acme/web/-/merge_requests/7')).toMatch(/ MR #7$/);
    expect(t3ToolPresentation(t3ToolDefinition('t3-code.link_pull_request'), 'completed', { arguments: { number: 7 } })?.displayName).toMatch(/ PR #7$/);
    const failed = work(item('f', 'dynamic_tool', 1, { toolName: 'mcp__t3-code__unwatch_pull_request', input: { number: 7 }, output: { isError: true } }));
    expect(rows(client([user('u', 0, 'go'), failed.item, answer('a', 2, 'ok')])).length).toBeGreaterThan(0);
    const entries = [1, 2].map(n => work(item(`w${n}`, 'dynamic_tool', n, { toolName: 'mcp__t3-code__watch_pull_request', input: { number: n }, output: '{"ok":true}' })));
    expect(summarizeToolGroup(entries).summary).toBe('Watching 2 pull requests');
    expect(summaryIcon(summaryKind(entries))).toBe('pull-request');
    expect(summarizeT3ToolCalls('unwatch-pr', [{ input: {}, output: null, outcome: 'unfinished' }]).label).toBe('Tried to stop watching 1 pull request');
  });
});

describe('T3 action tools (#25423) and background waits (#24610)', () => {
  test('an action tool call is labelled as the action it ran', () => {
    expect(t3ToolDefinition('mcp__t3-code__preview_act', { action: 'type' })?.labels).toEqual(t3ToolDefinition('preview_type')?.labels);
    expect(t3ToolDefinition('t3_thread_transfer', { action: 'fork' })?.displayName).toBe(t3ToolDefinition('t3_thread_fork')?.displayName);
    expect(t3ToolDefinition('preview_act', { action: 'unknown' })?.labels[2]).toBe('Used');
    expect(t3ToolDefinition('preview_act', { action: 'hover' })?.labels[2]).toBe('Hovered');
    const running = work(item('w', 'dynamic_tool', 1, { status: 'running', toolName: 'mcp__t3-code__preview_act', input: { action: 'click', locator: 'Save' }, output: null }));
    expect(rows(live([user('u', 0, 'go'), running.item])).find(row => row.kind === 'live')).toMatchObject({ title: 'Clicking in the preview browser' });
  });
  test('an action tool counts under the action each call ran', () => {
    expect(t3SummaryAction('mcp__t3-code__t3_queue', { action: 'list' })).toBe('queue-list');
    expect(t3SummaryAction('t3_queue', { action: 'edit' })).toBe('queue-edit');
    expect(t3SummaryAction('t3_thread_transfer', { action: 'status' })).toBe('thread-transfers');
    expect(t3SummaryAction('t3_thread_configure', {})).toBe('thread-configuration');
    expect(t3SummaryAction('schedule_task', { scheduledTaskId: 'task-1' })).toBe('schedule-update');
    expect(t3SummaryAction('delegate_task', { task: 'x' })).toBe('delegate');
    const lists = [1, 2].map(n => work(item(`q${n}`, 'dynamic_tool', n, { toolName: 'mcp__t3-code__t3_queue', input: { action: 'list' }, output: '{"ok":true}' })));
    expect(summarizeToolGroup(lists).summary).toBe('Listed queued messages 2 times');
  });
  test('an action tool without its action, and the preview tools it reaches, read their own labels', () => {
    const name = (tool: string) => t3ToolPresentation(t3ToolDefinition(`t3-code.${tool}`), 'completed')?.displayName;
    expect(['t3_queue', 't3_pending_request', 't3_thread_transfer', 't3_thread_configure', 'preview_act'].map(name)).toEqual([
      'Managed queued messages', 'Handled pending questions', 'Used thread transfers', 'Used thread model settings', 'Used the preview browser']);
    expect(['preview_select', 'preview_drag', 'preview_upload', 'preview_dialog', 'preview_hover'].map(name)).toEqual([
      'Chose an option in the preview browser', 'Dragged in the preview browser', 'Uploaded files to the preview browser',
      'Responded to a preview browser dialog', 'Hovered in the preview browser']);
  });
  test('a background wait reads "Updated background waits" and leads the group as an effect', () => {
    const call = { input: { taskIds: ['b1'] }, output: { matched: ['b1'], unmatched: [] }, outcome: 'completed' as const };
    expect(summarizeT3ToolCalls('background-wait', [call]).label).toBe('Updated background waits 1 time');
    expect(t3ToolPresentation(t3ToolDefinition('mcp__t3-code__t3_background_wait'), 'completed')?.displayName).toBe('Updated background waits');
    expect(t3ActionPriority('background-wait')).toBe(0);
  });
});

describe('T3 tool labels read as sentences (presentation.test.ts)', () => {
  const name = (tool: string, status: string) => t3ToolPresentation(t3ToolDefinition(`t3-code.${tool}`), status)?.displayName;
  test('an interrupted unwatch differs from a completed one', () => {
    expect(['inProgress', 'stopped', 'completed'].map(status => name('unwatch_pull_request', status)))
      .toEqual(['Unwatching a pull request', 'Stopped unwatching a pull request', 'Stopped watching a pull request']);
  });
  test('starting review-guide writing in rows and groups without outcomes', () => {
    const call = { input: {}, output: null, outcome: 'completed' as const };
    expect(name('write_pull_request_guides', 'completed')).toBe('Started writing review guides');
    expect(summarizeT3ToolCalls('write-pr-guides', [call]).label).toBe('Started writing review guides');
    expect(summarizeT3ToolCalls('write-pr-guides', [call, call]).label).toBe('Started writing review guides 2 times');
  });
  test('delegated-task cancellation reads as an ongoing action, then a request', () => {
    expect(['inProgress', 'completed'].map(status => name('task_cancel', status))).toEqual(['Canceling a delegated task', 'Requested cancellation of a delegated task']);
  });
});

describe('answered questions', () => {
  test('lead with the question, trail the answer, and expand to the history', async () => {
    const questionAnswer = { requestId: 'q', answers: { scope: { answers: ['Workspace'] } }, attachmentsByQuestionId: {}, questionTextById: { scope: 'Which scope should this fixture use?' } };
    const value = client([user('u', 0, 'fixture question'), item('q', 'user_input_request', 1, { questions: [{ id: 'scope', question: 'Which scope should this fixture use?' }], questionAnswer }), answer('a', 2, 'ok')]);
    const result = await opened(value);
    const row = result.find(candidate => candidate.activities?.some(activity => activity.id === key('q')))!;
    expect(row.activities!.find(activity => activity.id === key('q'))).toMatchObject({ label: 'Which scope should this fixture use?', answer: 'Workspace', body: '' });
    expect(row.code!.find(entry => entry.id === key('q'))).toMatchObject({ icon: 'QA', tokens: [{ text: 'Which scope should this fixture use?', cls: 'q' }, { text: 'Workspace', cls: 'a' }].map((token, index) => ({ id: String(index), ...token })) });
  });
});

describe('answered questions stay visible (#21086)', () => {
  const questionAnswer = { requestId: 'q', answers: { scope: { answers: ['Workspace'] } }, attachmentsByQuestionId: {}, questionTextById: { scope: 'Which scope?' } };
  const question = (seconds: number) => item('q', 'user_input_request', seconds, { questions: [{ id: 'scope', question: 'Which scope?' }], questionAnswer });
  test('a settled turn folds its work but not the answered question', () => {
    const result = rows(client([user('u', 0, 'go'), command('ls', 1, 'ls'), question(2), command('cat', 3, 'cat a'), answer('a', 4, 'ok')]));
    expect(result.map(row => row.kind)).toEqual(['user', 'work', 'entry', 'assistant']);
    expect(result[2]!.activities![0]).toMatchObject({ id: key('q'), label: 'Which scope?', answer: 'Workspace' });
  });
  test('in a running turn it keeps its own row, out of the live work around it', () => {
    const result = rows(live([user('u', 0, 'go'), command('ls', 1, 'ls'), question(2), command('cat', 3, 'cat a', { status: 'running' })]));
    const entry = result.find(row => row.activities?.some(activity => activity.id === key('q')))!;
    expect(entry.kind).toBe('entry');
    expect(entry.activities).toHaveLength(1);
  });
});

describe('labels redraw by key', () => {
  test('a live row carries its label keyed by its text', () => {
    const value = live([user('u', 0, 'go'), item('q', 'approval_request', 1, { status: 'pending', requestId: 'q', prompt: 'Allow the fixture?' })]);
    const row = rows(value).find(candidate => candidate.kind === 'live')!;
    expect(row.activities).toHaveLength(1);
    expect(row.activities![0]).toMatchObject({ label: 'Allow the fixture?', id: `${row.id}\u0000Allow the fixture?` });
  });
});

function rows(value: T3Client) { return transcriptRows(value); }

describe('mermaid fences', () => {
  const flow = 'graph TD\n  A --> B';
  const markdown = `Look:\n\n\`\`\`mermaid\n${flow}\n\`\`\`\n\n\`\`\`ts\nconst a = 1;\n\`\`\`\n`;
  test('closed mermaid fences carry a diagram, also while the reply streams, loading in each theme until its page reports', async () => {
    const { mermaidFences, messageDiagrams } = await import('../../src/timeline/timeline-mermaid');
    expect(mermaidFences(markdown)).toEqual([flow]);
    const [diagram] = messageDiagrams(markdown);
    expect(diagram).toMatchObject({ id: '0', code: flow, light: { diagram: 'loading' }, dark: { diagram: 'loading' } });
    // The Mermaid page (assets/mermaid/index.html) takes the fence, its theme, the font and its surface in its fragment.
    const page = new URL(diagram!.srcDark, 'exact-app://app/');
    expect(page.pathname).toBe('/assets/mermaid/index.html');
    expect(Object.fromEntries(new URLSearchParams(page.hash.slice(1)))).toMatchObject({ theme: 'dark', source: flow, view: 'chat' });
    // A streaming reply draws the fences it has closed (isClosedCodeFence); the one still open stays a code block.
    const streaming = `${markdown}\n\`\`\`mermaid\ngraph TD\n  C --> D\n\`\``;
    expect(messageDiagrams(streaming).map(entry => entry.code)).toEqual([flow]);
    expect(messageDiagrams(`${streaming}\``).map(entry => entry.code)).toEqual([flow, 'graph TD\n  C --> D']);
  });
  test('a page\'s report lands per theme and source; an error offers Retry only when Retry may help, and Retry loads the page again', async () => {
    const { diagramFor, noteMermaidFrame, retryMermaid } = await import('../../src/timeline/timeline-mermaid');
    const code = 'graph LR\n  X --> Y';
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 120.5, height: 80, theme: 'light', source: code }));
    expect(diagramFor('0', code)).toMatchObject({ light: { diagram: 'rendered', width: 120.5, height: 80 }, dark: { diagram: 'loading' } });
    noteMermaidFrame(JSON.stringify({ status: 'error', message: 'Mermaid failed to load.', retryable: true, theme: 'dark', source: code }));
    expect(diagramFor('0', code).dark).toEqual({ diagram: 'error', message: 'Mermaid failed to load.', retryable: true, width: 0, height: 0, sizes: [] });
    const before = diagramFor('0', code).srcDark;
    retryMermaid(code, 'dark');
    expect(diagramFor('0', code).dark.diagram).toBe('loading');
    expect(diagramFor('0', code).srcDark).not.toBe(before);
    expect(diagramFor('0', code).light.diagram).toBe('rendered');
    // A report that is not JSON, names no source or no size changes nothing.
    for (const junk of ['nope', JSON.stringify({ status: 'rendered', width: 1, height: 1, theme: 'dark', source: '' }), JSON.stringify({ status: 'rendered', width: 0, height: 5, theme: 'dark', source: code })]) noteMermaidFrame(junk);
    expect(diagramFor('0', code).dark.diagram).toBe('loading');
    // Pele lays a diagram out for its page's width, so a file's page and the preview's keep out of the lane's report.
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 60, height: 200, theme: 'light', source: code, view: 'file' }));
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 30, height: 400, theme: 'light', source: code, view: 'preview' }));
    expect(diagramFor('0', code).light).toMatchObject({ width: 120.5, height: 80 });
    expect(diagramFor('0', code, 'file').light).toMatchObject({ width: 60, height: 200 });
    // One fence drawn at two widths keeps each width's height; the frame picks its own (mermaid-frame.contract).
    const wide = 'graph LR\n  W --> V';
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 700, height: 90, drawnFor: 736, theme: 'light', source: wide }));
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 300, height: 240, drawnFor: 320, theme: 'light', source: wide }));
    expect(diagramFor('0', wide).light).toMatchObject({ width: 300, height: 240, sizes: [{ drawnFor: 736, width: 700, height: 90 }, { drawnFor: 320, width: 300, height: 240 }] });
  });
  test('a user message with a mermaid fence presents its diagram beside its code highlights', () => {
    const value = client([user('u', 0, markdown)]);
    const row = rows(value).find(candidate => candidate.kind === 'user')!;
    expect(row.code!.map(code => code.code)).toEqual([flow, 'const a = 1;']);
    expect(row.diagrams!.map(diagram => [diagram.code, diagram.light.diagram])).toEqual([[flow, 'loading']]);
  });
});


test('workflow Stop tool names its requested effect and counts unique coordinator identifiers', () => {
  const definition = t3ToolDefinition('mcp__t3-code__t3_workflow_stop');
  expect(definition?.summaryAction).toBe('workflow-stop');
  expect(t3ToolPresentation(definition, 'completed')).toEqual({ displayName: 'Requested a stop for a workflow', icon: 't3-code' });
  expect(t3ToolPresentation(definition, 'failed')?.displayName).toBe('Failed to stop a workflow');
  expect(summarizeT3ToolCalls('workflow-stop', [
    { input: { subagentId: 'workflow-a' }, output: {}, outcome: 'completed' },
    { input: { subagentId: 'workflow-a' }, output: {}, outcome: 'completed' },
    { input: { subagentId: 'workflow-b' }, output: {}, outcome: 'completed' },
    { input: { subagentId: 'workflow-c' }, output: { isError: true }, outcome: 'failed' },
  ])).toEqual({ label: 'Requested stops for 2 workflows', failedCount: 1 });
  expect(summarizeT3ToolCalls('workflow-stop', [{ input: {}, output: null, outcome: 'unfinished' }]).label).toBe('Tried to stop 1 workflow');
  expect(t3ActionPriority('workflow-stop')).toBe(0);
});
