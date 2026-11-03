import { expect, test } from 'bun:test';
import { markdownHtmlImages } from '../../src/media/media-html';
import { markdownGallery, markdownMediaChips } from '../../src/media/media-views';
import { messageChips } from '../../src/timeline/message-chips';

const picture = `<picture>
<source media="(prefers-color-scheme: dark)" srcset="dark.png 2x, ignored.png 3x">
<source media="(prefers-color-scheme: dark)" srcset="second.png">
<source media="(prefers-color-scheme: light)" srcset="https://example.com/light.png">
<img src="fallback.png" alt="A &amp; B" onerror="alert(1)" width="120">
</picture>`;

test('raw image sources use the same media loader and theme-specific preview as Markdown images', () => {
  expect(markdownHtmlImages(picture)).toEqual([{ src: 'fallback.png', dark: 'dark.png', light: 'https://example.com/light.png', alt: 'A & B', offset: 0 }]);
  const chips = markdownMediaChips(picture, '/repo');
  expect(chips.map(chip => [chip.href, chip.icon, chip.target])).toEqual([
    ['t3-file:fallback.png', 'environment', 'media:/repo/fallback.png'],
    ['t3-file:dark.png', 'environment', 'media:/repo/dark.png'],
    ['https://example.com/light.png', 'direct', 'https://example.com/light.png'],
  ]);
  const document = { text: `![Before](before.png)\n\n${picture}\n\n![After](after.png)`, root: '/repo', scope: {} };
  expect(markdownGallery(document, 'dark').map(image => image.key)).toEqual(['media:/repo/before.png', 'media:/repo/dark.png', 'media:/repo/after.png']);
  expect(markdownGallery(document, 'light')[1]?.key).toBe('https://example.com/light.png');
});

test('sanitization drops forbidden sources and attributes, keeps the first source, and ignores non-image markup', () => {
  const text = `<picture><source media="(prefers-color-scheme: dark)" srcset="javascript:x"><source media="(prefers-color-scheme: dark)" srcset="later.png"><img src="https://example.com/fallback.png"></picture>`;
  expect(markdownHtmlImages(text)[0]?.dark).toBe('');
  expect(markdownGallery({ text, root: '/repo', scope: {} }, 'dark')[0]?.key).toBe('https://example.com/fallback.png');
  expect(markdownMediaChips('<img src="data:image/png;base64,AA" alt="blocked">', '/repo')[0]?.icon).toBe('blocked');
  expect(markdownHtmlImages('```html\n<img src="code.png">\n```\n\nText <img src="inline.png">\n\n<div><img src="wrapped.png"></div>')).toEqual([]);
  expect(markdownHtmlImages('<img src="a&#38;b.png" src="ignored.png">')[0]?.src).toBe('a&b.png');
});

test("every chip carries the text directions markdown.contract's ChipView requires", () => {
  const chips = messageChips({ type: 'assistant_message', text: `${picture}\n\n[notes](notes.md)` }, '/repo', []);
  expect(chips.length).toBeGreaterThan(1);
  for (const chip of chips) expect([chip.labelDir, chip.quoteDir, chip.commentDir]).toEqual(['ltr', 'ltr', 'ltr']);
});
