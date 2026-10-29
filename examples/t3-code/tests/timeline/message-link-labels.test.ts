// What a user message reads as before it collapses (message-chips.ts markdownLinkLabels, shouldCollapseUserMessage):
// each link or image as its label, the same as the reference's regular expression gives it, in linear time.
import { describe, expect, test } from 'bun:test';
import { markdownLinkLabels, shouldCollapseUserMessage } from '../../src/timeline/message-chips';

/** MessagesTimeline.logic.ts's MARKDOWN_LINK, which markdownLinkLabels reads as. */
const MARKDOWN_LINK = /!?\[([^\]\n]*)\]\((?:<[^>\n]*>|[^\s)]*)\)/g;

describe('a user message read with its links as their labels', () => {
  test('reads as the reference\'s expression does, over every arrangement of brackets, parentheses and spaces', () => {
    const alphabet = ['[', ']', '(', ')', '!', '<', '>', ' ', '\n', 'a', 'b', ' '];
    let seed = 7;
    const random = () => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed / 2147483648; };
    for (let i = 0; i < 20_000; i++) {
      const text = Array.from({ length: Math.floor(random() * 40) }, () => alphabet[Math.floor(random() * alphabet.length)]).join('');
      expect([text, markdownLinkLabels(text)]).toEqual([text, text.replace(MARKDOWN_LINK, '$1')]);
    }
    expect(markdownLinkLabels('See ![chart](a.png) and [the docs](<https://example.com/a b>) or [x]() !')).toBe('See chart and the docs or x !');
  });

  test('a line of many unclosed `![` or `(` reads in linear time', () => {
    // The reference's expression read on to the line's end from each `[`: 22 s in Bun, longer in the app's Hermes.
    for (const line of [`Unclosed images: ${'![a'.repeat(85_000)}`, '![a]('.repeat(51_000), `[${'[![a](b)'.repeat(32_000)}`]) {
      const start = performance.now();
      shouldCollapseUserMessage(line);
      const took = performance.now() - start;
      expect([line.slice(0, 8), took < 1_000]).toEqual([line.slice(0, 8), true]);
    }
  });
});
