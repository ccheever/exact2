// fileMetadata.ts / useFileMetadata.ts port: host paths, ordered batches, caching, and stale replies.
import { expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { metadataPath, refreshFileMetadata, fileMetadataNeeded, workspaceFileIcon, invalidateFileMetadata } from '../../src/shared/file-metadata';
import { messageChips } from '../../src/timeline/message-chips';
const native = { available: true } as Native;
function fixture(draft: string, answer: (paths: string[]) => unknown | Promise<unknown>) {
  const calls: string[][] = [];
  const app = { draft, draftKey: 'env:t1', generation: 1, environmentId: 'env', ready: true, projectId: 'p1', scopes: ['filesystem:read'],
    projection: { messages: [] }, shell: { projects: [{ id: 'p1', workspaceRoot: '/repo' }] },
    restAccess: () => ({ request: async (method: string, payload: { paths: string[] }) => { expect(method).toBe('filesystem.getMetadata'); calls.push(payload.paths); return answer(payload.paths); } }) } as unknown as T3Client;
  return { app, calls };
}
test('metadata paths are host paths, relative only with an explicit anchor, and bounded', () => {
  expect(metadataPath('src/../photo:12', '/repo')).toBe('/repo/photo');
  expect(metadataPath('~/a/../b')).toBe('~/b');
  expect(metadataPath('C:\\repo\\file.ts:12:3')).toBe('C:/repo/file.ts');
  expect(metadataPath('file:///repo/a%20b.ts#L12C3')).toBe('/repo/a b.ts');
  expect(metadataPath('relative')).toBeNull(); expect(metadataPath('https://example.test/x')).toBeNull();
  expect(metadataPath('/' + 'x'.repeat(512))).toBeNull();
});
test('server kind overrides dotted directories, and MIME identifies extensionless files', async () => {
  const { app, calls } = fixture('@release.v1 @capture @.env', paths => ({ entries: paths.map(path => path.endsWith('release.v1') ? { kind: 'directory' } : path.endsWith('capture') ? { kind: 'file', mimeType: 'image/png', byteLength: 42 } : { kind: 'file' }) }));
  await refreshFileMetadata(app, native, 100);
  expect(calls).toEqual([['/repo/release.v1', '/repo/capture', '/repo/.env']]);
  expect(workspaceFileIcon(app, 'release.v1', '/repo', 100)).toBe('folder');
  expect(workspaceFileIcon(app, 'capture', '/repo', 100)).toBe('image');
  expect(workspaceFileIcon(app, '.env', '/repo', 100)).not.toBe('folder');
  expect(fileMetadataNeeded(app, 100)).toBe(false);
});
test('sixty-four paths per ordered request, with duplicates shared across messages and the prompt', async () => {
  const { app, calls } = fixture(Array.from({ length: 70 }, (_, id) => `@p${id}`).join(' ') + ' @p1', paths => ({ entries: paths.map(() => ({ kind: 'file' })) }));
  await refreshFileMetadata(app, native, 0); await refreshFileMetadata(app, native, 0);
  expect(calls.map(paths => paths.length)).toEqual([64, 6]);
  expect(new Set(calls.flat()).size).toBe(70);
  expect(await refreshFileMetadata(app, native, 0)).toBe(false);
});
test('invalidation rejects a pre-write reply and the following query reads the new kind', async () => {
  let finish!: (value: unknown) => void;
  const { app, calls } = fixture('@entry', () => new Promise(resolve => { finish = resolve; }));
  const pending = refreshFileMetadata(app, native, 0);
  invalidateFileMetadata(app, 'entry', '/repo'); finish({ entries: [{ kind: 'directory' }] }); await pending;
  expect(workspaceFileIcon(app, 'entry', '/repo', 0)).not.toBe('folder'); expect(fileMetadataNeeded(app, 0)).toBe(true);
  const fresh = refreshFileMetadata(app, native, 0); finish({ entries: [{ kind: 'file' }] }); await fresh;
  expect(calls).toHaveLength(2); expect(fileMetadataNeeded(app, 0)).toBe(false);
});
test('metadata stays with its mounted thread, expires after leaving it, and never crosses a connection generation', async () => {
  const { app, calls } = fixture('@item', () => ({ entries: [{ kind: 'directory' }] }));
  await refreshFileMetadata(app, native, 0); expect(fileMetadataNeeded(app, 70_000)).toBe(false);
  Object.assign(app, { draftKey: 'env:t2' }); expect(fileMetadataNeeded(app, 70_000)).toBe(true);
  await refreshFileMetadata(app, native, 70_000);
  app.generation++; expect(fileMetadataNeeded(app, 70_001)).toBe(true);
  await refreshFileMetadata(app, native, 70_001); expect(calls).toHaveLength(3);
});
test('offline and unscoped paths never issue a metadata request', async () => {
  const { app, calls } = fixture('@folder', () => { throw Error('must not query'); });
  app.ready = false; expect(await refreshFileMetadata(app, native, 0)).toBe(false);
  app.ready = true; app.scopes = []; expect(await refreshFileMetadata(app, native, 0)).toBe(false);
  expect(calls).toEqual([]); expect(workspaceFileIcon(app, 'folder', '/repo', 0)).not.toBe('folder');
});
test('Markdown links and stored mention records share the same metadata and render resolved folder icons', async () => {
  const { app, calls } = fixture('', paths => ({ entries: paths.map(() => ({ kind: 'directory' })) }));
  const message = { text: '[docs](file:///repo/release.v1#L3) [folder](t3-context://v1/mention/entry)', context: { records: [{ kind: 'mention', contextId: 'entry', path: '/repo/release.v1', label: 'folder' }] } };
  app.projection.messages = [message];
  await refreshFileMetadata(app, native, 1);
  expect(calls).toEqual([['/repo/release.v1']]);
  expect(messageChips(message, '/repo', [], '', {}, app).map(chip => chip.icon)).toEqual(['folder', 'folder']);
});
test('failed batches remain bounded without retrying a mounted chip repeatedly before its backoff expires', async () => {
  const { app, calls } = fixture(Array.from({ length: 1100 }, (_, id) => `@p${id}`).join(' '), () => { throw new Error('Fixture metadata failure'); });
  for (let batch = 0; batch < 18; batch++) await refreshFileMetadata(app, native, 0);
  expect(calls).toHaveLength(18); expect(fileMetadataNeeded(app, 1)).toBe(false);
  expect(fileMetadataNeeded(app, 60_001)).toBe(true);
  Object.assign(app, { draftKey: 'env:t2', draft: '@p0' });
  expect(fileMetadataNeeded(app, 1)).toBe(true);
});
