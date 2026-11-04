import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj } from './shared/domain';
import { type Native } from './shared/protocol';
import { markdownMediaUrls } from './shared/media-views';
import { workspaceMutationId } from './workspace-mutation';

function fixture() {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://environment.example', environmentId: 'env', generation: 1,
    threadId: 'thread', connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { thread: { id: 'thread' }, visibleTurnItems: [{ item: { text: '![diagram](/repo/diagram.png)' } }], checkpoints: [] } };
  let signature = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input);
    return { ok: true, generation: client.generation, value: request.method === 'assets.createUrl'
      ? { relativeUrl: `/signed/diagram.png?sig=${++signature}` } : {} };
  } };
  return { client, native, signatures: () => signature };
}

describe('mobile workspace media invalidation', () => {
  test('workspaceMutationId ignores assistant text and in-flight work', () => {
    const f = fixture();
    expect(workspaceMutationId(f.client)).toBeNull();
    f.client.projection.visibleTurnItems.push({ item: { id: 'run-1', type: 'command_execution', status: 'running' } });
    expect(workspaceMutationId(f.client)).toBeNull();
    f.client.projection.visibleTurnItems.push({ item: { id: 'run-2', type: 'command_execution', status: 'completed' } });
    expect(workspaceMutationId(f.client)).toBe('["run-2",null]');
  });

  test('workspace media is signed once per mutation and re-signed after a settled change', async () => {
    const f = fixture();
    const first = await markdownMediaUrls(f.client, f.native, '/repo', 1, workspaceMutationId(f.client));
    expect(first[0]?.url).toContain('sig=1');
    expect(f.signatures()).toBe(1);
    const same = await markdownMediaUrls(f.client, f.native, '/repo', 2, workspaceMutationId(f.client));
    expect(same[0]?.url).toBe(first[0]?.url);
    expect(f.signatures()).toBe(1);
    f.client.connection = 'disconnected';
    const offline = await markdownMediaUrls(f.client, f.native, '/repo', 3, workspaceMutationId(f.client));
    expect(offline[0]?.url).toBe(first[0]?.url);
    expect(f.signatures()).toBe(1);
    f.client.connection = 'connected';
    const resumed = await markdownMediaUrls(f.client, f.native, '/repo', 4, workspaceMutationId(f.client));
    expect(resumed[0]?.url).toContain('sig=2');
    expect(f.signatures()).toBe(2);
    f.client.projection.visibleTurnItems.push({ item: { id: 'run-1', type: 'file_change', status: 'completed' } });
    const changed = await markdownMediaUrls(f.client, f.native, '/repo', 5, workspaceMutationId(f.client));
    expect(changed[0]?.url).toContain('sig=3');
    expect(f.signatures()).toBe(3);
  });
});
