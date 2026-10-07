import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileFilesRead, mobileFilesAction, mobileFileRead, mobileFileSnapshot, mobileFilesSnapshot } from './file-data';
import { buildFileTree, flattenFileTree } from './file-tree-model';

function fixture() {
  const client = new T3Client(); client.origin = 'https://example.test'; client.environmentId = 'env'; client.projectId = 'p'; client.generation = 1;
  client.connection = 'connected'; client.configLive = true; client.shellLive = true;
  client.shell.projects = [{ id: 'p', title: 'Project', workspaceRoot: '/repo' }];
  const calls: Obj[] = []; let hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    const value = request.op === 'http' ? { authenticated: true, permissions: ['filesystem:read'] }
      : request.method === 'projects.listEntries' ? { entries: obj(request.payload).directoryPath === 'src' ? [{ kind: 'file', path: 'src/a.ts' }] : [{ kind: 'directory', path: 'src' }, { kind: 'file', path: 'README.md' }] }
      : request.method === 'projects.searchEntries' ? { entries: [{ kind: 'file', path: 'other/Needle.ts' }], truncated: true }
      : request.method === 'projects.readFile' ? { contents: 'const a = 1;\r\n\tvalue\r\n', byteLength: 27, truncated: false } : {};
    return { ok: true, generation: client.generation, value };
  } };
  return { client, calls, native, hook(fn: typeof hook) { hook = fn; } };
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
