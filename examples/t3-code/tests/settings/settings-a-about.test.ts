// General → About's update button and Update track.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { applyCoreSetting, generalSections, resolveScope, serverContext } from '../../src/settings/settings-core';
import { DesktopUpdateChannelPersistenceError, rememberDelivery, updateConfirmation } from '../../src/settings/settings-a-about';
import { fakeClient, fakeNative } from './settings-a.test';
import { parseLocalBackendStatus } from '../../src/connections/local-backend';
import { toasts } from '../../src/shell/toast';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';

const as = (client: ReturnType<typeof fakeClient>) => client as unknown as T3Client;
const aboutRows = (client: ReturnType<typeof fakeClient>) => generalSections(as(client), serverContext(as(client), resolveScope(as(client), '', '', ''), new Map()))
  .find(section => section.id === 'about')!.rows;

describe('about: updates over the delivery facts', () => {
  test('a binary with no update store keeps Check for Updates disabled, like a disabled desktop updater; the Update track stays a select', () => {
    const client = fakeClient();
    rememberDelivery(as(client), 'embedded', false);
    const [version, track] = aboutRows(client);
    expect(version).toMatchObject({ id: 'version', value: '0.1.6', label: 'Check for Updates', value2: 'check', disabled: true, description: 'Current version of the application.' });
    // desktopUpdateChannelOf: this client is a stable release, so with nothing saved the track is Stable.
    expect(track).toMatchObject({ id: 'update-track', title: 'Update track', description: 'Use stable releases or nightly builds. Switch back anytime.', label: 'Stable', value: 'latest', disabled: false });
    expect(track!.options.map(option => [option.label, option.selected])).toEqual([['Stable', true], ['Nightly', false]]);
    // desktop-settings.json's saved choice.
    Object.assign(client, { localBackend: parseLocalBackendStatus({ desktopSettings: { updateChannel: 'nightly' } }) });
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Nightly', value: 'nightly', disabled: false });
  });
  test('a linked stream can check; its channel names the track; a staged entry is an install', () => {
    const client = fakeClient();
    Object.assign(client, { localBackend: parseLocalBackendStatus({ desktopSettings: { updateChannel: 'latest' } }) });
    rememberDelivery(as(client), 'nightly/abc123', false);
    expect(aboutRows(client)[0]).toMatchObject({ disabled: false, label: 'Check for Updates', note: 'Up to date' });
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Nightly', value: 'nightly', disabled: true });
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

// The reference with no update feed: handleUpdateChannelChange calls
// setUpdateChannel, which saves the choice (DesktopAppSettings.setUpdateChannel) and checks nothing.
describe('about: the Update track with no feed', () => {
  const desktopNative = (fail = '') => {
    const requests: Obj[] = [], settings: Obj = { localEnvironmentEnabled: true, serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443, updateChannel: 'nightly' };
    const native = { available: true, watch: () => {}, later: async (request: unknown) => {
      const { op, ...patch } = request as Obj;
      requests.push(request as Obj);
      if (op !== 'desktopSettingsSet') return { ok: true, generation: 1, value: {} };
      if (fail) return { ok: false, generation: 1, error: { kind: 'DesktopSettings', message: fail } };
      const before = settings.updateChannel;
      Object.assign(settings, patch);
      return { ok: true, generation: 1, value: { changed: settings.updateChannel !== before, settings: { ...settings } } };
    } } as unknown as Native;
    return { native, requests };
  };
  const withBackend = () => Object.assign(fakeClient(), { localBackend: parseLocalBackendStatus({ desktopSettings: { updateChannel: 'nightly' } }) });

  test('choosing a track saves it through desktopSettingsSet and the select shows it; the same track sends nothing', async () => {
    const client = withBackend(), { native, requests } = desktopNative();
    rememberDelivery(as(client), 'embedded', false);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'nightly');
    expect(requests).toEqual([]);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'latest');
    expect(requests).toEqual([{ op: 'desktopSettingsSet', updateChannel: 'latest' }]);
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Stable', value: 'latest' });
    expect(aboutRows(client)[0]).toMatchObject({ label: 'Check for Updates', disabled: true }); // still no feed to check
    await expect(applyCoreSetting(as(client), native, 'update-track:|||', 'beta')).rejects.toThrow('Unsupported update track.');
  });
  // DesktopUpdates.setChannel wraps the write failure (DesktopUpdates.ts:92-101); the renderer reads it through
  // Electron's invoke, which prefixes the channel and the error's toString (SettingsPanels.tsx:302-311).
  test('a failed save keeps the track and shows "Could not change update track" with the persistence error', async () => {
    const client = withBackend(), path = '/lane/userdata/desktop-settings.json';
    const { native } = desktopNative(`Desktop settings write failed during replace-settings-file at ${path}.`);
    rememberDelivery(as(client), 'embedded', false);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'latest');
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Nightly', value: 'nightly' });
    expect(toasts(as(client)).map(toast => [toast.kind, toast.title, toast.description, toast.stacked])).toEqual([
      ['error', 'Could not change update track',
        "Error invoking remote method 'desktop:update-set-channel': DesktopUpdateChannelPersistenceError: Failed to persist the latest desktop update channel.", true]]);
    const error = new DesktopUpdateChannelPersistenceError('nightly', new Error('write'));
    expect([error._tag, error.message, String(error)]).toEqual(['DesktopUpdateChannelPersistenceError', 'Failed to persist the nightly desktop update channel.',
      'DesktopUpdateChannelPersistenceError: Failed to persist the nightly desktop update channel.']);
  });
  test('a linked stream fixes the track: a change sends nothing', async () => {
    const client = withBackend(), { native, requests } = desktopNative();
    rememberDelivery(as(client), 'nightly/abc123', false);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'latest');
    expect(requests).toEqual([]);
  });
});

// AboutVersionTitle (T3 Code 2359643cff): the version is a button that copies it, with the anchored "Copied!".
describe('about: Copy version', () => {
  test('a copy writes the app version to the clipboard and counts on the row, so the title replays "Copied!"', async () => {
    const client = fakeClient(), sent: Obj[] = [];
    const native = { available: true, watch() {}, async later(request: unknown) { sent.push(request as Obj); return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
    expect(aboutRows(client)[0]).toMatchObject({ id: 'version', amount: 0 });
    expect(await applyCoreSetting(as(client), native, 'version:copy|||', '')).toBe('');
    expect(sent.map(request => [request.op, request.text])).toEqual([['copyText', '0.1.6']]);
    expect(aboutRows(client)[0]!.amount).toBe(1);
    await applyCoreSetting(as(client), native, 'version:copy|||', '');
    expect(aboutRows(client)[0]!.amount).toBe(2);
  });
  test('a failed copy says "Could not copy version" and does not count', async () => {
    const client = fakeClient();
    const native = { available: true, watch() {}, async later() { return { ok: false, generation: 0, error: { kind: 'Clipboard', message: 'The pasteboard refused.', uncertain: false } }; } } as unknown as Native;
    await applyCoreSetting(as(client), native, 'version:copy|||', '');
    expect(aboutRows(client)[0]!.amount).toBe(0);
    expect(toasts(as(client)).find(toast => toast.title === 'Could not copy version')).toMatchObject({ kind: 'error', description: 'The pasteboard refused.' });
  });
  test('the title is a muted inline button named for the version, with the Copy version tooltip', async () => {
    const source = await Bun.file(new URL('../../src/settings/settings-a-about.contract', import.meta.url)).text();
    expect(source).toContain('aria-label=`Copy version ${row.value}`');
    expect(source).toContain('text "Copy version"');
    expect(source).toContain('text "Copied!"');
  });
});
