import { test, expect } from 'bun:test';
import { providerRows, providerPage, providerWizard, acpRegistry, runProviderOp, providerFieldValues, healthInterval, type ProviderHost } from './providers';
import { withUpkeep } from './providers-upkeep';
import { providerSummary, versionAdvisory, checkedLabel, validateInstanceId } from './providers-meta';
import { obj, arr, str, type Obj } from './domain';
import type { Native } from './protocol';

const legacy = () => ({
  codex: { enabled: false, binaryPath: 'codex', homePath: '', shadowHomePath: '', launchArgs: '', customModels: [] },
  claudeAgent: { enabled: false, binaryPath: 'claude', homePath: '', customModels: [], launchArgs: '', autoCompactWindow: '' },
  cursor: { enabled: false, customModels: [] }, grok: { enabled: false, binaryPath: 'grok', customModels: [] },
  pi: { enabled: false, binaryPath: 'pi', launchArgs: '', customModels: [] },
  opencode: { enabled: false, binaryPath: 'opencode', serverUrl: '', serverPassword: '', customModels: [] },
  antigravity: { enabled: false, authMethod: 'oauth-personal', apiKey: '', gcpProject: '', gcpLocation: '', binaryPath: '', customModels: [] },
});
const fixtureProvider = { instanceId: 'exact_fixture', driver: 'codex', enabled: true, installed: true, version: '0.145.0', status: 'ready',
  auth: { status: 'authenticated', type: 'apiKey', label: 'OpenAI API Key' }, checkedAt: '2026-10-03T14:06:51.431Z',
  models: [{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna', isCustom: true, capabilities: { optionDescriptors: [{ id: 'reasoningEffort', type: 'select', options: [] }] } }],
  compatibilityAdvisory: { status: 'broken', message: 'This provider version is known to be incompatible with this T3 Code release. Use >=0.159.0.', recommendedVersion: null, recommendedRange: '>=0.159.0', latestVersionStatus: 'supported' },
  versionAdvisory: { status: 'behind_latest', latestVersion: '0.160.0', updateCommand: null } };

class FakeServer implements ProviderHost {
  ids?: (native: Native, count: number) => Promise<string[]>;
  ready = true; writable = true; local = { favoriteModels: [] as string[] };
  calls: Obj[] = [];
  settings: Obj = { providers: legacy(), providerInstances: { exact_fixture: { driver: 'codex', displayName: 'Exact verification fixture', enabled: true,
    config: { setupMode: 'existing', binaryPath: '/fixture/codex', homePath: '/fixture/home', customModels: [{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna' }] } } },
    backgroundActivity: { schemaVersion: 1, profile: 'balanced', overrides: {} }, cursorKeychainUsageEnabled: false };
  live: Obj[] = [{ instanceId: 'codex', driver: 'codex', enabled: false, status: 'disabled', auth: { status: 'unknown' }, checkedAt: '2026-10-03T14:06:51.431Z' },
    fixtureProvider, { instanceId: 'claudeAgent', driver: 'claudeAgent', enabled: false, status: 'disabled', auth: { status: 'unknown' }, checkedAt: '2026-10-03T14:06:51.431Z' },
    { instanceId: 'cursor', driver: 'cursor', enabled: false, status: 'disabled', auth: { status: 'unknown' }, setup: { canAuthenticate: true } }];
  config: Obj = {};
  constructor() { this.config = this.snapshot(); }
  snapshot(): Obj { return { environment: { label: 'Fixture Mac' }, settings: JSON.parse(JSON.stringify(this.settings)), providers: this.live }; }
  async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> {
    this.calls.push({ method, payload });
    if (method === 'server.getSettings') return JSON.parse(JSON.stringify(this.settings));
    if (method === 'server.getConfig') return this.snapshot();
    if (method === 'server.refreshProviders') return { providers: this.live };
    if (method === 'server.searchAcpRegistry') return { agents: [{ id: 'devin', name: 'Devin', version: '1.2.0', description: 'Devin CLI coding agent', website: 'https://docs.devin.ai/cli', repository: null, icon: 'https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg', distribution: 'binary' }] };
    if (method === 'server.prepareAcpRegistryAgent') return { agentId: payload.agentId, version: '1.2.0', distribution: 'binary', prepared: true };
    if (method === 'server.updateSettings') {
      const patch = obj(payload.patch);
      this.settings = { ...this.settings, ...patch };
      const mutation = obj(payload.providerInstanceMutation);
      if (mutation.operation) {
        const instances = { ...obj(this.settings.providerInstances) };
        const id = str(mutation.instanceId);
        if (mutation.operation === 'create' && id in instances) throw new Error(`Provider instance ${id} already exists.`);
        if (mutation.operation === 'remove') delete instances[id]; else instances[id] = mutation.instance;
        this.settings.providerInstances = instances;
      }
      return this.settings;
    }
    throw new Error(`unexpected ${method}`);
  }
}
const native = { available: true, watch() {}, later: async () => ({}) } as Native;
const writes = (server: FakeServer) => server.calls.filter(call => call.method === 'server.updateSettings');

test('rows follow T3 order: default slots, custom instances, visible cursor only with a live default', () => {
  const server = new FakeServer();
  expect(providerRows(obj(server.config.settings), server.live).map(row => [row.id, row.isDefault, row.isDirty])).toEqual([
    ['codex', true, true], ['exact_fixture', false, false], ['claudeAgent', true, true]]);
  const page = withUpkeep(server, providerPage(server, '', Date.parse('2026-10-03T14:07:20Z')));
  expect(page.rows.map(row => [row.name, row.status, row.enabled, row.version, row.update.show && row.update.warning])).toEqual([
    ['Codex', 'Disabled', false, '', false], ['Exact verification fixture', 'Authenticated · OpenAI API Key', true, 'v0.145.0', true], ['Claude', 'Disabled', false, '', false]]);
  expect(page.checked).toBe('Checked just now');
  expect(page.selectedId).toBe('codex');
  const editor = withUpkeep(server, providerPage(server, 'exact_fixture', 0)).editors[0]!;
  expect([editor.statusLead, editor.statusDetail, editor.canDelete, editor.canReset, editor.update.title]).toEqual(['Authenticated · OpenAI API Key', '· Incompatible', true, false, 'Known broken version']);
  expect(editor.fields.map(field => [field.key, field.value, field.placeholder])).toEqual([
    ['binaryPath', '/fixture/codex', 'codex'], ['homePath', '/fixture/home', '~/.codex'], ['shadowHomePath', '', '~/.codex-t3/personal'], ['launchArgs', '', '']]);
  expect(editor.modelSummary).toBe('1 model');
  expect(editor.models[0]).toMatchObject({ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna', custom: true, capabilities: 'Reasoning' });
  const codex = providerPage(server, 'codex', 0).editors[0]!;
  expect([codex.canDelete, codex.canReset, codex.statusLead, codex.title]).toEqual([false, true, 'Disabled', 'Codex']);
});

test('summary, advisory and checked labels mirror providerStatus.ts', () => {
  expect(providerSummary(undefined).headline).toBe('Checking provider status');
  expect(providerSummary({ enabled: true, installed: false, auth: {} }).headline).toBe('Not found');
  expect(providerSummary({ enabled: true, installed: true, status: 'ready', auth: { status: 'unauthenticated', label: 'API key' } }).headline).toBe('Not authenticated · API key');
  expect(providerSummary({ enabled: true, installed: true, status: 'error', auth: {}, message: 'boom' })).toEqual({ headline: 'Unavailable', detail: 'boom' });
  expect(versionAdvisory(fixtureProvider, false)?.title).toBe('Update available');
  expect(versionAdvisory({ versionAdvisory: { status: 'current' } }, true)).toBeNull();
  expect(checkedLabel('2026-10-03T14:00:00Z', Date.parse('2026-10-03T14:09:30Z'))).toBe('Checked 9m ago');
  expect(validateInstanceId('9x', new Set())).toContain('must start with a letter');
  expect(validateInstanceId('codex', new Set(['codex']))).toBe("An instance named 'codex' already exists.");
});

test('wizard derives a free instance id from the label and reserves dirty legacy slots', () => {
  const server = new FakeServer();
  expect(providerWizard(server, true, 1, 'codex', false, '', false, '')).toMatchObject({ label: 'Codex', instanceId: 'codex_2', idError: '', sessions: [{ key: 'wizard:1' }] });
  expect(providerWizard(server, true, 1, 'codex', true, 'Work Account', false, '').instanceId).toBe('codex_work_account');
  expect(providerWizard(server, true, 1, 'claudeAgent', true, '', false, '').instanceId).toBe('claudeAgent_2');
  expect(providerWizard(server, true, 1, 'claudeAgent', true, 'x', true, '').idError).toBe('Instance ID is required.');
  expect(providerWizard(server, true, 1, 'claudeAgent', false, '', true, 'exact_fixture').idError).toBe("An instance named 'exact_fixture' already exists.");
  expect(providerWizard(server, true, 1, 'grok', false, '', false, '').instanceId).toBe('grok');
  expect(providerWizard(server, false, 1, 'antigravity', false, '', false, '').fields.map(field => [field.index, field.key, field.control])).toEqual([
    [0, 'authMethod', 'select'], [1, 'apiKey', 'password'], [2, 'gcpProject', 'text'], [3, 'gcpLocation', 'text'], [4, 'binaryPath', 'text']]);
  expect(providerFieldValues('codex', ['/bin/codex', '', '', '--x', ''])).toEqual({ binaryPath: '/bin/codex', launchArgs: '--x' });
});

test('add creates exactly one instance with config and accent; duplicates and bad ids write nothing', async () => {
  const server = new FakeServer();
  const add = (id: string, input: Obj) => runProviderOp(server, native, 'provider-add', id, JSON.stringify(input));
  await expect(add('9bad', { driver: 'codex', label: 'Codex', fields: {} })).rejects.toThrow('must start with a letter');
  await expect(add('exact_fixture', { driver: 'codex', label: 'Codex', fields: {} })).rejects.toThrow("already exists");
  await expect(add('', { driver: 'codex', label: 'Codex', accentColor: 'red', fields: {} })).rejects.toThrow('six-digit hex');
  expect(writes(server)).toHaveLength(0);
  expect(await add('', { driver: 'codex', label: 'Codex', accentColor: '#2563eb', fields: { binaryPath: '/fixture/codex', homePath: '/fixture/home' } })).toBe('');
  expect(obj(server.settings.providerInstances).codex_2).toEqual({ driver: 'codex', enabled: true, displayName: 'Codex', accentColor: '#2563eb',
    config: { binaryPath: '/fixture/codex', homePath: '/fixture/home', setupMode: 'existing' } });
  expect(obj(obj(writes(server)[0]!.payload).providerInstanceMutation).operation).toBe('create');
  expect(await add('', { driver: 'antigravity', label: 'Gemini work', fields: { authMethod: 'gemini-api-key', binaryPath: '' } })).toBe('');
  expect(obj(server.settings.providerInstances).antigravity_gemini_work).toEqual({ driver: 'antigravity', enabled: true, displayName: 'Gemini work', config: { authMethod: 'gemini-api-key' } });
  await expect(add('', { driver: 'acpRegistry', label: '', fields: {} })).rejects.toThrow('Select an ACP or configure one manually.');
  expect(await add('', { driver: 'acpRegistry', label: '', fields: { agentId: 'devin', commandPath: '/nonexistent/devin' } })).toBe('');
  expect(obj(server.settings.providerInstances).acpRegistry_custom).toEqual({ driver: 'acpRegistry', enabled: true, config: { agentId: 'devin', commandPath: '/nonexistent/devin', distribution: 'auto' } });
});

test('ACP registry search and prepare fill the identity step without installing during search', async () => {
  const server = new FakeServer();
  expect(await acpRegistry(server, native, '', false, [])).toMatchObject({ ready: false, agents: [] });
  const result = await acpRegistry(server, native, ' dev ', true, ['devin']);
  expect(result).toMatchObject({ ready: true, status: '1 compatible agent found.', agents: [{ id: 'devin', added: true, link: 'https://docs.devin.ai/cli' }] });
  expect(obj(server.calls.at(-1)!.payload)).toEqual({ query: 'dev' });
  providerWizard(server, true, 7, 'acpRegistry', false, '', false, '');
  expect(await runProviderOp(server, native, 'acp-prepare', '', JSON.stringify({ key: 'devin', value: 'Devin' }))).toBe('');
  const wizard = providerWizard(server, true, 7, 'acpRegistry', false, '', false, '');
  expect(wizard).toMatchObject({ label: 'Devin', instanceId: 'acpRegistry_devin', prepared: [{ id: 'devin', version: '1.2.0', distribution: 'binary' }] });
  expect(await runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: 'Devin', fields: {} }))).toBe('');
  expect(obj(obj(server.settings.providerInstances).acpRegistry_devin).config).toEqual({ agentId: 'devin', registryIconUrl: 'https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg', distribution: 'auto' });
  expect(providerWizard(server, true, 8, 'acpRegistry', false, '', false, '').prepared).toEqual([]);
});

test('editor writes change only the selected instance and skip no-op commits', async () => {
  const server = new FakeServer();
  const op = (name: string, id: string, key: string, value: string) => runProviderOp(server, native, name, id, JSON.stringify({ key, value }));
  const before = JSON.stringify(obj(server.settings.providerInstances).exact_fixture);
  expect(await op('provider-display', 'exact_fixture', '', 'Exact verification fixture')).toBe('');
  expect(await op('provider-field', 'exact_fixture', 'binaryPath', ' /fixture/codex ')).toBe('');
  expect(writes(server)).toHaveLength(0);
  expect(JSON.stringify(obj(server.settings.providerInstances).exact_fixture)).toBe(before);
  await op('provider-display', 'exact_fixture', '', 'Renamed fixture');
  await op('provider-field', 'exact_fixture', 'launchArgs', '--verbose');
  await op('provider-field', 'exact_fixture', 'launchArgs', '');
  await op('provider-accent', 'exact_fixture', '', '#ff0000');
  const instance = obj(obj(server.settings.providerInstances).exact_fixture);
  expect(instance.displayName).toBe('Renamed fixture');
  expect(instance.accentColor).toBe('#ff0000');
  expect(obj(instance.config)).toEqual({ setupMode: 'existing', binaryPath: '/fixture/codex', homePath: '/fixture/home', customModels: [{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna' }] });
  await expect(op('provider-field', 'exact_fixture', 'secretKey', 'x')).rejects.toThrow('not available');
  await expect(op('provider-remove', 'codex', '', '')).rejects.toThrow('Built-in provider slots can be reset');
  expect(obj(server.settings.providers).codex).toEqual(legacy().codex);
});

test('default-slot upsert resets the legacy mirror; reset removes the explicit slot', async () => {
  const server = new FakeServer();
  await runProviderOp(server, native, 'provider-display', 'claudeAgent', JSON.stringify({ key: '', value: 'My Claude' }));
  expect(obj(server.settings.providerInstances).claudeAgent).toEqual({ driver: 'claudeAgent', enabled: false, displayName: 'My Claude',
    config: { binaryPath: 'claude', homePath: '', customModels: [], launchArgs: '', autoCompactWindow: '' } });
  expect(obj(obj(server.settings.providers).claudeAgent).enabled).toBe(true);
  await runProviderOp(server, native, 'provider-reset', 'claudeAgent', '{}');
  expect('claudeAgent' in obj(server.settings.providerInstances)).toBe(false);
  expect(obj(obj(server.settings.providers).claudeAgent)).toEqual({ enabled: true, binaryPath: 'claude', homePath: '', customModels: [], launchArgs: '', autoCompactWindow: '' });
});

test('environment variables stay local until valid, keep redacted secrets and publish once', async () => {
  const server = new FakeServer();
  const op = (name: string, key: string, value = '') => runProviderOp(server, native, name, 'exact_fixture', JSON.stringify({ key, value }));
  await op('provider-env-add', '');
  expect(writes(server)).toHaveLength(0);
  expect(providerPage(server, 'exact_fixture', 0).editors[0]!.variables).toHaveLength(1);
  expect(await op('provider-env-name', '0', '9BAD')).toContain('Use letters, digits and underscores');
  expect(writes(server)).toHaveLength(0);
  await op('provider-env-name', '0', 'FIXTURE_TOKEN');
  await op('provider-env-value', '0', 'isolated');
  expect(arr(obj(obj(server.settings.providerInstances).exact_fixture).environment)).toEqual([{ name: 'FIXTURE_TOKEN', value: 'isolated', sensitive: true }]);
  obj(obj(server.settings.providerInstances).exact_fixture).environment = [{ name: 'FIXTURE_TOKEN', value: '', sensitive: true, valueRedacted: true }];
  server.config = server.snapshot();
  expect(providerPage(server, 'exact_fixture', 0).editors[0]!.variables[0]).toMatchObject({ name: 'FIXTURE_TOKEN', value: '', redacted: true });
  await op('provider-env-add', '');
  await op('provider-env-name', '1', 'BASE_URL');
  await op('provider-env-sensitive', '1', 'false');
  await op('provider-env-value', '1', 'http://127.0.0.1:9');
  expect(arr(obj(obj(server.settings.providerInstances).exact_fixture).environment)).toEqual([
    { name: 'FIXTURE_TOKEN', value: '', sensitive: true, valueRedacted: true }, { name: 'BASE_URL', value: 'http://127.0.0.1:9', sensitive: false }]);
  await op('provider-env-remove', '0');
  expect(arr(obj(obj(server.settings.providerInstances).exact_fixture).environment)).toEqual([{ name: 'BASE_URL', value: 'http://127.0.0.1:9', sensitive: false }]);
});

test('custom models add and remove with T3 validation (a new bare entry is stored as its slug)', async () => {
  const server = new FakeServer();
  const op = (name: string, key: string, value = '') => runProviderOp(server, native, name, 'exact_fixture', JSON.stringify({ key, value }));
  await expect(op('provider-model-add', '')).rejects.toThrow('Enter a model slug.');
  await expect(op('provider-model-add', 'gpt-5.6-luna')).rejects.toThrow('already saved');
  await op('provider-model-add', 'gpt-fixture-mini');
  const stored = () => obj(obj(obj(server.settings.providerInstances).exact_fixture).config).customModels;
  expect(stored()).toEqual([{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna' }, 'gpt-fixture-mini']);
  await op('provider-model-remove', 'gpt-fixture-mini');
  expect(stored()).toEqual([{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna' }]);
});

test('enable, delete, refresh and the health interval use the documented server writes', async () => {
  const server = new FakeServer();
  await runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'codex', label: 'Disposable', fields: { binaryPath: '/fixture/codex' } }));
  expect(obj(server.settings.providerInstances).codex_disposable).toBeDefined();
  await runProviderOp(server, native, 'provider-enabled', 'codex_disposable', 'false');
  expect(obj(obj(server.settings.providerInstances).codex_disposable).enabled).toBe(false);
  await runProviderOp(server, native, 'provider-remove', 'codex_disposable', '{}');
  expect(Object.keys(obj(server.settings.providerInstances))).toEqual(['exact_fixture']);
  await runProviderOp(server, native, 'provider-refresh', '', '{}');
  expect(server.calls.find(call => call.method === 'server.refreshProviders')?.payload).toEqual({ refreshModels: true });
  expect(healthInterval(server.settings)).toEqual({ seconds: 300, preset: 300, base: 'balanced' });
  await runProviderOp(server, native, 'provider-health', '', JSON.stringify({ key: '', value: '330' }));
  expect(server.settings.backgroundActivity).toEqual({ schemaVersion: 1, profile: 'custom', baseProfile: 'balanced', overrides: { providerHealthRefreshInterval: 330000 } });
  expect(providerPage(server, '', 0)).toMatchObject({ healthSeconds: '330', healthCustom: true, healthDown: '300', healthUp: '360' });
  await runProviderOp(server, native, 'provider-health', '', JSON.stringify({ key: '', value: 'reset' }));
  expect(server.settings.backgroundActivity).toEqual({ schemaVersion: 1, profile: 'balanced', overrides: {} });
});

test('read-only and disconnected sessions expose no provider writes', () => {
  const server = new FakeServer();
  server.writable = false;
  expect(providerPage(server, '', 0)).toMatchObject({ available: true, writable: false, message: "This session can view this environment's providers but can't change their settings." });
  server.ready = false;
  expect(providerPage(server, '', 0)).toMatchObject({ available: false, message: 'Connect an environment to set up its providers.', rows: [] });
});

test('usage hubs add with a host-derived id and remove only that entry', async () => {
  const server = new FakeServer();
  const op = (name: string, id: string, key: string, value: string) => runProviderOp(server, native, name, id, JSON.stringify({ key, value }));
  await expect(op('provider-hub-add', '', 'https://hub.example.ts.net:8318', '')).rejects.toThrow('Enter the hub URL and management key.');
  await expect(op('provider-hub-add', '', 'not a url', 'secret')).rejects.toThrow('Enter a valid hub URL.');
  expect(writes(server)).toHaveLength(0);
  await op('provider-hub-add', 'Team hub', 'https://hub.example.ts.net:8318', 'secret');
  expect(obj(writes(server)[0]!.payload).patch).toEqual({ usageLimitSources: { 'cliproxy-hub.example.ts.net-8318': { kind: 'cliproxy', label: 'Team hub', url: 'https://hub.example.ts.net:8318', managementKey: 'secret', enabled: true } } });
  server.settings.usageLimitSources = { 'cliproxy-hub.example.ts.net-8318': { kind: 'cliproxy', label: 'Team hub', url: 'https://hub.example.ts.net:8318', managementKey: '', enabled: true } };
  server.config = server.snapshot();
  expect(providerPage(server, '', 0).hubs).toEqual([{ id: 'cliproxy-hub.example.ts.net-8318', first: true, label: 'Team hub', description: 'CLI Proxy · https://hub.example.ts.net:8318' }]);
  await op('provider-hub-remove', '', 'cliproxy-hub.example.ts.net-8318', '');
  expect(obj(writes(server).at(-1)!.payload).patch).toEqual({ usageLimitSources: { 'cliproxy-hub.example.ts.net-8318': null } });
});

test('a ChatGPT account becomes one managed Codex instance with a uuid id (AddCodexAccountDialog.tsx:50-53)', async () => {
  const server = new FakeServer(), uuids = ['5f0c7d2e-1111-4a5b-9c3d-000000000001', '5f0c7d2e-2222-4a5b-9c3d-000000000002'];
  server.ids = async (_native, count) => uuids.splice(0, count);
  const before = JSON.stringify(obj(server.settings.providerInstances).exact_fixture);
  await expect(runProviderOp(server, native, 'provider-chatgpt', '', JSON.stringify({ key: ' ', value: '' }))).rejects.toThrow('Enter an account name.');
  await runProviderOp(server, native, 'provider-chatgpt', '', JSON.stringify({ key: 'Personal', value: '' }));
  await runProviderOp(server, native, 'provider-chatgpt', '', JSON.stringify({ key: 'Personal', value: '' }));
  const instances = obj(server.settings.providerInstances);
  // "The ID is routing identity; the name is editable and need not be unique."
  for (const uuid of ['5f0c7d2e-1111-4a5b-9c3d-000000000001', '5f0c7d2e-2222-4a5b-9c3d-000000000002'])
    expect(instances[`codex_${uuid}`]).toEqual({ driver: 'codex', displayName: 'ChatGPT - Personal', enabled: true, config: { enabled: true, setupMode: 'managed' } });
  expect(Object.keys(instances).filter(id => id.startsWith('codex_chatgpt'))).toEqual([]);
  expect(JSON.stringify(instances.exact_fixture)).toBe(before);
  // Each create is one atomic mutation that names only the new instance.
  const creates = writes(server).map(call => obj(obj(call.payload).providerInstanceMutation)).filter(mutation => mutation.operation === 'create');
  expect(creates.map(mutation => mutation.instanceId)).toEqual(['codex_5f0c7d2e-1111-4a5b-9c3d-000000000001', 'codex_5f0c7d2e-2222-4a5b-9c3d-000000000002']);
});

// fix-provider-auth-state, bug 19 (#298): after Reconnect the list row kept painting "Not authenticated" while the
// page said "Authenticated · ChatGPT" (X64: macOS keeps a wrapped paragraph's raster once it shrinks below the
// raster size). The page answer was right; the row's status and the editor's status line are now new nodes per status.
test('the list row and the editor read the same status, and each status is a node of its own (X64)', async () => {
  const instances = { codex: { driver: 'codex', enabled: true, config: { setupMode: 'managed' } } };
  const live = (auth: Obj) => [{ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, status: auth.status === 'authenticated' ? 'ready' : 'warning',
    message: auth.status === 'authenticated' ? '' : 'Sign in with ChatGPT to use Codex.', auth }];
  const host = { config: { settings: { providerInstances: instances, providers: {} }, providers: live({ status: 'unauthenticated' }) }, ready: true, writable: true,
    local: { favoriteModels: [] }, rpc: async () => ({}) } as unknown as ProviderHost;
  const row = () => providerPage(host, 'codex', 0).rows.find(entry => entry.id === 'codex')!;
  expect(row().status).toBe('Not authenticated · Sign in with ChatGPT to use Codex.');
  host.config = { ...host.config, providers: live({ status: 'authenticated', type: 'chatgpt', label: 'ChatGPT' }) };
  const page = providerPage(host, 'codex', 0);
  expect([row().status, page.editors[0]!.statusLead, page.editors[0]!.statusLabel]).toEqual(['Authenticated · ChatGPT', 'Authenticated · ChatGPT', '']);
  const source = await Bun.file(new URL('./providers.contract', import.meta.url)).text();
  expect(source).toContain('each status in [row.status] key=status\n                              text status ');
  expect(source).toContain('each status in [`${editor.dot}|${editor.statusLead}|${editor.statusEmail}|${editor.statusLabel}|${editor.statusDetail}`] key=status\n              row width="100%" min-width=0 padding-top="0.125rem"');
});
