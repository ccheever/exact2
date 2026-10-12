// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): ProviderSettingsPanel.environment.test.tsx
// (:285 "routes refresh and provider update commands to the selected environment", original name),
// against a fake environment: a press sends the op its button sends. Clone cases follow for the
// advisory popover (ProviderInstanceCard versionAdvisoryNode), Update all (ProviderUpdatesAction),
// the launch notification (ProviderUpdatePrimaryNotification) and the custom model editor's Save.
import { describe, expect, it, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { providerPage, runProviderOp } from './providers';
import { advisoryView, providerUpkeepOp, runProviderUpdate, runUpdateAll, updateAllView, withUpkeep } from './providers-upkeep';
import { primaryTarget, providerUpdates, resetProviderUpdateNotifications, runLaunchUpdates, type UpdateTarget } from './provider-update-notify';
import { primaryAt, primaryOff, resetPrimary } from './local-primary-fixture';
import { toasts } from './toast';
import { adoptShellPrefs } from './shell-prefs';
import type { T3Client } from './client';

const codex = (extra: Obj = {}): Obj => ({ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, version: '1.0.0', status: 'ready', auth: { status: 'authenticated' },
  checkedAt: '2026-07-24T12:00:00.000Z', models: [], slashCommands: [], skills: [],
  versionAdvisory: { status: 'behind_latest', currentVersion: '1.0.0', latestVersion: '1.1.0', updateCommand: 'pnpm add -g @openai/codex@latest', canUpdate: true, checkedAt: '2026-07-24T12:00:00.000Z', message: 'Update available.' }, ...extra });

class Fake implements Native {
  available = true; writable = true; ready = true; generation = 1; environmentId = 'remote-device'; preferencesLoaded = true;
  local = { favoriteModels: [] as string[] } as Obj & { favoriteModels: string[] };
  config: Obj; calls: { method: string; payload: Obj }[] = []; ops: Obj[] = [];
  handlers: Record<string, (payload: Obj) => Obj | Promise<Obj>> = { 'server.getConfig': () => this.config };
  constructor(providers: Obj[] = [codex()], settings: Obj = { providerInstances: {}, providers: { codex: { enabled: true, binaryPath: 'codex', customModels: [] } } }) { this.config = { environment: { label: 'Remote device' }, settings, providers }; }
  watch() {}
  async later(request: unknown) { this.ops.push(request as Obj); return { ok: true, generation: this.generation, value: {} }; }
  async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> { this.calls.push({ method, payload }); return (this.handlers[method] ?? (() => ({})))(payload); }
  async call(_native: Native, request: unknown): Promise<Obj> { const value = obj(request); return this.rpc(this, String(value.method), obj(value.payload)); }
  called(method: string) { return this.calls.filter(call => call.method === method).map(call => call.payload); }
}
const target = (fake: Fake, label = 'Remote device', key = 'focus'): UpdateTarget => ({ key, label, providers: fake.config.providers as Obj[], request: (method, payload) => fake.rpc(fake, method, payload) });

describe('EnvironmentProviderSettings routing', () => {
  it('routes refresh and provider update commands to the selected environment', async () => {
    const fake = new Fake();
    await runProviderOp(fake, fake, 'provider-refresh', '', '{}');
    expect(fake.called('server.refreshProviders')).toEqual([{ refreshModels: true }]);
    const card = withUpkeep(fake, providerPage(fake, 'codex', 0)).editors[0]!.update;
    expect(card.action).toBe('Update now'); // onRunUpdate is offered
    await providerUpkeepOp(fake, fake, 'upkeep:update', 'codex', '', '');
    expect(fake.called('server.updateProvider')).toEqual([{ provider: 'codex', instanceId: 'codex' }]);
  });
});

// --- Clone cases ------------------------------------------------------------------------------

test('the advisory popover: one view for the row and the editor; Install vX, Update now, read-only, running', () => {
  const fake = new Fake();
  const page = withUpkeep(fake, providerPage(fake, 'codex', 0));
  const row = page.rows.find(entry => entry.id === 'codex')!.update, editor = page.editors[0]!.update;
  expect([row.mode, editor.mode, row.popId, editor.popId]).toEqual(['list', 'editor', 'provider-advisory-list-codex', 'provider-advisory-editor-codex']);
  const { mode: _a, popId: _b, ...rowRest } = row, { mode: _c, popId: _d, ...editorRest } = editor;
  expect(rowRest).toEqual(editorRest);
  expect(editor).toMatchObject({ show: true, label: 'Update available — view details', title: 'Update available', detail: 'Update available.', warning: false, strong: false,
    action: 'Update now', command: 'pnpm add -g @openai/codex@latest', divider: true, updating: false });
  const recommended = codex({ compatibilityAdvisory: { status: 'unsupported', latestVersionStatus: 'supported', message: 'Codex 1.0.0 is not supported. Install v1.0.5.', recommendedVersion: '1.0.5', recommendedRange: '>=1.0.5' },
    versionAdvisory: { status: 'behind_latest', latestVersion: '1.1.0', updateCommand: 'npm i -g codex', canUpdate: true, canInstallVersion: true } });
  expect(advisoryView(fake, 'codex', recommended, true, 'editor')).toMatchObject({ title: 'Unsupported version', strong: true, warning: true, action: 'Install v1.0.5', target: '1.0.5', command: '', divider: false });
  expect(advisoryView(fake, 'codex', { ...recommended, compatibilityAdvisory: { ...obj(recommended.compatibilityAdvisory), status: 'graceful' } }, true, 'editor')).toMatchObject({ title: 'Limited support', strong: false, warning: true });
  expect(advisoryView(fake, 'codex', codex({ updateState: { status: 'running' } }), true, 'list')).toMatchObject({ action: 'Updating', updating: true });
  expect(advisoryView(fake, 'codex', codex({ versionAdvisory: { status: 'behind_latest', latestVersion: '1.1.0', updateCommand: 'npm i', canUpdate: false } }), true, 'list')).toMatchObject({ action: '', command: 'npm i', divider: false });
  fake.writable = false;
  expect(advisoryView(fake, 'codex', codex(), true, 'editor')).toMatchObject({ show: true, action: '', command: 'pnpm add -g @openai/codex@latest' });
  expect(advisoryView(fake, 'codex', codex({ versionAdvisory: { status: 'current' } }), true, 'editor').show).toBe(false);
});

test('Update now: one request at a time per instance, Updating while it runs, a failure toasts the provider', async () => {
  const fake = new Fake();
  let release!: (value: Obj) => void;
  fake.handlers['server.updateProvider'] = () => new Promise<Obj>(resolve => { release = resolve; });
  const running = runProviderUpdate(fake, target(fake), 'codex', '');
  expect(advisoryView(fake, 'codex', codex(), true, 'editor')).toMatchObject({ action: 'Updating', updating: true });
  await runProviderUpdate(fake, target(fake), 'codex', '');
  expect(fake.called('server.updateProvider')).toHaveLength(1);
  release({ providers: [] });
  await running;
  expect(advisoryView(fake, 'codex', codex(), true, 'editor').updating).toBe(false);
  fake.handlers['server.updateProvider'] = () => { throw new Error('npm exited with code 1.'); };
  await runProviderUpdate(fake, target(fake), 'codex', '');
  expect(toasts(fake as unknown as T3Client).map(toast => [toast.kind, toast.title, toast.description, toast.stacked])).toEqual([['error', 'Could not update Codex', 'npm exited with code 1.', true]]);
});

test('Update all: every machine with a one-click update, one toast with a line per failure, hidden without candidates', async () => {
  const studio = new Fake([codex(), codex({ instanceId: 'claudeAgent', driver: 'claudeAgent', versionAdvisory: { status: 'behind_latest', latestVersion: '2.1.0', updateCommand: 'npm i -g @anthropic-ai/claude-code', canUpdate: true } })]);
  // Two Codex instances that disagree on the command are left to provider settings (no one-click).
  const laptop = new Fake([codex(), codex({ instanceId: 'codex_work', versionAdvisory: { status: 'behind_latest', latestVersion: '1.1.0', updateCommand: 'bun add -g @openai/codex', canUpdate: true } }),
    codex({ instanceId: 'cursor', driver: 'cursor', versionAdvisory: { status: 'behind_latest', latestVersion: '0.3.0', updateCommand: 'cursor update', canUpdate: true } })]);
  const targets = [target(studio, 'Mac Studio', 'studio'), target(laptop, 'Laptop', 'laptop')];
  expect(updateAllView(studio, targets)).toEqual({ show: true, pending: false, label: 'Update all', tip: 'Mac Studio: Codex, Claude\nLaptop: Cursor' });
  studio.handlers['server.updateProvider'] = payload => ({ providers: [{ instanceId: payload.instanceId, driver: payload.provider, updateState: payload.provider === 'claudeAgent' ? { status: 'failed', message: 'npm exited with code 1.' } : { status: 'succeeded' } }] });
  let release!: (value: Obj) => void;
  laptop.handlers['server.updateProvider'] = () => new Promise<Obj>(resolve => { release = resolve; });
  const run = runUpdateAll(studio, targets);
  expect(updateAllView(studio, targets)).toMatchObject({ pending: true, label: 'Updating…' });
  await runUpdateAll(studio, targets); // ignored while in flight
  release({ providers: [{ instanceId: 'cursor', driver: 'cursor', updateState: { status: 'succeeded' } }] });
  await run;
  expect([studio.called('server.updateProvider'), laptop.called('server.updateProvider')]).toEqual([
    [{ provider: 'codex', instanceId: 'codex' }, { provider: 'claudeAgent', instanceId: 'claudeAgent' }], [{ provider: 'cursor', instanceId: 'cursor' }]]);
  expect(toasts(studio as unknown as T3Client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', '1 of 3 provider updates failed', 'Mac Studio · Claude: npm exited with code 1.']]);
  expect(updateAllView(studio, [target(new Fake([codex({ versionAdvisory: { status: 'current' } })]))]).show).toBe(false);
});

test('the launch notification: one prompt, Update runs the one-click providers, the outcome follows updateState', async () => {
  resetProviderUpdateNotifications();
  const fake = new Fake();
  const client = fake as unknown as T3Client;
  providerUpdates(fake as never, target(fake));
  const prompt = toasts(client)[0]!;
  expect([prompt.kind, prompt.title, prompt.description, prompt.action?.label, prompt.action?.op, prompt.secondary?.label]).toEqual(['warning', 'Update Available: Codex v1.1.0', 'Install the update now or review provider settings.', 'Update', 'upkeep:update-launch', 'Settings']);
  providerUpdates(fake as never, target(fake)); // shown once per update set
  expect(toasts(client)).toHaveLength(1);
  fake.handlers['server.updateProvider'] = () => ({ providers: [codex({ updateState: { status: 'running' } })] });
  await runLaunchUpdates(fake as never, prompt.action!.id!, target(fake));
  expect(fake.called('server.updateProvider')).toEqual([{ provider: 'codex', instanceId: 'codex' }]);
  expect(toasts(client)).toEqual([]); // the prompt closed; running shows in the sidebar pill, not a toast
  fake.config.providers = [codex({ updateState: { status: 'unchanged', message: 'still old' } })];
  providerUpdates(fake as never, target(fake));
  expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description, toast.action?.label])).toEqual([['warning', 'Provider still needs an update', 'Codex still appears outdated. Check provider settings for details.', 'Settings']]);
});

test('the launch prompt follows the primary only: with no primary there is no prompt, whatever is focused', () => {
  // ProviderUpdateLaunchNotification is mounted only for an authenticated primary and reads primaryServerProvidersAtom.
  resetProviderUpdateNotifications();
  const fake = new Fake(); // an outdated Codex on the focused (remote) environment
  resetPrimary();
  expect(primaryTarget(fake as never, null)).toBeNull();
  providerUpdates(fake as never); // the source is primaryTarget
  expect(toasts(fake as unknown as T3Client)).toEqual([]);
  primaryOff(); // the Local environment switched off: the same
  providerUpdates(fake as never);
  expect(toasts(fake as unknown as T3Client)).toEqual([]);
  // A primary that is not the focus and has no background connection: still nothing (its providers are unknown).
  primaryAt('http://127.0.0.1:16437', 'env-local');
  expect(primaryTarget(fake as never, null)).toBeNull();
  // The focused environment is the primary: its providers prompt.
  fake.environmentId = 'env-local';
  expect(primaryTarget(fake as never, null)?.key).toBe('focus:env-local');
  providerUpdates(fake as never);
  expect(toasts(fake as unknown as T3Client).map(toast => toast.title)).toEqual(['Update Available: Codex v1.1.0']);
  resetPrimary();
});

test('the launch outcome: succeeded leaves after 3 s; a rejected request is the error toast', async () => {
  resetProviderUpdateNotifications();
  const fake = new Fake([codex({ versionAdvisory: { status: 'behind_latest', latestVersion: '1.2.0', updateCommand: 'pnpm add -g @openai/codex@latest', canUpdate: true } })]);
  const client = fake as unknown as T3Client;
  providerUpdates(fake as never, target(fake));
  fake.handlers['server.updateProvider'] = () => ({ providers: [codex({ version: '1.2.0', versionAdvisory: { status: 'current' }, updateState: { status: 'succeeded' } })] });
  await runLaunchUpdates(fake as never, toasts(client)[0]!.action!.id!, target(fake));
  expect(toasts(client).map(toast => [toast.kind, toast.title, toast.timeoutMs])).toEqual([['success', 'Provider updated', 3000]]);
  const second = new Fake([codex({ versionAdvisory: { status: 'behind_latest', latestVersion: '1.3.0', updateCommand: 'pnpm add -g @openai/codex@latest', canUpdate: true } })]);
  providerUpdates(second as never, target(second));
  second.handlers['server.updateProvider'] = () => { throw new Error('WebSocket closed'); };
  await runLaunchUpdates(second as never, toasts(second as unknown as T3Client)[0]!.action!.id!, target(second));
  expect(toasts(second as unknown as T3Client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Provider update failed', 'WebSocket closed']]);
});

test('Copy update command copies and toasts; the custom model editor saves through the instance upsert', async () => {
  const fake = new Fake([codex({ models: [{ slug: 'gpt-5', name: 'GPT-5', isCustom: false, capabilities: { optionDescriptors: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'high', label: 'High' }] }] } }] })],
    { providerInstances: { codex: { driver: 'codex', enabled: true, config: { customModels: ['gpt-x'] } } }, providers: {} });
  fake.handlers['server.getSettings'] = () => fake.config.settings as Obj;
  fake.handlers['server.getConfig'] = () => fake.config;
  fake.handlers['server.updateSettings'] = payload => { const mutation = obj(payload.providerInstanceMutation); fake.config.settings = { ...obj(fake.config.settings), providerInstances: { codex: mutation.instance as Obj } }; return {}; };
  await providerUpkeepOp(fake, fake, 'upkeep:copy-command', 'codex', '', '');
  expect(fake.ops[0]).toEqual({ op: 'copyText', text: 'pnpm add -g @openai/codex@latest' });
  expect(toasts(fake as unknown as T3Client).map(toast => [toast.title, toast.description])).toEqual([['Codex update command copied', 'Run it in a terminal when you are ready to update.']]);
  await providerUpkeepOp(fake, fake, 'upkeep:cm-open', 'codex', 'gpt-x', '');
  let editor = withUpkeep(fake, providerPage(fake, 'codex', 0)).editors[0]!;
  expect([editor.models.find(model => model.slug === 'gpt-x')?.editing, editor.upkeep.modelEditor[0]?.slug]).toEqual([true, 'gpt-x']);
  expect(withUpkeep(fake, providerPage(fake, 'codex', 0)).escapeOwned).toBe(true); // the editor's Cancel owns Escape
  await providerUpkeepOp(fake, fake, 'upkeep:cm-copy-from', 'codex', 'gpt-5', '');
  await providerUpkeepOp(fake, fake, 'upkeep:cm-text', 'codex', 'name', 'GPT X');
  await providerUpkeepOp(fake, fake, 'upkeep:cm-save', 'codex', '', '');
  expect(obj(obj(obj(fake.config.settings).providerInstances).codex).config).toEqual({ customModels: [{ slug: 'gpt-x', name: 'GPT X',
    capabilities: { optionDescriptors: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'high', label: 'High' }] }] } }] });
  editor = withUpkeep(fake, providerPage(fake, 'codex', 0)).editors[0]!;
  expect(editor.upkeep.modelEditor).toEqual([]);
  expect(withUpkeep(fake, providerPage(fake, 'codex', 0)).escapeOwned).toBe(false);
});

test('dismissed launch prompts are all kept (dismissProviderUpdateNotification has no limit)', () => {
  const keys = Array.from({ length: 150 }, (_, index) => `codex:1.${index}.0`);
  const next: Obj = {};
  adoptShellPrefs(next, { shell: { providerUpdateDismissals: keys } });
  expect((obj(next.shell).providerUpdateDismissals as string[]).length).toBe(150);
});
