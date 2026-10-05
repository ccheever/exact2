// Lane r4-surfaces: the right panel's tab model, the Files tree and preview, the
// Linked pull requests rows and the device wizard, against the reference's rules
// (rightPanelStore.ts, FileBrowserPanel/FilePreviewPanel, ThreadPullRequestsPanel,
// DeviceSetup.tsx). Network paths run against a recording fake of the native bridge.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { closeSurfaceIn, openFileIn, syncDiff, panelState, surfaceLocal, availability, panelView, workspacePath, type PanelState } from './r4-surfaces-panel';
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
  test('the launcher enables Files with a project, Device from a thread and Linked pull requests with links', () => {
    const { client } = fakeClient();
    expect(availability(client)).toMatchObject({ files: true, device: true, pullRequests: false });
    const rows = surfaces(client);
    expect(rows.find(row => row.id === 'files')).toMatchObject({ available: true, reason: '' });
    expect(rows.find(row => row.id === 'device')).toMatchObject({ available: true });
    expect(rows.find(row => row.id === 'terminal')).toMatchObject({ available: false, reason: 'Available when a project is open.' });
    expect(rows.find(row => row.id === 'pull-requests')).toMatchObject({ available: false, reason: 'No linked pull requests available.' });
    const draft = fakeClient({ threadId: '' }).client;
    expect(surfaces(draft).find(row => row.id === 'device')).toMatchObject({ available: false, reason: 'Available from a thread.' });
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
});
