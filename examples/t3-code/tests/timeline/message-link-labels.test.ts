// What a user message reads as before it collapses (message-chips.ts markdownLinkLabels, shouldCollapseUserMessage):
// each link or image as its label, the same as the reference's regular expression gives it, without its quadratic time.
import { describe, expect, test } from 'bun:test';
import { markdownLinkLabels, shouldCollapseUserMessage } from '../../src/timeline/message-chips';

/** MessagesTimeline.logic.ts's MARKDOWN_LINK, which markdownLinkLabels reads as. */
const MARKDOWN_LINK = /!?\[([^\]\n]*)\]\((?:<[^>\n]*>|[^\s)]*)\)/g;

/** mulberry32: integer steps, so the texts do not repeat, as a product kept in floating point soon does. */
function random32(seed: number): () => number {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let mixed = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

describe('a user message read with its links as their labels', () => {
  test('reads as the reference\'s expression does, over every arrangement of brackets, parentheses and spaces', () => {
    const alphabet = ['[', ']', '(', ')', '!', '<', '>', ' ', '\n', 'a', 'b', '\u00a0', '`'];
    const random = random32(7), texts = new Set<string>();
    for (const text of [...Array.from({ length: 20_000 }, () => Array.from({ length: Math.floor(random() * 40) }, () => alphabet[Math.floor(random() * alphabet.length)]).join('')),
      '![a](`) [![b](c)](d) `']) {
      texts.add(text);
      expect([text, markdownLinkLabels(text)]).toEqual([text, text.replace(MARKDOWN_LINK, '$1')]);
    }
    expect(texts.size).toBeGreaterThan(18_000);
    expect(markdownLinkLabels('See ![chart](a.png) and [the docs](<https://example.com/a b>) or [x]() !')).toBe('See chart and the docs or x !');
  });

  test('a line of many unclosed `![` or `(` takes a moment, not the square of its length', () => {
    // The reference's expression read on to the line's end from each `[`: 22 s in Bun, longer in the app's Hermes.
    for (const line of [`Unclosed images: ${'![a'.repeat(85_000)}`, '![a]('.repeat(51_000), `[${'[![a](b)'.repeat(32_000)}`]) {
      const start = performance.now();
      shouldCollapseUserMessage(line);
      const took = performance.now() - start;
      expect([line.slice(0, 8), took < 1_000]).toEqual([line.slice(0, 8), true]);
    }
  });
});
