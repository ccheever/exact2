import { test, expect } from 'bun:test';
import { parseManualSshTarget, filterSshHosts, formatSshTarget, formatSshError, sshHostsView, runSshOp, sshPairingVersionFailure, SSH_OPS } from '../../src/connections/settings-b-ssh';
import { CLIENT_VERSION } from '../../src/connections/version-skew';
import { ClientError } from '../../src/client/protocol';
import { connectionsProjection, CONNECTION_OPS, type ConnectionHost } from '../../src/connections/connections';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import type { T3Client } from '../../src/client/client';
import { AUTH_STANDARD_CLIENT_SCOPES, LEGACY_CLIENT_SCOPES, encodeOAuthScope } from '../../src/connections/remote-scopes';
const STANDARD_SCOPE = encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES), LEGACY_SCOPE = encodeOAuthScope(LEGACY_CLIENT_SCOPES);

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

test('host suggestions put alias prefixes first, then alias or hostname substrings, case-insensitively', () => {
  const hosts = [{ alias: 'staging', hostname: 'staging' }, { alias: 'dev-box', hostname: 'dev-box' }, { alias: 'box', hostname: 'box' }, { alias: 'mybox', hostname: 'mybox' }];
  expect(filterSshHosts(hosts, ' BOX ').map(host => host.alias)).toEqual(['box', 'dev-box', 'mybox']);
  expect(filterSshHosts(hosts, '').length).toBe(4);
  const resolved = [{ alias: 'devbox', hostname: 'devbox.local' }, { alias: 'gpu', hostname: 'gpu-box.local' }];
  expect(filterSshHosts(resolved, 'box.LOCAL')).toEqual(resolved);
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

test('a configured alias stays offered when only its resolved address is saved; a known host does not', async () => {
  const hosts = [{ alias: 'devbox', hostname: 'devbox.example.com', username: null, port: null, source: 'ssh-config' },
    { alias: 'build.example', hostname: 'build.example', username: null, port: null, source: 'known-hosts' }];
  const targets = { 'http://127.0.0.1:41234': { alias: 'devbox.example.com', hostname: 'devbox.example.com' }, 'http://127.0.0.1:41235': { alias: 'build', hostname: 'build.example' } };
  const { native } = fakeNative(request => request.op === 'sshHosts' ? ok({ hosts, available: true, targets }) : ok({}));
  const client = { config: { keybindings: [] } } as unknown as T3Client;
  await sshHostsView(client, native, false, '');
  expect((await sshHostsView(client, native, true, '')).hosts.map(host => [host.alias, host.address])).toEqual([['devbox', 'devbox.example.com']]);
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
  // `ssh`: the transport words a version-skewed server's refusal for an SSH host's pairing (ssh.provision).
  expect(calls[2]).toEqual({ op: 'pairEnvironment', origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123', scope: STANDARD_SCOPE, legacyScope: LEGACY_SCOPE, ssh: true });
  calls.length = 0;
  const result = await runSshOp(native, 'environment-ssh-add', 'root@box', 'username=&port=22', false);
  expect(calls.map(call => call.op)).toEqual(['sshConnect', 'connect']);
  expect(calls[0]).toMatchObject({ alias: 'box', username: 'root', port: 22 });
  expect(calls[1]).toMatchObject({ op: 'connect', ssh: true });
  expect(result.generation).toBe(2);
  const failing = fakeNative(request => request.op === 'sshConnect' ? { ok: false, generation: 0, error: { kind: 'Ssh', message: 'T3 Code is not installed on this host.' } } : ok({}));
  await expect(runSshOp(failing.native, 'environment-ssh-add', 'box', '', true)).rejects.toThrow('T3 Code is not installed on this host.');
  expect(CONNECTION_OPS).toEqual(expect.arrayContaining(SSH_OPS));
});

test('a pairing 500 from a server on another build names both versions; nothing is saved', async () => {
  // The transport ends a pairing exchange answered with HTTP 500, carrying the remote's descriptor label and version.
  const pairing500 = (serverVersion: string) => ({ ok: false, generation: 0, error: { kind: 'Pairing', message: 'The server returned HTTP 500.',
    reason: 'exchange_http_500', context: { label: 'devbox', serverVersion } } });
  const remote = (failure: unknown) => fakeNative(request => {
    if (request.op === 'sshConnect') return ok({ origin: 'http://127.0.0.1:41234', credential: 'PAIRCODE123' });
    if (request.op === 'pairEnvironment' || request.op === 'connect') return failure;
    return ok({});
  });
  const older = remote(pairing500('0.1.3'));
  await expect(runSshOp(older.native, 'environment-ssh-add', 'devbox', '', true)).rejects.toThrow(
    `devbox runs T3 Code 0.1.3, but this desktop is ${CLIENT_VERSION}, and that server could not accept its pairing. Restart the T3 server on devbox so it runs ${CLIENT_VERSION}, then retry.`);
  expect(older.calls.map(call => call.op)).toEqual(['sshConnect', 'pairEnvironment']);
  // The window's first connection takes the same sentence from `connect`.
  await expect(runSshOp(remote(pairing500('0.1.3')).native, 'environment-ssh-add', 'devbox', '', false)).rejects.toThrow('devbox runs T3 Code 0.1.3');
  // A server on this build, or another failure, keeps its own message.
  await expect(runSshOp(remote(pairing500(CLIENT_VERSION)).native, 'environment-ssh-add', 'devbox', '', true)).rejects.toThrow('The server returned HTTP 500.');
  const unauthorized = { ok: false, generation: 0, error: { kind: 'Authentication', message: 'The environment credential is invalid.' } };
  await expect(runSshOp(remote(unauthorized).native, 'environment-ssh-add', 'devbox', '', true)).rejects.toThrow('The environment credential is invalid.');
  expect(sshPairingVersionFailure(new ClientError('The server returned HTTP 502.', 'HTTP', false, { context: { label: 'devbox', serverVersion: '0.1.3' } }))).toBeNull();
  expect(sshPairingVersionFailure(new ClientError('The server returned HTTP 500.', 'Pairing', false, { reason: 'exchange_http_500' }))).toBeNull();
});

test('an SSH environment is listed as "SSH user@host" beside the paired loopback one (no "This machine")', () => {
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
    scope: STANDARD_SCOPE, legacyScope: LEGACY_SCOPE,
  });
});
