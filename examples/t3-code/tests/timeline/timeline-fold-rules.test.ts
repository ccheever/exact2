import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { deriveRows, type RowInput } from '../../src/timeline/timeline-rows';

const base = Date.parse('2026-10-28T00:00:00.000Z');
const at = (seconds: number) => new Date(base + seconds * 1000).toISOString();
let ordinal = 0;
const item = (id: string, type: string, seconds: number, extra: Obj = {}): Obj =>
  ({ id, threadId: 't1', runId: 'r1', nodeId: null, ordinal: ++ordinal, status: 'completed', startedAt: at(seconds), updatedAt: at(seconds), type, ...extra });
const user = (id: string, seconds: number, extra: Obj = {}) =>
  item(id, 'user_message', seconds, { text: 'go', messageId: `m-${id}`, inputIntent: 'turn_start', attachments: [], createdBy: 'user', ...extra });
const command = (id: string, seconds: number, input: string, extra: Obj = {}) => item(id, 'command_execution', seconds, { input, output: '', exitCode: 0, ...extra });
const failure = (id: string, seconds: number, failureClass: string) =>
  item(id, 'error', seconds, { status: 'failed', parentItemId: null, message: 'stopped', failure: { class: failureClass } });
const key = (id: string) => JSON.stringify(['t1', id]);

function input(items: Obj[], extra: Partial<RowInput> = {}): RowInput {
  return {
    rows: items.map((value, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: value.id, item: value })),
    runs: [], attempts: [], nodes: [], checkpoints: [], isWorking: false, runningRunId: '', latestRun: null, activeStartedAt: '',
    expandedRuns: new Set(), expandedAttempts: new Set(), expandedGroups: new Set(), root: '/work', rollback: false, ...extra,
  };
}

describe('the turn folds a usage-limit pause keeps (#24937)', () => {
  const work = [user('u', 0), command('pwd', 1, 'pwd'), command('ls', 2, 'ls'), command('false', 3, 'false')];
  const failedRun = { id: 'r1', status: 'failed', startedAt: at(0), completedAt: at(5) };

  test("a usage-limit pause keeps its run's tool calls in one folded group", () => {
    const rows = deriveRows(input([...work, failure('f', 4, 'usage_limit')], { latestRun: failedRun }));
    expect(rows.map(row => row.kind)).toEqual(['message', 'group', 'entry']);
    expect(rows[1]).toMatchObject({ kind: 'group', count: 3, expanded: false });
    expect(rows[2]).toMatchObject({ id: key('f') });
  });

  test('any other failure lists the run\'s work entry by entry', () => {
    const rows = deriveRows(input([...work, failure('f', 4, 'provider_error')], { latestRun: failedRun }));
    expect(rows.map(row => row.id)).toEqual([key('u'), key('pwd'), key('ls'), key('false'), key('f')]);
    expect(rows.some(row => row.kind === 'group' || row.kind === 'fold')).toBe(false);
  });

  test('a usage limit beside another failure of the same run still lists the work', () => {
    const rows = deriveRows(input([...work, failure('f1', 4, 'usage_limit'), failure('f2', 5, 'provider_error')], { latestRun: failedRun }));
    expect(rows.some(row => row.kind === 'group')).toBe(false);
  });

  test('a failed error item ends the group before it', () => {
    const rows = deriveRows(input([user('u', 0), command('pwd', 1, 'pwd'), failure('f', 2, 'usage_limit'), command('ls', 3, 'ls')], { latestRun: failedRun }));
    expect(rows.map(row => row.kind)).toEqual(['message', 'entry', 'entry', 'entry']);
    expect(rows.map(row => row.id)).toEqual([key('u'), key('pwd'), key('f'), key('ls')]);
  });
});

describe('a superseded attempt fold (#26333)', () => {
  test('a handoff before the attempt a steer cut off stays outside its fold', () => {
    const items = [
      item('handoff', 'handoff', 0, { nodeId: 'n1' }),
      user('u', 1),
      item('old', 'assistant_message', 2, { nodeId: 'n1', text: 'Partial', messageId: 'm-old', streaming: false }),
      user('s', 3, { inputIntent: 'steer' }),
      item('new', 'assistant_message', 4, { nodeId: 'n2', text: 'Done', messageId: 'm-new', streaming: false }),
    ];
    const attempts = [{ id: 'a1', runId: 'r1', rootNodeId: 'n1', status: 'superseded' }, { id: 'a2', runId: 'r1', rootNodeId: 'n2', status: 'completed' }];
    const rows = deriveRows(input(items, { attempts, runs: [{ id: 'r1', status: 'completed' }], expandedRuns: new Set(['r1']) }));
    expect(rows.map(row => row.id)).toEqual([key('handoff'), key('u'), 'turn-fold:r1', 'attempt-fold:a1', key('s'), key('new')]);
  });
});

describe('a queued message still pending (#26355)', () => {
  test('is the queue\'s row, not a timeline message; once it starts it is one', () => {
    const queued = (status: string) => user('q', 2, { inputIntent: 'queued_turn', status, runId: 'r2' });
    expect(deriveRows(input([user('u', 0), queued('pending')])).map(row => row.id)).toEqual([key('u')]);
    expect(deriveRows(input([user('u', 0), queued('completed')])).map(row => row.id)).toEqual([key('u'), key('q')]);
  });
});
