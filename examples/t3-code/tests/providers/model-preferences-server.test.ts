import { expect, test } from 'bun:test';
import { modelPreferenceSource, modelPreferencesMigrationPatch, runEnvironmentModelPrefOp, toggleModelFavorite } from '../../src/providers/model-preferences';
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
    calls.push({ method, payload }); return method === 'server.getSettings' ? config.settings as Obj : method === 'server.getConfig' ? config : {};
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
