// What the Files tree row, pull request number and chat file-link menus (Contract context popovers) show
// and do with the pick (FileBrowserPanel showEntryContextMenu, fileContextMenu.ts activate,
// pullRequestLinkContextMenu.ts, ChatMarkdown showFileContextMenu).
import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
import { appPath } from '../support/app-files';
afterEach(resetPrimary);
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import { toasts } from '../../src/shell/toast';
import { filesTreeChoice, filesTreeMenuRows, pullRequestLinkPick } from '../../src/panel/context-menu-actions';
import { openOnHostLabel } from '../../src/shared/context-menus';
import { filesState, markdownFilePick } from '../../src/panel/files-surface';
import { markdownFileMenuRows } from '../../src/timeline/file-link-menu';
import { loadSshAliases } from '../../src/connections/remote-open';
import { resetRemoteEditorsForTests } from '../../src/connections/remote-open';
import { attachmentUrls, forgetMediaPreviewUrl, imagePreviewAction, imagePreviewView } from '../../src/timeline/timeline-attachments';
import { externalOpensForTests, resetExternalOpensForTests } from '../../src/client/external-open';

function fixture(config: Obj = {}) {
  resetRemoteEditorsForTests();
  primaryAt('http://127.0.0.1:41857', 'env'); // the focused environment is this Mac's embedded server
  const owner = new T3Client();
  owner.origin = 'http://127.0.0.1:41857'; owner.generation = 0; owner.environmentId = 'env'; owner.projectId = 'p'; owner.threadId = 't';
  owner.shell.projects = [{ id: 'p', title: 'project', workspaceRoot: '/srv/project' }];
  owner.config = { availableEditors: ['cursor', 'vscode', 'file-manager'], shellRevealInFileManager: true, environment: { platform: { os: 'darwin' } }, ...config };
  const calls: Obj[] = [], requests: Obj[] = [];
  let editor: Obj = { applied: true };
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = obj(input); calls.push(request);
    let value: Obj = {};
    if (request.op === 'sshHosts') value = { targets: {} };
    if (request.op === 'editorEdit') value = editor;
    return { ok: true, generation: 0, value };
  } };
  owner.restAccess = ((n: Native) => ({ call: (request: Obj) => owner.call(n, request), request: async (method: string, body: Obj) => { requests.push({ method, ...body }); return {}; } })) as unknown as T3Client['restAccess'];
  filesState(owner).dirs.set('', []);
  // assets.createUrl for the linked media (resolveMarkdownMediaPreview); `refuse` makes the environment refuse it.
  const signed: Obj[] = [];
  let refuse = false;
  owner.connection = 'connected';
  owner.rpc = (async (_native: Native, method: string, payload: Obj) => { signed.push({ method, ...payload }); if (refuse) throw new Error('not found'); return { relativeUrl: `/api/assets/${signed.length}` }; }) as T3Client['rpc'];
  const menu = () => calls.find(call => call.op === 'contextMenu');
  return { owner, native, calls, requests, menu, signed, refuseSigning: () => { refuse = true; }, composer: (value: Obj) => { editor = value; } };
}

// The tree's one context popover (files-surface.contract FilesTreeMenu): its rows from the files view, its row's
// choice here, with no native menu.
describe('Files tree row menu', () => {
  const rows = (owner: T3Client) => filesTreeMenuRows(owner).map(row => [row.id, row.label, row.parent, row.sub]);
  test('the popover lists the five reference items with the Open with submenu', () => {
    const { owner } = fixture();
    expect(rows(owner)).toEqual([['open', 'Open', '', false], ['reveal-in-folder', 'Reveal in Finder', '', false], ['open-with', 'Open with', '', true],
      ['editor:cursor', 'Cursor', 'open-with', false], ['editor:vscode', 'VS Code', 'open-with', false], ['copy-mention', 'Copy mention', '', false], ['add-to-chat', 'Add to chat', '', false]]);
    // No workspace root: no absolute path, so the file actions go (buildFileContextMenuItems).
    owner.shell.projects = [{ id: 'p', title: 'project', workspaceRoot: '' }];
    expect(rows(owner).map(row => row[0])).toEqual(['copy-mention', 'add-to-chat']);
  });

  test('opens, reveals and opens with an editor through shell.openInEditor on the absolute path', async () => {
    for (const [pick, expected] of [['open', { editor: 'file-manager' }], ['reveal-in-folder', { editor: 'file-manager', reveal: true }], ['editor:vscode', { editor: 'vscode' }]] as const) {
      const { owner, native, requests, menu } = fixture();
      await filesTreeChoice(owner, native, 'src/index.ts', pick);
      expect(requests).toEqual([{ method: 'shell.openInEditor', cwd: '/srv/project/src/index.ts', ...expected }]);
      expect(menu()).toBeUndefined();
    }
  });

  test('copies the mention with its toast, and adds it at the end of the prompt', async () => {
    const copy = fixture();
    await filesTreeChoice(copy.owner, copy.native, 'docs/read me.md', 'copy-mention');
    expect(copy.calls.find(call => call.op === 'copyText')?.text).toBe('[read me.md](docs/read%20me.md)');
    expect(toasts(copy.owner).map(toast => [toast.title, toast.description])).toEqual([['Mention copied', 'docs/read me.md']]);
    const add = fixture();
    await filesTreeChoice(add.owner, add.native, 'src', 'add-to-chat');
    expect(add.calls.find(call => call.op === 'editorEdit')).toMatchObject({ text: '[src](src) ', pad: true });
    expect(add.calls.find(call => call.op === 'editorEdit')?.start).toBeUndefined();
    add.composer({ applied: false, reason: 'no composer' });
    await filesTreeChoice(add.owner, add.native, 'src', 'add-to-chat');
    expect(toasts(add.owner).map(toast => toast.description)).toEqual(['Open a chat for this project and try again.']);
  });

  test('keeps only the mention actions when the server offers no editors, and refuses a row it does not show', async () => {
    const { owner, native, requests } = fixture({ availableEditors: [], shellRevealInFileManager: false });
    expect(rows(owner).map(row => row[0])).toEqual(['copy-mention', 'add-to-chat']);
    await filesTreeChoice(owner, native, 'src/index.ts', 'open');
    await filesTreeChoice(owner, native, 'src/index.ts', 'editor:vscode');
    expect(requests).toEqual([]);
  });
});

// The menu is a Contract context popover (pages-prs.contract PrLinkMenu): Copy link, then
// Open on the row's host (context-menus.ts openOnHostLabel, in the row's `linkOpenLabel`); its row names the pick here.
describe('Pull request number menu', () => {
  test('Copy link and Open on the named host', async () => {
    const { owner, native, menu, calls } = fixture();
    await pullRequestLinkPick(owner, native, 'copy-link\ngitlab https://gitlab.com/a/b/-/merge_requests/7');
    expect(calls.find(call => call.op === 'copyText')?.text).toBe('https://gitlab.com/a/b/-/merge_requests/7');
    resetExternalOpensForTests();
    await pullRequestLinkPick(owner, native, 'open-external\ngithub https://github.com/a/b/pull/7');
    expect(externalOpensForTests()).toEqual(['https://github.com/a/b/pull/7']);
    expect(toasts(owner)).toEqual([]);
    expect(menu()).toBeUndefined(); // no native menu
    expect([openOnHostLabel('github'), openOnHostLabel('gitlab'), openOnHostLabel('azure-devops'), openOnHostLabel('')]).toEqual(['Open on GitHub', 'Open on GitLab', 'Open on Azure DevOps', 'Open on host']);
  });

  test('does nothing without a link or for a row it does not have', async () => {
    const { owner, native, calls } = fixture();
    resetExternalOpensForTests();
    await pullRequestLinkPick(owner, native, 'copy-link\ngithub ');
    await pullRequestLinkPick(owner, native, 'delete\ngithub https://github.com/a/b/pull/7');
    await pullRequestLinkPick(owner, native, 'github https://github.com/a/b/pull/7');
    expect(calls.filter(call => call.op === 'copyText')).toEqual([]);
    expect(externalOpensForTests()).toEqual([]);
  });

  test('the popover lists Copy link, then the row\'s Open on <host>, each picking with the row\'s link', async () => {
    const view = await Bun.file(appPath('pages-prs.contract')).text();
    const menu = view.slice(view.indexOf('component PrLinkMenu'));
    const rows = menu.split('\n').map(line => line.trim()).filter(line => line.startsWith('button ') || line.startsWith('text '));
    expect(rows[0]).toContain('press=pick(`copy-link\\n${link}`) popovertarget=menuId popovertargetaction="hide" role="menuitem"');
    expect(rows[1]).toStartWith('text "Copy link"');
    expect(rows[2]).toContain('press=pick(`open-external\\n${link}`) popovertarget=menuId popovertargetaction="hide" role="menuitem"');
    expect(rows[3]).toStartWith('text openLabel');
    const list = (await import('../support/contract-action-fixture')).contractComponent('pages-prs.contract', 'PrList');
    const state = list.state();
    list.call('linkMenuFor', state, {}, ['gitlab https://gitlab.com/a/b/-/merge_requests/7', 'Open on GitLab']);
    expect([state.linkMenuOf, state.linkMenuLabel]).toEqual(['gitlab https://gitlab.com/a/b/-/merge_requests/7', 'Open on GitLab']);
  });
});

// The chip's rows (file-link-menu.ts), which its context popover shows (link-menus.contract), and the row's pick.
describe('Chat file link menu', () => {
  test('a media link previews media; open and reveal follow the preferred editor and the server platform', async () => {
    const media = fixture({ environment: { platform: { os: 'linux' } } });
    expect(markdownFileMenuRows(media.owner, '/srv/project/shots/a.png').map(row => row.id)).toEqual(['preview-media', 'copy-relative', 'copy-full']); // the SSH aliases not read yet: no shell actions
    await loadSshAliases(media.native);
    expect(markdownFileMenuRows(media.owner, '/srv/project/shots/a.png')).toEqual([{ id: 'preview-media', label: 'Preview media' }, { id: 'open', label: 'Open in Cursor' },
      { id: 'reveal', label: 'Reveal in Files' }, { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' }]);
    const code = fixture({ availableEditors: ['file-manager'], shellRevealInFileManagerKind: 'file-explorer' });
    await loadSshAliases(code.native);
    expect(markdownFileMenuRows(code.owner, '/srv/project/src/index.ts:12')).toEqual([{ id: 'open', label: 'Open in editor' }, { id: 'reveal', label: 'Reveal in File Explorer' },
      { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' }]);
    // The pull request page has no thread to preview media in.
    expect(markdownFileMenuRows(media.owner, '/srv/project/shots/a.png', { previewMedia: false })[0]?.id).toBe('open');
    expect(media.menu()).toBeUndefined(); // no native menu
  });

  test('Preview media signs the link\'s media as a media-file of the thread, then opens the expanded media dialog with it alone', async () => {
    const { owner, native, signed } = fixture();
    await markdownFilePick(owner, native, '/srv/project/shots/a.png', 'preview-media');
    expect(signed).toEqual([{ method: 'assets.createUrl', resource: { _tag: 'media-file', path: '/srv/project/shots/a.png', threadId: 't' } }]);
    const view = imagePreviewView(owner);
    expect(view).toMatchObject({ imagePreviewId: 'media:/srv/project/shots/a.png', imagePreviewName: 'a.png', imagePreviewVideo: false, imagePreviewPrevious: false, imagePreviewNext: false, imagePreviewPosition: '' });
    expect(JSON.parse(view.imagePreviewSource)).toMatchObject({ kind: 'image', name: 'a.png', asset: { resource: { _tag: 'media-file', path: '/srv/project/shots/a.png' } }, reference: { kind: 'file', relativePath: 'shots/a.png' } });
    // The dialog's URL list carries the signed URL under the dialog's id; Retry video signs it again.
    expect((await attachmentUrls(owner, native, 0)).items.filter(item => item.id.includes('a.png')).map(item => [item.id, item.url])).toEqual([['media:/srv/project/shots/a.png', 'http://127.0.0.1:41857/api/assets/1']]);
    expect(forgetMediaPreviewUrl(owner, 'media:/srv/project/shots/a.png')).toBe(true);
    await attachmentUrls(owner, native, 0);
    expect(signed).toHaveLength(2);
    // Leaving the thread drops the preview.
    owner.threadId = 'other';
    expect(imagePreviewView(owner).imagePreviewId).toBe('');
    owner.threadId = 't';
    expect(imagePreviewView(owner).imagePreviewId).toBe('');
    const video = fixture();
    await markdownFilePick(video.owner, video.native, '/srv/project/clips/b.mp4', 'preview-media');
    expect(imagePreviewView(video.owner)).toMatchObject({ imagePreviewId: 'media:/srv/project/clips/b.mp4', imagePreviewVideo: true });
    imagePreviewAction(video.owner, 'image-close', '', '');
    expect(imagePreviewView(video.owner).imagePreviewId).toBe('');
  });

  test('a refused media opens no dialog and says Media unavailable', async () => {
    const { owner, native, refuseSigning } = fixture();
    refuseSigning();
    await markdownFilePick(owner, native, '/srv/project/shots/a.png', 'preview-media');
    expect(imagePreviewView(owner).imagePreviewId).toBe('');
    expect(toasts(owner).map(toast => [toast.title, toast.description])).toEqual([['Media unavailable', 'not found']]);
  });

  test('a page or a PDF offers Open in integrated browser after Open, where the thread\'s Browser can open, and opens it there', async () => {
    const { owner, native, signed, refuseSigning } = fixture();
    await loadSshAliases(native);
    const ids = (path: string, context = {}) => markdownFileMenuRows(owner, path, context).map(row => row.id);
    expect(ids('/srv/project/site/index.html')).toEqual(['open', 'reveal', 'copy-relative', 'copy-full']); // no preview:operate yet
    Object.assign(owner, { configLive: true, shellLive: true, threadLive: true, scopes: ['preview:operate'], available: true });
    expect(markdownFileMenuRows(owner, '/srv/project/site/index.html:3')).toEqual([{ id: 'open', label: 'Open in Cursor' }, { id: 'open-in-browser', label: 'Open in integrated browser' },
      { id: 'reveal', label: 'Reveal in Finder' }, { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' }]);
    expect(ids('/srv/project/docs/spec.PDF')).toContain('open-in-browser');
    expect(ids('/srv/project/src/index.ts')).not.toContain('open-in-browser');
    expect(ids('/srv/project/site/index.html', { previewMedia: false })).not.toContain('open-in-browser'); // the pull request page
    owner.available = false;
    expect(ids('/srv/project/site/index.html')).not.toContain('open-in-browser');
    owner.available = true;
    // The pick signs the page as a workspace file of the thread (openFileInPreview); a refusal is the reference's toast.
    refuseSigning();
    await markdownFilePick(owner, native, '/srv/project/site/index.html:3', 'open-in-browser');
    expect(signed).toEqual([{ method: 'assets.createUrl', resource: { _tag: 'workspace-file', threadId: 't', path: '/srv/project/site/index.html' } }]);
    expect(toasts(owner).map(toast => [toast.title, toast.description])).toEqual([['Unable to open file in browser', 'not found']]);
  });

  test('reveal strips the line and asks the file manager to reveal; a row the menu does not offer does nothing', async () => {
    const { owner, native, requests } = fixture();
    await markdownFilePick(owner, native, '/srv/project/src/index.ts:12', 'reveal');
    expect(requests).toEqual([{ method: 'shell.openInEditor', cwd: '/srv/project/src/index.ts', editor: 'file-manager', reveal: true }]);
    await markdownFilePick(owner, native, '/srv/project/src/index.ts:12', 'preview-media');
    await markdownFilePick(owner, native, '/srv/project/src/index.ts:12', 'delete');
    expect(requests).toHaveLength(1);
  });
});

