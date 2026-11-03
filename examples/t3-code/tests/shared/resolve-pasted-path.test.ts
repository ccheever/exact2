// Ported from T3 Code 5d3d78ab93 (MIT, see LICENSE-T3): packages/shared/src/path.test.ts "resolvePastedPath", every
// case by name and vector, held by the TypeScript port (project-paths.ts) and the Contract one (pasted-path.contract).
import { describe, expect, test } from 'bun:test';
import { resolvePastedPath as resolveTs } from '../../src/shared/project-paths';
import { contractFns } from '../support/contract-fn-fixture';

const contract = contractFns('src/shared/pasted-path.contract', [], { range: (n: number) => Array.from({ length: n }, (_, i) => i), first: <T>(xs: T[]) => xs[0] });
const resolveContract = contract.resolvePastedPath as unknown as (previous: string, next: string) => string;

for (const [port, resolvePastedPath] of [['TypeScript', resolveTs], ['Contract', resolveContract]] as const) describe(`resolvePastedPath (${port})`, () => {
  test.each([
    ['Finder, a space', '', "'/Users/me/Monarch Money'", '/Users/me/Monarch Money'],
    ['Finder, an apostrophe', '', "/Users/me/Bob\\'s\\ Stuff", "/Users/me/Bob's Stuff"],
    ['Finder, a backslash', '', '/Users/me/a\\\\\\ b', '/Users/me/a\\ b'],
    ['Finder, apostrophes around a name', '', "/Users/me/\\'My\\ Folder\\'", "/Users/me/'My Folder'"],
    ['Explorer, a drive', '', '"C:\\Users\\me\\My Code"', 'C:\\Users\\me\\My Code'],
    ['Explorer, a share', '', '"\\\\host\\share\\My Code"', '\\\\host\\share\\My Code'],
    ['an apostrophe in double quotes', '', '"/Users/me/Bob\'s Stuff"', "/Users/me/Bob's Stuff"],
    ['a shell, home-relative', '', "~/Bob\\'s\\ Stuff", "~/Bob's Stuff"],
    ['surrounding whitespace', '', "  '/Users/me/My Code'\n", '/Users/me/My Code'],
    ['over a selection', '/Users/me/old', "'/Users/me/My Code'", '/Users/me/My Code'],
    ['after a prefilled folder', '~/', "~/'/Users/me/My Code'", '/Users/me/My Code'],
    ['plain, after a prefilled folder', '~/', '~//Users/me/code', '/Users/me/code'],
    ['after a Windows folder', 'C:\\Users\\', 'C:\\Users\\"D:\\My Code"', 'D:\\My Code'],
    ["a shell's quoted apostrophe", '', "'/Users/me/Bob'\\''s Work'", "/Users/me/Bob's Work"],
    ['after a clone destination', '~/code/repo', "~/code/repo'/a b'", '/a b'],
    ['over part of the box', '~/code/old', "~/code/'/a b'", '/a b'],
  ])('reads the path copied from %s', (_source, previous, next, expected) => {
    expect(resolvePastedPath(previous, next)).toBe(expected);
  });

  test.each([
    ['typing a separator', '/Users/me', '/Users/me/'],
    ['typing a separator after a folder', '~/', '~//'],
    ['typing a trailing space', '/Users/My', '/Users/My '],
    ['deleting', '/Users/me/', '/Users/me'],
    ['pasting a relative folder', '~/', '~/Documents/code'],
    ['a longer path over a selection', '/Users/me', '/Users/me/code'],
    ['a longer path after a folder', '/Users/me/', '/Users/me/code'],
    ['quoted text that is no path', '', "'hello world'"],
    ['a quoted relative path', '', "'code/my app'"],
    ['a search ending in a quoted path', '', "find '/tmp/repo'"],
    ['a quote left open', '', "'/Users/me/My Code"],
    ['a quote inside the quotes', '', "'/Users/me/it's here'"],
    ['Windows separators', '', 'C:\\Users\\me\\code'],
    ['a Windows dot folder', '', 'C:\\Users\\me\\.config'],
    ['a Windows dot folder below home', '', '~/code\\.vscode'],
    ['a share', '', '\\\\host\\share\\code'],
    ['a backslash before a letter', '', '/Users/me/a\\b'],
  ])('keeps %s', (_case, previous, next) => {
    expect(resolvePastedPath(previous, next)).toBe(next);
  });

  test.each(['/Users/me/a\\ b', '~/code\\.vscode', "'/Users/me/My Code'"])('leaves %s typed one character at a time', typed => {
    const box = [...typed].reduce((text, char) => resolvePastedPath(text, text + char), '');
    expect(box).toBe(typed);
  });
});

test('the Contract port reads every short mix of quotes, escapes and separators as the TypeScript one does', () => {
  // A fixed linear congruential sequence, so a failure names the same inputs each run.
  let seed = 7;
  const next = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
  const alphabet = ["'", '"', '\\', ' ', '/', '~', 'a', 'C', ':', '(', '\n'];
  const text = (length: number) => Array.from({ length }, () => alphabet[Math.floor(next() * alphabet.length)]).join('');
  for (let i = 0; i < 4000; i++) {
    const previous = ['', '~/', '/a/', 'C:\\', text(3)][i % 5]!;
    const typed = `${next() < 0.5 ? previous : ''}${text(1 + Math.floor(next() * 10))}`;
    expect([previous, typed, resolveContract(previous, typed)]).toEqual([previous, typed, resolveTs(previous, typed)]);
  }
});
