// Lane composer-controls: the provider-native subagent bar (ProviderSubagentBar).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import type { Files, Native } from '../../src/client/protocol';
import { composerCommand } from '../../src/composer/composer-controls-commands';
import { snapshot } from '../../src/client/presentation';
import { formatSubagentStatus, selectionEffort, subagentBar } from '../../src/composer/composer-controls-subagent';
import { subagentTraits } from '../../src/sidebar/subagent-card';
import { formatDuration } from '../../src/timeline/timeline-worklog';
import { opened } from '../support/composer-controls-fixture';

describe('provider subagent bar', () => {
  test('durations and status lines read as the reference formats them', () => {
    expect([500, 1_000, 9_960, 12_000, 61_000, 3_725_000].map(formatDuration)).toEqual(['500ms', '1.0s', '10s', '12s', '1m 1s', '1h 2m 5s']);
    const start = '2026-10-03T01:00:00.000Z', t = Date.parse(start);
    expect(formatSubagentStatus(null, t)).toBe('Starting');
    expect(formatSubagentStatus({ status: 'running', startedAt: start, completedAt: '' }, t + 12_400)).toBe('Working 12s');
    expect(formatSubagentStatus({ status: 'completed', startedAt: start, completedAt: '2026-10-03T01:00:34.000Z' }, 0)).toBe('Completed in 34s');
    expect(formatSubagentStatus({ status: 'failed', startedAt: start, completedAt: '' }, 0)).toBe('Failed');
  });
  test('a provider-native subagent thread replaces the composer with the bar and drops its dock', async () => {
    const { client } = await opened();
    expect(snapshot(client, Date.now()).composer.subagent).toBe(false);
    const thread = obj(client.thread!.projection.thread);
    Object.assign(thread, { lineage: { relationshipToParent: 'subagent', parentThreadId: 't2' }, creationSource: 'provider',
      modelSelection: { instanceId: 'codex', model: 'model-a', options: [{ id: 'reasoningEffort', value: 'high' }] } });
    client.thread!.projection.nodes = [{ id: 'n1', kind: 'root_turn', runId: null, status: 'running', startedAt: '2026-10-03T01:00:00.000Z', completedAt: null }];
    const bar = subagentBar(client, Date.parse('2026-10-03T01:00:05.000Z'));
    expect(bar).toEqual({ subagent: true, subagentModel: 'Model A', subagentEffort: 'High', subagentStatus: 'Working 5.0s',
      subagentAnnounce: 'Model A, High subagent: Working', subagentParent: 't2' });
    const view = snapshot(client, Date.parse('2026-10-03T01:00:05.000Z')).composer;
    expect(view).toMatchObject({ subagent: true, notices: [], queued: [], activity: '' });
    Object.assign(thread, { creationSource: 'web' });
    expect(subagentBar(client, 0).subagent).toBe(false);
  });
});

describe('subagent effort (T3 Code 74541bab17)', () => {
  const effort = { id: 'effort', label: 'Effort', type: 'select', currentValue: 'medium',
    options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High', isDefault: true }, { id: 'xhigh', label: 'Extra High' }] };
  const models = [{ slug: 'claude-opus-5', capabilities: { optionDescriptors: [effort] } }];
  const selection = (options: Obj[] = [], extra: Obj = {}) => ({ instanceId: 'claudeAgent', model: 'claude-opus-5', options, ...extra });

  test('the bar names a stated effort, else the matching report, and never guesses a catalog default (#15710)', () => {
    expect(selectionEffort(selection([{ id: 'effort', value: 'xhigh' }]), models)).toBe('Extra High');
    const reported = selection([{ id: 'effort', value: 'xhigh' }]);
    expect(selectionEffort(selection(), models, reported)).toBe('Extra High');
    expect(selectionEffort(selection([{ id: 'effort', value: 'medium' }]), models, reported)).toBe('Medium');
    expect(selectionEffort(selection([{ id: 'effort', value: 'retired-effort' }]), models, reported)).toBe('');
    expect(selectionEffort(selection(), models)).toBe('');
    expect(selectionEffort(selection(), models, { ...reported, model: 'claude-opus-5-5' })).toBe('');
    expect(selectionEffort(selection(), models, { ...reported, instanceId: 'claudeAgent-other' })).toBe('');
  });

  test('Cursor\'s reasoning_effort is an effort for the bar and the hover card (#17419)', () => {
    const cursor = [{ slug: 'gpt-6', capabilities: { optionDescriptors: [{ ...effort, id: 'reasoning_effort' }] } }];
    const chosen = { instanceId: 'cursor', model: 'gpt-6', options: [{ id: 'reasoning_effort', value: 'high' }] };
    expect(selectionEffort(chosen, cursor)).toBe('High');
    expect(subagentTraits({ model: 'gpt-6', provider: { driver: 'cursor', models: cursor }, providerInstanceId: 'cursor', modelSelection: chosen }).effort).toBe('high');
  });
});

describe('Stop on the waiting-on-background-work notice', () => {
  // interruptThreadTurn: the active run, else the latest run while background work is pending, always with holdQueue.
  async function stop(runs: Obj[], pending: Obj[]) {
    const commands: Obj[] = [];
    const client = { threadId: 't1', thread: {}, projection: { runs, thread: { pullRequests: [{ host: 'github.com', repository: 'o/r', number: 7, watch: {} }] } },
      shell: { threads: [{ id: 't1', pendingBackgroundTasks: pending }] },
      restAccess: () => ({ ids: async () => ['c1'], dispatch: async (_storage: Files, payload: Obj) => { commands.push(payload); } }) } as unknown as T3Client;
    await composerCommand(client, {} as Native, {} as Files, 'stop-background', '', '');
    return commands;
  }
  const run = (id: string, status: string) => ({ id, status, ordinal: Number(id.slice(1)) });
  const child = [{ kind: 'subagent', description: 'Long child', childThreadId: 'thread:child' }];
  test('a settled parent with a running delegated child interrupts its latest run and holds the queue', async () => {
    expect(await stop([run('r1', 'completed'), run('r2', 'completed')], child))
      .toEqual([{ type: 'run.interrupt', commandId: 'c1', threadId: 't1', runId: 'r2', holdQueue: true }]);
  });
  test('an active run is the one stopped', async () => {
    expect(await stop([run('r1', 'waiting'), run('r2', 'queued')], child))
      .toEqual([{ type: 'run.interrupt', commandId: 'c1', threadId: 't1', runId: 'r1', holdQueue: true }]);
  });
  test('with no pending work and no active run, only pull request watches stop', async () => {
    expect(await stop([run('r1', 'completed')], [])).toEqual([{ type: 'thread.pull-request.watch', commandId: 'c1', threadId: 't1', host: 'github.com', repository: 'o/r', number: 7, watching: false }]);
  });
});
