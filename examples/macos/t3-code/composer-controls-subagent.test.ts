// Lane composer-controls: the provider-native subagent bar (ProviderSubagentBar).
import { describe, expect, test } from 'bun:test';
import { obj } from './domain';
import { snapshot } from './presentation';
import { formatDuration, formatSubagentStatus, subagentBar } from './composer-controls-subagent';
import { opened } from './composer-controls-fixture';

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
