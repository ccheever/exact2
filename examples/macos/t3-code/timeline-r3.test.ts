import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Message, Obj } from './domain';
import { chatLocal, transcriptRows } from './timeline-presentation';
import { dynamicToolTitle, claudeSkillInvocation, projectedWorkEntry, summarizeToolGroup, summaryIcon, summaryKind } from './timeline-worklog';
import { summarizeT3ToolCalls, t3ToolDefinition } from './timeline-t3tools';
import type { Native } from './protocol';

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

  test('a read expands to the paths it read, not the inspector', async () => {
    const view = client([user('u', 0, 'go'), tool('r', 1, 'Read', { file_path: 'src/app.ts' }), answer('a', 3, 'done')]);
    const entry = (await opened(view)).flatMap(row => row.activities ?? []).find(activity => activity.id === key('r'));
    expect(entry).toMatchObject({ label: 'Read project/src/app.ts', output: '/work/project/src/app.ts', body: '' });
  });
});

describe('inspector input (f786ff3)', () => {
  test('a dynamic tool heads its input and highlights it as JSON; a shell command stays plain', async () => {
    const value = client([user('u', 0, 'go'), tool('t', 1, 'Fetch', { url: 'https://example.com', retries: 2 }), command('c', 2, 'ls -la'), answer('a', 3, 'done')]);
    const row = (await opened(value)).find(candidate => candidate.activities?.some(activity => activity.id === key('t')))!;
    const code = row.code!.find(entry => entry.id === key('t'))!;
    expect(code.icon).toBe('INPUT');
    expect(code.tokens.map(token => token.cls)).toContain('tag'); // lane r12-render: Shiki's JSON key ink (#d5512f), as 'key' painted
    expect(row.code!.some(entry => entry.id === key('c'))).toBe(false);
    const json = client([user('u', 0, 'go'), command('j', 1, '{"cmd":["ls"]}'), answer('a', 3, 'done')]);
    expect((await opened(json)).flatMap(candidate => candidate.code ?? []).find(entry => entry.id === key('j'))).toMatchObject({ icon: '' });
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
    expect(result[1]).toMatchObject({ title: 'Worked for 2.0s' });
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
    const result = rows(client([user('u', 0, 'go'), note], { subagents: [{ ...child, result: 'All docs reviewed' }] }));
    const card = result.find(row => row.kind === 'subagent')!;
    expect(card.activities![0]).toMatchObject({ label: 'Review Docs', result: 'Finished', body: 'All docs reviewed', tone: 'success', status: 'event', targetId: 'child', output: '00:00' });
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
    const failed = work(item('f', 'dynamic_tool', 1, { toolName: 'mcp__t3-code__unwatch_pull_request', input: { number: 7 }, output: { isError: true } }));
    expect(rows(client([user('u', 0, 'go'), failed.item, answer('a', 2, 'ok')])).length).toBeGreaterThan(0);
    const entries = [1, 2].map(n => work(item(`w${n}`, 'dynamic_tool', n, { toolName: 'mcp__t3-code__watch_pull_request', input: { number: n }, output: '{"ok":true}' })));
    expect(summarizeToolGroup(entries).summary).toBe('Watching 2 pull requests');
    expect(summaryIcon(summaryKind(entries))).toBe('pull-request');
    expect(summarizeT3ToolCalls('unwatch-pr', [{ input: {}, output: null, outcome: 'unfinished' }]).label).toBe('Tried to stop watching 1 pull request');
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

describe('labels redraw by key', () => {
  test('a live row carries its label keyed by its text', () => {
    const value = live([user('u', 0, 'go'), item('q', 'approval_request', 1, { status: 'pending', requestId: 'q', prompt: 'Allow the fixture?' })]);
    const row = rows(value).find(candidate => candidate.kind === 'live')!;
    expect(row.activities).toHaveLength(1);
    expect(row.activities![0]).toMatchObject({ label: 'Allow the fixture?', id: `${row.id}\u0000Allow the fixture?` });
  });
});

function rows(value: T3Client) { return transcriptRows(value); }

describe('mermaid fences (5e35272)', () => {
  const flow = 'graph TD\n  A --> B';
  const markdown = `Look:\n\n\`\`\`mermaid\n${flow}\n\`\`\`\n\n\`\`\`ts\nconst a = 1;\n\`\`\`\n`;
  test('only settled mermaid fences carry a diagram, loading until the module answers', async () => {
    const { mermaidFences, messageDiagrams } = await import('./timeline-mermaid');
    expect(mermaidFences(markdown)).toEqual([flow]);
    expect(messageDiagrams(markdown, false)).toMatchObject([{ id: '0', code: flow, diagram: 'loading', items: [] }]);
    expect(messageDiagrams(markdown, true)).toEqual([]);
  });
  test('joins both themes into one geometry with both paints, and reports errors', async () => {
    const { joinThemes } = await import('./timeline-mermaid');
    const item = (fill: string) => ({ kind: 'path', d: 'M0 0H10', transform: 'matrix(1 0 0 1 0 0)', fill, stroke: '', width: 1, dash: '', cap: 'butt', join: 'miter' });
    const light = JSON.stringify({ status: 'rendered', width: 120, height: 80, viewBox: '0 0 120 80', items: [item('#ececff'), { kind: 'text', text: 'Start', transform: 'matrix(1 0 0 1 4 8)', fill: '#333333', x: 0, y: 16, size: 16, weight: 400, slant: 'normal', baseline: 'central' }] });
    const dark = JSON.stringify({ status: 'rendered', width: 120, height: 80, viewBox: '0 0 120 80', items: [item('#1f2020'), { kind: 'text', text: 'Start', transform: 'matrix(1 0 0 1 4 8)', fill: '#cccccc', x: 0, y: 16, size: 16, weight: 400, slant: 'normal', baseline: 'central' }] });
    const joined = joinThemes(light, dark);
    expect(joined).toMatchObject({ diagram: 'rendered', width: 120, viewBox: '0 0 120 80' });
    expect(joined.items[0]).toMatchObject({ fill: '#ececff', fillDark: '#1f2020', stroke: '#00000000', dash: 'none' });
    expect(joined.items[1]).toMatchObject({ kind: 'text', text: 'Start', fill: '#333333', fillDark: '#cccccc', baseline: 'central' });
    expect(joinThemes(JSON.stringify({ status: 'error', message: 'Parse error on line 2', retryable: false }), '{}')).toMatchObject({ diagram: 'error', message: 'Parse error on line 2', retryable: false, items: [] });
  });
  test('a user message with a mermaid fence presents its diagram beside its code highlights', () => {
    const value = client([user('u', 0, markdown)]);
    const row = rows(value).find(candidate => candidate.kind === 'user')!;
    expect(row.code!.map(code => code.code)).toEqual([flow, 'const a = 1;']);
    expect(row.diagrams!.map(diagram => [diagram.code, diagram.diagram])).toEqual([[flow, 'loading']]);
  });
});
