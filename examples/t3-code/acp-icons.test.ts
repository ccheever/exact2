// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/settings/
// AcpRegistryIcon.test.ts (:105, original name). The cache, blob and fetch-policy cases are the
// host's image loading (X44), not app code. Clone cases for the instance icon follow.
import { describe, expect, it, test } from 'bun:test';
import { instanceIconUrl, officialAcpRegistryIconUrlForAgentId, resolveOfficialAcpRegistryIconUrl, resolveProviderInstanceAcpRegistryIconUrl } from './acp-icons';

describe('AcpRegistryIcon', () => {
  it('accepts only credential-free HTTPS URLs on the official CDN', () => {
    expect(resolveOfficialAcpRegistryIconUrl('https://cdn.agentclientprotocol.com/registry/icons/gemini.png')).toBe('https://cdn.agentclientprotocol.com/registry/icons/gemini.png');
    expect(resolveOfficialAcpRegistryIconUrl('http://cdn.agentclientprotocol.com/icon.png')).toBeNull();
    expect(resolveOfficialAcpRegistryIconUrl('https://cdn.agentclientprotocol.com.evil/icon.png')).toBeNull();
    expect(resolveOfficialAcpRegistryIconUrl('https://user@cdn.agentclientprotocol.com/icon.png')).toBeNull();
    expect(resolveOfficialAcpRegistryIconUrl('https://cdn.agentclientprotocol.com:8443/icon.png')).toBeNull();
    expect(officialAcpRegistryIconUrlForAgentId('kilo')).toBe('https://cdn.agentclientprotocol.com/registry/v1/latest/kilo.svg');
    expect(officialAcpRegistryIconUrlForAgentId('../kilo')).toBeNull();
  });
});

test('an instance draws its saved registry icon, else its agent id\'s, and nothing for other drivers', () => {
  expect(resolveProviderInstanceAcpRegistryIconUrl({ driverKind: 'codex', agentId: 'kilo' })).toBeNull();
  expect(instanceIconUrl({ driver: 'acpRegistry', config: { agentId: 'devin', registryIconUrl: 'https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg' } }))
    .toBe('https://cdn.agentclientprotocol.com/registry/v1/latest/devin.svg');
  expect(instanceIconUrl({ driver: 'acpRegistry', config: { agentId: 'kilo', registryIconUrl: 'https://evil.example/kilo.svg' } })).toBe('https://cdn.agentclientprotocol.com/registry/v1/latest/kilo.svg');
  expect(instanceIconUrl({ driver: 'acpRegistry', config: {} })).toBe('');
  expect(instanceIconUrl({ driver: 'acpRegistry', config: { agentId: ' kilo ' } })).toBe('https://cdn.agentclientprotocol.com/registry/v1/latest/kilo.svg');
});
