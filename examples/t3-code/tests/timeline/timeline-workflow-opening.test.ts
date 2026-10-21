// A workflow coordinator's thread opens with its plan: subagentRuntime.test.ts (resolveWorkflowOpening) and
// WorkflowOpeningBlock.browser.test.tsx at T3 Code 74541bab17 (#20857).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { transcriptRows } from '../../src/timeline/timeline-presentation';
import { findLine, isWorkflowScriptMessage, resolveWorkflowOpening, workflowOpeningMessage, scriptMarkdown, syncWorkflowOpening, workflowOpeningHeadline, workflowOpeningParentThreadId,
  workflowOpeningRows } from '../../src/timeline/timeline-workflow-opening';

const SCRIPT = '// probe\nexport const meta = { name: "probe-wf" };\nconst GROUPS = [];';
const thread = { id: 'child', projectId: 'p1', lineage: { parentThreadId: 'parent', relationshipToParent: 'subagent', rootThreadId: 'parent' }, creationSource: 'provider' };
const brief = (text: string, workflowScript?: boolean): Obj => ({ id: 'message-1', role: 'user', text, ...(workflowScript === undefined ? {} : { workflowScript }) });
const projection = (message: Obj, extra: Obj = {}): Obj => ({ thread, messages: [message], ...extra });
const member = (extra: Obj = {}): Obj => ({ index: 1, label: 'alpha:one', state: 'running', phaseIndex: 1, phaseTitle: 'Alpha', startedAt: Date.parse('2026-10-21T00:00:00Z'), ...extra });
const coordinator = (workflow?: Obj): Obj => ({ id: 'coordinator', title: 'Run the alpha workflow', childThreadId: 'child', status: 'running', updatedAt: '2026-10-21T00:00:00Z',
  startedAt: '2026-10-21T00:00:00Z', driver: 'claudeAgent', providerInstanceId: 'claude', ...(workflow === undefined ? {} : { workflow }) });

describe('resolveWorkflowOpening', () => {
  test("opens a marked thread with the coordinator's plan and members", () => {
    const opening = resolveWorkflowOpening(projection(brief(SCRIPT, true)),
      [coordinator({ name: 'probe-wf', phases: [{ index: 1, title: 'Alpha' }], agents: [member({ childThreadId: 'member-1' })] })]);
    expect(opening).toMatchObject({ messageId: 'message-1', script: SCRIPT, name: 'probe-wf', description: 'Run the alpha workflow' });
    expect(opening?.group?.phases.map(phase => ({ title: phase.title, members: phase.members.map(agent => [agent.title, agent.status, agent.childThreadId]) })))
      .toEqual([{ title: 'Alpha', members: [['alpha:one', 'running', 'member-1']] }]);
  });
  test('recognizes an unmarked script before the parent loads, and an unmarked brief from the parent\'s workflow row', () => {
    expect(resolveWorkflowOpening(projection(brief(SCRIPT)), null)).toMatchObject({ script: SCRIPT, name: null, group: null });
    expect(resolveWorkflowOpening(projection(brief('Run the alpha workflow.')), [coordinator({ name: 'probe-wf', phases: [], agents: [] })])?.name).toBe('probe-wf');
  });
  test('leaves an ordinary subagent thread, other threads and a brief marked as not a script alone', () => {
    expect(resolveWorkflowOpening(projection(brief('Audit the last five commits.')), [coordinator()])).toBeNull();
    expect(resolveWorkflowOpening({ thread: { ...thread, lineage: { relationshipToParent: null, parentThreadId: null } }, messages: [brief(SCRIPT)] }, null)).toBeNull();
    expect(isWorkflowScriptMessage({ role: 'user', text: 'Explain `export const meta`.' })).toBe(false);
    expect(isWorkflowScriptMessage({ role: 'assistant', text: SCRIPT })).toBe(false);
    expect(isWorkflowScriptMessage(brief(SCRIPT, false))).toBe(false);
    expect(resolveWorkflowOpening(projection(brief(SCRIPT, false)), [coordinator()])).toBeNull();
    expect(workflowOpeningParentThreadId(projection(brief(SCRIPT, false)))).toBeNull();
  });
  test('only a thread opening with a script loads its parent', () => {
    expect(workflowOpeningParentThreadId(projection(brief(SCRIPT, true)))).toBe('parent');
    expect(workflowOpeningParentThreadId(projection(brief(SCRIPT)))).toBe('parent');
    expect(workflowOpeningParentThreadId(projection(brief('Audit the last five commits.')))).toBeNull();
  });
  test('heads the opening with the workflow name, description and agent tally', () => {
    const opening = resolveWorkflowOpening(projection(brief(SCRIPT, true)), [coordinator({ name: 'probe-wf', phases: [{ index: 1, title: 'Alpha' }],
      agents: [member(), member({ index: 2, label: 'alpha:two', state: 'completed' }), member({ index: 3, label: 'loose', state: 'failed', phaseIndex: undefined, phaseTitle: undefined })] })]);
    expect(opening && workflowOpeningHeadline(opening)).toEqual({ title: 'probe-wf', description: 'Run the alpha workflow', agents: '3 agents · 1 working · 1 done · 1 failed' });
    const unloaded = resolveWorkflowOpening(projection(brief(SCRIPT)), null);
    expect(unloaded && workflowOpeningHeadline(unloaded)).toEqual({ title: 'Workflow', description: null, agents: null });
  });
  test('the script is a fenced block longer than any backtick run in it', () => {
    expect(scriptMarkdown('a ```` b\n')).toBe('`````javascript\na ```` b\n`````');
    expect(scriptMarkdown('plain')).toBe('```javascript\nplain\n```');
  });
});

// The parent's bounded detail (client-fixture.ts's shape).
const parentDetail = (workflow: Obj): Obj => ({ snapshotSequence: 10, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0,
  projection: { thread: { id: 'parent', projectId: 'p1', title: 'Parent', modelSelection: { instanceId: 'claude', model: 'm' }, runtimeMode: 'approval-required', interactionMode: 'default' },
    runs: [], attempts: [], nodes: [], subagents: [coordinator(workflow)], providerSessions: [], providerThreads: [], providerTurns: [], runtimeRequests: [], messages: [], plans: [],
    turnItems: [], checkpointScopes: [], checkpoints: [], contextHandoffs: [], contextTransfers: [], visibleTurnItems: [], updatedAt: '2026-10-21T00:00:00Z' } });

describe('the opening in the timeline', () => {
  const workflow = { name: 'probe-wf', phases: [{ index: 1, title: 'Alpha' }],
    agents: [member({ childThreadId: 'member-1' }), member({ index: 2, label: 'alpha:two', state: 'failed', result: 'Boom `x`', completedAt: Date.parse('2026-10-21T00:01:00Z') }),
      member({ index: 3, label: 'loose', state: 'completed', phaseIndex: undefined, phaseTitle: undefined, result: 'Done' })] };
  test('phases head their members, drawn as subagent rows that open their threads; the unphased follow', () => {
    const opening = resolveWorkflowOpening(projection(brief(SCRIPT, true)), [coordinator(workflow)])!;
    const rows = workflowOpeningRows(opening, { agents: [], providers: [] });
    expect(rows.map(row => [row.tone, row.label, row.result, row.targetId ?? ''])).toEqual([
      ['phase', 'Alpha', '1/2', ''], ['info', 'alpha:one', 'Running', 'member-1'], ['error', 'alpha:two', 'Failed', ''], ['gap', '', '', ''], ['success', 'loose', 'Completed', '']]);
    expect(rows[0]!.detail).toBe('first');
    // A settled member reads its error or result; a live one its progress.
    expect(rows[2]!.body).toBe('Boom x');
  });
  test("the script's message is drawn as the opening once the parent's coordinator row is fetched", async () => {
    const turnItems = [{ id: 'i1', type: 'user_message', messageId: 'message-1', text: SCRIPT, runId: null, status: 'completed', inputIntent: 'turn_start', attachments: [], startedAt: '2026-10-21T00:00:00Z', updatedAt: '2026-10-21T00:00:00Z' }];
    const state = { projection: { ...projection(brief(SCRIPT, true)), runs: [], attempts: [], nodes: [], checkpoints: [],
      visibleTurnItems: turnItems.map((item, position) => ({ position, visibility: 'local', sourceThreadId: 'child', sourceItemId: item.id, item })) } };
    const requests: string[] = [];
    const client = { threadId: 'child', projectId: 'p1', generation: 1, shell: { projects: [], threads: [{ id: 'parent', updatedAt: 'a' }] }, config: { providers: [] },
      local: { deviceSettings: { timestampFormat: '24-hour' } }, thread: state, projection: state.projection,
      restAccess: () => ({ http: async (path: string) => { requests.push(path); return parentDetail(workflow); } }) } as unknown as T3Client;
    // Until the parent loads, the opening reads "Workflow" with no agents.
    expect(transcriptRows(client)[0]).toMatchObject({ kind: 'workflow', title: 'Workflow', actionLabel: '', activities: [] });
    await syncWorkflowOpening(client, { available: true } as unknown as Native, 1_000);
    expect(requests).toEqual(['/api/orchestration/threads/parent/bounded']);
    const opening = transcriptRows(client)[0]!;
    expect(opening).toMatchObject({ kind: 'workflow', title: 'probe-wf', detail: 'Run the alpha workflow', actionLabel: '3 agents · 1 working · 1 done · 1 failed', collapsible: false });
    expect(opening.body).toBe(scriptMarkdown(SCRIPT));
    // The parent's shell has not moved: no second fetch.
    await syncWorkflowOpening(client, { available: true } as unknown as Native, 10_000);
    expect(requests).toHaveLength(1);
  });
});

describe('thread find in the opening', () => {
  test("marks the query's matches in a line, case-insensitively, and leaves a line without one whole", () => {
    expect(findLine('title', 'Probe the Probe', 'probe').parts).toEqual([{ id: '0', text: 'Probe', find: 1 }, { id: '1', text: ' the ', find: 0 }, { id: '2', text: 'Probe', find: 1 }]);
    expect(findLine('detail', 'Nothing here', 'probe').parts).toEqual([{ id: '0', text: 'Nothing here', find: 0 }]);
    expect(findLine('detail', '', '').parts).toEqual([{ id: '0', text: '', find: 0 }]);
  });
  test('the title, the description and each phase title are find lines', () => {
    const opening = resolveWorkflowOpening(projection(brief(SCRIPT, true)), [coordinator({ name: 'probe-wf', phases: [{ index: 1, title: 'Alpha probe' }], agents: [member()] })])!;
    const message = workflowOpeningMessage(opening, { agents: [], providers: [] }, 'probe');
    expect(message.findLines.map(line => [line.id, line.parts.filter(part => part.find === 1).map(part => part.text)])).toEqual([
      ['title', ['probe']], ['detail', []], ['phase:1', ['probe']]]);
  });
});
