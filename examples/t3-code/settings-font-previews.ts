// Settings › Appearance font previews, adapted from T3 Code (MIT, see LICENSE-T3):
// SettingsFontPreviews.tsx. Its code sample is the diff panel's file diff rendered by
// the real pipeline (`preloadPatchFile`, unified), not a lookalike: here the same patch
// goes through this clone's patch parser and highlighter (diff.ts parsePatch,
// timeline-diff-syntax.ts lineTokens), with @pierre/diffs' default intraline emphasis
// (`lineDiffType: "word-alt"`, which the static preview keeps; the Diff panel turns it
// off). Pierre's computeLineDiffDecorations pairs each deletion with the addition at the
// same index of its change block, diffs them with jsdiff's diffWordsWithSpace and joins
// the spans with pushOrJoinSpan; both are ported below.
import { parsePatch } from './diff';
import { lineTokens, overlay, type Segment } from './timeline-diff-syntax';

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

// ── jsdiff 9.0.0 (lib/diff/word.js WordsWithSpaceDiff, lib/diff/base.js Diff), ported ──
// BSD 3-Clause License. Copyright (c) 2009-2015, Kevin Decker <kpdecker@gmail.com>
// All rights reserved.
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright notice, this list of
//    conditions and the following disclaimer in the documentation and/or other materials
//    provided with the distribution.
// 3. Neither the name of the copyright holder nor the names of its contributors may be used
//    to endorse or promote products derived from this software without specific prior
//    written permission.
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS
// OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
// COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
// EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
// GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
// AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
// NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
// ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// extendedWordChars and WordsWithSpaceDiff.tokenize: words, runs of non-newline space,
// each newline and each other character are tokens.
const WORD = 'a-zA-Z0-9_\\u{AD}\\u{C0}-\\u{D6}\\u{D8}-\\u{F6}\\u{F8}-\\u{2C6}\\u{2C8}-\\u{2D7}\\u{2DE}-\\u{2FF}\\u{1E00}-\\u{1EFF}';
const TOKEN = new RegExp(`(\\r?\\n)|[${WORD}]+|[^\\S\\n\\r]+|[^${WORD}]`, 'gu');
export const wordTokens = (text: string): string[] => text.match(TOKEN) ?? [];

export interface WordChange { value: string; added: boolean; removed: boolean }
type Component = { count: number; added: boolean; removed: boolean; previous?: Component };
type Path = { oldPos: number; last?: Component };

/** jsdiff `diffWordsWithSpace` (Diff.diffWithOptionsObj: Myers with its diagonal pruning, no options). */
export function diffWordsWithSpace(before: string, after: string): WordChange[] {
  const oldTokens = wordTokens(before), newTokens = wordTokens(after);
  const oldLen = oldTokens.length, newLen = newTokens.length;
  const extractCommon = (path: Path, diagonal: number): number => {
    let oldPos = path.oldPos, newPos = oldPos - diagonal, common = 0;
    while (newPos + 1 < newLen && oldPos + 1 < oldLen && oldTokens[oldPos + 1] === newTokens[newPos + 1]) { newPos++; oldPos++; common++; }
    if (common) path.last = { count: common, added: false, removed: false, previous: path.last };
    path.oldPos = oldPos;
    return newPos;
  };
  const addToPath = (path: Path, added: boolean, removed: boolean, oldPosInc: number): Path => {
    const last = path.last;
    return last && last.added === added && last.removed === removed
      ? { oldPos: path.oldPos + oldPosInc, last: { count: last.count + 1, added, removed, previous: last.previous } }
      : { oldPos: path.oldPos + oldPosInc, last: { count: 1, added, removed, previous: last } };
  };
  const buildValues = (last: Component | undefined): WordChange[] => {
    const components: Component[] = [];
    for (let component = last; component; component = component.previous) components.push(component);
    components.reverse();
    let newPos = 0, oldPos = 0;
    return components.map(component => {
      if (component.removed) {
        const value = oldTokens.slice(oldPos, oldPos + component.count).join('');
        oldPos += component.count;
        return { value, added: false, removed: true };
      }
      const value = newTokens.slice(newPos, newPos + component.count).join('');
      newPos += component.count;
      if (!component.added) oldPos += component.count;
      return { value, added: component.added, removed: false };
    });
  };
  const bestPath = new Map<number, Path | undefined>([[0, { oldPos: -1 }]]);
  const seed = bestPath.get(0)!;
  let newPos = extractCommon(seed, 0);
  if (seed.oldPos + 1 >= oldLen && newPos + 1 >= newLen) return buildValues(seed.last);
  let minDiagonal = -Infinity, maxDiagonal = Infinity;
  for (let editLength = 1; editLength <= newLen + oldLen; editLength++) {
    for (let diagonal = Math.max(minDiagonal, -editLength); diagonal <= Math.min(maxDiagonal, editLength); diagonal += 2) {
      const removePath = bestPath.get(diagonal - 1), addPath = bestPath.get(diagonal + 1);
      if (removePath) bestPath.set(diagonal - 1, undefined);
      let canAdd = false;
      if (addPath) { const addPathNewPos = addPath.oldPos - diagonal; canAdd = 0 <= addPathNewPos && addPathNewPos < newLen; }
      const canRemove = !!removePath && removePath.oldPos + 1 < oldLen;
      if (!canAdd && !canRemove) { bestPath.set(diagonal, undefined); continue; }
      const basePath = !canRemove || (canAdd && removePath!.oldPos < addPath!.oldPos) ? addToPath(addPath!, true, false, 0) : addToPath(removePath!, false, true, 1);
      newPos = extractCommon(basePath, diagonal);
      if (basePath.oldPos + 1 >= oldLen && newPos + 1 >= newLen) return buildValues(basePath.last);
      bestPath.set(diagonal, basePath);
      if (basePath.oldPos + 1 >= oldLen) maxDiagonal = Math.min(maxDiagonal, diagonal - 1);
      if (newPos + 1 >= newLen) minDiagonal = Math.max(minDiagonal, diagonal + 1);
    }
  }
  return [];
}

// ── @pierre/diffs 1.3.0-beta.10 (dist/utils/parseDiffDecorations.js pushOrJoinSpan and
// renderDiffWithHighlighter.js computeLineDiffDecorations), ported. Licensed under the Apache
// License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0); changed from the original:
// spans are this clone's word-mark segments instead of hast decorations. ──
type Span = [neutral: boolean, text: string];
/** pushOrJoinSpan with `enableJoin` ("word-alt"): a one-character neutral run joins the change before it. */
function pushOrJoin(spans: Span[], text: string, neutral: boolean, isLast: boolean): void {
  const last = spans[spans.length - 1];
  if (!last || isLast) { spans.push([neutral, text]); return; }
  if (neutral === last[0] || (neutral && text.length === 1 && !last[0])) { last[1] += text; return; }
  spans.push([neutral, text]);
}
/** A replaced line pair's word-alt marks (computeLineDiffDecorations): [deletion, addition] segments. */
export function wordAltMarks(before: string, after: string): [Segment[], Segment[]] {
  const changes = diffWordsWithSpace(before, after), last = changes[changes.length - 1];
  const deletion: Span[] = [], addition: Span[] = [];
  for (const change of changes) {
    const isLast = change === last;
    if (!change.added && !change.removed) { pushOrJoin(deletion, change.value, true, isLast); pushOrJoin(addition, change.value, true, isLast); }
    else pushOrJoin(change.removed ? deletion : addition, change.value, false, isLast);
  }
  const segments = (spans: Span[]): Segment[] => spans.map(([neutral, text], index) => ({ id: String(index), text, mark: !neutral, syntax: '' }));
  return [segments(deletion), segments(addition)];
}

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
