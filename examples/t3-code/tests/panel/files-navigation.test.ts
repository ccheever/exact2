import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { ClientError, reply, type Native } from '../../src/client/protocol';
import { failure } from '../../src/transport/transport-wire';
import { crumbs, filesLocal, filesState, filesView, treeRows } from '../../src/panel/files-surface';
import { fileBreadcrumbs, isListableHostFolder, joinHostPath, retainedBreadcrumbTrail, trailChildOf } from '../../src/panel/files-path';
import { navigateFolderIn, panelState, type PanelState, type Surface } from '../../src/panel/panel-surfaces';
import { adoptRightPanels, savedPanel } from '../../src/panel/saved-panel';
import { fileIconToken, languageIconToken } from '../../src/shared/timeline-files';

const file = (path: string): Surface => ({ id: `file:${path}`, kind: 'file', path, line: 0, reveal: 0 });
const panel = (...surfaces: Surface[]): PanelState => ({ surfaces, active: surfaces[0]?.id ?? '', visible: true, userRevision: 0 });
const native = { available: true } as Native;
function fixture(threadId = 't1') {
  const calls: { method: string; payload: Record<string, unknown> }[] = [];
  const client = {
    environmentId: 'env', threadId, projectId: 'p1', ready: true, connection: 'connected', origin: 'http://127.0.0.1:9', presentation: {}, config: {},
    get draftKey() { return `env:${this.threadId || 'new:p1'}`; },
    local: { clientSettings: {} }, shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: threadId ? [{ id: threadId, projectId: 'p1' }] : [] },
    rpc: async (_native: Native, method: string, payload: Record<string, unknown>) => { calls.push({ method, payload }); return { relativeUrl: '/api/assets/test' }; },
    restAccess: () => ({ request: async (method: string, payload: Record<string, unknown>) => {
      calls.push({ method, payload });
      if (method === 'projects.listEntries') return { entries: payload.directoryPath ? [] : [{ path: 'src', kind: 'directory' }] };
      if (method === 'projects.readFile') throw new ClientError('read failed', 'ProjectReadFileError', false, { context: { failure: payload.relativePath === 'src' ? 'path_not_file' : 'operation_failed', resolvedPath: '/repo/missing.ts', cwd: '/repo' } });
      return {};
    } }),
  } as unknown as T3Client;
  return { client, calls };
}

test('moving up keeps a muted trail and separates folder navigation from its contents menu', () => {
  expect(retainedBreadcrumbTrail('src/lib/a.ts', 'src')).toBe('src/lib/a.ts');
  expect(retainedBreadcrumbTrail('src/lib/a.ts', 'docs')).toBe('docs');
  expect(trailChildOf('src', 'src/lib/a.ts')).toBe('lib');
  expect(crumbs('demo', 'src', 'src/lib/a.ts')).toMatchObject([
    { id: '.', current: false, navigable: true, menu: true, descendant: false },
    { id: 'src', current: true, navigable: false, menu: true, descendant: false },
    { id: 'src/lib', current: false, navigable: true, menu: true, descendant: true },
    { id: 'src/lib/a.ts', current: false, navigable: true, menu: false, descendant: true },
  ]);
});

test('a file keeps its preview while folders move in place, and an existing target keeps its state', () => {
  const source = file('src/lib/a.ts'), state = panel(source);
  navigateFolderIn(state, source.id, 'src/lib', source.path, true);
  expect(state.surfaces.map(tab => tab.id)).toEqual([source.id, 'file:src/lib']);
  navigateFolderIn(state, state.active, 'src', source.path);
  expect(state.surfaces.map(tab => tab.id)).toEqual([source.id, 'file:src']);
  const existing = { ...file('docs'), reveal: 7 };
  state.surfaces.push(existing);
  expect(navigateFolderIn(state, state.active, 'docs', 'docs/readme.md')).toBe(existing);
  expect(existing.reveal).toBe(7);
  expect(state.surfaces.map(tab => tab.id)).toEqual([source.id, 'file:docs']);
});

test('folder trails survive saved-panel migration and malformed trails do not discard a tab', () => {
  const state = panel({ ...file('src'), folderTrail: 'src/lib/a.ts' });
  const saved = savedPanel(state)!;
  const next: { rightPanels?: Record<string, typeof saved> } = {};
  adoptRightPanels(next, { rightPanels: { t1: saved, t2: { ...saved, surfaces: [{ ...saved.surfaces[0], folderTrail: 42 }] } } });
  expect(next.rightPanels?.t1?.surfaces[0]).toMatchObject({ folderTrail: 'src/lib/a.ts' });
  expect(next.rightPanels?.t2?.surfaces[0]).toMatchObject({ id: 'file:src' });
  expect(next.rightPanels?.t2?.surfaces[0]).not.toHaveProperty('folderTrail');
});

test('a breadcrumb action projects the folder browser with the original file still available below it', async () => {
  const { client } = fixture(), source = file('src/lib/a.ts'), state = filesState(client), tabs = panelState(client);
  tabs.surfaces = [source]; tabs.active = source.id; tabs.visible = true;
  state.dirs.set('', [{ path: 'src', kind: 'directory', ignored: false }]);
  state.reads.set('src', { contents: '', byteLength: 0, truncated: false, error: 'not a file', notFile: true, attempted: '', errorCwd: '' });
  await filesLocal(client, native, 'crumb-go', 'src', '');
  const folder = tabs.surfaces.find(tab => tab.id === tabs.active)!;
  expect(tabs.surfaces.map(tab => tab.id)).toEqual([source.id, 'file:src']);
  expect(await filesView(client, native, folder)).toMatchObject({ path: 'src', preview: '', subheader: true, showExplorer: true });
  expect((await filesView(client, native, folder)).crumbs.at(-1)).toMatchObject({ label: 'a.ts', descendant: true, navigable: true });
  await filesLocal(client, native, 'crumb-go', '.', '');
  expect(tabs.active).toBe('files');
  expect(tabs.surfaces.map(tab => tab.id)).toEqual([source.id, 'files']);
  expect(tabs.surfaces[1]?.folderTrail).toBe(source.path);
  await filesLocal(client, native, 'crumb-go', 'src', '');
  expect(tabs.surfaces.map(tab => tab.id)).toEqual([source.id, 'file:src']);
});

test('host breadcrumbs preserve drive and UNC roots and never offer a contents menu', () => {
  expect(fileBreadcrumbs('demo', 'C:/Users/name/file.ts').map(crumb => crumb.path)).toEqual(['C:\\', 'C:\\Users', 'C:\\Users\\name', 'C:\\Users\\name\\file.ts']);
  expect(isListableHostFolder('\\\\server')).toBe(false);
  expect(isListableHostFolder('\\\\server\\share')).toBe(true);
  expect(joinHostPath('C:\\', 'Users/a')).toBe('C:\\Users\\a');
  expect(crumbs('demo', '/Users/name/file.ts').every(crumb => !crumb.menu)).toBe(true);
});

test('a loading expanded folder decorates its row while cached and collapsed folders stay still', () => {
  const dirs = new Map([['', [{ path: 'src', kind: 'directory' as const, ignored: false }]]]);
  expect(treeRows(dirs, new Set(['src']), '', new Set(['src']))[0]).toMatchObject({ expanded: true, loading: true });
  expect(treeRows(dirs, new Set(), '', new Set(['src']))[0]?.loading).toBe(false);
  expect(treeRows(dirs, new Set(['src']), '')[0]?.loading).toBe(false);
});

test('typed file errors keep the exact attempted path through the transport and preview', async () => {
  const wire = failure({ _tag: 'Fail', error: { _tag: 'ProjectReadFileError', failure: 'operation_failed', operation: 'read-file', resolvedPath: '/repo/file ', cwd: '/repo' } });
  expect(reply({ ok: false, generation: 1, error: wire.json() }).error?.context?.resolvedPath).toBe('/repo/file ');
  const { client } = fixture();
  const view = await filesView(client, native, file('missing.ts'));
  expect(view).toMatchObject({ preview: 'error', errorPath: '/repo/missing.ts', errorCwd: '/repo', previewError: 'The file could not be accessed or read. It may be missing or inaccessible.' });
});

test('draft image, video, HTML and PDF previews mint workspace assets before a thread exists', async () => {
  for (const [path, preview] of [['photo.png', 'media'], ['movie.mp4', 'media'], ['index.html', 'html'], ['notes.pdf', 'pdf']] as const) {
    const { client, calls } = fixture('');
    const view = await filesView(client, native, file(path));
    expect(view.preview).toBe(preview);
    expect(calls.find(call => call.method === 'assets.createUrl')?.payload).toMatchObject({ resource: { _tag: 'draft-workspace-file', cwd: '/repo', path: `/repo/${path}` } });
    expect(client.threadId).toBe('');
  }
});

test('fourteen additional language icons resolve in files and fenced code', () => {
  for (const [extension, token] of Object.entries({ cs: 'csharp', csx: 'csharp', dart: 'dart', diff: 'diff', patch: 'diff', ex: 'elixir', exs: 'elixir', hs: 'haskell', lhs: 'haskell', java: 'java', kt: 'kotlin', kts: 'kotlin', lua: 'lua', php: 'php', ps1: 'powershell', psm1: 'powershell', psd1: 'powershell', r: 'r', scala: 'scala', toml: 'toml', xml: 'xml' })) expect(fileIconToken(`file.${extension}`)).toBe(token);
  for (const language of ['csharp', 'elixir', 'haskell', 'kotlin', 'powershell']) expect(languageIconToken(language)).toBe(language);
});

test('generated file art retains language logos, theme tints and the diff/XML strokes', async () => {
  const drawings = await Bun.file(new URL('../../src/shared/timeline-files.contract', import.meta.url)).text();
  const art = await Bun.file(new URL('../../assets/composer-icons.json', import.meta.url)).json();
  for (const token of ['csharp', 'dart', 'diff', 'elixir', 'haskell', 'java', 'kotlin', 'lua', 'php', 'powershell', 'r', 'scala', 'toml', 'xml']) {
    expect(drawings).toContain(`token == "${token}"`);
    expect(art.pierre[token]).toContain('<path');
    expect(art.colors[token]).toHaveLength(2);
  }
  expect(art.pierre.xml).toContain('stroke="currentColor"');
  expect(drawings).toContain('stroke-width=w0');
});
