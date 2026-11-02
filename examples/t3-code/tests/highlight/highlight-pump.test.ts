// The root's highlight pump (app.contract highlightStep and its `then` actions): it asks the module for the texts
// waiting for their tokens, has the data crate's lexers answer for them and stores their records, one round at a time,
// then refreshes each view that painted a text without them, but never one whose answer is still in flight: a refresh
// replaces that answer. At launch, while the snapshot answer connected (seconds), a refresh every 16 ms kept it from
// ever landing to clear highlightPending; each replaced answer's native call stayed with the executor until the module
// replied, and a relaunch with a saved environment opened with no projects and "native executor admission limit
// reached". The actions run with the Contract action semantics of contract-action-fixture.ts.
import { afterEach, describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import { highlightAsk, highlightPending, highlightStore, highlightTokens, highlightView, resetHighlights, tokensOf, type HighlightView } from '../../src/highlight/highlights';
import { highlightCode, highlightRound, lexed, lineRecords, resetLexers, settleHighlights } from '../support/highlights-fixture';

type Answer = { name: string; highlightPending: boolean };

describe('the highlight pump runs one round at a time and leaves an answer in flight alone', () => {
  const root = contractComponent('app.contract', 'T3Code');
  /** The scope of a pump action: `inFlight` names the resources and the round's steps whose answers are pending. */
  const scope = (extra: Record<string, unknown>, inFlight: string[]) => {
    const sent: unknown[][] = [], refreshed: string[] = [];
    const steps = Object.fromEntries(['highlightAsked', 'highlightCoded', 'highlightStored'].map(name => [name, { step: name }]));
    return { sent, refreshed, scope: { ...steps, ...extra,
      pending: (target: unknown) => {
        const named = target as Partial<Answer & { step: string }> | null;
        return inFlight.includes(String(named?.name ?? named?.step ?? target));
      },
      refresh: (name: string) => refreshed.push(name),
      highlightAsk: () => sent.push(['highlightAsk']), highlightCode: (requests: unknown) => sent.push(['highlightCode', requests]),
      highlightStore: (results: unknown) => sent.push(['highlightStore', results]) } };
  };
  const answers = (data: boolean, shell: boolean, palette = false) => ({ data: { name: 'data', highlightPending: data }, shell: { name: 'shell', highlightPending: shell },
    palette: { name: 'palette', highlightPending: palette }, prDetail: { name: 'prDetail', highlightPending: false }, settingsCore: { name: 'settingsCore', highlightPending: false }, sideChat: { name: 'sideChat', highlightPending: false }, sideChatThread: '' });

  test('a round starts only when none of its three steps is in flight', () => {
    const step = (inFlight: string[]) => {
      const { sent, scope: s } = scope(answers(true, false), inFlight);
      root.call('highlightStep', {}, s, []);
      return sent;
    };
    expect(step([])).toEqual([['highlightAsk']]);
    for (const inFlight of [['highlightAsked'], ['highlightCoded'], ['highlightStored']]) expect(step(inFlight)).toEqual([]);
  });

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
    const request = { id: 1, language: 72, doc: 0, from: 0, text: 'let a' };
    expect(sends('highlightAskedThen', { highlightAsked: { requests: [], pending: false }, ...answers(true, false) })).toEqual([]);
    expect(sends('highlightAskedThen', { highlightAsked: { requests: [request], pending: false }, ...answers(true, false) })).toEqual([['highlightCode', [request]]]);
    expect(sends('highlightCodedThen', { highlightCoded: { results: [{ id: 1, line: 0, records: [5, 19] }] }, ...answers(true, false) }))
      .toEqual([['highlightStore', [{ id: 1, line: 0, records: [5, 19] }]]]);
  });
});

describe('an answer that painted a text without its tokens reports it', () => {
  afterEach(() => resetHighlights());

  test('after the pump stores that text during the answer', async () => {
    const text = 'const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400);
    const answer = await highlightView('shell', async () => {
      expect(highlightTokens(text, 'a.ts', 0, 'shell')).toBeNull(); // the shell's panel asks; the text waits for the lexers
      expect(highlightPending()).toBe(true);
      settleHighlights(); // the root's rounds land while the answer awaits its last native calls
      expect(highlightPending()).toBe(false);
      return {};
    });
    expect(answer.highlightPending).toBe(true);
    // The answer asked again gets the tokens and reports nothing.
    const again = await highlightView('shell', () => { expect(highlightTokens(text, 'a.ts', 0, 'shell')).not.toBeNull(); return {}; });
    expect(again.highlightPending).toBe(false);
  });

  test('a round whose lexers never answered asks for its texts again', () => {
    highlightTokens('let a = 1;', 'ts');
    const first = highlightAsk();
    expect(first.requests.map(request => request.text)).toEqual(['let a = 1;']);
    // The lexers' call failed: no store. The next round asks again, and its answer stores the tokens.
    const second = highlightAsk();
    expect(second.requests.map(request => request.text)).toEqual(['let a = 1;']);
    highlightStore(highlightCode(second.requests));
    expect(highlightTokens('let a = 1;', 'ts')!.map(token => token.cls)).toContain('kw');
    expect(highlightPending()).toBe(false);
  });
});

describe('what the views paint settles, and nothing starves', () => {
  afterEach(() => { resetHighlights(); resetLexers(); });
  /** A view's answer painting these texts, and whether it painted each with its tokens. */
  const paint = (view: HighlightView, texts: readonly string[], language = 'ts') => highlightView(view, () => ({ colored: texts.map(text => highlightTokens(text, language, 0, view)?.some(token => token.cls) === true) }));

  test('a diff of more than 400 texts settles: none of them is dropped while the view paints them', async () => {
    // Both sides of 300 hunks, about 90 of them a round.
    const texts = Array.from({ length: 600 }, (_, i) => `export const value${i} = compute(${i}, "side ${i % 2}");`);
    let answer = await paint('data', texts), rounds = 0;
    while (answer.highlightPending) {
      if (++rounds > 50) throw new Error('the diff never settled');
      if (highlightRound(2_000_000).finished) answer = await paint('data', texts); // the pump refreshes the view
    }
    expect(rounds).toBeGreaterThan(5);
    expect(answer.colored.every(Boolean)).toBe(true);
    // Its next answer, and one after more rounds of other texts, paint every text without asking again.
    for (let other = 0; other < 500; other++) highlightTokens(`let other${other} = ${other};`, 'ts');
    settleHighlights();
    expect((await paint('data', texts)).highlightPending).toBe(false);
  });

  test('the lexed texts no view paints are kept up to 400, the most recently used', () => {
    const texts = Array.from({ length: 450 }, (_, i) => `let kept${i} = ${i};`);
    for (const text of texts) highlightTokens(text, 'ts');
    settleHighlights();
    // The 400 most recently used stay; the first 50 lexed go.
    expect(texts.map(text => highlightTokens(text, 'ts') !== null && highlightTokens(text, 'ts')!.some(token => token.cls)).filter(Boolean).length).toBe(400);
  });

  test('a long text in Files gets its tokens while a reply streams a block, which keeps getting its own', async () => {
    // Several calls' worth: each call gives the file, which waited first, half its budget (highlights.rs answer;
    // its own test holds the Rust to that), and the block the rest.
    const file = 'export function step(index: number): number { return index * 2 + offset; } // a long file\n'.repeat(3_500);
    expect(file.length).toBeGreaterThan(256 * 1024);
    const reply = Array.from({ length: 200 }, (_, i) => `const line${i} = ${i};`).join('\n');
    let shell = await paint('shell', [file]), streamed = 0, rounds = 0, behind = 0;
    while (shell.highlightPending) {
      if (++rounds > 40) throw new Error(`the file waited ${rounds} rounds`);
      streamed = Math.min(reply.length, streamed + 120); // the reply grows every round
      const block = reply.slice(0, streamed);
      await paint('data', [block]);
      if (highlightRound().finished) shell = await paint('shell', [file]);
      // The version the round took has its tokens, while the file still waits for its own.
      if ((await paint('data', [block])).highlightPending) behind++;
    }
    expect(rounds).toBeGreaterThan(5);
    expect(behind).toBe(0);
    expect(shell.colored).toEqual([true]);
  });

  test('a streaming block lexes the line it continues and its new lines, and paints what the whole text would', () => {
    const reply = Array.from({ length: 400 }, (_, i) => `const line${i} = compute(${i}); // ${'x'.repeat(i % 40)}`).join('\n');
    lexed.bytes = 0;
    for (let end = 50; end <= reply.length; end += 50) {
      highlightTokens(reply.slice(0, end), 'ts');
      settleHighlights();
    }
    const tokens = highlightTokens(reply.slice(0, reply.length - reply.length % 50), 'ts')!;
    const whole = reply.slice(0, reply.length - reply.length % 50);
    expect(tokens).toEqual(tokensOf(whole, lineRecords(72, whole)));
    // Lexed again whole, each of its 400 versions would cost half the block on average: 200 blocks.
    expect(lexed.bytes).toBeLessThan(3 * reply.length);
  });

  test('a document whose answer was lost starts again from its first line', () => {
    const block = 'const a = 1;\nconst b = 2;\n';
    highlightTokens(block, 'ts');
    settleHighlights();
    highlightTokens(`${block}const c`, 'ts');
    const lost = highlightAsk(); // its document took the text, but the answer never stored
    expect(lost.requests.map(request => [request.from, request.text])).toEqual([[0, `${block}const c`]]);
    highlightCode(lost.requests);
    const again = highlightAsk();
    expect(again.requests.map(request => [request.doc, request.from])).toEqual([[lost.requests[0]!.doc, 0]]);
    highlightStore(highlightCode(again.requests));
    expect(highlightTokens(`${block}const c`, 'ts')!.map(token => token.text).join('')).toBe(`${block}const c`);
  });
});
