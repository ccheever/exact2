// The composer's ACP agent mark (acp-instance-icon.ts) reads an instance's icon as the sidebar's rows do.
import { describe, expect, test } from 'bun:test';
import { acpInstanceIcon } from '../../src/composer/acp-instance-icon';
import { officialAcpRegistryIconUrlForAgentId } from '../../src/providers/acp-icons';
import { traitsControl } from '../../src/settings/settings-core';

const config = (entry: Record<string, unknown>) => ({ settings: { providerInstances: { 'acp-1': { config: entry } } } });

describe('acpInstanceIcon', () => {
  test('an ACP Registry instance draws its agent id\'s official icon', () => {
    expect(acpInstanceIcon(config({ agentId: 'cursor' }), { driver: 'acpRegistry', instanceId: 'acp-1' }))
      .toBe(officialAcpRegistryIconUrlForAgentId('cursor')!);
  });
  test('a local agent and every other driver have none', () => {
    expect(acpInstanceIcon(config({ agentId: 'cursor', source: 'local' }), { driver: 'acpRegistry', instanceId: 'acp-1' })).toBe('');
    expect(acpInstanceIcon(config({ agentId: 'cursor' }), { driver: 'codex', instanceId: 'acp-1' })).toBe('');
    expect(acpInstanceIcon({}, undefined)).toBe('');
  });
});

test('Settings › General › Model draws the same mark on its trigger', () => {
  const provider = { driver: 'acpRegistry', instanceId: 'acp-1' };
  const row = traitsControl({ providers: [provider], settings: config({ agentId: 'cursor' }).settings }, provider, { slug: 'm', name: 'M' }, []);
  expect(row.markIcon).toBe(officialAcpRegistryIconUrlForAgentId('cursor')!);
});
