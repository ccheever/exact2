import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { adoptWorktreeSetup, agentStarted, setupView, threadWorktreeSetup, visibleWorktreeSetup, worktreeSetupEvent, wantsWorktreeSetup } from './timeline-worktree';

const at = (seconds: number) => new Date(Date.parse('2026-10-03T00:00:00.000Z') + seconds * 1000).toISOString();
const stage = (id: string, status: string, extra: Obj = {}) => ({ id, status, startedAt: status === 'pending' ? null : at(0), endedAt: ['done', 'failed', 'skipped', 'warning'].includes(status) ? at(3) : null, percent: null, detail: null, tail: [], ...extra });
function snapshot(phase: string, stages: Obj[], extra: Obj = {}): Obj {
  return { threadId: 't1', phase, startedAt: at(0), endedAt: phase === 'running' ? null : at(9), branch: 't3code/fixture-branch', baseRef: 'origin/main',
    worktreePath: '/work/.t3/worktrees/fixture', setupScript: { name: 'Install deps', command: 'bun install', terminalId: 'setup-1' }, stages, error: null, sequence: 3, ...extra };
}
function client(projection: Obj = {}): T3Client {
  const thread = { projection: { thread: { id: 't1', projectId: 'p1', worktreePath: '/work/.t3/worktrees/fixture' }, runs: [], visibleTurnItems: [], ...projection } };
  return { threadId: 't1', thread, projection: thread.projection, ready: true } as unknown as T3Client;
}

describe('worktree setup card (3a7058d)', () => {
  const running = snapshot('running', [stage('fetch', 'done', { detail: 'origin/main' }), stage('checkout', 'running', { percent: 42 }), stage('submodules', 'pending'),
    stage('setup-script', 'pending'), stage('agent', 'pending')]);
  test('stages read like work rows: labels, the checkout percentage, pending rows dimmed by status', () => {
    const view = setupView(running, false, false);
    expect(view.header).toBe('Setting up worktree…');
    expect(view.stages.map(entry => [entry.label, entry.status, entry.trailing])).toEqual([
      ['Fetch base branch', 'done', 'origin/main'], ['Check out files', 'running', '42%'], ['Init submodules', 'pending', ''],
      ['Install deps', 'pending', ''], ['Start agent', 'pending', '']]);
    expect(view).toMatchObject({ showHeader: false, collapsed: false, cancel: true, error: '' });
    expect(view.details.map(entry => `${entry.id}: ${entry.text}`)).toEqual(['Branch: t3code/fixture-branch', 'Base: origin/main', 'Path: /work/.t3/worktrees/fixture', 'Setup: bun install']);
  });
  test('the script tail appears with its first line, four fixed slots, and stays after a failure', () => {
    const silent = setupView(snapshot('running', [stage('setup-script', 'running')]), false, false);
    expect(silent.stages[0]!.tail).toEqual([]);
    const printing = setupView(snapshot('running', [stage('setup-script', 'running', { tail: ['resolving', 'fetching'] })]), false, false);
    expect(printing.stages[0]!.tail.map(line => line.text)).toEqual(['', '', 'resolving', 'fetching']);
    const failed = setupView(snapshot('done', [stage('setup-script', 'failed', { detail: 'exit 1' }), stage('agent', 'done')]), false, false);
    expect(failed).toMatchObject({ header: 'Worktree ready, setup script failed', tone: 'warning', showHeader: false, summary: 'failed' });
    expect(failed.stages[0]!.tail).toHaveLength(4);
  });
  test('only a failed or cancelled setup brings its own header; a handed-off one collapses', () => {
    expect(setupView(snapshot('failed', [stage('fetch', 'failed')], { error: 'git fetch failed' }), false, false)).toMatchObject({ showHeader: true, header: 'Worktree setup failed', tone: 'failed', error: 'git fetch failed' });
    expect(setupView(snapshot('cancelled', [stage('fetch', 'done')]), false, false)).toMatchObject({ showHeader: true, header: 'Worktree setup cancelled', summary: 'failed' });
    expect(setupView(snapshot('done', [stage('agent', 'done')]), false, false)).toMatchObject({ showHeader: false, header: 'Worktree ready' });
    expect(setupView(snapshot('done', [stage('agent', 'done')]), true, false)).toMatchObject({ collapsed: true, cancel: false });
  });
  test('visibility: running always, a clean finish leaves once the turn is live, a follow-up retires every outcome', () => {
    const done = snapshot('done', [stage('agent', 'done')]), scriptFailed = snapshot('done', [stage('setup-script', 'failed'), stage('agent', 'done')]);
    expect(visibleWorktreeSetup(running, true, true)).toBe(running);
    expect(visibleWorktreeSetup(done, false, false)).toBe(done);
    expect(visibleWorktreeSetup(done, true, false)).toBeNull();
    expect(visibleWorktreeSetup(scriptFailed, true, false)).toBe(scriptFailed);
    expect(visibleWorktreeSetup(scriptFailed, true, true)).toBeNull();
    expect(agentStarted(done)).toBe(true);
  });
  test('the stream keeps the newest snapshot for the subscribed thread', () => {
    const value = client();
    adoptWorktreeSetup(value, running);
    expect(threadWorktreeSetup(value)).toMatchObject({ snapshot: running, preparing: true });
    expect(wantsWorktreeSetup(value)).toBe('t1');
  });
  test('a stream event for another subscription or an older sequence is ignored', () => {
    const value = client();
    adoptWorktreeSetup(value, running);
    worktreeSetupEvent(value, { subscriptionId: 'other', value: snapshot('done', [], { sequence: 9 }) });
    expect(threadWorktreeSetup(value).snapshot?.phase).toBe('running');
  });
});

describe('the setup card in the timeline', () => {
  const user = { position: 0, visibility: 'local', sourceThreadId: 't1', sourceItemId: 'u', item: { id: 'u', threadId: 't1', runId: 'r1', type: 'user_message', text: 'build it', messageId: 'm-u', inputIntent: 'turn_start', attachments: [], createdBy: 'user', status: 'completed', startedAt: at(0), updatedAt: at(0) } };
  function timelineClient(run: Obj, setup: Obj | null): T3Client {
    const thread = { projection: { thread: { id: 't1', projectId: 'p1', worktreePath: '/work/.t3/worktrees/fixture' }, runs: [run], attempts: [], nodes: [], checkpoints: [], subagents: [], visibleTurnItems: [user] } };
    const value = { threadId: 't1', projectId: 'p1', ready: true, shell: { projects: [{ id: 'p1', workspaceRoot: '/work/project' }], threads: [] }, config: { providers: [] },
      local: { deviceSettings: { timestampFormat: '24-hour' } }, thread, projection: thread.projection } as unknown as T3Client;
    adoptWorktreeSetup(value, setup);
    return value;
  }
  test('a running setup sits under "Setting up worktree…" in the working slot, with no Thinking row', async () => {
    const { transcriptRows } = await import('./timeline-presentation');
    const rows = transcriptRows(timelineClient({ id: 'r1', status: 'preparing', requestedAt: at(0), startedAt: null, completedAt: null },
      snapshot('running', [stage('fetch', 'running'), stage('agent', 'pending')])));
    expect(rows.map(row => row.kind)).toEqual(['user', 'working', 'worktree-setup']);
    expect(rows[1]).toMatchObject({ title: 'Setting up worktree…', live: true });
    expect(rows[2]!.setup![0]).toMatchObject({ header: 'Setting up worktree…', cancel: true, collapsed: false });
  });
  test('a finished setup keeps the slot until the run starts, then leaves', async () => {
    const { transcriptRows } = await import('./timeline-presentation');
    const done = snapshot('done', [stage('fetch', 'done'), stage('agent', 'done')]);
    const waiting = transcriptRows(timelineClient({ id: 'r1', status: 'starting', requestedAt: at(0), startedAt: null, completedAt: null }, done));
    expect(waiting.map(row => row.kind)).toEqual(['user', 'working', 'worktree-setup']);
    expect(waiting[1]).toMatchObject({ title: 'Working for' });
    const live = transcriptRows(timelineClient({ id: 'r1', status: 'running', requestedAt: at(0), startedAt: at(10), completedAt: null }, done));
    expect(live.map(row => row.kind)).toEqual(['user', 'working', 'thinking']);
  });
  test('a failed script outlives the handoff as one collapsed row under the send', async () => {
    const { transcriptRows } = await import('./timeline-presentation');
    const failed = snapshot('done', [stage('setup-script', 'failed', { detail: 'exit 1' }), stage('agent', 'done')]);
    const rows = transcriptRows(timelineClient({ id: 'r1', status: 'running', requestedAt: at(0), startedAt: at(10), completedAt: null }, failed));
    expect(rows.map(row => row.kind)).toEqual(['user', 'worktree-setup', 'working', 'thinking']);
    expect(rows[1]!.setup![0]).toMatchObject({ collapsed: true, summary: 'failed', header: 'Worktree ready, setup script failed' });
  });
});
