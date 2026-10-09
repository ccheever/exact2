import { expect, test } from 'bun:test';
import { mobileClearClientCaches } from './mobile-cache-controls';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';

function fixture() {
  const client = new T3Client(), calls: Obj[] = [];
  client.environmentId = 'cache-controls'; client.origin = 'https://cache-controls.invalid';
  client.threadId = 'thread'; client.projectId = 'project'; client.modelId = 'model';
  client.shell = { sequence: 1, projects: [{ id: 'project' }], threads: [{ id: 'thread' }] }; client.shellLoaded = true;
  client.config = { environment: { environmentId: client.environmentId } };
  client.local.drafts[client.draftKey] = 'keep draft';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); return { ok: true, generation: 1, value: { removed: 1 } };
  } };
  return { client, calls, native };
}
test('clear removes warm offline display and preserves selection, drafts and preferences', async () => {
  const f = fixture(), preferences = JSON.stringify(f.client.local), origin = f.client.origin;
  await mobileClearClientCaches(f.client, f.native, f.client.environmentId);
  expect(f.calls).toEqual([
    { op: 'mobileClientCache', action: 'clear', kind: 'project-favicon', environmentId: 'cache-controls' },
    { op: 'mobileClientCache', action: 'clear', environmentId: 'cache-controls' },
  ]);
  expect(f.client.shell.threads).toEqual([]); expect(f.client.shellLoaded).toBe(false); expect(f.client.config).toEqual({});
  expect(f.client.modelId).toBe(''); expect(f.client.threadId).toBe('thread'); expect(f.client.origin).toBe(origin);
  expect(JSON.stringify(f.client.local)).toBe(preferences); expect(f.client.ready).toBe(false);
});
test.each(['connected', 'other environment'])('clear preserves the %s display', async mode => {
  const f = fixture(), shell = f.client.shell, config = f.client.config;
  if (mode === 'connected') f.client.connection = 'connected';
  await mobileClearClientCaches(f.client, f.native, mode === 'connected' ? undefined : 'other');
  expect(f.client.shell).toBe(shell); expect(f.client.config).toBe(config);
  if (mode === 'connected') expect(f.calls).toEqual([
    { op: 'mobileClientCache', action: 'clearKind', kind: 'project-favicon' }, { op: 'mobileClientCache', action: 'clear' },
  ]);
});
test('failed icon clear reports failure and does not claim the general store was cleared', async () => {
  const f = fixture(), shell = f.client.shell;
  f.native.later = async input => { f.calls.push(obj(input)); return { ok: false, generation: 1, error: { kind: 'Disk', message: 'Disk failed' } }; };
  await expect(mobileClearClientCaches(f.client, f.native)).rejects.toThrow('Could not clear cached project icons.');
  expect(f.calls).toHaveLength(1); expect(f.client.shell).toBe(shell);
});


test('a delayed clear completion cannot erase fresh snapshots from a newer connection', async () => {
  const f = fixture(), entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>(), original = f.native.later;
  f.native.later = async input => {
    const result = await original(input);
    if (obj(input).action === 'clear') { entered.resolve(); await release.promise; }
    return result;
  };
  const pending = mobileClearClientCaches(f.client, f.native);
  await entered.promise;
  f.client.adoptStatus({ state: 'connected', origin: f.client.origin, environmentId: f.client.environmentId, message: '' }, 2);
  const fresh = { sequence: 100, projects: [], threads: [{ id: 'fresh' }] };
  f.client.shell = fresh; f.client.shellLoaded = true;
  f.client.adoptStatus({ state: 'disconnected', origin: f.client.origin, environmentId: f.client.environmentId, message: '' }, 2);
  release.resolve(); await pending;
  expect(f.client.shell).toBe(fresh); expect(f.client.shellLoaded).toBe(true);
});
