// AssistantCitationSource.tsx (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): View source brings the cited answer's text to
// min(120px, a third of the scroller) under its top. The root (app.contract T3Code citePlace) places it with the
// transcript's keyed scrollIntoView, from the frames the host reports, until it holds.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const root = contractComponent('app.contract', 'T3Code');
const column = contractComponent('app-main.contract', 'ChatColumn');
const KEY = '["t1","a2"]';

/** The column's citation half over a 600pt transcript at y 50; `frames` says where the row and its answer are drawn. */
function citation(frames: Record<string, { x?: number; y: number; width?: number; height: number } | null>, reduced = false) {
  const calls: unknown[][] = [];
  const state: Env = { citeKey: '', citeNonce: 0, citeDone: 0, citeTries: 0, citeSmooth: false, citeMiss: 0 };
  const scope: Env = { min: Math.min, max: Math.max, viewport: { prefersReducedMotion: reduced }, data: { citeJump: { key: KEY, nonce: 1 } },
    scrollIntoView: (...args: unknown[]) => calls.push(args),
    frame: (id: string) => {
      const box = id === 'transcript' ? { y: 50, height: 600 } : frames[id];
      return box ? { x: 0, width: 600, ...box, unavailable: false } : { x: 0, y: 0, width: 0, height: 0, unavailable: true };
    } };
  root.call('citeBegin', state, scope, []);
  return { calls, state, place: () => root.call('citePlace', state, scope, []), scope };
}

describe('a citation\'s View source places its answer', () => {
  test('a row the list has not built is approached by its key, the lead above it', () => {
    const c = citation({});
    c.place();
    expect(c.calls).toEqual([['transcript', KEY, { offset: -120, behavior: 'instant' }]]);
    expect([c.state.citeTries, c.state.citeDone]).toEqual([1, 0]);
  });

  test('a built answer goes 120pt under the top (its 2pt padding inside), smooth the first time, then holds', () => {
    // The row at 400, its answer 30pt into it: the answer's text is 380pt under the transcript's top.
    const frames: Record<string, { y: number; height: number }> = { [`row-${KEY}`]: { y: 400, height: 200 }, [`cite-${KEY}`]: { y: 430, height: 160 } };
    const c = citation(frames);
    c.place();
    expect(c.calls).toEqual([['transcript', KEY, { offset: 30 + 2 - 120, behavior: 'smooth' }]]);
    expect(c.state.citeSmooth).toBe(true);
    // Rows above it measured as they mounted: still 10pt off, placed again at once.
    frames[`row-${KEY}`] = { y: 68 + 10, height: 200 };
    frames[`cite-${KEY}`] = { y: 98 + 10, height: 160 };
    c.state.citeSmooth = false;
    c.place();
    expect(c.calls.at(-1)).toEqual(['transcript', KEY, { offset: 30 + 2 - 120, behavior: 'instant' }]);
    // In place: done.
    frames[`cite-${KEY}`] = { y: 50 + 120 - 2, height: 160 };
    c.place();
    expect(c.state.citeDone).toBe(1);
  });

  test('a third of a short transcript is the lead; reduced motion places it at once', () => {
    const c = citation({ [`row-${KEY}`]: { y: 700, height: 100 }, [`cite-${KEY}`]: { y: 700, height: 100 } }, true);
    (c.scope.frame as unknown) = ((original: (id: string) => Env) => (id: string) => id === 'transcript' ? { y: 0, height: 300, unavailable: false } : original(id))(c.scope.frame as (id: string) => Env);
    c.place();
    expect(c.calls).toEqual([['transcript', KEY, { offset: 2 - 100, behavior: 'instant' }]]);
  });

  test('an answer the scroll cannot take further ends the placement; the reader\'s wheel ends it too', () => {
    const frames = { [`row-${KEY}`]: { y: 500, height: 100 }, [`cite-${KEY}`]: { y: 500, height: 100 } };
    const c = citation(frames, true);
    c.place();
    c.place(); // nothing moved: the same miss
    expect([c.calls.length, c.state.citeDone]).toEqual([1, 1]);
    const d = citation({});
    column.call('transcriptWheel', {}, { citeEnd: () => root.call('citeEnd', d.state, d.scope, []), restGesture: () => {} }, [{ deltaY: 4 }]);
    expect(d.state.citeDone).toBe(1);
  });
});
