import { expect, test } from 'bun:test';
import { migrateModelPreferences, modelPreferenceSource, modelPreferencesMigrationPatch, runEnvironmentModelPrefOp, toggleModelFavorite } from '../../src/providers/model-preferences';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
const native = { available: true, watch() {}, later: async () => ({}) } as Native;
const local = { favoriteModels: [JSON.stringify(['codex', 'm'])], providerModelPreferences: { codex: { hiddenModels: ['old'], modelOrder: ['m'] } } };
test('server preferences, including empty lists, override the legacy device seed', () => {
  expect(modelPreferenceSource({ config: { settings: { favorites: [], providerModelPreferences: {} } }, local })).toEqual({ favoriteModels: [], providerModelPreferences: {} });
  expect(modelPreferenceSource({ config: { settings: { favorites: null, providerModelPreferences: null } }, local })).toEqual(local);
  expect(modelPreferencesMigrationPatch({ favorites: [], providerModelPreferences: {} }, local)).toBeNull();
  expect(modelPreferencesMigrationPatch({ favorites: null, providerModelPreferences: null }, local)).toEqual({ migrateModelPreferences: {
    favorites: [{ provider: 'codex', model: 'm' }], providerModelPreferences: local.providerModelPreferences } });
});
test('favorite and visibility commands send atomic desired-state server patches', async () => {
  const calls: { method: string; payload: Obj }[] = [];
  const config: Obj = { environment: { environmentId: 'a' }, settings: { favorites: [], providerModelPreferences: {} } };
  const host = { config, local, async rpc(_native: Native, method: string, payload: Obj) {
    calls.push({ method, payload }); return method === 'server.getConfig' ? config : {};
  } };
  await toggleModelFavorite(host, native, 'codex', 'm');
  await toggleModelFavorite(host, native, 'codex', 'm');
  await runEnvironmentModelPrefOp(host, native, 'provider-model-hide', 'codex', 'm', 'true', [{ slug: 'm', isCustom: false }]);
  expect(calls.filter(call => call.method === 'server.updateSettings').map(call => call.payload.patch)).toEqual([
    { setModelFavorites: [{ provider: 'codex', model: 'm', favorite: true }] },
    { setModelFavorites: [{ provider: 'codex', model: 'm', favorite: false }] },
    { setModelsHidden: [{ provider: 'codex', model: 'm', hidden: true }] },
  ]);
  expect(local.favoriteModels).toEqual([JSON.stringify(['codex', 'm'])]);
});

// useSettings.favorites.browser.test.tsx "legacy model preferences migration": a server that predates
// migrateModelPreferences drops it and keeps answering null favorites; the client tries once per connection.
test.each(['succeeds', 'fails'])('the migration waits for a connection and retries only after a reconnect when the first save %s', async outcome => {
  const writes: string[] = [];
  const settings = { favorites: null, providerModelPreferences: null };
  const host = (environmentId: string) => ({ ready: false, generation: 0, local, config: { environment: { environmentId }, settings } as Obj,
    async rpc(_native: Native, method: string) {
      if (method === 'server.updateSettings') {
        writes.push(environmentId);
        if (outcome === 'fails' && writes.length === 1) throw new Error('save failed');
      }
      return method === 'server.getConfig' ? { environment: { environmentId }, settings } : {};
    } });
  const run = (target: ReturnType<typeof host>) => migrateModelPreferences(target, native).catch(() => {});
  const mine = host(`migration-${outcome}`), other = host(`other-${outcome}`);
  await run(mine); // not connected yet
  Object.assign(mine, { ready: true, generation: 1 });
  await run(mine);
  await run(mine); // another settings push on the same connection
  Object.assign(other, { ready: true, generation: 7 });
  await run(other);
  await run(mine);
  Object.assign(mine, { ready: false });
  await run(mine);
  Object.assign(mine, { ready: true, generation: 2 });
  await run(mine);
  expect(writes).toEqual([`migration-${outcome}`, `other-${outcome}`, `migration-${outcome}`]);
});
