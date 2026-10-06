// Lane settings-a: General → About's update button and Update track.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { applyCoreSetting, generalSections, resolveScope, serverContext } from './settings-core';
import { rememberDelivery, updateConfirmation } from './settings-a-about';
import { fakeClient, fakeNative } from './settings-a.test';

const as = (client: ReturnType<typeof fakeClient>) => client as unknown as T3Client;
const aboutRows = (client: ReturnType<typeof fakeClient>) => generalSections(as(client), serverContext(as(client), resolveScope(as(client), '', '', ''), new Map()))
  .find(section => section.id === 'about')!.rows;

describe('about: updates over the delivery facts', () => {
  test('a binary with no update store keeps Check for Updates disabled, like a disabled desktop updater', () => {
    const client = fakeClient();
    rememberDelivery(as(client), 'embedded', false);
    const [version, track] = aboutRows(client);
    expect(version).toMatchObject({ id: 'version', value: '0.0.46-nightly.20261004.1', label: 'Check for Updates', value2: 'check', disabled: true, description: 'Current version of the application.' });
    expect(track).toMatchObject({ id: 'update-track', title: 'Update track', description: 'Use stable releases or nightly builds. Switch back anytime.', label: 'Stable', disabled: true });
    expect(track!.options.map(option => [option.label, option.selected])).toEqual([['Stable', true], ['Nightly', false]]);
  });
  test('a linked stream can check; its channel names the track; a staged entry is an install', () => {
    const client = fakeClient();
    rememberDelivery(as(client), 'nightly/abc123', false);
    expect(aboutRows(client)[0]).toMatchObject({ disabled: false, label: 'Check for Updates', note: 'Up to date' });
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Nightly', value: 'nightly' });
    rememberDelivery(as(client), 'stable/abc123', true);
    expect(aboutRows(client)[0]).toMatchObject({ label: 'Install', value2: 'install', description: 'Update available.', note: 'Update downloaded. Click to restart and install.' });
  });
  test('Install asks first; cancel and a vanished entry clear the question', async () => {
    const client = fakeClient();
    rememberDelivery(as(client), 'stable/abc', true);
    expect(updateConfirmation(as(client))).toBe(false);
    await applyCoreSetting(as(client), fakeNative, 'version:install|||', '');
    expect(updateConfirmation(as(client))).toBe(true);
    await applyCoreSetting(as(client), fakeNative, 'update:cancel|||', '');
    expect(updateConfirmation(as(client))).toBe(false);
    await applyCoreSetting(as(client), fakeNative, 'version:install|||', '');
    rememberDelivery(as(client), 'stable/abc', false);
    expect(updateConfirmation(as(client))).toBe(false);
    await expect(applyCoreSetting(as(client), fakeNative, 'version:install|||', '')).rejects.toThrow('No downloaded update is waiting to install.');
  });
});
