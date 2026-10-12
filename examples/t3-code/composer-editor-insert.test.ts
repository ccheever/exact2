import { describe, expect, test } from 'bun:test';
import { insertContext, localId } from './composer-editor';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';

// The insert-context op other lanes call (sidebar drags, citations): what it
// asks the native editor to insert at the caret.
function harness() {
  const inserted: string[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input) as Obj;
    if (request.op === 'editorInsert') inserted.push(String(request.text));
    return { ok: true, generation: 1, value: { applied: true } };
  } };
  const client = { shell: { threads: [{ id: 'abc', title: 'Fix [the] build' }] } } as unknown as T3Client;
  return { native, client, inserted };
}

describe('insert-context', () => {
  test('a thread becomes a kind-scoped context link labelled by its title', async () => {
    const { native, client, inserted } = harness();
    await insertContext(client, native, 'thread', 'abc');
    expect(inserted).toEqual(['[Fix the build](t3-context://v1/thread/thread_abc)']);
    await expect(insertContext(client, native, 'thread', 'missing')).rejects.toThrow('Use threads from this environment');
  });
  test('files become file links, folders @mentions (quoted with spaces)', async () => {
    const { native, client, inserted } = harness();
    await insertContext(client, native, 'file', 'src/app (1).ts');
    await insertContext(client, native, 'folder', 'docs/');
    await insertContext(client, native, 'folder', 'my docs');
    expect(inserted).toEqual(['[app (1).ts](src/app%20%281%29.ts)', '@docs', '@"my docs"']);
  });
  test('citations insert an Assistant quote link and refuse other hrefs', async () => {
    const { native, client, inserted } = harness();
    await insertContext(client, native, 'citation', 't3-citation://v1/env/t1/m1?text=hi&start=0&end=2');
    expect(inserted).toEqual(['[Assistant quote](t3-citation://v1/env/t1/m1?text=hi&start=0&end=2)']);
    await expect(insertContext(client, native, 'citation', 'https://example.com')).rejects.toThrow('Unable to add to chat');
    await expect(insertContext(client, native, 'image', 'x')).rejects.toThrow('Unable to add to chat');
  });
});

describe('local ids', () => {
  test('are v4 UUIDs from crypto (data sources refuse Math.random)', () => {
    const real = Math.random;
    Math.random = () => { throw new Error('Math.random() is unavailable in data sources'); };
    try {
      const id = localId();
      expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
      expect(localId()).not.toBe(id);
    } finally { Math.random = real; }
  });
});
