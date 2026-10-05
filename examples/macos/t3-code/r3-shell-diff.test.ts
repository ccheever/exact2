import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { snapshot } from './presentation';
import { diffSnapshot } from './diff';
import { obj, type Obj } from './domain';
import type { Native, Files } from './protocol';

// Upstream d1034d62b2: the diff panel opens on Changes (the branch since its base), not Uncommitted.
const patch = 'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,1 +1,1 @@\n-one\n+two\n';
function harness() {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'request' && request.method === 'review.getDiffPreview') return { ok: true, generation: 1, value: { cwd: '/repo', sources: [
      { kind: 'working-tree', diff: '', truncated: false }, { kind: 'branch-range', diff: patch, truncated: false }] } };
    return { ok: true, generation: 1, value: {} };
  } };
  let saved = '';
  const storage: Files = { fs: { async mkdir() {}, async readFile() { if (!saved) throw new Error('missing'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { saved = new TextDecoder().decode(bytes); } } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', environmentId: 'env', projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:read', 'orchestration:operate'], config: { environment: { capabilities: {} } } });
  client.shell.projects = [{ id: 'p1', title: 'Fixture', workspaceRoot: '/repo' }];
  client.thread = { projection: { thread: { id: 't1' }, runtimeRequests: [], turnItems: [], runs: [], checkpoints: [] }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  return { client, command: (op: string, id = '', value = '') => client.command(op, id, value, 0, native, storage) };
}

describe('Changes is the default diff scope', () => {
  test('a generic open selects Changes, even after Uncommitted was chosen; labels and loading text', async () => {
    const { client, command } = harness();
    expect(diffSnapshot(client, 0)).toMatchObject({ diffScope: 'branch', diffScopeLabel: 'Changes', diffLoadingLabel: 'Loading changes...' });
    await command('diff');
    expect(snapshot(client)).toMatchObject({ diffOpen: true, diffScope: 'branch', diffScopeLabel: 'Changes', diffAdditions: 1, diffDeletions: 1 });
    await command('diff-scope', '', 'unstaged');
    expect(snapshot(client)).toMatchObject({ diffScopeLabel: 'Uncommitted', diffLoadingLabel: 'Loading uncommitted changes...', diffEmpty: true });
    await command('close-diff');
    await command('diff');
    expect(snapshot(client)).toMatchObject({ diffScope: 'branch', diffScopeLabel: 'Changes' });
  });
});
