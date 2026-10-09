// Lane r4-surfaces: the right panel's tab model, the Files tree and preview, the
// Linked pull requests rows and the device wizard, against the reference's rules
// (rightPanelStore.ts, FileBrowserPanel/FilePreviewPanel, ThreadPullRequestsPanel,
// DeviceSetup.tsx). Network paths run against a recording fake of the native bridge.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { closeSurfaceIn, openFileIn, syncDiff, panelState, surfaceLocal, surfaceStore, availability, panelView, workspacePath, type PanelState } from './r4-surfaces-panel';
import { draftThreadId } from './r7-handoff-thread';
import { deviceThreadId } from './r6-media-device';
import { toasts } from './toast';
import { treeRows, searchRows, searchMatches, crumbs, codeLines, sortEntries, filesState, editFile, pendingPaths } from './r4-surfaces-files';
import { resolveChains, listLines, prsView, prCommandPayload } from './r4-surfaces-prs';
import { platformSetupStatus, hubStatusLabel, configureInput, deviceStateEvent, watchDevice, deviceReady, DEVICE_STATE_KEY } from './r4-surfaces-device';
import { markdownDocument, parseDelimited, inlineRuns } from './r4-surfaces-render';
import { surfaces } from './shell';
import type { Native } from './protocol';

const panel = (): PanelState => ({ surfaces: [], active: '', visible: false, userRevision: 0 });
type Call = { method: string; payload: Record<string, unknown>; write: boolean };
function fakeClient(over: Record<string, unknown> = {}) {
  const calls: Call[] = [];
  const replies: Record<string, (payload: Record<string, unknown>) => unknown> = {};
  const client = {
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
    // ChatView `deviceAvailable={activeThreadRef !== null}`: a project's draft has its thread ref too (PA-1).
    const draft = fakeClient({ threadId: '' }).client;
    expect(surfaces(draft).find(row => row.id === 'device')).toMatchObject({ available: true, reason: '' });
    const none = fakeClient({ threadId: '', projectId: '' }).client;
    expect(surfaces(none).find(row => row.id === 'device')).toMatchObject({ available: false, reason: 'Available from a thread.' });
  });
  test('a Terminal tab wears the terminal glyph the shell icon set draws (PA-10)', async () => {
    const icons = await Bun.file(new URL('./shell-icons.contract', import.meta.url)).text();
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
  // FilePreviewPanel showsRawText and canOpenInBrowser (PA-4, PA-5): word wrap only over source text; a page or a PDF opens in the Browser.
  test('the subheader offers word wrap only for source text and "Open file in preview browser" for a page or a PDF', async () => {
    const rpcs: { method: string; payload: Record<string, unknown> }[] = [];
    const { client, replies } = fakeClient({ available: true, origin: 'http://127.0.0.1:9',
      rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => { rpcs.push({ method, payload }); return { relativeUrl: `/api/assets/${rpcs.length}` }; } });
    replies['projects.listEntries'] = () => ({ entries: [{ path: 'docs', kind: 'directory' }], truncated: false });
    replies['projects.readFile'] = payload => ({ relativePath: payload.relativePath, contents: '%PDF-1.4 text', byteLength: 13, truncated: false });
    const subheader = async (path: string) => { await surfaceLocal(client, native, 'file', path, ''); const files = (await panelView(client, native, 0)).files; return [files.preview, files.rawText, files.openInBrowser]; };
    expect(await subheader('docs/notes.ts')).toEqual(['code', true, false]);
    expect(await subheader('docs/page.html')).toEqual(['html', false, true]);
    expect(await subheader('docs/guide.pdf')).toEqual(['pdf', false, true]);
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
      expect(raws.filter(request => request.op === 'browserSync').at(-1)?.tabs).toHaveLength(1);
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
  test('Markdown renders as transcript blocks; CSV keeps quoted cells', () => {
    const document = markdownDocument('d', '# Title\n\nSome `code` here.\n\n- one\n- two\n\n```ts\nconst a = 1;\n```');
    expect(document.blocks.map(block => `${block.kind}:${block.gap}`)).toEqual(['heading:0', 'paragraph:10.4', 'item:10.4', 'item:4', 'code:10.4']);
    expect(document.blocks[1]!.flow).toBe(true);
    expect(inlineRuns('see [docs](docs/guide.md) and **bold**', false).map(run => run.kind || (run.weight === 600 ? 'bold' : 'text'))).toEqual(['text', 'file', 'text', 'bold']);
    expect(parseDelimited('name,note\n"a, b","say ""hi"""\n', ',').rows).toEqual([['name', 'note'], ['a, b', 'say "hi"']]);
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
    const { prsCommand } = await import('./r4-surfaces-prs');
    await prsCommand(client, native, {} as never, 'watch', 'github.com/o/r#4', 'true');
    expect(calls.find(call => call.method === 'dispatch')!.payload).toMatchObject({ type: 'thread.pull-request.watch', number: 4, watching: true });
  });
});

describe('Device setup', () => {
  test('platform status and hub labels match DeviceSetup', () => {
    const state = { hostStatus: 'ready', hosts: [{ platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: false, reason: 'Install the Android SDK.' }] }], devices: [] };
    expect(platformSetupStatus(state, 'ios').message).toBe('Xcode is installed, but no iOS Simulator is available. Install a runtime in Xcode Settings → Components.');
    expect(platformSetupStatus(state, 'android')).toEqual({ ready: false, message: 'Install the Android SDK.' });
    expect(hubStatusLabel('ready', false)).toBe('Device hub is ready.');
    expect(hubStatusLabel('installing', true)).toBe('Installing device hub…');
    expect(configureInput('hub', 'false')).toEqual({ enabled: false, agentAccessEnabled: false });
    expect(configureInput('complete', '')).toEqual({ onboardingCompleted: true });
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
  test('M on a draft opens Device for the draft\'s own thread id, allocated as it opens (PA-1)', async () => {
    const { client } = fakeClient({ threadId: '', local: { clientSettings: { wordWrap: true }, composerControls: {} } });
    expect(availability(client).device).toBe(true);
    await surfaceLocal(client, native, 'open', 'm', '');
    expect(draftThreadId(client)).toBe('cmd-0');
    expect(deviceThreadId(client)).toBe('cmd-0');
    expect(surfaceStore(client).deviceSetup).toBe('env:cmd-0');
  });
});

// RightPanelTabs handleKeyDown (PA-2), read from the Contract source as menu-keys.test.ts reads its menus; the macOS drive
// in tasks/20261009-right-panel-launcher-and-files.md proves the keys.
describe('the surface launcher keyboard', () => {
  const component = async (name: string) => {
    const lines = (await Bun.file(new URL('./shell-panels.contract', import.meta.url)).text()).split('\n');
    const start = lines.indexOf(`component ${name}`), end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
    return lines.slice(start, end).join('\n');
  };
  test('one highlight over the available rows: the arrows move and wrap it, Enter opens it, a chord or a letter is the panel\'s', async () => {
    expect(await component('SurfacePanel')).toContain('SurfaceLauncher(surfaces=shell.surfaces, ui=ui)');
    const launcher = await component('SurfaceLauncher');
    expect(launcher).toContain('state highlight = -1');
    expect(launcher).toContain('derive ids = map(filter(surfaces, (entry) => entry.available), (entry) => entry.id)');
    expect(launcher).toContain('derive lit = length(ids) == 0 ? -1 : min(highlight, length(ids) - 1)');
    expect(launcher).toContain('if e.metaKey or e.ctrlKey or e.altKey or length(ids) == 0\n      ui("key", k)');
    expect(launcher).toContain('else if k == "ArrowDown" or k == "ArrowRight"\n      preventDefault()\n      highlight = (lit + 1) % length(ids)');
    expect(launcher).toContain('else if k == "ArrowUp" or k == "ArrowLeft"\n      preventDefault()\n      highlight = lit == -1 ? length(ids) - 1 : (lit - 1 + length(ids)) % length(ids)');
    expect(launcher).toContain('else if k == "Enter" and lit >= 0\n      preventDefault()\n      match at(ids, lit)\n        case some(id)\n          ui("choose", id)');
    expect(launcher).toContain('    else\n      ui("key", k)');
    // realinput-1010-fixes RI-1: a letter of an available row, either case, is the launcher's alone (the reference's
    // capture listener prevents and stops it), so a real key handed on by the window's monitor goes nowhere else.
    expect(launcher).toContain('derive letters = concat(map(filter(surfaces, (entry) => entry.available), (entry) => entry.shortcut), map(filter(surfaces, (entry) => entry.available), (entry) => entry.letter))');
    expect(launcher).toContain('else if includes(letters, k)\n      preventDefault()\n      stopPropagation()\n      ui("key", k)\n    else\n      ui("key", k)');
    // A pointer over an available row moves the same highlight; leaving that row clears it.
    expect(launcher).toContain('highlight = over ? indexOf(ids, id) : (highlight == indexOf(ids, id) ? -1 : highlight)');
    expect(launcher).toContain('id="surface-chooser" key=keys hatch="t3-launcher"');
    expect(launcher).toContain('highlighted=(surface.available and lit >= 0 and indexOf(ids, surface.id) == lit), tipped=(hovered == surface.id)');
    // Enter on a focused row is the row's own press (`event.target !== event.currentTarget`).
    expect(await component('SurfaceRow')).toContain('action rowKey(k: string)\n    if k == "Enter"\n      stopPropagation()');
  });
});
