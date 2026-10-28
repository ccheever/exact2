// The root's highlight pump (app.contract highlightStep and its `then` actions): it asks the module for the texts
// waiting for their tokens, has the data crate's lexers answer for them and stores their records, one round at a time,
// then refreshes `data` and `shell` when a text got its tokens, but never one whose answer is still in flight: a
// refresh replaces that answer. At launch, while the snapshot answer connected (seconds), a refresh every 16 ms kept it
// from ever landing to clear highlightPending; each replaced answer's native call stayed with the executor until the
// module replied, and a relaunch with a saved environment opened with no projects and "native executor admission limit
// reached". The actions run with the Contract action semantics of contract-action-fixture.ts.
import { afterEach, describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { highlightAsk, highlightPending, highlightPendingSince, highlightStore, highlightTokens, resetHighlights } from '../../src/highlight/highlights';
import { lineRecords, settleHighlights } from '../support/highlights-fixture';

type Answer = { name: string; highlightPending: boolean };

describe('the highlight pump runs one round at a time and leaves an answer in flight alone', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const scope = (extra: Record<string, unknown>, inFlight: string[]) => {
    const sent: unknown[][] = [], refreshed: string[] = [];
    return { sent, refreshed, scope: { ...extra,
      pending: (target: Answer | unknown) => typeof target === 'object' && target !== null && 'name' in target ? inFlight.includes((target as Answer).name) : inFlight.includes(String(target)),
      refresh: (name: string) => refreshed.push(name),
      highlightAsk: () => sent.push(['highlightAsk']), highlightCode: (requests: unknown) => sent.push(['highlightCode', requests]),
      highlightStore: (results: unknown) => sent.push(['highlightStore', results]) } };
  };
  const answers = (data: boolean, shell: boolean, palette = false) => ({ data: { name: 'data', highlightPending: data }, shell: { name: 'shell', highlightPending: shell },
    palette: { name: 'palette', highlightPending: palette }, prDetail: { name: 'prDetail', highlightPending: false }, settingsCore: { name: 'settingsCore', highlightPending: false } });

  test('a text that got its tokens refreshes each view that waited; a round with nothing new refreshes none', () => {
    const run = (stored: { finished: boolean; pending: boolean } | null, data: boolean, shell: boolean, inFlight: string[] = [], palette = false) => {
      const { refreshed, scope: s } = scope({ highlightStored: stored, ...answers(data, shell, palette) }, inFlight);
      root.call('highlightStoredThen', {}, s, []);
      return refreshed;
    };
    expect(run({ finished: true, pending: false }, true, true)).toEqual(['data', 'shell']);
    expect(run({ finished: true, pending: true }, true, false, [], true)).toEqual(['data', 'palette']);
    expect(run({ finished: false, pending: false }, true, true)).toEqual([]);
    expect(run({ finished: true, pending: false }, true, true, ['data'])).toEqual(['shell']);
    expect(run(null, true, true)).toEqual(['data']);
  });

  test('an ask with nothing waiting refreshes a view still reporting highlightPending, which painted before its tokens landed', () => {
    const { refreshed, scope: s } = scope({ highlightAsked: { requests: [], pending: false }, ...answers(false, true, true) }, ['palette']);
    root.call('highlightAskedThen', {}, s, []);
    expect(refreshed).toEqual(['shell']);
  });

  test('the ask goes to the lexers only with requests, and their records to the module', () => {
    const sends = (action: string, values: Record<string, unknown>) => {
      const { sent, scope: s } = scope(values, []);
      root.call(action, {}, s, []);
      return sent;
    };
    expect(sends('highlightAskedThen', { highlightAsked: { requests: [], pending: false }, ...answers(true, false) })).toEqual([]);
    expect(sends('highlightAskedThen', { highlightAsked: { requests: [{ id: 1, language: 72, text: 'let a' }], pending: false }, ...answers(true, false) }))
      .toEqual([['highlightCode', [{ id: 1, language: 72, text: 'let a' }]]]);
    expect(sends('highlightCodedThen', { highlightCoded: { results: [{ id: 1, records: [5, 19] }] }, ...answers(true, false) }))
      .toEqual([['highlightStore', [{ id: 1, records: [5, 19] }]]]);
    // One round at a time: no ask while a step of the round is in flight.
    expect(sends('highlightStep', { highlightAsked: null, highlightCoded: null, highlightStored: null, ...answers(true, false) })).toEqual([['highlightAsk']]);
  });
});

describe('an answer that painted a text without its tokens reports it', () => {
  afterEach(() => resetHighlights());

  test('after the pump stores that text during the answer', () => {
    const text = 'const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400);
    const answer = highlightPendingSince();
    expect(highlightTokens(text, 'a.ts')).toBeNull(); // the shell's panel asks; the text waits for the lexers
    expect(highlightPending()).toBe(true);
    settleHighlights(); // the root's rounds land while the answer awaits its last native calls
    expect(highlightPending()).toBe(false);
    expect(answer()).toBe(true);
    // The answer asked again gets the tokens and reports nothing.
    const again = highlightPendingSince();
    expect(highlightTokens(text, 'a.ts')).not.toBeNull();
    expect(again()).toBe(false);
  });

  test('a round whose lexers never answered asks for its texts again', () => {
    highlightTokens('let a = 1;', 'ts');
    const first = highlightAsk();
    expect(first.requests.map(request => request.text)).toEqual(['let a = 1;']);
    // The lexers' call failed: no store. The next round asks again, and its answer stores the tokens.
    const second = highlightAsk();
    expect(second.requests.map(request => request.text)).toEqual(['let a = 1;']);
    highlightStore(second.requests.map(request => ({ id: request.id, records: lineRecords(request.language, request.text) })));
    expect(highlightTokens('let a = 1;', 'ts')!.map(token => token.cls)).toContain('kw');
    expect(highlightPending()).toBe(false);
  });
});
