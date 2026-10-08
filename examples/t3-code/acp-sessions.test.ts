// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/settings/
// AcpSessionManagementSection.test.tsx ("AcpSessionManagementSection", the five cases, original
// names) and ProviderSettingsPanel.environment.test.tsx (:584 "routes explicit ACP browser
// authentication consent to the selected environment"), against a fake environment in place of
// the atom mocks: a press sends the op its button sends, and the view is the section's render.
// The destructive confirms are AppConfirm (app-settings.contract): Confirm sends the op, Cancel
// sends nothing, so a confirmed press is the op itself. Clone cases follow.
import { beforeEach, describe, expect, it, test } from 'bun:test';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { acpOp, acpSectionView, urlAuthView, type AcpToast } from './acp-sessions';

const instanceId = 'acpRegistry_antigravity';
const projectId = 'project-antigravity';
const session = { sessionId: 'native-session-1', cwd: '/workspace/antigravity', additionalDirectories: [], title: 'Native Antigravity session', updatedAt: '2026-08-23T00:00:00Z', importedThreadId: null };
const provider = { instanceId, driver: 'acpRegistry', enabled: true, installed: true, version: '1.0.0', status: 'ready', auth: { status: 'authenticated', canLogout: true },
  nativeSessions: { canList: true, canLoad: true, canResume: true, canDelete: true }, configurableProviders: true, checkedAt: '2026-08-23T00:00:00.000Z', models: [], slashCommands: [], skills: [] };

class Fake implements Native {
  available = true; writable = true;
  config: Obj = { providers: [provider] };
  shell = { projects: [{ id: projectId, title: 'Antigravity', workspaceRoot: session.cwd }] as Obj[] };
  calls: { method: string; payload: Obj }[] = []; ops: Obj[] = []; toasts: AcpToast[] = [];
  handlers: Record<string, (payload: Obj) => Obj | Promise<Obj>> = {
    'server.listAcpRegistrySessions': () => ({ sessions: [session], nextCursor: null, canLoad: true, canResume: true, canDelete: true }),
    'server.importAcpRegistrySession': () => ({ threadId: 'thread-imported', imported: true }),
    'server.deleteAcpRegistrySession': () => ({ deleted: true }),
    'server.listAcpRegistryProviders': () => ({ providers: [{ providerId: 'google', supported: ['openai'], required: false, current: { apiType: 'openai', baseUrl: 'https://api.example.test/v1' } }] }),
    'server.setAcpRegistryProvider': () => ({ configured: true }),
    'server.disableAcpRegistryProvider': () => ({ disabled: true }),
    'server.logoutAcpRegistry': () => ({ loggedOut: true }),
    'server.acceptAcpRegistryUrlAuth': () => ({ accepted: true }),
  };
  watch() {}
  async later(request: unknown) { this.ops.push(request as Obj); return { ok: true, generation: 0, value: { opened: true, recorded: true } }; }
  async rpc(_native: Native, method: string, payload: Obj): Promise<Obj> { this.calls.push({ method, payload }); return this.handlers[method]!(payload); }
  render() { return acpSectionView(this, instanceId, provider)[0]!; }
  press(what: string, field = '', value = '') { return acpOp(this, this, what, instanceId, field, value, toast => this.toasts.push(toast)); }
  called(method: string) { return this.calls.filter(call => call.method === method).map(call => call.payload); }
}
let fake: Fake;

describe('AcpSessionManagementSection', () => {
  beforeEach(() => { fake = new Fake(); });

  it('lists and imports native sessions through the owning environment', async () => {
    expect(fake.render().listLabel).toBe('List sessions');
    await fake.press('list');
    expect(fake.called('server.listAcpRegistrySessions')).toEqual([{ instanceId, projectId }]);
    expect(fake.render().sessions[0]!.importLabel).toBe('Import');
    await fake.press('import', session.sessionId);
    expect(fake.called('server.importAcpRegistrySession')).toEqual([{ instanceId, projectId, sessionId: session.sessionId, title: session.title, updatedAt: session.updatedAt }]);
    expect(fake.render().sessions[0]!.importLabel).toBe('Imported');
  });

  it('logs out the provider instance through the owning environment', async () => {
    expect(fake.render().logoutLabel).toBe('Log out');
    await fake.press('logout');
    expect(fake.called('server.logoutAcpRegistry')).toEqual([{ instanceId }]);
  });

  it('deletes unimported native sessions after destructive confirmation', async () => {
    await fake.press('list');
    const row = fake.render().sessions[0]!;
    expect([row.deleteLabel, row.deleteTitle]).toEqual(['Delete', 'Permanently delete native ACP session "Native Antigravity session"?']);
    await fake.press('delete', session.sessionId); // Confirm in AppConfirm
    expect(fake.called('server.deleteAcpRegistrySession')).toEqual([{ instanceId, projectId, sessionId: session.sessionId }]);
  });

  it('locks the project picker while a project-scoped request is pending', async () => {
    let resolveProviders!: (value: Obj) => void;
    fake.handlers['server.listAcpRegistryProviders'] = () => new Promise<Obj>(resolve => { resolveProviders = resolve; });
    expect(fake.render().providersLabel).toBe('List providers');
    const pending = fake.press('providers');
    expect(fake.render().pickerDisabled).toBe(true);
    resolveProviders({ providers: [] });
    await pending;
    expect(fake.render().pickerDisabled).toBe(false);
  });

  it('lists, saves, and disables configurable ACP providers', async () => {
    await fake.press('providers');
    expect(fake.called('server.listAcpRegistryProviders')).toEqual([{ instanceId, projectId }]);
    const row = fake.render().providers[0]!;
    expect([row.providerId, row.apiType, row.baseUrl, row.saveLabel, row.showDisable]).toEqual(['google', 'openai', 'https://api.example.test/v1', 'Save', true]);
    await fake.press('save', 'google');
    expect(fake.called('server.setAcpRegistryProvider')).toEqual([{ instanceId, projectId, providerId: 'google', apiType: 'openai', baseUrl: 'https://api.example.test/v1' }]);
    await fake.press('disable', 'google'); // Confirm in AppConfirm
    expect(fake.called('server.disableAcpRegistryProvider')).toEqual([{ instanceId, projectId, providerId: 'google' }]);
  });
});

describe('EnvironmentProviderSettings routing', () => {
  it('routes explicit ACP browser authentication consent to the selected environment', async () => {
    const action = { elicitationId: 'google-login-1', url: 'https://accounts.google.com/login', message: 'Continue with Google' };
    const fake = new Fake();
    const live = { ...provider, auth: { status: 'unauthenticated', action } };
    fake.config = { providers: [live] };
    expect(urlAuthView(fake, live)).toEqual([{ key: 'google-login-1', elicitationId: 'google-login-1', url: action.url, message: 'Continue with Google' }]);
    await acpOp(fake, fake, 'url-auth', instanceId, action.elicitationId, action.url, toast => fake.toasts.push(toast));
    expect(fake.called('server.acceptAcpRegistryUrlAuth')).toEqual([{ instanceId, elicitationId: action.elicitationId }]);
    expect(fake.ops).toEqual([{ op: 'remoteEditorsOpen', url: action.url }]);
    expect(fake.toasts).toEqual([]);
  });
});

// --- Clone cases: toasts, the expired request, read-only, drafts, paging -------------------------

test('the reference toasts: imported, deleted, configured, disabled, logged out; failures name the operation', async () => {
  fake = new Fake();
  await fake.press('list'); await fake.press('import', session.sessionId);
  await fake.press('logout');
  await fake.press('providers'); await fake.press('save', 'google'); await fake.press('disable', 'google');
  fake.handlers['server.listAcpRegistrySessions'] = () => { throw new Error('agent not reachable'); };
  await fake.press('list');
  expect(fake.toasts).toEqual([{ kind: 'success', title: 'ACP session imported' }, { kind: 'success', title: 'Logged out of ACP agent' },
    { kind: 'success', title: 'ACP provider configured' }, { kind: 'success', title: 'ACP provider disabled' },
    { kind: 'error', title: 'Could not list ACP sessions', description: 'agent not reachable' }]);
});

test('an expired URL authentication request warns; a failure toasts; read-only hides the link and sends nothing', async () => {
  const action = { elicitationId: 'google-login-2', url: 'https://accounts.google.com/login', message: 'Continue with Google' };
  const fake = new Fake();
  fake.config = { providers: [{ ...provider, auth: { status: 'unauthenticated', action } }] };
  fake.handlers['server.acceptAcpRegistryUrlAuth'] = () => ({ accepted: false });
  await fake.press('url-auth', action.elicitationId, action.url);
  expect(fake.toasts).toEqual([{ kind: 'warning', title: 'Authentication request expired', description: 'Refresh the provider and start the authentication flow again.' }]);
  fake.handlers['server.acceptAcpRegistryUrlAuth'] = () => { throw new Error('No pending request.'); };
  await fake.press('url-auth', action.elicitationId, action.url);
  expect(fake.toasts[1]).toEqual({ kind: 'error', title: 'Could not continue authentication', description: 'No pending request.' });
  fake.writable = false;
  expect(urlAuthView(fake, obj(fake.config.providers))).toEqual([]);
  const before = fake.calls.length;
  await fake.press('url-auth', action.elicitationId, action.url);
  expect(fake.calls.length).toBe(before);
});

test('drafts, headers validation, Load more, and the read-only section', async () => {
  fake = new Fake();
  await fake.press('providers');
  await fake.press('draft-url', 'google', 'https://proxy.example.test');
  await fake.press('draft-headers', 'google', '[1]');
  await fake.press('save', 'google');
  expect(fake.toasts).toEqual([{ kind: 'error', title: 'Invalid provider headers', description: 'Headers must be a JSON object with string values.' }]);
  await fake.press('draft-headers', 'google', '{"Authorization":"Bearer test-only"}');
  await fake.press('save', 'google');
  expect(fake.called('server.setAcpRegistryProvider')).toEqual([{ instanceId, projectId, providerId: 'google', apiType: 'openai', baseUrl: 'https://proxy.example.test', headers: { Authorization: 'Bearer test-only' } }]);
  expect(fake.render().providers[0]!.headers).toBe(''); // the list after saving starts from the server's values; headers stay write-only
  fake.handlers['server.listAcpRegistrySessions'] = payload => payload.cursor ? { sessions: [{ ...session, sessionId: 'native-session-2', title: null }], nextCursor: null } : { sessions: [session], nextCursor: 'c2' };
  await fake.press('list');
  expect([fake.render().showMore, fake.render().moreLabel]).toEqual([true, 'Load more']);
  await fake.press('more');
  expect(fake.called('server.listAcpRegistrySessions').at(-1)).toEqual({ instanceId, projectId, cursor: 'c2' });
  expect(fake.render().sessions.map(row => [row.title, row.key])).toEqual([['Native Antigravity session', 'native-session-1'], ['native-session-2', 'native-session-2']]);
  fake.writable = false;
  const view = fake.render();
  expect([view.listDisabled, view.logoutDisabled, view.pickerDisabled, view.sessions[0]!.importDisabled, view.providers[0]!.saveDisabled]).toEqual([true, true, true, true, true]);
});

test('the section shows only for an ACP agent with something to manage; no project says so', () => {
  const fake = new Fake();
  expect(acpSectionView(fake, 'codex', { ...provider, driver: 'codex' })).toEqual([]);
  expect(acpSectionView(fake, instanceId, { ...provider, nativeSessions: {}, configurableProviders: false, auth: { status: 'authenticated', canLogout: false } })).toEqual([]);
  fake.shell.projects = [];
  const view = acpSectionView(fake, 'acp_other', provider)[0]!;
  expect([view.status, view.showPicker, view.listDisabled]).toEqual(['Add a project before importing sessions.', false, true]);
});

const obj = (value: unknown): Obj | undefined => (Array.isArray(value) ? value[0] : undefined) as Obj | undefined;
