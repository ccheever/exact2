// General → About: the Version row's update button and the Update track row
// (lane settings-a). Reference SettingsPanels.tsx AboutVersionSection and
// desktopUpdate.logic.ts. The app's own delivery facts (LLP 1030 D7, the
// `exactDelivery` resource) stand in for the Electron updater's state: a binary
// with no update store linked answers the `embedded` stream, which is the
// reference's disabled updater (Check for Updates, disabled, "Up to date"); a
// staged entry is a downloaded update ("Install", after a confirmation). With no
// store, the Update track is the reference's no-feed select (blocked-desktop-update-
// controls, option a): enabled, it saves desktop-settings.json's `updateChannel`
// (DesktopUpdates.setChannel: persist, then a base state that checks nothing) and
// shows what it saved; nothing is checked, downloaded or installed (X40 stays closed).
import type { T3Client } from './client';
import { coreRow, type CoreRow } from './settings-core';
import { ClientError, type Native } from './protocol';
import { mobileBetaRow, showMobileBeta } from './settings-mobile-beta';
import { CLIENT_VERSION } from './connections';
import { defaultUpdateChannel, writeDesktopSettings, type DesktopUpdateChannel } from './local-backend';
import { letGo } from './let-go';
import { pushToast } from './toast';

export type DeliveryFacts = { stream: string; staged: boolean };
const facts = new WeakMap<T3Client, DeliveryFacts>();
const confirming = new WeakSet<T3Client>();

/** settingsCore's delivery arguments, remembered for the General rows. */
export function rememberDelivery(client: T3Client, stream: string, staged: boolean): void {
  facts.set(client, { stream: stream || 'embedded', staged });
  if (!staged) confirming.delete(client);
}
const factsOf = (client: T3Client): DeliveryFacts => facts.get(client) ?? { stream: 'embedded', staged: false };
/** Whether an update store answers this binary: the embedded stream is the no-store answer. */
export const updatesEnabled = (delivery: DeliveryFacts) => delivery.stream !== '' && delivery.stream !== 'embedded';
/** A stream is `<channel>/<compatibilityId>`; nightly is the only other track the reference names. */
export const updateChannel = (delivery: DeliveryFacts) => delivery.stream.split('/')[0] === 'nightly' ? 'nightly' : 'latest';
/** The saved track (`DesktopSettings.updateChannel`), the reference's `updateState.channel` while it has no feed. */
const savedChannel = (client: T3Client): DesktopUpdateChannel => client.localBackend?.settings.updateChannel ?? defaultUpdateChannel(CLIENT_VERSION);

/** The About rows: Version (with its update button) and Update track. */
export function aboutRows(client: T3Client, version: string): CoreRow[] {
  const delivery = factsOf(client), enabled = updatesEnabled(delivery), channel = enabled ? updateChannel(delivery) : savedChannel(client);
  const options = [['latest', 'Stable'], ['nightly', 'Nightly']].map(([value, label]) => ({ id: value!, value: value!, label: label!, detail: '', icon: '', selected: value === channel, disabled: false }));
  return [
    coreRow('version', 'Version', delivery.staged ? 'Update available.' : 'Current version of the application.', 'version', {
      value: version || 'Unknown',
      // resolveDesktopUpdateButtonAction: install once staged; otherwise check (the contract shows Checking… while one runs).
      label: delivery.staged ? 'Install' : 'Check for Updates', value2: delivery.staged ? 'install' : 'check',
      disabled: !enabled && !delivery.staged,
      note: delivery.staged ? 'Update downloaded. Click to restart and install.' : 'Up to date',
    }),
    // No store: the saved preference, switchable (disabled only while a change runs, the command's busy).
    // A linked stream's channel was fixed when the binary was delivered: shown, not switchable here.
    coreRow('update-track', 'Update track', 'Use stable releases or nightly builds. Switch back anytime.', 'select', {
      value: channel, label: channel === 'nightly' ? 'Nightly' : 'Stable', options, width: 160, disabled: enabled, menuWidth: 0,
    }),
    // f1dcd93931: Mobile app after the version/update rows on a Nightly build or the Nightly track.
    ...(showMobileBeta(CLIENT_VERSION, channel) ? [mobileBetaRow(client)] : []),
  ];
}

/** The install confirmation the root dialog shows (getDesktopUpdateInstallConfirmationMessage). */
export function updateConfirmation(client: T3Client): boolean { return confirming.has(client) && factsOf(client).staged; }

/** settings-core rows `version:install` (ask first) and `update:cancel`. */
export function updateCommand(client: T3Client, row: string, part: string): string {
  if (row === 'update' && part === 'cancel') { confirming.delete(client); return ''; }
  if (row === 'version' && part === 'install') {
    if (!factsOf(client).staged) throw new ClientError('No downloaded update is waiting to install.');
    confirming.add(client);
    return '';
  }
  throw new ClientError('Unsupported update action.');
}

/**
 * settings-core row `update-track` (handleUpdateChannelChange): the same track does nothing; another is saved
 * through `desktopSettingsSet` (DesktopAppSettings.setUpdateChannel, which also marks it the user's choice).
 * A failed write changes nothing and says so in the reference's toast.
 */
export async function updateTrackCommand(client: T3Client, native: Native, value: string): Promise<string> {
  if (value !== 'latest' && value !== 'nightly') throw new ClientError('Unsupported update track.');
  if (updatesEnabled(factsOf(client)) || value === savedChannel(client)) return '';
  try {
    await writeDesktopSettings(native, { updateChannel: value }, client);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not change update track', description: error instanceof Error && error.message ? error.message : 'Update track change failed.', stacked: true });
  }
  return '';
}
