// The plain-text fast path (`isPlain`, `parse`): every string it admits must parse exactly as the parser parses it.
// The corpus is every fixture message and embed text, then generated strings that mix Markdown's characters and
// Discord's line-start syntax into plain text at random, at line starts and inside it.
import { expect, test } from 'bun:test';
import { createTranscriptWorld } from '../fixture/transcript-world';
import { isPlain, jumbo, parse, parseMarkdown } from './markdown';

const same = (s: string, options?: { headings?: boolean }) => expect(parse(s, options)).toEqual(parseMarkdown(s, options));

test('every fixture message and embed text parses the same through the fast path', () => {
  const world = createTranscriptWorld('http://fixture') as unknown as { messages: Map<string, unknown> | Record<string, unknown> };
  const groups = world.messages instanceof Map ? [...world.messages.values()] : Object.values(world.messages);
  const texts: string[] = [];
  for (const group of groups) {
    const messages = Array.isArray(group) ? group : group instanceof Map ? [...group.values()] : Object.values(group as object);
    for (const m of messages as Record<string, any>[]) {
      texts.push(String(m.content ?? ''));
      for (const e of m.embeds ?? []) { texts.push(String(e.description ?? '')); for (const f of e.fields ?? []) texts.push(String(f.value ?? '')); }
    }
  }
  expect(texts.length).toBeGreaterThan(10_000);
  let admitted = 0;
  for (const s of texts) { if (isPlain(s)) admitted++; same(s); same(s, { headings: false }); }
  expect(admitted / texts.length).toBeGreaterThan(0.6);
});

// Line-start syntax and inline constructs: none is admitted, and each still parses as the parser does.
const LINE_STARTS = ['- ', '-', '* ', '+ ', '+', '1. ', '1.', '12) ', '3)', '0. ', '-# ', '># ', '> ', '>>> ', '# ', '## ', '### ', '#', '  - ', '\t+ ', ' 1. '];
const CONSTRUCTS = ['||spoiler||', '[label](https://example.com)', '[x](<https://a.b>)', ':smile:', ':a:', '<@123>', '<@!5>', '<#9>', '<@&7>',
  '<:wave:123>', '<a:dance:456>', '<t:1700000000:R>', '@everyone', '@here', 'https://example.com', 'http://a.b', '<https://x.y>',
  '`code`', '``a`b``', '```js\nx\n```', '**bold**', '*it*', '_it_', '__u__', '~~s~~', '\\*', '\\_', 'a  \nb', 'a\n\nb', 'mailto:x@y'];

test('line-start syntax, lists and constructs are never admitted, and parse the same', () => {
  for (const start of LINE_STARTS) for (const rest of ['item', 'two words', '😀', '']) {
    const s = start + rest;
    if (s.trim()) expect(isPlain(s), JSON.stringify(s)).toBe(false);
    same(s);
  }
  for (const c of CONSTRUCTS) for (const s of [c, `before ${c}`, `${c} after`, `a ${c} b`]) {
    expect(isPlain(s), JSON.stringify(s)).toBe(false);
    same(s);
  }
});

test('emoji-only lines stay jumbo; blank, whitespace and very long strings parse the same', () => {
  for (const s of ['😀', '😀😀😀', '👍🏽', '👨‍👩‍👧', '🇯🇵🇺🇸', '❤️', ' 😀 ', '😀 hi']) {
    same(s);
    expect(jumbo(parse(s))).toBe(jumbo(parseMarkdown(s)));
  }
  expect(jumbo(parse('😀😀'))).toBe(true);
  for (const s of ['', ' ', '\t', '   \t  ', ' ', '　', ' a', 'a ', '\ta\t', 'a b', 'two  spaces', 'trailing   ']) same(s);
  const long = 'The quick brown fox jumps over the lazy dog, again and again. '.repeat(2000);
  expect(isPlain(long)).toBe(true);
  same(long);
  same(long + '*'); same('- ' + long); same(long + '\n' + long);
});

// A seeded generator (mulberry32, its full 32 bits): plain words and punctuation the gate admits, mixed with every
// excluded character and construct, at line starts, inside the text and as its last character, from one character
// to tens of thousands. The long and suffix cases are scheduled, not left to chance, and the test checks it covered them.
test('generated strings: every admitted one parses as the parser does', () => {
  let state = 0x2b7e1516;
  const next = () => { state = (state + 0x6d2b79f5) | 0; let t = Math.imul(state ^ (state >>> 15), 1 | state); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const rnd = (n: number) => Math.floor(next() * n);
  const pick = <T>(xs: T[]) => xs[rnd(xs.length)];
  const words = ['the', 'Excord', 'page', 'lands', 'café', '日本語', 'naïve', 'x-y', 'a+b', '2024', '3', '42.', 'v1.2', 'e.g.', 'it\'s',
    '"quoted"', '(paren)', '{brace}', '$5', '100%', 'a/b', 'a=b', '^up', 'what?', 'wow!', 'semi;', 'comma,', '😀', '👍🏽', '\u00a0', '\t', '  '];
  const excluded = ['*', '_', '~', '`', '|', '>', '#', '[', ']', '<', ':', '@', '\\', '\n', '\r', '\r\n', '||', '**', '~~', '__', '```'];
  const starts = [...LINE_STARTS, '', '', '', '', '', ' ', '  ', '\t', '1', '1 ', '-x', '+1', '(1) ', 'a. '];
  const ends = ['', '', '', ' ', '  ', '\t', '.', '!', '?', '-', '+', '1.', '😀', '\u00a0', ...excluded, ...CONSTRUCTS.slice(0, 8)];
  let admitted = 0, longest = 0, longAdmitted = 0;
  const lastChars = new Set<string>();
  for (let i = 0; i < 6000; i++) {
    // Every 50th a long one (2k–40k characters); every 4th only the gate's own words (near misses: `-`, `+`,
    // digits and dots inside), so many are admitted, the long ones among them.
    const long = i % 50 === 0, plain = i % 4 === 0;
    const parts: string[] = [plain ? '' : pick(starts)];
    const length = long ? 400 + rnd(6000) : 1 + rnd(24);
    for (let k = 0; k < length; k++) {
      const r = plain ? 99 : rnd(100);
      parts.push(r < 8 ? pick(excluded) : r < 11 ? pick(CONSTRUCTS) : r < 13 ? '\n' + pick(starts) : pick(words));
      if (rnd(4)) parts.push(' ');
    }
    parts.push(plain ? pick(['', '.', '!', 'end']) : pick(ends));
    const s = parts.join('');
    longest = Math.max(longest, s.length);
    lastChars.add(s.slice(-1));
    if (isPlain(s)) { admitted++; if (s.length > 10_000) longAdmitted++; }
    same(s);
  }
  expect(admitted).toBeGreaterThan(1000);
  expect(longest).toBeGreaterThan(10_000);
  expect(longAdmitted).toBeGreaterThan(0);
  expect(lastChars.size).toBeGreaterThan(20);
});
