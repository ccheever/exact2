// Settings › Appearance font previews, adapted from T3 Code (MIT, see LICENSE-T3):
// SettingsFontPreviews.tsx. Its code sample is the diff panel's file diff rendered by
// the real pipeline (`preloadPatchFile`, unified), not a lookalike: here the same patch
// goes through this clone's patch parser and highlighter (diff.ts parsePatch,
// timeline-diff-syntax.ts lineTokens), with @pierre/diffs' default intraline emphasis
// (`lineDiffType: "word-alt"`, which the static preview keeps; the Diff panel turns it
// off). Pierre's computeLineDiffDecorations pairs each deletion with the addition at the
// same index of its change block, diffs them with jsdiff's diffWordsWithSpace and joins
// the spans with pushOrJoinSpan; both are ported in word-diff.ts.
import { parsePatch } from './diff';
import { lineTokens, overlay, type Segment } from './timeline-diff-syntax';
import { wordAltMarks } from './word-diff';

export const DIFF_PREVIEW_PATCH = [
  'diff --git a/src/formatUser.ts b/src/formatUser.ts',
  '--- a/src/formatUser.ts',
  '+++ b/src/formatUser.ts',
  '@@ -1,3 +1,3 @@',
  ' export function formatUser(user: User) {',
  '-  return user.name.toUpperCase();',
  '+  return `${user.name} <${user.email}>`; // 0O 1lI',
  ' }',
  '',
].join('\n');

export interface FontDiffLine { id: string; tone: string; number: string; segments: Segment[] }
export interface FontDiffPreview { path: string; status: string; additions: number; deletions: number; scheme: string; lines: FontDiffLine[] }

/** The sample patch as unified rows: syntax per side (each side highlighted as one text), word-alt marks on paired lines. */
function previewLines(): Omit<FontDiffPreview, 'scheme'> {
  const file = parsePatch(DIFF_PREVIEW_PATCH)[0]!;
  const lines: FontDiffLine[] = [];
  for (const hunk of file.hunks) {
    const oldSide = hunk.lines.filter(line => line.kind !== 'addition'), newSide = hunk.lines.filter(line => line.kind !== 'deletion');
    const oldTokens = lineTokens(oldSide.map(line => line.text), file.path), newTokens = lineTokens(newSide.map(line => line.text), file.path);
    const tokensOf = (line: (typeof hunk.lines)[number]) => line.kind === 'addition' ? newTokens[newSide.indexOf(line)] ?? [] : oldTokens[oldSide.indexOf(line)] ?? [];
    const plain = (text: string): Segment[] => [{ id: '0', text, mark: false, syntax: '' }];
    for (let index = 0; index < hunk.lines.length;) {
      const line = hunk.lines[index]!;
      if (line.kind === 'context') {
        lines.push({ id: `c${lines.length}`, tone: 'context', number: String(line.next), segments: overlay(plain(line.text), tokensOf(line)) });
        index++; continue;
      }
      const deletions: typeof hunk.lines = [], additions: typeof hunk.lines = [];
      while (hunk.lines[index]?.kind === 'deletion') deletions.push(hunk.lines[index++]!);
      while (hunk.lines[index]?.kind === 'addition') additions.push(hunk.lines[index++]!);
      const marks = deletions.map((deletion, at) => additions[at] ? wordAltMarks(deletion.text, additions[at]!.text) : null);
      deletions.forEach((deletion, at) => lines.push({ id: `d${lines.length}`, tone: 'deletion', number: String(deletion.old), segments: overlay(marks[at]?.[0] ?? plain(deletion.text), tokensOf(deletion)) }));
      additions.forEach((addition, at) => lines.push({ id: `a${lines.length}`, tone: 'addition', number: String(addition.next), segments: overlay(marks[at]?.[1] ?? plain(addition.text), tokensOf(addition)) }));
    }
  }
  return { path: file.path, status: file.status, additions: file.additions, deletions: file.deletions, lines };
}
let cached: Omit<FontDiffPreview, 'scheme'> | undefined;
/** CodeFontPreview's content, in the chosen diff colours (red-green or blue-orange). */
export function fontDiffPreview(diffColorScheme: string): FontDiffPreview {
  cached ??= previewLines();
  return { ...cached, scheme: diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green' };
}
