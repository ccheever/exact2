// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): ProviderSetupSection.test.tsx
// "Antigravity setup" (original names), ProviderSettingsPanel.environment.test.tsx (:313,
// :350, :552) and ProviderInstanceCard.test.ts (:91), against a fake environment and native
// module in place of the atom mocks. A test renders by asking providerPage for the editor
// and presses by sending the op its button sends; the Contract confirm dialog's Cancel sends
// nothing, its Confirm sends the op. Clone-specific cases follow the ported ones.
import { describe, expect, test } from 'bun:test';
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { providerPage, providerSetupStreams, providerWizard, runProviderOp, type ProviderHost } from './providers';
import { providerSetupEvent, providerSetupOp, setupOf, watchProviderSetup } from './provider-setup';
import { redactedPlaceholder } from './redacted-text';

const instanceId = 'antigravity_work';
const provider = (extra: Obj = {}): Obj => ({ instanceId, driver: 'antigravity', installed: true, enabled: true, version: 'test-version', status: 'error',
  auth: { status: 'unauthenticated' }, checkedAt: '2026-09-02T00:00:00.000Z', models: [], skills: [], slashCommands: [], setup: { canAuthenticate: true, canInstall: true }, ...extra });
const authState = (patch: Obj = {}): Obj => ({ instanceId, phase: 'waiting', flowId: 'flow-1', authorizationUrl: 'https://accounts.google.com/o/oauth2/v2/auth?state=test-only',
  expiresAt: '2026-09-02T00:05:00.000Z', message: null, ...patch });
const idle = (): Obj => ({ driver: 'antigravity', operationId: null, phase: 'idle', downloadedBytes: 0, totalBytes: null, version: null, installedVersion: null, canRemove: false, message: null });

/** The selected environment and its native module: records every server call and native op, in order. */
class Fake implements ProviderHost, Native {
  ready = true; writable = true; generation = 1; available = true; local = { favoriteModels: [] as string[] };
  config: Obj; log: string[] = []; calls: { method: string; payload: Obj }[] = []; ops: Obj[] = []; serial = 0;
  subs: Record<string, string> = {};
  handlers: Record<string, (payload: Obj) => Obj | Promise<Obj>> = {};
  opened = true;
  constructor(live: Obj[] = [provider()], instances: Obj = { [instanceId]: { driver: 'antigravity', enabled: true } }) {
    this.config = { environment: { label: 'Remote Google device' }, settings: { providerInstances: instances, providers: {} }, providers: live };
  }
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input), op = str(request.op);
    this.ops.push(request); this.log.push(`native:${op}`);
    const ok = (value: Obj) => ({ ok: true, generation: this.generation, value });
    if (op === 'subscribe') { const id = `sub-${++this.serial}`; this.subs[str(request.key)] = id; return ok({ id }); }
    if (op === 'unsubscribe') { delete this.subs[str(request.key)]; return ok({}); }
    if (op === 'remoteEditorsOpen') return ok({ opened: this.opened, recorded: true });
    if (op === 'copyText') return { ok: true, generation: this.generation, value: { copied: true } };
    return ok({});
  }
  async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> {
    this.calls.push({ method, payload }); this.log.push(`rpc:${method}`);
    const handler = this.handlers[method];
    return handler ? handler(payload) : {};
  }
  /** A stream value for `kind` of instance `id` (the transport's inbox entry, drained by client.ts). */
  emit(kind: 'auth' | 'install', value: Obj, id = instanceId) {
    const key = `provider-${kind}:${id}`;
    providerSetupEvent(this, { generation: this.generation, key, subscriptionId: this.subs[key], value });
  }
  editor(selection = instanceId) { return providerPage(this, selection, 0).editors[0]!; }
  account(selection = instanceId) { return this.editor(selection).setup.account[0]!; }
  runtime(selection = instanceId) { return this.editor(selection).setup.runtime[0]!; }
  async show(selection = instanceId) { await watchProviderSetup(this, this, 'page', providerSetupStreams(this, selection)); }
  press(op: string, token = '', value = '', id = instanceId) { return providerSetupOp(this, this, op, id, token, value); }
}
async function rendered(state: Obj | null = authState(), install: Obj | null = idle(), fake = new Fake()) {
  await fake.show();
  if (state) fake.emit('auth', state);
  if (install) fake.emit('install', install);
  return fake;
}

describe('Antigravity setup', () => {
  test('waits for verified auth after submitting a callback to the selected environment', async () => {
    const fake = await rendered();
    const callbackUrl = 'http://127.0.0.1:5555/?state=test-only&code=test-only';
    const before = fake.account();
    expect(before.callback).toBe(true);
    await fake.press('setup:auth-callback', before.draftId, callbackUrl);
    expect(fake.calls.filter(call => call.method === 'provider.auth.complete')).toEqual([{ method: 'provider.auth.complete', payload: { instanceId, flowId: 'flow-1', callbackUrl } }]);
    expect(fake.account().description).not.toBe('Signed in.');
    // The callback draft is cleared: the form is keyed anew.
    expect(fake.account().formKey).not.toBe(before.formKey);
    fake.emit('auth', authState({ phase: 'verifying', authorizationUrl: null }));
    expect(fake.account().description).not.toBe('Signed in.');
    fake.emit('auth', authState({ phase: 'succeeded', authorizationUrl: null }));
    fake.config.providers = [provider({ status: 'ready', auth: { status: 'authenticated' } })];
    expect(fake.account().description).toBe('Signed in.');
  });

  test('offers sign-in again when credentials expire after a completed auth flow', async () => {
    const fake = await rendered(authState({ phase: 'succeeded', authorizationUrl: null, message: 'Google sign-in complete.' }));
    fake.config.providers = [provider({ status: 'ready', auth: { status: 'authenticated' } })];
    expect(fake.account().startLabel).toBe('Change account');
    fake.config.providers = [provider()];
    const expired = fake.account();
    expect(expired.showStart && expired.startLabel).toBe('Sign in');
    expect(expired.description).not.toBe('Signed in.');
    expect(expired.status).toBe('');
  });

  test('does not send a callback left over from a replaced sign-in flow', async () => {
    const interaction = { type: 'browser', id: 'same-interaction', url: 'https://example.com/login', requiresConsent: false, acceptsCallback: true };
    const fake = await rendered(authState({ interaction }));
    const old = fake.account();
    fake.emit('auth', authState({ flowId: 'flow-2', interaction }));
    await fake.press('setup:auth-callback', old.draftId, 'http://127.0.0.1:5555/?state=old-flow&code=test-only');
    expect(fake.calls.some(call => call.method === 'provider.auth.complete')).toBe(false);
    // The replaced flow's form is a new form: the old draft is not carried over.
    expect(fake.account().formKey).not.toBe(old.formKey);
    expect(fake.ops.filter(op => op.op === 'subscribe').map(op => op.payload)).toEqual([{ instanceId }, { instanceId }]);
  });

  test('coalesces repeated sign-in clicks while start is pending', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }));
    let finish: (value: Obj) => void = () => { throw new Error('Missing start resolver.'); };
    fake.handlers['provider.auth.start'] = () => new Promise<Obj>(resolve => { finish = resolve; });
    const token = fake.account().token;
    const first = fake.press('setup:auth-start', token), second = fake.press('setup:auth-start', token);
    await second;
    expect(fake.calls.filter(call => call.method === 'provider.auth.start')).toEqual([{ method: 'provider.auth.start', payload: { instanceId } }]);
    finish(authState({ phase: 'starting' }));
    await first;
    // A third press drawn from the same (now stale) state sends nothing either: the queue runs it after the first.
    await fake.press('setup:auth-start', token);
    expect(fake.calls.filter(call => call.method === 'provider.auth.start')).toHaveLength(1);
  });

  test('shows a repeated runtime status message only once', async () => {
    const fake = await rendered(authState(), { ...idle(), operationId: 'install-1', phase: 'verifying', message: 'Checking the downloaded runtime.' });
    const runtime = fake.runtime();
    expect(runtime.statusMessage).toBe('Checking the downloaded runtime.');
    expect(runtime.message).toBe('');
  });

  test('removes an owned damaged runtime only after confirmation', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), { ...idle(), phase: 'failed', canRemove: true, installedVersion: null });
    const runtime = fake.runtime();
    expect(runtime.showRemove && !runtime.removeDisabled).toBe(true);
    expect(runtime.removeTitle).toBe('Remove the downloaded Antigravity runtime from Remote Google device?');
    expect(runtime.removeBody).toBe('Google sign-in and thread history are kept.');
    // Pressing Remove opens the confirmation; Cancel sends nothing.
    expect(fake.calls.some(call => call.method === 'provider.install.remove')).toBe(false);
    await fake.press('setup:install-remove');
    expect(fake.calls.filter(call => call.method === 'provider.install.remove')).toEqual([{ method: 'provider.install.remove', payload: { instanceId } }]);
  });

  for (const enabled of [true, false]) {
    test(`can sign out a verified account when its instance is enabled=${enabled}`, async () => {
      const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), idle(),
        new Fake([provider({ enabled, installed: false, status: enabled ? 'warning' : 'disabled', auth: { status: 'authenticated' } })]));
      const account = fake.account();
      expect(account.showSignOut && !account.signOutDisabled).toBe(true);
      await fake.press('setup:auth-logout');
      expect(fake.calls.filter(call => call.method === 'provider.auth.logout')).toEqual([{ method: 'provider.auth.logout', payload: { instanceId } }]);
    });
  }

  for (const status of ['unauthenticated', 'unknown']) {
    test(`does not offer sign-out for an unverified ${status} account`, async () => {
      const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), idle(), new Fake([provider({ auth: { status, canLogout: true } })]));
      const account = fake.account();
      expect(account.showStart && account.startLabel).toBe('Sign in');
      expect(account.showSignOut).toBe(false);
    });
  }

  test('offers account actions after verified login when discovery cannot identify auth', async () => {
    const fake = await rendered(authState({ phase: 'succeeded', flowId: null, authorizationUrl: null }), idle(), new Fake([provider({ auth: { status: 'unknown' } })]));
    const account = fake.account();
    expect(account.startLabel).toBe('Change account');
    expect(account.showSignOut).toBe(true);
  });

  test('does not let a shared managed install hide an invalid custom binary path', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), { ...idle(), installedVersion: 'test-version', canRemove: true },
      new Fake([provider({ installed: false })], { [instanceId]: { driver: 'antigravity', enabled: true, config: { binaryPath: '/missing/antigravity' } } }));
    expect(fake.account().startDisabled).toBe(true);
    expect(fake.runtime()).toMatchObject({ statusMessage: 'The configured Antigravity runtime is unavailable.', customNote: true });
    expect(fake.calls.some(call => call.method === 'provider.auth.start')).toBe(false);
  });

  for (const mode of ['read-only', 'older-server']) {
    test(`does not open private setup subscriptions for a ${mode} view`, async () => {
      const { setup: _setup, ...older } = provider();
      const fake = new Fake(mode === 'read-only' ? [provider()] : [older]);
      fake.writable = mode !== 'read-only';
      await fake.show();
      expect(fake.ops.some(op => op.op === 'subscribe')).toBe(false);
      expect(fake.editor().setup).toMatchObject({ kind: 'antigravity', mode: mode === 'read-only' ? 'unavailable' : 'update', account: [], runtime: [] });
    });
  }
});

describe('ProviderSettingsPanel environment', () => {
  test('opens the requested provider instance instead of the first provider', () => {
    const fake = new Fake([provider(), { ...provider(), instanceId: 'codex_custom', driver: 'codex' }],
      { codex_custom: { driver: 'codex', enabled: true }, [instanceId]: { driver: 'antigravity', enabled: true } });
    expect(providerPage(fake, 'target:codex_custom', 0).editors[0]?.id).toBe('codex_custom');
  });
  test('does not substitute another account when the requested instance was removed', () => {
    const fake = new Fake();
    const page = providerPage(fake, 'target:codex_custom', 0);
    expect(page.editors).toEqual([]);
    expect(page.emptyEditor).toBe('This provider instance is no longer available on this device.');
    // A row the user chose falls back to the first, as the reference's plain selection does.
    expect(providerPage(fake, 'codex_custom', 0).editors[0]?.id).toBeDefined();
  });
  test('keeps the signed-in ACP account visible when login methods are no longer advertised', () => {
    const id = 'acpRegistry_devin';
    const fake = new Fake([{ ...provider(), instanceId: id, driver: 'acpRegistry', auth: { status: 'authenticated', canLogout: false }, setup: { canAuthenticate: false, canInstall: false } }],
      { [id]: { driver: 'acpRegistry', enabled: true, config: { agentId: 'devin' } } });
    const editor = fake.editor(`target:${id}`);
    expect(editor.setup.kind).toBe('account');
    expect(editor.setup.account[0]).toMatchObject({ description: 'Signed in.', showSignOut: false });
  });
});

describe('ProviderInstanceCard', () => {
  test('shows a redacted provider email in the editor header status line', () => {
    const fake = new Fake([{ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, version: '1.0.0', status: 'ready',
      auth: { status: 'authenticated', email: 'developer@example.com' }, checkedAt: '2026-08-27T12:00:00.000Z', models: [], slashCommands: [], skills: [] }], { codex: { driver: 'codex', enabled: true } });
    const editor = fake.editor('codex');
    expect(editor.statusLead).toBe('Authenticated as');
    expect(editor).toMatchObject({ statusEmail: 'developer@example.com', statusEmailPlaceholder: redactedPlaceholder('developer@example.com') });
    expect(editor.statusEmailPlaceholder).not.toContain('developer');
  });
});

describe('provider setup in the clone', () => {
  test('consent is recorded before the native open, and the URL is the browser interaction\'s', async () => {
    const interaction = { type: 'browser', id: 'consent', url: 'https://cursor.com/login?state=x', requiresConsent: true };
    const fake = await rendered(authState({ interaction, authorizationUrl: null }));
    fake.log = [];
    const account = fake.account();
    expect(account).toMatchObject({ url: 'https://cursor.com/login?state=x', callback: false, description: 'Finish signing in in your browser.' });
    await fake.press('setup:auth-open', account.draftId);
    expect(fake.log).toEqual(['rpc:provider.auth.respond', 'native:remoteEditorsOpen']);
    expect(fake.calls[0]).toEqual({ method: 'provider.auth.respond', payload: { instanceId, flowId: 'flow-1', interactionId: 'consent', response: { type: 'browser', action: 'accept' } } });
    expect(fake.ops.find(op => op.op === 'remoteEditorsOpen')?.url).toBe('https://cursor.com/login?state=x');
    // A refused consent opens nothing; a failed open asks for the copied link.
    fake.handlers['provider.auth.respond'] = () => { throw new Error('Consent expired.'); };
    fake.log = [];
    await fake.press('setup:auth-open', account.draftId);
    expect(fake.log).toEqual(['rpc:provider.auth.respond']);
    expect(fake.account().error).toBe('Consent expired.');
    delete fake.handlers['provider.auth.respond']; fake.opened = false;
    await fake.press('setup:auth-open', account.draftId);
    expect(fake.account().error).toBe('Could not open the sign-in page. Copy the link and open it in your browser.');
  });

  test('copy link copies first, then records consent; no consent without requiresConsent', async () => {
    const interaction = { type: 'browser', id: 'i', url: 'https://cursor.com/login', requiresConsent: true };
    const fake = await rendered(authState({ interaction }));
    fake.log = [];
    await fake.press('setup:auth-copy', fake.account().draftId);
    expect(fake.log).toEqual(['native:copyText', 'rpc:provider.auth.respond']);
    fake.emit('auth', authState({ interaction: { ...interaction, id: 'j', requiresConsent: false } }));
    fake.log = [];
    await fake.press('setup:auth-copy', fake.account().draftId);
    expect(fake.log).toEqual(['native:copyText']);
  });

  test('paste redirect sends the trimmed URL once', async () => {
    const fake = await rendered(authState({ interaction: { type: 'browser', id: 'b', url: 'https://accounts.google.com/x', requiresConsent: false, acceptsCallback: true } }));
    const draft = fake.account().draftId;
    await fake.press('setup:auth-callback', draft, '  http://127.0.0.1:5555/?code=1  ');
    await fake.press('setup:auth-callback', draft, '  http://127.0.0.1:5555/?code=1  ');
    await fake.press('setup:auth-callback', draft, '   ');
    expect(fake.calls).toEqual([{ method: 'provider.auth.complete', payload: { instanceId, flowId: 'flow-1', callbackUrl: 'http://127.0.0.1:5555/?code=1' } }]);
  });

  test('phases: descriptions, buttons and failed and cancelled flows follow the reference', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }));
    expect(fake.account()).toMatchObject({ description: 'Sign in on Remote Google device.', showStart: true, startLabel: 'Sign in', showCancel: false, url: '' });
    fake.emit('auth', authState({ phase: 'starting', authorizationUrl: null }));
    expect(fake.account()).toMatchObject({ description: 'Starting sign-in…', showCancel: true, showStart: false });
    fake.emit('auth', authState());
    expect(fake.account()).toMatchObject({ description: 'Finish signing in in your browser.', showCancel: true, callback: true, url: 'https://accounts.google.com/o/oauth2/v2/auth?state=test-only' });
    fake.emit('auth', authState({ phase: 'verifying', authorizationUrl: null }));
    expect(fake.account()).toMatchObject({ description: 'Checking your account…', showCancel: true });
    fake.emit('auth', authState({ phase: 'failed', authorizationUrl: null, message: 'SUBSCRIPTION_REQUIRED: This Google account cannot use Antigravity.' }));
    expect(fake.account()).toMatchObject({ status: 'SUBSCRIPTION_REQUIRED: This Google account cannot use Antigravity.', startLabel: 'Retry sign-in', showCancel: false });
    fake.emit('auth', authState({ phase: 'cancelled', authorizationUrl: null, message: 'Google sign-in was cancelled.' }));
    expect(fake.account()).toMatchObject({ status: '', startLabel: 'Retry sign-in' });
    fake.emit('auth', authState({ phase: 'waiting' }));
    await fake.press('setup:auth-cancel', fake.account().token);
    expect(fake.calls.at(-1)).toEqual({ method: 'provider.auth.cancel', payload: { instanceId, flowId: 'flow-1' } });
  });

  test('sign out asks with the reference copy; Change account starts a new flow', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), idle(),
      new Fake([provider({ displayName: 'Google work', status: 'ready', auth: { status: 'authenticated', email: 'dev@example.com' } })]));
    const account = fake.account();
    expect(account).toMatchObject({ email: 'dev@example.com', emailPlaceholder: redactedPlaceholder('dev@example.com'), startLabel: 'Change account', showSignOut: true,
      signOutTitle: 'Sign out of Google work on Remote Google device?', signOutBody: 'This stops running threads that share this sign-in. Thread history is kept.' });
    await fake.press('setup:auth-start', account.token);
    expect(fake.calls).toEqual([{ method: 'provider.auth.start', payload: { instanceId } }]);
  });

  test('credentials: password-type secret fields, values sent once, drafts cleared on success', async () => {
    const id = 'acpRegistry_kilo', fields = [{ name: 'KILO_API_KEY', label: 'API key', secret: true }, { name: 'KILO_ORG', label: 'Organization', secret: false }];
    const live = { ...provider(), instanceId: id, driver: 'acpRegistry', auth: { status: 'unauthenticated' }, setup: { canAuthenticate: true, canInstall: false } };
    const fake = new Fake([live], { [id]: { driver: 'acpRegistry', enabled: true, config: { agentId: 'kilo' } } });
    await fake.show(id);
    expect(fake.account(id).description).toBe('Discovering sign-in methods…');
    fake.emit('auth', { instanceId: id, phase: 'idle', flowId: null, authorizationUrl: null, expiresAt: null, message: null, methods: [{ id: 'key', name: 'API key', description: null, type: 'credentials' }] }, id);
    expect(fake.account(id)).toMatchObject({ description: 'Sign in on Remote Google device.', pickMethod: false, showStart: true });
    fake.emit('auth', { instanceId: id, phase: 'waiting', flowId: 'f', authorizationUrl: null, expiresAt: null, message: null, interaction: { type: 'credentials', id: 'c', fields } }, id);
    const account = fake.account(id);
    expect(account.description).toBe('Enter your credentials below.');
    expect(account.credentials).toEqual([{ index: 0, name: 'KILO_API_KEY', label: 'API key', secret: true }, { index: 1, name: 'KILO_ORG', label: 'Organization', secret: false }]);
    const values = ['sk-test&1=2', 'acme', '', ''].map(encodeURIComponent).join('&');
    await fake.press('setup:auth-credentials', account.draftId, values, id);
    await fake.press('setup:auth-credentials', account.draftId, values, id);
    expect(fake.calls).toEqual([{ method: 'provider.auth.respond', payload: { instanceId: id, flowId: 'f', interactionId: 'c', response: { type: 'credentials', values: { KILO_API_KEY: 'sk-test&1=2', KILO_ORG: 'acme' } } } }]);
    expect(fake.account(id).formKey).not.toBe(account.formKey);
  });

  test('ACP methods: a picker for more than one, the method sent; none advertised offers the docs', async () => {
    const id = 'acpRegistry_kilo';
    const live = { ...provider(), instanceId: id, driver: 'acpRegistry', auth: { status: 'unauthenticated' }, setup: { canAuthenticate: true, canInstall: false, documentationUrl: 'https://kilo.ai/docs' } };
    const fake = new Fake([live], { [id]: { driver: 'acpRegistry', enabled: true } });
    await fake.show(id);
    fake.emit('auth', { instanceId: id, phase: 'idle', flowId: null, authorizationUrl: null, expiresAt: null, message: null,
      methods: [{ id: 'browser', name: 'Browser', description: null, type: 'agent' }, { id: 'key', name: 'API key', description: null, type: 'credentials' }] }, id);
    expect(fake.account(id)).toMatchObject({ pickMethod: true, methods: [{ id: 'browser', name: 'Browser' }, { id: 'key', name: 'API key' }] });
    await fake.press('setup:auth-start', fake.account(id).token, 'key', id);
    expect(fake.calls).toEqual([{ method: 'provider.auth.start', payload: { instanceId: id, methodId: 'key' } }]);
    fake.emit('auth', { instanceId: id, phase: 'idle', flowId: null, authorizationUrl: null, expiresAt: null, message: null, methods: [] }, id);
    expect(fake.account(id)).toMatchObject({ description: "No in-app sign-in advertised. Follow the provider's docs to finish setup.", docsUrl: 'https://kilo.ai/docs', showStart: false });
  });

  test('device code text and the terminal branch', async () => {
    const fake = await rendered(authState({ interaction: { type: 'deviceCode', id: 'd', url: 'https://example.com/device', userCode: 'ABCD-1234' } }));
    expect(fake.account()).toMatchObject({ deviceCode: 'ABCD-1234', url: 'https://example.com/device', details: true, callback: false });
    fake.emit('auth', authState({ interaction: { type: 'terminal', id: 't', output: 'login> ', outputOffset: 7 } }));
    expect(fake.account()).toMatchObject({ terminal: true, flow: 'flow-1:t', output: 'login> ', offset: 7, description: 'Complete sign-in in the terminal below.' });
  });

  test('runtime row: byte progress, labels, cancel and stream errors with Retry setup status', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }), idle(), new Fake([provider({ installed: false })]));
    expect(fake.runtime()).toMatchObject({ statusMessage: 'Not installed.', showInstall: true, installLabel: 'Install Antigravity', installDisabled: false, progressMax: 0 });
    await fake.press('setup:install-start', fake.runtime().token);
    expect(fake.calls).toEqual([{ method: 'provider.install.start', payload: { instanceId } }]);
    fake.emit('install', { ...idle(), operationId: 'op-1', phase: 'downloading', downloadedBytes: 12_345_678, totalBytes: 45_600_000 });
    expect(fake.runtime()).toMatchObject({ statusMessage: 'Downloading 12.3 MB of 45.6 MB.', progressValue: 12_345_678, progressMax: 45_600_000, progressPercent: 27.1, showCancel: true, showInstall: false, showRemove: false });
    await fake.press('setup:install-cancel', fake.runtime().token);
    expect(fake.calls.at(-1)).toEqual({ method: 'provider.install.cancel', payload: { instanceId, operationId: 'op-1' } });
    fake.emit('install', { ...idle(), operationId: 'op-1', phase: 'extracting' });
    expect(fake.runtime().statusMessage).toBe('Extracting Antigravity.');
    fake.emit('install', { ...idle(), operationId: 'op-1', phase: 'cancelled', message: 'Installation was cancelled.' });
    expect(fake.runtime()).toMatchObject({ installLabel: 'Retry installation', message: 'Installation was cancelled.' });
    fake.emit('install', { ...idle(), phase: 'succeeded', version: '1.1.1', installedVersion: '1.1.1', canRemove: true });
    fake.config.providers = [provider()];
    expect(fake.runtime()).toMatchObject({ statusMessage: 'Installed.', installLabel: 'Reinstall Antigravity', showRemove: true });
    fake.emit('install', { ...idle(), phase: 'idle', version: '1.2.0', installedVersion: '1.1.1', canRemove: true });
    expect(fake.runtime().installLabel).toBe('Update Antigravity');
    // A stream failure is the query error: actions wait and "Retry setup status" subscribes again.
    fake.emit('install', { _transportError: { message: 'Install status is unavailable.' } });
    expect(fake.runtime()).toMatchObject({ error: 'Install status is unavailable.', showRetry: true, installDisabled: true });
    expect(fake.account().disabled).toBe(false);
    const before = fake.ops.filter(op => op.op === 'subscribe').length;
    await fake.press('setup:retry');
    await fake.show();
    expect(fake.ops.filter(op => op.op === 'subscribe').length).toBe(before + 1);
    expect(fake.runtime().showRetry).toBe(false);
  });

  test('streams: one per row, closed when the page closes, resubscribed after _retryDue', async () => {
    const fake = await rendered();
    expect(fake.ops.filter(op => op.op === 'subscribe').map(op => [op.key, op.method])).toEqual([
      ['provider-auth:antigravity_work', 'provider.auth.subscribe'], ['provider-install:antigravity_work', 'provider.install.subscribe']]);
    expect(Object.keys(fake.subs)).toHaveLength(2);
    fake.emit('auth', { _retryDue: true });
    await fake.show();
    expect(fake.ops.filter(op => op.op === 'subscribe')).toHaveLength(3);
    await watchProviderSetup(fake, fake, 'page', { auth: [], install: [] });
    expect(fake.ops.filter(op => op.op === 'unsubscribe').map(op => op.key)).toEqual(['provider-auth:antigravity_work', 'provider-install:antigravity_work']);
    expect(setupOf(fake, instanceId).auth.state).toBeNull();
  });

  test('a read-only session sends no setup command', async () => {
    const fake = await rendered(authState({ phase: 'idle', flowId: null, authorizationUrl: null }));
    fake.writable = false;
    await fake.press('setup:auth-start', 'idle:');
    await fake.press('setup:install-remove');
    expect(fake.calls).toEqual([]);
  });

  test('terminal events are scoped to their flow', async () => {
    const fake = await rendered(authState({ interaction: { type: 'terminal', id: 't', output: '' } }));
    await fake.press('setup:auth-event', '', JSON.stringify({ authFlow: 'old:t', type: 'auth-error' }));
    expect(fake.account().error).toBe('');
    await fake.press('setup:auth-event', '', JSON.stringify({ authFlow: 'flow-1:t', type: 'auth-error' }));
    expect(fake.account().error).toBe('The provider sign-in terminal is no longer available.');
    await fake.press('setup:auth-event', '', JSON.stringify({ authFlow: 'flow-1:t', type: 'auth-recovered' }));
    expect(fake.account().error).toBe('');
  });

  test('mounting: a provider that can sign in gets the Account row; Cursor with an API key keeps its note; Enable Antigravity', async () => {
    const cursor = { instanceId: 'cursor', driver: 'cursor', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, setup: { canAuthenticate: false, canInstall: false } };
    const fake = new Fake([cursor, provider()], { cursor: { driver: 'cursor', enabled: true }, [instanceId]: { driver: 'antigravity', enabled: false } });
    expect(fake.editor('cursor').setup).toMatchObject({ kind: 'cursor', cursorNote: "Using CURSOR_API_KEY. Remove it from this provider's environment to use browser sign-in.", account: [] });
    fake.config.providers = [{ ...cursor, setup: { canAuthenticate: true, canInstall: false } }, provider()];
    expect(fake.editor('cursor').setup.kind).toBe('account');
    expect(fake.editor().setup).toMatchObject({ kind: 'antigravity', showEnable: true, environmentLabel: 'Remote Google device' });
    expect(fake.editor().showSetup).toBe(true);
  });

  test('the wizard follows its created ACP instance into the Sign in step', async () => {
    const id = 'acpRegistry_kilo';
    const fake = new Fake([], {});
    fake.handlers['server.getSettings'] = () => obj(fake.config.settings);
    fake.handlers['server.updateSettings'] = payload => { const mutation = obj(payload.providerInstanceMutation); obj(fake.config.settings).providerInstances = { [str(mutation.instanceId)]: mutation.instance }; return {}; };
    fake.handlers['server.getConfig'] = () => ({ ...fake.config, providers: [{ ...provider(), instanceId: id, driver: 'acpRegistry', setup: { canAuthenticate: true, canInstall: false } }] });
    providerWizard(fake, true, 7, 'acpRegistry', true, 'Kilo', true, id);
    await runProviderOp(fake, fake, 'provider-add', id, JSON.stringify({ driver: 'acpRegistry', label: 'Kilo', fields: { agentId: 'kilo' } }));
    const wizard = providerWizard(fake, true, 7, 'acpRegistry', true, 'Kilo', true, id);
    expect(wizard.created).toBe(id);
    expect(wizard.auth[0]).toMatchObject({ instanceId: id, discovering: true, description: 'Discovering sign-in methods…', account: [] });
    // A closed or reopened wizard forgets it.
    expect(providerWizard(fake, true, 8, 'acpRegistry', false, '', false, '').auth).toEqual([]);
    expect(arr(fake.calls.map(call => ({ method: call.method } as Obj)))).toContainEqual({ method: 'server.updateSettings' });
  });
});
