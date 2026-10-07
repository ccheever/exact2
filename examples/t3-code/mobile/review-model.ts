// T3 Code365aa87982 reviewModel.ts/nativeReviewDiffAdapter.ts, over the existing parser.
// @ref llp/1107.006-review-and-files.decision.md#mobile-presentation
import { parsePatch, type DiffFileModel } from './shared/diff';
import { diffReviewLines, type ReviewLine } from './shared/diff-comments';
import { mobileCodeTokens, type ThreadCodeToken } from './thread-highlight';
import { inferReviewCommentFenceLanguage } from './shared/diff-comments';

export interface ReviewRow {
  id: string; kind: string; path: string; filePath: string; text: string; title: string; detail: string; action: string;
  oldNumber: string; newNumber: string; lineIndex: number; change: string; selected: boolean;
  expanded: boolean; viewed: boolean; additions: number; deletions: number; tokens: ThreadCodeToken[];
}
export interface ReviewFile {
  id: string; name: string; path: string; previousPath: string; additions: number; deletions: number;
  expanded: boolean; viewed: boolean; pending: boolean; error: boolean; notice: string;
}
export const reviewRow = (id: string, kind: string): ReviewRow => ({ id, kind, path: '', filePath: '', text: '', title: '', detail: '', action: '',
  oldNumber: '', newNumber: '', lineIndex: -1, change: '', selected: false, expanded: false, viewed: false, additions: 0, deletions: 0, tokens: [] });
const nonText = /\.(?:png|jpe?g|gif|webp|avif|ico|bmp|tiff?|heic|mp3|wav|ogg|flac|m4a|mp4|mov|avi|mkv|webm|pdf|zip|gz|tgz|bz2|7z|rar|woff2?|ttf|otf|eot|wasm|exe|dll|so|dylib)$/i;

export function reviewSuppression(file: DiffFileModel): 'non-text' | 'large' | '' {
  if (file.binary || nonText.test(file.path)) return 'non-text';
  const lines = file.hunks.flatMap(hunk => hunk.lines);
  const headers = file.hunks.map(hunk => `@@ -${hunk.oldStart},${hunk.lines.filter(line => line.kind !== 'addition').length} +${hunk.newStart},${hunk.lines.filter(line => line.kind !== 'deletion').length} @@`);
  return lines.length > 400 || lines.reduce((count, line) => count + line.text.length, 0) + headers.join('').length > 24_000 ? 'large' : '';
}
export function reviewPatch(text: string) {
  const truncated = /\n?\[truncated\]\s*$/.test(text), patch = text.replace(/\n?\[truncated\]\s*$/, '').trim();
  return { text: patch, files: parsePatch(patch), notice: truncated ? 'Diff output hit the server size cap. Showing the available excerpt.' : '',
    rawReason: truncated ? 'Diff was truncated before it could be parsed completely. Showing the raw excerpt.' : 'Unsupported diff format. Showing raw patch.' };
}
/** Highlight each complete side so multiline lexical state survives neighboring lines. */
export function reviewLineTokens(lines: ReviewLine[], path: string, dark: boolean): ThreadCodeToken[][] {
  const side = (old: boolean) => {
    const indices = lines.map((line, index) => ({ line, index })).filter(({ line }) => line.change !== (old ? 'add' : 'delete'));
    const text = indices.map(({ line }) => line.content).join('\n');
    const colored: ThreadCodeToken[][] = [[]];
    for (const token of mobileCodeTokens(text, inferReviewCommentFenceLanguage(path), dark)) {
      token.text.split('\n').forEach((part, index) => {
        if (index) colored.push([]);
        if (part) colored.at(-1)!.push({ ...token, id: `${token.id}:${index}`, text: part.replace(/\t/g, '    ') });
      });
    }
    return new Map(indices.map(({ index }, at) => [index, colored[at] ?? []]));
  };
  const old = side(true), next = side(false);
  return lines.map((line, index) => (line.change === 'delete' ? old : next).get(index) ?? []);
}
export function reviewFileRows(file: DiffFileModel, dark: boolean, selection: { start: number; end: number } | null): ReviewRow[] {
  const lines = diffReviewLines(file), tokens = reviewLineTokens(lines, file.path, dark), rows: ReviewRow[] = [];
  let at = 0;
  file.hunks.forEach((hunk, index) => {
    const oldCount = hunk.lines.filter(line => line.kind !== 'addition').length, newCount = hunk.lines.filter(line => line.kind !== 'deletion').length;
    rows.push({ ...reviewRow(`${file.path}:hunk:${index}`, 'hunk'), path: file.path, text: `@@ -${hunk.oldStart},${oldCount} +${hunk.newStart},${newCount} @@` });
    for (const line of hunk.lines) {
      rows.push({ ...reviewRow(`${file.path}:line:${at}`, 'line'), path: file.path, lineIndex: at,
        text: line.text.replace(/\t/g, '    '), oldNumber: line.old ? String(line.old) : '', newNumber: line.next ? String(line.next) : '',
        change: line.kind, tokens: tokens[at] ?? [], selected: !!selection && at >= selection.start && at <= selection.end });
      at++;
    }
  });
  return rows;
}
