// The highlight cache (highlights.ts) with the lexers as the data crate runs them (highlights-fixture.ts: the module's
// live tokenizer, highlights.rs's budget and its eight documents): which texts go to a document and how, what each
// view's answer pins and reports, and what the cache keeps.
import { AsyncLocalStorage } from 'node:async_hooks';
import { afterAll, afterEach, beforeAll, describe, expect, test } from 'bun:test';
import { highlightAsk, highlightPending, highlightStore, highlightTokens, highlightView, resetHighlights, tokensOf, WHOLE_BYTES } from '../../src/highlight/highlights';
import { messageCodeBlocks } from '../../src/highlight/timeline-highlight';
import { highlightCode, highlightRound, lexed, lineRecords, resetLexers, settleHighlights } from '../support/highlights-fixture';

afterEach(() => { resetHighlights(); resetLexers(); });
const whole = (text: string) => tokensOf(text, lineRecords(72, text));
/** Whether `text` has its tokens now (a text not yet lexed, or one that extends nothing lexed, paints plain). */
const lexedNow = (text: string) => highlightTokens(text, 'ts')?.some(token => token.cls) === true;

describe('documents', () => {
  test('nine long texts at once each get their tokens, and each document starts once', async () => {
    // highlights.rs keeps eight documents. A ninth dropped the oldest while it still lexed, which started again and
    // dropped the next: none finished.
    const line = 'export function step(index: number): number { return index * 2 + offset; } // a long file\n';
    const texts = Array.from({ length: 9 }, (_, i) => `// file ${i}\n${line.repeat(450)}`);
    expect(texts.every(text => text.length > WHOLE_BYTES)).toBe(true);
    const paint = () => highlightView('shell', () => ({ waiting: texts.filter(text => !lexedNow(text)).length }));
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
    // Each character's class, a CR's aside: the document ends a line at a lone CR, the CR its own plain token, where
    // the whole text's records keep it in the token before. A CR draws nothing either way.
    const drawn = (tokens: readonly { text: string; cls: string }[]) => tokens.flatMap(token => [...token.text].map(c => (c === '\r' ? c : c + token.cls)));
    let text = 'const value = compute(input, { retries: 3 });\r\n'.repeat(800);
    expect(text.length).toBeGreaterThan(WHOLE_BYTES);
    for (const version of [text, text += 'const tail', text += ' = 1;\r', text += '\nlet more = 2;']) {
      highlightTokens(version, 'ts');
      const sent: { from: number; text: string }[] = [];
      while (highlightPending()) sent.push(...highlightRound().requests);
      expect(sent[0]).toMatchObject({ from: 0, text: version });
      expect(drawn(highlightTokens(version, 'ts')!)).toEqual(drawn(whole(version)));
    }
  });

  test('a reply streaming a code block colours it as it grows, each version appended to its document', () => {
    const code = Array.from({ length: 120 }, (_, i) => `export function stream${i}(value: number): number { return value * ${i} + base; } // ${i}`).join('\n');
    lexed.bytes = 0;
    for (let end = 40; end <= code.length; end += 40) {
      // The fence is still open: the block runs to the end of the reply so far.
      const reply = `Here is the block.\n\n\`\`\`ts\n${code.slice(0, end)}`;
      expect(messageCodeBlocks(reply).map(block => block.code)).toEqual([code.slice(0, end).replace(/\n+$/, '')]);
      settleHighlights();
      const [block] = messageCodeBlocks(reply);
      expect(block!.tokens.map(token => [token.text, token.cls])).toEqual(whole(block!.code).map(token => [token.text, token.cls]));
    }
    // Lexed again whole, each of its 285 versions would cost half the block on average: about 140 blocks.
    expect(lexed.bytes).toBeLessThan(4 * code.length);
  });

  test('a block that grows faster than the rounds lex has its newest version\'s tokens each round', async () => {
    const code = Array.from({ length: 200 }, (_, i) => `const line${i} = compute(${i});`).join('\n');
    const paint = (version: string) => highlightView('data', () => { highlightTokens(version, 'ts'); return {}; });
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

describe('a view pins and reports only what it paints', () => {
  // The runtime's answer context (js/src/prelude.js `__exact_async_context`) as the bundle's async helper keeps it:
  // an async function resumes under the answer it began under (LLP 1133.002 D14).
  const answers = new AsyncLocalStorage<object>();
  beforeAll(() => { (globalThis as { __exact_async_context?: unknown }).__exact_async_context = { current: () => answers.getStore() ?? null }; });
  afterAll(() => { delete (globalThis as { __exact_async_context?: unknown }).__exact_async_context; });
  /** A runtime answer: `run` under an answer of its own. */
  const answer = <T>(run: () => T): T => answers.run({}, run);
  const gate = () => { let open!: () => void; const shut = new Promise<void>(resolve => { open = resolve; }); return { shut, open }; };

  test('a text another view paints while an answer is in flight is neither pinned nor reported by it', async () => {
    highlightTokens('let shellText = 1;', 'ts');
    settleHighlights();
    const wait = gate();
    const shell = answer(() => highlightView('shell', async () => { highlightTokens('let shellText = 1;', 'ts'); await wait.shut; return {}; }));
    const data = await answer(() => highlightView('data', () => { highlightTokens('let dataOnly = 2;', 'ts'); return {}; }));
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
    const shell = answer(() => highlightView('shell', async () => { await shellWait.shut; highlightTokens('let shellLate = 2;', 'ts'); return {}; }));
    const data = answer(() => highlightView('data', async () => { highlightTokens('let dataText = 1;', 'ts'); await dataWait.shut; return {}; }));
    shellWait.open();
    await shell.then(value => expect(value.highlightPending).toBe(true));
    dataWait.open();
    expect((await data).highlightPending).toBe(false);
  });

  test('the texts an answer in flight has painted stay while the pump trims the cache', async () => {
    const texts = Array.from({ length: 450 }, (_, i) => `let painted${i} = ${i};`);
    for (const text of texts) highlightTokens(text, 'ts');
    settleHighlights();
    const wait = gate();
    const data = answer(() => highlightView('data', async () => {
      texts.forEach(lexedNow);
      await wait.shut;
      return { lexed: texts.filter(lexedNow).length };
    }));
    // The pump's rounds lex 450 other texts, past KEPT, while the answer waits.
    for (let i = 0; i < 450; i++) highlightTokens(`let other${i} = ${i};`, 'ts');
    settleHighlights();
    wait.open();
    expect((await data).lexed).toBe(450);
  });

  test('a message whose blocks come from the cache of painted results keeps their texts pinned', async () => {
    const message = 'Intro\n\n```ts\nconst pinned = 1;\n```\n';
    messageCodeBlocks(message);
    settleHighlights();
    for (let round = 0; round < 3; round++) {
      // Each answer paints the cached blocks again (highlightShown), and 450 other texts are lexed between them.
      await answer(() => highlightView('data', () => ({ code: messageCodeBlocks(message) })));
      for (let i = 0; i < 450; i++) highlightTokens(`let other${round}x${i} = ${i};`, 'ts');
      settleHighlights();
    }
    expect(lexedNow('const pinned = 1;')).toBe(true);
  });
});

describe('what the cache keeps', () => {
  test('the lexed texts no view paints are kept up to 4,000,000 characters, the most recently used', () => {
    // 160 texts of 26,814 characters: 4.3 million, under 400 texts.
    const texts = Array.from({ length: 160 }, (_, i) => `// text ${String(i).padStart(3, '0')}\n${'export const value = compute(input, { retries: 3, label: "x" });\n'.repeat(400)}`);
    for (const text of texts) {
      highlightTokens(text, 'ts');
      settleHighlights();
    }
    const kept = Math.floor(4_000_000 / texts[0]!.length);
    expect(texts.map(lexedNow)).toEqual(texts.map((_, i) => i >= texts.length - kept));
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
