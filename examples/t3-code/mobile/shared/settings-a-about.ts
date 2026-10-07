// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/settings-a-about.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// General → About: the Version row's update button and the Update track row
// (lane settings-a). Reference SettingsPanels.tsx AboutVersionSection and
// desktopUpdate.logic.ts. The app's own delivery facts (LLP 1030 D7, the
// `exactDelivery` resource) stand in for the Electron updater's state: a binary
// with no update store linked answers the `embedded` stream, which is the
// reference's disabled updater (Check for Updates, disabled, "Up to date"); a
// staged entry is a downloaded update ("Install", after a confirmation).
import type { T3Client } from './client';
import { coreRow, type CoreRow } from './settings-core';
import { ClientError } from './protocol';
import { mobileBetaRow, showMobileBeta } from './settings-mobile-beta';
import { CLIENT_VERSION } from './connections';

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

/** The About rows: Version (with its update button) and Update track. */
export function aboutRows(client: T3Client, version: string): CoreRow[] {
  const delivery = factsOf(client), enabled = updatesEnabled(delivery), channel = updateChannel(delivery);
  const options = [['latest', 'Stable'], ['nightly', 'Nightly']].map(([value, label]) => ({ id: value!, value: value!, label: label!, detail: '', icon: '', selected: value === channel, disabled: false }));
  return [
    coreRow('version', 'Version', delivery.staged ? 'Update available.' : 'Current version of the application.', 'version', {
      value: version || 'Unknown',
      // resolveDesktopUpdateButtonAction: install once staged; otherwise check (the contract shows Checking… while one runs).
      label: delivery.staged ? 'Install' : 'Check for Updates', value2: delivery.staged ? 'install' : 'check',
      disabled: !enabled && !delivery.staged,
      note: delivery.staged ? 'Update downloaded. Click to restart and install.' : 'Up to date',
    }),
    // The track is the stream's channel, fixed when the binary was delivered: shown, not switchable here.
    coreRow('update-track', 'Update track', 'Use stable releases or nightly builds. Switch back anytime.', 'select', {
      value: channel, label: channel === 'nightly' ? 'Nightly' : 'Stable', options, width: 160, disabled: true, menuWidth: 0,
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
