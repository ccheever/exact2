// Desktop audit 2026-10-09, task timeline-work-rows: the reference's own cases where it has them
// (orchestrationV2.test.ts "background work kinds from older or newer servers", threadWorkflows.test.ts
// "allows native, portable, and capability-unknown exact-run forks"), and the audit's fixture rows.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Message, Obj } from './domain';
import { chatLocal, timelineMessages, transcriptRows } from './timeline-presentation';
import { entryIcon, projectedWorkEntry } from './timeline-worklog';
import { subagent } from './timeline-events';
import {
  assistantCanFork, canForkProjectedAssistantItem, decodeNotificationSource, externalWebLinkHref, formatElapsedSeconds,
  groupToolPresentation, inspectorExtras, subagentElapsedMs,
} from './timeline-work-rows';
import type { Native } from './protocol';
import { selectTurn } from './diff';
import { opened } from './client-fixture';
import { snapshot } from './presentation';

const base = Date.parse('2026-10-09T09:00:00.000Z');
const at = (seconds: number) => new Date(base + seconds * 1000).toISOString();
let ordinal = 0;
function item(id: string, type: string, seconds: number, extra: Obj = {}): Obj {
  return { id, threadId: 't1', runId: null, nodeId: null, providerThreadId: null, ordinal: ++ordinal, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra };
}
const user = (id: string, seconds: number) => item(id, 'user_message', seconds, { text: 'go', messageId: `m-${id}`, inputIntent: 'turn_start', attachments: [], createdBy: 'user' });
function client(items: Obj[], projection: Obj = {}, providers: Obj[] = []): T3Client {
  const thread = { projection: { thread: { id: 't1', projectId: 'p1' }, runs: [], attempts: [], nodes: [], checkpoints: [], subagents: [], providerThreads: [], providerSessions: [], ...projection,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) } };
  return { threadId: 't1', projectId: 'p1', shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [] },
    config: { providers }, local: { deviceSettings: { timestampFormat: '24-hour' } }, thread, projection: thread.projection } as unknown as T3Client;
}
const rows = (value: T3Client): Message[] => transcriptRows(value);
const native = { available: true } as unknown as Native;
const key = (id: string) => JSON.stringify(['t1', id]);
const notification = (source: unknown) => item('n', 'notification', 4, { outcome: 'completed', summary: 'Background build finished', detail: 'bun run build exited 0', source });

describe('background work kinds from older or newer servers (TH-2)', () => {
  test('decodes notification sources stored before specific kinds existed', () => {
    expect(decodeNotificationSource({ kind: 'background_task', nativeRef: { driver: 'claude', nativeId: 'task-1' } })).toEqual({ kind: 'background_task' });
    expect(decodeNotificationSource({ kind: 'background_command' })).toEqual({ kind: 'command' });
    expect(decodeNotificationSource({ kind: 'monitor' })).toEqual({ kind: 'monitor' });
  });

  test('reads the specific kind back from the shapes older clients decode', () => {
    expect(decodeNotificationSource({ kind: 'background_task', work: 'subagent', childThreadId: 'child' })).toEqual({ kind: 'subagent', childThreadId: 'child' });
    expect(decodeNotificationSource({ kind: 'background_task', work: 'subagent' })).toEqual({ kind: 'subagent' });
    expect(decodeNotificationSource({ kind: 'delegated_task', taskIds: ['task-1'], childThreadId: 'child' })).toEqual({ kind: 'delegated_task', taskIds: ['task-1'], childThreadId: 'child' });
  });

  test('decodes a notification source kind from a newer server as generic background work', () => {
    expect(decodeNotificationSource({ kind: 'workflow', workflowId: 'wf-1' })).toEqual({ kind: 'background_task' });
    expect(decodeNotificationSource({})).toEqual({ kind: 'background_task' });
  });

  test('round-trips decoded sources; a known kind whose fields do not decode is a defect', () => {
    for (const source of [{ kind: 'delegated_task', taskIds: ['task-1', 'task-2'] }, { kind: 'delegated_task', taskIds: ['task-1'], childThreadId: 'child' },
      { kind: 'subagent', childThreadId: 'child' }, { kind: 'subagent' }, { kind: 'command' }, { kind: 'monitor' }, { kind: 'background_task' }]) {
      expect(decodeNotificationSource(source)).toEqual(source);
    }
    expect(decodeNotificationSource({ kind: 'delegated_task' })).toBeNull();
    expect(decodeNotificationSource({ kind: 'subagent', childThreadId: 7 })).toBeNull();
  });

  test('"Background build finished" shows the terminal icon', () => {
    expect(entryIcon(projectedWorkEntry({ visibility: 'local', item: notification({ kind: 'background_command' }) }))).toBe('terminal');
    expect(entryIcon(projectedWorkEntry({ visibility: 'local', item: notification({ kind: 'background_task', work: 'subagent' }) }))).toBe('bot');
    expect(entryIcon(projectedWorkEntry({ visibility: 'local', item: notification({ kind: 'background_task' }) }))).toBe('zap');
  });

  test('a background_task/subagent notification is drawn as that subagent card', () => {
    const agent = { id: 'sa', childThreadId: 'child', title: 'Subagent: /root/review_docs', status: 'completed', driver: 'codex', providerInstanceId: 'codex',
      result: 'All docs reviewed', startedAt: at(0), completedAt: at(65) };
    const card = rows(client([user('u', 0), notification({ kind: 'background_task', work: 'subagent', childThreadId: 'child' })], { subagents: [agent] }))
      .find(row => row.kind === 'subagent')!;
    expect(card.activities![0]).toMatchObject({ label: 'Review Docs', result: 'Finished', body: 'All docs reviewed', targetId: 'child', detail: 'codex', startedMs: 0 });
  });

  test('a subagent notification the parent has no record of keeps its row and Open subagent', () => {
    const row = rows(client([user('u', 0), notification({ kind: 'background_task', work: 'subagent', childThreadId: 'child' })])).find(value => value.kind === 'entry')!;
    expect(row.activities![0]).toMatchObject({ label: 'Background build finished', icon: 'bot', targetId: 'child', targetLabel: 'Open subagent' });
  });
});

describe('Fork from this response (TH-1)', () => {
  const capabilities = (input: { nativeFork?: boolean; portableFork?: boolean } = {}) => ({
    threads: { canForkThread: input.nativeFork ?? false, canForkFromTurn: input.nativeFork ?? false },
    identity: { nativeThreadIds: input.nativeFork ? 'strong' : 'none' }, context: { supportsFullThreadHandoff: input.portableFork ?? false } });
  test('allows native, portable, and capability-unknown exact-run forks', () => {
    const answer = { type: 'assistant_message', runId: 'run', status: 'completed' };
    expect(canForkProjectedAssistantItem(answer, capabilities({ nativeFork: true }))).toBe(true);
    expect(canForkProjectedAssistantItem(answer, capabilities({ portableFork: true }))).toBe(true);
    expect(canForkProjectedAssistantItem(answer)).toBe(true);
    expect(canForkProjectedAssistantItem(answer, capabilities())).toBe(false);
    expect(canForkProjectedAssistantItem({ ...answer, status: 'running' })).toBe(false);
    expect(canForkProjectedAssistantItem({ ...answer, runId: null })).toBe(false);
  });

  test('reads the capabilities of the item’s provider session', () => {
    const projection = { providerThreads: [{ id: 'pt', providerSessionId: 'ps' }], providerSessions: [{ id: 'ps', capabilities: capabilities() }] };
    expect(assistantCanFork(projection, { type: 'assistant_message', runId: 'run', status: 'completed', providerThreadId: 'pt' })).toBe(false);
    expect(assistantCanFork(projection, { type: 'assistant_message', runId: 'run', status: 'completed', providerThreadId: 'other' })).toBe(true);
  });

  test('a message with no runId shows Copy and the time only; one with a run keeps Fork', () => {
    const now = base + 120_000;
    const fixture = client([user('u', 0), item('a', 'assistant_message', 60, { text: 'Known', messageId: 'm-a', streaming: false })]);
    expect(timelineMessages(fixture, rows(fixture), now).find(row => row.kind === 'assistant')).toMatchObject({ actionsId: key('a'), actionsFork: false });
    const run = client([user('u', 0), item('a', 'assistant_message', 60, { runId: 'r1', text: 'Known', messageId: 'm-a', streaming: false })],
      { runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(60) }] });
    expect(timelineMessages(run, rows(run), now).find(row => row.kind === 'assistant')).toMatchObject({ actionsId: key('a'), actionsFork: true });
    // An incapable provider session hides it; an inherited row's source thread, not loaded here, keeps the fallback.
    const capability = { providerThreads: [{ id: 'pt', providerSessionId: 'ps' }], providerSessions: [{ id: 'ps', capabilities: capabilities() }],
      runs: [{ id: 'r1', status: 'completed', startedAt: at(0), completedAt: at(60) }] };
    const answer = item('a', 'assistant_message', 60, { runId: 'r1', providerThreadId: 'pt', text: 'Known', messageId: 'm-a', streaming: false });
    const incapable = client([user('u', 0), answer], capability);
    expect(timelineMessages(incapable, rows(incapable), now).find(row => row.kind === 'assistant')).toMatchObject({ actionsFork: false });
    const inherited = client([user('u', 0), answer], capability);
    const visible = (inherited.projection.visibleTurnItems as Obj[]);
    visible[1] = { ...visible[1], visibility: 'inherited', sourceThreadId: 'parent' };
    expect(timelineMessages(inherited, rows(inherited), now).find(row => row.kind === 'assistant')).toMatchObject({ actionsFork: true });
  });
});

describe('the work group icon (TH-3)', () => {
  const themed = { _tag: 'themed-logo', logoUrl: 'data:image/png;base64,AAAA' };
  const website = { _tag: 'website', pageUrl: 'https://example.test/proof', faviconUrl: 'data:image/png;base64,BBBB' };
  const source = { key: 'browser-use', name: 'Browser', kind: 'browser', icon: { _tag: 'website', pageUrl: 'https://example.test/' } };
  test('the primary tool source’s icon first, then the last entry’s icon; the surface before the summary glyph', () => {
    expect(groupToolPresentation([{ toolIcon: website }, { toolIcon: themed }, {}])).toEqual({ toolIcon: themed });
    expect(groupToolPresentation([{ toolIcon: themed }, { toolSource: source }, { toolSource: source, toolIcon: website }])).toEqual({ toolIcon: website });
    expect(groupToolPresentation([{ toolSource: source, toolSurface: 'computer' }, { toolSurface: 'browser' }])).toEqual({ toolIcon: source.icon, toolSurface: 'computer' });
    expect(groupToolPresentation([{ toolIcon: { _tag: 'website', pageUrl: 'ftp://nope' } }, {}])).toEqual({});
  });

  test('the fixture group shows the themed logo square', async () => {
    const tool = (id: string, seconds: number, extra: Obj = {}) => item(id, 'dynamic_tool', seconds, { toolName: `fixture_${id}`, input: { proof: id }, output: `Output owned by ${id}`, ...extra });
    // As the audit's fixture thread: no prompt before the work, so no turn fold hides it.
    const value = client([item('c', 'command_execution', 1, { input: 'printf "verified output"', output: 'verified output', exitCode: 0 }),
      item('x', 'command_execution', 2, { input: 'exit 2', output: 'failure', exitCode: 2 }),
      tool('website', 3, { toolIcon: website }), tool('themed', 4, { toolIcon: themed }), tool('failed-icon', 5, { status: 'failed', toolIcon: themed }), tool('empty', 6)]);
    const group = rows(value).find(row => row.kind === 'group')!;
    expect(group).toMatchObject({ icon: 'hammer' });
    expect(group.activities![0]).toMatchObject({ iconLight: themed.logoUrl, iconDark: themed.logoUrl });
    await chatLocal(value, native, 'group', group.groupId!, '');
    expect(rows(value).find(row => row.kind === 'details')!.activities!.map(activity => activity.iconLight)).toContain(website.faviconUrl);
  });
});

describe('the inspector’s search results and file changes (TH-5)', () => {
  const relative = (path: string) => `project/${path}`;
  test('file_search lists each result’s path and line over its preview', () => {
    expect(inspectorExtras({ type: 'file_search', pattern: 'parse', results: [{ fileName: 'fixture.txt', line: 1, preview: 'Timeline verification fixture' }] }, relative).results)
      .toEqual([{ id: '0', kind: 'file', label: 'project/fixture.txt:1', href: '', preview: 'Timeline verification fixture' }]);
  });
  test('web_search links each result’s title over its snippet', () => {
    expect(inspectorExtras({ type: 'web_search', patterns: ['markdown table syntax'], results: [{ title: 'Tables', url: 'https://example.test/tables', snippet: 'Pipes and dashes.' },
      { url: 'javascript:alert(1)' }] }, relative).results).toEqual([
      { id: '0', kind: 'web', label: 'Tables', href: 'https://example.test/tables', preview: 'Pipes and dashes.' },
      { id: '1', kind: 'web', label: 'javascript:alert(1)', href: '', preview: '' }]);
    expect(externalWebLinkHref('//example.test/a')).toBe('https://example.test/a');
  });
  test('file_change shows its path with green +N and red -M, and Open diff for a run’s change', () => {
    expect(inspectorExtras({ type: 'file_change', fileName: 'fixture.txt', additions: 1, deletions: 0, runId: null }, relative))
      .toMatchObject({ changePath: 'project/fixture.txt', stats: true, additions: 1, deletions: 0, diffRunId: '', results: [] });
    expect(inspectorExtras({ type: 'file_change', fileName: 'a.ts', runId: 'r1', changes: [{ operation: 'rename', oldPath: 'b.ts', path: 'a.ts', fileType: 'text' }] }, relative))
      .toMatchObject({ stats: false, diffRunId: 'r1', diffPath: 'a.ts', results: [{ kind: 'change', label: 'rename b.ts → project/a.ts (text)' }] });
  });
  test('Open diff selects the change’s run and file, as onOpenTurnDiff does', () => {
    const value = { environmentId: 'e', threadId: 't1', diffState: { selections: {} as Obj } } as unknown as T3Client;
    selectTurn(value, 'r1', 'src/a.ts');
    expect((value.diffState as unknown as { selections: Obj }).selections['e:t1']).toEqual({ kind: 'turn', runId: 'r1', filePath: 'src/a.ts' });
    expect(() => selectTurn(value, '', 'src/a.ts')).toThrow('That turn is no longer available.');
  });
  test('expanded rows carry the parts instead of a plain output block', async () => {
    const value = client([
      item('fs', 'file_search', 1, { pattern: 'parse', results: [{ fileName: 'fixture.txt', line: 1, preview: 'Timeline verification fixture' }] }),
      item('ws', 'web_search', 2, { patterns: ['markdown table syntax'], results: [{ title: 'Tables', url: 'https://example.test/tables', snippet: 'Pipes and dashes.' }] }),
      item('fc', 'file_change', 3, { fileName: 'fixture.txt', additions: 1, deletions: 0 }), item('r', 'reasoning', 4, { text: 'Planning' })]);
    await chatLocal(value, native, 'group', rows(value).find(row => row.kind === 'group')!.groupId!, '');
    const entries = rows(value).find(row => row.kind === 'details')!.activities!;
    expect(entries.find(entry => entry.id === key('fs'))).toMatchObject({ body: 'parse', output: '', results: [{ label: 'project/fixture.txt:1' }] });
    expect(entries.find(entry => entry.id === key('ws'))).toMatchObject({ body: 'markdown table syntax', output: '', results: [{ label: 'Tables', href: 'https://example.test/tables' }] });
    expect(entries.find(entry => entry.id === key('fc'))).toMatchObject({ body: '', changePath: 'project/fixture.txt', stats: true, additions: 1, deletions: 0 });
  });
});

describe('the subagent card (TH-6)', () => {
  test('formatElapsedSeconds', () => {
    expect([0, 12, 65, 3_720, 59.9].map(formatElapsedSeconds)).toEqual(['0s', '12s', '1m 05s', '1h 02m', '59s']);
  });
  test('deriveSubagentElapsedMs: live agents run to now, settled ones stop at completion', () => {
    expect(subagentElapsedMs({ status: 'running', startedAt: at(0), completedAt: null }, base + 12_000)).toBe(12_000);
    expect(subagentElapsedMs({ status: 'completed', startedAt: at(0), completedAt: at(65) }, 0)).toBe(65_000);
    expect(subagentElapsedMs({ status: 'completed', startedAt: at(0), completedAt: null }, 0)).toBeNull();
    expect(subagentElapsedMs({ status: 'running', startedAt: null, completedAt: null }, 0)).toBeNull();
  });
  test('the driver’s mark, "0s", and no button without a child thread; a live one ticks from its start', () => {
    const settled = item('x', 'subagent', 6, { subagentId: 'node-sub-1', driver: 'codex', providerInstanceId: 'codex', childThreadId: null, completedAt: at(6), result: 'Found two flaky tests.' });
    expect(subagent(settled, { agents: [], providers: [{ instanceId: 'codex', displayName: 'Codex', driver: 'codex' }] }))
      .toMatchObject({ detail: 'codex', initials: 'CO', output: '0s', startedMs: 0, targetId: '', body: 'Found two flaky tests.' });
    const live = subagent({ ...settled, status: 'running', completedAt: null, childThreadId: 'child' });
    expect(live).toMatchObject({ output: '', startedMs: base + 6_000, targetId: 'child', tone: 'info' });
    // The parent's record of the subagent wins over the item, as SubagentTimelineLink reads it.
    expect(subagent(settled, { agents: [{ id: 'node-sub-1', status: 'running', startedAt: at(0), completedAt: null }], providers: [] })).toMatchObject({ startedMs: base, output: '' });
  });
  test('a live subagent keeps the timeline clock ticking', () => {
    const value = client([item('x', 'subagent', 6, { subagentId: 's', driver: 'codex', providerInstanceId: 'codex', status: 'running', childThreadId: null, result: null })]);
    expect(rows(value).find(row => row.kind === 'subagent')!.activities![0]).toMatchObject({ startedMs: base + 6_000 });
  });
  test('subagentsLive: on for a running subagent with a start, off once it settles or without a start (the root’s liveTick gate)', async () => {
    const { client: live } = await opened();
    const agent = item('sa', 'subagent', 6, { subagentId: 's', driver: 'codex', providerInstanceId: 'codex', status: 'running', childThreadId: null, result: null });
    const show = (value: Obj) => { live.thread!.projection.visibleTurnItems = [{ position: 0, visibility: 'local', sourceThreadId: 't1', sourceItemId: 'sa', item: value }]; return snapshot(live); };
    // No run is live: the subagent alone keeps the clock going.
    expect(show(agent)).toMatchObject({ running: false, subagentsLive: true });
    expect(show({ ...agent, status: 'completed', completedAt: at(12), result: 'Found two flaky tests.' })).toMatchObject({ running: false, subagentsLive: false });
    expect(show({ ...agent, startedAt: null })).toMatchObject({ subagentsLive: false });
  });
});

// The Contract side of TH-4, TH-5, TH-6 and TH-10, read from the sources as hover-layer.test.ts reads them.
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}
/** A Contract expression of numbers, comparisons, ternaries, templates and field reads, as JavaScript (`floor`, `max` bound). */
const evaluate = (expression: string, names: string[]) => new Function(...names, 'floor', 'max', `return (${expression});`) as (...values: unknown[]) => string;

describe('the timeline clock and the work rows in Contract (TH-4, TH-5, TH-6, TH-10)', () => {
  test('TH-6: the root ticks liveNow each second while a subagent is live, and the row passes it on', async () => {
    const app = await source('app.contract');
    expect(app).toMatch(/\n {4}if data\.running or data\.subagentsLive or [^\n]*\n {6}liveNow = now\(\)\n/);
    expect(app).toContain('task liveClock mount\n    every(1000, liveTick)');
    expect(await source('app-window.contract')).toContain('timelineNow=(wallTime.epochAtZero + max(liveNow, elapsed))');
    expect(await source('timeline.contract')).toContain('SubagentRow(agent=agent, command=command, now=now)');
  });
  test('TH-6: the row’s elapsedLabel is formatElapsedSeconds, and a live row counts up from its start each second', async () => {
    const work = await source('timeline-work.contract');
    const fn = /^fn elapsedLabel\(seconds: number\): string = (.+)$/m.exec(work)?.[1];
    expect(fn).toBeDefined();
    const elapsedLabel = (seconds: number) => evaluate(fn!, ['seconds'])(seconds, Math.floor, Math.max);
    const seconds = [...Array.from({ length: 7_501 }, (_, index) => index), 363_659];
    expect(seconds.map(elapsedLabel)).toEqual(seconds.map(formatElapsedSeconds));
    const content = await component('timeline-work.contract', 'SubagentRowContent');
    const label = /\n {4}text \((agent\.startedMs > 0 \? .+? : agent\.output)\) [^\n]*testId=`subagent-elapsed-\$\{agent\.id\}`/.exec(content)?.[1];
    expect(label).toBe('agent.startedMs > 0 ? elapsedLabel(max(0, floor((now - agent.startedMs) / 1000))) : agent.output');
    const row = (agent: Obj, now: number) => evaluate(label!, ['agent', 'now', 'elapsedLabel'])(agent, now, elapsedLabel, Math.floor, Math.max);
    const running = { startedMs: base + 6_000, output: '' };
    expect([0, 999, 1_000, 59_999, 65_000, 3_720_000].map(after => row(running, base + 6_000 + after))).toEqual(['0s', '0s', '1s', '59s', '1m 05s', '1h 02m']);
    expect(row(running, base)).toBe('0s'); // a clock behind the start reads 0s, as AgentElapsed's max(0, …)
    expect(row({ startedMs: 0, output: '1m 05s' }, base + 999_999)).toBe('1m 05s'); // settled: fixed
  });
  test('TH-4: an expanded group’s cap grows by what its open rows add, each row reporting its own extra', async () => {
    const entries = await component('timeline.contract', 'WorkEntries');
    expect(entries).toContain('max-height=`calc(min(${18 * rem}px, 50dvh) + ${opened}px)`');
    expect(entries).toContain('WorkEntry(entry=entry, grouped=true, command=command, code=message.code, codeSize=message.md.codeSize, sized=entrySized)');
    // A running total: a report replaces that row's last one.
    expect(entries).toContain('action entrySized(id: string, extra: number)\n    let previous = match first(filter(extras, (e) => e.id == id)) { case some(e) => e.extra, case none => 0 }\n    opened = max(0, opened - previous + extra)');
    const entry = await component('timeline-work.contract', 'WorkEntry');
    // Only an open row adds; a shut one (a provider error's detail line included) reports 0.
    expect(entry).toContain('action rowSized(width: number, height: number)\n    sized(entry.id, expanded ? max(0, height - 1.5 * rem) : 0)');
    expect(entry).toMatch(/\n {4}column hover=setHover resize=rowSized /);
  });
  test('TH-5: a web result presses link-open "external" and Open diff presses turn-diff with the change’s file and run', async () => {
    // browser-links.test.ts and diff.test.ts take the two ops from here.
    const item = await component('timeline-work.contract', 'InspectorItem');
    expect(item).toContain('link href=item.href press=open("external", item.href)');
    const entry = await component('timeline-work.contract', 'WorkEntry');
    expect(entry).toContain('InspectorItem(item=item, codeSize=codeSize, open=linkOpen)');
    expect(entry).toContain('button press=command("turn-diff", entry.diffPath, entry.diffRunId, 0) testId=`work-open-diff-${entry.id}`');
    expect(await source('app-window.contract')).toContain('action linkOpen(kind: string, href: string)\n    chatLocal("link-open", kind, href)');
  });
  test('TH-10: every work-log image icon is muted but a live row’s highlight: opacity 0.7 here, brightness 0.6 in light in the hook', async () => {
    const line = await component('timeline-work.contract', 'WorkLine');
    expect(line).toContain('label=label, muted=(not highlighted))');
    const icon = await component('timeline-icons.contract', 'ToolActivityIcon');
    expect(icon).toContain('opacity=(muted ? 0.7 : 1)');
    expect(icon).toContain('data-tool-icon-muted=(muted ? "1" : "")');
    const hook = await source('modules/apple/T3ToolActivityIcon.swift');
    expect(hook).toContain('muted: element.data[.toolIconMuted] == "1"');
    expect(hook).toContain('guard muted, effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) != .darkAqua else { return }');
    expect(hook).toContain('NSColor.black.withAlphaComponent(0.4).setFill()');
  });
});
