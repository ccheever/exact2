import { describe, test, expect } from 'bun:test';
import { providerRows, providerPage, providerWizard, acpRegistry, refreshAcpRegistry, runProviderOp, providerFieldValues, healthInterval, type ProviderHost } from '../../src/providers/providers';
import { withUpkeep, versionAdvisoryPresentation } from '../../src/providers/providers-upkeep';
import { providerSummary, providerStatusKey, versionAdvisory, checkedLabel, validateInstanceId, providerInstanceInitials } from '../../src/providers/providers-meta';
import { obj, arr, str, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { existsSync } from 'node:fs';
import { appPath, appText } from '../support/app-files';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { PROVIDER_DISPLAY_NAMES } from '../../src/providers/provider-updates';
import { searchSettings } from '../../src/settings/settings-search';
import { DRIVER_MARKS } from '../../tools/generate/icon-marks.mjs';

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
  registry: Obj[] = [{ id: 'devin', name: 'Devin', version: '1.2.0', description: 'Devin CLI coding agent', website: 'https://docs.devin.ai/cli', repository: null, icon: 'https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg', distribution: 'binary' }];
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
    if (method === 'server.searchAcpRegistry') return { agents: this.registry };
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

// providerStatus.test.ts "Antigravity with unchecked sign-in".
describe('Antigravity with unchecked sign-in', () => {
  const unchecked = { instanceId: 'antigravity', driver: 'antigravity', enabled: true, installed: true, status: 'warning', auth: { status: 'unknown' },
    message: 'Antigravity is installed. Google account access is not checked yet.' };
  test('an unchecked Google sign-in reads neutrally, without claiming sign-in is required', () => {
    for (const auth of [unchecked.auth, { status: 'unknown', type: 'oauth-personal' }]) {
      expect(providerSummary({ ...unchecked, auth })).toEqual({ headline: 'Installed · Sign-in not checked', detail: unchecked.message });
      expect(providerStatusKey({ ...unchecked, auth })).toBe('neutral');
    }
  });
  test('actionable warnings, failures and disabled states keep their reading', () => {
    const cases: [string, Obj, string, string][] = [
      ['confirmed sign-out', { auth: { status: 'unauthenticated' } }, 'Not authenticated', 'warning'],
      ['warning despite known authentication', { auth: { status: 'authenticated' } }, 'Needs attention', 'warning'],
      ['probe failure', { status: 'error' }, 'Unavailable', 'error'],
      ['missing CLI', { installed: false }, 'Not found', 'warning'],
      ['API key authentication', { auth: { status: 'unknown', type: 'gemini-api-key' } }, 'Needs attention', 'warning'],
      ['Agent Platform authentication', { auth: { status: 'unknown', type: 'agent-platform' } }, 'Needs attention', 'warning'],
      ['another provider', { driver: 'codex' }, 'Needs attention', 'warning'],
      ['disabled provider', { enabled: false }, 'Disabled', 'disabled'],
    ];
    for (const [name, change, headline, status] of cases) expect([name, providerSummary({ ...unchecked, ...change }).headline, providerStatusKey({ ...unchecked, ...change })]).toEqual([name, headline, status]);
  });
});

test('no update is offered that cannot clear a limited-support warning (providerStatus.test.ts)', () => {
  // OpenCode 1.18.35 is the newest `opencode-ai`; the recommended 2.x ships as another package.
  const advisory = { status: 'current', currentVersion: '1.18.35', latestVersion: '1.18.35', updateCommand: 'npm install -g opencode-ai@latest', canUpdate: true, message: null };
  const compatibilityAdvisory = { status: 'graceful', latestVersionStatus: 'graceful', message: null, recommendedRange: '>=2.0.18', recommendedVersion: null };
  expect(versionAdvisory({ versionAdvisory: advisory, compatibilityAdvisory }, true)).toEqual({ title: 'Limited support', warning: true, detail: 'Use >=2.0.18 for full support.', command: '' });
  expect(versionAdvisory({ versionAdvisory: { ...advisory, status: 'behind_latest', currentVersion: '1.18.34' }, compatibilityAdvisory }, true)?.command).toBe(advisory.updateCommand);
  expect(versionAdvisoryPresentation(advisory, compatibilityAdvisory)).toEqual({ title: 'Limited support', detail: 'Use >=2.0.18 for full support.', updateCommand: null, emphasis: 'normal', targetVersion: null });
  expect(versionAdvisoryPresentation({ ...advisory, status: 'behind_latest', currentVersion: '1.18.34' }, compatibilityAdvisory)?.updateCommand).toBe(advisory.updateCommand);
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

// AcpRegistrySearchStep's "Local ACP command": after Enter manually, it opens the ACP Registry
// configured by hand with the local source, labelled "Local ACP"; its variables go with the instance.
test('Local ACP command: the entry after Enter manually opens the local configuration, whose variables the instance keeps', async () => {
  const dialog = contractComponent('providers-wizard.contract', 'AddProviderDialog');
  const calls: unknown[][] = [];
  const state: Obj = { f0: '', f1: 'devin', f2: '', f3: 'auto', f4: '', commandArgs: ['--x'], variables: [{ name: 'A', value: 'b', sensitive: true }], published: [{ name: 'A', value: 'b', sensitive: true }] };
  dialog.call('localCommand', state, { ui: (...args: unknown[]) => calls.push(args) }, []);
  expect([state.f0, state.f1, state.f2, state.f3, state.f4, state.commandArgs, state.variables, state.published, calls]).toEqual(['local', '', '', '', '', [], [], [], [['local', '']]]);
  const source = appText('providers-wizard.contract');
  expect(source).toMatch(/testId="acp-enter-manually"[^\n]*\n[^\n]*text "Enter manually"[^\n]*\n[^\n]*\/\/[^\n]*\n\s+button press=localCommand class=PvOutline[^\n]*testId="acp-local-command"[^\n]*\n\s+text "Local ACP command"/);
  expect(source).toContain('text (localAcp ? "Local ACP command" : "Enter manually")');
  expect(source).toContain('text (localAcp ? `Run an installed ACP executable on ${wizard.environment}.` : "Enter an official registry ID and any local executable or auth override.")');
  expect(source).toContain('summary=(shownStep > 0 ? (isAcp ? (localAcp ? "Local ACP command" : wizard.label) : wizard.driverLabel) : "")');
  // ProviderSettingsForm's local command: commandPath is the Executable, "e.g. dsh"; ACP source has no description.
  const fieldRow = contractComponent('providers-wizard.contract', 'WizardFieldRow');
  const fieldScope = (local: boolean, key: string) => ({ local, field: { key, label: 'Executable override', description: '', placeholder: 'Registry default', control: 'text', index: 2, options: [] },
    value: '', set: () => {}, SelectOption: (args: Obj) => args });
  const placeholder = (local: boolean, key: string) => fieldRow.attribute('input value=value input=set(field.index)', 'placeholder', {}, fieldScope(local, key));
  expect([placeholder(true, 'commandPath'), placeholder(false, 'commandPath'), placeholder(true, 'agentId')]).toEqual(['e.g. dsh', 'Registry default', 'Registry default']);
  const wizardRow = contractComponent('providers-wizard.contract', 'WizardRow');
  expect([wizardRow.drawn('text description', {}, { title: 'ACP source', description: '', status: '' }), wizardRow.drawn('text description', {}, { title: 'Executable', description: 'x', status: '' })]).toEqual([false, true]);
  expect(providerWizard(new FakeServer(), true, 1, 'acpRegistry', false, '', false, '').fields.find(field => field.key === 'source')?.description).toBe('');
  // isLocalAcp needs the hand configuration: back on the registry search, the draft's local source is not the local command.
  expect(source).toContain('derive localAcp = wizard.driver == "acpRegistry" and manual and f0 == "local"');
  // Enter manually, a registry agent's Add and a provider picked after the ACP Registry clear its draft.
  expect(source).toContain('button press=manualEntry class=PvGhost');
  expect(source).toContain('button press=prepareAgent(agent.id, agent.name) class=PvOutline');
  expect(source.split('WizardDriverOption(driver=driver, manual=manual, ui=pickDriver)').length).toBe(3);
  const drafted = (): Obj => ({ f0: 'local', f1: '', f2: 'dsh', f3: '', f4: '', commandArgs: ['--acp'], variables: [{ name: 'A', value: 'b', sensitive: true }], published: [{ name: 'A', value: 'b', sensitive: true }] });
  const fields = (draft: Obj) => [draft.f0, draft.f1, draft.f2, draft.f3, draft.f4, draft.commandArgs, draft.variables, draft.published];
  const sent: unknown[][] = [];
  const props = (driver: string) => ({ wizard: { driver }, ui: (...args: unknown[]) => sent.push(['ui', ...args]), op: (...args: unknown[]) => sent.push(['op', ...args]) });
  const manual = drafted(), prepared = drafted(), left = drafted(), codex = drafted();
  dialog.call('manualEntry', manual, props('acpRegistry'), []);
  dialog.call('prepareAgent', prepared, props('acpRegistry'), ['devin', 'Devin']);
  dialog.call('pickDriver', left, props('acpRegistry'), ['driver', 'codex']);
  dialog.call('pickDriver', codex, props('codex'), ['driver', 'claudeAgent']);
  const cleared = ['', '', '', '', '', [], [], []];
  expect([fields(manual), fields(prepared), fields(left), fields(codex)]).toEqual([cleared, cleared, cleared, fields(drafted())]);
  expect(sent).toEqual([['ui', 'manual', 'true'], ['op', 'acp-prepare', '', 'devin', 'Devin', true], ['ui', 'driver', 'codex'], ['ui', 'driver', 'claudeAgent']]);
  // The root's half: the ACP Registry configured by hand, labelled "Local ACP", an agent added before it dropped
  // (handleLocalAcpConfiguration).
  const root = contractComponent('app.contract', 'T3Code');
  const window: Obj = { modalError: 'x', providerSelected: '', wizardDriver: 'codex', wizardManual: false, wizardLabel: '', wizardLabelSet: false, wizardIdSet: true, providerDialog: 'add', wizardAgentEpoch: 0 };
  root.call('providerUi', window, {}, ['local', '']);
  expect([window.wizardDriver, window.wizardManual, window.wizardLabel, window.wizardLabelSet, window.wizardIdSet, window.wizardAgentEpoch]).toEqual(['acpRegistry', true, 'Local ACP', true, false, 1]);
  // The window's half starts the attempt over (setHasAttemptedSubmit(false)).
  const attempt: Obj = { wizardAttempted: true };
  contractComponent('app-window.contract', 'T3Window').call('providerUi', attempt, { wizardStep: 0, wizardManual: false, providerWizard: { idError: '' },
    providerUiRoot: () => {}, min: Math.min, max: Math.max }, ['local', '']);
  expect(attempt.wizardAttempted).toBe(false);
  // The server still refuses a bad name (the dialog publishes none), and leaves a blank row out.
  const server = new FakeServer();
  const add = (input: Obj) => runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: 'Local ACP', fields: { source: 'local', commandPath: 'dsh' }, commandArgs: ['--acp'], ...input }));
  await expect(add({ environment: ['1bad name=x'] })).rejects.toThrow('Use letters, digits and underscores for variable names');
  expect(await add({ environment: ['1API_KEY=secret=1', '0BASE_URL=http://127.0.0.1', '1='] })).toBe('');
  expect(obj(server.settings.providerInstances).acpRegistry_local_acp).toEqual({ driver: 'acpRegistry', enabled: true, displayName: 'Local ACP',
    config: { source: 'local', commandPath: 'dsh', commandArgs: ['--acp'], distribution: 'auto' },
    environment: [{ name: 'API_KEY', value: 'secret=1', sensitive: true }, { name: 'BASE_URL', value: 'http://127.0.0.1', sensitive: false }] });
});

// AddProviderInstanceDialog's ACP Registry: Search registry keeps it chosen with no agent (:516-520), so the Provider
// step's prerequisite (acpSelectionError, resolveWizardNavigation) stops Next and the step header until an agent is
// added or one is configured by hand, a local command with its Executable (:230-236).
test('Add provider: past the ACP Registry, Next waits for an agent, a hand-entered ID or an Executable', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const win: Obj = { modalError: '', providerSelected: '', wizardDriver: 'codex', wizardManual: false, wizardLabel: '', wizardLabelSet: false, wizardIdSet: false, providerDialog: 'add', wizardAgentEpoch: 0 };
  root.call('providerUi', win, {}, ['local', '']);
  root.call('providerUi', win, {}, ['manual', 'false']); // Search registry
  expect([win.wizardDriver, win.wizardManual, win.wizardAgentEpoch]).toEqual(['acpRegistry', false, 1]);
  root.call('providerUi', win, {}, ['manual', 'true']); // Enter manually
  expect([win.wizardDriver, win.wizardManual, win.wizardAgentEpoch]).toEqual(['acpRegistry', true, 2]);
  // Enter manually from a fresh dialog (Codex chosen), then Search registry: still the ACP Registry.
  const fresh: Obj = { ...win, wizardDriver: 'codex', wizardManual: false, wizardAgentEpoch: 0 };
  root.call('providerUi', fresh, {}, ['manual', 'true']);
  root.call('providerUi', fresh, {}, ['manual', 'false']);
  expect([fresh.wizardDriver, fresh.wizardManual, fresh.wizardAgentEpoch]).toEqual(['acpRegistry', false, 1]);
  const dialog = contractComponent('providers-wizard.contract', 'AddProviderDialog');
  const sent: unknown[][] = [];
  const go = (draft: Obj, manual: boolean, prepared: Obj[], value: string, step = 0) =>
    dialog.call('go', { ...draft }, { wizard: { driver: 'acpRegistry', prepared }, manual, step, ui: (...args: unknown[]) => sent.push(args) }, ['step', value]);
  const local = { f0: 'local', f1: '', f2: '', f3: '', f4: '' }, byHand = { f0: '', f1: 'devin', f2: '', f3: '', f4: '' };
  go(local, false, [], 'next'); // the search with nothing added
  go(local, false, [], '1'); // the step header
  go(local, false, [{ id: 'devin' }], 'next'); // an agent added
  go(local, true, [], 'next'); // the local command, no Executable
  go({ ...local, f2: 'dsh' }, true, [], 'next');
  go({ ...byHand, f1: '' }, true, [], 'next'); // by hand, no ID
  go(byHand, true, [], 'next');
  go(local, false, [], 'back', 1);
  expect(sent).toEqual([['blocked', ''], ['blocked', ''], ['step', 'next'], ['blocked', ''], ['step', 'next'], ['blocked', ''], ['step', 'next'], ['step', 'back']]);
  // The block marks the attempt (the window's half), which shows the prerequisite's own words, under the form or the search.
  const attempt: Obj = { wizardAttempted: false };
  contractComponent('app-window.contract', 'T3Window').call('providerUi', attempt, { wizardStep: 0, wizardManual: true, providerWizard: { idError: '' },
    providerUiRoot: () => {}, min: Math.min, max: Math.max }, ['blocked', '']);
  expect(attempt.wizardAttempted).toBe(true);
  const source = appText('providers-wizard.contract');
  expect(source).toContain('derive selectionError = not isAcp ? "" : (localAcp ? (f2.trim() == "" ? "Executable is required." : "") : (wizard.prepared.length > 0 or (manual and f1.trim() != "") ? "" : "Select an ACP or configure one manually."))');
  expect(source.split('if attempted and selectionError != ""\n').length).toBe(3);
  expect(source.split('go=go)').length).toBe(4);
  expect(source.split('button press=go("step", "next")').length).toBe(3);
  // A provider picked from the search takes none of the local draft.
  const picked: Obj = { ...local, f2: 'dsh', commandArgs: ['--acp'], variables: [], published: [] };
  dialog.call('pickDriver', picked, { wizard: { driver: 'acpRegistry' }, ui: () => {} }, ['driver', 'claudeAgent']);
  expect([picked.f0, picked.f2, picked.commandArgs]).toEqual(['', '', []]);
});

// The add's press: an agent added from the search brings its own config (handleAcpPrepared), so only a configuration by
// hand sends the fields, which the root then reads for a local command; the variables are the published set.
test("Add provider: the add sends a hand configuration's fields and its published variables", () => {
  const line = appText('providers-wizard.contract').split('\n').find(entry => entry.includes('testId="create-provider"'))!;
  const press = line.slice(line.indexOf('press=') + 6, line.indexOf(' class=PvPrimary'));
  const args = (manual: boolean, f0: string) => value(press, { add: (...values: unknown[]) => values, wizard: { driver: 'acpRegistry' }, shownLabel: 'Local ACP', shownId: '', accent: '',
    f0, f1: '', f2: 'dsh', f3: '', f4: '', commandArgs: ['--acp'], isAcp: true, manual, localAcp: manual && f0 === 'local',
    published: [{ name: 'API_KEY', value: 'k', sensitive: true }], variables: [{ name: '1BAD', value: 'x', sensitive: true }] } as Env) as unknown[];
  expect(args(true, 'local')).toEqual(['acpRegistry', 'Local ACP', '', '', ['local', '', 'dsh', '', ''], ['--acp'], ['1API_KEY=k']]);
  expect(args(false, 'local')).toEqual(['acpRegistry', 'Local ACP', '', '', ['', '', '', '', ''], ['--acp'], []]);
  // The root closes the dialog on a local command only (provider-add-local), and passes the variables on.
  const root = contractComponent('app.contract', 'T3Code');
  const sends: unknown[][] = [];
  for (const fields of [['local', '', 'dsh', '', ''], ['', '', '', '', '']]) {
    const state: Obj = { commandPending: false, pendingModal: '', providerCreated: '', modalError: '' };
    root.call('providerAdd', state, { providerPage: { environment: 'env1' }, providerAdd: (...values: unknown[]) => { sends.push(values); return {}; } },
      ['acpRegistry', 'Local ACP', '', '', fields, ['--acp'], ['1API_KEY=k']]);
    sends.push([state.pendingModal]);
  }
  expect(sends).toEqual([['acpRegistry', 'Local ACP', '', '', ['local', '', 'dsh', '', ''], 'env1', ['--acp'], ['1API_KEY=k']], ['provider-add-local'],
    ['acpRegistry', 'Local ACP', '', '', ['', '', '', '', ''], 'env1', ['--acp'], ['1API_KEY=k']], ['provider-add']]);
  // The window's half hands them to the root.
  const handed: unknown[][] = [];
  contractComponent('app-window.contract', 'T3Window').call('providerAdd', { wizardAttempted: false }, { providerAddRoot: (...values: unknown[]) => handed.push(values) },
    ['acpRegistry', 'Local ACP', '', '', ['local'], ['--acp'], ['1API_KEY=k']]);
  expect(handed).toEqual([['acpRegistry', 'Local ACP', '', '', ['local'], ['--acp'], ['1API_KEY=k']]]);
});

// ProviderEnvironmentSection: the rows publish as a field is left, a lock toggled or a row removed, only while every
// name is valid (a row left as added is skipped), so a bad name keeps the last valid set and the add goes on with it
// (:324-344); the lock and remove are named by the variable (:446,462) and the lock has its tooltip.
test("Add provider's variables publish as ProviderEnvironmentSection's do", () => {
  const dialog = contractComponent('providers-wizard.contract', 'AddProviderDialog');
  const row = (name: string, value = 'v', sensitive = true) => ({ name, value, sensitive });
  // The file's own name rule, and the row's record.
  const scope = { WizardVariable: (fields: Obj) => fields, envNameValid: contractFns('providers-wizard.contract').envNameValid };
  expect(['API_KEY', '_x', 'a1', '1BAD', 'BAD-NAME', ''].map(name => scope.envNameValid(name as never))).toEqual([true, true, true, false, false, false]);
  const state: Obj = { variables: [row(' API_KEY ')], published: [] };
  dialog.call('commitVariables', state, scope, []);
  expect([state.variables, state.published]).toEqual([[row('API_KEY')], [row('API_KEY')]]);
  state.variables = [row('API_KEY'), row('1BAD')];
  dialog.call('commitVariables', state, scope, []);
  expect(state.published).toEqual([row('API_KEY')]);
  dialog.call('toggleVariableSensitive', state, scope, [0]);
  expect(state.published).toEqual([row('API_KEY')]); // still a bad name among the rows
  dialog.call('removeVariable', state, scope, [1]);
  expect(state.published).toEqual([row('API_KEY', 'v', false)]);
  state.variables = [row('API_KEY', 'v', false), row('', '')]; // a row left as added
  dialog.call('commitVariables', state, scope, []);
  expect(state.published).toEqual([row('API_KEY', 'v', false)]);
  dialog.call('addVariable', state, scope, []);
  expect(state.published).toEqual([row('API_KEY', 'v', false)]); // adding a row publishes nothing
  const source = appText('providers-wizard.contract');
  expect(source).toContain('fn envNameValid(name: string): bool = name != "" and not "0123456789".includes(name.slice(0, 1)) and name.split("").filter((c) => not "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_".includes(c)).length == 0');
  expect(source).toContain('Tip(label=(variable.sensitive ? "Sensitive, stored separately" : "Plain text"), side="top", align="center", offset=24)');
  expect(source).toContain('aria-pressed=variable.sensitive aria-label=(variable.name != "" ? `Mark environment variable ${variable.name} as sensitive` : `Mark environment variable ${index + 1} as sensitive`)');
  expect(source).toContain('aria-label=(variable.name != "" ? `Remove environment variable ${variable.name}` : `Remove environment variable ${index + 1}`)');
  expect(source.match(/blur=commitVariables submit=commitVariables/g)).toHaveLength(2);
  expect(source).toContain('text "Sensitive values are stored separately and never returned to the app."');
});

// handleManualAcpConfiguration and handleLocalAcpConfiguration drop an agent added before them (selectedAcp = null):
// the wizard's answer forgets it as the agent epoch moves, so the Identity step shows no card, a label by hand starts
// empty, and a local command never takes its ID.
test('Add provider: an agent added before Enter manually or Local ACP command is forgotten', async () => {
  const server = new FakeServer();
  const wizard = (epoch: number) => providerWizard(server, true, 9, 'acpRegistry', false, '', false, '', '', '', epoch);
  wizard(0);
  expect(await runProviderOp(server, native, 'acp-prepare', '', JSON.stringify({ key: 'devin', value: 'Devin' }))).toBe('');
  expect([wizard(0).prepared.map(agent => agent.id), wizard(0).label]).toEqual([['devin'], 'Devin']);
  expect([wizard(1).prepared, wizard(1).label]).toEqual([[], '']);
  await expect(runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: '', fields: {}, commandArgs: [], environment: [] })))
    .rejects.toThrow('Select an ACP or configure one manually.');
  expect(await runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: 'Local ACP', fields: providerFieldValues('acpRegistry', ['local', '', 'dsh', '', '']),
    commandArgs: [], environment: [] }))).toBe('');
  expect(obj(obj(server.settings.providerInstances).acpRegistry_local_acp).config).toEqual({ source: 'local', commandPath: 'dsh', commandArgs: [], distribution: 'auto' });
  // A prepared agent still current never goes with a local command either.
  wizard(1);
  await runProviderOp(server, native, 'acp-prepare', '', JSON.stringify({ key: 'devin', value: 'Devin' }));
  expect(await runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: 'Local two', fields: providerFieldValues('acpRegistry', ['local', '', 'dsh', '', '']),
    commandArgs: [], environment: [] }))).toBe('');
  expect(obj(obj(server.settings.providerInstances).acpRegistry_local_two).config).toEqual({ source: 'local', commandPath: 'dsh', commandArgs: [], distribution: 'auto' });
  // The epoch is the root's, through the wizard's resource and the scoped source.
  expect(appText('app.contract')).toContain('providerDialog, providerTarget, providerPage.environment, wizardAgentEpoch) as shape ProviderWizard');
  expect(appText('providers-scope.ts')).toContain("dialog, target, Number(args[11]) || 0);");
});

// ProviderCommandArguments and ProviderEnvironmentSection: SettingsRows whose control is the outline sm add button with
// its plus, top right, and whose rows sit below at mt-3 space-y-2 pb-2, inset as the field rows are.
test("Add provider's Arguments and Variables are rows whose add button is their control, the rows below", () => {
  const source = appText('providers-wizard.contract');
  const block = (title: string, add: string, rows: string) => new RegExp('\\n( +)column width="100%" box-sizing="border-box" padding-left="1rem" padding-right="1rem" padding-top="0\\.75rem" padding-bottom="0\\.75rem" testId="[^"]+"\\n'
    + '\\1  row width="100%" gap="2rem" align-items="center"\\n\\1    column flex=1 min-width=0 gap="0\\.25rem"\\n\\1      text "' + title + '"[^\\n]*\\n\\1      text [^\\n]*\\n'
    + '\\1    button press=' + add + ' class=PvOutline[^\\n]*\\n\\1      PvIcon\\(name="plus"[^\\n]*\\n\\1      text "[^"]+"[^\\n]*\\n'
    + '\\1  if ' + rows + '\\.length > 0\\n\\1    column width="100%" min-width=0 gap="0\\.5rem" margin-top="0\\.75rem" padding-bottom="0\\.5rem"\\n');
  expect(source).toMatch(block('Arguments', 'addArgument', 'commandArgs'));
  expect(source).toMatch(block('Variables', 'addVariable', 'variables'));
  // An argument row: its mono field and the ghost-destructive remove.
  expect(source).toMatch(/input value=arg input=editArgument\(index\)[^\n]*font-family="ui-monospace"[^\n]*\n\s+button press=removeArgument\(index\) class=PvMicro[^\n]*\n\s+PvIcon\(name="x", size=12, color="light-dark\(#e7000b, #ff6467\)"\)/);
});

// The Identity card links Docs to the agent's website and Source to its repository, each only when the registry gives
// it (AddProviderInstanceDialog); the search row's About link is the website, else the repository (aboutUrl).
test("Add provider's agent card shows Docs for a website and Source for a repository", async () => {
  const card = contractComponent('providers-wizard.contract', 'AcpAgentCard');
  const links = (website: string, repository: string) => {
    const scope = { agent: { id: 'amp-acp', name: 'Amp', version: '0.10.0', distribution: 'binary', website, repository } };
    return [card.drawn('text "Docs"', {}, scope), card.drawn('text "Source"', {}, scope)];
  };
  expect([links('', 'https://github.com/x/amp-acp'), links('https://amp.dev', ''), links('https://amp.dev', 'https://github.com/x/amp-acp')]).toEqual([[false, true], [true, false], [true, true]]);
  const server = new FakeServer();
  server.registry = [...server.registry, { id: 'amp-acp', name: 'Amp', version: '0.10.0', description: 'ACP wrapper for Amp', website: null, repository: 'https://github.com/x/amp-acp', icon: '', distribution: 'binary' }];
  await acpRegistry(server, native, '', false, []);
  const amp = (await acpRegistry(server, native, 'amp', true, [])).agents.find(entry => entry.id === 'amp-acp')!;
  expect([amp.link, amp.website, amp.repository]).toEqual(['https://github.com/x/amp-acp', '', 'https://github.com/x/amp-acp']);
});

test('ACP registry search and prepare fill the identity step without installing during search', async () => {
  const server = new FakeServer();
  expect(await acpRegistry(server, native, '', false, [])).toMatchObject({ ready: false, agents: [] });
  const result = await acpRegistry(server, native, ' dev ', true, ['devin']);
  expect(result).toMatchObject({ ready: true, status: '1 compatible agent found.', agents: [{ id: 'devin', added: true, link: 'https://docs.devin.ai/cli' }] });
  expect(obj(server.calls.at(-1)!.payload)).toEqual({ query: 'dev' });
  // The unsearched list asks for the whole compatible catalog, not the server's default 20 (#19145).
  await acpRegistry(server, native, '  ', true, []);
  expect(obj(server.calls.at(-1)!.payload)).toEqual({ query: '', limit: 512 });
  providerWizard(server, true, 7, 'acpRegistry', false, '', false, '');
  expect(await runProviderOp(server, native, 'acp-prepare', '', JSON.stringify({ key: 'devin', value: 'Devin' }))).toBe('');
  const wizard = providerWizard(server, true, 7, 'acpRegistry', false, '', false, '');
  expect(wizard).toMatchObject({ label: 'Devin', instanceId: 'acpRegistry_devin', prepared: [{ id: 'devin', version: '1.2.0', distribution: 'binary' }] });
  expect(await runProviderOp(server, native, 'provider-add', '', JSON.stringify({ driver: 'acpRegistry', label: 'Devin', fields: {} }))).toBe('');
  expect(obj(obj(server.settings.providerInstances).acpRegistry_devin).config).toEqual({ agentId: 'devin', registryIconUrl: 'https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg', distribution: 'auto' });
  expect(providerWizard(server, true, 8, 'acpRegistry', false, '', false, '').prepared).toEqual([]);
});

test('Add provider leads with first-class providers and warns before a second Codex or Claude account', () => {
  const server = new FakeServer();
  const wizard = providerWizard(server, true, 1, 'codex', false, '', false, '');
  expect(wizard.drivers.map(driver => [driver.id, driver.badge])).toEqual([['codex', ''], ['claudeAgent', ''], ['cursor', ''], ['grok', ''], ['opencode', ''],
    ['antigravity', ''], ['muse', ''], ['pi', '']]);
  // packages/provider-fx's client is not featured: fx is listed under Show more, with the ACP Registry.
  expect(wizard.moreDrivers.map(driver => [driver.id, driver.label, driver.badge])).toEqual([['fx', 'fx', '']]);
  expect(providerWizard(server, true, 1, 'fx', false, '', false, '')).toMatchObject({ label: 'fx', instanceId: 'fx' });
  expect(providerWizard(server, true, 1, 'fx', false, '', false, '').fields.map(field => [field.key, field.label, field.description, field.placeholder])).toEqual([
    ['binaryPath', 'Binary path', 'Path to the fx binary. All fx instances on this machine share its fx login.', 'fx']]);
  // MultipleAccountsNote: Codex is signed in here (exact_fixture), so choosing Codex and Add ChatGPT account say so; Claude is not.
  expect([wizard.accountsNote, wizard.chatgptAccountsNote]).toEqual(['Codex', 'Codex']);
  expect(providerWizard(server, true, 1, 'claudeAgent', false, '', false, '').accountsNote).toBe('');
  server.live = [...server.live, { instanceId: 'claude_work', driver: 'claudeAgent', enabled: true, auth: { status: 'authenticated' } }];
  server.config = server.snapshot();
  expect(providerWizard(server, true, 1, 'claudeAgent', false, '', false, '').accountsNote).toBe('Claude');
  expect(['grok', 'acpRegistry'].map(driver => providerWizard(server, true, 1, driver, false, '', false, '').accountsNote)).toEqual(['', '']);
});

test('the ACP Registry list leaves out first-class agents and keeps an answer for five minutes', async () => {
  const server = new FakeServer(), at = 1_000_000;
  const agent = (id: string): Obj => ({ id, name: id, version: '1.0.0', description: '', website: null, repository: null, icon: null, distribution: 'npx' });
  server.registry = ['codex-acp', 'claude-acp', 'cursor', 'grok-build', 'opencode', 'opencode-v2', 'antigravity-acp', 'pi-acp', 'devin', 'mistral-vibe'].map(agent);
  const searches = () => server.calls.filter(call => call.method === 'server.searchAcpRegistry').length;
  expect(await acpRegistry(server, native, '', false, [], at)).toMatchObject({ ready: false, agents: [] }); // nothing kept yet
  const first = await acpRegistry(server, native, '', true, [], at);
  expect([first.agents.map(entry => entry.id), first.status]).toEqual([['devin', 'mistral-vibe'], '2 compatible agents found.']);
  // Closed, it holds what the dialog opens on; reopened within five minutes it shows it without searching again.
  expect((await acpRegistry(server, native, '', false, [], at + 60_000)).agents.map(entry => entry.id)).toEqual(['devin', 'mistral-vibe']);
  expect((await acpRegistry(server, native, '', true, ['devin'], at + 4 * 60_000)).agents.map(entry => [entry.id, entry.added])).toEqual([['devin', true], ['mistral-vibe', false]]);
  expect(searches()).toBe(1);
  await acpRegistry(server, native, '', true, [], at + 5 * 60_000 + 1); // stale: asked again
  expect(searches()).toBe(2);
  refreshAcpRegistry(server, ' '); // Enter on the same search
  await acpRegistry(server, native, '', true, [], at + 5 * 60_000 + 2);
  expect(searches()).toBe(3);
  // Kept five minutes after the dialog stops asking for it, then dropped.
  await acpRegistry(server, native, '', false, [], at + 6 * 60_000);
  expect((await acpRegistry(server, native, '', false, [], at + 11 * 60_000)).agents).toHaveLength(2);
  expect(await acpRegistry(server, native, '', false, [], at + 11 * 60_000 + 1)).toMatchObject({ ready: false, agents: [] });
});

test('list badges are the instance initials, punctuation breaking words', () => {
  expect(['Codex (hub)', 'Work', "Theo's Codex", "Codex 'hub'", '‘hub’', '(hub)', 'work_codex', '😀 Work', '😀'].map(providerInstanceInitials))
    .toEqual(['CH', 'WO', 'TC', 'CH', 'HU', 'HU', 'WC', '😀W', '😀']);
  const server = new FakeServer();
  server.settings.providerInstances = { ...obj(server.settings.providerInstances), codex_hub: { driver: 'codex', displayName: 'Codex (hub)', accentColor: '#2563eb', enabled: true, config: {} } };
  server.config = server.snapshot();
  expect(providerPage(server, '', 0).rows.find(row => row.id === 'codex_hub')?.badge).toBe('CH');
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

// ClaudeSettings `idleCompaction`, after Auto-compact after: Use default leaves the key out (ProviderSettingsSelect's first
// option), as a select field's first option does everywhere.
test("a Claude instance's own idle compaction: a select after Auto-compact after, Use default saved as no key", async () => {
  const server = new FakeServer();
  server.settings.providerInstances = { ...obj(server.settings.providerInstances), claude_work: { driver: 'claudeAgent', enabled: true, config: { binaryPath: 'claude' } } };
  server.config = server.snapshot();
  const field = () => providerPage(server, 'claude_work', 0).editors[0]!.fields.find(entry => entry.key === 'idleCompaction')!;
  expect(providerPage(server, 'claude_work', 0).editors[0]!.fields.map(entry => entry.key)).toEqual(['binaryPath', 'homePath', 'autoCompactWindow', 'idleCompaction', 'launchArgs']);
  expect([field().label, field().control, field().description]).toEqual(['Compact idle threads', 'select',
    'When to compact an idle thread over 200,000 tokens: after you return, just before its prompt cache expires, or never. Overrides the default in Providers > Advanced.']);
  expect(field().options.map(option => [option.label, option.selected])).toEqual([['Use default', true], ['When you return', false], ['Before the cache expires (beta)', false], ['Never', false]]);
  const op = (value: string) => runProviderOp(server, native, 'provider-field', 'claude_work', JSON.stringify({ key: 'idleCompaction', value }));
  await op('off');
  server.config = server.snapshot();
  expect(obj(obj(obj(server.settings.providerInstances).claude_work).config).idleCompaction).toBe('off');
  expect(field().options.find(option => option.selected)?.label).toBe('Never');
  await op('default');
  expect('idleCompaction' in obj(obj(obj(server.settings.providerInstances).claude_work).config)).toBe(false);
  const count = writes(server).length;
  await op('default');
  expect(writes(server)).toHaveLength(count);
});

// ProviderModelsSection's custom model field: CUSTOM_MODEL_PLACEHOLDER_BY_KIND names five drivers' slugs, and every
// other driver's field has no placeholder (ProviderModelsSection.tsx:20-26,411).
test("the custom model field shows its driver's placeholder, and none for fx, Grok, Muse or the ACP Registry", () => {
  const server = new FakeServer();
  server.settings.providerInstances = { ...obj(server.settings.providerInstances), fx_work: { driver: 'fx', enabled: true }, grok_work: { driver: 'grok', enabled: true },
    muse_work: { driver: 'muse', enabled: true }, acp_work: { driver: 'acpRegistry', enabled: true, config: { agentId: 'devin' } } };
  server.config = server.snapshot();
  const placeholder = (id: string) => providerPage(server, id, 0).editors[0]!.modelPlaceholder;
  expect(['exact_fixture', 'fx_work', 'grok_work', 'muse_work', 'acp_work'].map(placeholder)).toEqual(['gpt-6.7-codex-ultra-preview', '', '', '', '']);
});

// packages/provider-fx: its client icon in Grok's ink, its entry in PROVIDER_DISPLAY_NAMES (model.ts), and the search terms.
test('fx: its mark, its display name and the search terms that find it', () => {
  const mark = contractComponent('provider-icons-kit.contract', 'DriverMark');
  const props = { driver: 'fx', size: 16, initials: 'FX', icon: '' };
  expect([mark.drawn('FxDriverMark(size=size)', {}, props), mark.drawn('text initials', {}, props)]).toEqual([true, false]);
  expect(DRIVER_MARKS.FxDriverMark).toMatch(/^<svg viewBox="166\.241 0 155\.861 156" [^>]*color="light-dark\(#0F0F0F, #F5F5F5\)">/);
  expect(PROVIDER_DISPLAY_NAMES.fx).toBe('fx');
  expect(searchSettings('fx', { connected: true, autoSettle: true, scopeKind: 'environment' }).map(item => item.id)).toEqual(expect.arrayContaining(['providers', 'provider-update-checks']));
});

// AntigravityIcon is a PNG (Icons.tsx embeds it); DriverMark draws the same file from the app's assets/, by its path.
test("Antigravity's mark is the bundled PNG it names", () => {
  const props = { driver: 'antigravity', size: 16, initials: 'AG', icon: '' };
  const mark = contractComponent('provider-icons-kit.contract', 'DriverMark');
  expect([mark.drawn('image "assets/provider-antigravity.png"', {}, props), mark.drawn('text initials', {}, props)]).toEqual([true, false]);
  expect(existsSync(appPath('assets/provider-antigravity.png'))).toBe(true);
});

// ProviderSettingsSelect in Add provider's Config step, as on the instance's editor: a select, not a row of radios.
test("a provider's select field is a Select in Add provider and on the instance's editor", () => {
  const field = { key: 'authMethod', label: 'Sign-in method', description: '', placeholder: '', control: 'select', index: 2,
    options: [{ value: 'google', label: 'Google account', selected: true }, { value: 'agent-platform', label: 'Agent Platform (Vertex AI)', selected: false }] };
  const wizardRow = contractComponent('providers-wizard.contract', 'WizardFieldRow');
  const scope = { local: false, field, value: '', set: (index: number) => ['set', index], SelectOption: (args: Obj) => args };
  expect(wizardRow.drawn('SettingsSelect(selectId=`new-provider-${field.key}`', {}, scope)).toBe(true);
  expect(wizardRow.callArguments('SettingsSelect(selectId=`new-provider-${field.key}`', {}, scope)).toMatchObject({ selectId: 'new-provider-authMethod', valueLabel: 'Google account', choose: ['set', 2] });
  const ops: unknown[][] = [];
  contractComponent('providers.contract', 'ProviderConfigField').call('pick', {}, { instance: 'antigravity', field, op: (...args: unknown[]) => ops.push(args) }, ['agent-platform']);
  expect(ops).toEqual([['provider-field', 'antigravity', 'authMethod', 'agent-platform', true]]);
  expect(appText('providers.contract')).not.toContain('role="radiogroup" aria-label=field.label');
});

// Providers › Advanced's core rows (CompactIdleThreadsRow) follow Health check interval; their edits go, scoped, through
// the root's settings command as the core pages' do.
test('Providers › Advanced draws its core rows and sends their edits to the settings scope through the root', () => {
  const ops: unknown[][] = [];
  contractComponent('providers.contract', 'ProvidersPanel').call('coreChange', {}, { page: { writeScope: 'env1||' }, op: (...args: unknown[]) => ops.push(args) }, ['compact-idle-threads', 'value', 'off']);
  expect(ops).toEqual([['settings-core', 'compact-idle-threads:value|env1||', '', 'off', true]]);
  expect(appText('providers.contract')).toMatch(/\n {12}for row in page\.advanced key=row\.id\n {14}CoreRowView\(reduced=still, row=row, section="provider-advanced", [^\n]*change=coreChange,/);
  const commands: unknown[][] = [];
  contractComponent('app.contract', 'T3Code').call('providerOp', { commandPending: false }, { settingsCommand: (...args: unknown[]) => commands.push(args), providerPage: { environment: 'env1', projectIds: '' } },
    ['settings-core', 'compact-idle-threads:value|env1||', '', 'off', true]);
  expect(commands).toEqual([['settings-core', 'compact-idle-threads:value|env1||', 'off']]);
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

// SettingsWindow holds an open "Add custom model" field by its Models block's key
// (`startAdd` records `blockKey`), shows it while `modelAdding == block.key` and counts it as an Escape owner only
// while that key is on the page (`modelAddingShown`). ProviderModelsSection keeps isAdding through unrelated writes,
// keeps it after a refused add (its error shows) and clears it after a successful one (handleAdd's setIsAdding(false)).
test('an open "Add custom model" field survives other writes and a refused add, and closes for good after a successful add', async () => {
  const server = new FakeServer();
  const op = (name: string, id: string, key: string, value = '') => runProviderOp(server, native, name, id, JSON.stringify({ key, value }));
  const page = (selection = 'exact_fixture') => providerPage(server, selection, 0);
  const blockKey = () => page().editors[0]!.modelBlocks[0]!.key;
  // The window's derive: the field is there (and may own Escape) while the shown editor has that block.
  const shown = (adding: string, selection?: string) => adding !== '' && page(selection).editors.some(editor => editor.modelBlocks.some(block => block.key === adding));
  const adding = blockKey(); // Add custom model
  expect(shown(adding)).toBe(true);
  await op('provider-accent', 'exact_fixture', '', '#ff0000');
  await op('provider-field', 'exact_fixture', 'launchArgs', '--verbose');
  await op('provider-env-add', 'exact_fixture', '');
  expect(shown(adding)).toBe(true);
  await expect(op('provider-model-add', 'exact_fixture', 'gpt-5.6-luna')).rejects.toThrow('already saved');
  await expect(op('provider-model-add', 'exact_fixture', '')).rejects.toThrow('Enter a model slug.');
  expect(shown(adding)).toBe(true);
  expect(shown(adding, 'codex')).toBe(false); // another provider: the section unmounts
  await op('provider-model-add', 'exact_fixture', 'gpt-fixture-mini');
  const added = blockKey();
  expect(added).not.toBe(adding);
  expect(shown(adding)).toBe(false); // the field closed: Escape is Back's again
  // Removing the model it added restores the stored list, not the key: the closed field does not come back.
  await op('provider-model-remove', 'exact_fixture', 'gpt-fixture-mini');
  expect(obj(obj(obj(server.settings.providerInstances).exact_fixture).config).customModels).toEqual([{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna' }]);
  expect([adding, added]).not.toContain(blockKey());
  expect(shown(adding)).toBe(false);
  expect(shown(added)).toBe(false);
  // A removed instance takes its field with it.
  const reopened = blockKey();
  await op('provider-remove', 'exact_fixture', '');
  expect(shown(reopened)).toBe(false);
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
  expect(providerPage(server, '', 0)).toMatchObject({ available: true, writable: false, message: 'This session can view providers but cannot manage provider instances.' });
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

// #298: after Reconnect the list row kept painting "Not authenticated" while the
// page said "Authenticated · ChatGPT" (issues/closed/20261009-macos-small-text-stale-raster.md: macOS kept a wrapped
// paragraph's raster once it shrank below the raster size). The page answer was right; a workaround made the row's
// status and the editor's status line new nodes per status. Main #327 retires the old ink, so they are plain texts
// again, as the reference's.
test('the list row and the editor read the same status, each one text the host repaints (fixed on main by #327)', async () => {
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
  const source = await Bun.file(appPath('providers.contract')).text();
  expect(source).not.toContain('key=status');
  expect(source).toContain('                            text row.status font-size="0.75rem" line-height="1.125rem"');
  expect(source).toContain('        row width="100%" min-width=0 padding-top="0.125rem" gap="0.375rem" flex-wrap="wrap" align-items="baseline" testId="provider-editor-status"');
});

test('new settings use instance mutations for enablement and reset without the removed providers map', async () => {
  const server = new FakeServer();
  server.settings = { providerInstances: {} }; server.config = server.snapshot();
  expect(providerRows(server.settings, []).map(row => [row.id, row.isDirty])).toEqual([['codex', false], ['claudeAgent', false]]);
  expect(providerPage(server, '', 0).rows.map(row => row.enabled)).toEqual([true, true]);
  await runProviderOp(server, native, 'provider-enabled', 'codex', 'false');
  expect(obj(obj(server.settings.providerInstances).codex).enabled).toBe(false);
  expect(writes(server).every(call => obj(obj(call.payload).patch).providers === undefined)).toBe(true);
  await runProviderOp(server, native, 'provider-reset', 'codex', '');
  expect(obj(server.settings.providerInstances).codex).toBeUndefined();
});

// ProviderModelsSection's name tooltip goes to the window's hover layer (kind "model-name": 600 ms, name over the mono slug).
test('a model name hands its tooltip to the hover layer, with the slug when it differs', () => {
  const line = contractComponent('providers.contract', 'ProviderModelLine');
  const { hoverTipAtFrame } = contractFns('hover-layer.contract', ['HoverTip']);
  const tips: unknown[][] = [];
  const scope = { rem: 16, instanceId: 'codex', model: { slug: 'gpt-6-astra', name: 'GPT-6 Astra', showSlug: true }, hoverTipAtFrame,
    frame: (id: string) => ({ id, x: 40, y: 200, width: 120, height: 16 }), hoverTipAt: (...args: unknown[]) => { tips.push(args); } };
  const state = line.state();
  line.call('nameTip', state, scope, [true]);
  line.call('nameTip', state, scope, [false]);
  expect(tips.map(([tip, part, inside]) => [obj(tip).kind, obj(tip).text, obj(tip).lines, obj(tip).side, obj(tip).align, obj(tip).x, part, inside])).toEqual([
    ['model-name', 'GPT-6 Astra', ['gpt-6-astra'], 'top', 'start', 40, 'trigger', true], ['model-name', 'GPT-6 Astra', ['gpt-6-astra'], 'top', 'start', 40, 'trigger', false]]);
  const { hoverOpenDelay } = contractFns('hover-layer.contract', ['HoverTip']);
  expect(hoverOpenDelay('model-name')).toBe(600);
});
