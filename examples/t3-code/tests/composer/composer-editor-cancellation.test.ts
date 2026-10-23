import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import { letGo, letGoAware } from '../../src/client/let-go';
import { ClientError, type Native } from '../../src/client/protocol';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerEditorView } from '../../src/composer/composer-editor';
import { composerFieldObserved } from '../../src/composer/composer-field-bridge';
import { Backend, storage } from '../support/client-fixture';

type Boundary = 'editorPoke' | 'projects.searchEntries';
const path = 'src/__name__/file.ts', query = `@${path}`;
const aborted = () => Object.assign(new Error('The answer was let go before this reply.'), { name: 'FetchError', kind: 'Aborted' });
async function opened() {
  const client = new T3Client(), backend = new Backend(), disk = storage();
  backend.scopes.push('filesystem:read');
  await client.refresh(backend, disk.files);
  await client.command('select-thread', 't1', '', 0, backend, disk.files);
  await client.refresh(backend, disk.files);
  const owner = composerFieldOwner(client);
  composerFieldObserved(client, [{ generation: client.generation, snapshotOwner: client.snapshotOwner, owner, value: query,
    start: query.length, end: query.length, composing: false, live: true, focused: true, inputSerial: 1 }, true], owner);
  const calls: Obj[] = [];
  const view = (failure?: { boundary: Boundary; error: Error }, entries = [{ path, kind: 'file' }]) => {
    const native: Native = { available: true, watch: topic => backend.watch(topic), later: async request => {
      const value = obj(request); calls.push(value);
      const boundary = value.op === 'request' ? value.method : value.op;
      if (failure && boundary === failure.boundary) throw failure.error;
      if (value.op === 'request' && value.method === 'projects.searchEntries') return backend.good({ entries, truncated: false });
      return backend.later(request);
    } };
    return composerEditorView(client, letGoAware(native));
  };
  const searches = () => calls.filter(call => call.method === 'projects.searchEntries');
  return { view, searches };
}

describe('composer path search cancellation', () => {
  for (const boundary of ['editorPoke', 'projects.searchEntries'] satisfies Boundary[]) {
    test(`a let-go at ${boundary} rejects without poisoning the next answer`, async () => {
      const { view, searches } = await opened();
      if (boundary !== 'editorPoke') expect((await view()).menu.loading).toBe(true);
      let failure: unknown;
      try { await view({ boundary, error: aborted() }); } catch (error) { failure = error; }
      let recovered = await view();
      if (recovered.menu.loading) recovered = await view();
      expect(recovered.menu.rows.map(row => row.id)).toEqual([`path:file:${path}`]);
      expect(letGo(failure)).toBe(true);
      expect(searches().length).toBe(boundary === 'projects.searchEntries' ? 2 : 1);
    });
  }
  test('a superseded search does not become a cached empty search result', async () => {
    const { view, searches } = await opened();
    await view();
    const error = new ClientError('This operation was superseded.', 'superseded');
    await view({ boundary: 'projects.searchEntries', error }).catch(() => undefined);
    expect((await view()).menu.rows.map(row => row.id)).toEqual([`path:file:${path}`]);
    expect(searches().length).toBe(2);
  });
  for (const boundary of ['editorPoke'] satisfies Boundary[]) {
    test(`an ordinary ${boundary} error retains its existing fallback`, async () => {
      const { view, searches } = await opened();
      expect((await view({ boundary, error: new Error('Editor unavailable') })).menu.loading).toBe(true);
      expect((await view()).menu.rows.map(row => row.id)).toEqual([`path:file:${path}`]);
      expect(searches().length).toBe(1);
    });
  }
  test('a real search failure remains a cached failure without an automatic retry', async () => {
    const { view, searches } = await opened();
    await view();
    const failed = await view({ boundary: 'projects.searchEntries', error: new Error('Search failed') });
    expect(failed.menu.loading).toBe(false);
    expect(failed.menu.rows).toEqual([]);
    expect(failed.menu.emptyText).toBe('No matching files or folders.');
    expect((await view()).menu.rows).toEqual([]);
    expect(searches().length).toBe(1);
  });
  test('a successful empty search stays cached', async () => {
    const { view, searches } = await opened();
    await view();
    expect((await view(undefined, [])).menu.rows).toEqual([]);
    expect((await view()).menu.rows).toEqual([]);
    expect(searches().length).toBe(1);
  });
});
