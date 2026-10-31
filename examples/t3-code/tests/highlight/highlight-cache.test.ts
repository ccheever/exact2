// The highlight cache (highlights.ts) with the lexers as the data crate runs them (highlights-fixture.ts: the module's
// live tokenizer, highlights.rs's budget and its eight documents): which texts go to a document and how, what each
// view's answer pins and reports, and what the cache keeps.
import { AsyncLocalStorage } from 'node:async_hooks';
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { highlightAsk, highlightPending, highlightStore, highlightTokens, highlightView, resetHighlights, textBytes, tokensOf, WHOLE_BYTES, type HighlightView } from '../../src/highlight/highlights';
import { messageBlocksCached, messageCodeBlocks, resetMessageBlocks } from '../../src/highlight/timeline-highlight';
import { highlightCode, highlightRound, lexed, lineRecords, resetLexers, settleHighlights } from '../support/highlights-fixture';

afterEach(() => { resetHighlights(); resetMessageBlocks(); resetLexers(); });
const whole = (text: string) => tokensOf(text, lineRecords(72, text));
/** Whether `text` has its tokens now (a text not yet lexed, or one that extends nothing lexed, paints plain), painted
 *  for `view`. */
const lexedNow = (text: string, view: HighlightView | null = null) => highlightTokens(text, 'ts', 0, view)?.some(token => token.cls) === true;

describe('documents', () => {
  test('nine long texts at once each get their tokens, and each document starts once', async () => {
    // highlights.rs keeps eight documents. A ninth dropped the oldest while it still lexed, which started again and
    // dropped the next: none finished.
    const line = 'export function step(index: number): number { return index * 2 + offset; } // a long file\n';
    const texts = Array.from({ length: 9 }, (_, i) => `// file ${i}\n${line.repeat(450)}`);
    expect(texts.every(text => text.length > WHOLE_BYTES)).toBe(true);
    const paint = () => highlightView('shell', () => ({ waiting: texts.filter(text => !lexedNow(text, 'shell')).length }));
    let answer = await paint(), rounds = 0, starts = 0;
    while (answer.highlightPending) {
      if (++rounds > 200) throw new Error(`${answer.waiting} texts still waited after 200 rounds`);
      const round = highlightRound();
      starts += round.requests.filter(request => request.doc && request.from === 0).length;
      if (round.finished) answer = await paint();
    }
    expect(answer.waiting).toBe(0);
    expect(starts).toBe(9);
  });

  /** Nine long texts that grow, text `i` by a line every `every(i)` rounds, painted each round by one view and lexed
   *  a round at a time: each text's longest run of rounds painting a version without its tokens, the documents
   *  started again, and the bytes lexed. */
  async function growing(rounds: number, every: (text: number) => number) {
    const line = (i: number, n: number) => `export function step${n}(index: number): number { return index * ${i} + offset; } // text ${i}\n`;
    const texts = Array.from({ length: 9 }, (_, i) => Array.from({ length: 400 }, (_, n) => line(i, n)).join(''));
    expect(texts.every(text => text.length > WHOLE_BYTES)).toBe(true);
    const behind = texts.map(() => 0), worst = texts.map(() => 0);
    let starts = 0;
    lexed.bytes = 0;
    for (let round = 0; round < rounds; round++) {
      texts.forEach((text, i) => { if (round % every(i) === 0) texts[i] = text + line(i, 400 + round); });
      await highlightView('shell', () => { texts.forEach((text, i) => { behind[i] = lexedNow(text, 'shell') ? 0 : behind[i]! + 1; worst[i] = Math.max(worst[i]!, behind[i]!); }); return {}; });
      starts += highlightRound().requests.filter(request => request.doc && request.from === 0).length;
    }
    const length = texts.reduce((sum, text) => sum + text.length, 0);
    return { worst: Math.max(...worst), restarts: starts - texts.length, lexedPerText: lexed.bytes / length };
  }

  test('nine long texts that grow every round each get a document in turn', async () => {
    // The ninth never got one: the eight with documents always had work in flight.
    const { worst, restarts } = await growing(200, () => 1);
    expect(worst).toBeLessThanOrEqual(20);
    expect(restarts).toBeLessThanOrEqual(200 / 16 + 9);
  });

  test('nine long texts that grow every few rounds keep their documents while they grow', async () => {
    // Documents dropped as soon as their texts paused started them again every few rounds: 128 to 206 restarts.
    const { worst, restarts, lexedPerText } = await growing(300, i => 2 + (i % 4));
    expect(worst).toBeLessThanOrEqual(20);
    expect(restarts).toBeLessThanOrEqual(300 / 16 + 9);
    expect(lexedPerText).toBeLessThan(4);
  });

  test('a block with CRLF line ends, streamed a character at a time, paints what its whole text does', () => {
    // incrementalHighlighting.test.ts's CRLF sample. An append that splits a CRLF, or a version that ends in a CR,
    // lexes otherwise than the whole text, so such a text is never appended to (the reference lexes it whole).
    const code = '/* multi\r\nline */\r\nconst x = 1;\r\n';
    for (let end = 1; end <= code.length; end++) {
      const version = code.slice(0, end);
      highlightTokens(version, 'ts');
      settleHighlights();
      expect(highlightTokens(version, 'ts')).toEqual(whole(version));
    }
  });

  test('a long text with CRLF line ends starts its document again for each version', () => {
    let text = 'const value = compute(input, { retries: 3 });\r\n'.repeat(800);
    expect(text.length).toBeGreaterThan(WHOLE_BYTES);
    for (const version of [text, text += 'const tail', text += ' = 1;\r', text += '\nlet more = 2;']) {
      highlightTokens(version, 'ts');
      const sent: { from: number; text: string }[] = [];
      while (highlightPending()) sent.push(...highlightRound().requests);
      expect(sent[0]).toMatchObject({ from: 0, text: version });
      expect(highlightTokens(version, 'ts')).toEqual(whole(version));
    }
  });

  test('a long text\'s lone CRs stay in the tokens before them, as the whole text has them', () => {
    // The live tokenizer ends a line at a lone CR; the whole text's records (mode 3) keep the CR in the token before.
    const lines = Array.from({ length: 900 }, (_, i) => `const value${i} = compute(input, "${i}");${i % 7 ? '\n' : '\r'}`);
    for (const text of [lines.join(''), `${lines.join('')}// a comment\r`, `${lines.join('')}let tail = "a\rb";\r`]) {
      expect(text.length).toBeGreaterThan(WHOLE_BYTES);
      highlightTokens(text, 'ts');
      settleHighlights();
      expect(highlightTokens(text, 'ts')).toEqual(whole(text));
    }
  });

  test('a reply streaming a code block colours it as it grows, each version appended to its document', () => {
    const code = Array.from({ length: 120 }, (_, i) => `export function stream${i}(value: number): number { return value * ${i} + base; } // ${i}`).join('\n');
    lexed.bytes = 0;
    for (let end = 40; end <= code.length; end += 40) {
      // The fence is still open: the block runs to the end of the reply so far.
      const reply = `Here is the block.\n\n\`\`\`ts\n${code.slice(0, end)}`;
      expect(messageCodeBlocks(reply, null, 'reply').map(block => block.code)).toEqual([code.slice(0, end).replace(/\n+$/, '')]);
      settleHighlights();
      const [block] = messageCodeBlocks(reply, null, 'reply');
      expect(block!.tokens.map(token => [token.text, token.cls])).toEqual(whole(block!.code).map(token => [token.text, token.cls]));
    }
    // Lexed again whole, each of its 285 versions would cost half the block on average: about 140 blocks.
    expect(lexed.bytes).toBeLessThan(4 * code.length);
  });

  test('a block that grows faster than the rounds lex has its newest version\'s tokens each round', async () => {
    const code = Array.from({ length: 200 }, (_, i) => `const line${i} = compute(${i});`).join('\n');
    const paint = (version: string) => highlightView('data', () => { highlightTokens(version, 'ts', 0, 'data'); return {}; });
    let end = 0, behind = 0;
    while (end < code.length) {
      // Three versions a round: the older two are no view's by the round, and its document takes the newest.
      for (let version = 0; version < 3; version++) await paint(code.slice(0, end = Math.min(code.length, end + 30)));
      highlightRound();
      if ((await paint(code.slice(0, end))).highlightPending) behind++;
    }
    expect(behind).toBe(0);
    expect(highlightTokens(code, 'ts')).toEqual(whole(code));
  });
});

// Each call that paints names its view, so what an answer pins and reports does not depend on the runtime's answer
// context, whose two shapes differ: Hermes's `current()` gives an answer's call record, the web's a new object each
// time (js/src/prelude.js `__exact_async_context`, host/web-js/ts-data.js). Each test runs with neither, and with each.
const answers = new AsyncLocalStorage<object>();
const shapes: [string, (() => unknown) | null][] = [
  ['no runtime context', null],
  ['a Hermes-shaped context', () => answers.getStore() ?? null],
  ['a web-shaped context', () => ({ ...answers.getStore() })],
];
for (const [shape, current] of shapes) describe(`a view pins and reports only what it paints, with ${shape}`, () => {
  beforeEach(() => {
    const scope = globalThis as { __exact_async_context?: unknown };
    if (current) scope.__exact_async_context = { current, swap: () => null };
    else delete scope.__exact_async_context;
  });
  afterEach(() => { delete (globalThis as { __exact_async_context?: unknown }).__exact_async_context; });
  /** A runtime answer: `run` under an answer of its own. */
  const answer = <T>(run: () => T): T => answers.run({}, run);
  const gate = () => { let open!: () => void; const shut = new Promise<void>(resolve => { open = resolve; }); return { shut, open }; };

  test('a text another view paints while an answer is in flight is neither pinned nor reported by it', async () => {
    highlightTokens('let shellText = 1;', 'ts');
    settleHighlights();
    const wait = gate();
    const shell = answer(() => highlightView('shell', async () => { highlightTokens('let shellText = 1;', 'ts', 0, 'shell'); await wait.shut; return {}; }));
    const data = await answer(() => highlightView('data', () => { highlightTokens('let dataOnly = 2;', 'ts', 0, 'data'); return {}; }));
    wait.open();
    expect(data.highlightPending).toBe(true);
    expect((await shell).highlightPending).toBe(false);
    settleHighlights();
    // The data view stops painting its text: no view pins it, and 500 other texts push it out.
    await answer(() => highlightView('data', () => ({})));
    for (let i = 0; i < 500; i++) highlightTokens(`let other${i} = ${i};`, 'ts');
    settleHighlights();
    expect(lexedNow('let dataOnly = 2;')).toBe(false);
    expect(lexedNow('let shellText = 1;')).toBe(true);
  });

  test('a text an answer paints after an await is its own, whatever other answer is in flight', async () => {
    highlightTokens('let dataText = 1;', 'ts');
    settleHighlights();
    const shellWait = gate(), dataWait = gate();
    const shell = answer(() => highlightView('shell', async () => { await shellWait.shut; highlightTokens('let shellLate = 2;', 'ts', 0, 'shell'); return {}; }));
    const data = answer(() => highlightView('data', async () => { highlightTokens('let dataText = 1;', 'ts', 0, 'data'); await dataWait.shut; return {}; }));
    shellWait.open();
    expect((await shell).highlightPending).toBe(true);
    dataWait.open();
    expect((await data).highlightPending).toBe(false);
    // The shell's late text stays: its answer pins it through 500 others.
    settleHighlights();
    for (let i = 0; i < 500; i++) highlightTokens(`let other${i} = ${i};`, 'ts');
    settleHighlights();
    expect(lexedNow('let shellLate = 2;')).toBe(true);
  });

  test('the texts an answer in flight has painted stay while the pump trims the cache', async () => {
    const texts = Array.from({ length: 450 }, (_, i) => `let painted${i} = ${i};`);
    for (const text of texts) highlightTokens(text, 'ts');
    settleHighlights();
    const wait = gate();
    const data = answer(() => highlightView('data', async () => {
      texts.forEach(text => lexedNow(text, 'data'));
      await wait.shut;
      return { lexed: texts.filter(text => lexedNow(text, 'data')).length };
    }));
    // The pump's rounds lex 450 other texts, past KEPT, while the answer waits.
    for (let i = 0; i < 450; i++) highlightTokens(`let other${i} = ${i};`, 'ts');
    settleHighlights();
    wait.open();
    expect((await data).lexed).toBe(450);
  });

  test('a message whose blocks come from the cache of painted results keeps their texts pinned', async () => {
    const message = 'Intro\n\n```ts\nconst pinned = 1;\n```\n';
    messageCodeBlocks(message, null, 'message');
    settleHighlights();
    for (let round = 0; round < 3; round++) {
      // Each answer paints the cached blocks again (highlightShown), and 450 other texts are lexed between them.
      await answer(() => highlightView('data', () => ({ code: messageCodeBlocks(message, 'data', 'message') })));
      for (let i = 0; i < 450; i++) highlightTokens(`let other${round}x${i} = ${i};`, 'ts');
      settleHighlights();
    }
    expect(lexedNow('const pinned = 1;')).toBe(true);
  });
});

describe('what the cache keeps', () => {
  test('the lexed texts no view paints are kept while their tokens take up to 8 MiB, the most recently used', () => {
    // 40 texts of 26,814 characters and 6,402 tokens each: about 0.8 MB apiece in Hermes's heap, under 400 texts.
    const texts = Array.from({ length: 40 }, (_, i) => `// text ${String(i).padStart(3, '0')}\n${'export const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400)}`);
    for (const text of texts) {
      highlightTokens(text, 'ts');
      settleHighlights();
    }
    const kept = Math.floor((8 << 20) / textBytes(texts[0]!, whole(texts[0]!).length));
    expect(kept).toBeLessThan(texts.length);
    expect(texts.map(text => lexedNow(text))).toEqual(texts.map((_, i) => i >= texts.length - kept));
  });

  test('the cache of message blocks keeps one version a message, while their tokens take up to 4 MiB', () => {
    const block = (i: number, lines: number) => `\`\`\`ts\n${Array.from({ length: lines }, (_, n) => `const value${i}x${n} = compute(${n});`).join('\n')}\n\`\`\``;
    // A reply streaming its block: each version replaces the last.
    for (let lines = 10; lines <= 200; lines += 10) {
      messageCodeBlocks(block(0, lines), null, 'reply');
      settleHighlights();
      messageCodeBlocks(block(0, lines), null, 'reply');
    }
    expect(messageBlocksCached().entries).toBe(1);
    // Many messages: the least recently used go past 4 MiB.
    for (let i = 1; i <= 60; i++) {
      messageCodeBlocks(block(i, 300), null, `message ${i}`);
      settleHighlights();
      messageCodeBlocks(block(i, 300), null, `message ${i}`);
    }
    const { entries, bytes } = messageBlocksCached();
    expect(entries).toBeLessThan(61);
    expect(bytes).toBeLessThanOrEqual(4 << 20);
  });

  test('a text the lexers left out goes before the texts painted after it', () => {
    const text = (i: number) => `const value${i} = compute(input);\n`.repeat(200);
    for (const i of [0, 1, 2]) highlightTokens(text(i), 'ts');
    const { requests } = highlightAsk();
    highlightTokens(text(3), 'ts'); // painted while the round is in flight
    // A budget that fits one: the other two are left out and wait again, before the text painted after them.
    highlightStore(highlightCode(requests, 4_000_000));
    expect(highlightAsk().requests.map(request => request.text)).toEqual([text(1), text(2), text(3)]);
  });
});
