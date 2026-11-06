// Settings › Devices › Device sources › Machine over SSH: the editor and the hosts' commands.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { connectionKey, deviceHostsCommand, hostChecks, hostEditorView, parseHost, updateDeviceHosts } from '../../src/device/settings-a-hosts';
import { toasts } from '../../src/shell/toast';

const mini = { id: 'mini', label: 'Mac mini', target: 'me@mini' };
describe('device hosts', () => {
  test('a draft parses as SshDeviceHostConfig', () => {
    expect(parseHost({ id: 'h1', label: ' Mac mini ', target: ' me@mini ', identityFile: ' ', port: '' })).toEqual({ id: 'h1', label: 'Mac mini', target: 'me@mini' });
    expect(parseHost({ id: 'h1', label: 'x', target: 'me@mini', identityFile: '~/.ssh/id', port: '2222' })).toEqual({ id: 'h1', label: 'x', target: 'me@mini', identityFile: '~/.ssh/id', port: 2222 });
    expect(parseHost({ id: 'h1', label: 'x', target: '-oProxy', port: '' })).toBe('Enter an SSH target such as user@host or an SSH alias.');
    expect(parseHost({ id: 'h1', label: 'x', target: 'a b', port: '' })).toBe('Enter an SSH target such as user@host or an SSH alias.');
    expect(parseHost({ id: 'h1', label: 'x', target: 'mini', port: '70000' })).toBe('Use a port between 1 and 65535.');
    expect(parseHost({ id: 'local', label: 'x', target: 'mini' })).toBe('That host id is not valid.');
    expect(connectionKey({ target: ' mini ', port: 22, identityFile: '' })).toBe('mini|22|');
  });
  test('updateDeviceHosts edits by id, then by destination, and removes', () => {
    const other = { id: 'other', label: 'Other', target: 'other' };
    expect(updateDeviceHosts([mini, other], { ...mini, label: 'Renamed' }, false)).toEqual([{ ...mini, label: 'Renamed' }, other]);
    expect(updateDeviceHosts([mini], { id: 'fresh', label: 'Same', target: 'me@mini' }, false)).toEqual([{ id: 'mini', label: 'Same', target: 'me@mini' }]);
    expect(updateDeviceHosts([mini, other], mini, true)).toEqual([other]);
    expect(updateDeviceHosts([], mini, false)).toEqual([mini]);
    expect(() => updateDeviceHosts([mini, { ...mini, id: 'dup' }], { id: 'new', label: 'n', target: 'me@mini' }, false)).toThrow('Multiple hosts match this SSH destination.');
  });
  test('the editor opens, checks and saves; a row tests, retries, refreshes and removes', async () => {
    const requests: [string, Obj][] = [];
    const settings: Obj = { deviceHosts: [mini] };
    const replies: Record<string, Obj | Error> = { 'device.testHost': { id: 'mini', kind: 'ssh', label: 'Mac mini', platforms: [{ platform: 'ios', available: true }, { platform: 'android', available: false }], hubInstalled: false, agentDeviceInstalled: false } };
    const client = { ready: true, environmentId: 'env1', config: { environment: { label: 'Studio' }, settings },
      restAccess: () => ({ request: async (method: string, payload: Obj) => {
        requests.push([method, payload]);
        if (method === 'server.getConfig') return { settings };
        if (method === 'server.updateSettings' && !(replies[method] instanceof Error)) { Object.assign(settings, payload.patch as Obj); return settings; }
        const reply = replies[method]; if (reply instanceof Error) throw reply; return reply ?? {};
      } }) } as unknown as T3Client;
    const native = { available: true } as unknown as Native;
    // A project scope cannot add; Machine over SSH opens a blank editor.
    await expect(deviceHostsCommand(client, native, 'hosts-open', {}, true)).rejects.toThrow('Device hosts belong to the environment.');
    await deviceHostsCommand(client, native, 'hosts-open', {}, false);
    const editor = hostEditorView(client);
    expect(editor).toMatchObject({ open: true, title: 'Add device host', description: 'Connect from Studio. Hosts on the same machine are skipped.', label: '', target: '' });
    // Test connection from the dialog: one environment, its platforms.
    await deviceHostsCommand(client, native, 'hosts-test', { id: editor.id, label: '', target: 'me@studio', identityFile: '', port: '' }, false);
    expect(requests.at(-1)).toEqual(['device.testHost', { id: editor.id, label: 'me@studio', target: 'me@studio' }]);
    expect(hostChecks(client).find(group => group.key === 'me@studio||')).toMatchObject({ checking: false, summary: 'Connection checks passed', results: [{ label: 'Studio', status: 'Connected', platforms: 'iOS available   Android unavailable' }] });
    await deviceHostsCommand(client, native, 'hosts-save', { id: editor.id, label: 'Studio box', target: 'me@studio', identityFile: '', port: '22' }, false);
    expect(requests.at(-1)).toEqual(['server.updateSettings', { patch: { deviceHosts: [mini, { id: editor.id, label: 'Studio box', target: 'me@studio', port: 22 }] } }]);
    expect(hostEditorView(client).open).toBe(false);
    // A row's Test connection reports through a toast; a failure names the environment.
    replies['device.testHost'] = new Error('Permission denied (publickey).');
    await deviceHostsCommand(client, native, 'hosts-test-row', { id: 'mini' }, false);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Mac mini: 1 of 1 environments failed', description: 'Studio: Permission denied (publickey).' });
    await deviceHostsCommand(client, native, 'hosts-retry', { id: 'mini' }, false);
    expect(requests.at(-1)).toEqual(['device.list', { retryHostId: 'mini' }]);
    await deviceHostsCommand(client, native, 'hosts-refresh', {}, false);
    expect(requests.at(-1)).toEqual(['device.list', {}]);
    await deviceHostsCommand(client, native, 'hosts-remove', { id: 'mini' }, false);
    expect((settings.deviceHosts as Obj[]).map(host => host.id)).toEqual([editor.id]);
    // A refused write names the environment and keeps the editor open.
    replies['server.updateSettings'] = new Error('Device host ids must be unique.');
    await deviceHostsCommand(client, native, 'hosts-edit', { id: editor.id }, false);
    await deviceHostsCommand(client, native, 'hosts-save', { id: editor.id, label: 'Renamed', target: 'me@studio', identityFile: '', port: '' }, false);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Device host not saved everywhere', description: 'Studio: Device host ids must be unique.' });
    expect(hostEditorView(client).open).toBe(true);
  });
});
