// The images inside a line of a message (media-views.ts markdownInlineImages, markdownImages): the same images as the
// scan that read on to the line's end from each `![`, `[` and run of backticks found, in linear time.
import { describe, expect, test } from 'bun:test';
import { markdownImages, markdownInlineImages } from '../../src/media/media-views';

/** The scan as it read before it found each mark once: each image's alt, destination and whether it is in a link's text. */
function readingOn(line: string): { alt: string; src: string; linked: boolean }[] {
  const run = (from: number) => { let end = from; while (line[end] === '`') end++; return end - from; };
  const codeSpanEnd = (at: number) => {
    const length = run(at);
    for (let from = at + length; ;) {
      const found = line.indexOf('`'.repeat(length), from);
      if (found < 0) return at + length;
      if (run(found) === length) return found + length;
      from = found + run(found);
    }
  };
  const startsLinkText = (open: number) => {
    for (let at = open + 1, depth = 1; at < line.length;) {
      if (line[at] === '\\') at += 2;
      else if (line[at] === '`') at = codeSpanEnd(at);
      else if (line[at] === '[') { depth++; at++; }
      else if (line[at] === ']') { if (--depth === 0) return line[at + 1] === '('; at++; }
      else at++;
    }
    return false;
  };
  const imageSpan = (at: number) => {
    const close = line.indexOf(']', at + 2);
    if (close < 0 || line[close + 1] !== '(') return null;
    for (let index = close + 1, depth = 0; index < line.length; index++) {
      if (line[index] === '(') depth++;
      else if (line[index] === ')' && --depth === 0) return { close, end: index };
    }
    return null;
  };
  const target = (inside: string) => {
    let value = inside.trim();
    const cut = value.search(/["']/);
    if (cut >= 0) value = value.slice(0, cut).trim();
    return value.replace(/^<+/, '').replace(/>+$/, '');
  };
  const images: { alt: string; src: string; linked: boolean }[] = [], opened: number[] = [];
  for (let at = 0; at < line.length;) {
    if (line[at] === '\\') at += 2;
    else if (line[at] === '`') at = codeSpanEnd(at);
    else if (line[at] === '[') opened.push(at++);
    else if (line[at] === ']') { opened.pop(); at++; }
    else if (line[at] === '!' && line[at + 1] === '[') {
      const span = imageSpan(at);
      if (!span || line.slice(at + 2, span.close).includes('[')) { at++; continue; }
      const linked = opened.length > 0 && startsLinkText(opened[opened.length - 1]!);
      images.push({ alt: line.slice(at + 2, span.close), src: target(line.slice(span.close + 2, span.end)), linked });
      at = span.end + 1;
    } else at++;
  }
  return images;
}

describe('the images inside a line', () => {
  test('are the ones the scan reading on from each mark found, over every arrangement of brackets, parentheses, escapes and backticks', () => {
    const alphabet = ['!', '[', ']', '(', ')', '`', '\\', 'a', ' ', '<', '>', '"'];
    let seed = 11;
    const random = () => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed / 2147483648; };
    // Each line starts with a letter, so that none opens a fence.
    for (let i = 0; i < 20_000; i++) {
      const line = `a${Array.from({ length: Math.floor(random() * 40) }, () => alphabet[Math.floor(random() * alphabet.length)]).join('')}`;
      const found = markdownInlineImages(line).map(({ alt, src, linked }) => ({ alt, src, linked }));
      expect([line, found]).toEqual([line, line.includes('![') ? readingOn(line) : []]);
    }
    expect(markdownInlineImages('[See ![a](a.png)](https://a.com) `![b](b.png)` ![c](c (1).png "t") \\![d](d.png)').map(({ alt, src, linked }) => [alt, src, linked]))
      .toEqual([['a', 'a.png', true], ['c', 'c (1).png', false]]);
    expect(markdownImages('![a](a.png)\n![b](b.png) after\n![c](c (1).png)').map(image => image.src)).toEqual(['a.png', 'c (1).png']);
  });

  test('a line of many unclosed `![` or `(`, or of images in an unclosed link\'s text, reads in linear time', () => {
    // Each took from 0.4 s to 7 s in Bun when the scan read on to the line's end from each mark it found, and some
    // forty times that in the app's Hermes.
    const ticks = Array.from({ length: 1_000 }, (_, k) => `${'`'.repeat(k + 1)}x `).join('');
    for (const line of [`x ${'![a'.repeat(170_000)}`, '![a]('.repeat(51_000), `[${'[![a](b)'.repeat(32_000)}`, `![a](b) ${ticks}`, `![${'a!['.repeat(170_000)}](b)`]) {
      const start = performance.now();
      markdownInlineImages(line);
      markdownImages(line);
      const took = performance.now() - start;
      expect([line.slice(0, 8), took < 300]).toEqual([line.slice(0, 8), true]);
    }
  });
});
