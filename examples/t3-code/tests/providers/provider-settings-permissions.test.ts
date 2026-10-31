import { expect, test } from 'bun:test';
import { connected } from '../support/client-fixture';
import { obj } from '../../src/client/domain';
import { providerPage } from '../../src/providers/providers';
import { pickerCatalog } from '../../src/composer/model-catalog';

for (const permission of ['settings:write', 'providers:manage']) {
  test(`${permission} admits its provider-page writes and refuses the other grant's writes`, async () => {
    const { client, native, command } = await connected();
    client.scopes = ['orchestration:read', permission];
    obj(client.config.environment).capabilities = { projectProviderInstanceOverrides: true };
    obj(client.config.settings).providerInstances = { 'codex-personal': { driver: 'codex', enabled: true } };
    const settings = permission === 'settings:write';
    expect(providerPage(client, '', 0, ['p1'])).toMatchObject({ writable: !settings, settingsWritable: settings, enableWritable: settings });
    expect(providerPage(client, '', 0).enableWritable).toBe(!settings);
    expect(pickerCatalog(client, '', '', () => ({ providerBadge: '', providerBadgeColor: '' })).favoriteWritable).toBe(settings);
    await command('favorite-model', 'codex-personal', 'model-a');
    await command('provider-model-hide', 'codex-personal', JSON.stringify({ key: 'model-b', value: 'true' }));
    await command('provider-model-move', 'codex-personal', JSON.stringify({ key: 'model-a', value: 'down' }));
    await command('provider-enabled', 'codex-personal', JSON.stringify({ value: 'false', projectIds: ['p1'] }));
    const beforeCrud = native.calls.filter(call => call.method === 'server.updateSettings');
    expect(beforeCrud.length).toBe(settings ? 4 : 0);
    if (settings) {
      expect(beforeCrud.map(call => obj(call.payload).patch)).toMatchObject([
        { setModelFavorites: [{ provider: 'codex-personal', model: 'model-a', favorite: true }] },
        { setModelsHidden: [{ provider: 'codex-personal', model: 'model-b', hidden: true }] },
        { setProviderModelOrder: { provider: 'codex-personal', modelOrder: [] } },
        { projectSettingsOverrides: { p1: { providerInstances: { 'codex-personal': { enabled: false } } } } },
      ]);
      expect(obj(native.config.settings)).toMatchObject({ favorites: [{ provider: 'codex-personal', model: 'model-a' }],
        projectSettingsOverrides: { p1: { providerInstances: { 'codex-personal': { enabled: false } } } } });
    }
    await command('provider-display', 'codex-personal', JSON.stringify({ value: 'Personal' }));
    expect(native.calls.filter(call => call.method === 'server.updateSettings').length - beforeCrud.length).toBe(settings ? 0 : 1);
  });
}
