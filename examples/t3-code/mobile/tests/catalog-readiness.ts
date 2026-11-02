import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { answer } from '../app';
import { value } from '../../tests/support/contract-action-fixture';
import { mobileSnapshotRead as read } from '../client';
import { obj } from '../shared/domain';
import type { Files, Native } from '../shared/protocol';
import { fleet } from '../shared/settings-b-fleet';

const storage: Files = { fs: {
  async mkdir() { throw new Error('Unexpected storage call'); },
  async readFile() { throw new Error('Unexpected storage call'); },
  async atomicWriteFile() { throw new Error('Unexpected storage call'); },
} };
const expression = /^  resource home = (.*) as shape HomeView$/m.exec(readFileSync(new URL('../app.contract', import.meta.url), 'utf8'))?.[1] ?? assert.fail('The Home resource is missing.');
function expectCatalog(loaded: boolean) {
  const data = answer('snapshot', ['light', 't3-code', 98, 1000], undefined, undefined);
  const home = obj(value(expression, {
    data, snapshotRead: { serial: 98, catalogLoaded: true }, wallTime: { epochAtZero: 0 }, elapsed: 1000,
    performanceNow: () => 1000, max: Math.max, homeQuery: '', settledCount: 10,
    shelves: { loaded: false, workingEnabled: false, workingExpanded: false, snoozedExpanded: false, settledExpanded: false },
    chrome: { environmentId: '', projectKey: '' }, appearance: { groupingMode: 'repository' }, nav: [],
    top: () => ({ id: 'startup', name: 'home' }), workspace: { usesSplitView: false, sidebarVisible: false },
    homeView: (...args: unknown[]) => answer('homeView', args, undefined, undefined),
  }));
  assert.equal(home.emptyTitle, loaded ? 'No environments connected' : 'Loading environments');
  assert.equal(home.loading, !loaded);
}
expectCatalog(false);
const bake = await read(undefined, storage);
assert.ok(bake.serial > 0);
expectCatalog(false);
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
  await read(native, storage); expectCatalog(false);
  mode = 'malformed';
  await read(native, storage); expectCatalog(false);
  mode = 'held';
  const stale = read(native, storage); await entered.promise;
  mode = 'failed';
  const latest = await read(native, storage); release.resolve();
  assert.deepEqual(await stale, latest);
  expectCatalog(false);
  mode = 'loaded';
  await read(native, storage); expectCatalog(true);
  assert.deepEqual(fleet.saved, []);
  mode = 'failed';
  await read(native, storage); expectCatalog(true);
  await read(undefined, storage); expectCatalog(true);
  cancelPreferences = true;
  await assert.rejects(read(native, storage), (error: unknown) => obj(error).kind === 'superseded');
} finally { release.resolve(); }
console.log('Catalog readiness verified.');
