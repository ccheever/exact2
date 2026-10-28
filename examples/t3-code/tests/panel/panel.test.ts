// The right panel's tab model, the Files tree and preview, the
// Linked pull requests rows and the device wizard, against the reference's rules
// (rightPanelStore.ts, FileBrowserPanel/FilePreviewPanel, ThreadPullRequestsPanel,
// DeviceSetup.tsx). Network paths run against a recording fake of the native bridge.
import { frames } from '../../src/browser/browser-frames';
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { closeSurfaceIn, openFileIn, syncDiff, panelKey, panelState, surfaceLocal, surfaceStore, availability, panelView, workspacePath, type PanelState } from '../../src/panel/panel-surfaces';
import { draftThreadId } from '../../src/composer/draft-thread-id';
import { deviceThreadId, miniDeviceOf, selectDeviceTarget } from '../../src/device/device-session';
import { toasts } from '../../src/shell/toast';
import { treeRows, searchRows, searchMatches, crumbs, codeLines, sortEntries, filesState, editFile, pendingPaths, setMarkdownTaskChecked } from '../../src/panel/files-surface';
import { resolveChains, listLines, prsView, prCommandPayload } from '../../src/panel/linked-prs';
import { platformSetupStatus, hubStatusLabel, deviceHubReadiness, deviceLocal, deviceView, configureInput, deviceStateEvent, watchDevice, deviceReady, DEVICE_STATE_KEY } from '../../src/device/device-surface';
import { parseDelimitedPreview } from '../../file-preview-host/src/shared/delimited-preview';
import { surfaces } from '../../src/shell/shell';
import { decodeClientPrefs } from '../../src/settings/settings-core';
import { diagramPreviewAction, diagramPreviewView, noteMermaidFrame } from '../../src/timeline/timeline-mermaid';
import type { Native } from '../../src/client/protocol';
import { value } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';
import { memoryAppFiles, serveAssets } from '../support/asset-server';

const panel = (): PanelState => ({ surfaces: [], active: '', visible: false, userRevision: 0 });
type Call = { method: string; payload: Record<string, unknown>; write: boolean };
function fakeClient(over: Record<string, unknown> = {}) {
  const calls: Call[] = [];
  const replies: Record<string, (payload: Record<string, unknown>) => unknown> = {};
  const client = {
    scopes: ['filesystem:read', 'filesystem:write', 'preview:operate', 'terminal:read', 'terminal:operate'],
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, revision: 0, generation: 1, diffOpen: false, diffText: '', diffError: '', diffLoading: false,
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    config: { environment: { capabilities: { threadPullRequests: true, threadPullRequestWatch: true } } },
    local: { clientSettings: { wordWrap: true } },
    shell: { projects: [{ id: 'p1', title: 'surface-fixture', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', pullRequests: [] as unknown[] }] },
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>, write = false) => { calls.push({ method, payload, write }); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); },
      call: async (request: Record<string, unknown>) => { calls.push({ method: String(request.op), payload: request, write: false }); return { id: '1-7' }; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `cmd-${index}`),
      dispatch: async (_storage: unknown, payload: Record<string, unknown>) => { calls.push({ method: 'dispatch', payload, write: true }); return {}; },
    }),
    ...over,
  } as unknown as T3Client;
  return { client, calls, replies };
}
const native = { available: true } as unknown as Native;

describe('rightPanelStore rules', () => {
  test('closing the active tab activates its neighbour; the last tab closes the panel', () => {
    const state = panel();
    state.surfaces = ['files', 'pull-requests', 'device'].map(id => ({ id, kind: id as 'files', path: '', line: 0, reveal: 0 }));
    state.active = 'pull-requests'; state.visible = true;
    closeSurfaceIn(state, 'pull-requests');
    expect(state.active).toBe('device');
    closeSurfaceIn(state, 'device');
    expect(state.active).toBe('files');
    closeSurfaceIn(state, 'files');
    expect(state).toMatchObject({ visible: false, active: '', surfaces: [] });
  });
  test('openFile replaces the standalone explorer, keeps one tab per path and bumps its reveal', () => {
    const state = panel();
    state.surfaces = [{ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }];
    openFileIn(state, 'src/index.ts', 0);
    openFileIn(state, 'README.md/', 3);
    const again = openFileIn(state, 'src/index.ts', 0);
    expect(state.surfaces.map(entry => entry.id)).toEqual(['file:src/index.ts', 'file:README.md']);
    expect(again.reveal).toBe(2);
    expect(state.surfaces[1]!.line).toBe(3);
    expect(openFileIn(state, '.', 0).id).toBe('files');
  });
  test('chat links open workspace-relative paths at their line', () => {
    expect(workspacePath('/repo/src/a.ts:3', '/repo')).toEqual({ path: 'src/a.ts', line: 3 });
    expect(workspacePath('/repo', '/repo/')).toEqual({ path: '.', line: 0 });
    expect(workspacePath('/etc/hosts', '/repo')).toEqual({ path: '/etc/hosts', line: 0 });
    expect(workspacePath('docs/guide.md', '/repo', 4)).toEqual({ path: 'docs/guide.md', line: 4 });
    // workspaceRelativeFilePath (regression #16354): a `..` that leaves the root is a host path, not a workspace path.
    expect(workspacePath('/repo/project/../other/notes.md', '/repo/project')).toEqual({ path: '/repo/project/../other/notes.md', line: 0 });
    expect(workspacePath('/repo/project/docs/a/../b.md', '/repo/project')).toEqual({ path: 'docs/a/../b.md', line: 0 });
  });
  test('the Diff tab follows the client diff panel, which other controls open and close', () => {
    const state = panel();
    syncDiff({ diffOpen: true } as unknown as T3Client, state);
    expect(state).toMatchObject({ active: 'diff', visible: true });
    expect(state.surfaces[0]!.id).toBe('diff');
    syncDiff({ diffOpen: false } as unknown as T3Client, state);
    expect(state.visible).toBe(false);
  });
  test('the launcher enables Files with a project, Device from a thread or a draft and Linked pull requests with links', () => {
    const { client } = fakeClient();
    expect(availability(client)).toMatchObject({ files: true, device: true, pullRequests: false });
    const rows = surfaces(client);
    expect(rows.find(row => row.id === 'files')).toMatchObject({ available: true, reason: '' });
    expect(rows.find(row => row.id === 'device')).toMatchObject({ available: true });
    expect(rows.find(row => row.id === 'terminal')).toMatchObject({ available: true, reason: '' });
    expect(rows.find(row => row.id === 'pull-requests')).toMatchObject({ available: false, reason: 'No linked pull requests available.' });
    // ChatView `deviceAvailable={activeThreadRef !== null}`: a project's draft has its thread ref too.
    const draft = fakeClient({ threadId: '' }).client;
    expect(surfaces(draft).find(row => row.id === 'device')).toMatchObject({ available: true, reason: '' });
    const none = fakeClient({ threadId: '', projectId: '' }).client;
    expect(surfaces(none).find(row => row.id === 'device')).toMatchObject({ available: false, reason: 'Available from a thread.' });
  });
  test('a Terminal tab wears the terminal glyph the shell icon set draws', async () => {
    const icons = await Bun.file(appPath('shell-icons.contract')).text();
    const { client } = fakeClient();
    const state = panelState(client);
    state.surfaces = [{ id: 'terminal', kind: 'terminal', path: '', line: 0, reveal: 0 }]; state.active = 'terminal'; state.visible = false;
    const tab = (await panelView(client, null, 0)).tabs[0]!;
    expect(tab.icon).toBe('square-terminal');
    expect(icons).toContain(`name == "${tab.icon}" ?`);
  });
});

describe('Files surface', () => {
  const entries = [{ path: 'src', kind: 'directory' as const, ignored: false }, { path: 'README.md', kind: 'file' as const, ignored: false },
    { path: '.github', kind: 'directory' as const, ignored: false }, { path: 'package.json', kind: 'file' as const, ignored: false }, { path: '.gitignore', kind: 'file' as const, ignored: false }];
  test('folders first, then names without case', () => {
    expect(sortEntries(entries).map(entry => entry.path)).toEqual(['.github', 'src', '.gitignore', 'package.json', 'README.md']);
  });
  test('rows follow expansion, flatten single-folder chains and carry level guides', () => {
    const dirs = new Map([['', sortEntries(entries)], ['src', sortEntries([{ path: 'src/util', kind: 'directory', ignored: false }, { path: 'src/index.ts', kind: 'file', ignored: false }])],
      ['.github', [{ path: '.github/workflows', kind: 'directory' as const, ignored: false }]], ['.github/workflows', [{ path: '.github/workflows/ci.yml', kind: 'file' as const, ignored: false }]]]);
    const rows = treeRows(dirs, new Set(['src', '.github/workflows']), 'src/index.ts');
    expect(rows.map(row => `${row.depth}:${row.name}`)).toEqual(['0:.github/workflows', '1:ci.yml', '0:src', '1:util', '1:index.ts', '0:.gitignore', '0:package.json', '0:README.md']);
    expect(rows.find(row => row.name === 'index.ts')).toMatchObject({ selected: true, token: 'typescript', guides: [{ id: '0', left: 13.6 }] });
    expect(rows.find(row => row.name === 'src')!.expanded).toBe(true);
  });
  test('search shows the matches under their open folders', () => {
    const rows = searchRows([{ path: 'src/util/math.ts', kind: 'file', ignored: false }], '');
    expect(rows.map(row => `${row.depth}:${row.name}:${row.expanded}`)).toEqual(['0:src/util:true', '1:math.ts:false']);
    const fuzzy = [{ path: 'docs/guide.md', kind: 'file' as const, ignored: false }, { path: 'src/index.ts', kind: 'file' as const, ignored: false }];
    expect(searchMatches(fuzzy, 'Guide').map(entry => entry.path)).toEqual(['docs/guide.md']);
    expect(searchMatches(fuzzy, 'src/in').map(entry => entry.path)).toEqual(['src/index.ts']);
  });
  test('breadcrumbs, numbered lines and syntax runs', () => {
    expect(crumbs('surface-fixture', 'src/index.ts').map(crumb => [crumb.label, crumb.current])).toEqual([['surface-fixture', false], ['src', false], ['index.ts', true]]);
    const lines = codeLines('src/index.ts', 'export const PI = 3.14;\n');
    expect(lines.map(line => line.number)).toEqual(['1', '2']);
    expect(lines[0]!.runs.some(run => run.syntax === 'kw' && run.text === 'export')).toBe(true);
  });
  test('opening a file lists its folders, reads it and reveals it; the panel shows it as a tab', async () => {
    const { client, calls, replies } = fakeClient();
    replies['projects.listEntries'] = payload => ({ entries: payload.directoryPath === '' ? entries : [{ path: 'src/index.ts', kind: 'file' }, { path: 'src/util', kind: 'directory' }], truncated: false });
    replies['projects.readFile'] = () => ({ relativePath: 'src/index.ts', contents: 'export const a = 1;\n', byteLength: 20, truncated: false });
    await surfaceLocal(client, native, 'file', 'src/index.ts', '');
    expect(calls.filter(call => call.method === 'projects.listEntries').map(call => call.payload.directoryPath).sort()).toEqual(['', 'src']);
    const view = await panelView(client, native, 0);
    expect(view).toMatchObject({ open: true, kind: 'file', count: 1 });
    expect(view.tabs[0]).toMatchObject({ title: 'index.ts', fileToken: 'typescript', active: true });
    expect(view.files).toMatchObject({ path: 'src/index.ts', preview: 'code', editable: true, wrap: true, showExplorer: true });
    expect(view.files.rows.find(row => row.path === 'src/index.ts')!.selected).toBe(true);
    expect(view.files.crumbs.map(crumb => crumb.label)).toEqual(['surface-fixture', 'src', 'index.ts']);
  });
  // FileBreadcrumbs' Menu is a popover: its toggle opens a crumb's listing and closes it. Another crumb's menu can open
  // before the first one's close arrives, which then leaves the new listing alone.
  test("a crumb's popover opens its listing; a late close of another crumb's leaves it", async () => {
    const { client, replies } = fakeClient();
    replies['projects.listEntries'] = payload => ({ entries: payload.directoryPath === '' ? entries : [{ path: 'src/index.ts', kind: 'file' }, { path: 'src/util', kind: 'directory' }], truncated: false });
    replies['projects.readFile'] = () => ({ relativePath: 'src/index.ts', contents: 'export const a = 1;\n', byteLength: 20, truncated: false });
    await surfaceLocal(client, native, 'file', 'src/index.ts', '');
    const menu = async () => { const crumbMenu = (await panelView(client, native, 0)).files.crumbMenu; return [crumbMenu.open, crumbMenu.root]; };
    expect(await menu()).toEqual([false, '']);
    await surfaceLocal(client, native, 'files-crumb', 'src', 'open');
    expect(await menu()).toEqual([true, 'src']);
    await surfaceLocal(client, native, 'files-crumb', 'src', 'open');
    expect(await menu()).toEqual([true, 'src']);
    await surfaceLocal(client, native, 'files-crumb', '.', 'open');
    await surfaceLocal(client, native, 'files-crumb-close', 'src', '');
    expect(await menu()).toEqual([true, '.']);
    await surfaceLocal(client, native, 'files-crumb-close', '.', '');
    expect(await menu()).toEqual([false, '']);
  });
  // FilePreviewPanel showsRawText and canOpenInBrowser: word wrap only over source text; a page or a PDF opens in the Browser.
  test('the subheader offers word wrap only for source text and "Open file in preview browser" for a page or a PDF', async () => {
    const rpcs: { method: string; payload: Record<string, unknown> }[] = [];
    const { client, replies } = fakeClient({ available: true, origin: 'http://127.0.0.1:9',
      rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => { rpcs.push({ method, payload }); return { relativeUrl: `/api/assets/${rpcs.length}` }; } });
    replies['projects.listEntries'] = () => ({ entries: [{ path: 'docs', kind: 'directory' }], truncated: false });
    replies['projects.readFile'] = payload => ({ relativePath: payload.relativePath, contents: '%PDF-1.4 text', byteLength: 13, truncated: false });
    const subheader = async (path: string) => { await surfaceLocal(client, native, 'file', path, ''); const files = (await panelView(client, native, 0)).files; return [files.preview, files.rawText, files.openInBrowser]; };
    expect(await subheader('docs/notes.ts')).toEqual(['code', true, false]);
    expect(await subheader('docs/page.html')).toEqual(['html', false, true]);
    // The PDF opens in the viewer page at its signed URL (files-viewer.ts).
    const server = serveAssets(() => ({ body: '%PDF-1.4' }));
    memoryAppFiles();
    try { expect(await subheader('docs/guide.pdf')).toEqual(['viewer', false, true]); } finally { server.restore(); }
    expect(await subheader('docs/logo.png')).toEqual(['media', false, false]);
    // The page's source (the eye toggle) is text again: word wrap comes back, the Browser button stays.
    await surfaceLocal(client, native, 'files-render', 'docs/page.html', '');
    expect(await subheader('docs/page.html')).toEqual(['code', true, true]);
    // Without the module's web views there is no Browser (isPreviewSupportedInRuntime).
    (client as unknown as { available: boolean }).available = false;
    expect((await subheader('docs/guide.pdf'))[2]).toBe(false);
  });
  test('"Open file in preview browser" signs the workspace file for the thread and opens it in a Browser tab (openFileInPreview)', async () => {
    const live = { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    // A page and a PDF take the same path: the signed `workspace-file` URL opens in a new Browser tab.
    for (const [path, tab] of [['docs/page.html', 'tab-1'], ['docs/guide.pdf', 'tab-2']] as const) {
      const rpcs: { method: string; payload: Record<string, unknown> }[] = [], raws: Record<string, unknown>[] = [];
      const { client } = fakeClient({ available: true, origin: 'http://127.0.0.1:9', generation: 1, presentation: {},
        raw: async (_native: unknown, request: Record<string, unknown>) => { raws.push(request); return { ok: true, value: {} }; },
        rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => {
          rpcs.push({ method, payload });
          if (method === 'assets.createUrl') return { relativeUrl: `/api/assets/token/${path.split('/').pop()}` };
          if (method === 'preview.list') return { sessions: [], serverEpoch: 'epoch-1', revision: 1 };
          if (method === 'preview.open') return { threadId: 't1', tabId: tab, navStatus: { _tag: 'Loading', url: String(payload.url), title: '' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-09T00:00:00.000Z' };
          return {};
        } });
      await surfaceLocal(client, live, 'files-open-browser', path, '');
      expect(rpcs.find(call => call.method === 'assets.createUrl')?.payload).toEqual({ resource: { _tag: 'workspace-file', threadId: 't1', path: `/repo/${path}` } });
      expect(rpcs.find(call => call.method === 'preview.open')?.payload).toMatchObject({ threadId: 't1', url: `http://127.0.0.1:9/api/assets/token/${path.split('/').pop()}` });
      expect(panelState(client)).toMatchObject({ active: `browser:${tab}`, visible: true });
      expect(frames(client).entries.size).toBe(1); // its frame mounts in the window's frame layer
      expect(toasts(client)).toEqual([]);
    }
    // A refused signature is the reference's stacked toast, and no tab opens.
    const refused = fakeClient({ available: true, origin: 'http://127.0.0.1:9', rpc: async () => { throw new Error('Workspace context not found.'); } }).client;
    await surfaceLocal(refused, live, 'files-open-browser', 'docs/guide.pdf', '');
    expect(toasts(refused).at(-1)).toMatchObject({ title: 'Unable to open file in browser', description: 'Workspace context not found.' });
    expect(panelState(refused).surfaces.some(surface => surface.kind === 'browser')).toBe(false);
  });
  test('edits are written with projects.writeFile, newest contents last, and the tab is pending until confirmed', async () => {
    const { client, calls, replies } = fakeClient();
    let release: () => void = () => {};
    replies['projects.writeFile'] = () => new Promise(resolve => { release = () => resolve({ relativePath: 'a.ts' }); });
    filesState(client).reads.set('a.ts', { contents: 'x', byteLength: 1, truncated: false, error: '', notFile: false });
    const first = editFile(client, native, 'a.ts', 'xy');
    await Promise.resolve();
    await editFile(client, native, 'a.ts', 'xyz');
    expect(pendingPaths(client).has('a.ts')).toBe(true);
    release(); await Promise.resolve(); await Promise.resolve();
    release(); await first;
    const writes = calls.filter(call => call.method === 'projects.writeFile');
    expect(writes.map(call => call.payload.contents)).toEqual(['xy', 'xyz']);
    expect(writes.every(call => call.write && call.payload.cwd === '/repo')).toBe(true);
    expect(pendingPaths(client).has('a.ts')).toBe(false);
  });
  // FileMarkdownPreview is ChatMarkdown over the file, its links and images resolved
  // against the file's folder, and a task checkbox writes the file (RenderedMarkdownSurface onTaskListChange).
  test('a rendered Markdown file is the chat renderer\'s source, and a task checkbox saves the file', async () => {
    const { client, calls, replies } = fakeClient({ local: { clientSettings: decodeClientPrefs({}) } });
    const text = '# Notes\n\nSee [app.ts](../src/app.ts) and [link](https://example.com).\n\n- [ ] Write the tests\n- [x] Ship the fix\n\n```ts\nconst x = 1;\n```\n';
    replies['projects.listEntries'] = payload => ({ entries: payload.directoryPath === '' ? [{ path: 'docs', kind: 'directory' }] : [{ path: 'docs/notes.md', kind: 'file' }], truncated: false });
    replies['projects.readFile'] = () => ({ relativePath: 'docs/notes.md', contents: text, byteLength: text.length, truncated: false });
    replies['projects.writeFile'] = payload => ({ relativePath: payload.relativePath });
    await surfaceLocal(client, native, 'file', 'docs/notes.md', '');
    let files = (await panelView(client, native, 0)).files;
    if (files.preview !== 'markdown') { await surfaceLocal(client, native, 'files-render', 'docs/notes.md', ''); files = (await panelView(client, native, 0)).files; }
    expect(files.preview).toBe('markdown');
    expect(files.markdownSource).toEqual([{ id: 'file:docs/notes.md', kind: 'file', title: '', body: text }]);
    expect(files.md.chips.map(chip => [chip.href, chip.tip])).toEqual([['t3-file:../src/app.ts', '/repo/src/app.ts']]);
    expect(files.code.map(block => block.code)).toEqual(['const x = 1;']);
    expect(files.code[0]!.icon).not.toBe('');
    const offset = text.indexOf('[ ] Write');
    await surfaceLocal(client, native, 'files-task', '3', `${offset}\ttrue`);
    const writes = () => calls.filter(call => call.method === 'projects.writeFile');
    expect(writes().map(call => call.payload)).toEqual([{ cwd: '/repo', relativePath: 'docs/notes.md', contents: text.replace('- [ ] Write', '- [x] Write') }]);
    // The next render reads the saved contents; an offset that no longer holds a marker writes nothing.
    expect((await panelView(client, native, 0)).files.markdownSource[0]!.body).toContain('- [x] Write the tests');
    await surfaceLocal(client, native, 'files-task', '3', `${offset + 1}\tfalse`);
    expect(writes()).toHaveLength(1);
    expect(setMarkdownTaskChecked('- [X] a', 2, false)).toBe('- [ ] a');
    expect(setMarkdownTaskChecked('- (x) a', 2, false)).toBe('- (x) a');
  });
  // FileMarkdownPreview is ChatMarkdown: a settled ```mermaid fence draws as a diagram (MarkdownMermaidCodeBlock), and
  // its expand opens the diagram preview as the transcript's does.
  test('a rendered Markdown file draws its Mermaid fences as diagrams', async () => {
    const { client, calls, replies } = fakeClient({ origin: 'http://127.0.0.1:1', local: { clientSettings: decodeClientPrefs({}) } });
    const flow = 'graph LR\n  Files --> Preview', text = `# Flow\n\n\`\`\`mermaid\n${flow}\n\`\`\`\n`;
    replies['projects.listEntries'] = payload => ({ entries: payload.directoryPath === '' ? [{ path: 'docs', kind: 'directory' }] : [{ path: 'docs/flow.md', kind: 'file' }], truncated: false });
    replies['projects.readFile'] = () => ({ relativePath: 'docs/flow.md', contents: text, byteLength: text.length, truncated: false });
    await surfaceLocal(client, native, 'file', 'docs/flow.md', '');
    let files = (await panelView(client, native, 0)).files;
    if (files.preview !== 'markdown') { await surfaceLocal(client, native, 'files-render', 'docs/flow.md', ''); files = (await panelView(client, native, 0)).files; }
    expect(files.diagrams.map(diagram => [diagram.code, diagram.light.diagram, diagram.dark.diagram])).toEqual([[flow, 'loading', 'loading']]);
    expect(calls.some(call => call.method === 'mermaidRender')).toBe(false); // the page renders it, no native module
    // The light page reports its render; the next view carries the diagram, and its expand opens it.
    noteMermaidFrame(JSON.stringify({ status: 'rendered', width: 120, height: 80, theme: 'light', source: flow, view: 'file' }));
    files = (await panelView(client, native, 0)).files;
    expect(files.diagrams[0]).toMatchObject({ code: flow, light: { diagram: 'rendered', width: 120, height: 80 } });
    diagramPreviewAction(client, 'diagram-open', flow);
    expect(diagramPreviewView(client).diagramPreview.map(diagram => diagram.code)).toEqual([flow]);
  });
  test("CSV keeps quoted cells (the viewer page's parseDelimitedPreview)", () => {
    expect(parseDelimitedPreview('name,note\n"a, b","say ""hi"""\n', ',').rows).toEqual([['name', 'note'], ['a, b', 'say "hi"']]);
    // The large-text viewer's limits: a page of rows, so many columns, so long a cell.
    expect(parseDelimitedPreview('a,b,c\n1,2,3\n', ',', { maxRows: 1, maxColumns: 2, maxCellLength: 1 })).toEqual({ rows: [['a', 'b']], truncated: true });
  });
});

describe('Linked pull requests', () => {
  const link = (number: number, snapshot: Record<string, unknown> | null, extra: Record<string, unknown> = {}) => ({ host: 'github.com', repository: 'o/r', number, url: `https://github.com/o/r/pull/${number}`, source: 'manual', linkedAt: '2026-10-04T00:00:00.000Z', snapshot, stack: null, ...extra });
  test('stacks chain by base branch and list newest first, bottom layer first', () => {
    const base = link(1, { title: 'Base', state: 'open', headBranch: 'a', baseBranch: 'main', updatedAt: '2026-10-04T01:00:00.000Z' });
    const top = link(2, { title: 'Top', state: 'open', headBranch: 'b', baseBranch: 'a', updatedAt: '2026-10-04T03:00:00.000Z' });
    const lone = link(3, null, { linkedAt: '2026-10-04T02:00:00.000Z' });
    const lines = listLines(resolveChains([lone, top, base]));
    expect(lines.map(line => [line.link.number, line.depth, line.stack?.size ?? 0])).toEqual([[1, 0, 2], [2, 1, 0], [3, 0, 0]]);
  });
  // threadPullRequests.test.ts at 74541bab17: stale saved layer lists.
  const nativeStack = (id: string, numbers: number[]) => ({ kind: 'native', id, number: Number(id), url: `https://github.com/o/r/stacks/${id}`, base: 'main', layers: numbers.map(number => ({ number, headBranch: `pr-${number}`, state: 'open' })) });
  test('uses a later complete native snapshot for layer order and the current pull request', async () => {
    const links = [18, 20, 83, 84].map(number => link(number, { state: 'open', headBranch: `pr-${number}`, baseBranch: 'main' }, { linkedAt: '2026-01-01T00:00:00.000Z',
      stack: nativeStack('80', number < 83 ? [79, 37, 38, 39, 76, 18, 20] : [79, 37, 38, 39, 76, 18, 20, 83, 84]) }));
    expect(resolveChains(links).map(chain => chain.layers.map(layer => layer.number))).toEqual([[18, 20, 83, 84]]);
    const { resolveThreadCurrentPullRequestLink } = await import('../../src/sidebar/thread-reference');
    expect(resolveThreadCurrentPullRequestLink(links)?.number).toBe(84);
  });
  test('uses the earliest longest partial snapshot and appends missing members in stored order', () => {
    const links = [[3, 2], [9, 2, 3], [9, 3, 2], [9, 3, 2]].map((numbers, index) => link(index + 1, null, { stack: nativeStack('1', numbers) }));
    expect(resolveChains(links).map(chain => chain.layers.map(layer => layer.number))).toEqual([[2, 3, 1, 4]]);
  });
  test('siblings stay under their parent, and deeper layers precede a later shallow sibling for the current PR', async () => {
    const make = (number: number, headBranch: string, baseBranch: string) => link(number, { state: 'open', headBranch, baseBranch });
    const links = [make(1, 'base', 'main'), make(2, 'first', 'base'), make(3, 'deep', 'first'), make(4, 'sibling', 'base')];
    const chains = resolveChains(links);
    expect(chains).toHaveLength(1);
    expect(listLines(chains).map(row => [row.link.number, row.depth, row.stack?.size ?? 0])).toEqual([[1, 0, 4], [2, 1, 0], [3, 2, 0], [4, 1, 0]]);
    const { resolveThreadCurrentPullRequestLink } = await import('../../src/sidebar/thread-reference');
    expect(resolveThreadCurrentPullRequestLink(links)?.number).toBe(3);
    expect(resolveThreadCurrentPullRequestLink(links.map(entry => ({ ...entry, snapshot: { ...entry.snapshot, state: 'merged' } })))?.number).toBe(3);
    const cyclic = [make(5, 'a', 'b'), make(6, 'b', 'a')];
    expect(listLines(resolveChains(cyclic)).map(row => row.depth)).toEqual([0, 0]);
  });
  test('a stack\'s first row names its host from the URL, or its term for a chain by base branch (ThreadPullRequestsPanel)', () => {
    const { client } = fakeClient();
    const native = (url: (number: number) => string) => [1, 2].map(number => link(number, { state: 'open', headBranch: `l${number}`, baseBranch: 'main' },
      { url: url(number), stack: nativeStack('9', [1, 2]) }));
    const labels = (links: unknown[]) => { (client.shell.threads[0] as { pullRequests: unknown[] }).pullRequests = links; return prsView(client, 0).rows.map(row => row.stackLabel); };
    expect(labels(native(number => `https://github.com/o/r/pull/${number}`))[0]).toBe('GitHub stack of 2: merging a layer lands the ones below it.');
    // GitHub Enterprise Server: GitHub's route on another hostname; GitCafe's own hosts are GitCafe's.
    expect(labels(native(number => `https://git.corp.example/o/r/pull/${number}`))[0]).toBe('GitHub stack of 2: merging a layer lands the ones below it.');
    expect(labels(native(number => `https://git.cafe/o/r/pull/${number}`))[0]).toBe('GitCafe stack of 2: merging a layer lands the ones below it.');
    expect(labels(native(number => `https://example.com/o/r/changes/${number}`))[0]).toBe('the host stack of 2: merging a layer lands the ones below it.');
    const chained = (url: (number: number) => string) => [link(1, { state: 'open', headBranch: 'a', baseBranch: 'main' }, { url: url(1) }), link(2, { state: 'open', headBranch: 'b', baseBranch: 'a' }, { url: url(2) })];
    expect(labels(chained(number => `https://gitlab.com/o/r/-/merge_requests/${number}`))).toEqual(['2 merge requests chained by base branch.', '']);
    expect(labels(chained(number => `https://example.com/o/r/changes/${number}`))[0]).toBe('2 change requests chained by base branch.');
  });
  test('a row\'s quick actions are its host\'s, by the registry; a link no host claims has none (ThreadPullRequestsPanel linkHost)', () => {
    const { client } = fakeClient();
    const speed = (url: string, snapshot: Record<string, unknown>) => {
      (client.shell.threads[0] as { pullRequests: unknown[] }).pullRequests = [link(9, { headBranch: 'h', baseBranch: 'main', ...snapshot }, { url })];
      return prsView(client, 0).rows[0]!.speed.map(action => action.action);
    };
    expect(speed('https://github.com/o/r/pull/9', { state: 'open' })).toEqual(['close', 'merge']);
    // GitHub's route alone does not make an unknown host GitHub.
    expect(speed('https://example.com/o/r/changes/9', { state: 'open' })).toEqual([]);
    // GitCafe's staging host claims `/pulls/` before Forgejo, and GitCafe offers Ready on a draft.
    expect(speed('https://staging.git.cafe/o/r/pulls/3', { state: 'open', isDraft: true })).toEqual(['close', 'ready']);
    expect(speed('https://codeberg.org/o/r/pulls/3', { state: 'open', isDraft: true })).toEqual(['close']);
  });
  test('rows carry watch state, waiting links and the footer; watch and unlink are orchestration commands', async () => {
    const watched = link(4, { title: 'Watched', state: 'open', isDraft: false, headBranch: 'h', baseBranch: 'main', updatedAt: '2026-10-04T00:00:00.000Z', syncedAt: '2026-10-04T00:00:00.000Z', additions: 3, deletions: 1, author: { login: 'octo' } }, { watch: { since: 'x' } });
    const { client, calls } = fakeClient();
    (client.shell.threads[0] as { pullRequests: unknown[] }).pullRequests = [watched, link(5, null, { linkedAt: '2026-10-04T00:01:00.000Z' }), link(6, null, { source: 'stack-dismissed' })];
    const view = prsView(client, Date.parse('2026-10-04T00:05:00.000Z'));
    expect(view.rows.map(row => [row.number, row.watching, row.waiting])).toEqual([[5, false, true], [4, true, false]]);
    expect(view.rows[1]).toMatchObject({ additions: '+3', deletions: '-1', author: 'octo', canWatch: true, unlinkLabel: 'Unlink from thread' });
    expect(view.footer).toBe('2 open · 2 linked · synced 5m ago');
    expect(prCommandPayload('watch', 't1', watched, 'c', false)).toEqual({ type: 'thread.pull-request.watch', commandId: 'c', threadId: 't1', host: 'github.com', repository: 'o/r', number: 4, watching: false });
    await surfaceLocal(client, native, 'open', 'l', '');
    expect(panelState(client)).toMatchObject({ active: 'pull-requests', visible: true });
    const { prsCommand } = await import('../../src/panel/linked-prs');
    await prsCommand(client, native, {} as never, 'watch', 'github.com/o/r#4', 'true');
    expect(calls.find(call => call.method === 'dispatch')!.payload).toMatchObject({ type: 'thread.pull-request.watch', number: 4, watching: true });
  });
});

describe('Device setup', () => {
  test('platform status and hub labels match DeviceSetup', () => {
    const state = { hostStatus: 'ready', hosts: [{ id: 'local', kind: 'local', platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: false, reason: 'Install the Android SDK.' }] }], devices: [] };
    expect(platformSetupStatus(state, 'ios').message).toBe('Xcode is installed, but no iOS Simulator is available. Install a runtime in Xcode Settings → Components.');
    expect(platformSetupStatus(state, 'android')).toEqual({ ready: false, message: 'Install the Android SDK.' });
    expect(hubStatusLabel({ hostStatus: 'ready' }, false)).toBe('Device hub is ready.');
    expect(hubStatusLabel({ hostStatus: 'installing' }, true)).toBe('Installing device hub…');
    expect(hubStatusLabel({ hostStatus: 'idle', hostStatuses: { mini: { status: 'ready' } } }, false)).toBe('Device hub is ready.');
    expect(configureInput('hub', 'false')).toEqual({ enabled: false, agentAccessEnabled: false });
    expect(configureInput('complete', '')).toEqual({ onboardingCompleted: true });
  });
  // IntegrationsSettings.test.tsx "counts a remote device host when this machine has no simulator tooling".
  const remoteOnly = () => ({
    hostStatus: 'idle', hostStatuses: { macmini: { status: 'ready' } }, sessions: [], onboardingCompleted: false, agentAccessEnabled: false,
    hosts: [
      { id: 'local', kind: 'local', label: 'This Mac', platforms: [{ platform: 'ios', available: false, reason: 'Xcode was not found.' }, { platform: 'android', available: false, reason: 'Android SDK was not found.' }] },
      { id: 'macmini', kind: 'ssh', label: 'Mac Mini', hubInstalled: true, agentDeviceInstalled: true, platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: false, reason: 'Android SDK was not found.' }] },
    ],
    devices: [{ hostId: 'macmini', id: 'iphone', platform: 'ios', name: 'iPhone 16 Pro', version: 'iOS 27.0', booted: true, physical: false }],
  });
  test('a ready remote device host is enough: readiness, the platform names it, Continue moves on', async () => {
    const state = remoteOnly();
    expect(deviceHubReadiness(state)).toEqual({ ready: true, busy: false });
    expect(platformSetupStatus(state, 'ios')).toEqual({ ready: true, message: 'Xcode and iOS Simulator are available on Mac Mini.' });
    expect(platformSetupStatus(state, 'android').ready).toBe(false);
    expect(deviceHubReadiness({ hostStatus: 'idle', hostStatuses: { macmini: { status: 'starting' } } })).toEqual({ ready: false, busy: true });
    expect(deviceHubReadiness({ hostStatus: 'starting', hostStatuses: { macmini: { status: 'ready' } } })).toEqual({ ready: true, busy: false });
    const { client, calls } = fakeClient();
    await surfaceLocal(client, native, 'open', 'm', '');
    deviceStateEvent(client, { subscriptionId: '1-9', key: DEVICE_STATE_KEY, value: state });
    const view = await panelView(client, native, 0);
    expect(view.device).toMatchObject({ canContinue: true, hubReady: true, hubStatus: 'Device hub is ready.', busy: false });
    expect(view.device.platforms[0]!.message).toBe('Xcode and iOS Simulator are available on Mac Mini.');
    // DeviceSetup probes once while it is open with the hub enabled; remote hosts report platforms only after one.
    await panelView(client, native, 0);
    expect(calls.filter(call => call.method === 'device.list').length).toBe(1);
    await deviceLocal(client, native, 'continue', '', '');
    expect((await deviceView(client, native)).step).toBe(1);
  });
  // DeviceSetup.test.ts "names an EAS host instead of local tooling when only the cloud can run iOS" (74541bab17); the
  // EAS host summary arrives as `easHosts` and its devices join the list beside the other hosts'.
  test('an EAS host names the cloud, and its simulators join the device list', async () => {
    const local = remoteOnly().hosts[0]!;
    const wire = { ...remoteOnly(), hostStatuses: { eas: { status: 'ready' } }, hosts: [local],
      easHosts: [{ id: 'eas', kind: 'eas', label: 'EAS Simulator', hubInstalled: true, agentDeviceInstalled: true, platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: false, reason: 'Not supported yet.' }] }],
      devices: [{ hostId: 'eas', id: 'eas-ios', platform: 'ios', name: 'iPhone (EAS Simulator)', version: 'Cloud', booted: false, physical: false }] };
    const { client } = fakeClient();
    await surfaceLocal(client, native, 'open', 'm', '');
    deviceStateEvent(client, { subscriptionId: '1-9', key: DEVICE_STATE_KEY, value: wire });
    const view = await deviceView(client, native);
    expect(view.platforms[0]).toEqual({ id: 'ios', platform: 'iOS', ready: true, message: 'EAS Simulator starts iOS Simulators in the cloud with EAS Simulator.' });
    expect(view.groups).toEqual([{ id: 'ios', label: 'iOS Simulators', devices: [{ id: 'eas\u0000eas-ios', name: 'iPhone (EAS Simulator)', platform: 'ios', description: 'EAS Simulator · Cloud · Stopped', action: 'Start' }] }]);
  });
  test('Device opens the wizard until onboarding is complete; the stream decides readiness', async () => {
    const { client, calls } = fakeClient();
    await surfaceLocal(client, native, 'open', 'm', '');
    expect(calls.some(call => call.method === 'subscribe' && call.payload.method === 'subscribeDeviceState')).toBe(true);
    let view = await panelView(client, native, 0);
    expect(view.deviceSetup).toBe(true);
    expect(view.device).toMatchObject({ loaded: false, step: 0, enabled: false, canContinue: false });
    deviceStateEvent(client, { subscriptionId: '1-8', key: DEVICE_STATE_KEY, value: { hosts: [], hostStatus: 'disabled', hostStatuses: {}, devices: [], sessions: [], onboardingCompleted: false, agentAccessEnabled: false } });
    view = await panelView(client, native, 0);
    expect(view.device.loaded).toBe(true);
    expect(deviceReady(client)).toBe(false);
    await surfaceLocal(client, native, 'setup-close', '', '');
    expect((await panelView(client, native, 0)).deviceSetup).toBe(false);
    await watchDevice(client, native);
    expect(calls.filter(call => call.method === 'subscribe').length).toBe(1);
  });
  test('the Device panel lists the devices each time it shows, once the state has loaded with the hub enabled (DevicePanel)', async () => {
    const { client, calls, replies } = fakeClient();
    const ready = { hosts: [{ id: 'local', label: 'This machine', kind: 'local', platforms: [{ platform: 'ios', available: true }] }], hostStatus: 'ready', hostStatuses: { local: { status: 'ready' } },
      devices: [] as unknown[], sessions: [], onboardingCompleted: true, agentAccessEnabled: false };
    replies['device.list'] = () => ({ ...ready, devices: [{ hostId: 'local', id: 'D1', name: 'Fixture iPhone', platform: 'ios', version: 'iOS 26.0', booted: true }] });
    const lists = () => calls.filter(call => call.method === 'device.list').length;
    await watchDevice(client, native);
    deviceStateEvent(client, { subscriptionId: '1-7', key: DEVICE_STATE_KEY, value: ready });
    await surfaceLocal(client, native, 'open', 'm', '');
    expect((await panelView(client, native, 0)).deviceSetup).toBe(false);
    expect(lists()).toBe(1);
    await Promise.resolve();
    // One list per stint: the panel's later answers do not ask again; the answer lists the hub's devices.
    const view = await panelView(client, native, 0);
    expect(lists()).toBe(1);
    expect(view.device.groups.map(group => group.devices.map(device => device.name))).toEqual([['Fixture iPhone']]);
    // Hidden and shown again, it lists again.
    await surfaceLocal(client, native, 'hide', '', '');
    await panelView(client, native, 0);
    await surfaceLocal(client, native, 'show', '', '');
    await panelView(client, native, 0);
    expect(lists()).toBe(2);
  });
  test('a hub that is off is not asked: the panel lists nothing while device support is disabled', async () => {
    const { client, calls } = fakeClient();
    await watchDevice(client, native);
    deviceStateEvent(client, { subscriptionId: '1-7', key: DEVICE_STATE_KEY, value: { hosts: [], hostStatus: 'disabled', hostStatuses: {}, devices: [], sessions: [], onboardingCompleted: true, agentAccessEnabled: false } });
    await surfaceLocal(client, native, 'open', 'm', '');
    await panelView(client, native, 0);
    expect(calls.some(call => call.method === 'device.list')).toBe(false);
  });
  test('M on a draft opens Device for the draft\'s own thread id, allocated as it opens', async () => {
    const { client } = fakeClient({ threadId: '', local: { clientSettings: { wordWrap: true }, composerControls: {} } });
    expect(availability(client).device).toBe(true);
    await surfaceLocal(client, native, 'open', 'm', '');
    expect(draftThreadId(client)).toBe('cmd-0');
    expect(deviceThreadId(client)).toBe('cmd-0');
    expect(surfaceStore(client).deviceSetup).toBe('env:cmd-0');
  });
  test('closing the panel floats its device only while the thread\'s session on it is live (floatDeviceSurface)', async () => {
    const { client } = fakeClient();
    const iphone = { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' };
    const devices = (sessions: unknown[]) => deviceStateEvent(client, { subscriptionId: '1-7', key: DEVICE_STATE_KEY, value: { hosts: [{ id: 'local', label: 'This machine', kind: 'local', platforms: [] }],
      hostStatus: 'ready', hostStatuses: { local: { status: 'ready' } }, devices: [], sessions, onboardingCompleted: true, agentAccessEnabled: false } });
    const closeOnDevice = async () => {
      const state = panelState(client);
      state.surfaces = [{ id: 'device', kind: 'device', path: '', line: 0, reveal: 0 }]; state.active = 'device'; state.visible = true;
      selectDeviceTarget(client, panelKey(client), iphone);
      await surfaceLocal(client, native, 'hide', '', '');
      return state.visible;
    };
    await watchDevice(client, native);
    // A session another thread holds, or one on another device, has nothing of this tab's to stream.
    devices([{ threadId: 't2', hostId: 'local', deviceId: 'IPHONE' }, { threadId: 't1', hostId: 'local', deviceId: 'PIXEL' }]);
    expect(await closeOnDevice()).toBe(false);
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    devices([{ threadId: 't1', hostId: 'local', deviceId: 'IPHONE' }]);
    expect(await closeOnDevice()).toBe(false);
    expect(miniDeviceOf(client, 't1')).toMatchObject({ hostId: 'local', deviceId: 'IPHONE' });
  });
});

// RightPanelTabs handleKeyDown, read from the Contract source as menu-keys.test.ts reads its menus; driving the app
// on macOS checks the keys themselves.
describe('the surface launcher keyboard', () => {
  const component = async (name: string) => {
    const lines = (await Bun.file(appPath('shell-panels.contract')).text()).split('\n');
    const start = lines.indexOf(`component ${name}`), end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
    return lines.slice(start, end).join('\n');
  };

  // The native launcher's key re-post is gone. The launcher focuses itself as it mounts
  // (`autofocus`, focusOnMount), and each available row declares its letter, either case, as `aria-keyshortcuts`: the host
  // presses it before any key handler wherever the focus is but an input context (Shortcut.permits), behind no modal and
  // not while the launcher is inert, once per press, by the physical key under a non-Latin layout. An unavailable row has
  // no button, so no shortcut. Type-to-focus leaves the letters to it (the menu-bar AppKit suite).
  test('each available row is its letter\'s shortcut, in either case', async () => {
    const row = await component('SurfaceRow');
    const button = row.split('\n').find(line => line.includes('button press=choose(surface.id)'))!;
    const chord = /aria-keyshortcuts=(`[^`]*`)/.exec(button)![1]!;
    for (const shortcut of ['B', 'T', 'F', 'D', 'M', 'L', 'P']) {
      expect(value(chord, { surface: { letter: shortcut.toLowerCase(), shortcut } })).toBe(`${shortcut.toLowerCase()} Shift+${shortcut}`);
    }
    // Only the available branch draws the button; `data-surface-launcher-keys` names the same letters for type-to-focus.
    expect(row.indexOf('if surface.available')).toBeLessThan(row.indexOf('aria-keyshortcuts='));
    expect(row.indexOf('aria-keyshortcuts=')).toBeLessThan(row.indexOf('    else\n'));
    expect(await component('SurfaceLauncher')).toContain('data-surface-launcher-keys=surfaces.filter((entry) => entry.available).map((entry) => entry.shortcut).join("")');
  });
  // The Browser row's profile chevron by keys (its Enter, ↑ and its list's Enter stay out of the
  // launcher) is browser-launcher-chevron.test.contract, run against the app (`agent.mjs macos --test`).
});
