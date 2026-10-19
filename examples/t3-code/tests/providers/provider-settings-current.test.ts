import { expect, test } from 'bun:test';
import { obj } from '../../src/client/domain';
import { connected } from '../support/client-fixture';
import { buildProviderInstanceOverridePatch } from '../../src/providers/project-provider-overrides';
import { providerPage, providerWizard } from '../../src/providers/providers';
import { formatProviderUpdateRequiredNotice } from '../../src/providers/provider-update-required-models';
import { providerRowLabel } from '../../src/composer/model-catalog';
import { mergeUsage, providersWithUsage } from '../../src/usage/pages-usage';

test('project provider switches update every checkout without changing environment enablement', async () => {
  const { client, native, command } = await connected();
  obj(client.config.environment).capabilities = { ...obj(obj(client.config.environment).capabilities), projectProviderInstanceOverrides: true };
  obj(client.config.settings).providerInstances = { 'codex-personal': { driver: 'codex', enabled: true } };
  obj(client.config.settings).projectSettingsOverrides = { p1: { defaultRuntimeMode: 'full-access' } };
  await command('provider-enabled', 'codex-personal', JSON.stringify({ value: 'false', projectIds: ['p1', 'p2'] }));
  expect(obj(client.config.settings).projectSettingsOverrides).toEqual({
    p1: { defaultRuntimeMode: 'full-access', providerInstances: { 'codex-personal': { enabled: false } } },
    p2: { providerInstances: { 'codex-personal': { enabled: false } } },
  });
  expect(providerPage(client, 'codex-personal', 0, ['p1']).rows.find(row => row.id === 'codex-personal')).toMatchObject({ enabled: false, projectOverride: true });
  expect(providerPage(client, 'codex-personal', 0).rows.find(row => row.id === 'codex-personal')?.enabled).toBe(true);
  await command('provider-enabled', 'codex-personal', JSON.stringify({ value: 'inherit', projectIds: ['p1', 'p2'] }));
  expect(obj(native.config.settings).projectSettingsOverrides).toEqual({ p1: { defaultRuntimeMode: 'full-access', providerInstances: {} } });
  expect(buildProviderInstanceOverridePatch({ projectSettingsOverrides: { p: { providerInstances: { other: { enabled: true } } } } }, ['p'], 'codex', false)).toEqual({
    p: { providerInstances: { other: { enabled: true }, codex: { enabled: false } } },
  });
});

test('local ACP creation skips registry preparation and preserves literal arguments', async () => {
  const { client, native, command } = await connected();
  providerWizard(client, true, 994, 'acpRegistry', false, '', false, '');
  const commandArgs = ['--acp', '  spaced value  ', '', '$HOME'];
  await command('provider-add', 'acp-local', JSON.stringify({ driver: 'acpRegistry', label: 'Local ACP', fields: { source: 'local', commandPath: '/bin/local-acp' }, commandArgs }));
  expect(obj(obj(obj(native.config.settings).providerInstances)['acp-local']).config).toMatchObject({ source: 'local', commandPath: '/bin/local-acp', commandArgs });
  expect(native.calls.filter(call => call.method === 'server.prepareAcpRegistryAgent')).toEqual([]);
  expect(providerWizard(client, true, 994, 'acpRegistry', false, '', false, '').created).toBe('');
  await command('provider-argument-value', 'acp-local', JSON.stringify({ key: '1', value: ' literal [text](path) ' }));
  expect(obj(obj(obj(obj(native.config.settings).providerInstances)['acp-local']).config).commandArgs).toEqual(['--acp', ' literal [text](path) ', '', '$HOME']);
});

test('CLI unlock notices filter models and choose the highest required version', () => {
  const provider = { driver: 'codex', updateRequiredModels: [
    { slug: 'one', name: 'One', minVersion: '1.2.0' }, { slug: 'two', name: 'Two', minVersion: '1.10.0' },
  ] };
  expect(formatProviderUpdateRequiredNotice(provider)).toBe('Update the Codex CLI to v1.10.0 or newer to use One and Two.');
  expect(formatProviderUpdateRequiredNotice(provider, 'ONE')).toBe('Update the Codex CLI to v1.2.0 or newer to use One.');
  expect(formatProviderUpdateRequiredNotice(provider, 'absent')).toBeNull();
  expect(providerRowLabel('OpenCode', 'OpenCode Go')).toBe('OpenCode Go');
  expect(providerRowLabel('OpenCode', 'Zen')).toBe('OpenCode · Zen');
  const merged = mergeUsage([]);
  merged.providers = [{ provider: 'pi', costUsd: 0, totalTokens: 100, sessions: 1, costShare: 0, tokenShare: 1 }];
  expect(providersWithUsage(merged)).toEqual(['pi']);
});
