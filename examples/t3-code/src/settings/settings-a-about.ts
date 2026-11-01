// General → About: the Version row's copy and update buttons and the Update track row.
// Reference SettingsPanels.tsx AboutVersionSection, AboutVersionTitle.tsx and
// desktopUpdate.logic.ts. The app's own delivery facts (LLP 1030 D7, the
// `exactDelivery` resource) stand in for the Electron updater's state: a binary
// with no update store linked answers the `embedded` stream, which is the
// reference's disabled updater (Check for Updates, disabled, "Up to date"); a
// staged entry is a downloaded update ("Install", after a confirmation). With no
// store, the Update track is the reference's no-feed select:
// enabled, it saves desktop-settings.json's `updateChannel`
// (DesktopUpdates.setChannel: persist, then a base state that checks nothing) and
// shows what it saved; nothing is checked, downloaded or installed.
import type { T3Client } from '../client/client';
import { coreRow, type CoreRow } from './settings-core';
import { ClientError, type Native } from '../client/protocol';
import { obj } from '../client/domain';
import { mobileAppRow } from './settings-mobile-beta';
import { CLIENT_VERSION } from '../connections/connections';
import { defaultUpdateChannel, writeDesktopSettings, type DesktopUpdateChannel } from '../connections/local-backend';
import { letGo } from '../client/let-go';
import { pushToast } from '../shell/toast';
import { announce } from '../shell/announce';
import { updateEverythingMachines } from '../sidebar/update-everything';
import { awaitsTrackSwitch, channelSwitchLabel } from '../sidebar/update-everything-logic';

export type DeliveryFacts = { stream: string; staged: boolean };
const facts = new WeakMap<T3Client, DeliveryFacts>();
const confirming = new WeakSet<T3Client>();
/** The version's copies so far: the row's `amount`, each of which replays the title's "Copied!". */
const versionCopies = new WeakMap<T3Client, number>();

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

/**
 * AboutVersionSection's unswitchedMachines: this app when its updater runs off the chosen track, and each connected
 * machine this session maintains that a switch to the track could still move (awaitsTrackSwitch).
 */
export function unswitchedMachines(client: T3Client, updaterOn: boolean, channel: DesktopUpdateChannel): number {
  const appOffTrack = updaterOn && defaultUpdateChannel(CLIENT_VERSION) !== channel;
  return (appOffTrack ? 1 : 0) + updateEverythingMachines(client).filter(machine => machine.connected && machine.canMaintain && awaitsTrackSwitch(machine, channel)).length;
}

/** The About rows: Version (with its update button) and Update track. */
export function aboutRows(client: T3Client, version: string): CoreRow[] {
  const delivery = factsOf(client), enabled = updatesEnabled(delivery), channel = enabled ? updateChannel(delivery) : savedChannel(client);
  const unswitched = unswitchedMachines(client, enabled, channel);
  const options = [['latest', 'Stable'], ['nightly', 'Nightly']].map(([value, label]) => ({ id: value!, value: value!, label: label!, detail: '', icon: '', selected: value === channel, disabled: false }));
  return [
    coreRow('version', 'Version', delivery.staged ? 'Update available.' : 'Current version of the application.', 'version', {
      value: version || 'Unknown', amount: versionCopies.get(client) ?? 0,
      // resolveDesktopUpdateButtonAction: install once staged; otherwise check (the contract shows Checking… while one runs).
      label: delivery.staged ? 'Install' : 'Check for Updates', value2: delivery.staged ? 'install' : 'check',
      disabled: !enabled && !delivery.staged,
      note: delivery.staged ? 'Update downloaded. Click to restart and install.' : 'Up to date',
    }),
    // No store: the saved preference, switchable (disabled only while a change runs, the command's busy).
    // A linked stream's channel was fixed when the binary was delivered: shown, not switchable here.
    // With machines on the other track, the description counts them and `label2` is its Review link.
    coreRow('update-track', 'Update track', unswitched > 0 ? `${unswitched === 1 ? '1 machine still runs' : `${unswitched} machines still run`} ${channelSwitchLabel(channel === 'nightly' ? 'latest' : 'nightly')}.`
      : 'Switches this app and every connected machine whose database the target release supports.', 'select', {
      value: channel, label: channel === 'nightly' ? 'Nightly' : 'Stable', options, width: 160, disabled: enabled, menuWidth: 0, label2: unswitched > 0 ? 'Review' : '',
    }),
    mobileAppRow(),
  ];
}

/** settings-core row `version:copy` (AboutVersionTitle useCopyToClipboard): the app's own version, or the error toast. */
export async function copyVersion(client: T3Client, native: Native): Promise<string> {
  try {
    const response = obj(await native.later({ op: 'copyText', text: CLIENT_VERSION, generation: client.generation }));
    if (response.ok !== true) throw new ClientError(String(obj(response.error).message || 'Could not copy to the clipboard.'));
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not copy version', description: error instanceof Error ? error.message : String(error), stacked: true });
    return '';
  }
  versionCopies.set(client, (versionCopies.get(client) ?? 0) + 1);
  // The anchored toast's live region says it once; the drawn "Copied!" stays out of the accessibility tree.
  announce(client, 'Copied!', 'version-copy');
  return '';
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

/** DesktopUpdates.ts DesktopUpdateChannelPersistenceError: setChannel's wrap of a failed settings write. */
export class DesktopUpdateChannelPersistenceError extends Error {
  readonly _tag = 'DesktopUpdateChannelPersistenceError';
  override readonly name = 'DesktopUpdateChannelPersistenceError';
  constructor(readonly channel: DesktopUpdateChannel, readonly cause: unknown) { super(`Failed to persist the ${channel} desktop update channel.`); }
}

/**
 * What the reference's renderer reads when `bridge.setUpdateChannel` rejects: Electron's `ipcRenderer.invoke` wraps the
 * main process's `error.toString()` (the tagged error's name, then its message) in its own Error.
 */
export const setUpdateChannelFailure = (error: Error): string => `Error invoking remote method 'desktop:update-set-channel': ${String(error)}`;

/**
 * settings-core row `update-track` (handleUpdateChannelChange): the same track does nothing; another is saved
 * through `desktopSettingsSet` (DesktopAppSettings.setUpdateChannel, which also marks it the user's choice).
 * A failed write changes nothing and says so in the reference's toast: DesktopUpdates.setChannel's persistence
 * error, as the renderer reads it across the IPC.
 */
export async function updateTrackCommand(client: T3Client, native: Native, value: string): Promise<string> {
  if (value !== 'latest' && value !== 'nightly') throw new ClientError('Unsupported update track.');
  if (updatesEnabled(factsOf(client)) || value === savedChannel(client)) return '';
  try {
    await writeDesktopSettings(native, { updateChannel: value }, client).catch((cause: unknown) => {
      throw cause instanceof ClientError && cause.kind === 'DesktopSettings' ? new DesktopUpdateChannelPersistenceError(value, cause) : cause;
    });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not change update track', description: error instanceof Error ? setUpdateChannelFailure(error) : 'Update track change failed.', stacked: true });
  }
  return '';
}
