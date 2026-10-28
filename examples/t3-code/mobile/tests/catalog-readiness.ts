import assert from 'node:assert/strict';
import { mobileSnapshotRead as read } from '../client';
import { obj } from '../shared/domain';
import type { Files, Native } from '../shared/protocol';
import { fleet } from '../shared/settings-b-fleet';

const storage: Files = { fs: {
  async mkdir() { throw new Error('Unexpected storage call'); },
  async readFile() { throw new Error('Unexpected storage call'); },
  async atomicWriteFile() { throw new Error('Unexpected storage call'); },
} };
const bake = await read(undefined, storage);
assert.ok(bake.serial > 0);
assert.equal(bake.catalogLoaded, false);
let mode: 'failed' | 'held' | 'malformed' | 'loaded' = 'failed', cancelPreferences = false;
const entered = Promise.withResolvers<void>(), release = Promise.withResolvers<void>();
const failure = { ok: false, generation: 0, error: { kind: 'test', message: 'Unavailable for this read.' } };
const native: Native = { available: true, watch() {}, async later(input) {
  const op = obj(input).op;
  if (op === 'connectionPreferences') {
    if (cancelPreferences) throw { name: 'FetchError', kind: 'Aborted' };
    throw new Error('Preferences unavailable.');
  }
  if (op !== 'environments') return failure;
  if (mode === 'held') { entered.resolve(); await release.promise; return { ok: true, generation: 0, value: { saved: [] } }; }
  if (mode === 'malformed') return { ok: true, generation: 0, value: { saved: {} } };
  return mode === 'loaded' ? { ok: true, generation: 0, value: { saved: [] } } : failure;
} };
try {
  assert.equal((await read(native, storage)).catalogLoaded, false);
  mode = 'malformed';
  assert.equal((await read(native, storage)).catalogLoaded, false);
  mode = 'held';
  const stale = read(native, storage); await entered.promise;
  mode = 'failed';
  const latest = await read(native, storage); release.resolve();
  assert.deepEqual(await stale, latest);
  assert.equal(latest.catalogLoaded, false);
  mode = 'loaded';
  assert.equal((await read(native, storage)).catalogLoaded, true);
  assert.deepEqual(fleet.saved, []);
  mode = 'failed';
  assert.equal((await read(native, storage)).catalogLoaded, true);
  assert.equal((await read(undefined, storage)).catalogLoaded, true);
  cancelPreferences = true;
  await assert.rejects(read(native, storage), error => obj(error).kind === 'superseded');
} finally { release.resolve(); }
console.log('Catalog readiness verified.');
