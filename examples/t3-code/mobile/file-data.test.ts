import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { arr, obj, type Obj } from './shared/domain';
import { attachmentDocumentProjection, type AttachmentDocumentCache } from './attachment-document';
import { decodeFilePreviewText } from './attachment-document-kind';
import { mobileMarkdownBlocks } from './markdown-blocks';
import type { Native } from './shared/protocol';
import { mobileFilesRead, mobileFilesAction, mobileFileRead, mobileFileSnapshot, mobileFilesSnapshot, mobileFileMenu, mobileFilePresentation } from './file-data';
import { buildFileTree, flattenFileTree } from './file-tree-model';
import { workspaceSvgSnapshot, workspaceSvgRead, workspaceSvgPrepare, workspaceSvgAction, workspaceSvgObserve, workspaceSvgConnection, type SvgVisit } from './workspace-svg';
import { mobileNative, mobileClient } from './client';
import { answer } from './app';
import { filesState } from './shared/r4-surfaces-files';

function fixture() {
  const client = new T3Client(); client.origin = 'https://example.test'; client.environmentId = 'env'; client.projectId = 'p'; client.generation = 1;
  client.connection = 'connected'; client.configLive = true; client.shellLive = true;
  client.shell.projects = [{ id: 'p', title: 'Project', workspaceRoot: '/repo' }];
  const calls: Obj[] = []; let hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    const value = request.op === 'mobileMediaShare' ? { identifier: JSON.parse(String(request.sourceJSON)).identifier, routeKey: request.routeKey, phase: 'terminal', message: '' }
      : request.op === 'mobileMediaShareStatus' ? { phase: 'idle' } : request.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] }
      : request.method === 'projects.listEntries' ? { entries: obj(request.payload).directoryPath === 'src' ? [{ kind: 'file', path: 'src/a.ts' }] : [{ kind: 'directory', path: 'src' }, { kind: 'file', path: 'README.md' }] }
      : request.method === 'projects.searchEntries' ? { entries: [{ kind: 'file', path: 'other/Needle.ts' }], truncated: true }
      : request.method === 'projects.readFile' ? { contents: 'const a = 1;\r\n\tvalue\r\n', byteLength: 27, truncated: false } : {};
    return { ok: true, generation: client.generation, value };
  } };
  return { client, calls, native, hook(fn: typeof hook) { hook = fn; } };
}
function directoryErrorFixture() {
  const f = fixture(), entries = new Map<string, Obj[]>([
    ['', [{ kind: 'directory', path: 'src' }, { kind: 'file', path: 'README.md' }]],
    ['src', [{ kind: 'directory', path: 'src/nested' }, { kind: 'file', path: 'src/a.ts' }]],
    ['src/nested', [{ kind: 'file', path: 'src/nested/b.ts' }]],
  ]), errors = new Map<string, string>(), search = { error: '' };
  f.hook(request => {
    if (request.method === 'projects.listEntries') {
      const path = String(obj(request.payload).directoryPath ?? '');
      if (errors.has(path)) throw new Error(errors.get(path));
      return { ok: true, generation: f.client.generation, value: { entries: entries.get(path) ?? [] } };
    }
    if (request.method === 'projects.searchEntries' && search.error) throw new Error(search.error);
  });
  return { ...f, entries, errors, search };
}
describe('mobile workspace files', () => {
  test('source tree expands directories without desktop compacting; path search keeps ancestors', () => {
    const tree = buildFileTree([{ kind: 'file', path: 'src/components/SomeFile10.ts' }, { kind: 'file', path: 'src/components/SomeFile2.ts' }, { kind: 'file', path: 'z.ts' }]);
    expect(flattenFileTree({ nodes: tree, expanded: new Set(['src', 'src/components']) }).map(row => row.node.name)).toEqual(['src', 'components', 'SomeFile2.ts', 'SomeFile10.ts', 'z.ts']);
    expect(flattenFileTree({ nodes: tree, expanded: new Set(), searchQuery: 'src fle2' }).map(row => row.node.path)).toEqual(['src', 'src/components', 'src/components/SomeFile2.ts']);
  });
  test('root/toggle/search use existing shared methods and real server paths', async () => {
    const f = fixture(), view = await mobileFilesRead('', '', f.native, f.client);
    expect(view.rows.map(row => row.path)).toEqual(['src', 'README.md']);
    expect(f.calls.find(call => call.method === 'projects.listEntries')?.payload).toEqual({ cwd: '/repo', directoryPath: '' });
    const opened = await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    expect(opened.data.rows.map(row => row.path)).toEqual(['src', 'src/a.ts', 'README.md']);
    expect(opened.data.rows[0]).toMatchObject({ loaded: true, count: '1', expanded: true });
    const searched = await mobileFilesRead('needle', '', f.native, f.client);
    expect(searched.rows.map(row => row.path)).toEqual(['other', 'other/Needle.ts']); expect(searched.truncated).toBe(true);
    expect(f.calls.find(call => call.method === 'projects.searchEntries')?.payload).toEqual({ cwd: '/repo', query: 'needle', limit: 200 });
  });
  test('source normalizes CR/LF, expands tabs, preserves trailing line and clamps requested line', async () => {
    const f = fixture(), view = await mobileFileRead('src/a.ts', f.native, false, 99, false, f.client);
    expect(view.contents).toBe('const a = 1;\n\tvalue\n'); expect(view.rows.map(row => row.text)).toEqual(['const a = 1;', '    value', '']);
    expect(view.copyContents).toBe('const a = 1;\r\n\tvalue\r\n');
    expect(JSON.parse(mobileFileMenu(view, false, false, 'file-visit', true).configuration).actions.find((row: Obj) => row.id === 'copy-contents').copyValue).toBe(view.copyContents);
    expect(view.initialRowId).toBe('source-line:2'); expect(view.rows[2]?.selected).toBe(true);
    expect(view.subtitle).toBe('Project · src'); expect(f.calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/repo', relativePath: 'src/a.ts' });
    expect(f.calls.some(call => call.method === 'projects.writeFile')).toBe(false);
  });
  test('permission downgrade hides already cached tree and file content without another host read', async () => {
    const f = fixture(); await mobileFilesRead('', '', f.native, f.client); await mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
    f.hook(request => request.op === 'http' ? { ok: true, generation: 1, value: { authenticated: true, permissions: [], scopes: ['filesystem:read'] } } : undefined);
    f.calls.length = 0;
    const denied = await mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
    expect(denied.contents).toBe(''); expect(denied.rows).toEqual([]); expect(denied.error).toContain('cannot read host files');
    expect(denied.copyContents).toBe('');
    expect(JSON.parse(mobileFileMenu(denied, false, false, 'file-visit', true).configuration).actions.some((row: Obj) => row.id === 'copy-contents')).toBe(false);
    expect(mobileFilesSnapshot('', f.client).rows).toEqual([]); expect(f.calls.some(call => call.method)).toBe(false);
  });
  test('workspace switch while authorizing issues no old-workspace read', async () => {
    const f = fixture(); let release!: (value: unknown) => void;
    f.hook(request => request.op === 'http' ? new Promise(resolve => { release = resolve; }) : undefined);
    const pending = mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
    f.client.projectId = 'other'; release({ ok: true, generation: 1, value: { authenticated: true, permissions: ['filesystem:read'] } });
    await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.calls.some(call => call.method)).toBe(false);
  });
  test('late old search authorization cannot replace a newer query', async () => {
    const f = fixture(); let release!: (value: unknown) => void; let auth = 0;
    f.hook(request => request.op === 'http' && ++auth === 1 ? new Promise(resolve => { release = resolve; }) : undefined);
    const old = mobileFilesRead('old', '', f.native, f.client);
    await mobileFilesRead('needle', '', f.native, f.client);
    release({ ok: true, generation: 1, value: { authenticated: true, permissions: ['filesystem:read'] } });
    await expect(old).rejects.toMatchObject({ kind: 'superseded' });
    expect(mobileFilesSnapshot('', f.client).query).toBe('needle');
    expect(f.calls.filter(call => call.method === 'projects.searchEntries').map(call => obj(call.payload).query)).toEqual(['needle']);
  });
});

describe('Files directory error projection', () => {
  test.each(['file', 'empty'] as const)('refresh hides removed directory errors with a %s root without discarding the cache', async root => {
    const f = directoryErrorFixture(), view = await mobileFilesRead('', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    const state = filesState(f.client), cached = state.dirs.get('src');
    f.entries.set('', root === 'file' ? [{ kind: 'file', path: 'README.md' }] : []);
    f.errors.set('src', 'Removed src could not reload'); f.calls.length = 0;
    const refreshed = await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client);
    expect(f.calls.filter(call => call.method === 'projects.listEntries').map(call => obj(call.payload).directoryPath)).toEqual(['', 'src']);
    expect(filesState(f.client)).toBe(state); expect(state.dirs.get('src')).toBe(cached);
    expect(state.errors.get('src')).toBe('Removed src could not reload'); expect(state.requested.has('src')).toBe(true);
    expect(state.expanded.has('src')).toBe(true); expect(refreshed.message).toBe('');
    expect(refreshed.data.rows.map(row => row.path)).toEqual(root === 'file' ? ['README.md'] : []);
    const count = f.calls.length;
    expect(mobileFilesSnapshot('', f.client)).toEqual(refreshed.data); expect(f.calls).toHaveLength(count);
    expect(refreshed.data.error).toBe('');
    if (root === 'empty') expect(refreshed.data.emptyDetail).toBe('The workspace is empty.');
  });
  test('a loaded descendant error remains visible under a collapsed reachable ancestor', async () => {
    const f = directoryErrorFixture(), view = await mobileFilesRead('', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src/nested', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    f.errors.set('src/nested', 'Nested directory unavailable');
    const refreshed = await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client);
    expect(refreshed.data.rows.map(row => row.path)).toEqual(['src', 'README.md']);
    expect(refreshed.data.rows[0]).toMatchObject({ expanded: false, loaded: true });
    expect(refreshed.data.error).toBe('Nested directory unavailable');
    expect(filesState(f.client).errors.get('src/nested')).toBe('Nested directory unavailable');
  });
  test('root and active search failures keep priority over reachable directory errors', async () => {
    const f = directoryErrorFixture(), view = await mobileFilesRead('', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    f.errors.set('src', 'Directory unavailable');
    expect((await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client)).data.error).toBe('Directory unavailable');
    f.search.error = 'Search unavailable';
    expect((await mobileFilesRead('needle', '', f.native, f.client)).error).toBe('Search unavailable');
    f.errors.set('', 'Root unavailable');
    const failed = await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client);
    expect(filesState(f.client).search?.error).toBe('Search unavailable');
    expect(filesState(f.client).errors.get('src')).toBe('Directory unavailable'); expect(failed.data.error).toBe('Root unavailable');
    const rejected = await mobileFilesAction(view.owner, 'toggle', 'missing', '', f.native, f.client);
    expect(rejected.data.error).toBe('That directory is no longer available.');
  });
  test('a removed directory failure becomes visible again when its root entry reappears', async () => {
    const f = directoryErrorFixture(), view = await mobileFilesRead('', '', f.native, f.client);
    await mobileFilesAction(view.owner, 'toggle', 'src', '', f.native, f.client);
    f.errors.set('src', 'src unavailable'); f.entries.set('', [{ kind: 'file', path: 'README.md' }]);
    const removed = await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client);
    expect(filesState(f.client).errors.get('src')).toBe('src unavailable');
    f.entries.set('', [{ kind: 'directory', path: 'src' }, { kind: 'file', path: 'README.md' }]);
    const reappeared = await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client);
    expect(reappeared.data.rows.map(row => row.path)).toEqual(['src', 'src/nested', 'src/a.ts', 'README.md']);
    expect(reappeared.data.error).toBe('src unavailable');
    f.errors.delete('src');
    expect((await mobileFilesAction(view.owner, 'refresh', '', '', f.native, f.client)).data.error).toBe('');
    expect(removed.data.error).toBe('');
  });
});


test('tree and file permission reads settle independently in either order', async () => {
  for (const first of ['tree', 'file']) {
    const f = fixture(); let release!: (value: unknown) => void; let auth = 0;
    f.hook(request => request.op === 'http' && ++auth === 1 ? new Promise(resolve => { release = resolve; }) : undefined);
    const pending = first === 'tree' ? mobileFilesRead('', '', f.native, f.client)
      : mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
    if (first === 'tree') await mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
    else await mobileFilesRead('', '', f.native, f.client);
    release({ ok: true, generation: 1, value: { authenticated: true, permissions: ['filesystem:read'] } });
    await pending;
    expect(mobileFilesSnapshot('', f.client).rows.map(row => row.path)).toEqual(['src', 'README.md']);
    expect(mobileFileSnapshot('src/a.ts', false, 0, f.client).contents).toBe('const a = 1;\n\tvalue\n');
    expect(f.calls.filter(call => call.method === 'projects.readFile')).toHaveLength(1);
  }
});

test('tree refresh does not cancel an in-flight file result in the same workspace', async () => {
  const f = fixture(); let release!: (value: unknown) => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; });
  f.hook(request => request.method === 'projects.readFile' ? new Promise(resolve => { release = resolve; entered(); }) : undefined);
  const file = mobileFileRead('src/a.ts', f.native, false, 0, false, f.client); await started;
  const tree = await mobileFilesRead('', '', f.native, f.client);
  await mobileFilesAction(tree.owner, 'refresh', '', '', f.native, f.client);
  release({ ok: true, generation: 1, value: { contents: 'after refresh', byteLength: 13, truncated: false } });
  expect((await file).contents).toBe('after refresh');
  expect(mobileFilesSnapshot('', f.client).rows.length).toBeGreaterThan(0);
});

test('permission denial revokes both panes and refuses an older in-flight file result', async () => {
  const f = fixture(); let release!: (value: unknown) => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; });
  await mobileFilesRead('', '', f.native, f.client);
  f.hook(request => request.method === 'projects.readFile' ? new Promise(resolve => { release = resolve; entered(); }) : undefined);
  const file = mobileFileRead('src/a.ts', f.native, false, 0, false, f.client); await started;
  f.hook(request => request.op === 'http' ? { ok: true, generation: 1, value: { authenticated: true, permissions: [] } } : undefined);
  const denied = await mobileFilesRead('', '', f.native, f.client);
  expect(denied.rows).toEqual([]); expect(denied.error).toContain('cannot read host files');
  release({ ok: true, generation: 1, value: { contents: 'must not publish', byteLength: 16, truncated: false } });
  await expect(file).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileFileSnapshot('src/a.ts', false, 0, f.client).contents).toBe('');
  expect(mobileFilesSnapshot('', f.client).rows).toEqual([]);
});

test('older grant cannot restore either pane after a concurrent permission denial', async () => {
  const f = fixture(); let release!: (value: unknown) => void; let auth = 0;
  f.hook(request => request.op === 'http' ? ++auth === 1 ? new Promise(resolve => { release = resolve; })
    : { ok: true, generation: 1, value: { authenticated: true, permissions: [] } } : undefined);
  const old = mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
  await mobileFilesRead('', '', f.native, f.client);
  release({ ok: true, generation: 1, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(old).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.method)).toBe(false);
  expect(mobileFileSnapshot('src/a.ts', false, 0, f.client).rows).toEqual([]);
  expect(mobileFilesSnapshot('', f.client).rows).toEqual([]);
});


test('a file deep link loads its selected ancestry in the inspector tree', async () => {
  const f = fixture();
  const view = await mobileFilesRead('', 'src/a.ts', f.native, f.client);
  expect(view.rows.map(row => row.path)).toEqual(['src', 'src/a.ts', 'README.md']);
  expect(view.rows.find(row => row.path === 'src')).toMatchObject({ expanded: true, loaded: true });
  expect(view.rows.find(row => row.path === 'src/a.ts')).toMatchObject({ selected: true });
  expect(f.calls.filter(call => call.method === 'projects.listEntries').map(call => obj(call.payload).directoryPath)).toEqual(['', 'src']);
});

test('file actions preserve escaped paths, empty contents and bounded preview; unavailable text cannot be copied', () => {
  const path = 'src/"quoted"\\file.ts';
  const present = (read?: { contents: string; truncated?: boolean; error?: string }, loading = false, error = '') =>
    mobileFilePresentation({ owner: 'owner', revision: 1, projectName: 'Project', loading, error, read }, path);
  const actions = (data: ReturnType<typeof present>) => JSON.parse(mobileFileMenu(data, false, false, 'file-visit', false).configuration).actions as Obj[];
  const empty = actions(present({ contents: '' }));
  expect(empty.map(row => row.title)).toEqual(['Enable word wrap', 'Copy path', 'Copy contents']);
  expect(empty[0].disabled).toBe(true); expect(empty[1].copyValue).toBe(path); expect(empty[2].copyValue).toBe('');
  const raw = '\tquoted "value"\r\n\u0000end\r';
  const truncated = actions(present({ contents: raw, truncated: true }));
  expect(truncated[2]).toMatchObject({ title: 'Copy preview', copyValue: raw });
  for (const data of [present(), present({ contents: raw }, true), present({ contents: raw, error: 'read failed' }), present({ contents: raw }, false, 'permission denied')])
    expect(actions(data).some(row => row.id === 'copy-contents')).toBe(false);
});

test('Markdown file actions keep source default and omit wrapping only for the selected preview', () => {
  const data = mobileFilePresentation({ owner: 'owner', revision: 1, projectName: 'Project', loading: false, error: '', read: { contents: '# Heading\n' } }, 'README.md');
  const actions = (preview: boolean) => JSON.parse(mobileFileMenu(data, preview, true, 'file-visit', true).configuration).actions as Obj[];
  expect(actions(false).map(row => row.id)).toEqual(['preview', 'source', 'word-wrap', 'copy-path', 'copy-contents']);
  expect(actions(false).slice(0, 2).map(row => [row.title, row.selected, row.inline])).toEqual([['Preview', false, true], ['Source', true, true]]);
  expect(actions(false)[2].title).toBe('Disable word wrap');
  expect(actions(true).map(row => row.id)).toEqual(['preview', 'source', 'copy-path', 'copy-contents']);
  expect(actions(true)[0].selected).toBe(true); expect(actions(true)[1].selected).toBe(false);
});

test('file previews preserve prose and independently copyable fenced code under the existing read cache', () => {
  const read = { contents: '# File\r\n\r\nFirst line\r\ncontinues with `code`.\r\n\r\n```ts\r\nconst first = 1;\r\n```\r\nBetween\r\n\r\n| **Name** | Value |\r\n| --- | --- |\r\n| `item` **ready** | **42** |\r\n\r\n```\r\nsecond\r\n```\r\n', error: '' };
  const present = (path = 'README.md', dark = false) => mobileFilePresentation({ owner: 'owner', revision: 1,
    projectName: 'Project', loading: false, error: '', read }, path, dark);
  const light = present(), codes = light.previewBlocks.filter(block => block.kind === 'code');
  expect(light.previewBlocks.map(block => block.kind)).toEqual(['heading', 'markdown', 'code', 'markdown', 'table', 'code']);
  expect(codes.map(block => [block.language, block.text])).toEqual([['ts', 'const first = 1;'], ['', 'second']]);
  expect(light.previewBlocks.filter(block => block.kind === 'markdown').map(block => block.text.trim()))
    .toEqual(['First line continues with `code`.', 'Between']);
  for (const block of codes) expect(block.tokens.map(token => token.text).join('')).toBe(block.text);
  expect(light.previewBlocks.find(block => block.kind === 'table')).toMatchObject({ columns: 2, rows: [{ header: true }, { header: false }] });
  expect(light.previewBlocks.find(block => block.kind === 'table')?.rows[1]?.cells[0]?.runs).toEqual([
    { id: '1:0:0', kind: 'code', text: 'item' }, { id: '1:0:1', kind: 'text', text: ' ' },
    { id: '1:0:2', kind: 'strong', text: 'ready' },
  ]);
  expect(light.copyContents).toBe(read.contents);
  expect(present().previewBlocks).toBe(light.previewBlocks);
  expect(present('README.md', true).previewBlocks.find(block => block.kind === 'code')?.tokens.map(token => token.color))
    .not.toEqual(codes[0]?.tokens.map(token => token.color));
  expect(present('README.txt').previewBlocks).toEqual([]);
  read.contents = '```text\nchanged\n```';
  expect(present().previewBlocks).toMatchObject([{ kind: 'code', language: 'text', text: 'changed' }]);
  read.error = 'File removed';
  expect(present()).toMatchObject({ previewBlocks: [], copyContents: '' });
});

test('source presentation keeps current visit metadata when the same read is reused', () => {
  const read = { contents: 'const count = 1;\r\n', truncated: false };
  const first = mobileFilePresentation({ owner: 'first', revision: 1, projectName: 'First', loading: false, error: '', read }, 'src/count.ts');
  read.truncated = true;
  const next = mobileFilePresentation({ owner: 'next', revision: 2, projectName: 'Next', loading: true, error: 'Reconnecting', read }, 'src/count.ts');
  expect(next).toMatchObject({ owner: 'next', revision: 2, subtitle: 'Next · src', loading: true, error: 'Reconnecting',
    truncated: true, copyContents: '', contents: 'const count = 1;\n', rows: first.rows });
  expect(next.notice).toContain('truncated');
});

test('both emphasis preview owners retain Source, returned Copy inputs, fences and appearance refresh', () => {
  const raw = '😀 File\r\n\r\n| *Header* | _Value_ |\r\n| --- | --- |\r\n| pre _ready_ **bold** `code` post | *&#49;&#50;* ms |\r\n\r\n```sh\r\npwd\r\n\r\n```\r\n';
  const read = { contents: raw, error: '' };
  const input = { owner: 'owner', revision: 1, projectName: 'Project', loading: false, error: '', read };
  const bytes = new TextEncoder().encode(raw);
  const entry: AttachmentDocumentCache = { name: 'format.md', kind: 'markdown', mimeType: 'text/markdown', sizeBytes: bytes.length,
    content: decodeFilePreviewText(bytes), contentError: '', sourceJSON: '', uri: '', authorizedAt: 0, dark: null, rows: [], previewBlocks: [] };
  for (const dark of [false, true, false]) {
    const workspace = mobileFilePresentation(input, 'format.md', dark);
    const attachment = attachmentDocumentProjection('file', entry, 'transcript', true, dark, true);
    expect(workspace.previewBlocks).toEqual(attachment.previewBlocks);
    expect(workspace.previewBlocks.find(block => block.kind === 'table')?.rows[1]?.cells[0]?.runs.map(run => run.kind))
      .toEqual(['text', 'emphasis', 'text', 'strong', 'text', 'code', 'text']);
    expect(workspace.copyContents).toBe(raw);
    expect(attachment.text).toBe(raw);
    const menu = obj(JSON.parse(mobileFileMenu(workspace, true, false, 'visit', true).configuration));
    expect(arr(menu.actions).map(obj).find(action => action.id === 'copy-contents')?.copyValue).toBe(raw);
    expect(workspace.previewBlocks.filter(block => block.kind === 'code'))
      .toEqual(mobileMarkdownBlocks(raw, dark).filter(block => block.kind === 'code').map(block => ({ ...block, level: 0, columns: 0, rows: [] })));
    expect(mobileFilePresentation(input, 'format.md', dark).previewBlocks).toBe(workspace.previewBlocks);
    expect(attachmentDocumentProjection('file', entry, 'transcript', true, dark, true).previewBlocks).toBe(attachment.previewBlocks);
    expect(attachmentDocumentProjection('file', entry, 'transcript', false, dark, true)).toMatchObject({ activeMode: 'source', text: raw });
    expect(attachmentDocumentProjection('file', entry, 'transcript', true, dark, false)).toMatchObject({ activeMode: 'source', text: raw });
  }
});

test('source rows follow changed bytes, read failures, language, theme and selected line', () => {
  const read = { contents: '\tconst count = 1;\nreturn count;', error: '' };
  const present = (path = 'count.ts', dark = false, line = 0) =>
    mobileFilePresentation({ owner: 'owner', revision: 1, projectName: 'Project', loading: false, error: '', read }, path, dark, line);
  const light = present(), dark = present('count.ts', true, 2), plain = present('count.txt');
  expect(light.rows[0]?.tokens.map(token => token.text).join('')).toBe(light.rows[0]?.text);
  expect(dark.rows[1]?.selected).toBe(true); expect(dark.initialRowId).toBe('source-line:1');
  expect(dark.rows[0]?.tokens.map(token => token.color)).not.toEqual(light.rows[0]?.tokens.map(token => token.color));
  expect(plain.rows[0]?.tokens).not.toEqual(light.rows[0]?.tokens);
  expect(present().rows.some(row => row.selected)).toBe(false);
  read.contents = 'updated\n';
  expect(present().rows.map(row => row.text)).toEqual(['updated', '']);
  read.error = 'File removed';
  expect(present()).toMatchObject({ error: 'File removed', rows: [], copyContents: '' });
  read.error = '';
  expect(present().rows.map(row => row.text)).toEqual(['updated', '']);
});

test('source presentation preserves every line and trailing newline of a large read', () => {
  const lines = Array.from({ length: 6175 }, (_, index) => `line ${index + 1}`), contents = `${lines.join('\n')}\n`;
  const data = mobileFilePresentation({ owner: 'owner', revision: 1, projectName: 'Project', loading: false, error: '', read: { contents } }, 'large.txt', false, 6175);
  expect(data.rows).toHaveLength(6176); expect(data.rows[6174]).toMatchObject({ text: 'line 6175', newNumber: '6175', selected: true });
  expect(data.rows[6175]?.text).toBe(''); expect(data.copyContents).toBe(contents);
});

test('source width counts displayed UTF-16 columns across the complete file', () => {
  const read = { contents: 'short\n\t😀 longest line\n', error: '' };
  const present = () => mobileFilePresentation({ owner: 'owner', revision: 1, projectName: 'Project', loading: false, error: '', read }, 'width.txt');
  expect(present().maxColumns).toBe('    😀 longest line'.length);
  read.contents = 'tiny'; expect(present().maxColumns).toBe(4);
  read.error = 'Unavailable'; expect(present().maxColumns).toBe(0);
});

test.each([
  ['docs/guide.md#draft', true],
  ['docs/guide.mdx?raw=1', true],
  ['docs/guide.MD#draft?raw=1', true],
  ['docs/plain.txt#README.md', false],
  ['docs/plain.txt?name=README.mdx', false],
  ['docs/guide.md%23draft', false],
  ['docs/guide%2Emd', false],
  ['docs/guide%23draft.md', true],
] as const)('file actions classify Markdown before a query or fragment without changing literal path %s', async (path, markdown) => {
  const f = fixture(), view = await mobileFileRead(path, f.native, false, 0, false, f.client);
  const actions = JSON.parse(mobileFileMenu(view, false, false, 'file-visit', true).configuration).actions as Obj[];
  expect(f.calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/repo', relativePath: path });
  expect(view.path).toBe(path);
  expect(actions.find(row => row.id === 'copy-path')?.copyValue).toBe(path);
  expect(actions.find(row => row.id === 'copy-contents')?.copyValue).toBe('const a = 1;\r\n\tvalue\r\n');
  expect(actions.map(row => row.id)).toEqual(markdown
    ? ['preview', 'source', 'word-wrap', 'copy-path', 'copy-contents']
    : ['word-wrap', 'copy-path', 'copy-contents']);
  if (markdown) expect(actions.slice(0, 2).map(row => [row.id, row.selected])).toEqual([['preview', false], ['source', true]]);
});

test('an unrelated tree refresh preserves the captured file menu while a changed file or visit retires it', async () => {
  const f = fixture(); await mobileFileRead('src/a.ts', f.native, false, 0, false, f.client);
  const menu = (visit = 'file-visit') => mobileFileMenu(mobileFileSnapshot('src/a.ts', false, 0, f.client), false, false, visit, true).configuration;
  const before = menu();
  await mobileFilesRead('', '', f.native, f.client);
  await mobileFilesAction(mobileFilesSnapshot('', f.client).owner, 'refresh', '', '', f.native, f.client);
  expect(menu()).toBe(before); expect(menu('new-file-visit')).not.toBe(before);
  f.hook(request => request.method === 'projects.readFile' ? { ok: true, generation: 1, value: { contents: 'changed bytes', byteLength: 13, truncated: false } } : undefined);
  await mobileFileRead('src/a.ts', f.native, false, 0, true, f.client);
  expect(menu()).not.toBe(before);
});

function svgFixture(path = 'assets/icon.svg') {
  const f = fixture(); f.client.threadId = 'thread'; f.client.threadLive = true;
  let grants = true, signs = 0;
  const visit: SvgVisit = { path, location: `/threads/env/thread/files/${encodeURIComponent(path)}`, visit: 'visit-a', flowOwner: '' };
  const observe = (at = visit) => workspaceSvgObserve([{ id: at.visit, name: at.path ? 'threadFile' : 'thread', url: at.location, params: { filePath: at.path } }], f.client);
  observe();
  f.hook(request => request.op === 'http' ? { ok: true, generation: 1, value: { authenticated: true, permissions: grants ? ['filesystem:read'] : [] } }
    : request.method === 'assets.createUrl' ? { ok: true, generation: 1, value: { relativeUrl: `/signed.svg?signature=${++signs}` } } : undefined);
  const read = (at = visit) => workspaceSvgPrepare(at, f.native, false, false, f.client);
  const snapshot = (at = visit) => workspaceSvgSnapshot(at, false, f.client)!;
  const action = (operation: string) => workspaceSvgAction(JSON.stringify({ identifier: snapshot().owner, operation }),
    { owner: snapshot().owner, visit: visit.visit, location: visit.location }, f.native, f.client);
  const status = () => workspaceSvgRead(visit, f.native, false, f.client);
  return { ...f, visit, read, snapshot, status, action, observe, deny: () => { grants = false; } };
}

test.each([
  ['assets/icon.svg', 'workspace-file', '/repo/assets/icon.svg', 'assets/icon.svg'],
  ['/other/raw.svg#draft?x', 'media-file', '/other/raw.svg#draft?x', ''],
  ['C:\\outside\\icon.svg', 'media-file', 'C:\\outside\\icon.svg', ''],
  ['\\\\server\\share\\icon.svg', 'media-file', '\\\\server\\share\\icon.svg', ''],
] as const)('SVG signs the reference resource and preserves literal paths: %s', async (path, tag, absolutePath, relativePath) => {
  const f = svgFixture(path), data = (await f.read())!;
  expect(f.calls.filter(call => call.method === 'projects.readFile')).toHaveLength(0);
  expect(f.calls.find(call => call.method === 'assets.createUrl')?.payload).toEqual({ resource: { _tag: tag, threadId: 'thread', path: absolutePath } });
  expect(data).toMatchObject({ path, fullPath: absolutePath, relativePath, loading: false, svgCanRead: true, rows: [], contents: '' });
  expect(JSON.parse(data.svgSource)).toMatchObject({ source: 'remote', workspaceSvg: true, url: 'https://example.test/signed.svg?signature=1' });
});

test('SVG image menu has only reference image actions and shares fresh original bytes', async () => {
  const f = svgFixture(), data = (await f.read())!;
  const actions = JSON.parse(mobileFileMenu(data, false, false, 'visit-a', true).configuration).actions as Obj[];
  expect(actions.map(row => row.title)).toEqual(['Copy full path', 'Copy relative path', 'Save or share', 'Refresh']);
  expect(actions.slice(0, 2).map(row => row.copyValue)).toEqual(['/repo/assets/icon.svg', 'assets/icon.svg']);
  expect((await f.action('share')).message).toBe('');
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
  const request = f.calls.find(call => call.op === 'mobileMediaShare')!;
  expect(JSON.parse(String(request.sourceJSON))).toMatchObject({ source: 'remote', kind: 'file', name: 'icon.svg', url: 'https://example.test/signed.svg?signature=2' });
  expect(f.snapshot().svgSource).toBe(data.svgSource);
});

test('SVG Refresh reuses the signature; explicit retry remints and changes the native identity', async () => {
  const f = svgFixture(), first = (await f.read())!;
  await f.action('refresh'); const refreshed = f.snapshot();
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(1);
  expect(JSON.parse(refreshed.svgSource).url).toBe('https://example.test/signed.svg?signature=1&revision=1');
  expect(JSON.parse(refreshed.svgSource).identifier).not.toBe(JSON.parse(first.svgSource).identifier);
  await f.action('retry');
  expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
  expect(JSON.parse(f.snapshot().svgSource).url).toContain('signature=2');
});

test('failed SVG resolution settles and a retry clears the failed result', async () => {
  const f = svgFixture(), original = f.native.later; let fail = true;
  f.native.later = input => obj(input).method === 'assets.createUrl' && fail
    ? Promise.resolve({ ok: false, generation: 1, error: { kind: 'File', message: 'missing' } }) : original(input);
  expect(await f.read()).toMatchObject({ loading: false, svgFailure: 'failed', svgSource: '', error: 'missing' });
  fail = false; await f.action('retry');
  expect(f.snapshot()).toMatchObject({ loading: false, svgFailure: '', error: '' });
  expect(f.snapshot().svgSource).not.toBe('');
});

test('late SVG response cannot enter a new visit of the same path', async () => {
  const f = svgFixture(), original = f.native.later;
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).method === 'assets.createUrl' ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const pending = f.read(); await started;
  const next = { ...f.visit, visit: 'visit-b' }; f.observe(next);
  release({ ok: true, generation: 1, value: { relativeUrl: '/stale.svg' } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot(next).svgSource).toBe('');
});

test('a newer permission denial retires an in-flight SVG signature', async () => {
  const f = svgFixture(), original = f.native.later;
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).method === 'assets.createUrl' ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const pending = f.read(); await started; f.deny();
  await f.client.http(mobileNative(f.native, f.client), '/api/auth/session');
  release({ ok: true, generation: 1, value: { relativeUrl: '/denied.svg' } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot()).toMatchObject({ svgSource: '', svgCanRead: false, svgFailure: 'failed' });
});

test('leaving an SVG visit drops the JS owner without using a captured native handle', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).op === 'mobileMediaShare'
    ? (f.calls.push(obj(input)), began(), new Promise(resolve => { release = resolve; })) : original(input);
  const pending = f.action('share'); await started;
  await f.action('share'); expect(f.calls.filter(call => call.op === 'mobileMediaShare')).toHaveLength(1);
  f.observe({ path: '', location: '/', visit: 'home', flowOwner: '' });
  expect(f.calls.some(call => call.op === 'mobileMediaShareCancel')).toBe(false);
  release({ ok: false, generation: 1, error: { kind: 'Media', message: 'cancelled' } });
  expect((await pending).message).toBe('');
});

test('SVG initial load and share signing settle independently', async () => {
  const f = svgFixture(), original = f.native.later; let signs = 0;
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).method === 'assets.createUrl' && ++signs === 1
    ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const loading = f.read(); await started; expect((await f.action('share')).message).toBe('');
  release({ ok: true, generation: 1, value: { relativeUrl: '/initial.svg' } });
  expect(await loading).toMatchObject({ loading: false, svgSharing: false, error: '' });
  expect(JSON.parse(f.snapshot().svgSource).url).toBe('https://example.test/initial.svg');
});

test('SVG retry re-mints after reconnect with unchanged connection identity', async () => {
  const f = svgFixture(), original = f.native.later; f.client.connection = 'disconnected';
  await f.read();
  f.native.later = async input => { if (obj(input).op === 'retry') { f.client.connection = 'connected'; return { ok: true, generation: 1, value: {} }; } return original(input); };
  expect((await f.action('retry')).message).toBe('');
  expect(f.snapshot()).toMatchObject({ loading: false, error: '', svgFailure: '' });
  expect(JSON.parse(f.snapshot().svgSource).url).toContain('/signed.svg');
});

test('SVG failed retry preserves its error and loses authority when its route departs', async () => {
  const f = svgFixture(); f.client.connection = 'disconnected'; await f.read();
  f.native.later = async () => ({ ok: false, generation: 1, error: { kind: 'Reconnect', message: 'retry denied', uncertain: true } });
  expect(await f.action('retry')).toMatchObject({ message: 'retry denied', kind: 'Reconnect', uncertain: true });
  let release!: (value: unknown) => void, started!: () => void;
  const pending = new Promise<void>(resolve => { started = resolve; });
  f.native.later = () => (started(), new Promise(resolve => { release = resolve; }));
  const retry = f.action('retry'); await pending;
  f.observe({ ...f.visit, location: '/threads/env/thread', path: '', visit: 'back' });
  release({ ok: false, generation: 1, error: { kind: 'Reconnect', message: 'old error' } });
  expect((await retry).message).toBe('');
});

test('SVG revoked share settles and can retry without a stuck busy flag', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).op === 'mobileMediaShare' ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const share = f.action('share'); await started; f.deny();
  await f.client.http(mobileNative(f.native, f.client), '/api/auth/session');
  expect(f.snapshot()).toMatchObject({ svgSharing: true, svgCanRead: false, loading: false });
  await f.status();
  release({ ok: false, generation: 1, error: { kind: 'Media', message: 'cancelled' } }); await share;
  expect(f.snapshot().svgSharing).toBe(false);
  await f.action('retry'); expect(f.snapshot().loading).toBe(false);
});

test('SVG retry cannot start signing in a different focused environment after awaiting', async () => {
  const f = svgFixture(); f.client.connection = 'disconnected'; await f.read();
  let release!: (value: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => { f.calls.push(obj(input)); began(); return new Promise(resolve => { release = resolve; }); };
  const pending = f.action('retry'); await started;
  f.client.environmentId = 'other'; f.client.origin = 'https://other.test'; f.client.connection = 'connected';
  release({ ok: true, generation: 1, value: {} });
  expect((await pending).message).toBe('');
  expect(f.calls.some(call => call.method === 'assets.createUrl')).toBe(false);
});

test('SVG drive-slash workspace uses the reference Windows join', async () => {
  const f = svgFixture(); f.client.shell.projects[0]!.workspaceRoot = 'C:/repo';
  const data = (await f.read())!;
  expect(data.fullPath).toBe('C:/repo\\assets\\icon.svg');
  expect(obj(f.calls.find(call => call.method === 'assets.createUrl')?.payload).resource).toMatchObject({ path: data.fullPath });
});

test('SVG same-id location replacement rejects a captured action without snapshot demand', async () => {
  const f = svgFixture(); await f.read(); const captured = f.snapshot(); const before = f.calls.length;
  f.observe({ ...f.visit, path: 'other.svg', location: '/threads/env/thread/files/other.svg' });
  await workspaceSvgAction(JSON.stringify({ identifier: captured.owner, operation: 'share' }),
    { owner: captured.owner, visit: f.visit.visit, location: f.visit.location }, f.native, f.client);
  expect(f.calls).toHaveLength(before);
  expect(() => f.snapshot()).toThrow('The file route changed.');
});

test.each([
  ['offline', 'You are offline', false, false], ['connecting', 'Connecting to Laptop...', true, true],
  ['reconnecting', 'Reconnecting to Laptop...', true, true], ['unsupported', 'Client not supported', false, false],
  ['available', 'Laptop is disconnected', true, false], ['error', 'Laptop is unavailable', true, false],
] as const)('SVG connection notice follows reference %s presentation', (phase, title, retry, retrying) => {
  expect(workspaceSvgConnection(phase, 'Laptop', '', 'trace')).toMatchObject({ title, retry, retrying, trace: 'trace' });
  expect(workspaceSvgConnection('reconnecting', 'Laptop', 'network error').detail).toBe('The app will keep retrying automatically. network error');
});

test('SVG connecting is loading while reconnecting uses the reference connection notice', async () => {
  const f = svgFixture(); f.client.connection = 'connecting';
  expect(f.snapshot()).toMatchObject({ svgPhase: 'connecting', svgFailure: '', loading: true });
  f.client.connection = 'reconnecting';
  expect(f.snapshot()).toMatchObject({ svgPhase: 'reconnecting', svgFailure: 'disconnected', svgRetrying: true, svgRetryAllowed: true });
});

let restoreAppClient: (() => void) | undefined;
afterEach(() => { if (restoreAppClient) { workspaceSvgObserve([], mobileClient); restoreAppClient(); restoreAppClient = undefined; } });
function appSvgFixture(path: string) {
  const saved = { ...mobileClient }, projects = mobileClient.shell.projects;
  restoreAppClient = () => { Object.assign(mobileClient, saved); mobileClient.shell.projects = projects; };
  const generation = mobileClient.generation + 1;
  Object.assign(mobileClient, { origin: 'https://example.test', environmentId: 'env', projectId: 'p', generation,
    connection: 'connected', configLive: true, shellLive: true, threadId: 'thread', threadLive: true });
  mobileClient.shell.projects = [{ id: 'p', title: 'Project', workspaceRoot: '/repo' }];
  const visit = { path, location: '/threads/env/thread/files/' + encodeURIComponent(path), visit: 'actual-' + generation, flowOwner: '' };
  const args = [path, 'light', 0, false, visit.location, '', visit.visit], calls: Obj[] = [];
  let hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    return { ok: true, generation, value: request.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] }
      : request.method === 'assets.createUrl' ? { relativeUrl: '/real.svg' }
      : request.method === 'projects.readFile' ? { contents: '<svg>cached</svg>', truncated: false } : {} };
  } };
  const call = (name: 'workspace' | 'fileSnapshot' | 'filePrepare', values: unknown[], input: Native | null = native) => answer(name, values as never, {} as never, null as never, input as never);
  const observe = (active: boolean) => call('workspace', [[{ id: active ? visit.visit : 'home', name: active ? 'threadFile' : 'home',
    url: active ? visit.location : '/', params: active ? { filePath: path, threadEnvironment: 'env', threadId: 'thread' } : {} }], { width: 390, height: 844 }, 0]);
  return { visit, args, calls, native, call, observe, hook: (next: typeof hook) => { hook = next; } };
}

test('actual app rejects a queued SVG prepare after workspace departure without a text fallback', async () => {
  const f = appSvgFixture('assets/offroute.svg'); f.observe(true); f.observe(false);
  await expect(Promise.resolve().then(() => f.call('filePrepare', f.args))).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.method === 'projects.readFile')).toBe(false);
});

test('actual app discards a late SVG failure after departure without entering the text reader', async () => {
  const f = appSvgFixture('assets/late.svg'); f.observe(true);
  let release!: (value: unknown) => void, began!: () => void; const started = new Promise<void>(resolve => { began = resolve; });
  f.hook(request => request.method === 'assets.createUrl' ? (began(), new Promise(resolve => { release = resolve; })) : undefined);
  const pending = Promise.resolve(f.call('filePrepare', f.args)); await started; f.observe(false);
  release({ ok: false, generation: mobileClient.generation, error: { kind: 'File', message: 'asset missing' } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.method === 'projects.readFile')).toBe(false);
});

test('actual app cannot reveal cached SVG source from an old snapshot', async () => {
  const f = appSvgFixture('assets/cached.svg'); await mobileFileRead(f.visit.path, f.native, false, 0, false, mobileClient);
  f.observe(true); f.observe(false);
  await expect(Promise.resolve().then(() => f.call('fileSnapshot', f.args))).rejects.toThrow('The file route changed.');
});

test('actual app treats a missing native SVG bridge as a preview failure, never source text', async () => {
  const f = appSvgFixture('assets/no-native.svg'); f.observe(true);
  expect(await f.call('filePrepare', f.args, null)).toMatchObject({ svg: true, rows: [], loading: false, svgSource: '', svgFailure: 'failed' });
  expect(f.calls).toHaveLength(0);
});

test.each([false, true])('obsolete SVG load %s cannot replace a newer successful retry', async failed => {
  const f = svgFixture(), original = f.native.later; let signs = 0;
  let release!: (value: unknown) => void, began!: () => void; const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).method === 'assets.createUrl' && ++signs === 1
    ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const first = f.read(); await started; await f.action('retry');
  const latest = f.snapshot().svgSource;
  release(failed ? { ok: false, generation: 1, error: { kind: 'File', message: 'old failure' } }
    : { ok: true, generation: 1, value: { relativeUrl: '/old.svg' } });
  await expect(first).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot()).toMatchObject({ svgSource: latest, error: '', loading: false });
});

test('a later SVG action suppresses an old share error report', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let release!: (value: unknown) => void, began!: () => void; const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => obj(input).op === 'mobileMediaShare' ? (began(), new Promise(resolve => { release = resolve; })) : original(input);
  const share = f.action('share'); await started; await f.action('refresh');
  release({ ok: false, generation: 1, error: { kind: 'Media', message: 'obsolete share error' } });
  expect((await share).message).toBe('');
  expect(f.snapshot().error).toBe('');
});


test('an abandoned Share acknowledgement remains busy until fresh native status settles', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let status: Obj = {}, shareCalls = 0;
  f.native.later = async input => {
    const request = obj(input);
    if (request.op === 'mobileMediaShare') {
      shareCalls++;
      status = { identifier: JSON.parse(String(request.sourceJSON)).identifier, routeKey: f.visit.visit, phase: 'preparing', message: '' };
      throw Object.assign(new Error('abandoned'), { name: 'FetchError', kind: 'Aborted' });
    }
    if (request.op === 'mobileMediaShareStatus') return { ok: true, generation: 1, value: status };
    return original(input);
  };
  expect((await f.action('share')).message).toBe('');
  expect(f.snapshot().svgSharing).toBe(true);
  await f.action('refresh'); await f.action('share');
  expect(shareCalls).toBe(1); expect((await f.status())!.svgSharing).toBe(true);
  status = { ...status, phase: 'cancelling' };
  expect((await f.status())!.svgSharing).toBe(true);
  status = { ...status, phase: 'terminal', message: 'Download failed' };
  expect(await f.status()).toMatchObject({ svgSharing: false, svgShareMessage: 'Download failed' });
  expect((await f.status())!.svgShareMessage).toBe('Download failed');
  await f.action('share'); expect(shareCalls).toBe(2);
});

test('revocation uses the fresh status answer and keeps busy through native cleanup', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let status: Obj = {}, cancelCalls = 0;
  f.native.later = async input => {
    const request = obj(input);
    if (request.op === 'mobileMediaShare') {
      status = { identifier: JSON.parse(String(request.sourceJSON)).identifier, routeKey: f.visit.visit, phase: 'preparing' };
      return { ok: true, generation: 1, value: status };
    }
    if (request.op === 'mobileMediaShareStatus') return { ok: true, generation: 1, value: status };
    if (request.op === 'mobileMediaShareCancel') {
      cancelCalls++; expect(request.identifier).toBe(status.identifier);
      status = { ...status, phase: 'cancelling' }; return { ok: true, generation: 1, value: status };
    }
    return original(input);
  };
  await f.action('share'); f.deny(); await f.client.http(mobileNative(f.native, f.client), '/api/auth/session');
  expect(await f.status()).toMatchObject({ svgSharing: true, svgCanRead: false }); expect(cancelCalls).toBe(1);
  expect(await f.status()).toMatchObject({ svgSharing: true, svgCanRead: false }); expect(cancelCalls).toBe(1);
  status = { ...status, phase: 'terminal', message: 'The file route changed.' };
  expect(await f.status()).toMatchObject({ svgSharing: false, svgShareMessage: '' });
});

test('a delayed idle status cannot clear a newer share attempt', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later;
  let release!: (value: unknown) => void, started!: () => void;
  const began = new Promise<void>(resolve => { started = resolve; });
  f.native.later = input => {
    const request = obj(input);
    if (request.op === 'mobileMediaShareStatus') { started(); return new Promise(resolve => { release = resolve; }); }
    if (request.op === 'mobileMediaShare') return Promise.resolve({ ok: true, generation: 1, value: {
      identifier: JSON.parse(String(request.sourceJSON)).identifier, routeKey: f.visit.visit, phase: 'presented' } });
    return original(input);
  };
  const pending = f.status(); await began; await f.action('share');
  release({ ok: true, generation: 1, value: { phase: 'idle' } }); await pending;
  expect(f.snapshot().svgSharing).toBe(true);
});


test('a mismatched-owner cancelling status does not request another cancellation', async () => {
  const f = svgFixture(); await f.read(); const original = f.native.later; let cancelCalls = 0;
  f.native.later = async input => {
    const request = obj(input);
    if (request.op === 'mobileMediaShareStatus') return { ok: true, generation: 1,
      value: { identifier: 'retired-owner-share', routeKey: f.visit.visit, phase: 'cancelling' } };
    if (request.op === 'mobileMediaShareCancel') cancelCalls++;
    return original(input);
  };
  await f.status(); await f.status(); expect(cancelCalls).toBe(0);
});

test('an abandoned SVG preparation can sign again on its next same-visit preparation', async () => {
  const f = svgFixture(), original = f.native.later; let signs = 0;
  f.native.later = async input => {
    if (obj(input).method === 'assets.createUrl' && ++signs === 1)
      throw Object.assign(new Error('answer replaced'), { name: 'FetchError', kind: 'Aborted' });
    return original(input);
  };
  await expect(f.read()).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot()).toMatchObject({ svgSource: '', error: '', svgFailure: '' });
  expect(await f.read()).toMatchObject({ loading: false, error: '', svgFailure: '' });
  expect(signs).toBe(2); expect(f.snapshot().svgSource).not.toBe('');
});

test.each(['success', 'error', 'new-generation'] as const)('overlapping same-visit preparation fences the older %s result', async older => {
  const f = svgFixture(), original = f.native.later; let signs = 0;
  let release!: (value: unknown) => void, reject!: (reason: unknown) => void, began!: () => void;
  const started = new Promise<void>(resolve => { began = resolve; });
  f.native.later = input => {
    if (obj(input).method === 'assets.createUrl' && ++signs === 1) {
      began(); return new Promise((resolve, fail) => { release = resolve; reject = fail; });
    }
    return Promise.resolve(original(input)).then(reply => ({ ...obj(reply), generation: f.client.generation }));
  };
  const pending = f.read().catch(error => error); await started;
  if (older === 'new-generation') f.client.generation++;
  const replacement = await f.read(); expect(signs).toBe(2);
  expect(replacement).toMatchObject({ loading: false, error: '', svgFailure: '' });
  const source = replacement!.svgSource; expect(source).not.toBe('');
  if (older === 'error') reject(new Error('old preparation error'));
  else release({ ok: true, generation: 1, value: { relativeUrl: '/obsolete.svg' } });
  expect(await pending).toMatchObject({ kind: 'superseded' });
  expect(f.snapshot()).toMatchObject({ svgSource: source, loading: false, error: '', svgFailure: '' });
});


test('workspace formatted preview folds soft breaks while returned Source and copy data stay exact', () => {
  const contents = 'Before **ready**\r\nafter `code`\r\n\r\n```txt\r\nSENTINEL\r\nkeep newline\r\n\r\n```\r\n';
  const view = mobileFilePresentation({ owner: 'file', revision: 1, projectName: 'Project', loading: false, error: '',
    read: { contents, error: '' } }, 'notes.md', false);
  expect(view.previewBlocks[0]?.text).toBe('Before **ready** after `code`\n\n');
  expect(view.previewBlocks[1]).toMatchObject({ id: '1', kind: 'code', text: 'SENTINEL\nkeep newline\n', language: 'txt' });
  expect(view.copyContents).toBe(contents);
  expect(view.contents).toBe(contents.replaceAll('\r\n', '\n'));
  expect(view.rows.map(row => row.text)).toEqual(['Before **ready**', 'after `code`', '', '```txt', 'SENTINEL', 'keep newline', '', '```', '']);
});
