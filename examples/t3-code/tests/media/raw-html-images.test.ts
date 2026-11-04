import { expect, test } from 'bun:test';
import { markdownHtmlImages } from '../../src/media/media-html';
import { markdownGallery, markdownMediaChips } from '../../src/media/media-views';
import { messageChips } from '../../src/timeline/message-chips';
import { contractFns } from '../support/contract-action-fixture';

const picture = `<picture>
<source media="(prefers-color-scheme: dark)" srcset="dark.png 2x, ignored.png 3x">
<source media="(prefers-color-scheme: dark)" srcset="second.png">
<source media="(prefers-color-scheme: light)" srcset="https://example.com/light.png">
<img src="fallback.png" alt="A &amp; B" onerror="alert(1)" width="120">
</picture>`;

test('raw image sources use the same media loader and theme-specific preview as Markdown images', () => {
  expect(markdownHtmlImages(picture)).toEqual([{ src: 'fallback.png', dark: 'dark.png', light: 'https://example.com/light.png', alt: 'A & B', width: 120, height: 0, link: '', offset: 0, inline: false }]);
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
  const kept = markdownHtmlImages('```html\n<img src="code.png">\n```\n\nText <img src="inline.png"> and `<img src="span.png">`\n\n<div><img src="wrapped.png"></div>\n\n<span><img src="literal.png"></span>');
  expect(kept.map(image => [image.src, image.inline])).toEqual([['inline.png', true], ['wrapped.png', false]]);
  expect(markdownHtmlImages('<img src="a&#38;b.png" src="ignored.png">')[0]?.src).toBe('a&b.png');
});

test("every chip carries the text directions markdown.contract's ChipView requires", () => {
  const chips = messageChips({ type: 'assistant_message', text: `${picture}\n\n[notes](notes.md)` }, '/repo', []);
  expect(chips.length).toBeGreaterThan(1);
  for (const chip of chips) expect([chip.labelDir, chip.quoteDir, chip.commentDir]).toEqual(['ltr', 'ltr', 'ltr']);
});

// The five cases markdown-parse draws since LLP 1033 D9's amendment (html.rs), each with the href its block or run keeps.
test('an image inside a line or an element, an open picture, references and data-dark-src are indexed as drawn', () => {
  const text = [
    'Before <img src="shots/inline.png" alt="inline"> after.',
    '',
    '<div align="center"><img src="shots/div.png" alt="div"></div>',
    '',
    '<img src="javascript&colon;alert(1)" alt="&copy; colon">',
    '',
    '<img src="shots/light.png" data-dark-src="shots/dark.png" alt="themed">',
    '',
    '<picture>',
    '<source media="(prefers-color-scheme: dark)" srcset="shots/dark.png">',
    '<img src="shots/light.png" alt="open picture">',
  ].join('\n');
  // The three top-level images after the `<div>` share one line box: media runs, as markdown_media.rs's test has them.
  expect(markdownHtmlImages(text).map(image => [image.alt, image.src, image.dark, image.inline])).toEqual([
    ['inline', 'shots/inline.png', '', true],
    ['div', 'shots/div.png', '', false],
    ['\u00a9 colon', '', '', true],
    ['themed', 'shots/light.png', 'shots/dark.png', true],
    ['open picture', 'shots/light.png', 'shots/dark.png', true],
  ]);
  const chips = markdownMediaChips(text, '/repo');
  expect(chips.map(chip => [chip.href, chip.icon])).toEqual([
    // The media runs' hrefs (markdown_media.rs inline_html_image): a themed one's light, then dark, source.
    ['t3-image:t3-file:shots/inline.png', 'environment'],
    ['t3-file:shots/div.png', 'environment'],
    ['t3-image:', 'blocked'],
    ['t3-image:t3-file:shots/light.png', 'environment'],
    ['t3-image:t3-file:shots/dark.png', 'environment'],
  ]);
  const gallery = (scheme: string) => markdownGallery({ text, root: '/repo', scope: {} }, scheme).map(image => image.key);
  expect(gallery('dark')).toEqual(['media:/repo/shots/inline.png', 'media:/repo/shots/div.png', 'media:/repo/shots/dark.png', 'media:/repo/shots/dark.png']);
  expect(gallery('light')).toEqual(['media:/repo/shots/inline.png', 'media:/repo/shots/div.png', 'media:/repo/shots/light.png', 'media:/repo/shots/light.png']);
  // The end tag after a blank line closes the open picture and draws nothing; a user message keeps HTML literal.
  expect(markdownHtmlImages('<picture><img src="a.png">\n\n</picture>\n\nText <img src="b.png">').map(image => image.src)).toEqual(['a.png', 'b.png']);
  expect(markdownMediaChips(text, '/repo', { rawHtml: false })).toEqual([]);
});

// The layout sample (acp-standin HTML_LAYOUT_SAMPLE), indexed as markdown_media.rs's an_html_image_line_keeps_its_size_link_and_align draws it.
test('images on one line box, a centred row, a linked image and sized and themed images in a line are media runs', () => {
  const text = [
    '<img src="shots/a.png" width="120" height="48" alt="first">\n<img src="shots/b.png" width="120" height="48" alt="second">',
    '<img src="shots/c.png" alt="third">',
    'Between the rows.',
    '<p align="center">\n<img src="shots/a.png" width="60" height="24" alt="centred a">\n<img src="shots/b.png" width="60" height="24" alt="centred b">\n</p>',
    '<a href="https://example.com/docs"><img src="shots/c.png" width="80" height="32" alt="linked"></a>',
    '<a href="javascript:alert(1)"><img src="shots/a.png" alt="refused link"></a>',
    'Sized: <img src="shots/inline.png" width="16" height="16" alt="sized"> and themed: <picture><source media="(prefers-color-scheme: dark)" srcset="shots/dark.png"><img src="shots/light.png" width="40" height="16" alt="themed"></picture> in a line.',
  ].join('\n\n');
  expect(markdownHtmlImages(text).map(image => [image.alt, image.inline, image.width, image.height, image.link])).toEqual([
    ['first', true, 120, 48, ''], ['second', true, 120, 48, ''], ['third', true, 0, 0, ''],
    ['centred a', true, 60, 24, ''], ['centred b', true, 60, 24, ''],
    ['linked', true, 80, 32, 'https://example.com/docs'],
    // A refused `href` is dropped: one image alone, unlinked, is an image block.
    ['refused link', false, 0, 0, ''],
    ['sized', true, 16, 16, ''], ['themed', true, 40, 16, ''],
  ]);
  const hrefs = markdownMediaChips(text, '/repo').map(chip => chip.href);
  expect(hrefs).toContain('t3-image:t3-file:shots/dark.png');
  expect(hrefs).toContain('t3-file:shots/a.png');
  // A linked image's press is its link's: it is not one of the gallery's.
  expect(markdownGallery({ text, root: '/repo', scope: {} }, 'light').map(image => image.name)).not.toContain('linked');
});

test('a line centres its items when it holds an image of no authored height or one taller than the line', () => {
  const fns = contractFns('src/timeline/markdown.contract') as { proseMedia: (runs: unknown[]) => boolean; imageLine: (runs: unknown[]) => boolean };
  const word = { kind: '', text: 'before', icon: '', height: 0 };
  const image = (height: number, icon = '') => ({ kind: 'media', text: 'alt', icon, height });
  expect([fns.proseMedia([word, image(0)]), fns.proseMedia([word, image(32)]), fns.proseMedia([word, image(16)]), fns.proseMedia([image(0, 'standalone')]), fns.proseMedia([word])])
    .toEqual([true, true, false, false, false]);
  expect([fns.imageLine([image(0), image(16)]), fns.imageLine([word, image(0)])]).toEqual([true, false]);
});
