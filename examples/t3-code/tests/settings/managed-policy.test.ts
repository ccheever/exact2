// An administrator's managed policy (ServerConfig.managedPolicy): the helpers, a locked settings row, the
// About rows, and the provider-instance locks, each from the config a server reports to the view it draws.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { managedCapabilityDenial, type ManagedPolicySummary } from '../../src/client/managed-policy';
import { firstManagedDenial, managedByOrganization, managedInstanceLocks, managedSettingReason, managedTelemetryOffMessage } from '../../src/settings/managed-policy';
import { generalSections, resolveScope, serverContext } from '../../src/settings/settings-core';
import { providerPage, providerWizard, type ProviderHost } from '../../src/providers/providers';
import { agentRows } from '../../src/pages/pages-welcome';
import { fakeClient } from './settings-a.test';

const policy: ManagedPolicySummary = {
  organization: 'Example Corp',
  lockedSettings: [],
  limits: { providers: { lockedInstanceFields: { codex: ['config.binaryPath', 'environment.OPENAI_BASE_URL', 'environment.HTTPS_PROXY'] } } },
};

describe('managed policy: the web helpers', () => {
  test('a row is managed when any target locks one of its paths', () => {
    expect(managedSettingReason([undefined, { ...policy, lockedSettings: ['defaultRuntimeMode'] }], ['defaultRuntimeMode'])).toBe('Managed by Example Corp.');
    expect(managedSettingReason([{ ...policy, lockedSettings: ['worktreeCleanup'] }], ['worktreeCleanup', 'storageCleanup.worktreeOnMerge'])).toBe('Managed by Example Corp.');
    expect(managedSettingReason([{ ...policy, lockedSettings: ['storageCleanup.logsAfterDays'] }], ['storageCleanup.browserArtifactsAfterDays'])).toBeUndefined();
    expect(managedSettingReason([policy], [])).toBeUndefined();
  });

  test('the first denial among several targets, telemetry and the organization notice', () => {
    const limited: ManagedPolicySummary = { organization: 'Example Corp', supportUrl: 'https://help.example.com', lockedSettings: [], limits: { connect: false, telemetry: false } };
    expect(firstManagedDenial([undefined, policy, limited], entry => managedCapabilityDenial(entry, 'connect'))).toBe("Example Corp's policy does not allow T3 Connect.");
    expect(firstManagedDenial([undefined, policy], entry => managedCapabilityDenial(entry, 'connect'))).toBeUndefined();
    expect(managedTelemetryOffMessage(limited)).toBe('Example Corp turned off anonymous usage data.');
    expect(managedTelemetryOffMessage(policy)).toBeUndefined();
    expect(managedByOrganization(limited)).toEqual({ text: 'This environment is managed by Example Corp.', helpUrl: 'https://help.example.com' });
    expect(managedByOrganization(policy).helpUrl).toBe('');
  });

  // managedPolicy.test.ts (upstream), as is.
  test('managedInstanceLocks splits locked instance paths into config keys and environment names', () => {
    const locks = managedInstanceLocks(policy, 'codex');
    expect(locks && [...locks.configKeys]).toEqual(['binaryPath']);
    expect(locks && [...locks.environmentNames]).toEqual(['OPENAI_BASE_URL', 'HTTPS_PROXY']);
    expect(locks?.reason).toBe('Managed by Example Corp.');
    const ceilingPolicy = { ...policy, limits: { providers: { lockedInstanceFields: { codex: ['config.binaryPath', 'environment'] } } } };
    expect(managedInstanceLocks(ceilingPolicy, 'codex')?.wholeEnvironment).toBe(true);
    expect(managedInstanceLocks(policy, 'codex')?.wholeEnvironment).toBe(false);
    expect(managedInstanceLocks(policy, 'claudeAgent')).toBeUndefined();
    expect(managedInstanceLocks(undefined, 'codex')).toBeUndefined();
  });
});

describe('managed policy: settings rows', () => {
  const rowsOf = (managedPolicy: Obj) => {
    const client = fakeClient();
    client.scopes = ['settings:write'];
    (client.config as Obj).managedPolicy = managedPolicy;
    const as = client as unknown as T3Client;
    return generalSections(as, serverContext(as, resolveScope(as, '', '', ''), new Map())).flatMap(section => section.rows);
  };

  test('a locked setting is inert with the policy as its reason, wins over other reasons, and offers no reset', () => {
    const rows = rowsOf({ organization: 'Example Corp', lockedSettings: ['defaultRuntimeMode', 'autoResumeLimitedThreads'] });
    expect(rows.find(row => row.id === 'default-permissions')).toMatchObject({ inert: true, note: 'Managed by Example Corp.', resettable: false });
    expect(rows.find(row => row.id === 'auto-resume-limited-threads')).toMatchObject({ inert: true, note: 'Managed by Example Corp.' });
    expect(rows.find(row => row.id === 'new-threads')).toMatchObject({ inert: false, note: '' });
    // Unmanaged, the same row is the user's.
    expect(rowsOf({}).find(row => row.id === 'default-permissions')).toMatchObject({ inert: false, note: '' });
  });

  test('a runtime-mode ceiling keeps broader permissions listed, disabled with the reason', () => {
    const options = rowsOf({ organization: 'Example Corp', lockedSettings: [], limits: { maxRuntimeMode: 'auto-accept-edits' } })
      .find(row => row.id === 'default-permissions')!.options;
    expect(options.map(option => [option.value, option.disabled, option.detail])).toEqual([
      ['approval-required', false, 'Ask before commands and file changes.'],
      ['auto-accept-edits', false, 'Auto-approve edits, ask before other actions.'],
      ['auto', true, 'Example Corp limits threads to Auto-accept edits.'],
      ['full-access', true, 'Example Corp limits threads to Auto-accept edits.'],
    ]);
  });

  test('About names the organization and its help page, and follows the update policy', () => {
    const rows = rowsOf({ organization: 'Example Corp', supportUrl: 'https://help.example.com', lockedSettings: [], limits: { updates: { mode: 'off', allowNightly: false } } });
    expect(rows.find(row => row.id === 'organization')).toMatchObject({ title: 'Organization', kind: 'external-link', label: 'Get help', target: 'https://help.example.com',
      description: 'This environment is managed by Example Corp. Some settings and features follow its policy.' });
    expect(rows.find(row => row.id === 'version')).toMatchObject({ inert: true, note: 'Example Corp manages T3 Code updates.', description: 'Example Corp manages T3 Code updates.' });
    const track = rows.find(row => row.id === 'update-track')!;
    expect(track).toMatchObject({ inert: true, description: 'Example Corp manages T3 Code updates. Nightly builds are not available.' });
    expect(track.options.find(option => option.value === 'nightly')?.disabled).toBe(true);
    expect(rowsOf({}).some(row => row.id === 'organization')).toBe(false);
  });
});

describe('managed policy: provider instances', () => {
  const host = (managedPolicy: Obj): ProviderHost => ({
    ready: true, writable: true, local: { favoriteModels: [] }, async rpc() { return {}; },
    config: { environment: { label: 'Studio' }, managedPolicy, providers: [{ instanceId: 'codex', driver: 'codex', enabled: true, status: 'ready', auth: { status: 'authenticated' } }],
      settings: { providerInstances: { codex: { driver: 'codex', enabled: true, config: { binaryPath: '/opt/codex', setupMode: 'existing' },
        environment: [{ name: 'OPENAI_BASE_URL', value: 'https://llm.example.com', sensitive: false }, { name: 'MY_FLAG', value: '1', sensitive: false }] },
        claudeAgent: { driver: 'claudeAgent', enabled: false } } } },
  });

  test('pinned config keys and variables are inert with the reason; the rest stay editable', () => {
    const editor = providerPage(host(policy as unknown as Obj), 'codex', 0).editors[0]!;
    expect(editor.fields.find(field => field.key === 'binaryPath')?.managed).toBe('Managed by Example Corp.');
    expect(editor.fields.filter(field => field.key !== 'binaryPath').every(field => field.managed === '')).toBe(true);
    expect(editor.variables.map(variable => [variable.name, variable.managed])).toEqual([['OPENAI_BASE_URL', 'Managed by Example Corp.'], ['MY_FLAG', '']]);
    expect(editor).toMatchObject({ envManaged: '', argsManaged: '' });
    const ceiling = { organization: 'Example Corp', lockedSettings: [], limits: { providers: { lockedInstanceFields: { codex: ['environment'] } } } };
    expect(providerPage(host(ceiling), 'codex', 0).editors[0]!.envManaged).toBe('Managed by Example Corp.');
  });

  test('a driver the policy leaves out cannot be switched on, is not offered to add, and the welcome skips it', () => {
    const onlyCodex = { organization: 'Example Corp', lockedSettings: [], limits: { providers: { allowedDrivers: ['codex'] } } };
    const page = providerPage(host(onlyCodex), 'codex', 0);
    expect(page.rows.map(row => [row.id, row.managedDriver])).toEqual([['codex', ''], ['claudeAgent', "Example Corp's policy does not allow the Claude provider."]]);
    expect(page.canAdd).toBe(true);
    const wizard = providerWizard(host(onlyCodex), true, 1, 'claudeAgent', false, '', false, '');
    expect([wizard.driver, wizard.drivers.map(driver => driver.id), wizard.moreDrivers, wizard.acpAllowed]).toEqual(['codex', ['codex'], [], false]);
    expect(agentRows(host(onlyCodex).config).map(agent => agent.driver)).toEqual(['codex']);
    expect(providerPage(host({}), 'codex', 0).rows.every(row => row.managedDriver === '')).toBe(true);
  });

  test('a policy owning Codex setupMode rules out Continue with ChatGPT', () => {
    const pinned = { organization: 'Example Corp', lockedSettings: [], limits: { providers: { lockedInstanceFields: { codex: ['config.setupMode'] } } } };
    expect(providerWizard(host(pinned), true, 2, 'codex', false, '', false, '').chatgptAllowed).toBe(false);
    expect(providerWizard(host({}), true, 3, 'codex', false, '', false, '').chatgptAllowed).toBe(true);
  });

  test('renaming a variable to a pinned name is refused', async () => {
    const { runProviderOp } = await import('../../src/providers/providers');
    const server = host(policy as unknown as Obj);
    const writes: string[] = [];
    server.rpc = async (_native, method) => { if (method !== 'server.getConfig') writes.push(method); return method === 'server.getConfig' ? server.config : {}; };
    expect(await runProviderOp(server, {} as Native, 'provider-env-name', 'codex', JSON.stringify({ key: '1', value: 'HTTPS_PROXY' }))).toBe('');
    expect(writes).toEqual([]);
    expect(providerPage(server, 'codex', 0).editors[0]!.variables.map(variable => variable.name)).toEqual(['OPENAI_BASE_URL', 'MY_FLAG']);
    await runProviderOp(server, {} as Native, 'provider-env-name', 'codex', JSON.stringify({ key: '1', value: 'OTHER_FLAG' }));
    expect(writes).toContain('server.updateSettings');
  });
});
