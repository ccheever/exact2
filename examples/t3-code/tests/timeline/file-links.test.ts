import { describe, expect, test } from 'bun:test';
import { chatFileLink, markdownFileLinkPath } from '../../src/timeline/markdown-file-links';
import { workspaceRelativeFilePath } from '../../src/timeline/workspace-file-lookup';
import { messageChips } from '../../src/timeline/message-chips';
import { inlineCodeFilePathCandidate } from '../../src/timeline/inline-code-files';
import { workspacePath } from '../../src/panel/panel-surfaces';
import { appText } from '../support/app-files';

/** The destinations data/src/markdown_links.rs reads too: each, and whether it is a file link. */
const cases = appText('file-link-cases.tsv').split('\n').filter(line => line && !line.startsWith('#')).map(line => line.split('\t'));

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
  test('the shared table: Rust and TypeScript call the same destinations file links', () => {
    expect(cases.length).toBeGreaterThan(20);
    for (const [href, kind] of cases) expect([href, chatFileLink(href!)]).toEqual([href, kind === 'chip']);
  });
  test('inline code needs a file-shaped last segment: a trailing slash stays plain code (6de7da548d)', () => {
    expect(inlineCodeFilePathCandidate('src/main.ts/')).toBeNull();
    expect(inlineCodeFilePathCandidate('src/main.ts')).toBe('src/main.ts');
    expect(messageChips({ text: 'See `src/main.ts/` and `src/app.ts`' }, '/repo', []).map(chip => chip.href)).toEqual(['t3-inline-code:src/app.ts']);
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
  test('opening one keeps the host path (ChatMarkdown panelPath), where a path inside opens in the workspace', () => {
    expect(workspacePath('/repo/project/../other/notes.md', '/repo/project')).toEqual({ path: '/repo/project/../other/notes.md', line: 0 });
    expect(workspacePath('/repo/project/docs/a/../b.md:3', '/repo/project')).toEqual({ path: 'docs/a/../b.md', line: 3 });
    expect(workspacePath('/repo/project', '/repo/project')).toEqual({ path: '.', line: 0 });
  });
});
