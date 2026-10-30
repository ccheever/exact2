// Pending approvals on their timeline row (T3 Code v0.1.5 TimelinePendingApprovalRow, MessagesTimeline.logic; #26255).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Message, Obj } from '../../src/client/domain';
import { transcriptRows } from '../../src/timeline/timeline-presentation';
import { noteComposerTyping } from '../../src/composer/composer-typing-guard';
import { composerRequests } from '../../src/composer/requests';

const base = Date.parse('2026-10-28T00:00:00.000Z');
const at = (seconds: number) => new Date(base + seconds * 1000).toISOString();
let ordinal = 0;
const item = (id: string, type: string, seconds: number, extra: Obj = {}): Obj =>
  ({ id, threadId: 't1', runId: 'r1', nodeId: null, ordinal: ++ordinal, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra });
const user = (id: string, seconds: number) => item(id, 'user_message', seconds, { text: 'go', messageId: `m-${id}`, inputIntent: 'turn_start', attachments: [], createdBy: 'user' });
const command = (id: string, seconds: number, input: string) => item(id, 'command_execution', seconds, { input, output: '', exitCode: 0 });
const approvalItem = (id: string, seconds: number, requestId: string, extra: Obj = {}) =>
  item(id, 'approval_request', seconds, { status: 'running', requestId, requestKind: 'command', prompt: 'rm -rf build', ...extra });
const request = (id: string, kind: string, capability = 'live') => ({ id, kind, status: 'pending', createdAt: at(1), responseCapability: { type: capability } });

function client(items: Obj[], requests: Obj[], extra: Obj = {}): T3Client {
  const projection = { thread: { id: 't1', projectId: 'p1' }, runs: [{ id: 'r1', status: 'running', startedAt: at(0), completedAt: null }],
    attempts: [], nodes: [], checkpoints: [], turnItems: items, runtimeRequests: requests,
    visibleTurnItems: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })) };
  return { threadId: 't1', projectId: 'p1', environmentId: 'env', draftKey: 'env:t1', scopes: ['orchestration:operate'], local: { pending: {}, deviceSettings: { timestampFormat: '24-hour' } },
    shell: { projects: [{ id: 'p1', workspaceRoot: '/work' }], threads: [] }, config: { providers: [] }, thread: { projection }, projection, ...extra } as unknown as T3Client;
}
const approvals = (rows: Message[]) => rows.flatMap(row => row.approval ?? []);

describe('a pending approval on its timeline row', () => {
  test('a command approval shows what it runs, Approve and Decline on the row, the rest under More', () => {
    const value = client([user('u', 0), command('ls', 1, 'ls'), approvalItem('a', 2, 'rq', { options: [
      { decision: 'accept', label: 'Approve' }, { decision: 'acceptForSession', label: 'Always allow this session', warning: 'Skips later prompts' },
      { decision: 'decline', label: 'Decline' }, { decision: 'cancel', label: 'Cancel' }] })], [request('rq', 'command')]);
    const rows = transcriptRows(value);
    const row = rows.find(row => (row.approval ?? []).length > 0)!;
    expect(row).toMatchObject({ id: JSON.stringify(['t1', 'a']), kind: 'entry', activities: [] });
    expect(row.approval![0]).toMatchObject({ id: 'rq', icon: 'terminal', label: 'Command approval', detailLabel: 'Command', detail: 'rm -rf build', live: true, operable: true, responding: false,
      options: [{ id: 'accept', label: 'Approve' }, { id: 'decline', label: 'Decline' }],
      more: [{ id: 'acceptForSession', warning: 'Skips later prompts' }, { id: 'cancel', label: 'Cancel' }] });
    // Nothing is thinking while the agent waits for the answer.
    expect(rows.some(row => row.kind === 'thinking')).toBe(false);
  });

  test('each request kind names its row and its detail', () => {
    for (const [kind, label, detailLabel, icon] of [
      ['command', 'Command approval', 'Command', 'terminal'], ['file-change', 'File change approval', 'File change', 'square-pen'],
      ['file-read', 'File read approval', 'File to read', 'eye'], ['permission', 'App permission approval', 'Permission request', 'wrench'],
      ['mcp-elicitation', 'App access approval', 'App access request', 'hammer']] as const) {
      const rows = transcriptRows(client([user('u', 0), approvalItem('a', 1, 'rq', { prompt: '' })], [request('rq', kind)]));
      expect(approvals(rows)).toMatchObject([{ label, detailLabel, icon, detail: label }]);
    }
  });

  test('full access that still asks says why; a gone provider takes no answer', () => {
    const asked = transcriptRows(client([user('u', 0), approvalItem('a', 1, 'rq', { fullAccessOverride: 'ask_rule', appName: 'Figma', reason: 'Reads the file' })], [request('rq', 'command')]));
    expect(approvals(asked)[0]).toMatchObject({ appName: 'Figma', reason: 'Reads the file' });
    expect(approvals(asked)[0]!.whyAsked).toStartWith('Full access skips approvals, but an ask rule');
    const gone = transcriptRows(client([user('u', 0), approvalItem('a', 1, 'rq')], [request('rq', 'command', 'not_resumable')]));
    expect(approvals(gone)).toMatchObject([{ live: false }]);
  });

  test('a pending approval stays out of the tool group and the live row; an answered one is an ordinary entry again', () => {
    const items = [user('u', 0), command('ls', 1, 'ls'), approvalItem('a', 2, 'rq'), command('pwd', 3, 'pwd')];
    const pending = transcriptRows(client(items, [request('rq', 'command')]));
    const key = (id: string) => JSON.stringify(['t1', id]);
    expect(pending.map(row => [row.kind, row.id, (row.approval ?? []).length])).toEqual([['user', key('u'), 0], ['working', 'working-indicator-row', 0],
      ['entry', key('ls'), 0], ['entry', key('a'), 1], ['live', `work-live:${key('pwd')}`, 0]]);
    // The work after it no longer reads as live while it waits.
    expect(pending.at(-1)).toMatchObject({ live: false });
    const answered = transcriptRows(client(items, [{ ...request('rq', 'command'), status: 'resolved' }]));
    expect(answered.map(row => [row.kind, row.id])).toEqual([['user', key('u')], ['working', 'working-indicator-row'], ['live', 'live-activity-row']]);
  });

  test('an approval whose item is not loaded still answers, after the rows', () => {
    const rows = transcriptRows(client([user('u', 0)], [request('rq', 'command')]));
    expect(rows.at(-1)).toMatchObject({ id: 'pending-approval:rq', kind: 'entry', approval: [{ id: 'rq', label: 'Command approval' }] });
  });

  test('an approval that arrives while the user types is held: its row is not answerable until they stop', () => {
    const value = client([user('u', 0)], []);
    // The composer reads the requests on every snapshot, so its guard has seen this thread before the approval comes.
    composerRequests(value);
    noteComposerTyping(value, true);
    const arrived = client([user('u', 0), approvalItem('a', 1, 'rq')], [request('rq', 'command')]);
    Object.assign(value, { thread: arrived.thread, projection: arrived.projection });
    expect(approvals(transcriptRows(value))).toHaveLength(0);
    noteComposerTyping(value, false);
    expect(approvals(transcriptRows(value))).toMatchObject([{ id: 'rq' }]);
  });

  test('an answer on its way disables the row', () => {
    const value = client([user('u', 0), approvalItem('a', 1, 'rq')], [request('rq', 'command')], {});
    (value.local as unknown as { pending: Record<string, Obj> }).pending.env = { payload: { requestId: 'rq' } };
    Object.defineProperty(value, 'pending', { get: () => (value.local as unknown as { pending: Record<string, Obj> }).pending.env });
    expect(approvals(transcriptRows(value))).toMatchObject([{ responding: true }]);
  });
});
