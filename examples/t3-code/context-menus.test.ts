// Ported from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3): apps/web/src/fileContextMenu.test.ts,
// components/preview/fileExplorerLabel.test.ts (the OS and kind tables) and editorLabels.test.ts
// (openInEditorMenuLabel). Changes from the reference:
// - `vite-plus/test` is `bun:test`; `it.each` tables are loops; NodeAssert.ok is an expect.
// - the reference has no tests for its pull request link menu, the file tree's mention items or
//   ChatMarkdown's file-link menu; those below follow the reference's sources.
import { describe, expect, it } from 'bun:test';

import {
  buildFileContextMenuItems, fileActionFailureTitle, fileContextMenuCapabilities, fileTreeContextMenuItems, markdownFileMenuItems, menuItemIds,
  openInEditorMenuLabel, openOnHostLabel, pullRequestLinkContextMenuItems, resolveFileContextMenuAbsolutePath, revealInFileExplorerLabelForKind,
  revealInFileExplorerLabelForOs, revealLabelFor,
} from './context-menus';

const BASE_TARGET = { environmentId: 'environment-local', filePath: 'src/index.ts', workspaceRoot: '/workspace/project' };

describe('resolveFileContextMenuAbsolutePath', () => {
  it('joins workspace-relative diff paths onto the workspace root', () => {
    expect(resolveFileContextMenuAbsolutePath(BASE_TARGET)).toBe('/workspace/project/src/index.ts');
  });

  it('strips the repository prefix when the repo root is nested in the workspace', () => {
    expect(resolveFileContextMenuAbsolutePath({ ...BASE_TARGET, workspaceRoot: '/workspace/project/packages/app', repositoryRoot: '/workspace/project', filePath: 'packages/app/src/index.ts' }))
      .toBe('/workspace/project/packages/app/src/index.ts');
  });

  it('returns null for paths outside the workspace when a repository root is set', () => {
    expect(resolveFileContextMenuAbsolutePath({ ...BASE_TARGET, workspaceRoot: '/workspace/project/packages/app', repositoryRoot: '/workspace/project', filePath: 'other/src/index.ts' })).toBeNull();
  });

  it('rejects absolute paths without a workspace root, matching diff path resolution', () => {
    expect(resolveFileContextMenuAbsolutePath({ ...BASE_TARGET, workspaceRoot: undefined, filePath: '/absolute/src/index.ts' })).toBeNull();
  });
});

describe('buildFileContextMenuItems', () => {
  it('offers open, reveal, and an open-with submenu when all are available', () => {
    const items = buildFileContextMenuItems({ hasAbsolutePath: true, capabilities: { revealLabel: 'Reveal in Finder', canOpenDefault: true, editorIds: ['vscode', 'cursor', 'file-manager'] } });
    expect(items.map(item => item.id)).toEqual(['open', 'reveal-in-folder', 'open-with']);
    expect(items[0]).toMatchObject({ label: 'Open' });
    expect(items[1]).toMatchObject({ label: 'Reveal in Finder' });
    const openWith = items[2];
    expect(openWith).toBeDefined();
    expect(openWith!.children?.map(child => child.id)).toEqual(['editor:vscode', 'editor:cursor']);
  });

  it('offers only the reveal item when just reveal is enabled', () => {
    const items = buildFileContextMenuItems({ hasAbsolutePath: true, capabilities: { revealLabel: 'Reveal in File Explorer', canOpenDefault: false, editorIds: [] } });
    expect(items.map(item => item.id)).toEqual(['reveal-in-folder']);
    expect(items[0]).toMatchObject({ label: 'Reveal in File Explorer' });
  });

  it('offers nothing when the path cannot be resolved', () => {
    expect(buildFileContextMenuItems({ hasAbsolutePath: false, capabilities: { revealLabel: 'Reveal in Finder', canOpenDefault: true, editorIds: ['vscode'] } })).toEqual([]);
  });
});

describe('revealInFileExplorerLabelForOs', () => {
  for (const [os, expected] of [['darwin', 'Reveal in Finder'], ['windows', 'Reveal in File Explorer'], ['linux', 'Reveal in Files'], ['unknown', 'Reveal in Files']] as const) {
    it(`maps ${os} to ${expected}`, () => expect(revealInFileExplorerLabelForOs(os)).toBe(expected));
  }
});

describe('revealInFileExplorerLabelForKind', () => {
  for (const [kind, expected] of [['finder', 'Reveal in Finder'], ['file-explorer', 'Reveal in File Explorer'], ['files', 'Reveal in Files']] as const) {
    it(`maps ${kind} to ${expected}`, () => expect(revealInFileExplorerLabelForKind(kind)).toBe(expected));
  }
});

describe('openInEditorMenuLabel', () => {
  it('names the preferred editor', () => {
    expect(openInEditorMenuLabel('zed')).toBe('Open in Zed');
  });

  it('keeps the generic label for the default file handler and missing preferences', () => {
    expect(openInEditorMenuLabel('file-manager')).toBe('Open in editor');
    expect(openInEditorMenuLabel(null)).toBe('Open in editor');
  });
});

// useFileContextMenu / ChatMarkdown: the reveal item and its wording come from the server's config.
describe('the server config', () => {
  const config = (extra: Record<string, unknown> = {}) => ({
    availableEditors: ['cursor', 'vscode', 'file-manager', 'not-an-editor'], shellRevealInFileManager: true,
    environment: { platform: { os: 'linux' } }, ...extra,
  });

  it('words the reveal by the server-selected kind, else by the environment OS', () => {
    expect(revealLabelFor(config({ shellRevealInFileManagerKind: 'file-explorer' }), 'env')).toBe('Reveal in File Explorer');
    expect(revealLabelFor(config(), 'env')).toBe('Reveal in Files');
    expect(revealLabelFor(config({ environment: { platform: { os: 'darwin' } } }), 'env')).toBe('Reveal in Finder');
  });

  it('offers no reveal without the server flag, the file manager or an environment', () => {
    expect(revealLabelFor(config({ shellRevealInFileManager: false }), 'env')).toBeUndefined();
    expect(revealLabelFor(config({ availableEditors: ['cursor'] }), 'env')).toBeUndefined();
    expect(revealLabelFor(config(), null)).toBeUndefined();
  });

  it('gives the Files tree its five items in the reference order', () => {
    const capabilities = fileContextMenuCapabilities(config({ environment: { platform: { os: 'darwin' } } }), 'env');
    expect(capabilities).toEqual({ revealLabel: 'Reveal in Finder', canOpenDefault: true, editorIds: ['cursor', 'vscode', 'file-manager'] });
    const items = fileTreeContextMenuItems(buildFileContextMenuItems({ hasAbsolutePath: true, capabilities }));
    expect(items.map(item => item.label)).toEqual(['Open', 'Reveal in Finder', 'Open with', 'Copy mention', 'Add to chat']);
    expect(items[2]!.children?.map(child => child.label)).toEqual(['Cursor', 'VS Code']);
    expect(items.some(item => item.separatorBefore || item.destructive || item.disabled)).toBe(false);
    expect(menuItemIds(items)).toEqual(['open', 'reveal-in-folder', 'editor:cursor', 'editor:vscode', 'copy-mention', 'add-to-chat']);
  });

  it('keeps the mention actions when nothing can open the file', () => {
    const items = fileTreeContextMenuItems(buildFileContextMenuItems({ hasAbsolutePath: true, capabilities: fileContextMenuCapabilities({ availableEditors: [] }, 'env') }));
    expect(items.map(item => item.id)).toEqual(['copy-mention', 'add-to-chat']);
  });

  it('names the failed file action', () => {
    expect(fileActionFailureTitle('open')).toBe('Could not open file');
    expect(fileActionFailureTitle('reveal-in-folder')).toBe('Unable to reveal file');
    expect(fileActionFailureTitle('editor:zed')).toBe('Could not open in Zed');
  });
});

// pullRequestLinkContextMenu.ts
describe('pull request link menu', () => {
  it('copies first, then opens on the named host', () => {
    expect(pullRequestLinkContextMenuItems(openOnHostLabel('github'))).toEqual([{ id: 'copy-link', label: 'Copy link' }, { id: 'open-external', label: 'Open on GitHub' }]);
  });

  it('names every host, and an unknown one generically', () => {
    expect(['github', 'gitlab', 'forgejo', 'bitbucket', 'azure-devops', 'gitea', ''].map(openOnHostLabel))
      .toEqual(['Open on GitHub', 'Open on GitLab', 'Open on Forgejo', 'Open on Bitbucket', 'Open on Azure DevOps', 'Open on host', 'Open on host']);
  });
});

// ChatMarkdown MarkdownFileLink showFileContextMenu (without "Open in integrated browser", X1).
describe('chat file link menu', () => {
  it('previews media, opens in the generic editor and reveals with the server wording', () => {
    expect(markdownFileMenuItems({ canPreviewMedia: true, canOpen: true, preferredEditor: null, revealLabel: 'Reveal in Files' }).map(item => item.label))
      .toEqual(['Preview media', 'Open in editor', 'Reveal in Files', 'Copy relative path', 'Copy full path']);
  });

  it('names the preferred editor, and the file manager as the generic editor', () => {
    expect(markdownFileMenuItems({ canPreviewMedia: false, canOpen: true, preferredEditor: 'cursor', revealLabel: 'Reveal in Finder' }).map(item => item.label))
      .toEqual(['Open in Cursor', 'Reveal in Finder', 'Copy relative path', 'Copy full path']);
    expect(markdownFileMenuItems({ canPreviewMedia: false, canOpen: true, preferredEditor: 'file-manager', revealLabel: undefined }).map(item => item.label))
      .toEqual(['Open in editor', 'Copy relative path', 'Copy full path']);
  });

  it('keeps only the copies (and media) without shell actions', () => {
    expect(markdownFileMenuItems({ canPreviewMedia: true, canOpen: false, preferredEditor: 'cursor', revealLabel: 'Reveal in Finder' }).map(item => item.id))
      .toEqual(['preview-media', 'copy-relative', 'copy-full']);
  });
});
