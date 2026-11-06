// The root's one-second tick (app.contract liveTick) dispatches the timeline's reads while the last snapshot set
// timelineReadsNeeded. Their mutations refresh `data` as they land, and a refresh replaces an answer in flight, so
// the tick leaves a pending data answer alone, as the highlight pump does (highlight-pump.test.ts).
// The action runs with the Contract action semantics of contract-action-fixture.ts.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

type Answer = { name: string; timelineReadsNeeded: boolean; running: boolean; subagentsLive: boolean; sidebar: { ticking: boolean; undoUntil: number } };

describe('the live tick leaves a data answer in flight alone', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const tick = (needed: boolean, inFlight: string[]) => {
    const dispatched: string[] = [];
    const data: Answer = { name: 'data', timelineReadsNeeded: needed, running: false, subagentsLive: false, sidebar: { ticking: false, undoUntil: 0 } };
    root.call('liveTick', { workspaceNow: 0 }, {
      data, settingsOpen: false, pageCover: false, welcome: { show: false },
      workspaceDiscovery: { name: 'workspaceDiscovery', needed: false },
      prDetail: { links: { polling: false } }, sideChatThread: '',
      wallTime: { epochAtZero: 0 }, liveNow: 0, performanceNow: () => 1000,
      pending: (answer: { name: string }) => inFlight.includes(answer.name),
      dispatchTimelineReads: () => dispatched.push('reads'),
    }, []);
    return dispatched;
  };

  test('reads go out once the data answer has landed, never while it is in flight', () => {
    expect(tick(true, [])).toEqual(['reads']);
    expect(tick(true, ['data'])).toEqual([]);
    expect(tick(false, [])).toEqual([]);
    // A launch's snapshot answer that waits seconds on its connections: no tick replaces it.
    expect(Array.from({ length: 10 }, () => tick(true, ['data'])).flat()).toEqual([]);
  });
});
