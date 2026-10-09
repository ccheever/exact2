// Lane settings-a: General → About's update button and Update track.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { applyCoreSetting, generalSections, resolveScope, serverContext } from './settings-core';
import { rememberDelivery, updateConfirmation } from './settings-a-about';
import { fakeClient, fakeNative } from './settings-a.test';
import { parseLocalBackendStatus } from './local-backend';
import { toasts } from './toast';
import type { Obj } from './domain';
import type { Native } from './protocol';

const as = (client: ReturnType<typeof fakeClient>) => client as unknown as T3Client;
const aboutRows = (client: ReturnType<typeof fakeClient>) => generalSections(as(client), serverContext(as(client), resolveScope(as(client), '', '', ''), new Map()))
  .find(section => section.id === 'about')!.rows;

describe('about: updates over the delivery facts', () => {
  test('a binary with no update store keeps Check for Updates disabled, like a disabled desktop updater; the Update track stays a select', () => {
    const client = fakeClient();
    rememberDelivery(as(client), 'embedded', false);
    const [version, track] = aboutRows(client);
    expect(version).toMatchObject({ id: 'version', value: '0.0.46-nightly.20261004.1', label: 'Check for Updates', value2: 'check', disabled: true, description: 'Current version of the application.' });
    // resolveDefaultDesktopUpdateChannel: this client is a Nightly, so with nothing saved the track is Nightly.
    expect(track).toMatchObject({ id: 'update-track', title: 'Update track', description: 'Use stable releases or nightly builds. Switch back anytime.', label: 'Nightly', value: 'nightly', disabled: false });
    expect(track!.options.map(option => [option.label, option.selected])).toEqual([['Stable', false], ['Nightly', true]]);
    // desktop-settings.json's saved choice.
    Object.assign(client, { localBackend: parseLocalBackendStatus({ desktopSettings: { updateChannel: 'latest' } }) });
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Stable', value: 'latest', disabled: false });
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

// The reference with no update feed (blocked-desktop-update-controls, option a): handleUpdateChannelChange calls
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
  test('a failed save keeps the track and shows "Could not change update track"', async () => {
    const client = withBackend(), path = '/lane/userdata/desktop-settings.json';
    const { native } = desktopNative(`Desktop settings write failed during replace-settings-file at ${path}.`);
    rememberDelivery(as(client), 'embedded', false);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'latest');
    expect(aboutRows(client)[1]).toMatchObject({ label: 'Nightly', value: 'nightly' });
    expect(toasts(as(client)).map(toast => [toast.kind, toast.title, toast.description, toast.stacked])).toEqual([
      ['error', 'Could not change update track', `Desktop settings write failed during replace-settings-file at ${path}.`, true]]);
  });
  test('a linked stream fixes the track: a change sends nothing', async () => {
    const client = withBackend(), { native, requests } = desktopNative();
    rememberDelivery(as(client), 'nightly/abc123', false);
    await applyCoreSetting(as(client), native, 'update-track:|||', 'latest');
    expect(requests).toEqual([]);
  });
});

// SidebarUpdatePill with no feed (SidebarUpdatePill.tsx:111-160, canCheckForUpdate false): the disabled round control
// at the end of SidebarUtilityMenu, which the thread sidebar, the legacy sidebar and the Settings nav each draw.
describe('about: the no-feed "Check for updates" control', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
  test('it is disabled, labelled, and in all three footers with its side-top tooltip', async () => {
    const pill = (await source('sidebar-icons.contract')).split('\ncomponent SidebarUpdatePill\n')[1]!;
    expect(pill).toContain('button disabled=true aria-label="Check for updates" cursor="not-allowed" testId=testId');
    expect(pill).toContain('opacity=0.6');
    expect(pill).not.toContain('press=');
    for (const file of ['sidebar.contract', 'legacy-sidebar.contract']) {
      expect(await source(file)).toContain('SidebarTip(tip="sidebar-update", hoverCard=hoverCard)\n            SidebarUpdatePill(color="light-dark(#a8a8ae, #545454)", testId="sidebar-check-updates")');
    }
    expect(await source('r4-polish-tip.contract')).toContain('TipCard(label="Check for updates", shown=(hoverId == "sidebar-update"))');
    expect(await source('settings-core.contract')).toContain('box hover=updateTip("settings-update") margin-left="auto" flex-shrink=0 display="flex" testId="tip-settings-update"\n          SidebarUpdatePill(color=pal.sidebarIcon, testId="settings-check-updates")');
    expect(await source('app-settings.contract')).toContain('TipCard(label="Check for updates", shown=updateTipHover)');
  });
});
