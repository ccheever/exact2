// The root's highlight pump (app.contract highlightStepped) refreshes `data` and `shell` when a background
// highlight slice finishes a text, or when the queue is empty and their landed answer still reports
// highlightPending, but never one whose answer is still in flight: a refresh replaces that answer. At launch, while
// the snapshot answer connected (seconds), a refresh every 16 ms kept it from ever landing to clear highlightPending;
// each replaced answer's native call stayed with the executor until the module replied, and a relaunch with a saved
// environment opened with no projects and "native executor admission limit reached".
// The actions run with the Contract action semantics of contract-action-fixture.ts.
import { afterEach, describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { highlightPending, highlightPendingSince, highlightSlice, resetHighlightSlicing, shikiTokens, startHighlightTurn } from '../../src/highlight/shiki-highlight';

type Answer = { name: string; highlightPending: boolean };

describe('the highlight pump leaves an answer in flight alone', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const step = (slice: { finished: boolean; pending: boolean } | null, data: Answer, shell: Answer, inFlight: string[]) => {
    const refreshed: string[] = [];
    root.call('highlightStepped', {}, {
      highlightSliced: slice, data, shell,
      pending: (answer: Answer) => inFlight.includes(answer.name),
      refresh: (name: string) => refreshed.push(name),
    }, []);
    return refreshed;
  };
  const answers = (data: boolean, shell: boolean): [Answer, Answer] =>
    [{ name: 'data', highlightPending: data }, { name: 'shell', highlightPending: shell }];

  test('a finished text refreshes both; an empty queue, what still reports highlightPending', () => {
    expect(step({ finished: true, pending: true }, ...answers(false, false), [])).toEqual(['data', 'shell']);
    expect(step({ finished: false, pending: false }, ...answers(true, true), [])).toEqual(['data', 'shell']);
    expect(step({ finished: false, pending: false }, ...answers(true, false), [])).toEqual(['data']);
    expect(step({ finished: false, pending: true }, ...answers(true, true), [])).toEqual([]);
    expect(step(null, ...answers(true, true), [])).toEqual(['data']);
  });

  test('an answer in flight is not replaced, however many slices run while it connects', () => {
    const refreshed: string[] = [];
    // A second of the pump's 16 ms turns while the launch's snapshot answer waits on its connection; the shell's
    // answer lands at once, and its next one reports nothing pending.
    let shellPending = true;
    for (let turn = 0; turn < 60; turn++) {
      const asked = step({ finished: turn === 3, pending: turn < 3 }, ...answers(true, shellPending), ['data']);
      if (asked.includes('shell')) shellPending = false;
      refreshed.push(...asked, ...step(null, ...answers(true, shellPending), ['data']));
    }
    expect(refreshed).toEqual(['shell']);
    expect(step({ finished: true, pending: false }, ...answers(true, true), ['shell'])).toEqual(['data']);
    // Landed, still reporting highlightPending: the next turn asks once more, and that answer clears it.
    expect(step({ finished: false, pending: false }, ...answers(true, false), [])).toEqual(['data']);
  });
});

describe('an answer that painted a text without its tokens reports it', () => {
  afterEach(() => resetHighlightSlicing());

  test('after a slice finishes that text during the answer', () => {
    resetHighlightSlicing();
    const text = 'const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400);
    startHighlightTurn();
    const answer = highlightPendingSince();
    expect(shikiTokens(text, 'a.ts')).toBeNull(); // the shell's panel asks; the turn's budget defers the rest
    // Background slices finish the job while the answer awaits its last native calls.
    for (let slices = 0; highlightPending() && slices < 500; slices++) highlightSlice();
    expect(highlightPending()).toBe(false);
    expect(answer()).toBe(true);
    // The answer asked again gets the tokens and reports nothing.
    startHighlightTurn();
    const again = highlightPendingSince();
    expect(shikiTokens(text, 'a.ts')).not.toBeNull();
    expect(again()).toBe(false);
  });
});
