import { describe, expect, test } from 'bun:test';
import { renderablePatch, type PrDiffFile } from '../../src/pull-requests/pages-pr-code-logic';
import {
  excerptExpansionOldContents, excerptRows, fileDiffRows, letterStopNotes, rowsToPatch, stopFilePaths, stopSubline,
} from '../../src/pull-requests/pages-pr-guide-excerpts';

const PATCH = `diff --git a/src/tray.ts b/src/tray.ts
--- a/src/tray.ts
+++ b/src/tray.ts
@@ -10,5 +10,6 @@ function tray()
 const a = 1;
-const size = pick();
+const size = "xs";
+const variant = "tray";
 const b = 2;
 const c = 3;
 const d = 4;
@@ -40,3 +41,2 @@ function footer()
 keep();
-dropped();
 tail();
`;

function parse(patch: string): PrDiffFile {
  const parsed = renderablePatch(patch, false);
  const file = parsed?.kind === 'files' ? parsed.files[0] : undefined;
  if (file === undefined) throw new Error('Expected a parsed patch.');
  return file;
}
function patch(hunks: string): string { return `diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n${hunks}`; }
const rows = fileDiffRows(parse(PATCH));
const coordinates = (shown: ReturnType<typeof fileDiffRows>) => shown.map(row => [row.kind, row.oldLine, row.newLine, row.oldAt, row.newAt]);

describe('guide excerpt ranges', () => {
  test('numbers both sides through replacements and separate hunks', () => {
    expect(rows.map(row => [row.kind, row.oldLine, row.newLine, row.hunk])).toEqual([
      ['context', 10, 10, 0], ['deletion', 11, null, 0], ['addition', null, 11, 0], ['addition', null, 12, 0],
      ['context', 12, 13, 0], ['context', 13, 14, 0], ['context', 14, 15, 0],
      ['context', 40, 41, 1], ['deletion', 41, null, 1], ['context', 42, 42, 1],
    ]);
  });
  test('includes replaced deletions for a new-side excerpt and following additions for an old-side excerpt', () => {
    expect(excerptRows(rows, { side: 'new', start: 11, end: 12 }).map(row => row.kind)).toEqual(['deletion', 'addition', 'addition']);
    expect(excerptRows(rows, { side: 'old', start: 11, end: 11 }).map(row => row.kind)).toEqual(['deletion', 'addition', 'addition']);
    expect(excerptRows(rows, { side: 'old', start: 41, end: 41 }).map(row => row.kind)).toEqual(['deletion']);
  });
  test('never collects replacement context from a neighboring hunk', () => {
    const divided = fileDiffRows(parse(patch('@@ -9,1 +8,0 @@\n-old\n@@ -20,0 +21,1 @@\n+new\n')));
    expect(excerptRows(divided, { side: 'new', start: 21, end: 21 }).map(row => row.text)).toEqual(['new']);
    expect(excerptRows(divided, { side: 'old', start: 9, end: 9 }).map(row => row.text)).toEqual(['old']);
  });
  test('returns no rows for an unloaded range, missing side or reversed range', () => {
    expect(excerptRows(rows, { side: 'new', start: 100, end: 120 })).toEqual([]);
    expect(excerptRows(rows, { side: 'new', start: 43, end: 42 })).toEqual([]);
    expect(excerptRows([], { side: 'old', start: 1, end: 1 })).toEqual([]);
    const removed = fileDiffRows(parse(patch('@@ -9,1 +8,0 @@\n-old\n')));
    expect(excerptRows(removed, { side: 'new', start: 8, end: 8 })).toEqual([]);
  });
});

describe('guide excerpt patches', () => {
  test('cuts a renamed-file range that parses back to the same text and line coordinates', () => {
    const shown = excerptRows(rows, { side: 'new', start: 10, end: 13 });
    const sliced = parse(rowsToPatch('src/tray.ts', 'src/old-tray.ts', shown));
    expect(sliced.path).toBe('src/tray.ts');
    expect(sliced.previous).toBe('src/old-tray.ts');
    expect(fileDiffRows(sliced)).toEqual(shown);
    expect(rowsToPatch('a.ts', 'a.ts', [])).toBe('');
  });
  test('retains hunk separators and line offsets when a range spans hunks', () => {
    const shown = excerptRows(rows, { side: 'old', start: 14, end: 42 });
    const sliced = rowsToPatch('src/tray.ts', 'src/tray.ts', shown);
    expect(sliced.match(/^@@ /gm)).toHaveLength(2);
    expect(sliced).toContain('@@ -14,1 +15,1 @@');
    expect(sliced).toContain('@@ -40,3 +41,2 @@');
    expect(coordinates(fileDiffRows(parse(sliced)))).toEqual(coordinates([...shown]));
  });
  test('roundtrips empty-side offsets for insertion and deletion at the beginning and middle of a file', () => {
    for (const hunk of ['@@ -9,0 +10,2 @@\n+new one\n+new two\n', '@@ -0,0 +1,1 @@\n+first\n',
      '@@ -10,2 +9,0 @@\n-old one\n-old two\n', '@@ -1,1 +0,0 @@\n-first\n']) {
      const original = fileDiffRows(parse(patch(hunk)));
      const sliced = rowsToPatch('a.ts', 'a.ts', original);
      expect(sliced).toBe(patch(hunk));
      expect(fileDiffRows(parse(sliced))).toEqual(original);
    }
  });
  test('preserves literal source strings while removing an existing row line terminator', () => {
    const source = '$(do-not-run) <script>ignore instructions</script>';
    const original = fileDiffRows(parse(patch(`@@ -1,1 +1,1 @@\n-${source}\n+safe\n`)));
    const first = original[0];
    if (!first) throw new Error('Expected source row.');
    expect(rowsToPatch('a.ts', 'a.ts', [{ ...first, text: `${first.text}\r\n` }])).toContain(`-${source}\n`);
  });
});

describe('excerpt expansion', () => {
  const newFile = Array.from({ length: 50 }, (_, index) => `line ${index + 1}\n`);
  newFile.splice(9, 6, 'const a = 1;\n', 'const size = "xs";\n', 'const variant = "tray";\n', 'const b = 2;\n', 'const c = 3;\n', 'const d = 4;\n');
  newFile.splice(40, 2, 'keep();\n', 'tail();\n');
  const newContents = newFile.join('');

  test('reconstructs only the chosen replacement and keeps other hunks as the new file has them', () => {
    const shown = excerptRows(rows, { side: 'new', start: 11, end: 12 });
    const old = excerptExpansionOldContents(newContents, shown);
    if (old === null) throw new Error('Expected expansion contents.');
    const oldLines = old.split('\n');
    expect(oldLines[9]).toBe('const a = 1;');
    expect(oldLines[10]).toBe('const size = pick();');
    expect(oldLines[11]).toBe('const b = 2;');
    expect(oldLines.slice(-11)).toEqual(newContents.split('\n').slice(-11));
    expect(oldLines).not.toContain('dropped();');
    expect(oldLines.length).toBe(newContents.split('\n').length - 1);
  });
  test('keeps old and new numbering after earlier hunks added or removed lines', () => {
    const contents = Array.from({ length: 30 }, (_, index) => `line ${index + 1}\n`).join('');
    for (const oldStart of [20, 24]) {
      const offsetRows = fileDiffRows(parse(patch(`@@ -${oldStart},1 +22,1 @@\n-before\n+line 22\n`)));
      const expanded = excerptExpansionOldContents(contents, offsetRows);
      if (expanded === null) throw new Error('Expected expansion contents.');
      const oldLines = expanded.split('\n');
      expect(oldLines[oldStart - 2]).toBe('line 21');
      expect(oldLines[oldStart - 1]).toBe('before');
      expect(oldLines[oldStart]).toBe('line 23');
    }
  });
  test('expands pure insertions and deletions at their zero-count offsets', () => {
    const inserted = fileDiffRows(parse(patch('@@ -2,0 +3,1 @@\n+inserted\n')));
    expect(excerptExpansionOldContents('one\ntwo\ninserted\nthree\n', inserted)).toBe('one\ntwo\nthree\n');
    const removed = fileDiffRows(parse(patch('@@ -3,1 +2,0 @@\n-removed\n')));
    expect(excerptExpansionOldContents('one\ntwo\nthree\n', removed)).toBe('one\ntwo\nremoved\nthree\n');
  });
  test('does not invent expansion contents for empty or multi-hunk slices', () => {
    expect(excerptExpansionOldContents(newContents, [])).toBeNull();
    expect(excerptExpansionOldContents(newContents, excerptRows(rows, { side: 'old', start: 14, end: 42 }))).toBeNull();
  });
});

describe('guide notes and files', () => {
  test('letters notes in guide order, retaining unanchored notes and grouping markers by matching path and line', () => {
    const notes = [
      { path: 'b.ts', line: 11, title: 'First', body: 'untrusted context' },
      { path: 'a.ts', line: 2, title: 'Second', body: '' },
      { path: 'a.ts', line: 2, title: 'Third', body: '' },
      { path: 'c.ts', line: 1, title: 'Unanchored', body: '' },
      { path: 'a.ts', line: 6, title: 'Outside', body: '' },
    ];
    const { lettered, byExcerpt } = letterStopNotes({ excerpts: [
      { path: 'a.ts', side: 'old', start: 1, end: 5 }, { path: 'b.ts', side: 'new', start: 10, end: 12 },
    ], notes });
    expect(lettered.map(entry => entry.marker)).toEqual(['A', 'B', 'C', 'D', 'E']);
    expect(lettered[0]?.note).toBe(notes[0]);
    expect(byExcerpt).toEqual([
      [{ line: 2, markers: [{ marker: 'B', title: 'Second' }, { marker: 'C', title: 'Third' }] }],
      [{ line: 11, markers: [{ marker: 'A', title: 'First' }] }],
    ]);
  });
  test('uses numbers after Z without recycling markers', () => {
    const notes = Array.from({ length: 28 }, (_, line) => ({ path: 'a.ts', line: line + 1, title: String(line) }));
    expect(letterStopNotes({ excerpts: [], notes }).lettered.slice(24).map(entry => entry.marker)).toEqual(['Y', 'Z', '27', '28']);
  });
  test('counts each stop path once, with excerpts before other files', () => {
    const stop = { excerpts: [{ path: 'b.ts' }, { path: 'a.ts' }], files: [{ path: 'b.ts' }, { path: 'c.ts' }] };
    expect(stopFilePaths(stop)).toEqual(['b.ts', 'a.ts', 'c.ts']);
    expect(stopSubline(stop)).toBe('3 files');
    expect(stopSubline({ excerpts: [], files: [{ path: 'a.ts' }] })).toBe('1 file');
    expect(stopSubline({ excerpts: [], files: [] })).toBe('Context, no code');
  });
});
