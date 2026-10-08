// ManagedCodexSetup (CodexSetupSection.tsx at T3 Code 1e2ecbd975, MIT; see LICENSE-T3) has no
// reference test, so these cases are behavioral: a fake environment and native module answer the
// setup's requests, a test renders by asking for the view (Settings › Providers, the welcome card,
// the Add ChatGPT account dialog) and presses by sending the op its button sends. The snapshot
// answer's effects run as `codexSetupPrepare`. No OpenAI account is involved.
import { describe, expect, test } from 'bun:test';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { providerPage, providerSetupStreams, providerWizard, runProviderOp, wizardSetupStreams, type ProviderHost } from './providers';
import { providerSetupEvent, watchProviderSetup } from './provider-setup';
import { codexFlow, codexSetupView, readCodexSetupMode } from './codex-setup';
import { codexSetupOp, codexSetupPrepare, RECEIVE_FAILED, type CodexContext } from './codex-setup-ops';
import { codexHandoffEvent } from './codex-handoff-events';

const instanceId = 'codex_personal';
const port = 16241, state = 'a'.repeat(43);
const authorizationUrl = `https://auth.openai.com/api/accounts/authorize?client_id=dynamic_agent_client&response_type=code&redirect_uri=${encodeURIComponent(`http://127.0.0.1:${port}/auth/callback`)}&state=${state}&code_challenge_method=S256&code_challenge=${'b'.repeat(43)}`;
const callbackUrl = `http://127.0.0.1:${port}/auth/callback?state=${state}&code=one-time-code&client_id=oaiapp_test`;
const provider = (extra: Obj = {}): Obj => ({ instanceId, driver: 'codex', displayName: 'ChatGPT - Personal', enabled: true, installed: false, status: 'warning',
  auth: { status: 'unauthenticated' }, setup: { canInstall: true, canAuthenticate: true }, models: [], ...extra });
const auth = (patch: Obj = {}): Obj => ({ instanceId, phase: 'idle', flowId: null, authorizationUrl: null, expiresAt: null, message: null,
  methods: [{ id: 'chatgpt', name: 'ChatGPT', description: null, type: 'agent' }, { id: 'chatgpt-change-account', name: 'Use a different account', description: null, type: 'agent' }], ...patch });
const waiting = (flowId = 'flow-1', patch: Obj = {}) => auth({ phase: 'waiting', flowId, authorizationUrl, interaction: { type: 'browser', id: flowId, url: authorizationUrl, requiresConsent: false, acceptsCallback: true }, ...patch });
const install = (patch: Obj = {}): Obj => ({ driver: 'codex', operationId: null, phase: 'idle', downloadedBytes: 0, totalBytes: null, version: '0.156.1', installedVersion: null,
  executablePath: null, source: null, canRemove: false, message: null, ...patch });
const installed = (patch: Obj = {}) => install({ phase: 'succeeded', installedVersion: '0.156.1', source: 'managed', executablePath: '/lane/t3/codex/0.156.1/codex', ...patch });

/** One environment and the app's native module: every server call and native op, in order. */
class Fake implements ProviderHost, Native {
  ready = true; writable = true; generation = 1; available = true; local = { favoriteModels: [] as string[] };
  origin = 'http://127.0.0.1:16240'; environmentId = 'env-lane'; ids?: (native: Native, count: number) => Promise<string[]>;
  config: Obj; log: string[] = []; calls: { method: string; payload: Obj }[] = []; ops: Obj[] = []; serial = 0;
  subs: Record<string, string> = {}; handlers: Record<string, (payload: Obj) => Obj | Promise<Obj>> = {};
  /** What `codexAuthTake` answers next, and what the listener was asked. */
  taken: Obj = { phase: 'waiting' }; startFailure = '';
  constructor(live: Obj[] = [provider()]) {
    this.config = { environment: { label: 'This Mac' }, settings: { providers: {}, providerInstances: { [instanceId]: { driver: 'codex', displayName: 'ChatGPT - Personal', enabled: true, config: { enabled: true, setupMode: 'managed' } } } }, providers: live };
  }
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), op = str(request.op);
    this.ops.push(request); this.log.push(`native:${op}`);
    const ok = (value: Obj) => ({ ok: true, generation: this.generation, value });
    if (op === 'subscribe') { const id = `sub-${++this.serial}`; this.subs[str(request.key)] = id; return ok({ id }); }
    if (op === 'unsubscribe') { delete this.subs[str(request.key)]; return ok({}); }
    if (op === 'remoteEditorsOpen') return ok({ opened: true, recorded: true });
    if (op === 'codexAuthStart') return this.startFailure ? { ok: false, generation: this.generation, error: { kind: 'CodexAuth', message: this.startFailure, uncertain: false } } : ok({ listening: true });
    if (op === 'codexAuthTake') { const value = this.taken; this.taken = { phase: 'none' }; return ok(value); }
    return ok({});
  }
  async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> {
    this.calls.push({ method, payload }); this.log.push(`rpc:${method}`);
    const handler = this.handlers[method];
    return handler ? handler(payload) : {};
  }
  emit(kind: 'auth' | 'install', value: Obj, id = instanceId) {
    const key = `provider-${kind}:${id}`;
    providerSetupEvent(this, { generation: this.generation, key, subscriptionId: this.subs[key], value });
  }
  context(extra: Partial<CodexContext> = {}): CodexContext { return { host: this, native: this, module: this, instanceId, origin: this.origin, environmentId: this.environmentId, primary: null, ...extra }; }
  setup() { return providerPage(this, instanceId, 0).editors[0]!.setup.codex[0]!; }
  async show() { await watchProviderSetup(this, this, 'page', providerSetupStreams(this, instanceId)); }
  press(op: string, value = '', token = this.setup().token) { return codexSetupOp(this.context(), op, token, value, async () => { this.log.push('saved'); }); }
  prepare(extra: Partial<CodexContext> = {}) { return codexSetupPrepare([this.context(extra)]); }
  methods(name: string) { return this.calls.filter(call => call.method === name); }
  nativeOps(name: string) { return this.ops.filter(op => op.op === name); }
}
async function rendered(state: Obj | null = auth(), installation: Obj | null = install(), fake = new Fake()) {
  await fake.show();
  if (state) fake.emit('auth', state);
  if (installation) fake.emit('install', installation);
  return fake;
}

describe('managed Codex setup (CodexSetupSection)', () => {
  test('readCodexSetupMode: only an explicit "managed" is managed', () => {
    expect(readCodexSetupMode({ setupMode: 'managed' })).toBe('managed');
    expect(readCodexSetupMode({ setupMode: 'existing' })).toBe('existing');
    expect(readCodexSetupMode({})).toBe('existing');
    expect(readCodexSetupMode(null)).toBe('existing');
  });

  test('Settings mounts the ChatGPT account row for a managed instance with both streams, and folds its runtime paths', async () => {
    const fake = new Fake([provider({ runtimePaths: { homePath: '/lane/home/.codex', shadowHomePath: '/lane/t3/providers/codex/codex_personal/shadow' } })]);
    const editor = providerPage(fake, instanceId, 0).editors[0]!;
    expect(editor.setup.kind).toBe('codex');
    expect(editor.fields).toEqual([]);
    expect(providerSetupStreams(fake, instanceId)).toEqual({ auth: [instanceId], install: [instanceId] });
    await fake.show(); fake.emit('install', installed());
    expect(providerPage(fake, instanceId, 0).editors[0]!.codexRuntime).toEqual([{ key: instanceId, binaryPath: '/lane/t3/codex/0.156.1/codex', binaryPlaceholder: 'Not installed',
      homePath: '/lane/home/.codex', shadowPath: '/lane/t3/providers/codex/codex_personal/shadow', shadowPlaceholder: 'Not used', shadowDescription: 'Account-specific home sharing the Codex state above.' }]);
    // An environment without setup yet: "Preparing sign-in."
    const preparing = new Fake([provider({ setup: undefined })]);
    expect(providerPage(preparing, instanceId, 0).editors[0]!.setup.codex[0]!.kind).toBe('preparing');
  });

  test('install then sign-in: labels in order, one browser open per flow, complete carries the callback, finishing until the snapshot flips', async () => {
    const fake = await rendered();
    expect(fake.setup().description).toBe('Use your ChatGPT subscription.');
    expect(fake.setup().controlLabel).toBe('Continue with ChatGPT');
    await fake.press('setup:codex-setup');
    expect(fake.methods('provider.install.start')).toEqual([{ method: 'provider.install.start', payload: { instanceId } }]);
    expect(fake.methods('provider.auth.start')).toEqual([]);
    const labels: string[] = [];
    for (const phase of [install({ phase: 'downloading', operationId: 'op-1', downloadedBytes: 12_400_000, totalBytes: 127_394_863 }), install({ phase: 'extracting', operationId: 'op-1' }), install({ phase: 'verifying', operationId: 'op-1' })]) {
      fake.emit('install', phase); await fake.prepare(); labels.push(fake.setup().description);
    }
    expect(labels).toEqual(['Downloading 12.4 of 127.4 MB.', 'Installing Codex.', 'Checking Codex.']);
    expect(fake.methods('provider.auth.start')).toEqual([]);
    fake.emit('install', installed({ operationId: 'op-1' })); await fake.prepare();
    // continueWithSignIn: the install succeeded, so the same press signs in, in client callback mode, without a returnUrl.
    expect(fake.methods('provider.auth.start')).toEqual([{ method: 'provider.auth.start', payload: { instanceId, methodId: 'chatgpt', callbackMode: 'client' } }]);
    fake.emit('auth', auth({ phase: 'starting', flowId: 'flow-1' })); await fake.prepare();
    expect(fake.setup().control).toBe('waiting');
    fake.emit('auth', waiting()); await fake.prepare(); await fake.prepare();
    expect(fake.nativeOps('codexAuthStart')).toEqual([{ op: 'codexAuthStart', authorizationUrl }]);
    expect(fake.nativeOps('remoteEditorsOpen')).toEqual([]);
    expect(fake.setup().controlLabel).toBe('Open sign-in page');
    // "Open sign-in page" again re-opens the browser on the listener the flow already holds.
    await fake.press('setup:codex-open');
    expect(fake.nativeOps('codexAuthStart')).toHaveLength(1);
    expect(fake.nativeOps('remoteEditorsOpen')).toEqual([{ op: 'remoteEditorsOpen', url: authorizationUrl }]);
    fake.taken = { phase: 'received', callbackUrl }; await fake.prepare();
    expect(fake.methods('provider.auth.complete')).toEqual([{ method: 'provider.auth.complete', payload: { instanceId, flowId: 'flow-1', callbackUrl } }]);
    fake.emit('auth', auth({ phase: 'verifying', flowId: 'flow-1' })); await fake.prepare();
    fake.emit('auth', auth({ phase: 'succeeded', flowId: 'flow-1' })); await fake.prepare(); await fake.prepare();
    expect(fake.methods('server.refreshProviders')).toEqual([{ method: 'server.refreshProviders', payload: { instanceId } }]);
    expect(fake.setup().description).toBe('Finishing sign-in...');
    expect(fake.setup().controlLabel).toBe('Finishing sign-in...');
    expect(fake.setup().showCancel).toBe(false);
    fake.config.providers = [provider({ installed: true, status: 'ready', auth: { status: 'authenticated', type: 'chatgpt', label: 'ChatGPT', email: 'person@example.com', subscriptionSharing: true, profileId: 'oaiapp_one' } })];
    await fake.prepare();
    const signedIn = fake.setup();
    expect([signedIn.control, signedIn.description, signedIn.email, signedIn.usage]).toEqual(['authenticated', '', 'person@example.com', true]);
    expect(signedIn.emailPlaceholder).not.toBe('person@example.com');
    expect(fake.log.filter(entry => entry.startsWith('rpc:provider.') || entry === 'native:codexAuthStart')).toEqual(['rpc:provider.install.start', 'rpc:provider.auth.start', 'native:codexAuthStart', 'rpc:provider.auth.complete']);
  });

  test('saved profiles: Reconnect sends chatgpt-profile:<id>, Use a different account sends chatgpt-change-account', async () => {
    const methods = [{ id: 'chatgpt', name: 'ChatGPT', description: null, type: 'agent', accountEmail: 'person@example.com' }, { id: 'chatgpt-change-account', name: 'Use a different account', description: null, type: 'agent' },
      { id: 'chatgpt-profile:oaiapp_one', name: 'person@example.com · Connection 1', description: null, type: 'agent', accountEmail: 'person@example.com' }];
    const fake = await rendered(auth({ methods }), installed());
    const view = fake.setup();
    expect([view.savedAccount, view.controlLabel, view.description]).toEqual([true, 'Reconnect account', 'person@example.com']);
    const picker = providerWizard(fake, true, 7, 'codex', false, '', false, '', 'codex-picker', instanceId).codexPicker;
    expect(picker).toEqual([{ key: instanceId, instanceId, target: instanceId, token: view.token, profiles: [{ id: 'chatgpt-profile:oaiapp_one', name: 'person@example.com · Connection 1' }], first: 'chatgpt-profile:oaiapp_one' }]);
    await fake.press('setup:codex-pick', 'chatgpt-profile:oaiapp_one');
    fake.emit('auth', waiting('flow-2', { methods })); await fake.prepare();
    expect(fake.setup().description).toBe('Continue as person@example.com on OpenAI.');
    await fake.press('setup:codex-cancel');
    fake.emit('auth', auth({ phase: 'cancelled', message: 'Sign-in cancelled.', methods })); await fake.prepare();
    await fake.press('setup:codex-different');
    fake.emit('auth', auth({ phase: 'failed', message: 'x', methods }));
    await fake.press('setup:codex-pick', 'chatgpt-profile:oaiapp_gone');
    expect(fake.methods('provider.auth.start').map(call => call.payload.methodId)).toEqual(['chatgpt-profile:oaiapp_one', 'chatgpt-change-account', 'chatgpt-change-account']);
  });

  test('Having trouble signing in?: one complete per pasted URL, for the flow its field was drawn for; Try sign-in opens the page', async () => {
    const fake = await rendered(waiting(), installed());
    const view = fake.setup();
    expect([view.callback, view.flowId, view.formKey]).toEqual([true, 'flow-1', 'flow-1:0']);
    await fake.press('setup:codex-callback', `  ${callbackUrl}  `, 'flow-1');
    await fake.press('setup:codex-callback', callbackUrl, 'flow-stale');
    expect(fake.methods('provider.auth.complete')).toEqual([{ method: 'provider.auth.complete', payload: { instanceId, flowId: 'flow-1', callbackUrl } }]);
    expect(fake.setup().formKey).toBe('flow-1:1');
    await fake.press('setup:codex-browser');
    expect(fake.nativeOps('remoteEditorsOpen')).toEqual([{ op: 'remoteEditorsOpen', url: authorizationUrl }]);
  });

  test('cancel releases the listener and restores the start state; a failed listener shows the desktop message', async () => {
    const fake = await rendered(auth(), installed());
    await fake.press('setup:codex-setup');
    fake.emit('auth', waiting()); await fake.prepare();
    expect(fake.nativeOps('codexAuthStart')).toHaveLength(1);
    await fake.press('setup:codex-cancel');
    expect(fake.methods('provider.auth.cancel')).toEqual([{ method: 'provider.auth.cancel', payload: { instanceId, flowId: 'flow-1' } }]);
    fake.emit('auth', auth({ phase: 'cancelled', flowId: 'flow-1', message: 'Sign-in cancelled.' }));
    fake.taken = { phase: 'failed', message: 'Sign-in cancelled on this computer.' };
    await fake.prepare();
    expect(fake.nativeOps('codexAuthCancel')).toEqual([{ op: 'codexAuthCancel', authorizationUrl }]);
    const view = fake.setup();
    expect([view.control, view.controlLabel, view.description, view.error]).toEqual(['connect', 'Continue with ChatGPT', 'Sign-in cancelled.', RECEIVE_FAILED]);
    // A port in use (or any listener failure) is the reference's one message; the start state stays usable.
    const busy = await rendered(auth(), installed());
    busy.startFailure = 'The ChatGPT callback port is in use on this computer. Close the other sign-in and try again, or paste the redirect URL in T3 Code.';
    await busy.press('setup:codex-setup'); busy.emit('auth', waiting()); await busy.prepare();
    expect(busy.setup().error).toBe('Could not finish sign-in on this computer. Try again or paste the redirect URL below.');
    expect(busy.setup().callback).toBe(true);
  });

  test('Disconnect signs out once and shows the "Signed out locally." notice; a hidden setup lets its listener go', async () => {
    const fake = await rendered(auth({ phase: 'idle' }), installed(), new Fake([provider({ installed: true, status: 'ready', auth: { status: 'authenticated', type: 'chatgpt', subscriptionSharing: true } })]));
    expect(fake.setup().control).toBe('authenticated');
    expect(fake.setup().description).toBe('Signed in with ChatGPT.');
    await fake.press('setup:codex-logout'); await fake.press('setup:codex-logout');
    expect(fake.methods('provider.auth.logout')).toEqual([{ method: 'provider.auth.logout', payload: { instanceId } }]);
    fake.config.providers = [provider({ installed: true })];
    fake.emit('auth', auth({ message: 'Signed out locally. ChatGPT could not be reached to revoke the session.' }));
    expect(fake.setup().logoutWarning).toBe('Signed out locally. ChatGPT could not be reached to revoke the session.');
    // Unmounted: the listener goes, silently.
    const hidden = await rendered(auth(), installed());
    await hidden.press('setup:codex-setup'); hidden.emit('auth', waiting()); await hidden.prepare();
    await watchProviderSetup(hidden, hidden, 'page', { auth: [], install: [] });
    await hidden.prepare();
    expect(hidden.nativeOps('codexAuthCancel')).toEqual([{ op: 'codexAuthCancel', authorizationUrl }]);
    expect(codexFlow(hidden, instanceId).error).toBe('');
  });

  test('a stale press sends nothing (pendingRef)', async () => {
    const fake = await rendered(auth(), installed());
    const token = fake.setup().token;
    await fake.press('setup:codex-setup', '', token);
    await fake.press('setup:codex-setup', '', token);
    fake.emit('auth', auth({ phase: 'starting', flowId: 'flow-1' }));
    await fake.press('setup:codex-setup', '', token);
    expect(fake.methods('provider.auth.start')).toHaveLength(1);
  });

  test('the welcome card: Continue with ChatGPT switches to managed and starts on its own; Use existing CLI switches back', async () => {
    const fake = new Fake([provider({ installed: true, status: 'warning' })]);
    fake.config.settings = { providers: { codex: { enabled: true, binaryPath: 'codex' } }, providerInstances: {} };
    fake.handlers['server.getSettings'] = () => obj(fake.config.settings);
    fake.handlers['server.getConfig'] = () => fake.config;
    const existing = codexSetupView(fake, instanceId, provider({ installed: true }), { presentation: 'onboarding', mode: 'existing', enabled: true, readOnly: false, allowExistingCli: true });
    expect([existing.kind, existing.control, existing.secondary]).toEqual(['existing', 'connect', 'existing']);
    await codexSetupOp(fake.context(), 'setup:codex-mode', existing.token, 'managed', async () => {});
    const write = fake.methods('server.updateSettings')[0]!.payload;
    expect(obj(write.providerInstanceMutation)).toEqual({ operation: 'upsert', instanceId, instance: { driver: 'codex', enabled: true, config: { enabled: true, setupMode: 'managed' } } });
    expect(codexFlow(fake, instanceId).autoStart).toBe(true);
    const view = () => codexSetupView(fake, instanceId, provider(), { presentation: 'onboarding', mode: 'managed', enabled: true, readOnly: false, allowExistingCli: true });
    await watchProviderSetup(fake, fake, 'welcome', { auth: [instanceId], install: [instanceId] });
    expect(view().description).toBe('Complete sign-in in your browser.');
    expect(view().control).toBe('waiting');
    await fake.prepare();
    expect(fake.methods('provider.install.start')).toEqual([]);
    fake.emit('install', install()); await fake.prepare();
    expect(fake.methods('provider.install.start')).toHaveLength(1);
    await fake.prepare();
    expect(fake.methods('provider.install.start')).toHaveLength(1);
    expect(view().description).toBe('Code with your ChatGPT subscription.');
    expect(view().secondary).toBe('existing');
    await codexSetupOp(fake.context(), 'setup:codex-mode', view().token, 'existing', async () => {});
    expect(obj(obj(obj(fake.methods('server.updateSettings')[1]!.payload).providerInstanceMutation).instance).config).toEqual({ enabled: true, setupMode: 'existing' });
  });

  test('the Add ChatGPT account dialog carries the created account\'s setup, starts it, and reports when the plan is shared', async () => {
    const fake = new Fake([]);
    fake.ids = async (_native: Native, count: number) => ['8f9e', '0000'].slice(0, count);
    fake.handlers['server.getSettings'] = () => obj(fake.config.settings);
    fake.handlers['server.getConfig'] = () => fake.config;
    providerWizard(fake, true, 3, 'codex', false, '', false, '', 'chatgpt');
    await runProviderOp(fake, fake, 'provider-chatgpt', '', JSON.stringify({ key: 'Work', value: '' }));
    expect(wizardSetupStreams(fake, true, 3, 'chatgpt')).toEqual({ auth: ['codex_8f9e'], install: ['codex_8f9e'] });
    let wizard = providerWizard(fake, true, 3, 'codex', false, '', false, '', 'chatgpt');
    expect(wizard.chatgpt).toEqual([{ key: 'codex_8f9e', instanceId: 'codex_8f9e', title: 'ChatGPT - Work', setup: [] }]);
    expect(codexFlow(fake, 'codex_8f9e').autoStart).toBe(true);
    fake.config.providers = [provider({ instanceId: 'codex_8f9e', displayName: 'ChatGPT - Work', installed: true, status: 'ready', auth: { status: 'authenticated', subscriptionSharing: true } })];
    wizard = providerWizard(fake, true, 3, 'codex', false, '', false, '', 'chatgpt');
    expect(wizard.chatgpt[0]!.setup[0]!.secondary).toBe('');
    expect(wizard.chatgptConnected).toBe(true);
    expect(providerWizard(fake, true, 4, 'codex', false, '', false, '', 'chatgpt').chatgpt).toEqual([]);
  });

  // fix-provider-auth-state (#312 review): bug 21's cause was a dialog drawing the Account row the page behind it
  // draws, under the same ids. The Add ChatGPT account dialog draws CodexSetupRow for the account it created; the
  // Settings editor behind it keeps the instance it had selected (only "provider-add" selects the created one), and
  // every element id CodexSetupRow draws is made from its instance id, so the two never share an id.
  test('the Add ChatGPT account dialog and the Settings editor behind it name no element id alike', async () => {
    const fake = new Fake([provider({ installed: true, setup: { canInstall: true, canAuthenticate: true } })]);
    fake.ids = async (_native: Native, count: number) => ['7d1c', '0000'].slice(0, count);
    fake.handlers['server.getSettings'] = () => obj(fake.config.settings);
    fake.handlers['server.getConfig'] = () => fake.config;
    providerWizard(fake, true, 5, 'codex', false, '', false, '', 'chatgpt');
    await runProviderOp(fake, fake, 'provider-chatgpt', '', JSON.stringify({ key: 'Work', value: '' }));
    fake.config.providers = [...(fake.config.providers as Obj[]), provider({ instanceId: 'codex_7d1c', displayName: 'ChatGPT - Work', setup: { canInstall: true, canAuthenticate: true } })];
    const dialog = providerWizard(fake, true, 5, 'codex', false, '', false, '', 'chatgpt').chatgpt.flatMap(created => created.setup.map(setup => setup.instanceId));
    expect(dialog).toEqual(['codex_7d1c']);
    // Behind it: the row Settings had selected (or the first row), drawn with its own CodexSetupRow.
    for (const selection of ['', instanceId]) {
      const behind = providerPage(fake, selection, 0).editors.flatMap(editor => editor.setup.codex.map(setup => setup.instanceId));
      expect(behind).toEqual([instanceId]);
      expect(behind.filter(id => dialog.includes(id))).toEqual([]);
    }
    const contract = await Bun.file(new URL('./codex-setup.contract', import.meta.url)).text();
    const lines = contract.split('\n');
    const component = (name: string) => {
      const start = lines.indexOf(`component ${name}`), end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
      return lines.slice(start, end).join('\n');
    };
    // Every id, focus target and aria-controls the row and its parts draw comes from the instance id ("" draws none).
    const drawn = ['CodexSetupRow', 'CodexCallbackForm', 'CodexHelpToggle', 'ChatGptAccountSetup'].map(component).join('\n');
    const ids = drawn.match(/\b(?:id|target|aria-controls)=(?:`[^`]*`|"[^"]*")/g) ?? [];
    expect(ids.length).toBeGreaterThanOrEqual(5);
    expect(ids.filter(value => !/\$\{(?:setup\.)?instanceId\}/.test(value) && !/=""$/.test(value))).toEqual([]);
    // Only an added provider becomes the page's selection; the ChatGPT account create is "provider" (or "provider-close" on the welcome).
    const app = await Bun.file(new URL('./app.contract', import.meta.url)).text();
    expect(app.match(/providerSelected = providerCreated/g)).toHaveLength(1);
    expect(app).toContain('if pendingModal == "provider-add"\n            providerDialog = wizardManual or wizardDriver == "acpRegistry" ? providerDialog : ""\n            providerSelected = providerCreated');
    expect(app).toContain('(op == "provider-chatgpt" and welcome.show) ? "provider-close" : "provider")');
  });

  test('a remote environment signs in on the loopback primary: profile, handoff, import; a dropped stream is never subscribed again', async () => {
    const fake = await rendered(auth(), installed());
    const primary: Obj[] = [];
    const primaryNative: Native = { available: true, watch() {}, async later(request: unknown) { primary.push(obj(request)); return { ok: true, generation: 4, value: { id: `handoff-${primary.length}` } }; } };
    const remote = { origin: 'https://devbox.example.ts.net:16250', primary: { key: 'primary-key', generation: 4, native: primaryNative } };
    const profile = { clientId: 'oaiapp_one', email: 'person@example.com' };
    fake.handlers['provider.chatgpt.reconnect-profile'] = () => profile;
    await codexSetupOp(fake.context(remote), 'setup:codex-setup', fake.setup().token, '', async () => {});
    expect(fake.methods('provider.chatgpt.reconnect-profile')).toEqual([{ method: 'provider.chatgpt.reconnect-profile', payload: { instanceId, methodId: 'chatgpt' } }]);
    expect(fake.methods('provider.auth.start')).toEqual([]);
    expect(primary).toEqual([{ op: 'subscribe', key: `chatgpt-handoff:${instanceId}`, method: 'provider.chatgpt.handoff.subscribe', generation: 4,
      payload: { instanceId, environmentId: 'env-lane', attemptId: `${instanceId}:1`, returnUrl: '', profile } }]);
    // The primary's auth state drives the row and opens the page there (no local listener: the primary listens).
    codexHandoffEvent({ key: `chatgpt-handoff:${instanceId}`, subscriptionId: 'handoff-1', value: { phase: 'auth', state: waiting('primary-flow') } });
    await codexSetupPrepare([fake.context(remote)]);
    expect(fake.nativeOps('remoteEditorsOpen')).toEqual([{ op: 'remoteEditorsOpen', url: authorizationUrl }]);
    expect(fake.nativeOps('codexAuthStart')).toEqual([]);
    expect(fake.setup().callback).toBe(false);
    const transferred = { registration: { clientId: 'oaiapp_one' }, credentials: { clientId: 'oaiapp_one' } };
    codexHandoffEvent({ key: `chatgpt-handoff:${instanceId}`, subscriptionId: 'handoff-1', value: { phase: 'finished', profile: transferred } });
    await codexSetupPrepare([fake.context(remote)]); await codexSetupPrepare([fake.context(remote)]);
    expect(fake.methods('provider.chatgpt.import-profile')).toEqual([{ method: 'provider.chatgpt.import-profile', payload: { instanceId, profile: transferred } }]);
    expect(primary.at(-1)).toEqual({ op: 'unsubscribe', key: `chatgpt-handoff:${instanceId}`, generation: 4 });
    expect(fake.setup().description).toBe('Finishing sign-in...');
    // A second attempt whose stream fails: the error shows and nothing resubscribes.
    const again = await rendered(auth(), installed());
    again.handlers['provider.chatgpt.reconnect-profile'] = () => ({});
    const count = primary.length;
    await codexSetupOp(again.context(remote), 'setup:codex-setup', again.setup().token, '', async () => {});
    const subscription = `handoff-${primary.length}`;
    expect(primary.length).toBe(count + 1);
    expect(obj(primary.at(-1)!.payload).profile).toBe(null);
    codexHandoffEvent({ key: `chatgpt-handoff:${instanceId}`, subscriptionId: subscription, value: { _retryDue: true } });
    await codexSetupPrepare([again.context(remote)]); await codexSetupPrepare([again.context(remote)]);
    expect(again.setup().error).toBe('ChatGPT sign-in on the primary environment was interrupted. Try again.');
    expect(primary.slice(count + 1).filter(request => request.op === 'subscribe')).toEqual([]);
  });
});
