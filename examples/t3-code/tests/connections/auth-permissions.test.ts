import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
import { AUTH_SCOPE_OPTIONS, READ_ONLY_SCOPES, pairingScopeOptions, sessionPermissionRows } from '../../src/connections/auth-scope-options';
import { requireRpcPermissions, rpcPermissions } from '../../src/connections/command-permissions';
import { decodeClientSession, decodePairingLink } from '../../src/connections/auth-access';
import { closedNetworkView, networkUi, resetNetwork, runNetworkOp } from '../../src/connections/connections-network';
import { toasts } from '../../src/shell/toast';
import { providerWritable } from '../../src/providers/provider-permissions';
import { terminalDrawerView, terminalMetadataEvent, terminalOps } from '../../src/terminal/terminal-drawer-view';

const accepted = (generation: number, value: Obj = {}) => ({ ok: true, generation, value });
function clientFixture(permissions: readonly string[]) {
  const client = new T3Client(), requests: Obj[] = [];
  Object.assign(client, { connection: 'connected', configLive: true, shellLive: true, threadLive: true,
    generation: 1, environmentId: 'env', projectId: 'project', threadId: 'thread', scopes: [...permissions] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, settings: {} };
  client.shell.projects = [{ id: 'project', workspaceRoot: '/fixture' }];
  client.shell.threads = [{ id: 'thread', projectId: 'project' }];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); requests.push(request);
    return accepted(1, request.op === 'subscribe' ? { id: '1-1' } : request.op === 'localAccess' ? { id: 'created-link', credential: 'fixture-credential', expiresAt: '2040-01-01T00:00:00Z' } : {});
  } };
  return { client, requests, native };
}

describe('effective session permissions', () => {
  test('the standard selector contains all sixteen unique permissions and the exact read-only preset', () => {
    expect(AUTH_SCOPE_OPTIONS).toHaveLength(16);
    expect(new Set(AUTH_SCOPE_OPTIONS.map(option => option.scope))).toEqual(new Set(AUTH_STANDARD_CLIENT_SCOPES));
    expect(new Set(pairingScopeOptions().filter(option => option.readOnly).map(option => option.scope))).toEqual(new Set(READ_ONLY_SCOPES));
    expect(closedNetworkView().scopeOptions).toEqual(pairingScopeOptions());
  });

  test('pairing and client records display permissions rather than the compatibility scopes, including an empty grant', () => {
    const base = { id: 'link', sessionId: 'session', createdAt: '2036-01-01T00:00:00Z', expiresAt: '2036-01-01T12:00:00Z', scopes: ['orchestration:operate'], permissions: [] };
    expect(decodePairingLink(base)?.scopes).toEqual([]);
    expect(decodeClientSession(base)?.scopes).toEqual([]);
    const rows = sessionPermissionRows(['terminal:read']);
    expect(rows).toHaveLength(16);
    expect(rows.filter(row => row.allowed).map(row => row.scope)).toEqual(['terminal:read']);
  });

  test('a custom pairing grant reaches the primary unchanged; unknown or repeated scopes never send a request', async () => {
    resetNetwork();
    const { client, native, requests } = clientFixture(AUTH_STANDARD_CLIENT_SCOPES);
    await runNetworkOp(client, native, 'pairing-create', 'Reviewer', 'orchestration:read filesystem:read terminal:read');
    expect(obj(requests.at(-1)?.body).scopes).toEqual(['orchestration:read', 'filesystem:read', 'terminal:read']);
    const serial = networkUi.createSerial, count = requests.length;
    for (const invalid of ['orchestration:read bogus', 'terminal:read terminal:read', '']) {
      await runNetworkOp(client, native, 'pairing-create', 'Reviewer', invalid);
      expect(requests).toHaveLength(count);
      expect(networkUi.createSerial).toBe(serial);
      expect(toasts(client).at(-1)?.kind).toBe('error');
    }
    resetNetwork();
  });

  for (const [method, permission] of [
    ['server.upsertKeybinding', 'settings:write'], ['provider.auth.start', 'providers:manage'],
    ['server.updateServer', 'environment:maintain'], ['preview.open', 'preview:operate'],
    ['server.getUsageSummary', 'diagnostics:read'], ['terminal.observe', 'terminal:read'],
    ['terminal.write', 'terminal:operate'], ['vcs.createRef', 'source-control:write'],
    ['projects.readFile', 'filesystem:read'], ['projects.writeFile', 'filesystem:write'],
  ] as const) {
    test(`${method} needs ${permission}, independently of task operation`, async () => {
      const denied = clientFixture(['orchestration:read', 'orchestration:operate']);
      await expect(denied.client.request(denied.native, method, {})).rejects.toThrow(permission);
      expect(denied.requests).toHaveLength(0);
      const allowed = clientFixture([permission]);
      await allowed.client.request(allowed.native, method, {});
      expect(allowed.requests).toMatchObject([{ op: 'request', method }]);
    });
  }

  test('provider management and settings patches do not borrow each other’s grant', () => {
    expect(rpcPermissions('server.updateSettings', { patch: { branchNamePrefix: 'fixture' } })).toEqual(['settings:write']);
    expect(rpcPermissions('server.updateSettings', { patch: {}, providerInstanceMutation: {} })).toEqual(['providers:manage']);
    expect(rpcPermissions('server.updateSettings', { patch: { providerInstances: {}, branchNamePrefix: 'fixture' } })).toEqual(['settings:write', 'providers:manage']);
    expect(providerWritable({ ready: true, writable: true, scopes: ['orchestration:operate'] })).toBe(false);
    expect(providerWritable({ ready: true, writable: false, scopes: ['providers:manage'] })).toBe(true);
    expect(() => requireRpcPermissions(['source-control:write'], { op: 'request', method: 'git.preparePullRequestThread', payload: { mode: 'worktree', threadId: 'thread' } })).toThrow('orchestration:operate');
    expect(() => requireRpcPermissions(['orchestration:operate'], { op: 'request', method: 'projects.mutate', payload: { type: 'project.update', scripts: [] } })).toThrow('settings:write');
  });

  test('a terminal reader can view existing sessions without creating, splitting, closing, or writing them', async () => {
    const { client, native, requests } = clientFixture(['orchestration:read', 'terminal:read']);
    await terminalDrawerView(client, native, 840, 0);
    terminalMetadataEvent(client, { subscriptionId: '1-1', value: { type: 'snapshot', terminals: [{ threadId: 'thread', terminalId: 'term-1', status: 'running', label: 'Reader terminal', cwd: '/fixture' }] } });
    const out = { message: '', id: '', value: '' };
    await terminalOps.call(client, 'terminallocal:toggle', '', '', 0, native, {} as Files, out);
    const view = await terminalDrawerView(client, native, 840, 0);
    expect(view).toMatchObject({ available: true, open: true, terminalId: 'term-1', readOnly: true, splitDisabled: true });
    for (const action of ['new', 'split', 'close']) await expect(terminalOps.call(client, `terminallocal:${action}`, 'env:thread|term-1', '', 0, native, {} as Files, out)).rejects.toThrow('terminal:operate');
    expect(requests.some(request => request.method === 'terminal.open' || request.method === 'terminal.write')).toBe(false);
  });
});
