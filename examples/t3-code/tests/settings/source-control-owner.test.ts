import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { sourceControlPage } from '../../src/settings/source-control-view';
import { restCommand } from '../../src/settings/settings-rest-commands';
import { storage } from '../support/client-fixture';

const capabilities = { serverResolvedCommandContext: true, projectSettingsOverrides: true };
function setup(scopes = ['orchestration:read', 'settings:write']) {
  const client = new T3Client();
  Object.assign(client, { loaded: true, connection: 'connected', origin: 'http://127.0.0.1:16410', configLive: true, shellLive: true, threadLive: true,
    environmentId: 'local', scopes: ['orchestration:read', 'orchestration:operate'] });
  client.config = { environment: { environmentId: 'local', label: 'Local', capabilities }, settings: {}, providers: [] };
  const settings = { defaultAutoPull: true, branchNamingMode: 'static', branchNamePrefix: 'remote-prefix', backgroundActivity: { profile: 'performance' }, sourceControlHosts: { github: { hosts: { 'github.com': { enabled: true, account: 'remote-bob' } } }, gitcafe: { token: '<redacted>' } } };
  const entry = { key: 'remote', environmentId: 'remote-env', origin: 'http://127.0.0.1:16411', generation: 3, synchronized: 3, phase: 'connected', scopes,
    config: { environment: { environmentId: 'remote-env', label: 'Remote', capabilities }, settings, providers: [] },
    shell: { projects: [], threads: [], sequence: 1 }, subscriptions: {}, requested: true } as unknown as FleetEntry;
  fleet.entries.set(entry.key, entry);
  const calls: Obj[] = [], native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.fleet !== entry.key) throw new Error('Focused Local transport must not be used');
    let value: Obj;
    if (request.method === 'server.discoverSourceControl') value = { versionControlSystems: [], sourceControlProviders: [{ kind: 'github', label: 'GitHub', status: 'available', version: 'gh remote',
      auth: { status: 'authenticated', account: 'remote-bob', accounts: [{ host: 'github.com', account: 'remote-bob', authenticated: true }, { host: 'github.com', account: 'remote-alice', authenticated: true }] } }] };
    else if (request.method === 'server.getSettings') value = obj(entry.config.settings);
    else if (request.method === 'server.updateSettings') value = { ...obj(entry.config.settings), ...obj(obj(request.payload).patch) };
    else throw new Error(`Unexpected request ${request.method}`);
    return { ok: true, generation: entry.generation, value };
  } };
  const page = (machine = entry.environmentId, rescan = 0) => sourceControlPage(client, native, 'local', '', true, rescan, machine);
  const write = () => restCommand(client, native, storage().files, 'host-credentials', 'local:', 'environment=remote-env&kind=github&action=account&host=github.com&value=remote-alice');
  return { client, entry, calls, native, page, write };
}
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('source control discovery and credentials belong to the scoped environment', () => {
  test('background selection scans and writes only its own transport with settings permission', async () => {
    const { client, entry, calls, page, write } = setup();
    const view = await page();
    expect(view.available).toBe(true);
    expect(view.providers[0].version).toBe('gh remote');
    expect(view.repositories.find(row => row.key === 'defaultAutoPull')).toMatchObject({ checked: true, disabled: false });
    expect(view.text.find(row => row.key === 'branchNamePrefix')).toMatchObject({ value: 'remote-prefix', disabled: false });
    expect(view.fetches[0].seconds).toBe('15');
    expect(view.hostCredentials.find(form => form.kind === 'github')).toMatchObject({ environmentId: 'remote-env', disabled: false, groups: [{ selected: 'remote-bob' }] });
    await write();
    expect(calls.map(call => [call.fleet, call.method])).toEqual([
      ['remote', 'server.discoverSourceControl'], ['remote', 'server.getSettings'], ['remote', 'server.updateSettings']]);
    expect(obj(obj(obj(entry.config.settings).sourceControlHosts).github).hosts).toEqual({ 'github.com': { enabled: true, account: 'remote-alice' } });
    expect(client.config.settings).toEqual({});
    expect(client.environmentId).toBe('local');
  });
  test('the selected connection grants control writes independently of the focused connection', async () => {
    const { client, calls, page, write } = setup(['orchestration:read', 'orchestration:operate']);
    client.scopes.push('settings:write');
    const view = await page();
    expect(view.hostCredentials.every(form => form.disabled)).toBe(true);
    expect([...view.repositories, ...view.text].every(row => row.disabled)).toBe(true);
    await expect(write()).rejects.toThrow('settings:write');
    expect(calls.map(call => call.method)).toEqual(['server.discoverSourceControl']);
  });
  test('a connected background scope remains available while the focused connection is offline', async () => {
    const { client, calls, page } = setup();
    client.connection = 'disconnected';
    const view = await page();
    expect(view.available).toBe(true);
    expect(view.hostCredentials[0].environmentId).toBe('remote-env');
    expect(calls.every(call => call.fleet === 'remote')).toBe(true);
  });
  test('removed or disconnected selections do not fall back to the focused environment', async () => {
    const { entry, calls, page } = setup();
    expect((await page('removed')).available).toBe(false);
    entry.phase = 'disconnected';
    expect((await page()).available).toBe(false);
    expect(calls).toEqual([]);
  });
  test('selected checkout rows use that owner project identity and inherited label', async () => {
    const { client, entry, native, calls } = setup();
    entry.shell.projects = [{ id: 'remote-project', workspaceRoot: '/remote/project' }];
    entry.config.settings = { ...obj(entry.config.settings), projectSettingsOverrides: { 'remote-project': { branchNamePrefix: 'checkout-prefix' } } };
    const view = await sourceControlPage(client, native, 'local', '', true, 0, entry.environmentId, 'remote-env:/remote/project', 'remote-project');
    expect(view.available).toBe(true);
    expect(view.project).toBe(true);
    expect(view.text.find(row => row.key === 'branchNamePrefix')).toMatchObject({ value: 'checkout-prefix', state: 'overridden' });
    expect(view.repositories.find(row => row.key === 'defaultAutoPull')?.summary).toBe('Inherited from Remote');
    expect(view.hostCredentials.every(form => form.disabled)).toBe(true);
    expect(calls.every(call => call.fleet === entry.key)).toBe(true);
  });
  test('discovery stays cached per owner and invalidates on that owner generation or rescan', async () => {
    const { entry, calls, page } = setup();
    await page(); await page();
    expect(calls).toHaveLength(1);
    entry.generation++; entry.synchronized = entry.generation;
    await page();
    expect(calls).toHaveLength(2);
    await page(entry.environmentId, 1);
    expect(calls).toHaveLength(3);
  });
});
