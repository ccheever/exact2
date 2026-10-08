// Settings › Providers shows the environment its scope selector chose (fix-providers-environment-scope,
// found by #312): T3 Code 1e2ecbd975 routes/settings.providers.tsx:7-30. This Mac (the focused
// connection) has Codex signed in; a second, background environment has not. Before the fix the page
// kept this Mac's providers whichever environment the menu ticked, and the second server got no
// provider queries. These tests go through the app's own answer(), as the Contract asks it.
import { afterEach, describe, expect, test } from 'bun:test';
import { answer } from './app';
import { Backend, storage } from './client-fixture';
import { environmentKey, fleet } from './settings-b-fleet';
import { initialShell, obj, arr, str, type Obj } from './domain';
import type { Native } from './protocol';
import { scopeMachine } from './settings-b-scope';
import { primaryAt, resetPrimary } from './local-primary-fixture';
import { resetHighlightSlicing } from './r12-render-highlight';

const ORIGIN = 'http://192.168.1.225:16261', KEY = environmentKey(ORIGIN, 'env2');
const codex = (auth: Obj): Obj => ({ instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, status: 'ready', version: '0.151.0',
  auth, setup: { canAuthenticate: true }, checkedAt: '2026-10-08T08:37:00.000Z', models: [{ slug: 'gpt-6-luna', name: 'GPT-6 Luna', isDefault: true }] });
const instances = { codex: { driver: 'codex', enabled: true, config: {} } };

/** This Mac's server (the focused connection) and the second environment's, behind one native: a `fleet` request is the second's. */
class TwoServers implements Native {
  available = true;
  calls: Obj[] = [];
  constructor(readonly focus: Backend, readonly second: Backend) {}
  watch(topic: string) { this.focus.watch(topic); }
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (request.fleet === undefined) return this.focus.later(input);
    if (request.fleet !== KEY) return { ok: false, generation: 0, error: { kind: 'Disconnected', message: 'That environment is not connected.', uncertain: false } };
    const { fleet: _key, ...rest } = request;
    return this.second.later(rest);
  }
  /** What reached the second environment's transport. */
  remote(op: string) { return this.calls.filter(call => call.fleet === KEY && call.op === op); }
}

function servers() {
  const focus = new Backend(), second = new Backend();
  focus.generation = 41; // a generation no other file's server used: the app's client bootstraps from this one
  focus.config = { ...focus.config, providers: [codex({ status: 'authenticated', email: 'mac@example.com', label: 'ChatGPT' })],
    settings: { ...obj(focus.config.settings), providerInstances: instances } };
  second.generation = 3;
  second.config = { environment: { environmentId: 'env2', label: 'LAN box', platform: { os: 'darwin', machine: 'server' } },
    providers: [codex({ status: 'unauthenticated' })], settings: { providerInstances: instances } };
  const native = new TwoServers(focus, second), disk = storage();
  return { focus, second, native, files: disk.files };
}
/** The second environment as the fleet knows it once its transport synchronized (settings-b-fleet.ts). */
function background(second: Backend, phase: 'connected' | 'reconnecting' = 'connected') {
  fleet.entries.set(KEY, { key: KEY, origin: ORIGIN, environmentId: 'env2', phase, message: '', traceId: '', generation: 3, synchronized: phase === 'connected' ? 3 : -1,
    lastEvent: 0, subscriptions: {}, config: second.config, shell: initialShell(), scopes: ['orchestration:read', 'orchestration:operate'], error: '', requested: true });
}
/** The providerPage resource's arguments (app.contract): selection, active, revision, clock, then the scope. */
const page = async (native: Native, files: Parameters<typeof answer>[3], machine: string, extra: string[] = ['', '', '']) =>
  obj(await answer('providerPage', ['', true, 0, Date.parse('2026-10-08T08:41:00.000Z'), machine, ...extra], null, files, native));
const statuses = (view: Obj) => arr(view.rows).map(row => `${str(row.name)}: ${str(row.status)}`);

// answer() starts a highlight turn (app.ts): later files' highlighter tests expect it unsliced.
afterEach(() => { fleet.entries.delete(KEY); resetPrimary(); resetHighlightSlicing(); });

describe('Settings › Providers follows the scope selector', () => {
  test('choosing a second environment shows its providers and sends the page reads there; choosing back shows this Mac again', async () => {
    const { second, native, files } = servers();
    await answer('snapshot', [], null, files, native);
    background(second);

    const mine = await page(native, files, '');
    expect(statuses(mine)).toEqual(['Codex: Authenticated · ChatGPT']);

    const before = native.calls.length;
    const theirs = await page(native, files, 'env2');
    expect(statuses(theirs)).toEqual(['Codex: Not authenticated']);
    expect(obj(arr(theirs.editors)[0]).statusLead).toBe('Not authenticated');
    expect([mine.environment, theirs.environment]).toEqual(['env1', 'env2']); // what the page's actions name
    // The selected editor's Account row watches its sign-in state on the second server only.
    expect(native.calls.slice(before).filter(call => call.op === 'subscribe').map(call => [call.fleet === KEY, call.method, obj(call.payload).instanceId]))
      .toEqual([[true, 'provider.auth.subscribe', 'codex']]);

    const back = await page(native, files, 'env1');
    expect(statuses(back)).toEqual(['Codex: Authenticated · ChatGPT']);
    expect(native.remote('unsubscribe').map(call => call.key)).toEqual(['provider-auth:codex']);
  });

  test("the page's actions go to the environment it shows", async () => {
    const { second, native, files } = servers();
    await answer('snapshot', [], null, files, native);
    background(second);
    await page(native, files, 'env2');

    // "Refresh provider status": providerChange(op, id, field, value, providerPage.environment).
    await answer('providerChange', ['provider-refresh', '', '', '', 'env2'], null, files, native);
    expect(native.calls.filter(call => obj(call).method === 'server.refreshProviders').map(call => call.fleet ?? 'focused')).toEqual([KEY]);
    // Enable switch: an instance write reads the second server's settings and writes there.
    await answer('providerChange', ['provider-enabled', 'codex', '', 'false', 'env2'], null, files, native);
    expect(native.calls.filter(call => obj(call).method === 'server.updateSettings').map(call => call.fleet ?? 'focused')).toEqual([KEY]);
    expect(obj(obj(obj(second.config.settings).providerInstances).codex).enabled).toBe(false);
    // Sign in on the second server's Account row.
    await answer('providerChange', ['setup:auth-start', 'codex', ':', '', 'env2'], null, files, native);
    expect(native.calls.filter(call => obj(call).method === 'provider.auth.start').map(call => call.fleet ?? 'focused')).toEqual([KEY]);
    // Add provider: the wizard reads the second environment, the ACP search asks it, the new instance is written there.
    expect(obj(await answer('providerWizard', [true, 1, 'claudeAgent', true, 'Work', false, '', 0, 'add', '', 'env2'], null, files, native)).environment).toBe('LAN box');
    await answer('acpRegistry', ['', true, 0, 'env2'], null, files, native);
    expect(native.calls.filter(call => obj(call).method === 'server.searchAcpRegistry').map(call => call.fleet ?? 'focused')).toEqual([KEY]);
    await answer('providerAdd', ['claudeAgent', 'Work', '', '', '', '', '', '', '', 'env2'], null, files, native);
    expect(Object.keys(obj(obj(second.config.settings).providerInstances))).toContain('claudeAgent_work');
    expect(native.calls.filter(call => obj(call).method === 'server.updateSettings').map(call => call.fleet ?? 'focused')).toEqual([KEY, KEY]);
  });

  test("an environment's unsaved variable rows stay on that environment", async () => {
    const { second, native, files } = servers();
    await answer('snapshot', [], null, files, native);
    background(second);
    await answer('providerChange', ['provider-env-add', 'codex', '', '', 'env1'], null, files, native);
    expect(obj(arr((await page(native, files, 'env1')).editors)[0]).hasVariables).toBe(true);
    expect(obj(arr((await page(native, files, 'env2')).editors)[0]).hasVariables).toBe(false);
  });

  test('a disconnected or removed environment leaves the words to the scope boundary', async () => {
    const { second, native, files } = servers();
    await answer('snapshot', [], null, files, native);
    background(second, 'reconnecting');
    const core = (machine: string) => answer('settingsCore', [machine, '', '', '', 'providers', '', true], null, files, native).then(obj);

    const offline = await page(native, files, 'env2');
    expect([offline.available, offline.message, offline.environment]).toEqual([false, '', '-']);
    expect((await core('env2')).message).toBe('Reconnect LAN box to change its settings.');
    // A dialog the page opened before (a sign-out confirm, the wizard) now writes nowhere, never to this Mac.
    const refused = obj(await answer('providerChange', ['provider-refresh', '', '', '', offline.environment], null, files, native));
    expect(str(refused.message)).toContain('Reconnect before making changes.');
    expect(native.calls.some(call => obj(call).method === 'server.refreshProviders')).toBe(false);

    const gone = await page(native, files, 'env-removed');
    expect([gone.available, gone.message]).toEqual([false, '']);
    expect((await core('env-removed')).message).toBe('This environment is no longer available.');
  });
});

test('with no environment chosen, Providers takes the primary, else a connected one (selectSingleEnvironmentScope)', () => {
  const client = { environmentId: 'env-remote' };
  const candidates = [{ environmentId: 'env-remote', connection: { phase: 'connected' } }, { environmentId: 'env-mac', connection: { phase: 'connected' } }];
  expect(scopeMachine(client, 'providers', '', candidates)).toBe('env-remote');
  primaryAt('http://127.0.0.1:16550', 'env-mac');
  expect(scopeMachine(client, 'providers', '', candidates)).toBe('env-mac');
  expect(scopeMachine(client, 'providers', 'env-remote', candidates)).toBe('env-remote');
  expect(scopeMachine(client, 'general', '', candidates)).toBe('');
  expect(scopeMachine(client, 'providers', '', [])).toBe('');
});
