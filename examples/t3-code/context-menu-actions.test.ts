// context-menu-gaps: what the Files tree row, pull request number and chat file-link menus send to
// the module's `contextMenu` op and do with the pick (FileBrowserPanel showEntryContextMenu,
// fileContextMenu.ts activate, pullRequestLinkContextMenu.ts, ChatMarkdown showFileContextMenu).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { T3Client } from './client';
import { toasts } from './toast';
import { filesTreeMenu, pullRequestLinkMenu } from './context-menu-actions';
import { filesState, markdownFileMenu } from './r4-surfaces-files';
import { resetRemoteEditorsForTests } from './remote-open';

function fixture(config: Obj = {}) {
  resetRemoteEditorsForTests();
  const owner = new T3Client();
  owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'p'; owner.threadId = 't';
  owner.shell.projects = [{ id: 'p', title: 'project', workspaceRoot: '/srv/project' }];
  owner.config = { availableEditors: ['cursor', 'vscode', 'file-manager'], shellRevealInFileManager: true, environment: { platform: { os: 'darwin' } }, ...config };
  const calls: Obj[] = [], requests: Obj[] = [];
  let picked = '', editor: Obj = { applied: true };
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = obj(input); calls.push(request);
    let value: Obj = {};
    if (request.op === 'sshHosts') value = { targets: {} };
    if (request.op === 'contextMenu') value = { clicked: picked || null };
    if (request.op === 'editorEdit') value = editor;
    if (request.op === 'terminalOpenExternal') value = { opened: true };
    return { ok: true, generation: 0, value };
  } };
  owner.restAccess = ((n: Native) => ({ call: (request: Obj) => owner.call(n, request), request: async (method: string, body: Obj) => { requests.push({ method, ...body }); return {}; } })) as unknown as T3Client['restAccess'];
  filesState(owner).dirs.set('', []);
  const menu = () => calls.find(call => call.op === 'contextMenu');
  return { owner, native, calls, requests, menu, pick: (value: string) => { picked = value; }, composer: (value: Obj) => { editor = value; } };
}

describe('Files tree row menu', () => {
  test('sends the five reference items with the Open with submenu', async () => {
    const { owner, native, menu } = fixture();
    await filesTreeMenu(owner, native, 'src/index.ts');
    expect(menu()?.items).toEqual([
      { id: 'open', label: 'Open' }, { id: 'reveal-in-folder', label: 'Reveal in Finder' },
      { id: 'open-with', label: 'Open with', children: [{ id: 'editor:cursor', label: 'Cursor' }, { id: 'editor:vscode', label: 'VS Code' }] },
      { id: 'copy-mention', label: 'Copy mention' }, { id: 'add-to-chat', label: 'Add to chat' },
    ]);
  });

  test('opens, reveals and opens with an editor through shell.openInEditor on the absolute path', async () => {
    for (const [pick, expected] of [['open', { editor: 'file-manager' }], ['reveal-in-folder', { editor: 'file-manager', reveal: true }], ['editor:vscode', { editor: 'vscode' }]] as const) {
      const { owner, native, requests, pick: choose } = fixture();
      choose(pick);
      await filesTreeMenu(owner, native, 'src/index.ts');
      expect(requests).toEqual([{ method: 'shell.openInEditor', cwd: '/srv/project/src/index.ts', ...expected }]);
    }
  });

  test('copies the mention with its toast, and adds it at the end of the prompt', async () => {
    const copy = fixture();
    copy.pick('copy-mention');
    await filesTreeMenu(copy.owner, copy.native, 'docs/read me.md');
    expect(copy.calls.find(call => call.op === 'copyText')?.text).toBe('[read me.md](docs/read%20me.md)');
    expect(toasts(copy.owner).map(toast => [toast.title, toast.description])).toEqual([['Mention copied', 'docs/read me.md']]);
    const add = fixture();
    add.pick('add-to-chat');
    await filesTreeMenu(add.owner, add.native, 'src');
    expect(add.calls.find(call => call.op === 'editorEdit')).toMatchObject({ text: '[src](src) ', pad: true });
    expect(add.calls.find(call => call.op === 'editorEdit')?.start).toBeUndefined();
    add.composer({ applied: false, reason: 'no composer' });
    await filesTreeMenu(add.owner, add.native, 'src');
    expect(toasts(add.owner).map(toast => toast.description)).toEqual(['Open a chat for this project and try again.']);
  });

  test('keeps only the mention actions when the server offers no editors', async () => {
    const { owner, native, menu } = fixture({ availableEditors: [], shellRevealInFileManager: false });
    await filesTreeMenu(owner, native, 'src/index.ts');
    expect(obj(menu()).items).toEqual([{ id: 'copy-mention', label: 'Copy mention' }, { id: 'add-to-chat', label: 'Add to chat' }]);
  });
});

describe('Pull request number menu', () => {
  test('Copy link and Open on the named host', async () => {
    const { owner, native, menu, calls, pick } = fixture();
    pick('copy-link');
    await pullRequestLinkMenu(owner, native, 'gitlab https://gitlab.com/a/b/-/merge_requests/7');
    expect(menu()?.items).toEqual([{ id: 'copy-link', label: 'Copy link' }, { id: 'open-external', label: 'Open on GitLab' }]);
    expect(calls.find(call => call.op === 'copyText')?.text).toBe('https://gitlab.com/a/b/-/merge_requests/7');
    pick('open-external');
    await pullRequestLinkMenu(owner, native, 'github https://github.com/a/b/pull/7');
    expect(calls.find(call => call.op === 'terminalOpenExternal')?.url).toBe('https://github.com/a/b/pull/7');
    expect(toasts(owner)).toEqual([]);
  });

  test('shows nothing without a link', async () => {
    const { owner, native, menu } = fixture();
    await pullRequestLinkMenu(owner, native, 'github ');
    expect(menu()).toBeUndefined();
  });
});

describe('Chat file link menu', () => {
  test('a media link previews media; open and reveal follow the preferred editor and the server platform', async () => {
    const media = fixture({ environment: { platform: { os: 'linux' } } });
    await markdownFileMenu(media.owner, media.native, '/srv/project/shots/a.png');
    expect(obj(media.menu()).items).toEqual([{ id: 'preview-media', label: 'Preview media' }, { id: 'open', label: 'Open in Cursor' },
      { id: 'reveal', label: 'Reveal in Files' }, { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' }]);
    const code = fixture({ availableEditors: ['file-manager'], shellRevealInFileManagerKind: 'file-explorer' });
    await markdownFileMenu(code.owner, code.native, '/srv/project/src/index.ts:12');
    expect(obj(code.menu()).items).toEqual([{ id: 'open', label: 'Open in editor' }, { id: 'reveal', label: 'Reveal in File Explorer' },
      { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' }]);
  });

  test('reveal strips the line and asks the file manager to reveal', async () => {
    const { owner, native, requests, pick } = fixture();
    pick('reveal');
    await markdownFileMenu(owner, native, '/srv/project/src/index.ts:12');
    expect(requests).toEqual([{ method: 'shell.openInEditor', cwd: '/srv/project/src/index.ts', editor: 'file-manager', reveal: true }]);
  });
});
