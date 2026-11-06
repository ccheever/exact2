// Settings › Devices › Device sources (DeviceSourcesSettings.tsx at T3 Code 5d3d78ab93): the rows, statuses and
// platform popovers the section draws over the scope's environments.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { HostTarget } from '../../src/device/settings-a-hosts';
import { deviceSourcesView } from '../../src/device/settings-device-sources';

const mini = { id: 'mini', label: 'Mac mini', target: 'me@mini' };
const cloud = { kind: 'eas', id: 'cloud', label: 'Cloud iPhone', accessToken: '', maxSessionMinutes: 40, idleMinutes: 5, maxConcurrentSessions: 1 };
const target = (environmentId: string, label: string, settings: Obj, state: Obj | null = null): HostTarget =>
  ({ client: { environmentId, config: { settings } } as unknown as T3Client, label, secondary: '', state });
const ios = (available: boolean, reason?: string) => ({ platform: 'ios', available, ...(reason ? { reason } : {}) });
const android = (available: boolean, reason?: string) => ({ platform: 'android', available, ...(reason ? { reason } : {}) });

describe('Device sources', () => {
  test('one environment with devices off: Local simulators alone, no status or platform, Add source on', () => {
    const view = deviceSourcesView([target('env1', 'This machine', { enableDeviceSupport: false })], 'env1', true, false, false);
    expect(view).toMatchObject({ canAdd: true, disabled: false, eas: true });
    expect(view.rows).toMatchObject([{ key: 'local', title: 'Local simulators', kind: '', description: "Simulators and emulators installed on this environment's machine.", problems: [], pending: '', platforms: [], actions: false }]);
    // A project scope, no connected environment, or an open editor turns Add source off; the first two the rows' actions too.
    expect(deviceSourcesView([target('env1', 'Studio', {})], 'env1', true, true, false)).toMatchObject({ canAdd: false, disabled: true });
    expect(deviceSourcesView([], '', false, false, false)).toMatchObject({ canAdd: false, disabled: true, rows: [{ key: 'local' }] });
    expect(deviceSourcesView([target('env1', 'Studio', {})], 'env1', true, false, true)).toMatchObject({ canAdd: false, disabled: false });
  });

  test('an SSH host: its target, a failure with Retry, and its platforms; testing is pending', () => {
    const state = { hosts: [{ id: 'local', kind: 'local', platforms: [ios(false, 'Install Xcode.'), android(true)] }, { id: 'mini', kind: 'ssh', platforms: [ios(true), android(false, 'No SDK.')] }],
      hostStatuses: { mini: { status: 'failed', detail: 'ssh: connect refused' } }, supportsHostRetry: true };
    const view = deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] }, state)], 'env1', false, false, false);
    const [local, ssh] = view.rows;
    expect(local!.platforms.map(platform => [platform.id, platform.available, platform.title, platform.reason])).toEqual([['ios', false, 'iOS not available', 'Install Xcode.'], ['android', true, 'Android available', '']]);
    expect(ssh).toMatchObject({ title: 'Mac mini', kind: 'SSH', description: 'me@mini', actions: true, test: true, edit: 'hosts-edit', editParams: 'id=mini', remove: 'hosts-remove',
      problems: [{ tone: 'error', text: 'ssh: connect refused', action: 'Retry', op: 'hosts-retry', params: 'id=mini&environment=env1' }] });
    expect(ssh!.platforms.map(platform => [platform.aria, platform.available])).toEqual([['Mac mini: iOS available', true], ['Mac mini: Android not available', false]]);
    // Without retry support the status is the plain "Failed"; a starting host is pending; no device state yet is "Loading".
    const plain = deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] }, { ...state, supportsHostRetry: false })], 'env1', false, false, false).rows[1]!;
    expect(plain.problems).toMatchObject([{ text: 'ssh: connect refused', action: '' }]);
    const starting = deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] }, { ...state, hostStatuses: { mini: { status: 'starting' } } })], 'env1', false, false, false).rows[1]!;
    expect([starting.pending, starting.problems]).toEqual(['Starting', []]);
    expect(deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] })], 'env1', false, false, false).rows[1]!.pending).toBe('Loading');
  });

  test('a failed local host offers Refresh; a ready one with an incomplete Android list warns', () => {
    const failed = { hosts: [{ id: 'local', kind: 'local', platforms: [] }], hostStatuses: { local: { status: 'failed', detail: 'Hub crashed.' } } };
    expect(deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true }, failed)], 'env1', false, false, false).rows[0]!.problems)
      .toEqual([{ id: 'env1', tone: 'error', text: 'Hub crashed.', action: 'Refresh', op: 'hosts-refresh', params: 'environment=env1' }]);
    const partial = { hosts: [{ id: 'local', kind: 'local', platforms: [] }], hostStatuses: { local: { status: 'ready', androidDiscoveryIncomplete: true } } };
    expect(deviceSourcesView([target('env1', 'Studio', { enableDeviceSupport: true }, partial)], 'env1', false, false, false).rows[0]!.problems)
      .toMatchObject([{ tone: 'warning', text: 'Android list may be incomplete', action: '' }]);
  });

  test('across environments a shared host is one row; its popover lists where it is connected and not added', () => {
    const state = (available: boolean) => ({ hosts: [{ id: 'mini', kind: 'ssh', platforms: [ios(available, available ? undefined : 'Xcode missing.')] }], hostStatuses: {} });
    const targets = [target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] }, state(true)), target('env2', 'Laptop', { enableDeviceSupport: true }, state(true))];
    const view = deviceSourcesView(targets, 'env1', false, false, false);
    expect(view.rows[0]!.description).toBe("Simulators and emulators installed on each environment's own machine.");
    const ssh = view.rows[1]!;
    expect([ssh.key, ssh.editParams]).toEqual(['ssh:mini:env1', 'id=mini']);
    expect(ssh.platforms[0]).toMatchObject({ title: 'iOS available', groups: [{ heading: 'Connected', entries: [{ label: 'Studio' }] }, { heading: 'Not added', entries: [{ label: 'Laptop' }] }] });
    // Where the environments disagree the popover says which can use it, with each one's reason, and marks the icon partial.
    const both = [target('env1', 'Studio', { enableDeviceSupport: true, deviceHosts: [mini] }, state(true)), target('env2', 'Laptop', { enableDeviceSupport: true, deviceHosts: [mini] }, state(false))];
    expect(deviceSourcesView(both, 'env1', false, false, false).rows[1]!.platforms[0]).toMatchObject({ partial: true, title: 'iOS available on some',
      groups: [{ heading: 'Available', entries: [{ label: 'Studio' }] }, { heading: 'Not available', entries: [{ label: 'Laptop', detail: 'Xcode missing.' }] }] });
    // Hosts that cannot be matched across environments stay one row each, acting on their own environment.
    const clash = [target('env1', 'Studio', { deviceHosts: [mini] }), target('env2', 'Laptop', { deviceHosts: [{ ...mini, label: 'Other name' }] })];
    expect(deviceSourcesView(clash, 'env1', false, false, false).rows.slice(1).map(row => [row.title, row.editParams])).toEqual([['Mac mini', 'id=mini&environment=env1'], ['Other name', 'id=mini&environment=env2']]);
  });

  test('EAS hosts show only where the server supports them, once each, edited on an environment that holds them', () => {
    const targets = [target('env1', 'Studio', { easDeviceHosts: [] }), target('env2', 'Laptop', { easDeviceHosts: [cloud] })];
    expect(deviceSourcesView(targets, 'env1', false, false, false).rows.map(row => row.key)).toEqual(['local']);
    const view = deviceSourcesView(targets, 'env1', true, false, false);
    expect(view.rows[1]).toMatchObject({ key: 'eas:cloud', title: 'Cloud iPhone', kind: 'EAS', description: 'Up to 1 session at once, each stopped after 40 min or 5 idle min',
      test: false, edit: 'eas-edit', editParams: 'id=cloud&environment=env2', remove: 'eas-remove', removeParams: 'id=cloud' });
  });
});
