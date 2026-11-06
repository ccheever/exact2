// Golden ranges from T3 Code 365aa87982 compute/native adapter exports (MIT; see LICENSE-T3).
// Cases cover the executed intraline corpus, including UTF-16, admission and pairing boundaries.
import { describe, expect, test } from 'bun:test';
import { reviewWordRanges, reviewMarkedTokens } from './review-word-diff';
import { reviewFileRows, reviewPatch, reviewLineTokens } from './review-model';
import { mobileCodeTokens, type ThreadCodeToken } from './thread-highlight';
import { mobileReviewColors } from './review-colors';
import type { DiffFileModel } from './shared/diff';

type Ranges = { start: number; end: number }[][];
const cases: { name: string; before: string; after: string; ranges: Ranges }[] = [
  { name: "ordinary seed", before: "const label = 'before';", after: "const label = 'after';", ranges: [[{"start": 15, "end": 21}], [{"start": 15, "end": 20}]] },
  { name: "identical", before: "const label = 'same';", after: "const label = 'same';", ranges: [[], []] },
  { name: "both empty", before: "", after: "", ranges: [[], []] },
  { name: "empty deletion", before: "", after: "x", ranges: [[], []] },
  { name: "empty addition", before: "x", after: "", ranges: [[], []] },
  { name: "one separator joins", before: "old old", after: "new new", ranges: [[], []] },
  { name: "punctuation separators", before: "const stable = foo.bar.baz(value);", after: "const stable = new.bar.qux(value);", ranges: [[{"start": 15, "end": 18}, {"start": 23, "end": 26}], [{"start": 15, "end": 18}, {"start": 23, "end": 26}]] },
  { name: "boundary spaces", before: "  const stable = 'old';  ", after: "  const stable = 'new';  ", ranges: [[{"start": 18, "end": 21}], [{"start": 18, "end": 21}]] },
  { name: "indentation only", before: "  const label = 'same';", after: "    const label = 'same';", ranges: [[], []] },
  { name: "trailing spaces only", before: "const label = 'same';  ", after: "const label = 'same';    ", ranges: [[], []] },
  { name: "4 separated ranges", before: "stable_head_0123456789 old0 stable_anchor_0_0123456789 old1 stable_anchor_1_0123456789 old2 stable_anchor_2_0123456789 old3 stable_anchor_3_0123456789", after: "stable_head_0123456789 new0 stable_anchor_0_0123456789 new1 stable_anchor_1_0123456789 new2 stable_anchor_2_0123456789 new3 stable_anchor_3_0123456789", ranges: [[{"start": 23, "end": 27}, {"start": 55, "end": 59}, {"start": 87, "end": 91}, {"start": 119, "end": 123}], [{"start": 23, "end": 27}, {"start": 55, "end": 59}, {"start": 87, "end": 91}, {"start": 119, "end": 123}]] },
  { name: "5 separated ranges", before: "stable_head_0123456789 old0 stable_anchor_0_0123456789 old1 stable_anchor_1_0123456789 old2 stable_anchor_2_0123456789 old3 stable_anchor_3_0123456789 old4 stable_anchor_4_0123456789", after: "stable_head_0123456789 new0 stable_anchor_0_0123456789 new1 stable_anchor_1_0123456789 new2 stable_anchor_2_0123456789 new3 stable_anchor_3_0123456789 new4 stable_anchor_4_0123456789", ranges: [[], []] },
  { name: "coverage exactly45percent", before: "abcdefghijk oldoldold", after: "abcdefghijk newnewnew", ranges: [[{"start": 12, "end": 21}], [{"start": 12, "end": 21}]] },
  { name: "coverage over45percent", before: "abcdefghijk oldoldoldx", after: "abcdefghijk newnewnewy", ranges: [[], []] },
  { name: "line length 1000", before: 's'.repeat(995) + " old0", after: 's'.repeat(995) + " new0", ranges: [[{"start": 996, "end": 1000}], [{"start": 996, "end": 1000}]] },
  { name: "line length 1001", before: 's'.repeat(996) + " old0", after: 's'.repeat(996) + " new0", ranges: [[], []] },
  { name: "one side exceeds1000", before: 's'.repeat(995) + " old0", after: 's'.repeat(996) + " new0", ranges: [[], []] },
  { name: "accented Latin", before: "const café = Ångström + stable_anchor;", after: "const café = Éclair + stable_anchor;", ranges: [[{"start": 13, "end": 21}], [{"start": 13, "end": 19}]] },
  { name: "Greek identifiers", before: "const σταθερό = παλιό + anchor_value;", after: "const σταθερό = νέο + anchor_value;", ranges: [[{"start": 16, "end": 21}], [{"start": 16, "end": 19}]] },
  { name: "CJK identifiers", before: "const 稳定名称 = 旧值 + anchor_value;", after: "const 稳定名称 = 新值 + anchor_value;", ranges: [[{"start": 13, "end": 14}], [{"start": 13, "end": 14}]] },
  { name: "astral and combining", before: "const glyph = \"🙂 café é\"; const stable = 1;", after: "const glyph = \"😀 café è\"; const stable = 1;", ranges: [[{"start": 15, "end": 17}, {"start": 24, "end": 25}], [{"start": 15, "end": 17}, {"start": 24, "end": 25}]] },
  { name: "tab before changed word", before: "\tconst stable = 'old';", after: "\tconst stable = 'new';", ranges: [[{"start": 17, "end": 20}], [{"start": 17, "end": 20}]] },
  { name: "tab space only", before: "\tconst label = 'same';", after: "    const label = 'same';", ranges: [[], []] },
  { name: "one terminal newline", before: "const stable = old;\n", after: "const stable = new;\n", ranges: [[{"start": 15, "end": 18}], [{"start": 15, "end": 18}]] },
];
const bits = (text: string, ranges: Ranges[number]) => Array.from({ length: text.length }, (_, index) => ranges.some(range => index >= range.start && index < range.end));
const marks = (tokens: { text: string; mark: boolean }[]) => tokens.flatMap(token => Array.from({ length: token.text.length }, () => token.mark));

describe('mobile native intraline admission', () => {
  test.each(cases)('$name uses the recorded native ranges', ({ before, after, ranges }) => {
    expect(reviewWordRanges(before, after)).toEqual(ranges);
  });
  test.each(cases.filter(item => !item.before.includes('\n') && !item.after.includes('\n')))('$name reaches real Review tokens', ({ before, after, ranges }) => {
    const file: DiffFileModel = { path: 'example.txt', previous: 'example.txt', status: 'modified', additions: 1, deletions: 1, binary: false,
      hunks: [{ oldStart: 1, newStart: 1, lines: [{ kind: 'deletion', text: before, old: 1, next: 0 }, { kind: 'addition', text: after, old: 0, next: 1 }] }] };
    const rows = reviewFileRows(file, false, null).filter(row => row.kind === 'line');
    expect(rows.map(row => row.tokens.map(token => token.text).join(''))).toEqual([before, after].map(text => text.replace(/\t/g, '    ')));
    expect(rows.map(row => new Set(row.tokens.map(token => token.id)).size)).toEqual(rows.map(row => row.tokens.length));
    rows.forEach((row, side) => {
      const raw = side === 0 ? before : after;
      const displayed = bits(raw, ranges[side]!).flatMap((marked, index) => Array.from({ length: raw[index] === '\t' ? 4 : 1 }, () => marked));
      expect(marks(row.tokens)).toEqual(displayed);
    });
  });
});

const patches = [
  { name: "two paired lines", patch: "diff --git a/block.txt b/block.txt\n--- a/block.txt\n+++ b/block.txt\n@@ -1,2 +1,2 @@\n-const first = old;\n-const second = before;\n+const first = new;\n+const second = after;\n", rows: [{"id": "block.txt:line:0", "path": "block.txt", "lineIndex": 0, "text": "const first = old;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:1", "path": "block.txt", "lineIndex": 1, "text": "const second = before;", "oldNumber": "2", "newNumber": "", "change": "deletion", "ranges": [{"start": 15, "end": 21}]}, {"id": "block.txt:line:2", "path": "block.txt", "lineIndex": 2, "text": "const first = new;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:3", "path": "block.txt", "lineIndex": 3, "text": "const second = after;", "oldNumber": "", "newNumber": "2", "change": "addition", "ranges": [{"start": 15, "end": 20}]}] },
  { name: "one unpaired addition", patch: "diff --git a/block.txt b/block.txt\n--- a/block.txt\n+++ b/block.txt\n@@ -1,1 +1,2 @@\n-const first = old;\n+const first = new;\n+const unmatched = added;\n", rows: [{"id": "block.txt:line:0", "path": "block.txt", "lineIndex": 0, "text": "const first = old;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:1", "path": "block.txt", "lineIndex": 1, "text": "const first = new;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:2", "path": "block.txt", "lineIndex": 2, "text": "const unmatched = added;", "oldNumber": "", "newNumber": "2", "change": "addition", "ranges": []}] },
  { name: "context separates runs", patch: "diff --git a/block.txt b/block.txt\n--- a/block.txt\n+++ b/block.txt\n@@ -1,3 +1,3 @@\n-const first = old;\n+const first = new;\n unchanged context\n-const second = before;\n+const second = after;\n", rows: [{"id": "block.txt:line:0", "path": "block.txt", "lineIndex": 0, "text": "const first = old;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:1", "path": "block.txt", "lineIndex": 1, "text": "const first = new;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:2", "path": "block.txt", "lineIndex": 2, "text": "unchanged context", "oldNumber": "2", "newNumber": "2", "change": "context", "ranges": []}, {"id": "block.txt:line:3", "path": "block.txt", "lineIndex": 3, "text": "const second = before;", "oldNumber": "3", "newNumber": "", "change": "deletion", "ranges": [{"start": 15, "end": 21}]}, {"id": "block.txt:line:4", "path": "block.txt", "lineIndex": 4, "text": "const second = after;", "oldNumber": "", "newNumber": "3", "change": "addition", "ranges": [{"start": 15, "end": 20}]}] },
  { name: "two hunks stay separate", patch: "diff --git a/block.txt b/block.txt\n--- a/block.txt\n+++ b/block.txt\n@@ -1,1 +1,1 @@\n-const first = old;\n+const first = new;\n@@ -9,1 +9,1 @@\n-const second = before;\n+const second = after;\n", rows: [{"id": "block.txt:line:0", "path": "block.txt", "lineIndex": 0, "text": "const first = old;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:1", "path": "block.txt", "lineIndex": 1, "text": "const first = new;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 14, "end": 17}]}, {"id": "block.txt:line:2", "path": "block.txt", "lineIndex": 2, "text": "const second = before;", "oldNumber": "9", "newNumber": "", "change": "deletion", "ranges": [{"start": 15, "end": 21}]}, {"id": "block.txt:line:3", "path": "block.txt", "lineIndex": 3, "text": "const second = after;", "oldNumber": "", "newNumber": "9", "change": "addition", "ranges": [{"start": 15, "end": 20}]}] },
  { name: "two files stay separate", patch: "diff --git a/first.txt b/first.txt\n--- a/first.txt\n+++ b/first.txt\n@@ -1,1 +1,1 @@\n-const first = old;\n+const first = new;\ndiff --git a/second.txt b/second.txt\n--- a/second.txt\n+++ b/second.txt\n@@ -1,1 +1,1 @@\n-const second = before;\n+const second = after;\n", rows: [{"id": "first.txt:line:0", "path": "first.txt", "lineIndex": 0, "text": "const first = old;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 14, "end": 17}]}, {"id": "first.txt:line:1", "path": "first.txt", "lineIndex": 1, "text": "const first = new;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 14, "end": 17}]}, {"id": "second.txt:line:0", "path": "second.txt", "lineIndex": 0, "text": "const second = before;", "oldNumber": "1", "newNumber": "", "change": "deletion", "ranges": [{"start": 15, "end": 21}]}, {"id": "second.txt:line:1", "path": "second.txt", "lineIndex": 1, "text": "const second = after;", "oldNumber": "", "newNumber": "1", "change": "addition", "ranges": [{"start": 15, "end": 20}]}] },
  { name: "comment control", patch: "diff --git a/example.txt b/example.txt\n--- a/example.txt\n+++ b/example.txt\n@@ -1,3 +1,3 @@\n const stable = 1;\n-const label = 'before';\n+const label = 'after';\n const tail = 2;\n", rows: [{"id": "example.txt:line:0", "path": "example.txt", "lineIndex": 0, "text": "const stable = 1;", "oldNumber": "1", "newNumber": "1", "change": "context", "ranges": []}, {"id": "example.txt:line:1", "path": "example.txt", "lineIndex": 1, "text": "const label = 'before';", "oldNumber": "2", "newNumber": "", "change": "deletion", "ranges": [{"start": 15, "end": 21}]}, {"id": "example.txt:line:2", "path": "example.txt", "lineIndex": 2, "text": "const label = 'after';", "oldNumber": "", "newNumber": "2", "change": "addition", "ranges": [{"start": 15, "end": 20}]}, {"id": "example.txt:line:3", "path": "example.txt", "lineIndex": 3, "text": "const tail = 2;", "oldNumber": "3", "newNumber": "3", "change": "context", "ranges": []}] },
];

describe('Review source pairing and projection', () => {
  test.each(patches)('$name preserves source order, IDs and gutters', ({ patch, rows: expected }) => {
    const rows = reviewPatch(patch).files.flatMap(file => reviewFileRows(file, false, null)).filter(row => row.kind === 'line');
    expect(rows.map(({ id, path, lineIndex, text, oldNumber, newNumber, change }) => ({ id, path, lineIndex, text, oldNumber, newNumber, change })))
      .toEqual(expected.map(({ ranges, ...row }) => row));
    rows.forEach((row, index) => expect(marks(row.tokens)).toEqual(bits(row.text, expected[index]!.ranges)));
  });
  test('splitting preserves raw syntax styles and expands a tab only after source offsets', () => {
    const tokens: ThreadCodeToken[] = [
      { id: 'a', text: '\tconst ', color: '#123456', weight: 700, slant: 'italic' },
      { id: 'b', text: 'old value', color: '#654321', weight: 400, slant: 'normal' },
    ];
    const marked = reviewMarkedTokens(tokens, [{ start: 7, end: 10 }]);
    expect(marked).toEqual([
      { ...tokens[0]!, text: '    const ', mark: false },
      { ...tokens[1]!, id: 'b@0', text: 'old', mark: true },
      { ...tokens[1]!, id: 'b@3', text: ' value', mark: false },
    ]);
    expect(tokens[0]!.text).toBe('\tconst '); expect(tokens[1]!.text).toBe('old value');
  });
  test.each([false, true])('real complete-side syntax survives marked splits (dark=%s)', dark => {
    const before = "/* open\n * const label = 'before';\n */", after = "/* open\n * const label = 'after';\n */";
    const source = (text: string) => text.split('\n').map((content, index) => ({ content, change: 'context' as const, oldLineNumber: index + 1, newLineNumber: index + 1 }));
    const old = reviewLineTokens(source(before), 'a.ts', dark, [[], [{ start: 18, end: 24 }], []]);
    const next = reviewLineTokens(source(after), 'a.ts', dark, [[], [{ start: 18, end: 23 }], []]);
    const styles = (tokens: ThreadCodeToken[]) => tokens.flatMap(token => Array.from({ length: token.text.length }, () => [token.color, token.weight, token.slant]));
    expect(styles(old.flat())).toEqual(styles(mobileCodeTokens(before, 'ts', dark).map(token => ({ ...token, text: token.text.replace(/\n/g, '') }))));
    expect(styles(next.flat())).toEqual(styles(mobileCodeTokens(after, 'ts', dark).map(token => ({ ...token, text: token.text.replace(/\n/g, '') }))));
    expect(old.map(row => row.map(token => token.text).join(''))).toEqual(before.split('\n'));
    expect(next.map(row => row.map(token => token.text).join(''))).toEqual(after.split('\n'));
  });
  test('source-only Files and attachment rows remain unmarked', () => {
    const rows = reviewLineTokens([{ content: "\tconst label = 'before';", change: 'context', oldLineNumber: null, newLineNumber: 1 }], 'a.ts', false);
    expect(rows[0]!.map(token => token.text).join('')).toBe("    const label = 'before';");
    expect(rows.flat().every(token => !token.mark)).toBe(true);
  });
  test.each(['light', 'dark'])('intraline fills retain source bar colors at alpha .28 (%s)', scheme => {
    const colors = mobileReviewColors(scheme);
    expect(colors.additionHighlight).toBe('rgba(0, 202, 177, 0.28)');
    expect(colors.deletionHighlight).toBe('rgba(255, 46, 63, 0.28)');
  });
});
