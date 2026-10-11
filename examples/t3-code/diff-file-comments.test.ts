// Line comments on the Files preview (diff-file-comments.ts): FilePreviewPanel's selection, draft,
// review-comment chip record and comments that move with their text.
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { fileComment, fileCommentLines, remapLine } from './diff-file-comments';
import { messageContext } from './composer-editor';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

const readme = '# T3\n\nIntro line.\nSecond line.\nThird line.\nOutro.\n';
const lines = (text: string) => text.split('\n').map((line, index) => ({ id: String(index + 1), number: String(index + 1), runs: [{ id: '0', text: line, syntax: '' }] }));

function harness() {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) { calls.push(obj(input)); return { ok: true, generation: 1, value: { applied: true } }; } };
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't1', projectId: 'p1' });
  return { client, native, calls };
}

describe('file preview comments', () => {
  test('a comment on lines 3-5 becomes the "README.md L3 to L5" chip with the file excerpt, and its card sits under line 5', async () => {
    const { client, native, calls } = harness();
    await fileComment(client, native, 'line', 'README.md', '3', readme);
    await fileComment(client, native, 'line-shift', 'README.md', '5', readme);
    expect(fileCommentLines(client, 'README.md', readme, lines(readme), false).filter(line => line.selected).map(line => line.line)).toEqual([3, 4, 5]);
    await fileComment(client, native, 'begin', 'README.md', '4', readme);
    const drafted = fileCommentLines(client, 'README.md', readme, lines(readme), false);
    expect(drafted.findIndex(line => line.kind === 'draft')).toBe(5);
    expect(drafted[5]).toMatchObject({ label: 'L3 to L5' });
    await fileComment(client, native, 'save', 'README.md', 'Tighten this.', readme);
    const text = String(calls.at(-1)!.text);
    expect(text).toMatch(/^\[README\.md L3 to L5\]\(t3-context:\/\/v1\/review-comment\/review-comment_file-comment-[^)]+\) $/);
    client.local.drafts[client.draftKey] = text;
    expect(messageContext(client, text)).toMatchObject({ records: [{ kind: 'review-comment', label: 'README.md L3 to L5', sectionId: 'file:README.md', sectionTitle: 'File comment',
      startIndex: 2, endIndex: 4, rangeLabel: 'L3 to L5', text: 'Tighten this.', diff: 'Intro line.\nSecond line.\nThird line.', fenceLanguage: 'md' }] });
    expect(fileCommentLines(client, 'README.md', readme, lines(readme), false).filter(line => line.kind === 'note').map(line => [line.line, line.text])).toEqual([[5, 'Tighten this.']]);
    // An edit above the range moves the comment with its text; the editor hides the cards.
    const edited = `# T3\nNew line.\n${readme.slice(5)}`;
    expect(fileCommentLines(client, 'README.md', edited, lines(edited), true).some(line => line.kind === 'note')).toBe(false);
    expect(fileCommentLines(client, 'README.md', edited, lines(edited), false).find(line => line.kind === 'note')).toMatchObject({ line: 6 });
  });
  test('remapLine keeps lines above an edit, shifts lines below it and clamps inside it', () => {
    const before = 'a\nb\nc\nd', after = 'a\nX\nY\nZ\nd';
    expect([1, 2, 3, 4].map(line => remapLine(before, after, line))).toEqual([1, 2, 3, 5]);
  });
});
