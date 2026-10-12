// Ports of the reference tests for the install-aware update command and the standard scopes
// (T3 Code MIT, see LICENSE-T3; 1e2ecbd975: apps/web/src/versionSkew.test.ts,
// apps/web/src/components/ServerUpdateAction.test.tsx, packages/contracts/src/environment.test.ts).
import { describe, test, expect } from 'bun:test';
import { configInstallation, decodeServerInstallation, desktopManagedOnly, manualServerUpdateCommand, manualUpdateCopy, serverUpdateActionLabel,
  serverUpdateAriaLabel, DESKTOP_MANAGED_NOTE, type ServerInstallation } from './server-installation';
import { AUTH_STANDARD_CLIENT_SCOPES, encodeOAuthScope, withStandardScope } from './remote-scopes';
import { connectionsProjection, runConnectionOp, CLIENT_VERSION, type ConnectionHost } from './connections';
import { fleet } from './settings-b-fleet';
import { toasts } from './toast';
import { obj, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';

describe('versionSkew', () => {
  test('updates only the proven npm prefix and safely quotes its path', () => {
    expect(manualServerUpdateCommand('0.0.45', { kind: 'npm-global', prefix: '/opt/node' })).toBe("npm install --global --prefix '/opt/node' t3@0.0.45");
    expect(manualServerUpdateCommand('0.0.45', { kind: 'npm-global', prefix: "/opt/maria's node" })).toBe("npm install --global --prefix '/opt/maria'\\''s node' t3@0.0.45");
  });

  test('keeps runner and unknown commands as relaunches', () => {
    expect(manualServerUpdateCommand('0.0.45')).toBe('npx t3@0.0.45');
    expect(manualServerUpdateCommand('0.0.45', { kind: 'npx' })).toBe('npx t3@0.0.45');
    expect(manualServerUpdateCommand('0.0.45', { kind: 'pnpm-dlx' })).toBe('pnpm dlx t3@0.0.45');
    expect(manualServerUpdateCommand('0.0.45', { kind: 'bunx' })).toBe('bunx t3@0.0.45');
  });
});

describe('ServerEnvironmentDescriptor', () => {
  test('decodes old, recognized and future manual installation descriptors', () => {
    const descriptor = { environment: { capabilities: {} as Obj } };
    expect(configInstallation(descriptor)).toBeUndefined();
    for (const installation of [{ kind: 'npx' }, { kind: 'npm-global', prefix: '/opt/node' }]) {
      expect(configInstallation({ environment: { capabilities: { serverInstallation: installation } } })).toEqual(installation as ServerInstallation);
    }
    for (const installation of [{ kind: 'future-manager' }, { kind: 'npm-global' }]) {
      expect(configInstallation({ environment: { capabilities: { serverInstallation: installation } } })).toBeUndefined();
    }
    // TrimmedNonEmptyString: the prefix is trimmed and a blank one is absent.
    expect(decodeServerInstallation({ kind: 'npm-global', prefix: '  /opt/node ' })).toEqual({ kind: 'npm-global', prefix: '/opt/node' });
    expect(decodeServerInstallation({ kind: 'npm-global', prefix: '   ' })).toBeUndefined();
    expect(decodeServerInstallation('npx')).toBeUndefined();
  });
});

/** A transport that copies (or refuses to) and records every request. */
class Transport implements Native {
  available = true; calls: Obj[] = []; copied: string[] = []; refuseCopy = false;
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    if (request.op === 'copyText') {
      if (this.refuseCopy) return { ok: false, generation: 4, error: { kind: 'Pasteboard', message: 'The pasteboard refused the text.' } };
      this.copied.push(String(request.text)); return { ok: true, generation: 4, value: { copied: true } };
    }
    return { ok: true, generation: 4, value: {} };
  }
}
const focused = (capabilities: Obj) => ({ origin: 'http://127.0.0.1:16140', environmentId: 'env-a', connection: 'connected', generation: 4,
  config: { environment: { label: 'Test', serverVersion: '0.0.40', capabilities } } }) as unknown as T3Client;
const key = 'http://127.0.0.1:16140\nenv-a';

describe('ServerUpdateAction', () => {
  test.each([
    [{ kind: 'npm-global', prefix: '/opt/node' }, `npm install --global --prefix '/opt/node' t3@${CLIENT_VERSION}`, 'Update command copied', 'then restart t3', 'Copy update command'],
    [{ kind: 'npx' }, `npx t3@${CLIENT_VERSION}`, 'Relaunch command copied', 'This does not update an installed t3 command.', 'Copy relaunch command'],
    [undefined, `npx t3@${CLIENT_VERSION}`, 'Relaunch command copied', 'This does not update an installed t3 command.', 'Copy relaunch command'],
    [{ kind: 'pnpm-dlx' }, `pnpm dlx t3@${CLIENT_VERSION}`, 'Relaunch command copied', 'This does not update an installed t3 command.', 'Copy relaunch command'],
    [{ kind: 'bunx' }, `bunx t3@${CLIENT_VERSION}`, 'Relaunch command copied', 'This does not update an installed t3 command.', 'Copy relaunch command'],
    [{ kind: 'future-manager' }, `npx t3@${CLIENT_VERSION}`, 'Relaunch command copied', 'This does not update an installed t3 command.', 'Copy relaunch command'],
  ] as [Obj | undefined, string, string, string, string][])('copies an honest manual command for %j without invoking remote update', async (installation, command, title, guidance, label) => {
    const native = new Transport(), client = focused(installation ? { serverInstallation: installation } : {});
    await runConnectionOp(native, 'environment-update', key, CLIENT_VERSION, true, client);
    expect(native.copied).toEqual([command]);
    expect(native.calls.some(call => call.op === 'request')).toBe(false);
    const toast = toasts(client).at(-1)!;
    expect(toast).toMatchObject({ kind: 'success', title });
    expect(toast.description).toContain(guidance);
    expect(serverUpdateActionLabel('', configInstallation(client.config))).toBe(label);
  });

  test('the toasts and labels say exactly what the reference says', () => {
    expect(manualUpdateCopy('0.0.45', { kind: 'npm-global', prefix: '/opt/node' }, 'Box server')).toEqual({
      command: "npm install --global --prefix '/opt/node' t3@0.0.45", label: 'Copy update command', title: 'Update command copied',
      description: "Run `npm install --global --prefix '/opt/node' t3@0.0.45` on Box server, then restart t3 with your usual options.",
      failureTitle: 'Could not copy update command', failureMessage: 'Failed to copy update command to the clipboard.' });
    expect(manualUpdateCopy('0.0.45', { kind: 'bunx' }, 'Box server')).toMatchObject({ label: 'Copy relaunch command', title: 'Relaunch command copied',
      description: 'Stop t3 on Box server, then relaunch with `bunx t3@0.0.45` using the same subcommand and options. This does not update an installed t3 command.',
      failureMessage: 'Failed to copy relaunch command to the clipboard.' });
    expect(serverUpdateActionLabel('boot-service', undefined)).toBe('Update');
    expect(serverUpdateActionLabel('respawn', { kind: 'npm-global', prefix: '/x' }, 'Retry update')).toBe('Retry update');
    expect(serverUpdateAriaLabel('Copy relaunch command', 'Box server')).toBe('Copy relaunch command for Box server');
  });

  test('a refused pasteboard write is "Could not copy update command" with the copy error', async () => {
    const native = new Transport(), client = focused({ serverInstallation: { kind: 'npm-global', prefix: '/opt/node' } });
    native.refuseCopy = true;
    await expect(runConnectionOp(native, 'environment-update', key, CLIENT_VERSION, true, client)).rejects.toThrow('Failed to copy update command to the clipboard.');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not copy update command', description: 'Failed to copy update command to the clipboard.' });
    expect(native.calls.some(call => call.op === 'request')).toBe(false);
  });

  test('keeps the manual instruction for desktop servers without remote update support', () => {
    expect(desktopManagedOnly({ serverSelfUpdate: 'desktop-managed' })).toBe(true);
    expect(desktopManagedOnly({ serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true })).toBe(false);
    const host = (capabilities: Obj): ConnectionHost => ({ connection: 'connected', origin: 'http://127.0.0.1:16140', environmentId: 'env-a', statusMessage: '', scopes: [],
      config: { environment: { label: 'Box', serverVersion: '0.0.40', capabilities } } });
    const saved = [{ origin: 'http://127.0.0.1:16140', environmentId: 'env-a', label: 'Box', enabled: true }];
    fleet.entries.clear();
    const desktop = connectionsProjection(host({ serverSelfUpdate: 'desktop-managed' }), saved).environments[0]!;
    expect(desktop).toMatchObject({ update: '', updateNote: DESKTOP_MANAGED_NOTE });
    const remote = connectionsProjection(host({ serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true }), saved).environments[0]!;
    expect(remote).toMatchObject({ update: 'Update', updateNote: '' });
    const manual = connectionsProjection(host({ serverInstallation: { kind: 'npm-global', prefix: '/opt/node' } }), saved).environments[0]!;
    expect(manual).toMatchObject({ update: 'Copy update command', updateNote: '' });
    // A current server shows neither.
    const current = connectionsProjection({ ...host({ serverSelfUpdate: 'desktop-managed' }), config: { environment: { label: 'Box', serverVersion: CLIENT_VERSION, capabilities: { serverSelfUpdate: 'desktop-managed' } } } }, saved).environments[0]!;
    expect(current).toMatchObject({ update: '', updateNote: '' });
  });
});

describe('AuthStandardClientScopes', () => {
  test('the standard five, in the reference order, encoded as one space-separated scope', () => {
    expect([...AUTH_STANDARD_CLIENT_SCOPES]).toEqual(['orchestration:read', 'orchestration:operate', 'terminal:operate', 'review:write', 'relay:read']);
    expect(encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES)).toBe('orchestration:read orchestration:operate terminal:operate review:write relay:read');
    expect(() => encodeOAuthScope([])).toThrow(ClientError);
    expect(() => encodeOAuthScope(['a', 'a'])).toThrow('unique');
    expect(() => encodeOAuthScope(['has space'])).toThrow('syntactically valid');
  });

  test('a request with a credential asks for the standard scopes; a saved-token reconnect asks for none', () => {
    expect(withStandardScope({ origin: 'http://h', credential: 'code' })).toEqual({ origin: 'http://h', credential: 'code', scope: 'orchestration:read orchestration:operate terminal:operate review:write relay:read' });
    expect(withStandardScope({ origin: 'http://h', credential: '' })).toEqual({ origin: 'http://h', credential: '' });
  });

  test('every pairing path sends the standard scope to the transport', async () => {
    const native = new Transport();
    // Add environment while connected (pairEnvironment) and with nothing connected (connect).
    await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:16141', 'code-1', true).catch(() => undefined);
    await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:16141', 'code-2', false).catch(() => undefined);
    const exchanges = native.calls.filter(call => call.credential);
    expect(exchanges.map(call => [call.op, call.scope])).toEqual([
      ['pairEnvironment', encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES)],
      ['connect', encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES)]]);
  });

  test('a session paired before this change keeps working with its three scopes (U12: left as is)', () => {
    // The old exchange's grant, kept only as a fixture of a pre-change session.
    const legacy = 'orchestration:read orchestration:operate review:write'.split(' ');
    const host: ConnectionHost = { connection: 'connected', origin: 'http://127.0.0.1:16140', environmentId: 'env-a', statusMessage: '', scopes: legacy,
      config: { environment: { label: 'Old pairing', serverVersion: CLIENT_VERSION, capabilities: { environmentIcon: true } } } };
    const row = connectionsProjection(host, [{ origin: 'http://127.0.0.1:16140', environmentId: 'env-a', label: 'Old pairing', enabled: true }]).environments[0]!;
    expect(row.iconLock).toBe('');
    expect(legacy.includes('terminal:operate') || legacy.includes('relay:read')).toBe(false);
  });
});
