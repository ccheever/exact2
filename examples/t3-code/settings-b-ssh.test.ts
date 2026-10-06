import { test, expect } from 'bun:test';
import { parseManualSshTarget, filterSshHosts, formatSshTarget, formatSshError, sshHostsView, runSshOp, SSH_OPS } from './settings-b-ssh';
import { connectionsProjection, CONNECTION_OPS, type ConnectionHost } from './connections';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';

test('manual SSH targets parse like parseManualDesktopSshTarget', () => {
  expect(parseManualSshTarget({ host: 'devbox', username: '', port: '' })).toEqual({ alias: 'devbox', hostname: 'devbox', username: null, port: null });
  expect(parseManualSshTarget({ host: 'me@devbox:2222', username: '', port: '' })).toEqual({ alias: 'devbox', hostname: 'devbox', username: 'me', port: 2222 });
  expect(parseManualSshTarget({ host: 'me@devbox', username: 'root', port: '22' })).toEqual({ alias: 'devbox', hostname: 'devbox', username: 'root', port: 22 });
  expect(parseManualSshTarget({ host: '[fe80::1]:2200', username: '', port: '' })).toEqual({ alias: 'fe80::1', hostname: 'fe80::1', username: null, port: 2200 });
  expect(() => parseManualSshTarget({ host: '  ', username: '', port: '' })).toThrow('SSH host or alias is required.');
  expect(() => parseManualSshTarget({ host: 'devbox', username: '', port: '70000' })).toThrow('SSH port must be between 1 and 65535.');
  expect(formatSshTarget({ alias: 'a', hostname: 'h.example', username: 'u', port: 2222 })).toBe('u@h.example:2222');
  expect(formatSshTarget({ alias: 'a', hostname: 'h.example', username: null, port: null })).toBe('h.example');
  expect(formatSshError(new Error("Error invoking remote method 'desktop:ensure-ssh-environment': SshCommandError: Permission denied"))).toBe('Permission denied');
});

test('host suggestions put alias prefixes first, then substrings', () => {
  const hosts = [{ alias: 'staging' }, { alias: 'dev-box' }, { alias: 'box' }, { alias: 'mybox' }];
  expect(filterSshHosts(hosts, ' BOX ').map(host => host.alias)).toEqual(['box', 'dev-box', 'mybox']);
  expect(filterSshHosts(hosts, '').length).toBe(4);
});

function fakeNative(handler: (request: Obj) => unknown) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, later: async (input: unknown) => { const request = obj(input); calls.push(request); return handler(request); } };
  return { native, calls };
}
const ok = (value: unknown) => ({ ok: true, generation: 2, value });

test('the autocomplete hides saved hosts, numbers the first nine and says when nothing matches', async () => {
  const hosts = ['alpha', 'beta', 'gamma'].map(alias => ({ alias, hostname: alias, username: null, port: null, source: 'ssh-config' }));
  const { native } = fakeNative(request => request.op === 'sshHosts' ? ok({ hosts, available: true, targets: { 'http://127.0.0.1:41234': { alias: 'beta', hostname: 'beta' } } }) : ok({}));
  const client = { config: { keybindings: [] } } as unknown as T3Client;
  const view = await sshHostsView(client, native, true, '');
  expect(view.hosts.map(host => [host.index, host.alias])).toEqual([[0, 'alpha'], [1, 'gamma']]);
  expect(view.hasContent).toBe(true);
  expect((await sshHostsView(client, native, true, 'zz')).noMatch).toBe('No hosts match "zz".');
  expect((await sshHostsView(client, native, false, '')).hosts).toEqual([]);
});

test('adding over SSH tunnels, pairs beside the focused environment and toasts', async () => {
  const { native, calls } = fakeNative(request => {
    if (request.op === 'sshResolve') return ok({ alias: 'devbox', hostname: 'devbox.lan', username: 'me', port: 2222 });
    if (request.op === 'sshConnect') return ok({ origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123', remotePort: 3773, serverKind: 'external' });
    return ok({});
  });
  await runSshOp(native, 'environment-ssh-pick', 'devbox', '', true);
  expect(calls.map(call => call.op)).toEqual(['sshResolve', 'sshConnect', 'pairEnvironment']);
  expect(calls[1]).toMatchObject({ alias: 'devbox', hostname: 'devbox.lan', username: 'me', port: 2222, pair: true });
  expect(calls[2]).toEqual({ op: 'pairEnvironment', origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123', scope: 'orchestration:read orchestration:operate terminal:operate review:write relay:read' });
  calls.length = 0;
  const result = await runSshOp(native, 'environment-ssh-add', 'root@box', 'username=&port=22', false);
  expect(calls.map(call => call.op)).toEqual(['sshConnect', 'connect']);
  expect(calls[0]).toMatchObject({ alias: 'box', username: 'root', port: 22 });
  expect(result.generation).toBe(2);
  const failing = fakeNative(request => request.op === 'sshConnect' ? { ok: false, generation: 0, error: { kind: 'Ssh', message: 'T3 Code is not installed on this host.' } } : ok({}));
  await expect(runSshOp(failing.native, 'environment-ssh-add', 'box', '', true)).rejects.toThrow('T3 Code is not installed on this host.');
  expect(CONNECTION_OPS).toEqual(expect.arrayContaining(SSH_OPS));
});

test('an SSH environment is listed as "SSH user@host" beside the paired loopback one (r9-connect: no "This machine")', () => {
  const host: ConnectionHost = { connection: 'connected', origin: 'http://127.0.0.1:3773', environmentId: 'env-a', statusMessage: '', scopes: [],
    config: { environment: { label: 'This Mac', platform: { machine: 'laptop' } } } };
  const saved = [{ origin: 'http://127.0.0.1:3773', environmentId: 'env-a', label: 'This Mac', enabled: true },
    { origin: 'http://127.0.0.1:41234', environmentId: 'env-ssh', label: 'Devbox', enabled: false }];
  const page = connectionsProjection(host, saved, new Map(), '{}', {}, { 'http://127.0.0.1:41234': { alias: 'devbox', hostname: 'devbox.lan', username: 'me', port: 2222 } });
  expect(page.environments.map(row => [row.label, row.subtitle])).toEqual([['This Mac', 'http://127.0.0.1:3773/ · Connected'], ['Devbox', 'SSH me@devbox.lan:2222 · Off']]);
});

test('adding an SSH route requests five scopes and preserves the expected environment', async () => {
  const { native, calls } = fakeNative(request => request.op === 'sshConnect'
    ? ok({ origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123' }) : ok({ saved: [] }));
  await runSshOp(native, 'environment-ssh-add', 'devbox', 'expectedEnvironmentId=env-a', true);
  expect(calls.find(call => call.op === 'pairEnvironment')).toEqual({
    op: 'pairEnvironment', origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123', expectedEnvironmentId: 'env-a', ssh: true,
    scope: 'orchestration:read orchestration:operate terminal:operate review:write relay:read',
  });
});
