// A queued message's grip drag starts fresh each time, as the reference's does:
// QueuedRunsControl.tsx (T3 Code 1e2ecbd975, MIT, see LICENSE-T3) sets a new `dragState` ({ runId, insertIndex: null })
// on each dragstart and takes the insertion index from the row under the pointer as it is (`onDragOver`).
//
// This runs QueuedControl's own `drag` action and derives from composer-controls.contract with Contract's action
// semantics (contract-action-fixture.ts), over the client's own snapshot and drop (composer-controls-queue.ts
// `queuedDrop`). Main's `analyze-read-after-assign` (LLP 1116.003 D6) found the old body
// adding a move to the `dragDy` its start branch had just reset: a drag after an earlier one started at that drag's
// offset, so its insertion line and its drop were shifted by it.
import { describe, expect, test } from 'bun:test';
import { arr } from '../../src/client/domain';
import { snapshot } from '../../src/client/presentation';
import { opened, running } from '../support/composer-controls-fixture';
import { contractComponent } from '../support/contract-action-fixture';
import type { T3Client } from '../../src/client/client';

const control = contractComponent('composer-controls.contract', 'QueuedControl');
const helpers = { min: Math.min, max: Math.max, floor: Math.floor };

/** Three queued runs behind the running turn, in the server's `order`. */
function queue(client: T3Client, order: string[]) {
  const active = arr(client.thread!.projection.runs).filter(run => run.status === 'running');
  client.thread!.projection.runs = [...active, ...order.map((id, i) => ({ id, ordinal: 2 + i, status: 'queued', queuePosition: i + 1, userMessageId: `m${id.slice(1)}` }))];
}

async function grip() {
  const { client, native, command } = await opened();
  running(client, { activeAttemptId: 'a1', providerThreadId: 'pt' });
  Object.assign(client.thread!.projection, {
    messages: [{ id: 'm1', text: 'One' }, { id: 'm2', text: 'Two' }, { id: 'm3', text: 'Three' }],
    providerThreads: [{ id: 'pt', providerSessionId: 's' }], providerSessions: [{ id: 's', capabilities: { turns: { supportsQueuedMessages: true } } }],
    providerTurns: [{ runAttemptId: 'a1', status: 'running' }] });
  queue(client, ['q1', 'q2', 'q3']);
  const state = control.state();
  const scope = () => ({ ...helpers, data: snapshot(client) });
  const view = () => {
    const { dragLive, dragInsert } = control.derives(state, scope()) as { dragLive: boolean; dragInsert: number };
    return { dy: state.dragDy, from: state.dragFrom, insert: dragInsert, drop: dragLive ? `${state.dragRun}|${dragInsert}` : '' };
  };
  const index = (id: string) => snapshot(client).composer.queued.findIndex(row => row.runId === id);
  return {
    client, view,
    /** One pan event on `id`'s grip: the pointer moved `dy` points since the last one. */
    move: (id: string, dy: number) => control.call('drag', state, scope(), [id, index(id), 0, dy]),
    /** The pan's release: the client takes the drop (and bumps queueDropSeq); the server's new order follows. */
    release: async (order?: string[]) => { await command('cc:queued-drop', view().drop); if (order) queue(client, order); },
    reorders: () => native.committed.filter(entry => entry.type === 'queued-run.reorder').map(entry => [entry.runId, entry.beforeRunId]),
  };
}

describe('a queued message grip drag starts at its press (composer-controls.contract QueuedControl.drag)', () => {
  test('each move adds to the drag its press started; the drop moves the run to the insertion line', async () => {
    const g = await grip();
    g.move('q1', 10);
    expect(g.view()).toEqual({ dy: 10, from: 0, insert: 1, drop: 'q1|1' }); // on its own row: no move
    g.move('q1', 10);
    g.move('q1', 10);
    expect(g.view()).toEqual({ dy: 30, from: 0, insert: 2, drop: 'q1|2' }); // past the second row's middle
    await g.release(['q2', 'q1', 'q3']);
    expect(g.reorders()).toEqual([['q1', 'q3']]);
  });

  test('a drag on another run after a drop takes nothing from the last one', async () => {
    const g = await grip();
    for (let i = 0; i < 3; i++) g.move('q1', 10);
    await g.release(['q2', 'q1', 'q3']);
    g.move('q3', -3);
    // The old body gave { dy: 27, insert: 3 }: the last drag's 30 points, the line under the last row.
    expect(g.view()).toEqual({ dy: -3, from: 2, insert: 2, drop: 'q3|2' });
    g.move('q3', -27);
    expect(g.view()).toEqual({ dy: -30, from: 2, insert: 1, drop: 'q3|1' });
  });

  test('the same run dragged again after its drop starts from where it now is', async () => {
    const g = await grip();
    for (let i = 0; i < 3; i++) g.move('q1', 10);
    await g.release(['q2', 'q1', 'q3']);
    g.move('q1', 5);
    // The old body gave { dy: 35, insert: 3 }, and its drop moved the run to the end.
    expect(g.view()).toEqual({ dy: 5, from: 1, insert: 2, drop: 'q1|2' });
    await g.release();
    expect(g.reorders()).toEqual([['q1', 'q3']]); // the second drop is on its own row: no move
  });
});
