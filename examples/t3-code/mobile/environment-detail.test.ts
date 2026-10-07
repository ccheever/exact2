import { describe, expect, test } from 'bun:test';
import { environmentDetailProjection, mobileCanUpdateProvider, mobileSessionGrants, mobileReleaseChannel, newestMobileRelease, mobileProviderIconURL } from './environment-detail';
import { environmentSources } from './shared/connections';
import { showcaseCloudEnvironments, showcaseDisplayURL, showcaseSubmittedURL } from './showcase';

const session = { authenticated: true, permissions: ['environment:maintain', 'providers:manage', 'orchestration:read'] };
const provider = { enabled: true, installed: true, instanceId: 'codex-main', driver: 'codex', version: '1.0.0',
  versionAdvisory: { status: 'behind_latest', canUpdate: true, latestVersion: '1.1.0' } };
function source() {
  return environmentSources({ connection: 'connected', origin: 'http://localhost:3773', environmentId: 'local', scopes: [], statusMessage: '',
    config: { environment: { serverVersion: '1.0.0', label: 'Local' }, providers: [provider] } },
    [{ origin: 'http://localhost:3773', environmentId: 'local', enabled: true }], new Map())[0]!;
}
describe('mobile environment detail permissions', () => {
  test('explicit empty permissions deny even legacy operate grants', () => {
    const explicit = { authenticated: true, permissions: [], scopes: ['orchestration:operate', 'environment:maintain'] };
    expect(mobileSessionGrants(explicit, 'environment:maintain')).toBe(false);
    const data = environmentDetailProjection(source(), explicit);
    expect(data.disabled).toBe(true); expect(data.refreshDisabled).toBe(true); expect(data.providers[0]!.disabled).toBe(true);
  });
  test('old sessions retain upstream legacy fallback only without serverUpdateScope', () => {
    expect(mobileSessionGrants({ authenticated: true, scopes: ['orchestration:operate'] }, 'environment:maintain')).toBe(true);
    expect(mobileSessionGrants({ authenticated: true, scopes: ['orchestration:operate'], auth: { serverUpdateScope: 'environment:maintain' } }, 'environment:maintain')).toBe(false);
    expect(mobileSessionGrants({ authenticated: false, permissions: ['environment:maintain'] }, 'environment:maintain')).toBe(false);
  });
  test('disconnected, unverifiable sessions and busy providers disable maintenance', () => {
    const live = source(); live.phase = 'reconnecting';
    expect(environmentDetailProjection(live, session).disabled).toBe(true);
    expect(environmentDetailProjection(source(), null, { sessionFailed: true }).permissionNote).toContain('Could not verify');
    const busy = source(); busy.config.providers = [{ ...provider, updateState: { status: 'queued' } }];
    expect(environmentDetailProjection(busy, session).busy).toBe(true);
    expect(environmentDetailProjection(source(), session).disabled).toBe(false);
  });
  test('provider update eligibility excludes install, broken and queued cases', () => {
    expect(mobileCanUpdateProvider(provider)).toBe(true);
    for (const patch of [{ installed: false }, { availability: 'unavailable' }, { updateState: { status: 'queued' } }, { compatibilityAdvisory: { latestVersionStatus: 'broken' } }])
      expect(mobileCanUpdateProvider({ ...provider, ...patch })).toBe(false);
  });
});
describe('pinned release and icon rules', () => {
  test('channels skip drafts and do not cross release trains', () => {
    expect(mobileReleaseChannel('1.0.0-nightly.20261007.1')).toBe('nightly');
    expect(newestMobileRelease([{ tag_name: 'v2.0.0', draft: true }, { tag_name: 'v1.2.0-preview.20261007.1' }, { tag_name: 'v1.1.0' }], 'stable')).toBe('1.1.0');
    expect(() => newestMobileRelease([{}], 'stable')).toThrow('invalid');
  });
  test('ACP icons accept only official credential-free HTTPS origin', () => {
    expect(mobileProviderIconURL('https://cdn.agentclientprotocol.com/icon.svg')).toContain('/icon.svg');
    for (const value of ['http://cdn.agentclientprotocol.com/icon.svg', 'https://other.test/icon.svg', 'https://user@cdn.agentclientprotocol.com/a', 'https://cdn.agentclientprotocol.com:8080/a', null])
      expect(mobileProviderIconURL(value)).toBe('');
  });
});
test('showcase is opt-in and preserves actual endpoints when saving unchanged display URLs', () => {
  expect(showcaseCloudEnvironments(false).connected).toEqual([]);
  expect(showcaseCloudEnvironments(false).available).toEqual([]);
  expect(showcaseCloudEnvironments(true).connected[0]!.environmentLabel).toBe('Aurora GPU Pod');
  expect(showcaseCloudEnvironments(true).available[0]!.environment.label).toBe('Pocket Pi');
  expect(showcaseDisplayURL(false, 'Moonbase Terminal', 'http://localhost')).toBe('http://localhost');
  expect(showcaseSubmittedURL(true, 'http://localhost', 'https://moonbase.tail9f3a.ts.net/', 'https://moonbase.tail9f3a.ts.net/')).toBe('http://localhost');
});

import { mobileClient, mobileCommand } from './client';
import { mobileEnvironmentDetailCommand } from './environment-detail';
import type { Native, Files } from './shared/protocol';
const unusedStorage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };

test('targeted pairing forwards the expected identity and reports mismatch without catalogue writes', async () => {
  const requests: Record<string, unknown>[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = input as Record<string, unknown>; requests.push(request);
    if (request.op === 'environments') return { ok: true, generation: 0, value: { saved: [{ origin: 'https://old.example', environmentId: 'target', label: 'Target' }] } };
    if (request.op === 'devicePresentation' || request.op === 'nativeReadFile') return { ok: true, generation: 0, value: {} };
    if (request.op === 'pairEnvironment') return { ok: false, generation: 0, error: { kind: 'Pairing', message: 'That address reaches another environment, a different machine. Add it as its own environment instead.' } };
    throw new Error(`Unexpected native operation ${request.op}`);
  } };
  const result = await mobileCommand(['environment-add-route', 'https://new.example', 'one-time-code', 'target'], native, unusedStorage);
  expect(result.message).toContain('different machine');
  expect(requests.some(request => ['setRoutes', 'connect', 'fleetOutdatedPair'].includes(String(request.op)))).toBe(false);
  const pairing = requests.find(request => request.op === 'pairEnvironment');
  expect(pairing?.expectedEnvironmentId).toBe('target');
  expect(pairing?.origin).toBe('https://new.example');
});

test('focused provider refresh uses real auth permissions and the ordinary RPC dispatcher', async () => {
  const before = { connection: mobileClient.connection, generation: mobileClient.generation, origin: mobileClient.origin,
    environmentId: mobileClient.environmentId, config: mobileClient.config };
  Object.assign(mobileClient, { connection: 'connected', generation: 71, origin: 'https://detail.example', environmentId: 'detail-test', config: source().config });
  let permissions = ['orchestration:read'];
  const requests: Record<string, unknown>[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = input as Record<string, unknown>; requests.push(request);
    const value = request.op === 'environments' ? { saved: [{ origin: 'https://detail.example', environmentId: 'detail-test' }] }
      : request.op === 'status' ? { state: 'connected' } : request.op === 'http' ? { authenticated: true, permissions, scopes: ['orchestration:operate'] } : {};
    return { ok: true, generation: 71, value };
  } };
  try {
    const key = 'https://detail.example\ndetail-test';
    expect((await mobileEnvironmentDetailCommand('refresh-providers', key, '', native)).message).toBe('');
    expect(requests.find(request => request.op === 'request')?.method).toBe('server.refreshProviders');
    requests.length = 0; permissions = [];
    expect((await mobileEnvironmentDetailCommand('refresh-providers', key, '', native)).message).toContain('permission');
    expect(requests.some(request => request.op === 'request')).toBe(false);
  } finally { Object.assign(mobileClient, before); }
});
