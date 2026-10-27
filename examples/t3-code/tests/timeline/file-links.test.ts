import { describe, expect, test } from 'bun:test';
import { chatFileLink, markdownFileLinkPath } from '../../src/timeline/markdown-file-links';
import { workspaceRelativeFilePath } from '../../src/timeline/workspace-file-lookup';
import { messageChips } from '../../src/timeline/message-chips';

describe('chat file links (parseMarkdownFileLink at 7c3b3b3605ef)', () => {
  test('a folder link may end in a slash, names may use any script (#22188, #25802)', () => {
    expect(markdownFileLinkPath('patches/')).toBe('patches/');
    expect(markdownFileLinkPath('docs/My%20Folder/')).toBe('docs/My Folder/');
    expect(markdownFileLinkPath('pastexam/線性代數/exam_易志偉.pdf')).toBe('pastexam/線性代數/exam_易志偉.pdf');
    expect(markdownFileLinkPath('Résumé%20final.pdf')).toBe('Résumé final.pdf');
    expect(markdownFileLinkPath('資料.備份')).toBe('資料.備份');
    expect(markdownFileLinkPath('Dockerfile:8')).toBe('Dockerfile');
    for (const href of ['patches', 'notes', 'https://example.com/docs', '/chat/settings#L3', 'TODO:12']) expect(markdownFileLinkPath(href)).toBeNull();
  });
  test('a network path is no file link (a12d6ed942)', () => {
    expect(markdownFileLinkPath('\\\\attacker.test\\s\\x.ts')).toBe('\\\\attacker.test\\s\\x.ts');
    for (const href of ['\\\\attacker.test\\s\\x.ts', '/\\attacker.test\\s\\x.ts', 'file://attacker.test/s/x.ts']) expect(chatFileLink(href)).toBe(false);
    expect(chatFileLink('file:///repo/a.ts')).toBe(true);
  });
  test('only file-shaped destinations get a chip', () => {
    const chips = messageChips({ text: '[p](patches/) [t](線性代數/考試.pdf) [n](notes) [a](src/a.ts:3)' }, '/repo', []);
    expect(chips.map(chip => chip.href)).toEqual(['t3-file:patches/', 't3-file:線性代數/考試.pdf', 't3-file:src/a.ts:3']);
  });
});

describe('workspaceRelativeFilePath (#22853)', () => {
  test('a `..` that leaves the root is a host path, not a workspace path', () => {
    expect(workspaceRelativeFilePath('/repo/project/../other/notes.md', '/repo/project')).toBeNull();
    expect(workspaceRelativeFilePath('C:\\repo\\project\\src\\..\\..\\notes.md', 'C:\\repo\\project')).toBeNull();
    expect(workspaceRelativeFilePath('/repo/project/docs/a/../b.md', '/repo/project')).toBe('docs/a/../b.md');
  });
});
