import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { pendingHandoff } from '../../src/timeline/thread-status-line';

const from = 'codex', to = 'claude';
const run = (id: string, ordinal: number, instanceId: string, model: string, extra: Obj = {}): Obj =>
  ({ id, ordinal, modelSelection: { instanceId, model }, status: 'completed', contextHandoffId: null, startedAt: '2026-10-11T00:00:00Z', ...extra });
const projection: Obj = { runs: [run('source', 1, from, 'source')], visibleTurnItems: [{ item: { type: 'user_message' } }] };
const handingOff = (instanceId: string, status: string): Obj =>
  ({ ...projection, runs: [...(projection.runs as Obj[]), run('target', 2, instanceId, 'destination', { status, startedAt: null, contextHandoffId: 'handoff' })] });

describe('pending context handoff', () => {
  for (const instanceId of [from, to]) {
    test(`the preview goes while the handoff to ${instanceId} starts, so the committed handoff shows once`, () => {
      const selection = { instanceId, model: 'destination' }, providers = [{ instanceId: from, requiresNewThreadForModelChange: true }];
      const starting = handingOff(instanceId, 'starting');
      expect(pendingHandoff(projection, selection, providers)).not.toBeNull();
      expect(pendingHandoff(starting, selection, providers)).toBeNull();
      expect(pendingHandoff(starting, { instanceId: from, model: 'source' }, providers)).toEqual({ from: selection, to: { instanceId: from, model: 'source' } });
    });
  }

  for (const status of ['preparing', 'queued', 'failed', 'cancelled', 'rolled_back']) {
    test(`an unstarted handoff run that is ${status} keeps the preview`, () => {
      const selection = { instanceId: to, model: 'destination' };
      expect(pendingHandoff(handingOff(to, status), selection, [])).toEqual({ from: { instanceId: from, model: 'source' }, to: selection });
    });
  }
});
