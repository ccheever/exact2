import { expect, test } from 'bun:test';
import { markdownRoute, prefersMarkdown } from '../markdown.mjs';

test('Markdown is preferred only at a quality no lower than HTML', () => {
  expect(prefersMarkdown('text/markdown')).toBe(true);
  expect(prefersMarkdown('text/markdown, text/html;q=0.9')).toBe(true);
  expect(prefersMarkdown('text/html;q=0.5, text/markdown;q=0.5')).toBe(true);
  expect(prefersMarkdown('text/html;q=0.5, text/markdown;q=0.4')).toBe(false);
  expect(prefersMarkdown('text/markdown;q=0')).toBe(false);
  expect(prefersMarkdown('text/html,application/xhtml+xml,*/*;q=0.8')).toBe(false);
  expect(prefersMarkdown('*/*')).toBe(false);
  expect(prefersMarkdown('')).toBe(false);
});

test("a page's path names its Markdown file; a file's path names none", () => {
  expect(markdownRoute('/')).toBe('/index.md');
  expect(markdownRoute('/start')).toBe('/start.md');
  expect(markdownRoute('/learn/contract/')).toBe('/learn/contract.md');
  expect(markdownRoute('/app.js')).toBe(null);
  expect(markdownRoute('/start.md')).toBe(null);
  expect(markdownRoute('/%E0')).toBe(null);
});
