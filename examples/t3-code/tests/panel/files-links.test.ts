// The Files panel, chat file links and their paths (T3 Code 74541bab17: #21258, #13904, #17132, #16955, #20950,
// #14510, #18389, #21497, #19201, #19677, #21095).
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import { ClientError, reply, type Native } from '../../src/client/protocol';
import { failure } from '../../src/transport/transport-wire';
import { toasts } from '../../src/shell/toast';
import { announcements, announcing } from '../../src/shell/announce';
import { contractComponent } from '../support/contract-action-fixture';
import { editFile, filesLocal, filesState, filesView, mediaUnavailableDescription } from '../../src/panel/files-surface';
import { markdownFileMenuRows } from '../../src/timeline/file-link-menu';
import { loadSshAliases } from '../../src/connections/remote-open';
import { crumbs } from '../../src/panel/files-surface';
import { fileBreadcrumbs, isHostPath, isListableHostFolder } from '../../src/panel/files-path';
import { openFileLink, panelState, type Surface } from '../../src/panel/panel-surfaces';
import { resetRemoteEditorsForTests } from '../../src/connections/remote-open';
import { messageChips } from '../../src/timeline/message-chips';
import { previewPdfAssetUrl } from '../../src/browser/browser-url';
import { externalOpensForTests } from '../../src/client/external-open';
import { fileLinkCopyPaths, formatFilePathForClipboard, pickStrippedWorkspaceMatch, pickWorkspaceBasenameMatch, stripRepeatedWorkspacePrefix,
  workspaceLookupPath, workspaceRelativeFilePath } from '../../src/timeline/workspace-file-lookup';

afterEach(resetPrimary);
const file = (path: string): Surface => ({ id: `file:${path}`, kind: 'file', path, line: 0, reveal: 1 });
const files: Surface = { id: 'files', kind: 'files', path: '', line: 0, reveal: 0 };

type Answer = (method: string, payload: Obj) => unknown;
/** A client on `/work/repo` (or `root`) whose server answers each request with `answer`, and the native calls it made. */
function fixture(answer: Answer = () => ({}), options: { root?: string; remote?: boolean } = {}) {
  const root = options.root ?? '/work/repo', origin = 'http://127.0.0.1:41857';
  const client = new T3Client();
  client.origin = origin; client.generation = 0; client.environmentId = 'env'; client.projectId = 'p'; client.threadId = 't';
  client.scopes = ['filesystem:read', 'orchestration:operate'];
  Object.assign(client, { connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.shell.projects = [{ id: 'p', title: 'repo', workspaceRoot: root }];
  client.shell.threads = [{ id: 't', projectId: 'p' }];
  client.config = { availableEditors: ['vscode', 'file-manager'], shellRevealInFileManager: true, shellRevealInFileManagerKind: 'finder', environment: { label: 'devbox' } };
  if (!options.remote) primaryAt(origin, 'env');
  const calls: Obj[] = [], requests: { method: string; payload: Obj }[] = [];
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => {
    const request = obj(input); calls.push(request);
    if (request.op === 'request') {
      const method = String(request.method), payload = obj(request.payload);
      requests.push({ method, payload });
      try { return { ok: true, generation: 0, value: answer(method, payload) ?? {} }; }
      catch (error) { return { ok: false, generation: 0, error: { kind: error instanceof ClientError ? error.kind : 'rpc', message: (error as Error).message, ...(error instanceof ClientError ? { context: error.context } : {}) } }; }
    }
    if (request.op === 'sshHosts') return { ok: true, generation: 0, value: { targets: options.remote ? { [origin]: { alias: 'devbox', username: 'dev' } } : {} } };
    if (request.op === 'remoteEditorsProbe') return { ok: true, generation: 0, value: { editors: ['vscode'] } };
    if (request.op === 'copyText') return { ok: true, generation: 0, value: { copied: true } };
    return { ok: true, generation: 0, value: {} };
  } };
  return { client, native, calls, requests };
}
beforeEach(() => resetRemoteEditorsForTests());

describe('home-relative host paths', () => {
  test('a ~/ path is a host path with crumbs from ~ and listable folders below it', () => {
    expect(isHostPath('~/.claude/CLAUDE.md')).toBe(true);
    expect(isHostPath('docs/~/x.md')).toBe(false);
    expect(fileBreadcrumbs('t3code', '~/.claude/CLAUDE.md')).toEqual([
      { label: '~', path: '~', kind: 'directory' },
      { label: '.claude', path: '~/.claude', kind: 'directory' },
      { label: 'CLAUDE.md', path: '~/.claude/CLAUDE.md', kind: 'file' },
    ]);
    expect(isListableHostFolder('~/.claude')).toBe(true);
    expect(crumbs('repo', '~/.claude/CLAUDE.md').map(crumb => [crumb.label, crumb.navigable, crumb.menu])).toEqual([['~', false, false], ['.claude', true, false], ['CLAUDE.md', false, false]]);
  });
  test('a ~/ link opens in the file panel as a host file, read through the server and never revealed in the tree', async () => {
    const { client, native, requests } = fixture((method, payload) => method === 'projects.readFile' ? { contents: `read ${String(payload.relativePath)}`, byteLength: 4 } : method === 'projects.listEntries' ? { entries: [] } : {});
    const tabs = panelState(client); tabs.visible = true;
    await openFileLink(client, native, '~/.claude/CLAUDE.md', '');
    const active = tabs.surfaces.find(entry => entry.id === tabs.active)!;
    expect(active).toMatchObject({ kind: 'file', path: '~/.claude/CLAUDE.md' });
    const view = await filesView(client, native, active);
    expect(view).toMatchObject({ preview: 'code', text: 'read ~/.claude/CLAUDE.md', showExplorer: false, explorerToggle: false, editable: false, absolutePath: '~/.claude/CLAUDE.md' });
    expect(view.crumbs.map(crumb => crumb.label)).toEqual(['~', '.claude', 'CLAUDE.md']);
    expect(requests.filter(request => request.method === 'projects.readFile').map(request => request.payload.relativePath)).toEqual(['~/.claude/CLAUDE.md']);
    expect(requests.filter(request => request.method === 'projects.listEntries').map(request => request.payload.directoryPath)).toEqual(['']);
  });
});

describe('a press that begins editing', () => {
  test('the view carries the press number the module heard, with the editing it began', async () => {
    const { client, native } = fixture((method) => method === 'projects.readFile' ? { contents: 'one\n', byteLength: 4 } : method === 'projects.listEntries' ? { entries: [] } : {});
    const tabs = panelState(client), readme = file('README.md');
    tabs.surfaces = [readme]; tabs.active = readme.id; tabs.visible = true;
    expect((await filesView(client, native, readme)).editPress).toBe('');
    await filesLocal(client, native, 'begin-edit', readme.path, '4');
    expect(await filesView(client, native, readme)).toMatchObject({ editing: true, editorText: 'one\n', editPress: '4' });
  });
});

describe('home-relative host files are read-only', () => {
  test('a ~/ file neither enters editing nor writes', async () => {
    const { client, native, requests } = fixture((method) => method === 'projects.readFile' ? { contents: 'x', byteLength: 1 } : method === 'projects.listEntries' ? { entries: [] } : {});
    const tabs = panelState(client), home = file('~/.claude/CLAUDE.md');
    tabs.surfaces = [home]; tabs.active = home.id; tabs.visible = true;
    await filesView(client, native, home);
    await filesLocal(client, native, 'begin-edit', home.path, '');
    expect(filesState(client).editing).toBe('');
    await editFile(client, native, '~/x', 'changed');
    await filesLocal(client, native, 'edit', home.path, 'changed');
    expect(requests.some(request => request.method === 'projects.writeFile')).toBe(false);
  });
});

describe('a failed refresh', () => {
  test('keeps the last good read, so Open in still opens a file, until the path turns out to be a folder', async () => {
    let fail: '' | 'operation_failed' | 'path_not_file' = '';
    const { client, native, calls, requests } = fixture((method) => {
      if (method === 'projects.listEntries') return { entries: [] };
      if (method !== 'projects.readFile') return {};
      if (fail) throw new ClientError('read failed', 'ProjectReadFileError', false, { context: { failure: fail } });
      return { contents: 'kept', byteLength: 4 };
    }, { remote: true });
    const tabs = panelState(client), readme = file('README.md');
    tabs.surfaces = [readme]; tabs.active = readme.id; tabs.visible = true;
    await filesView(client, native, readme);
    fail = 'operation_failed';
    await filesLocal(client, native, 'refresh', '', '');
    expect(requests.filter(request => request.method === 'projects.readFile')).toHaveLength(2);
    const view = await filesView(client, native, readme);
    expect(view).toMatchObject({ preview: 'code', text: 'kept', previewError: '' });
    await filesLocal(client, native, 'open-editor', view.absolutePath, 'vscode');
    expect(externalOpensForTests().at(-1)).toBe('vscode://vscode-remote/ssh-remote+dev%40devbox/work/repo/README.md:1:1');
    // OpenInPicker announces the primary press's open, as the ⌘O shortcut's (neither names an editor); a choice from its
    // menu says nothing.
    expect(announcing(client)).toBe(false);
    await filesLocal(client, native, 'open-editor', view.absolutePath, '');
    expect(externalOpensForTests().at(-1)).toBe('vscode://vscode-remote/ssh-remote+dev%40devbox/work/repo/README.md:1:1');
    announcements(client, 1_000);
    expect(announcements(client, 1_500).first).toBe('Opened in VS Code');
    fail = 'path_not_file';
    await filesLocal(client, native, 'refresh', '', '');
    expect(filesState(client).reads.get('README.md')).toMatchObject({ notFile: true, contents: '' });
  });
});

describe('the Files tree', () => {
  test('expand all skips ignored folders, which still open by hand', async () => {
    const listing: Record<string, Obj[]> = {
      '': [{ path: 'src', kind: 'directory' }, { path: 'node_modules', kind: 'directory', ignored: true }, { path: 'README.md', kind: 'file' }],
      src: [{ path: 'src/lib', kind: 'directory' }], 'src/lib': [{ path: 'src/lib/a.ts', kind: 'file' }],
      node_modules: [{ path: 'node_modules/left-pad', kind: 'directory' }],
    };
    const { client, native, requests } = fixture((method, payload) => method === 'projects.listEntries' ? { entries: listing[String(payload.directoryPath)] ?? [] } : {});
    await filesView(client, native, files);
    await filesLocal(client, native, 'expand-all', '', '');
    const view = await filesView(client, native, files);
    expect(requests.filter(request => request.method === 'projects.listEntries').map(request => request.payload.directoryPath)).toEqual(['', 'src', 'src/lib']);
    expect(view.rows.map(row => [row.name, row.expanded])).toEqual([['node_modules', false], ['src/lib', true], ['a.ts', false], ['README.md', false]]);
    expect(view.allExpanded).toBe(true);
    await filesLocal(client, native, 'toggle', 'node_modules', '');
    expect(requests.at(-1)).toMatchObject({ method: 'projects.listEntries', payload: { directoryPath: 'node_modules' } });
  });
  test('a search with no matches says so once it settles, and the loading line keeps the rows in place', async () => {
    let search: () => unknown = () => ({ entries: [] });
    const { client, native } = fixture((method) => method === 'projects.listEntries' ? { entries: [{ path: 'src', kind: 'directory' }] } : method === 'projects.searchEntries' ? search() : {});
    expect(await filesView(client, native, files)).toMatchObject({ noMatches: false, emptySearch: false, loadingQuiet: true });
    await filesLocal(client, native, 'search', '', 'zzz');
    expect(await filesView(client, native, files)).toMatchObject({ rows: [], noMatches: true, emptySearch: true });
    search = () => { throw new Error('Search failed.'); };
    await filesLocal(client, native, 'search', '', 'qqq');
    expect(await filesView(client, native, files)).toMatchObject({ noMatches: true, emptySearch: false, error: 'Search failed.' });
    const state = filesState(client);
    state.searching = true; state.search = { query: 'src', entries: [{ path: 'src', kind: 'directory', ignored: false }], truncated: false, error: '' }; state.query = 'src';
    expect(await filesView(client, native, files)).toMatchObject({ searching: true, noMatches: false, loadingQuiet: true });
  });
});

describe('Open in from the file preview', () => {
  test('Finder reveals a file once its read settles, and a folder is opened, not revealed', async () => {
    const { client, native, requests } = fixture((method, payload) => method === 'projects.readFile' ? (payload.relativePath === 'docs' ? (() => { throw new ClientError('not a file', 'ProjectReadFileError', false, { context: { failure: 'path_not_file' } }); })() : { contents: 'x', byteLength: 1 }) : method === 'projects.listEntries' ? { entries: [] } : {});
    const tabs = panelState(client), readme = file('README.md');
    tabs.surfaces = [readme]; tabs.active = readme.id; tabs.visible = true;
    const view = await filesView(client, native, readme);
    await filesLocal(client, native, 'open-editor', view.absolutePath, 'file-manager');
    expect(requests.at(-1)).toEqual({ method: 'shell.openInEditor', payload: { cwd: '/work/repo/README.md', editor: 'file-manager', reveal: true } });
    await filesLocal(client, native, 'open-editor', view.absolutePath, 'vscode');
    expect(requests.at(-1)).toEqual({ method: 'shell.openInEditor', payload: { cwd: '/work/repo/README.md', editor: 'vscode' } });
    const docs = file('docs'); tabs.surfaces.push(docs); tabs.active = docs.id;
    const folder = await filesView(client, native, docs);
    await filesLocal(client, native, 'open-editor', folder.absolutePath, 'file-manager');
    expect(requests.at(-1)).toEqual({ method: 'shell.openInEditor', payload: { cwd: '/work/repo/docs', editor: 'file-manager' } });
  });
  test('a remote preview file opens in VS Code as a file, with the SSH login', async () => {
    const { client, native, calls } = fixture((method) => method === 'projects.readFile' ? { contents: 'x', byteLength: 1 } : method === 'projects.listEntries' ? { entries: [] } : {}, { remote: true });
    const tabs = panelState(client), readme = file('README.md');
    tabs.surfaces = [readme]; tabs.active = readme.id; tabs.visible = true;
    const view = await filesView(client, native, readme);
    await filesLocal(client, native, 'open-editor', view.absolutePath, 'vscode');
    expect(externalOpensForTests().at(-1)).toBe('vscode://vscode-remote/ssh-remote+dev%40devbox/work/repo/README.md:1:1');
  });
});

describe('chat file links resolve the file they name', () => {
  test('repo-root-relative paths drop a subfolder project\'s repeated folders', () => {
    expect(stripRepeatedWorkspacePrefix('apps/web/src/main.ts', '/repo/apps/web')).toBe('src/main.ts');
    expect(stripRepeatedWorkspacePrefix('web/src/main.ts', '/repo/apps/web')).toBe('src/main.ts');
    expect(stripRepeatedWorkspacePrefix('src/main.ts', '/repo/apps/web')).toBeNull();
    expect(stripRepeatedWorkspacePrefix('Apps\\Web\\src\\main.ts', 'C:\\repo\\apps\\web')).toBe('src/main.ts');
    expect(stripRepeatedWorkspacePrefix('/repo/apps/web/x.ts', '/repo/apps/web')).toBeNull();
    expect(pickStrippedWorkspaceMatch('src/main.ts', '/repo/apps/web', [{ path: 'src/main.tsx', kind: 'file' }, { path: 'src/main.ts', kind: 'file' }])).toBe('src/main.ts');
    expect(pickWorkspaceBasenameMatch('readme.md', [{ path: 'docs/README.md', kind: 'file' }])).toBe('docs/README.md');
    expect(pickWorkspaceBasenameMatch('readme.md', [{ path: 'a/README.md', kind: 'file' }, { path: 'b/Readme.md', kind: 'file' }])).toBeNull();
  });
  test('a link written from the workspace root carries what it wrote; one from another folder or absolute does not', () => {
    expect(workspaceLookupPath('apps/web/src/main.ts:12', '/repo/apps/web', '/repo/apps/web')).toBe('apps/web/src/main.ts');
    expect(workspaceLookupPath('README.md', '/repo/apps/web/docs', '/repo/apps/web')).toBe('');
    expect(workspaceLookupPath('/repo/apps/web/README.md', '/repo/apps/web', '/repo/apps/web')).toBe('');
    const chips = messageChips({ text: 'See [main](apps/web/src/main.ts) and `web/src/app.ts:4`.' }, '/repo/apps/web', []);
    expect(chips.map(chip => [chip.tip, chip.target])).toEqual([['/repo/apps/web/apps/web/src/main.ts', 'apps/web/src/main.ts'], ['/repo/apps/web/web/src/app.ts:4', 'web/src/app.ts']]);
  });
  test('a press opens the stripped match only when the path as written does not exist', async () => {
    for (const [exists, expected] of [[false, 'src/main.ts'], [true, 'apps/web/src/main.ts']] as const) {
      const { client, native, requests } = fixture((method) => method === 'projects.searchEntries' ? { entries: [{ path: 'src/main.ts', kind: 'file' }] }
        : method === 'filesystem.getMetadata' ? { entries: [exists ? { kind: 'file' } : null] } : method === 'projects.listEntries' ? { entries: [] } : { contents: '', byteLength: 0 }, { root: '/repo/apps/web' });
      await openFileLink(client, native, '/repo/apps/web/apps/web/src/main.ts:12', 'apps/web/src/main.ts');
      const tabs = panelState(client);
      expect(tabs.surfaces.find(entry => entry.id === tabs.active)).toMatchObject({ path: expected, line: 12 });
      expect(requests.find(request => request.method === 'projects.searchEntries')?.payload).toEqual({ cwd: '/repo/apps/web', query: 'src/main.ts', limit: 25, kind: 'file' });
      expect(requests.find(request => request.method === 'filesystem.getMetadata')?.payload).toEqual({ paths: ['/repo/apps/web/apps/web/src/main.ts'] });
    }
  });
  test('Open in editor, reveal and the copies act on the bare name\'s match', async () => {
    for (const picked of ['open', 'reveal', 'copy-full', 'copy-relative']) {
      const { client, native, calls, requests } = fixture((method) => method === 'projects.searchEntries' ? { entries: [{ path: 'docs/guide.md', kind: 'file' }] } : {});
      // The chip's popover row, as link-menus.contract sends it: the chip's full path, then the row and the chip's lookup.
      await filesLocal(client, native, 'link-pick', '/work/repo/guide.md:3', `${picked}\nguide.md`);
      const last = requests.at(-1)!;
      if (picked === 'open') expect(last).toEqual({ method: 'shell.openInEditor', payload: { cwd: '/work/repo/docs/guide.md:3', editor: 'vscode' } });
      if (picked === 'reveal') expect(last).toEqual({ method: 'shell.openInEditor', payload: { cwd: '/work/repo/docs/guide.md', editor: 'file-manager', reveal: true } });
      if (picked.startsWith('copy')) {
        const text = picked === 'copy-full' ? '/work/repo/docs/guide.md:3' : 'docs/guide.md:3';
        expect(calls.find(call => call.op === 'copyText')?.text).toBe(text);
        expect(toasts(client).at(-1)).toMatchObject({ title: picked === 'copy-full' ? 'Full path copied' : 'Relative path copied', description: text });
      }
    }
  });
  test('the relative copy is offered only inside the thread workspace, and paths keep their host separators', async () => {
    const { client, native } = fixture();
    await loadSshAliases(native);
    expect(markdownFileMenuRows(client, '/work/elsewhere/main.ts')).toEqual([{ id: 'open', label: 'Open in VS Code' }, { id: 'reveal', label: 'Reveal in Finder' }, { id: 'copy-full', label: 'Copy full path' }]);
    expect(workspaceRelativeFilePath('/work/repo/src/main.ts', '/work/repo/')).toBe('src/main.ts');
    expect(workspaceRelativeFilePath('C:/Repo/reports/a.csv', 'c:\\repo')).toBe('reports/a.csv');
    expect(fileLinkCopyPaths('C:/repo/reports/Device Users.csv:12:3', { absolutePath: 'C:/repo/reports/Device Users.csv', matched: false }, 'C:/repo'))
      .toEqual({ fullPath: 'C:\\repo\\reports\\Device Users.csv:12:3', relativePath: 'reports\\Device Users.csv:12:3' });
    expect(fileLinkCopyPaths('/home/repo/reports/report.csv', { absolutePath: '/home/repo/reports/report.csv', matched: false }, '/home/repo'))
      .toEqual({ fullPath: '/home/repo/reports/report.csv', relativePath: 'reports/report.csv' });
    expect(formatFilePathForClipboard('\\\\server\\share\\repo/assets/clip.mp4')).toBe('\\\\server\\share\\repo\\assets\\clip.mp4');
  });
});

describe('missing media', () => {
  test('names the other machine and the path its server checked', () => {
    const wire = failure({ _tag: 'Fail', error: { _tag: 'AssetWorkspaceAssetNotFoundError', resource: { _tag: 'media-file', path: '~/clip.mp4', threadId: 't' }, checkedPath: '/home/dev/clip.mp4' } });
    const decoded = reply({ ok: false, generation: 1, error: wire.json() }).error!;
    expect(decoded).toMatchObject({ kind: 'AssetWorkspaceAssetNotFoundError', message: 'Media file was not found.', context: { resourceTag: 'media-file', resourcePath: '~/clip.mp4', checkedPath: '/home/dev/clip.mp4' } });
    const error = new ClientError(decoded.message, decoded.kind, false, { context: decoded.context });
    expect(mediaUnavailableDescription(fixture(() => ({}), { remote: true }).client, error)).toBe('The file was not found on devbox at /home/dev/clip.mp4.');
    expect(mediaUnavailableDescription(fixture(() => ({}), { remote: true }).client, new ClientError('Media file was not found.', 'AssetWorkspaceAssetNotFoundError', false, { context: { resourceTag: 'media-file', resourcePath: '/old/clip.mp4' } })))
      .toBe('The file was not found on devbox at /old/clip.mp4.');
    expect(mediaUnavailableDescription(fixture().client, error)).toBe('Media file was not found.');
  });
});

describe('PDFs in a server browser tab', () => {
  test('the environment\'s signed PDF is fetched through this client\'s route', () => {
    expect(previewPdfAssetUrl('http://127.0.0.1:3773/api/assets/token/guide.PDF#page=2', 'http://127.0.0.1:41857')).toBe('http://127.0.0.1:41857/api/assets/token/guide.PDF#page=2');
    expect(previewPdfAssetUrl('https://env.example/api/assets/token/guide.pdf', 'https://env.example')).toBe('https://env.example/api/assets/token/guide.pdf');
    expect(previewPdfAssetUrl('https://other.example/api/assets/token/guide.pdf', 'https://env.example')).toBe('https://other.example/api/assets/token/guide.pdf');
    expect(previewPdfAssetUrl('https://example.com/paper.pdf', 'https://env.example')).toBeNull();
    expect(previewPdfAssetUrl('https://env.example/api/assets/token/index.html', 'https://env.example')).toBeNull();
    expect(previewPdfAssetUrl('https://env.example/api/assets/token/guide.pdf', null)).toBeNull();
  });
});

describe('the Files Open in presses (FileHeader, FileEditorsMenu)', () => {
  // OpenInPicker: the primary button names no editor (the preferred one opens, and is announced, as ⌘O's); a menu row
  // names its editor (no announcement).
  test('the primary press names no editor; a menu row names its own', () => {
    const files = { absolutePath: '/repo/README.md', editorId: 'vscode', editorUnavailable: '', editors: [{ id: 'zed', label: 'Zed', selected: false }] };
    const local = (...args: unknown[]) => args;
    const header = contractComponent('files-surface.contract', 'FileHeader');
    expect(header.attribute('testId="file-open-editor"', 'press', header.state(), { files, local, KmItem: (item: unknown) => item }))
      .toEqual(['surface-files-open-editor', '/repo/README.md', '']);
    const menu = contractComponent('files-surface.contract', 'FileEditorsMenu');
    expect(menu.attribute('id=`file-editor-${editor.id}`', 'press', menu.state(), { files, local, editor: files.editors[0] }))
      .toEqual(['surface-files-open-editor', '/repo/README.md', 'zed']);
  });
});
